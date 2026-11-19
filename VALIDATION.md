# Validation — techreborn slice 1

Evidence lives in `ports/_shared/evidence/techreborn/`. Every line below is a
command that was run and its observed result; nothing here is inferred.

The package changed format on 2026-11-18 (mod API 0.2, flat `.hmod`). §1–§3 and
§6 describe **0.2.0**, the current package; §4–§5 are the **retired 0.1.0**
`.hnmod` run, kept because it is the slice's behaviour evidence — the same
`src/runtime.rs` (byte-identical) and the same three ticks ran there.

## 1. Port crate (the pure runtime, unchanged)

```sh
external/techreborn$ cargo fmt --check                                          # rc 0
external/techreborn$ cargo clippy --all-targets --offline -- -D warnings         # rc 0, 0 warnings
external/techreborn$ cargo test --offline runtime::                              # 25 passed, 0 failed
external/techreborn$ cargo test --offline                                        # 25 passed, 0 failed
```

`src/runtime.rs` is **byte-identical** to the 0.1.0 `mixins/techreborn/runtime.rs`
(sha256 `1825e54947d24f111def3c94960fe13572a09f4f8d38c6740b4c2879e82b7c57`), and
its 25 tests were run green **before** `src/lib.rs` had any glue: the migration's
line is "logic untouched, glue rewritten", so the glue was written afterwards and
the tests were run again (same 25, plus the glue compiling as part of the crate).

Logs: `sdk-test.log` (`runtime::`), `sdk-test-all.log`, `sdk-clippy.log`,
`sdk-fmt.log`.

TDD (0.1.0, unchanged): the first run was **red** — `10 passed; 14 failed` with
stub bodies — then green after the implementation. Two test corrections came out
of that red run and are the record of real behaviour: a *fresh* generator only
sets `active` on the tick that loads fuel, and a neighbour that both gives and
takes gets a pull **and** a push in one tick.

## 2. SDK ladder (inside a build slot)

`ports/tools/build-slot.sh run` does not change directory, so each step is given
either `--manifest-path` or an explicit `cd`; `BUILD_LOCAL=1` keeps the SDK build
on this box (`remote-slot.sh` syncs by `ports/<name>` and cannot mirror
`external/techreborn`).

```sh
BUILD_LOCAL=1 ports/tools/build-slot.sh run --job sdk-techreborn --port external/techreborn --step test  -- cargo test --offline runtime:: --manifest-path external/techreborn/Cargo.toml
BUILD_LOCAL=1 ports/tools/build-slot.sh run --job sdk-techreborn --port external/techreborn --step test  -- cargo test --offline --manifest-path external/techreborn/Cargo.toml
BUILD_LOCAL=1 ports/tools/build-slot.sh run --job sdk-techreborn --port external/techreborn --step lint  -- cargo clippy --offline --all-targets --manifest-path external/techreborn/Cargo.toml -- -D warnings
BUILD_LOCAL=1 ports/tools/build-slot.sh run --job sdk-techreborn --port external/techreborn --step fmt   -- cargo fmt --check --manifest-path external/techreborn/Cargo.toml
BUILD_LOCAL=1 ports/tools/build-slot.sh run --job sdk-techreborn --port external/techreborn --step build -- bash -c 'cd external/techreborn && cargo horizon-mod build && cp target/horizon-mod/techreborn.hmod .'
BUILD_LOCAL=1 ports/tools/build-slot.sh run --job sdk-techreborn --port external/techreborn --step check -- bash -c 'cd external/techreborn && cargo horizon-mod check techreborn.hmod'
BUILD_LOCAL=1 ports/tools/build-slot.sh run --job sdk-techreborn --port external/techreborn --step probe -- bash -c 'cd external/techreborn && cargo horizon-mod fits techreborn.hmod'
```

| step | result | log |
|---|---|---|
| `cargo test` (`runtime::`) | **25 passed, 0 failed**, exit 0 | `sdk-test.log` |
| `cargo test` (whole crate) | **25 passed, 0 failed**, exit 0 | `sdk-test-all.log` |
| `cargo clippy -D warnings` | exit 0, **0 warnings** | `sdk-clippy.log` |
| `cargo fmt --check` | exit 0 | `sdk-fmt.log` |
| `cargo horizon-mod build` | exit 0, `Built ./target/horizon-mod/techreborn.hmod` | `sdk-build.log` |
| `cargo horizon-mod check` | exit 0 | `sdk-check.log` |
| `cargo horizon-mod fits` | exit 0 | `sdk-fits.log` |

## 3. The package

```sh
external/techreborn$ cargo horizon-mod check techreborn.hmod
techreborn 0.2.0 (native-mod, format native-1/0b14f8acfd97fa2c750e42795249dcc981178ca4/e59da9094f0c2fdda6c627fe9eff22dec6a906f0, for 26.3)
  45798 bytes, SHA-256 47a5cbc11ca0e355051bec566cb05b091a1cbb5e60952b08207fdbc42ebb9e43, unsigned

external/techreborn$ cargo horizon-mod fits techreborn.hmod
Tech Reborn:
0b14f8a  made for
```

`techreborn.hmod` **45,798 B**, sha256
`47a5cbc11ca0e355051bec566cb05b091a1cbb5e60952b08207fdbc42ebb9e43`, **43
entries**: `horizon-mod.toml`, `Cargo.toml`, `Cargo.lock`, `LICENSE`,
`LICENSE-NOTICE.md`, `README.md`, `src/lib.rs`, `src/runtime.rs`, `uses.json`,
3 item definitions, 3 blockstates, 7 models, 12 textures, `lang/en_us.json`,
3 block declarations, 1 creative tab, 3 loot tables, 1 block tag. A second
`cargo horizon-mod build` produced the **same sha256**.

## 4. In-game e2e — NOT RUN for 0.2.0

The SDK compiles a code mod **into the game** through the launcher:

```sh
horizon-launcher native-build external/techreborn --instance <instance> --trust-native-code
GAME=<that horizon-client> external/techreborn/tools/e2e.sh
```

This box has no `horizon-launcher` binary (`cargo horizon-mod run` answers
"code mods run through the launcher's native build"), so the scenario could not
be run for the new package. `tools/e2e.sh` is rewritten for that flow (no
`--dev-mod`; the content is in the same package; no embedded-content control
leg), passes `bash -n`, and refuses to start with the exact build command when
`GAME` is unset. **No 0.2.0 behaviour claim is made from it.**

The retired 0.1.0 run is the behaviour evidence for the slice:

| leg | log (under `retired-hnmod-0.1.0/logs/`) | verdict |
|---|---|---|
| default (`--dev-mod content/`) | `e2e-default.log` | `PASS` + `EXIT:0` |
| embedded (no `--dev-mod`) + unusable-`.horizon-cache` control | `e2e-embedded.log` | `PASS` + `EXIT:0` |
| `SELFTEST=1` (asserts 4,001 E for one coal) | `e2e-selftest.log` | `FAIL` + `EXIT:1` |

What that run proved, from its log:

- a machine placed from its item takes the placer's facing: from the west
  `facing=west`, from the east `facing=east`, from the north `facing=north`;
- a fresh generator is `0/10000`, `active=false`;
- a non-fuel item (dirt) makes no energy over 40 ticks and stays in the slot;
- **one coal is exactly 4,000 E** and leaves `burn=0/0`, `active=false`;
- **control**: a furnace with raw iron and 0 E never advances (`cook=0/200`
  after 220 ticks) and produces nothing;
- **1,000 E smelts one raw iron** in the recipe's 200 ticks: output
  `minecraft:iron_ingotx1`, buffer `986 → 800` (200 E spent);
- **generator → cable → furnace**: one iron ingot, and the three buffers hold
  `2288 + 512 + 1000 = 3800` = 4,000 − 200 (energy moved, never made);
- **differential**: a generator beside a furnace with no cable between them
  makes 3,090 E that never arrives — the furnace stays at 0 E with no output.

## 5. Gates

```sh
python3 ports/tools/check-skills.py --notes external/techreborn   # PASS (Skills: line present)
cargo horizon-mod check techreborn.hmod                           # rc 0
cargo horizon-mod fits techreborn.hmod                            # rc 0
```

`check-e2e-logs.py techreborn --strict` now finds **no e2e log for 0.2.0** —
because none was run (§4) — so it reports the current package as unverified
in-game. That is the honest state, not a regression of the 0.1.0 evidence kept
under `retired-hnmod-0.1.0/logs/`.

`upstream_ready.py techreborn` still reports `status is pending`: the ledger row
is the orchestrator's to record (this brief forbids editing `ports/ledger.csv`).
