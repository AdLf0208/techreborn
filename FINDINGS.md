# Tech Reborn — findings (jar inventory + upstream behaviour)

Port target: **Tech Reborn 6.2.1**, external to ATM10 (`external/README.md`).
Artefacts read:

| What | Where | Evidence |
|---|---|---|
| Jar (26.3 build, fabric) | `ports/_shared/licence-cache/TechReborn-6.2.1.jar` | 6,915,935 B, **10,122 entries**, 319 blockstates, 404 item models, **5,216 recipe files** (`unzip -l`) |
| Licence | `fabric.mod.json` | `"license": "MIT"`, `"id": "techreborn"`, `"version": "6.2.1"`, `"minecraft": "~26.3"` → **rank 0, publishable** |
| Upstream source | `github.com/TechReborn/TechReborn` @ `140bc0fc12adc74f0b1e81892d33db224ae84629` (`mod_version=6.2.1`, `minecraft_version=26.3`) | cloned to scratch `/tmp/opencode/trsrc` (never in the tree); the same version the jar is |

The jar is the FIT fabric build of the same release the upstream `master` tree
builds, so the assets are already 26.3-shaped and every number below can be
checked against the tree that produced the jar. The fabric loader is irrelevant
to a Rust port; what carries is the data and the behaviour.

## 1. What the jar holds (counted, not guessed)

| Family | Count | Notes |
|---|---:|---|
| `assets/techreborn/blockstates/*.json` | 319 | machines are `active × facing` variants; cables are 6-way `multipart` |
| `assets/techreborn/models/**` | 404 item models + block models | machines parent `minecraft:block/orientable` |
| `assets/techreborn/items/*.json` | 26.3 item definitions | `{"model":{"type":"minecraft:model", …}}` |
| `data/techreborn/recipe/**` | 5,216 | `crafting_table/*` (vanilla), `wire_mill/*`, `grinder/*`, `blast_furnace/*`, … (TR's own types) |
| `data/techreborn/loot_table/blocks/*` | one per block | already the singular `"condition": {"type": …}` shape |
| `assets/techreborn/lang/en_us.json` | names + tooltips | `block.techreborn.solid_fuel_generator = Generator` |

Blocks that matter to the slice (all present): `solid_fuel_generator`,
`copper_cable` (+ 8 more cables), `electric_furnace`, `basic_machine_frame`,
`basic_machine_casing`, `advanced_*`, the storage/tank units, the ore chain.

## 2. The slice: generator → cable → electric furnace

The smallest *closed* energy loop in Tech Reborn: an item makes energy, a cable
carries it, a machine spends it on a recipe. Every figure below is read from
the upstream tree at the commit above.

### Solid fuel generator — `techreborn:solid_fuel_generator`

`blockentity/generator/basic/SolidFuelGeneratorBlockEntity.java`,
`config/TechRebornConfig.java` (`GENERATORS_GENERATOR`):

- **10 E/t** (`GeneratorEnergyOutput` default 10) while burning.
- Buffer **10,000 E** (`GeneratorMaxEnergy`), output cap **32 E/t**
  (`GeneratorMaxOutput`), input **0** (`getBaseMaxInput` → 0, so
  `canAcceptEnergy` is false: it never takes energy).
- A fuel item's burn is `FuelRecipes.getBurnTime(...) / 4`; `FuelRecipes` reads
  the 26.3 `minecraft:cooking_fuel` component, i.e. the vanilla furnace burn
  time. Coal = **1600** ticks → **400** generator steps → **4,000 E per coal**.
- Tick order (faithful): burn one step when `room >= 10`; when the steps reach
  zero, load the next item **in the same tick**; `active = burn_time > 0 && room > 0`.
- `active` is the block state; the model has `_on` and `_off` fronts.

### Copper cable — `techreborn:copper_cable`

`blockentity/cable/CableBlockEntity.java`, `blockentity/cable/CableTickManager.java`,
`init/TRContent.java` (`Cables.COPPER`):

- Transfer rate **128 E/t** per side; buffer `rate * 4` = **512 E**.
- `CableTickManager`: BFS the connected cables of the same rate, pool their
  energy, **pull** into the network (budget = pooled free room), **push** out
  (budget = pooled amount), then split evenly. Each neighbour moves at most
  `min(remaining / remainingTargets, transferRate)`.
- The block has six connection booleans (`north…down`), `waterlogged` and
  `covered`; the shape is a 4 px core with 4 px arms (`cableThickness =
  12/2/16 = 0.375`, `CableShapeUtil`).
- `canKill` is true for the bare copper cable: touching it hurts.

### Electric furnace — `techreborn:electric_furnace`

`blockentity/machine/tier1/ElectricFurnaceBlockEntity.java`,
`PowerAcceptorBlockEntity`, `MachineBaseBlockEntity`:

- Smelts **vanilla smelting recipes** (`RecipeType.SMELTING`), so the cook time
  is the recipe's own `cookingtime` (200 ticks for `raw_iron` → `iron_ingot`).
- Spends **1 E/t** (`EnergyPerTick = 1`, `getEuPerTick` = base × default
  `powerMultiplier` 1) — only while `getStored() > 1`, strictly.
- Buffer **1,000 E**, input cap **32 E/t**, output **0** (it never gives energy).
- The output lands on the tick **after** `cookTime` reaches `cookTimeTotal`;
  that crafting tick costs nothing; the next item starts at 0.
- Slots: 0 input, 1 output, 2 battery (`charge(2)` moves FE out of an item in
  slot 2). `active` = a recipe is current.

### What the three share

- Block settings (`TRBlockSettings.machine/cable`): `SoundType.METAL`,
  `strength(2f, 2f)` for a machine, `strength(1f, 8f)` for a cable.
- `BlockMachineBase`: `FACING` (horizontal) + `ACTIVE` booleans; placed with
  `placer.getDirection().getOpposite()` — the machine faces the placer.
- Every machine and the cable are wrenchable (RebornCore's
  `BlockWrenchEventHandler`) — out of this slice.

## 3. Recipes the three would need (not plated in this slice)

| Result | Recipe file | Ingredients |
|---|---|---|
| `solid_fuel_generator` | `crafting_table/solid_fuel_generator_from_basic_machine_frame_and_furnace_and_red_cell_battery.json` | `techreborn:red_cell_battery`, `techreborn:basic_machine_frame`, `minecraft:furnace` |
| `electric_furnace` | `crafting_table/electric_furnace_from_electronic_circuit_and_iron_furnace_and_redstone.json` | `techreborn:electronic_circuit`, `techreborn:iron_furnace`, `minecraft:redstone` ×2 |
| `copper_cable` ×6 | `wire_mill/copper_cable.json` | `minecraft:copper_ingot` in **TR's own `techreborn:wire_mill`** recipe type |

All three name Tech Reborn items or a recipe type outside this slice
(`basic_machine_frame`, `red_cell_battery`, `electronic_circuit`,
`iron_furnace`, `techreborn:wire_mill`), so no recipe is carried: it would be
an ingredient nobody registers. Recorded as a gap in `notes.md`; the e2e gives
the items directly.

## 4. What the port does differently (deliberate)

| Upstream | Port | Why |
|---|---|---|
| Cable pools a whole network by BFS over same-rate cables and splits evenly | one cable's tick pulls from every neighbour it can and pushes to every neighbour that takes — a line of cables relays one hop per tick | the pin has no network/graph helper (ARCHITECTURE §3); for one cable the two are the same, and a line still carries the full 128 E/t |
| Cable target order is shuffled then sorted by simulated result | face order (down, up, north, south, west, east) | determinism; the split rule (`remaining / remainingTargets`, capped at the rate) is kept |
| `waterlogged`, `covered`, wrenching, `canKill` shock | not carried | content has no waterlogging/covers; shocks need an entity-damage path per cable touch |
| A battery in the furnace's slot 2 is discharged through `Capabilities.Energy.ITEM` | the port keeps slot 2 and does not discharge into it | battery items are outside the slice; the item-energy helper needs an item that declares a buffer |
| `active` is written back through `Level.setBlock` every state change | `native_set_block_property` on the machine's own position | same effect, the pin's supported path |
