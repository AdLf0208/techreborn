# Licence notice

The port is a derivative work of **Tech Reborn 6.2.1** (Team Reborn), MIT.

Read from:

- `fabric.mod.json` inside `TechReborn-6.2.1.jar`:
  `"license": "MIT"`, `"sources": "https://github.com/TechReborn/TechReborn"`;
- the `LICENSE.md` of that repository (MIT), and the MIT header carried by every
  source file, e.g.
  `Copyright (c) 2020 TechReborn` in
  `src/main/java/techreborn/blockentity/generator/basic/SolidFuelGeneratorBlockEntity.java`.

The files this port carries verbatim (blockstates, block and item models,
textures, item definitions, loot tables, English names) are Tech Reborn's own
MIT assets, read from the 26.3 build of the same release. The Rust code is a
clean-room rewrite of the observed behaviour (see `FINDINGS.md`); no Java source
is copied or translated.

Publishing a port still needs the campaign maintainer's approval
(`ARCHITECTURE.md` §8).
