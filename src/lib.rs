//! Tech Reborn for Horizon, slice 1: the solid fuel generator, the copper cable and the
//! electric furnace — energy in, energy moved, energy spent.
//!
//! Tech Reborn 6.2.1 (`TechReborn/TechReborn` @ `140bc0fc`, MIT) is a NeoForge mod; this is a
//! clean-room Rust rewrite of the behaviour recorded in `FINDINGS.md`, not a translation.
//!
//! The numbers live in [`runtime`], which has no `mod_api` types and is tested with `cargo test`
//! alone. This file is the glue: the three block entity Parts, the energy buffers and item
//! containers the three blocks declare, the machine's placement facing, and the `/techreborn`
//! harness commands.
//!
//! Energy lives in each block entity's saved compound as the int `energy` (the key
//! `ModEnergyBuffer` documents), so it survives a save and each block's own tick reads it
//! there. A ticking block entity is out of the world while its tick runs, so a block reads and
//! writes its own buffer through the entity it is handed and reaches its neighbours through the
//! `horizon:energy` capability.

pub mod runtime;

use mod_api::*;

use crate::runtime::{
    CABLE_CAPACITY, CABLE_RATE, Endpoint, FURNACE_CAPACITY, FURNACE_MAX_INPUT, FuelSlot, Furnace,
    FurnaceStep, GENERATOR_CAPACITY, GENERATOR_MAX_OUTPUT, Generator, Move, cable_move_limit,
    cable_plan, clamp_energy, facing_for_yaw, furnace_tick,
};

/// The three blocks of the slice (`TRContent.Machine.SOLID_FUEL_GENERATOR`,
/// `TRContent.Cables.COPPER`, `TRContent.Machine.ELECTRIC_FURNACE`).
const GENERATOR: &str = "techreborn:solid_fuel_generator";
const CABLE: &str = "techreborn:copper_cable";
const FURNACE: &str = "techreborn:electric_furnace";

/// `SolidFuelGeneratorBlockEntity`: the fuel slot, then the battery slot.
const GENERATOR_SLOTS: usize = 2;
/// `ElectricFurnaceBlockEntity`: input 0, output 1, battery 2.
const INPUT_SLOT: usize = 0;
const OUTPUT_SLOT: usize = 1;
/// The three slots the vanilla furnace menu shows.
const MENU_SLOTS: usize = 3;
/// The vanilla furnace menu's four data slots: the flame (lit) then the arrow
/// (`progress / total`).
const MENU_DATA: usize = 4;

/// Upstream's saved keys: `energy` (the buffer's amount), `BurnTime` and `TotalBurnTime`, and
/// the furnace's `cookTime` / `cookTimeTotal`.
const KEY_ENERGY: &str = "energy";
const KEY_BURN_TIME: &str = "BurnTime";
const KEY_TOTAL_BURN: &str = "TotalBurnTime";
const KEY_COOK_TIME: &str = "cookTime";
const KEY_COOK_TOTAL: &str = "cookTimeTotal";

/// The one `impl Hooks` at the crate root.
#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Attach the three block entities, their buffers, their containers and the machine
    /// facing to the three blocks of the carried content.
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let generator_block = r.block_key(GENERATOR);
        let cable_block = r.block_key(CABLE);
        let furnace_block = r.block_key(FURNACE);

        r.block_energy(&generator_block, generator_buffer());
        r.block_energy(&cable_block, cable_buffer());
        r.block_energy(&furnace_block, furnace_buffer());
        r.block_container(&generator_block, generator_container());
        r.block_container(&furnace_block, furnace_container());
        // Each machine is one Part attached once, to its block entity type:
        // `block_entity_part` also puts the block half of that part on every block of the type
        // (`expand_block_entity_parts`), which is what makes the placement facing below — a
        // block-value hook — reach the block. Calling `block_part` for the same type as well
        // attaches it twice, and the engine refuses the whole package ("… is attached to … twice").
        let generator = r.block_entity_type(GENERATOR).blocks(&[GENERATOR]).build();
        let cable = r.block_entity_type(CABLE).blocks(&[CABLE]).build();
        let furnace = r.block_entity_type(FURNACE).blocks(&[FURNACE]).build();
        r.block_entity_part(&generator, GeneratorPart)
            .expect("a block entity type this mod declares takes its own part");
        r.block_entity_part(&cable, CablePart)
            .expect("a block entity type this mod declares takes its own part");
        r.block_entity_part(&furnace, FurnacePart)
            .expect("a block entity type this mod declares takes its own part");
    }

    /// `/techreborn info <pos>` and `/techreborn energy <pos> <amount>`: upstream has no
    /// commands; these exist so a harness can read and set a buffer without a GUI.
    fn register_commands(
        &self,
        _: &mut dyn ServerHost,
        event: &mut mod_api::RegisterCommandsEvent<'_>,
    ) {
        event
            .commands
            .push(Command::new("techreborn info <pos:block_pos>"));
        event.commands.push(Command::op(
            "techreborn energy <pos:block_pos> <amount:integer>",
            2,
        ));
    }

    /// The executor of both `/techreborn` usages.
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::RunModCommandEvent<'_>,
    ) -> Flow {
        if event.command.literals.first().map(String::as_str) != Some("techreborn") {
            return Flow::Continue;
        }
        let Some(pos) = position_argument(event.command.argument("pos")) else {
            feedback(host, "usage: /techreborn info <x> <y> <z>");
            return Flow::Cancel;
        };
        match event.command.literals.get(1).map(String::as_str) {
            Some("info") => {
                let line = readout(host, pos);
                feedback(host, &line);
            }
            Some("energy") => {
                let Some(amount) = integer_argument(event.command.argument("amount")) else {
                    feedback(host, "usage: /techreborn energy <x> <y> <z> <amount>");
                    return Flow::Cancel;
                };
                let line = set_energy(host, pos, amount);
                feedback(host, &line);
            }
            _ => feedback(
                host,
                "usage: /techreborn info <x> <y> <z> | /techreborn energy <x> <y> <z> <amount>",
            ),
        }
        Flow::Cancel
    }
}

// ---------------------------------------------------------------------------
// The three parts: a block entity each, a tick each, and a machine facing
// ---------------------------------------------------------------------------

/// The solid fuel generator (`SolidFuelGeneratorBlockEntity`).
struct GeneratorPart;

#[mod_api::horizon_part]
impl BlockPart for GeneratorPart {
    /// `BlockMachineBase.setPlacedBy`: the machine faces its placer.
    fn state_for_placement(
        &self,
        level: &dyn LevelHost,
        event: &mut mod_api::StateForPlacementEvent<'_>,
    ) -> Option<Option<BlockState>> {
        facing_state(level, event)
    }
}

#[mod_api::horizon_part]
impl mod_api::BlockEntityPart for GeneratorPart {
    fn block_entity(
        &self,
        _: LevelSide,
        _: &mut mod_api::BlockEntityEvent<'_>,
    ) -> Option<Compound> {
        Some(Compound::new())
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut mod_api::BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    fn block_entity_tick(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::BlockEntityTickEvent<'_>,
    ) {
        generator_tick(host, event);
    }
}

/// The copper cable (`CableBlockEntity`): it has no facing and no container.
struct CablePart;

#[mod_api::horizon_part]
impl BlockPart for CablePart {}

#[mod_api::horizon_part]
impl mod_api::BlockEntityPart for CablePart {
    fn block_entity(
        &self,
        _: LevelSide,
        _: &mut mod_api::BlockEntityEvent<'_>,
    ) -> Option<Compound> {
        Some(Compound::new())
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut mod_api::BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    fn block_entity_tick(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::BlockEntityTickEvent<'_>,
    ) {
        cable_tick(host, event);
    }
}

/// The electric furnace (`ElectricFurnaceBlockEntity`).
struct FurnacePart;

#[mod_api::horizon_part]
impl BlockPart for FurnacePart {
    fn state_for_placement(
        &self,
        level: &dyn LevelHost,
        event: &mut mod_api::StateForPlacementEvent<'_>,
    ) -> Option<Option<BlockState>> {
        facing_state(level, event)
    }
}

#[mod_api::horizon_part]
impl mod_api::BlockEntityPart for FurnacePart {
    fn block_entity(
        &self,
        _: LevelSide,
        _: &mut mod_api::BlockEntityEvent<'_>,
    ) -> Option<Compound> {
        Some(Compound::new())
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut mod_api::BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    fn block_entity_tick(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::BlockEntityTickEvent<'_>,
    ) {
        furnace_tick_of(host, event);
    }
}

// ---------------------------------------------------------------------------
// What the blocks declare: energy buffers and item containers
// ---------------------------------------------------------------------------

/// Each block's energy buffer, as `ModEnergyBuffer` states it: the generator holds 10,000 E and
/// only gives (10 E a tick of burning, at most 32 out of one call), the cable holds 512 E and
/// moves 128 either way, the furnace holds 1,000 E and only takes (32 a call).
fn generator_buffer() -> ModEnergyBuffer {
    ModEnergyBuffer {
        capacity: GENERATOR_CAPACITY,
        max_insert: 0,
        max_extract: GENERATOR_MAX_OUTPUT,
        insert_faces: [false; 6],
        extract_faces: [true; 6],
    }
}

fn cable_buffer() -> ModEnergyBuffer {
    ModEnergyBuffer {
        capacity: CABLE_CAPACITY,
        max_insert: CABLE_RATE,
        max_extract: CABLE_RATE,
        insert_faces: [true; 6],
        extract_faces: [true; 6],
    }
}

fn furnace_buffer() -> ModEnergyBuffer {
    ModEnergyBuffer {
        capacity: FURNACE_CAPACITY,
        max_insert: FURNACE_MAX_INPUT,
        max_extract: 0,
        insert_faces: [true; 6],
        extract_faces: [false; 6],
    }
}

/// The generator's two slots: the fuel slot and a battery slot (`RebornInventory(2)`). It has
/// no screen in this slice, so it opens no menu.
fn generator_container() -> ModContainer {
    ModContainer {
        slots: GENERATOR_SLOTS,
        faces: std::array::from_fn(|_| vec![0, 1]),
        inputs: vec![0],
        outputs: Vec::new(),
        menu: None,
    }
}

/// The furnace's three slots (`RebornInventory(3)`: input, output, battery), shown by the
/// vanilla furnace menu.
fn furnace_container() -> ModContainer {
    ModContainer {
        slots: MENU_SLOTS,
        faces: std::array::from_fn(|_| vec![INPUT_SLOT, OUTPUT_SLOT, 2]),
        inputs: vec![INPUT_SLOT],
        outputs: vec![OUTPUT_SLOT],
        menu: Some("minecraft:furnace".to_owned()),
    }
}

/// The `facing` a machine takes when it is placed, as `BlockMachineBase`'s
/// `placer.getDirection().getOpposite()` gives it: the machine faces the player, so a look
/// along +Z (yaw 0, south) leaves it facing north.
fn facing_state(
    level: &dyn LevelHost,
    event: &mut mod_api::StateForPlacementEvent<'_>,
) -> Option<Option<BlockState>> {
    let facing = facing_for_yaw(event.context.yaw);
    level
        .state_with_property(event.state, "facing", facing)
        .map(Some)
}

// ---------------------------------------------------------------------------
// The ticks
// ---------------------------------------------------------------------------

/// `SolidFuelGeneratorBlockEntity.tick`: burn a step of fuel into the buffer, then load the
/// next item when the steps run out.
fn generator_tick(host: &mut dyn ServerHost, event: &mut mod_api::BlockEntityTickEvent<'_>) {
    let mut generator = Generator::new();
    generator.energy = be_int(&event.entity.nbt, KEY_ENERGY).clamp(0, GENERATOR_CAPACITY);
    generator.burn_time = be_int(&event.entity.nbt, KEY_BURN_TIME);
    generator.total_burn = be_int(&event.entity.nbt, KEY_TOTAL_BURN);
    if event.entity.items.len() < GENERATOR_SLOTS {
        event.entity.items.resize(GENERATOR_SLOTS, None);
    }
    {
        let mut fuel = SlotFuel {
            host: &mut *host,
            items: &mut event.entity.items,
        };
        generator.tick(&mut fuel);
    }
    set_be_int(&mut event.entity.nbt, KEY_ENERGY, generator.energy);
    set_be_int(&mut event.entity.nbt, KEY_BURN_TIME, generator.burn_time);
    set_be_int(&mut event.entity.nbt, KEY_TOTAL_BURN, generator.total_burn);
    set_active(host, event.pos, generator.active);
}

/// The generator's fuel slot: the item's smelting burn time, and the shrink (or the crafting
/// remainder) that takes it.
struct SlotFuel<'a> {
    host: &'a mut dyn ServerHost,
    items: &'a mut Vec<Option<ItemStack>>,
}

impl FuelSlot for SlotFuel<'_> {
    fn peek(&self) -> Option<u32> {
        let stack = self.items.first()?.as_ref()?;
        // The engine's `minecraft:cooking_fuel` value, 0 for an item that is no fuel.
        let ticks = self.host.item_burn_time(stack.item()).unwrap_or(0).max(0);
        Some(ticks as u32)
    }

    fn take(&mut self) {
        let Some(slot) = self.items.first_mut() else {
            return;
        };
        let Some(stack) = slot.clone() else {
            return;
        };
        match stack.clone().shrink(1) {
            Some(rest) => *slot = Some(rest),
            None => *slot = self.host.item_crafting_remainder(&stack),
        }
    }
}

/// `CableTickManager.handleCableTick` for one cable: pull from every neighbour that gives into
/// its 512 E buffer, then push into every neighbour that takes, at most 128 E per neighbour per
/// tick, sharing evenly as upstream's `min(remaining / remainingTargets, transferRate)` does.
fn cable_tick(host: &mut dyn ServerHost, event: &mut mod_api::BlockEntityTickEvent<'_>) {
    let energy = clamp_energy(be_int(&event.entity.nbt, KEY_ENERGY));
    let Some(level) = in_place_level(host) else {
        return;
    };
    let mut endpoints = Vec::with_capacity(6);
    for face in Direction::ALL {
        let at = neighbour(event.pos, face);
        let Some((stored, capacity)) = energy_of(host, level, at, face.opposite()) else {
            continue;
        };
        let space = capacity.saturating_sub(stored);
        // The energy capability answers amount and capacity, not NeoForge's per-side
        // canReceive/canExtract. Our own two machines' faces are in their declared buffers, so
        // they are exact; another mod's block is taken as willing where it has room or holds
        // energy, and the real move's answer corrects the accounting. Recorded in notes.md.
        let (can_insert, can_extract) = match block_at(host, at).as_deref() {
            Some(GENERATOR) => (false, true),
            Some(FURNACE) => (true, false),
            _ => (space > 0, stored > 0),
        };
        endpoints.push(Endpoint {
            face: face as u8,
            can_insert,
            can_extract,
            space,
            stored,
        });
    }
    let plan = cable_plan(energy, &endpoints);
    if plan.moves.is_empty() && plan.energy_after == energy {
        return;
    }
    // Execute the plan and account with what the neighbours really moved.
    let mut current = energy;
    for change in &plan.moves {
        let (face, amount, pull) = match *change {
            Move::Pull { face, amount } => (face, amount, true),
            Move::Push { face, amount } => (face, amount, false),
        };
        let direction = Direction::ALL[face as usize];
        let at = neighbour(event.pos, direction);
        let limit = cable_move_limit(*change, current).min(amount).max(0);
        if limit == 0 {
            continue;
        }
        let moved = move_energy(host, level, at, direction.opposite(), limit, pull);
        current = if pull {
            current.saturating_add(moved)
        } else {
            current.saturating_sub(moved)
        };
    }
    let after = clamp_energy(current);
    if after != energy {
        set_be_int(&mut event.entity.nbt, KEY_ENERGY, after);
    }
}

/// `ElectricFurnaceBlockEntity.tick`: smelt the vanilla smelting recipe of the input slot for
/// its own ticks, spending 1 E a tick, and put the result in the output slot when the cook
/// completes.
fn furnace_tick_of(host: &mut dyn ServerHost, event: &mut mod_api::BlockEntityTickEvent<'_>) {
    if event.entity.items.len() < MENU_SLOTS {
        event.entity.items.resize(MENU_SLOTS, None);
    }
    let input = event.entity.items[INPUT_SLOT].clone();
    let output = event.entity.items[OUTPUT_SLOT].clone();
    let recipe = input.as_ref().and_then(|stack| {
        let level = in_place_level(host)?;
        let kind = Identifier::parse("minecraft:smelting").ok()?;
        let found = level
            .recipe_for(host, &kind, &RecipeInput::Single(stack.clone()))
            .ok()
            .flatten()?;
        let max = host.max_stack_size(found.output.item()).unwrap_or(64);
        Some((found.processing_time, found.output, max))
    });
    let output_fits = match (&output, &recipe) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(slot), Some((_, result, max))) => {
            slot.is_stackable_with(result) && slot.count().saturating_add(result.count()) <= *max
        }
    };
    let mut furnace = Furnace {
        energy: be_int(&event.entity.nbt, KEY_ENERGY).clamp(0, FURNACE_CAPACITY),
        cook_time: be_int(&event.entity.nbt, KEY_COOK_TIME),
        cook_total: be_int(&event.entity.nbt, KEY_COOK_TOTAL),
        active: false,
    };
    let step = furnace_tick(
        &mut furnace,
        input.is_some(),
        recipe.as_ref().map(|(ticks, _, _)| *ticks),
        output_fits,
    );
    if step.energy_used != 0 || step.craft || furnace.cook_time == 0 {
        set_be_int(&mut event.entity.nbt, KEY_ENERGY, furnace.energy);
        set_be_int(&mut event.entity.nbt, KEY_COOK_TIME, furnace.cook_time);
        set_be_int(&mut event.entity.nbt, KEY_COOK_TOTAL, furnace.cook_total);
    }
    if step.craft
        && let Some((_, result, _)) = recipe
    {
        apply_craft(&mut event.entity, &result);
    }
    let active = furnace.active;
    let data = furnace_data(&furnace, &step);
    if event.entity.data.get(..MENU_DATA) != Some(&data[..]) {
        event.entity.data = data.to_vec();
    }
    set_active(host, event.pos, active);
}

/// Take one input item and put the recipe's result in the output slot
/// (`ElectricFurnaceBlockEntity.craftRecipe`).
fn apply_craft(entity: &mut mod_api::ModBlockEntityView<'_>, result: &ItemStack) {
    if let Some(slot) = entity.items.get_mut(INPUT_SLOT)
        && let Some(stack) = slot.take()
    {
        *slot = stack.shrink(1);
    }
    match entity.items.get_mut(OUTPUT_SLOT) {
        Some(slot) => match slot {
            Some(stack) if stack.is_stackable_with(result) => {
                let _ = stack.grow(result.count());
            }
            other => *other = Some(result.clone()),
        },
        None => entity.items.push(Some(result.clone())),
    }
}

/// The vanilla furnace menu's four data slots: the flame is on while a recipe is current, the
/// arrow is the cook's progress out of its total.
fn furnace_data(furnace: &Furnace, step: &FurnaceStep) -> [i32; MENU_DATA] {
    let lit = i32::from(furnace.active || step.craft);
    [lit, 1, furnace.cook_time, furnace.cook_total]
}

// ---------------------------------------------------------------------------
// Block state, energy and level helpers
// ---------------------------------------------------------------------------

/// Write the machine's `active` state when it changed (`BlockMachineBase.setActive`).
fn set_active(host: &mut dyn ServerHost, pos: [i32; 3], active: bool) {
    let want = if active { "true" } else { "false" };
    if property_of(host, pos, "active").as_deref() != Some(want) {
        host.set_block_property(pos, "active", want);
    }
}

/// Property `key` of the block at `pos` in the level in place.
fn property_of(host: &dyn ServerHost, pos: [i32; 3], key: &str) -> Option<String> {
    let level = in_place_level(host)?;
    level.block_property(host, pos, key)
}

/// The block at `pos` in the level in place, as its id.
fn block_at(host: &dyn ServerHost, pos: [i32; 3]) -> Option<String> {
    let level = in_place_level(host)?;
    Some(level.block(host, pos)?.id(host))
}

/// The level the server ticks now: the only one whose block entities a capability reads.
fn in_place_level(host: &dyn ServerHost) -> Option<LevelRef> {
    host.levels()
        .list_levels()
        .into_iter()
        .find(|level| level.in_place(host))
}

/// The energy buffer of the block at `pos`, reached through `side`: its amount and capacity.
fn energy_of(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    side: Direction,
) -> Option<(i32, i32)> {
    let handler = level.capability(host, &ENERGY, pos, Some(side))?;
    let stored = i32::try_from(handler.amount(&mut *host)).unwrap_or(i32::MAX);
    let capacity = i32::try_from(handler.capacity(&mut *host)).unwrap_or(i32::MAX);
    Some((stored, capacity))
}

/// Move up to `limit` energy out of (a pull) or into (a push) the block at `pos` through
/// `side`, in a transaction, and answer with what really moved.
fn move_energy(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    side: Direction,
    limit: i32,
    pull: bool,
) -> i32 {
    let amount = u32::try_from(limit).unwrap_or(0);
    let moved = host
        .transaction(|host, tx| -> Result<u32, ()> {
            let Some(handler) = level.capability(&mut *host, &ENERGY, pos, Some(side)) else {
                return Ok(0);
            };
            Ok(if pull {
                handler.extract(&mut *host, tx, amount)
            } else {
                handler.insert(&mut *host, tx, amount)
            })
        })
        .unwrap_or(0);
    i32::try_from(moved).unwrap_or(i32::MAX)
}

fn neighbour(pos: [i32; 3], face: Direction) -> [i32; 3] {
    let [dx, dy, dz] = face.normal();
    [pos[0] + dx, pos[1] + dy, pos[2] + dz]
}

// ---------------------------------------------------------------------------
// The `/techreborn` harness commands
// ---------------------------------------------------------------------------

/// The buffer size of the block at `pos`, if it is one of the slice's.
fn capacity_of(block: &str) -> Option<i32> {
    match block {
        GENERATOR => Some(GENERATOR_CAPACITY),
        CABLE => Some(CABLE_CAPACITY),
        FURNACE => Some(FURNACE_CAPACITY),
        _ => None,
    }
}

/// The block's state, as `/techreborn info` prints it: the buffer, the machine state and the
/// slots, one line of `key=value` fields the harness parses.
fn readout(host: &mut dyn ServerHost, pos: [i32; 3]) -> String {
    let Some(block) = block_at(host, pos) else {
        return format!("Tech Reborn: {pos:?} is not a mod block entity.");
    };
    let Some(capacity) = capacity_of(&block) else {
        return format!("Tech Reborn: {pos:?} is a {block}.");
    };
    let Some(level) = in_place_level(host) else {
        return format!("Tech Reborn: {pos:?} is in no open level.");
    };
    let Some(entity) = host.block_entity(level, pos) else {
        return format!("Tech Reborn: {pos:?} is not a mod block entity.");
    };
    let data = entity.data(host).unwrap_or_default();
    let energy = be_int(&data, KEY_ENERGY).clamp(0, capacity);
    let items = &*host;
    let slot = |index: u32| {
        entity
            .item_slot(items, index)
            .ok()
            .flatten()
            .map_or_else(|| "empty".to_owned(), |stack| slot_text(items, &stack))
    };
    format!(
        "Tech Reborn {block} at {pos:?}: energy={energy}/{capacity} active={} burn={}/{} cook={}/{} in={} out={}",
        property_of(host, pos, "active").unwrap_or_else(|| "none".to_owned()),
        be_int(&data, KEY_BURN_TIME),
        be_int(&data, KEY_TOTAL_BURN),
        be_int(&data, KEY_COOK_TIME),
        be_int(&data, KEY_COOK_TOTAL),
        slot(INPUT_SLOT as u32),
        slot(OUTPUT_SLOT as u32),
    )
}

/// `/techreborn energy <pos> <amount>`: set a block's buffer, clamped to its capacity.
fn set_energy(host: &mut dyn ServerHost, pos: [i32; 3], amount: i32) -> String {
    let Some(block) = block_at(host, pos) else {
        return format!("{pos:?} is not a Tech Reborn block.");
    };
    let Some(capacity) = capacity_of(&block) else {
        return format!("{pos:?} is not a Tech Reborn block.");
    };
    let Some(level) = in_place_level(host) else {
        return format!("{pos:?} is in no open level.");
    };
    let Some(entity) = host.block_entity(level, pos) else {
        return format!("{pos:?} is not a Tech Reborn block.");
    };
    let amount = amount.clamp(0, capacity);
    let mut data = Compound::new();
    data.insert(KEY_ENERGY.into(), Tag::Int(amount));
    match entity.merge_data(host, data) {
        Ok(true) => format!("Tech Reborn at {pos:?}: {amount}/{capacity} E."),
        Ok(false) => format!("Tech Reborn at {pos:?}: still {amount}/{capacity} E."),
        Err(_) => format!("{pos:?} is not a Tech Reborn block."),
    }
}

fn slot_text(host: &dyn ServerHost, stack: &ItemStack) -> String {
    format!("{}x{}", stack.item().id(host), stack.count())
}

fn position_argument(value: Option<&CommandArgumentValue>) -> Option<[i32; 3]> {
    match value? {
        CommandArgumentValue::BlockPos(pos) => Some(*pos),
        _ => None,
    }
}

fn integer_argument(value: Option<&CommandArgumentValue>) -> Option<i32> {
    match value? {
        CommandArgumentValue::Integer(amount) => Some(*amount),
        _ => None,
    }
}

fn feedback(host: &mut dyn ServerHost, line: &str) {
    let _ = host.commands().feedback(Text::literal(line));
}

// ---------------------------------------------------------------------------
// Saved-compound reads
// ---------------------------------------------------------------------------

fn be_int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(value)) => *value,
        _ => 0,
    }
}

fn set_be_int(nbt: &mut Compound, key: &str, value: i32) {
    nbt.insert(key.into(), Tag::Int(value));
}

#[cfg(test)]
mod registration {
    use super::*;

    /// The engine refuses a package that attaches one Part type to one block twice, and
    /// [`Registrar::block_entity_part`] already puts the block half of a block-entity part on every
    /// block of its declared type (`expand_block_entity_parts`). This runs the port's own `register`
    /// through the SDK's registration path, so the assertion is the engine's own verdict: before
    /// the redundant `block_part` calls were removed it answered `Mod parts refused: … is attached
    /// to … twice`.
    #[test]
    fn the_engine_accepts_every_part_of_this_package() {
        let category = |_: &str| false;
        let position = |_: &str| None;
        let schema = mod_api::__private::RuleSchema {
            category: &category,
            position: &position,
            rule_count: 0,
        };
        let mods = vec![mod_api::ModEntry::new("techreborn", Box::new(Mod))];
        let refused =
            mod_api::__private::install_parts(&mods, &schema, |_: &DimensionSettings| false);
        assert!(
            refused.is_ok(),
            "the engine refuses this package: {refused:?}"
        );
    }
}
