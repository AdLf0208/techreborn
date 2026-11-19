# Tech Reborn for Horizon — slice 1

A [Horizon](https://github.com/horizon-rust/horizon) native port of
[Tech Reborn](https://github.com/TechReborn/TechReborn) 6.2.1 (MIT), for
Minecraft Java 26.3. One package (`techreborn.hmod`, mod API 0.2) carries the
blocks, items, models, textures, loot tables and lang **and** the Rust behaviour.

## What it does

The smallest closed energy loop of the mod:

- **Solid fuel generator** (`techreborn:solid_fuel_generator`) — burns any item
  with a `minecraft:cooking_fuel` component for `burn_time / 4` steps and makes
  **10 E** a step into a 10,000 E buffer (coal: 400 steps, 4,000 E).
- **Copper cable** (`techreborn:copper_cable`) — moves up to **128 E** a tick
  either way through a 512 E buffer, joining its six sides.
- **Electric furnace** (`techreborn:electric_furnace`) — smelts vanilla smelting
  recipes for their own cooking time at **1 E** a tick (one raw iron: 200 ticks,
  200 E) into a 1,000 E buffer, with three slots and the furnace menu.

## What it does not do (yet)

No crafting recipes (they need Tech Reborn items outside this slice), no
generator GUI, no battery items, no upgrades, no wrenching, no other machines,
storage units, multiblocks, cables, ores or tools. `notes.md` lists the gaps and
the deliberate deviations; `FINDINGS.md` has every number with its upstream file.

## Install

The launcher compiles a native mod into the game it runs, so the package is
built for one exact Horizon build:

```sh
horizon-launcher native-build path/to/techreborn --instance <instance> --trust-native-code
```

The manifest pins Horizon `0b14f8ac` and mixins `e59da909`; `cargo horizon-mod
check techreborn.hmod` validates the package, and `cargo horizon-mod fits
techreborn.hmod` says which builds it fits.

## Build the package

```sh
cargo horizon-mod build          # writes target/horizon-mod/techreborn.hmod
cargo test                       # the pure logic in src/runtime.rs (25 tests)
```

## Licence

MIT, as upstream. `LICENSE` holds the upstream notice; `LICENSE-NOTICE.md` says
where it was read from. The carried assets (models, textures, lang, loot
tables, blockstates) are Tech Reborn's own MIT files, taken from the 26.3 build.
