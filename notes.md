# techreborn — Tech Reborn slice 1: generator → copper cable → electric furnace

**External mod** (not part of ATM10): tree at `external/techreborn`, rules in
`external/README.md`. Publishable.

| | |
|---|---|
| Mod | Tech Reborn **6.2.1** (`techreborn`), jar `ports/_shared/licence-cache/TechReborn-6.2.1.jar` |
| Class / tier | **content / deep** (`horizon/docs/modpacks.md`: powered machines, cables, storage units, multiblocks) |
| Licence | **MIT** — `fabric.mod.json`: `"license": "MIT"`; upstream `LICENSE.md` (MIT). **Rank 0**, publishable (maintainer approval for the actual publish still applies) |
| Package | **0.2.1**, flat SDK layout, mod API **0.2** — `techreborn.hmod` (45,253 B, sha256 `5bc5a327…4b70`), `[native]` pinned to horizon `0b14f8acfd97fa2c750e42795249dcc981178ca4`, mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0` |
| Upstream analysis | `github.com/TechReborn/TechReborn` @ `140bc0fc12adc74f0b1e81892d33db224ae84629` (`mod_version=6.2.1`, `minecraft_version=26.3`), read as scratch and deleted; the jar is kept as evidence |
| Approach | agent port (`deep`): content carried from the jar into the same package + native code for the energy loop |
| Previous delivery | **0.1.0**, the retired `.hnmod` container (AE2 embedded-content pattern), submitted as PR **#819**, closed by Horizon in the format purge. Kept under `ports/_shared/evidence/techreborn/retired-*/` |

## Why this slice

`deep` means block entities, menus, capabilities and multiblocks. The smallest
*closed* loop the tier honestly needs is **energy in, energy moved, energy
spent**: a solid fuel generator makes 10 E a tick out of a burnable item, the
mod's own copper cable carries 128 E a tick, and the electric furnace spends
1 E a tick on a vanilla smelting recipe. Everything else in Tech Reborn (the
other 20-odd machines, the storage units, the multiblocks, the ore chain, the
tools and armour) is a gap listed below — building one of them before this loop
would have meant a machine that cannot be powered.

Every number is upstream's, recorded with its file in `FINDINGS.md` §2:

| Figure | Value | Source |
|---|---|---|
| generator output | **10 E/t** | `TechRebornConfig.solidFuelGeneratorOutputAmount` (`GeneratorEnergyOutput` default 10) |
| generator buffer / out cap | **10,000 E** / **32 E/t** | `GeneratorMaxEnergy`, `GeneratorMaxOutput` |
| generator burn per item | `cooking_fuel / 4` | `SolidFuelGeneratorBlockEntity.getItemBurnTime`, `FuelRecipes` |
| coal | 1600 smelting ticks → **400 steps → 4,000 E** | vanilla `minecraft:cooking_fuel` (`data/minecraft/context_int_provider/cooking/time_coal.json`) |
| cable rate / buffer | **128 E/t** / **512 E** | `TRContent.Cables.COPPER`, `CableBlockEntity` (`rate * 4`) |
| furnace buffer / in cap | **1,000 E** / **32 E/t** | `ElectricFurnaceMaxEnergy`, `ElectricFurnaceInput` |
| furnace cost | **1 E/t**, recipe's own ticks (200 for raw iron) | `ElectricFurnaceBlockEntity.EnergyPerTick`, `RecipeType.SMELTING` |

## What is in it

One package, `techreborn.hmod` (42 entries, 45,253 B in 0.2.1):

- `assets/techreborn/` and `data/techreborn/` — the jar's own blockstates,
  models, textures, item definitions, loot tables and English names for
  `techreborn:solid_fuel_generator`, `techreborn:copper_cable` and
  `techreborn:electric_furnace`, a `techreborn:cables` block tag, and one
  creative tab (`Tech Reborn`). 32 content files.
- Machines are `minecraft:iron_block` templates with declared `active` (boolean)
  and `facing` (4-value enum) states — the jar's own `active × facing`
  blockstate then binds 8 states each. The cable is a `minecraft:chorus_plant`
  (a `PipeBlock`) with `connects_to: "#techreborn:cables"` and `thickness: 4`
  (upstream's `cableThickness = 12/2/16`), so the engine joins its six sides and
  builds the core+arm shapes.
- `src/runtime.rs` — the pure logic, **byte-identical** to the 0.1.0 file
  (sha256 `1825e549…c57`), 25 tests.
- `src/lib.rs` — the one `impl mod_api::Hooks`: three block entity Parts, the
  energy buffers and item containers the blocks declare, the placement facing
  and `/techreborn info|energy`.

## What the glue attaches (SDK 0.2)

| Old 0.1.0 helper | SDK 0.2 home | Kept? |
|---|---|---|
| `embedded_content.rs` (`content_root` injection) | none needed: `assets/` and `data/` are **in the package** | **retired** (format change, not a gap) |
| `block_entity.rs` | `BlockEntityPart::block_entity` on each of the three Parts (returns an empty compound) | kept |
| `block_entity_ticks.rs` | `BlockEntityPart::block_entity_ticks` (`ticks = true`) | kept |
| `block_entity_container.rs` | `Registrar::block_container` for the generator (2 slots) and the furnace (3 slots + `minecraft:furnace` menu) | kept |
| `block_entity_energy.rs` | `Registrar::block_energy` for all three buffers | kept |
| `block_entity_tick.rs` | `BlockEntityPart::block_entity_tick` (one per block; the three bodies share the runtime) | kept |
| `state_for_placement.rs` | `BlockPart::state_for_placement` on the generator and furnace Parts | kept |
| `register_commands.rs` | `Hooks::register_commands` (collect: two `Command`s) | kept |
| `perform_command.rs` | `Hooks::run_mod_command` (`CommandInvocation` literals + typed arguments) | kept |
| (no old equivalent) | `Registrar::block_entity_type(..).blocks(..).build()` + `block_entity_part` — the declaration that makes a block have a block entity | **new, required** |

The old `mark_modified` (chunk dirty) is gone: a tick that changes the entity's
fields answers `Changed`, and the engine dirties the chunk and re-sends the
block; `set_block_property` dirties its own write.

## Upstream defects fixed

None found in this slice, checked against the ruling of 2026-11-11:

- The generator's burn (`cooking_fuel / 4`) is deliberate, not a unit slip: the
  config comment says "Energy Per Tick" and the division is upstream's own
  documented conversion from smelting ticks to generator steps.
- `ElectricFurnaceBlockEntity.canAcceptOutput` calls
  `recipe.assemble(new SingleRecipeInput(inventory.getItem(slot)))` with the
  **output** slot's stack; `SmeltingRecipe.assemble` ignores its input and
  returns the result, so nothing observable differs. Recorded here rather than
  "fixed", because a port that started assembling from the output would be
  inventing a difference.
- `active` reads true for a machine that has a recipe but no energy
  (`updateState` = `currentRecipe != null || canCraftAgain()`). That is the
  mod's meaning of the state (front lit while it has work), so the port keeps
  it and the e2e asserts it (phase D) instead of calling it a bug.

## Deliberate deviations

| Upstream | Port | Why |
|---|---|---|
| A cable network pools every same-rate cable by BFS, then splits evenly | one cable's tick pulls from every neighbour that gives and pushes into every neighbour that takes; a line of cables relays one hop a tick | the pin has no network/graph helper (ARCHITECTURE §3). For one cable the behaviour is identical (the e2e's 3,800 E balance); a line still carries the full 128 E/t |
| Cable targets are shuffled then sorted by their simulated result | face order (down, up, north, south, west, east) | determinism; upstream's split rule `min(remaining / remainingTargets, transferRate)` is kept and unit-tested |
| A cable asks each neighbour `canReceive`/`canExtract` per side | the `horizon:energy` capability answers amount and capacity only. Its own two machines are read exactly from their declared `ModEnergyBuffer` faces; another mod's block counts as willing when it has room (a push) or holds energy (a pull), and the real move's answer corrects the accounting | the SDK capability has no per-side receive/extract flag (`EnergyHandler` = amount, capacity, insert, extract); recorded as an API gap below |
| The furnace looks its recipe up when its inventory is dirty (every 20 ticks) | it looks it up every tick through `LevelRef::recipe_for` | same observable behaviour for a stable input; a changed input resets the cook immediately instead of up to 20 ticks later |
| `cookingFuel` burn times come from the item component via a loot context | `Host::item_burn_time` (the item's `minecraft:cooking_fuel`) | same value, same source |
| `active` written with `Level.setBlock(…, 3)` | `Host::set_block_property` | the pin's supported path; the block entity survives (same block, property only) |
| `/techreborn info` printed `t=<game time>` | no `t=` field (TIME is 0) | the SDK has no server game-time answer; no assertion reads it |
| — | `/techreborn info` prints the **block** id where 0.1.0 printed the block entity type id | the two are the same string here (each type is declared on its own block), and the SDK has no public `BlockEntityTypeKey`→id answer |

## Gaps (slice 2+)

- **The generator has no GUI.** Its container (fuel + battery slot) is real —
  hoppers and `ctl container` reach it — but right-clicking says the block is
  not available, because no screen is drawn (`BlockEntityPart::screen` is not
  implemented). The furnace opens the *vanilla* furnace menu (its own GUI
  texture and energy gauge are not carried).
- **The battery slots are inert**: neither `discharge(1)` (generator) nor
  `charge(2)` (furnace) moves FE to or from an item. The pin has
  `ITEM_ENERGY` (API 307 in the old numbering); no battery item is in this slice.
- **Recipes**: none carried. All three name Tech Reborn items or a recipe type
  outside the slice (`basic_machine_frame`, `red_cell_battery`,
  `electronic_circuit`, `iron_furnace`, and `techreborn:wire_mill` for the
  cable). The e2e gives the items; a player cannot craft them yet.
- **Cable**: no `waterlogged`, no `covered`, no wrench, no electrocution
  (`canKill`), no per-face blocked sides; the network pooling above.
- **Machines**: no upgrades (`powerMultiplier`/`speedMultiplier` are fixed at
  1), no redstone control (`RedstoneConfiguration`), no muffler/sounds, no
  `setChanged` client sync beyond the menu's data slots.
- **Everything else in the mod**: the other generators (solar, thermal, diesel,
  semifluid, plasma, wind, water, gas, dragon egg), the 20-odd processing
  machines, the storage/tank units, the multiblocks, the ore/metal chain,
  rubber trees and worldgen, tools, armour, cables 2–9, the config file.

### API gap found by this migration (worth an issue)

**No per-side receive/extract on the energy capability.** `EnergyHandler` is
`amount`, `capacity`, `insert`, `extract`: a cable cannot ask a neighbour whether
a side accepts a fill or offers a drain, which NeoForge's
`IEnergyStorage.canReceive`/`canExtract` answers and the retired API-142
`native_energy` helper also answered (it returned `can_insert`/`can_extract`).
The port works around it as the deviation row above says (its own machines are
exact from the buffer they declare; another mod's block is tried and the real
move corrects the accounting); a logistics port with mixed directions would need
the flags.

## Migration to the SDK format

0.1.0 was the retired `.hnmod`: `horizon-mixin.toml` + one helper file per
injection + the content packed beside it as `data/techreborn-content.hmod` and
written out by a `content_root` injection. 0.2.0 is the flat package; the file
map is:

| 0.1.0 | 0.2.0 |
|---|---|
| `mixins/techreborn/horizon-mixin.toml` | `horizon-mod.toml` (`api = "0.2"`, `[native]` pins, `capabilities = ["registries"]`) — the modules, injections and bindings are gone |
| `mixins/techreborn/runtime.rs` | `src/runtime.rs`, **byte-identical** (sha256 `1825e549…c57`), `pub mod runtime` |
| `mixins/techreborn/support.rs` + 9 helper `.rs` files | `src/lib.rs` — one `impl Hooks` plus three `impl BlockEntityPart`/`impl BlockPart` Part structs |
| `mixins/techreborn/data/techreborn-content.hmod` + `content/` sources | `assets/techreborn/`, `data/techreborn/` at the package root; no embedded copy, no `content_root` |
| `content/techreborn.hmod` (built content package) | none: content and code are the same package |
| `techreborn.hnmod`, `compatibility.json`, `rust-toolchain.toml` | gone (the pin lives in `horizon-mod.toml`; the toolchain is the launcher's) |
| `tools/{horizon.py,package.py,assemble_content.py}` | gone: `cargo horizon-mod build` packs, `check` validates, `fits` compares builds |
| `Cargo.toml` (`[lib] path = mixins/…/runtime.rs`) | `Cargo.toml` with `horizon-mod-api = { path = ".horizon/sdk/api/crates/horizon-mod-api" }` |

The retired files are kept verbatim in
`ports/_shared/evidence/techreborn/retired-hnmod-0.1.0/`,
`retired-content-0.1.0/` and `retired-tools/`.

**Manifest mapping.** `[mod] id/version` → `id`/`version`; `[compatibility]
horizon_revision` → `[native] horizon`; the mixins pin moved from the campaign's
`5a64af93` to the SDK's `e59da909`; `[[module]] source = runtime.rs|support.rs` →
`src/runtime.rs` + `src/lib.rs`; every `[[injection]]` → a method of a Part or of
`impl Hooks`; `bindings` → the event fields the method receives.

**Commands, run through a lease** (`ports/tools/build-slot.sh`, `BUILD_LOCAL=1`
so the SDK build stays on this box):

```sh
ports/tools/build-slot.sh run --job sdk-techreborn --port external/techreborn --step test  -- cargo test --offline --manifest-path external/techreborn/Cargo.toml
ports/tools/build-slot.sh run --job sdk-techreborn --port external/techreborn --step lint  -- cargo clippy --offline --all-targets --manifest-path external/techreborn/Cargo.toml -- -D warnings
ports/tools/build-slot.sh run --job sdk-techreborn --port external/techreborn --step build -- bash -c 'cd external/techreborn && cargo horizon-mod build && cp target/horizon-mod/techreborn.hmod .'
ports/tools/build-slot.sh run --job sdk-techreborn --port external/techreborn --step check -- bash -c 'cd external/techreborn && cargo horizon-mod check techreborn.hmod'
ports/tools/build-slot.sh run --job sdk-techreborn --port external/techreborn --step probe -- bash -c 'cd external/techreborn && cargo horizon-mod fits techreborn.hmod'
```

`build-slot.sh run` does **not** change directory, so a port-relative command
needs `--manifest-path` or an explicit `cd`; `cargo horizon-mod` needs the `cd`
(it takes the package from the current directory).

**Traps this migration hit**

1. **`BlockKey`, `ItemKey`, `BlockEntityTypeKey`, `BlockState` are opaque.** The
   old plan compared `&str` block ids; a part is attached to a key, so no id
   comparison is needed — except in the harness command, which prints them, and
   does it with the keys' own `id(host)` (not a host method, which does not
   exist for the reverse direction).
2. **`cargo horizon-mod run` refuses code mods**: *"code mods run through the
   launcher's native build"*. In-game verification needs
   `horizon-launcher native-build <port> --instance <i> --trust-native-code`,
   and this box has no launcher binary, so **the e2e is written and syntax
   checked but not run** (see Verification).
3. **`cargo test` compiles `src/lib.rs`, not just `runtime.rs`.** The SDK has to
   be in `.horizon/sdk/` (any `cargo horizon-mod` command fetches it) or the
   port crate does not build at all. That is also the compile evidence for the
   glue, since `check` only validates the package.
4. **`Compound` keys are `JavaString`, which is not exported.** `nbt.get(&key.into())`
   and `nbt.insert(key.into(), …)` are inferred; `horizon_core::nbt::JavaString`
   cannot be named from a mod crate (only `horizon-mod-api` is a dependency).
5. **Host methods need the area traits in scope.** `use mod_api::prelude::*;`
   (the prelude names every host area); `LevelRef`/`BlockEntityRef` methods are
   inherent on the handles and need no import.
6. **`ServerHost` has no `block_state(pos)`.** The retired API read the level in
   place; the SDK reads through a `LevelRef`
   (`level.block_state/block/block_property/recipe_for`). Inside a block entity
   tick the level in place is found with
   `host.levels().list_levels().find(|l| l.in_place(host))`.
7. **A tick's write-back is optional.** Answering `Unchanged` (or editing
   nothing) keeps the chunk clean; the port edits the fields it always did and
   lets the default `Changed` apply them.
8. **`block_entity` must answer `Some(Compound)`** for the declared type, or a
   state-aware factory creates no entity; the three Parts all return an empty
   compound, as 0.1.0 did.
9. **`KeysHost` resolves a name *to* a key only.** The reverse is the keys' own
   inherent `BlockKey::id(host)`, `ItemKey::id(host)`,
   `BlockEntityTypeKey::id(host)`; `host.item_key_id(..)` does not exist and
   costs a compile cycle to discover. `BlockEntityPart::block_entity` also has a
   `ModBlockEntityView::id()` of its own (the entity's type id).
10. **A block *value* hook needs the part on the block's table — and `block_entity_part` puts it
    there.** `state_for_placement` is dispatched through `table.block_value(state)`, and the block
    half of a block-entity part is inserted by `expand_block_entity_parts` after every mod's
    `register`. This note originally concluded the opposite ("`block_entity_part` does not fill
    it, so attach the same struct with `block_part` too"), which attached each machine's Part twice
    and made the engine refuse the whole package at load — fixed in 0.2.1, see the last section.
    The rule: one Part, one `block_entity_part`; never both.
11. **No registration-level unit test is available in the package.** The SDK's
    `mod_api::testing::FakeHost` and `__private::part_tests` sit behind the
    `testing` feature (and `cfg(test)` of the SDK crate), so the declarations
    (`block_energy`/`block_container`/`block_entity_type`) and the Parts are
    checked by `cargo horizon-mod check` (package shape) and by reading the
    dispatch tables, not by a test of this crate. `cargo test` covers the pure
    logic only.

### What generalises to the next huge port

- The one-file-per-hook shape collapses into **one Part struct per block/entity
  type plus one `impl Hooks`**, and the mapping is mechanical when each old
  helper was already narrow: declaration hooks become `Registrar` calls or Part
  methods, global hooks become `Hooks` methods, and a moment about one object
  becomes a Part method.
- **The embedded-content pattern disappears.** A huge port's content half moves
  to `assets/`/`data/` at the package root: no `content_root` injection, no
  `native_data!`, no `.horizon-cache` write, no "is the embedded file identical"
  e2e leg. That removed 1 of 9 hooks and about a third of the old e2e.
- **`runtime.rs` carries over untouched** — this is the second huge port after
  `twilightforest` where the pure file needed no edit, so the campaign's rule
  holds: the glue is what a format change costs.
- What does **not** generalise: a machines port needs the declaration trio
  (`block_entity_type` + `block_container`/`block_energy`) which a logic or
  content port never touches, and any port that reads a *neighbour's* capability
  hits the per-side flag gap above.

## Verification

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port

**0.2.0 (SDK format), this migration** — logs in
`ports/_shared/evidence/techreborn/sdk-*.log`:

- `src/runtime.rs` is **byte-identical** to the 0.1.0 file (sha256
  `1825e54947d24f111def3c94960fe13572a09f4f8d38c6740b4c2879e82b7c57`), and its
  tests were run **before** any glue existed: `cargo test` **25/25**.
- `cargo test --offline` **25/25** · `cargo clippy --all-targets -- -D warnings`
  **0** · `cargo fmt --check` **0**, all after the glue.
- `cargo horizon-mod build` → `techreborn.hmod` **45,798 B**, sha256
  `47a5cbc11ca0e355051bec566cb05b091a1cbb5e60952b08207fdbc42ebb9e43`, **43
  entries**; a second build reproduces the same sha256 byte for byte;
  `cargo horizon-mod check` passes with the exact pins; `cargo horizon-mod fits`
  reports the package is made for `0b14f8a`.
- **The e2e was not run**: this box has no `horizon-launcher`, and the SDK
  compiles code mods only through `horizon-launcher native-build`. The script
  (`tools/e2e.sh`) is rewritten for that flow (no `--dev-mod`, no embedded-content
  control leg), passes `bash -n`, and fails loudly with the build command when
  `GAME` is unset. **No behaviour claim is made for 0.2.0**; the 0.1.0 run below
  is what the slice's behaviour evidence rests on, and it exercised the same
  `runtime.rs` and the same three ticks.

**0.1.0 (retired `.hnmod`)** — evidence under
`ports/_shared/evidence/techreborn/retired-hnmod-0.1.0/` (the bundle, its
`mixins/`, and `logs/` with the old ladder and the three `e2e-*.log` legs):
embedded 25/25, `.hnmod` 14
entries, e2e default/embedded `PASS`+`EXIT:0`, `SELFTEST=1` `FAIL`+`EXIT:1`.

## Upstream files read

`blockentity/generator/basic/SolidFuelGeneratorBlockEntity.java`,
`blockentity/cable/{CableBlockEntity,CableTickManager}.java`,
`blocks/cable/{CableBlock,CableShapeUtil}.java`,
`blockentity/machine/tier1/ElectricFurnaceBlockEntity.java`,
`RebornCore/.../powerSystem/{PowerAcceptorBlockEntity,RcEnergyTier}.java`,
`RebornCore/.../blockentity/MachineBaseBlockEntity.java`,
`RebornCore/.../blocks/BlockMachineBase.java`, `init/{TRContent,TRBlockSettings,FuelRecipes}.java`,
`config/TechRebornConfig.java`, plus the jar's `fabric.mod.json`, blockstates,
models, `items/*.json`, `loot_table/blocks/*`, `lang/en_us.json` and the recipe
files named in `FINDINGS.md` §3.

## Packaging gate and in-game leg (2026-11-20)

**0.2.0 never loaded.** The package carried a packed `Cargo.lock` (43 entries) *and* attached two
Parts twice; the lock was fixed first (`rm -f Cargo.lock` before **each** build), which exposed the
load refusal the lock had been hiding behind:

```
Horizon: Mod parts refused:
techreborn::GeneratorPart is attached to techreborn:solid_fuel_generator twice, by techreborn and techreborn
techreborn::FurnacePart is attached to techreborn:electric_furnace twice, by techreborn and techreborn
```

`src/lib.rs` attached `GeneratorPart`/`FurnacePart` with `r.block_part(...)` **and** with
`r.block_entity_part(...)`. The API's `expand_block_entity_parts`
(`horizon-mod-api/src/parts/block_entities.rs`) already inserts the `BlockPart` half of every
block-entity part onto every block of its declared type, so the `block_part` call was the second
attachment and `insert_part` refused the type. **Fixed in 0.2.1** by deleting the two `block_part`
calls; the block half — `state_for_placement`, the placement facing this port depends on — still
arrives through `block_entity_part`, which the in-game leg's phase P proves (three placements, three
facings).

### The fix, and the test that pins it (0.2.1, 2026-11-20)

| step | command | result |
|---|---|---|
| failing test first | `cargo test --offline` | **25 passed, 1 failed** — `registration::the_engine_accepts_every_part_of_this_package` printed the engine's own verdict: `Err(["techreborn::GeneratorPart is attached to techreborn:solid_fuel_generator twice, by techreborn and techreborn", "techreborn::FurnacePart is attached to techreborn:electric_furnace twice, by techreborn and techreborn"])` (`evidence/techreborn/registration-red.log`) |
| tests | `cargo test --offline` | **26 passed**, 0 failed, exit 0 (`sdk-test.log`, `registration-green.log`) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy.log`) |
| format | `cargo fmt --check` | exit 0 (`sdk-fmt.log`) |
| build 1 + 2 | `rm -f Cargo.lock && cargo horizon-mod build`, two runs `cmp` | **byte-identical**, no `Cargo.lock` before or after |
| package | `unzip -l techreborn.hmod` | **45,253 B**, SHA-256 `5bc5a327a1715b600b641b4569abfea46de6639da17c863f38b3a971a9604b70`, **42 entries** |
| package check | `cargo horizon-mod check techreborn.hmod` | `techreborn 0.2.1 (native-mod, format native-1/0b14f8a/e59da90, for 26.3)`, exit 0 (`sdk-check.log`) |
| fits | `cargo horizon-mod fits techreborn.hmod --build 0b14f8a` | exit 0, *"0b14f8a made for"* (`sdk-fits.log`) |
| gate | `python3 ports/tools/verify-sdk-package.py techreborn` | **1/1** |
| before (0.2.0) | the same sources with the two `block_part` calls restored, launcher-built and launched | **game refuses the package**, `LAUNCH-EXIT:3` (`evidence/techreborn-e2e/e2e-launcher-before.log`) |
| in-game (modded) | `launcher-game.sh build techreborn --port external/techreborn`, then the leg in a slot | **`PASS` + `EXIT:0`** (`evidence/techreborn-e2e/e2e-launcher.log`) |
| control | same script, `GAME=$CONTROL_GAME` (same Nightly, no package) | `Unknown item 'techreborn:solid_fuel_generator'` → **`EXIT:1`** (`…-control.log`) |
| selftest | `SELFTEST=1` (flips C's 4,000 to 4,001) | `FAIL: the block holds 4000 E, expected 4001` → **`EXIT:1`** (`…-selftest.log`) |
| log gate | `check-e2e-logs.py techreborn-e2e --strict` | `logs=4 ok=4` |

The registration test is the SDK's own path, not a copy of its rule: it builds a `RuleSchema`, puts
this crate's `Mod` in a `ModEntry`, and calls `mod_api::__private::install_parts`, which is what the
game calls at startup. It is the one test that installs parts (the state is process-wide), so it
must stay the only one.

The in-game run's phase P is what proves the *behaviour* the deleted call was believed to provide:
`placed from [-2, 0] … facing west`, `[2, 0] … facing east`, `[0, -2] … facing north` — all three
from `BlockPart::state_for_placement`, dispatched through `table.block_value`, reached through the
block half that `block_entity_part` installs.

**Trap 10 above is wrong and was the cause.** `block_entity_part` *does* fill the block table (in
`expand_block_entity_parts`, after every mod's `register` has run); a `block_part` call for the same
type is a second attachment, not a requirement.

