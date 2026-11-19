// Pure Tech Reborn slice-1 arithmetic (standalone crate lib, injected as
// `crate::game::techreborn`). No game access lives here: the solid fuel
// generator's burn, the copper cable's transfer plan and the electric
// furnace's cook step. Every number is upstream's (Tech Reborn 6.2.1,
// `src/main/java/techreborn/...`, read at commit 140bc0fc); see FINDINGS.md.
//
// The three blocks form the smallest closed energy loop Tech Reborn has:
// a generator that makes 10 E a tick out of a burnable item, a cable that
// moves at most 128 E a tick and buffers 512 E, and a machine that spends
// 1 E a tick to smelt a vanilla smelting recipe in the recipe's own ticks.

/// `TechRebornConfig.solidFuelGeneratorMaxEnergy`'s default (`GeneratorMaxEnergy`).
pub const GENERATOR_CAPACITY: i32 = 10_000;
/// `TechRebornConfig.solidFuelGeneratorOutputAmount` (`GeneratorEnergyOutput`):
/// energy one burning tick adds.
pub const GENERATOR_OUTPUT: i32 = 10;
/// `TechRebornConfig.solidFuelGeneratorMaxOutput` (`GeneratorMaxOutput`): the
/// most one extract call takes out of the generator's buffer.
pub const GENERATOR_MAX_OUTPUT: i32 = 32;
/// `SolidFuelGeneratorBlockEntity.getItemBurnTime`: a fuel's smelting burn
/// time divided by four gives its generator burn ticks.
pub const BURN_TICKS_DIVISOR: u32 = 4;

/// `Cables.COPPER`: the transfer rate of one copper cable.
pub const CABLE_RATE: i32 = 128;
/// `CableBlockEntity`: the cable's own buffer is `transferRate * 4`.
pub const CABLE_CAPACITY: i32 = CABLE_RATE * 4;

/// `TechRebornConfig.electricFurnaceMaxEnergy` (`ElectricFurnaceMaxEnergy`).
pub const FURNACE_CAPACITY: i32 = 1_000;
/// `TechRebornConfig.electricFurnaceMaxInput` (`ElectricFurnaceInput`).
pub const FURNACE_MAX_INPUT: i32 = 32;
/// `ElectricFurnaceBlockEntity.EnergyPerTick`, times the default
/// `powerMultiplier` of 1.
pub const FURNACE_EU_PER_TICK: i32 = 1;

/// The fuel slot, as `SolidFuelGeneratorBlockEntity.getItemBurnTime` reads it:
/// the next item's smelting burn time without taking it, and the shrink that
/// takes one.
pub trait FuelSlot {
    /// The next fuel item's smelting burn time in ticks, if the slot is not empty.
    fn peek(&self) -> Option<u32>;
    /// `ItemStack.shrink(1)`, or the crafting remainder in its place.
    fn take(&mut self);
}

/// What one tick of the generator did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratorTick {
    /// Energy added to the buffer (`0` or [`GENERATOR_OUTPUT`]).
    pub energy_added: i32,
    /// Fuel items the tick took out of the slot.
    pub fuel_taken: u32,
}

/// The solid fuel generator's saved state, under upstream's keys (`energy`,
/// `BurnTime`, `TotalBurnTime`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Generator {
    /// Stored energy.
    pub energy: i32,
    /// Burning steps left (`BurnTime`).
    pub burn_time: i32,
    /// The steps the last loaded fuel gave (`TotalBurnTime`).
    pub total_burn: i32,
    /// The `active` block state.
    pub active: bool,
}

impl Generator {
    /// An empty generator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The energy the buffer can still take.
    #[must_use]
    pub fn room(&self) -> i32 {
        GENERATOR_CAPACITY.saturating_sub(self.energy).max(0)
    }

    /// One server tick of `SolidFuelGeneratorBlockEntity.tick`, in its order:
    /// spend a burn step while the buffer has room for a whole one, then load
    /// the next fuel item when the steps run out (in the same tick).
    pub fn tick(&mut self, fuel: &mut impl FuelSlot) -> GeneratorTick {
        let mut tick = GeneratorTick::default();
        if self.room() >= GENERATOR_OUTPUT {
            if self.burn_time > 0 {
                self.burn_time -= 1;
                self.energy = (self.energy + GENERATOR_OUTPUT).min(GENERATOR_CAPACITY);
                tick.energy_added = GENERATOR_OUTPUT;
            }
        } else {
            self.update_state();
        }
        if self.burn_time == 0 {
            self.update_state();
            if let Some(ticks) = fuel.peek().map(burn_ticks).filter(|ticks| *ticks > 0) {
                fuel.take();
                tick.fuel_taken += 1;
                self.burn_time = ticks;
                self.total_burn = ticks;
                self.update_state();
            } else {
                self.total_burn = 0;
            }
        }
        tick
    }

    /// `updateState`: lit while steps are left and the buffer has any room.
    fn update_state(&mut self) {
        self.active = self.burn_time > 0 && self.room() > 0;
    }
}

/// The generator burn ticks one fuel item gives: `getItemBurnTime`'s
/// `burnTime / 4`, integer division as upstream's.
#[must_use]
pub fn burn_ticks(cooking_fuel_ticks: u32) -> i32 {
    (cooking_fuel_ticks / BURN_TICKS_DIVISOR) as i32
}

/// A block beside a cable, as the engine's energy helpers answer it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Endpoint {
    /// The face, 0..6 (`Direction`: down, up, north, south, west, east).
    pub face: u8,
    /// The block there takes energy through this side.
    pub can_insert: bool,
    /// The block there gives energy through this side.
    pub can_extract: bool,
    /// How much more energy that block takes.
    pub space: i32,
    /// How much energy that block holds.
    pub stored: i32,
}

/// One energy move a cable makes in its tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    /// Take `amount` from the block at `face` into the cable.
    Pull {
        /// The neighbour's face.
        face: u8,
        /// Energy moved.
        amount: i32,
    },
    /// Give `amount` from the cable to the block at `face`.
    Push {
        /// The neighbour's face.
        face: u8,
        /// Energy moved.
        amount: i32,
    },
}

/// What a cable does in one tick: its moves in order, and its energy after.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Pulls (face order), then pushes (face order).
    pub moves: Vec<Move>,
    /// The cable's energy once every move ran.
    pub energy_after: i32,
}

/// Plan one copper cable's tick, as `CableTickManager.handleCableTick` moves
/// energy for a one-cable network: pull into the buffer up to its free room,
/// then push out what it holds. Each neighbour gets at most the cable's
/// transfer rate and at most an even share of what is left to move, which is
/// upstream's `min(remaining / remainingTargets, transferRate)`.
#[must_use]
pub fn cable_plan(energy: i32, neighbours: &[Endpoint]) -> Plan {
    let mut current = clamp_energy(energy);
    let mut moves = Vec::new();
    // Pulls: what the buffer can still hold is this tick's budget.
    let mut budget = CABLE_CAPACITY - current;
    let providers: Vec<&Endpoint> = neighbours
        .iter()
        .filter(|endpoint| endpoint.can_extract && endpoint.stored > 0)
        .collect();
    for (index, endpoint) in providers.iter().enumerate() {
        let targets = (providers.len() - index) as i32;
        let asked = (budget / targets).clamp(0, CABLE_RATE);
        let moved = asked.min(endpoint.stored).max(0);
        if moved > 0 {
            current += moved;
            budget -= moved;
            moves.push(Move::Pull {
                face: endpoint.face,
                amount: moved,
            });
        }
    }
    // Pushes: what the cable holds is this tick's budget.
    let mut budget = current;
    let acceptors: Vec<&Endpoint> = neighbours
        .iter()
        .filter(|endpoint| endpoint.can_insert && endpoint.space > 0)
        .collect();
    for (index, endpoint) in acceptors.iter().enumerate() {
        let targets = (acceptors.len() - index) as i32;
        let asked = (budget / targets).clamp(0, CABLE_RATE);
        let moved = asked.min(endpoint.space).max(0);
        if moved > 0 {
            current -= moved;
            budget -= moved;
            moves.push(Move::Push {
                face: endpoint.face,
                amount: moved,
            });
        }
    }
    Plan {
        moves,
        energy_after: current,
    }
}

/// The most a planned move may really move: a pull is capped by the cable's
/// free room, a push by what the cable holds now (a neighbour may have given
/// less than the plan expected), so no energy is made from nothing.
#[must_use]
pub fn cable_move_limit(change: Move, energy: i32) -> i32 {
    let energy = energy.max(0);
    match change {
        Move::Pull { amount, .. } => amount.min(CABLE_CAPACITY.saturating_sub(energy).max(0)),
        Move::Push { amount, .. } => amount.min(energy),
    }
}

/// `energy` clamped into a cable buffer.
#[must_use]
pub fn clamp_energy(energy: i32) -> i32 {
    energy.clamp(0, CABLE_CAPACITY)
}

/// The `facing` a machine takes when it is placed, as `BlockMachineBase`'s
/// `placer.getDirection().getOpposite()` gives it: the machine faces the
/// player, so a look along +Z (yaw 0, south) leaves it facing north.
#[must_use]
pub fn facing_for_yaw(yaw: f32) -> &'static str {
    match ((f64::from(yaw) / 90.0 + 0.5).floor() as i64 & 3) as u8 {
        0 => "north",
        1 => "east",
        2 => "south",
        _ => "west",
    }
}

/// The electric furnace's saved state (`energy`, and upstream's `cookTime` /
/// `cookTimeTotal`, which are not saved but live as long as the entity).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Furnace {
    /// Stored energy.
    pub energy: i32,
    /// Ticks cooked of the current recipe.
    pub cook_time: i32,
    /// The current recipe's total ticks.
    pub cook_total: i32,
    /// The `active` block state: a recipe is current.
    pub active: bool,
}

/// What one tick of the furnace did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FurnaceStep {
    /// Energy spent this tick (`0` or [`FURNACE_EU_PER_TICK`]).
    pub energy_used: i32,
    /// Whether one input item becomes the output this tick.
    pub craft: bool,
}

/// One server tick of `ElectricFurnaceBlockEntity.tick`.
///
/// `recipe_ticks` is the vanilla smelting recipe's `cookingtime` for the
/// input (`None` when nothing in the slot smelts), and `output_fits` is
/// `canAcceptOutput`: the result is empty, or the same item with room for it.
#[must_use]
pub fn furnace_tick(
    furnace: &mut Furnace,
    has_input: bool,
    recipe_ticks: Option<u32>,
    output_fits: bool,
) -> FurnaceStep {
    let mut step = FurnaceStep::default();
    let Some(ticks) = recipe_ticks.filter(|_| has_input && output_fits) else {
        // `resetCrafter`: no input, nothing to smelt, or no room for the result.
        furnace.cook_time = 0;
        furnace.cook_total = 0;
        furnace.active = false;
        return step;
    };
    furnace.cook_total = ticks.max(1) as i32;
    furnace.active = true;
    if furnace.cook_time >= furnace.cook_total {
        // The crafting tick spends nothing, and `updateCurrentRecipe` after it
        // starts the next item at zero.
        step.craft = true;
        furnace.cook_time = 0;
    } else if furnace.energy > FURNACE_EU_PER_TICK {
        furnace.energy -= FURNACE_EU_PER_TICK;
        step.energy_used = FURNACE_EU_PER_TICK;
        furnace.cook_time += 1;
    }
    step
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fuel slot over a list of smelting burn times.
    struct FakeFuel {
        items: Vec<u32>,
    }

    impl FakeFuel {
        fn of(items: &[u32]) -> Self {
            Self {
                items: items.to_vec(),
            }
        }
    }

    impl FuelSlot for FakeFuel {
        fn peek(&self) -> Option<u32> {
            self.items.first().copied()
        }

        fn take(&mut self) {
            if !self.items.is_empty() {
                self.items.remove(0);
            }
        }
    }

    #[test]
    fn one_coal_is_four_hundred_burn_ticks_of_ten_energy() {
        // Vanilla's coal `minecraft:cooking_fuel` burn_time is 1600 ticks.
        assert_eq!(burn_ticks(1600), 400);
        assert_eq!(generator_burn_total(), 4000);
    }

    /// Tick a generator through one coal's whole burn.
    fn generator_burn_total() -> i32 {
        let mut generator = Generator::new();
        let mut fuel = FakeFuel::of(&[1600]);
        for _ in 0..430 {
            generator.tick(&mut fuel);
        }
        generator.energy
    }

    #[test]
    fn a_burn_tick_adds_ten_energy_and_spends_one_step() {
        let mut generator = Generator::new();
        let mut fuel = FakeFuel::of(&[1600]);
        // The first tick takes the fuel; the second is the first burning one.
        generator.tick(&mut fuel);
        let tick = generator.tick(&mut fuel);
        assert_eq!(tick.energy_added, GENERATOR_OUTPUT);
        assert_eq!(generator.energy, GENERATOR_OUTPUT);
        assert_eq!(generator.burn_time, 399);
        assert!(generator.active);
    }

    #[test]
    fn the_first_tick_takes_the_fuel_and_makes_nothing() {
        let mut generator = Generator::new();
        let mut fuel = FakeFuel::of(&[1600]);
        let tick = generator.tick(&mut fuel);
        assert_eq!(tick.fuel_taken, 1);
        assert_eq!(tick.energy_added, 0);
        assert_eq!(generator.burn_time, 400);
        assert_eq!(generator.total_burn, 400);
        assert_eq!(generator.energy, 0);
        assert!(generator.active);
    }

    #[test]
    fn a_full_buffer_burns_nothing_and_reads_inactive() {
        let mut generator = Generator {
            energy: GENERATOR_CAPACITY,
            burn_time: 5,
            total_burn: 5,
            ..Generator::new()
        };
        let mut fuel = FakeFuel::of(&[1600]);
        let tick = generator.tick(&mut fuel);
        assert_eq!(tick, GeneratorTick::default());
        assert_eq!(generator.burn_time, 5);
        assert!(!generator.active, "no room means no burn and no light");
    }

    #[test]
    fn a_partly_full_buffer_keeps_the_light_on_without_burning() {
        // Upstream's `updateState` asks for any room, while the burn asks for a
        // whole tick's worth: the generator reads active and stalls.
        let mut generator = Generator {
            energy: GENERATOR_CAPACITY - 5,
            burn_time: 5,
            total_burn: 5,
            ..Generator::new()
        };
        let mut fuel = FakeFuel::of(&[1600]);
        let tick = generator.tick(&mut fuel);
        assert_eq!(tick.energy_added, 0);
        assert_eq!(generator.burn_time, 5);
        assert!(generator.active);
    }

    #[test]
    fn a_fuel_under_four_smelting_ticks_is_not_consumed() {
        let mut generator = Generator::new();
        let mut fuel = FakeFuel::of(&[3]);
        let tick = generator.tick(&mut fuel);
        assert_eq!(tick.fuel_taken, 0);
        assert_eq!(fuel.items, vec![3]);
        assert_eq!(generator.burn_time, 0);
        assert!(!generator.active);
    }

    #[test]
    fn the_next_fuel_loads_on_the_tick_the_last_step_burns() {
        let mut generator = Generator {
            burn_time: 1,
            total_burn: 1,
            ..Generator::new()
        };
        let mut fuel = FakeFuel::of(&[1600]);
        let tick = generator.tick(&mut fuel);
        assert_eq!(tick.energy_added, GENERATOR_OUTPUT);
        assert_eq!(tick.fuel_taken, 1, "the next item loads in the same tick");
        assert_eq!(generator.burn_time, 400);
        assert!(generator.active);
    }

    #[test]
    fn no_fuel_reads_inactive() {
        let mut generator = Generator::new();
        let mut fuel = FakeFuel::of(&[]);
        let tick = generator.tick(&mut fuel);
        assert_eq!(tick, GeneratorTick::default());
        assert!(!generator.active);
    }

    /// The energy a plan pulls out of its neighbours.
    fn pulled(plan: &Plan) -> i32 {
        plan.moves
            .iter()
            .map(|m| match m {
                Move::Pull { amount, .. } => *amount,
                Move::Push { .. } => 0,
            })
            .sum()
    }

    /// The energy a plan pushes into its neighbours.
    fn pushed(plan: &Plan) -> i32 {
        plan.moves
            .iter()
            .map(|m| match m {
                Move::Push { amount, .. } => *amount,
                Move::Pull { .. } => 0,
            })
            .sum()
    }

    /// A neighbour that only gives energy (a generator: `canAcceptEnergy` is
    /// false, so its insert side moves nothing).
    fn provider(face: u8, stored: i32) -> Endpoint {
        Endpoint {
            face,
            can_insert: false,
            can_extract: true,
            space: 0,
            stored,
        }
    }

    /// A neighbour that only takes energy (a machine: `canProvideEnergy` is
    /// false, so its extract side moves nothing).
    fn acceptor(face: u8, space: i32) -> Endpoint {
        Endpoint {
            face,
            can_insert: true,
            can_extract: false,
            space,
            stored: 0,
        }
    }

    /// A neighbour that both gives and takes (a cable, or a storage unit).
    fn storage(face: u8, stored: i32, space: i32) -> Endpoint {
        Endpoint {
            face,
            can_insert: true,
            can_extract: true,
            space,
            stored,
        }
    }

    #[test]
    fn one_tick_pulls_at_most_the_transfer_rate() {
        let plan = cable_plan(0, &[provider(0, 1000)]);
        assert_eq!(
            plan.moves,
            vec![Move::Pull {
                face: 0,
                amount: 128
            }]
        );
        assert_eq!(plan.energy_after, 128);
    }

    #[test]
    fn a_pull_stops_at_the_free_room() {
        let plan = cable_plan(CABLE_CAPACITY - 10, &[provider(0, 1000)]);
        assert_eq!(
            plan.moves,
            vec![Move::Pull {
                face: 0,
                amount: 10
            }]
        );
        assert_eq!(plan.energy_after, CABLE_CAPACITY);
    }

    #[test]
    fn one_tick_pushes_at_most_the_transfer_rate() {
        let plan = cable_plan(512, &[acceptor(0, 5000)]);
        assert_eq!(
            plan.moves,
            vec![Move::Push {
                face: 0,
                amount: 128
            }]
        );
        assert_eq!(plan.energy_after, 384);
    }

    #[test]
    fn a_push_stops_at_the_neighbours_room() {
        let plan = cable_plan(512, &[acceptor(0, 30)]);
        assert_eq!(
            plan.moves,
            vec![Move::Push {
                face: 0,
                amount: 30
            }]
        );
        assert_eq!(plan.energy_after, 482);
    }

    #[test]
    fn two_providers_share_the_free_room_evenly() {
        let plan = cable_plan(0, &[provider(0, 1000), provider(1, 1000)]);
        assert_eq!(
            plan.moves,
            vec![
                Move::Pull {
                    face: 0,
                    amount: 128
                },
                Move::Pull {
                    face: 1,
                    amount: 128
                },
            ],
            "upstream divides what is left by the neighbours left"
        );
        assert_eq!(plan.energy_after, 256);
    }

    #[test]
    fn a_plan_conserves_energy() {
        let plan = cable_plan(300, &[storage(0, 400, 100), acceptor(2, 700)]);
        assert_eq!(
            plan.energy_after,
            clamp_energy(300 + pulled(&plan) - pushed(&plan))
        );
        assert_eq!(pulled(&plan), 128);
    }

    #[test]
    fn a_neighbour_that_takes_nothing_gets_no_move() {
        let mut closed = acceptor(0, 5000);
        closed.can_insert = false;
        let empty = acceptor(1, 0);
        let plan = cable_plan(64, &[closed, empty]);
        assert!(plan.moves.is_empty());
        assert_eq!(plan.energy_after, 64);
    }

    #[test]
    fn an_empty_cable_with_no_neighbours_plans_nothing() {
        let plan = cable_plan(0, &[]);
        assert!(plan.moves.is_empty());
        assert_eq!(plan.energy_after, 0);
    }

    #[test]
    fn a_move_is_capped_by_what_the_cable_really_holds() {
        // The plan expected the pull to land; the neighbour gave less.
        assert_eq!(
            cable_move_limit(
                Move::Push {
                    face: 0,
                    amount: 128
                },
                10
            ),
            10
        );
        assert_eq!(
            cable_move_limit(
                Move::Pull {
                    face: 0,
                    amount: 128
                },
                CABLE_CAPACITY - 4
            ),
            4
        );
    }

    /// A furnace holding `energy` with no cook in progress.
    fn furnace(energy: i32) -> Furnace {
        Furnace {
            energy,
            ..Furnace::default()
        }
    }

    #[test]
    fn a_placed_machine_faces_the_player() {
        // A look along +Z is yaw 0 (south); the machine's front ends up north.
        assert_eq!(facing_for_yaw(0.0), "north");
        assert_eq!(facing_for_yaw(90.0), "east");
        assert_eq!(facing_for_yaw(180.0), "south");
        assert_eq!(facing_for_yaw(-90.0), "west");
        // The quadrant, not the exact angle, decides.
        assert_eq!(facing_for_yaw(44.0), "north");
        assert_eq!(facing_for_yaw(46.0), "east");
    }

    #[test]
    fn a_smelting_tick_spends_one_energy_and_advances_one_tick() {
        let mut furnace = furnace(1000);
        let step = furnace_tick(&mut furnace, true, Some(200), true);
        assert_eq!(step.energy_used, FURNACE_EU_PER_TICK);
        assert_eq!(furnace.energy, 999);
        assert_eq!(furnace.cook_time, 1);
        assert_eq!(furnace.cook_total, 200);
        assert!(furnace.active);
    }

    #[test]
    fn one_energy_in_the_buffer_is_not_enough() {
        // `if (getStored() > getEuPerTick(EnergyPerTick))` — strictly greater.
        let mut furnace = furnace(1);
        let step = furnace_tick(&mut furnace, true, Some(200), true);
        assert_eq!(step.energy_used, 0);
        assert_eq!(furnace.energy, 1);
        assert_eq!(furnace.cook_time, 0);
    }

    #[test]
    fn the_output_appears_on_the_tick_after_the_cook_completes() {
        let mut furnace = furnace(1000);
        for _ in 0..200 {
            assert!(!furnace_tick(&mut furnace, true, Some(200), true).craft);
        }
        assert_eq!(furnace.cook_time, 200);
        let step = furnace_tick(&mut furnace, true, Some(200), true);
        assert!(step.craft);
        assert_eq!(step.energy_used, 0, "the crafting tick spends nothing");
        assert_eq!(furnace.cook_time, 0, "the next item starts at zero");
        assert_eq!(furnace.energy, 800, "200 ticks of cooking cost 200 energy");
    }

    #[test]
    fn no_recipe_means_no_progress() {
        let mut furnace = furnace(1000);
        let step = furnace_tick(&mut furnace, true, None, true);
        assert_eq!(step, FurnaceStep::default());
        assert_eq!(furnace.energy, 1000);
        assert_eq!(furnace.cook_time, 0);
        assert!(!furnace.active);
    }

    #[test]
    fn a_full_output_slot_stops_the_cook() {
        let mut furnace = furnace(1000);
        let step = furnace_tick(&mut furnace, true, Some(200), false);
        assert_eq!(step, FurnaceStep::default());
        assert_eq!(furnace.energy, 1000);
        assert!(!furnace.active);
    }

    #[test]
    fn an_empty_input_slot_resets_the_cook() {
        let mut furnace = Furnace {
            energy: 1000,
            cook_time: 120,
            cook_total: 200,
            active: true,
        };
        let step = furnace_tick(&mut furnace, false, Some(200), true);
        assert_eq!(step, FurnaceStep::default());
        assert_eq!(furnace.cook_time, 0);
        assert_eq!(furnace.cook_total, 0);
        assert!(!furnace.active);
    }

    #[test]
    fn a_partial_cook_keeps_its_progress_when_energy_returns() {
        let mut furnace = furnace(0);
        let _ = furnace_tick(&mut furnace, true, Some(200), true);
        assert_eq!(furnace.cook_time, 0);
        assert!(furnace.active, "the recipe stays current without energy");
        furnace.energy = 2;
        let step = furnace_tick(&mut furnace, true, Some(200), true);
        assert_eq!(step.energy_used, 1);
        assert_eq!(furnace.cook_time, 1);
    }
}
