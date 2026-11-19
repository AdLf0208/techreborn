#!/usr/bin/env bash
# In-game check for Tech Reborn slice 1: a solid fuel generator makes energy out of a burnable
# item, the copper cable carries it, and the electric furnace smelts a vanilla recipe with it.
#
# Every mod command is typed with `ctl chat "/cmd"` because `ctl command` runs the console path
# and bypasses the mod's `run_mod_command` hook.
#
# Since the SDK migration (2026-11-18) the mod is one flat `.hmod` (`horizon-mod.toml`,
# `src/`, `assets/`, `data/`) that the launcher compiles into the game, so this script needs a
# game the launcher built with it:
#
#   horizon-launcher native-build external/techreborn --instance <instance> --trust-native-code
#   GAME=<that horizon-client> tools/e2e.sh
#
# There is no `--dev-mod` leg any more: the content (blocks, items, models, textures, loot
# tables, lang) is inside the same package as the code. The retired 0.1.0 `.hnmod` run, with an
# embedded-content control leg, is kept under `ports/_shared/evidence/techreborn/retired-*/`.
#
# The End, in the void at x=200 (away from the central island, its dragon egg and the ambient
# item entities of a generated level). The stone floor is at y=100 and every block stands on
# y=101, spaced 4 apart in z so a block never sits between the player and the next target face.
#
# Default run:
#  P  a machine placed from its item faces the placer (the state carries the
#     quadrant of the player's yaw, as `BlockMachineBase.setPlacedBy` sets it);
#  A  a generator is empty and inactive;
#  B  a non-fuel item in the fuel slot makes no energy and is not used up;
#  C  one coal is 1,600 smelting ticks / 4 = 400 burn steps x 10 E = 4,000 E,
#     after which the generator is empty of steps and inactive;
#  D  CONTROL: a furnace with raw iron and no energy idles: no cook, no output;
#  E  a furnace given 1,000 E smelts one raw iron in the recipe's 200 ticks for
#     exactly 200 E (the output is an iron ingot, the buffer 800);
#  F  generator -> cable -> furnace: one iron ingot is smelted and the three
#     buffers hold exactly 4,000 - 200 = 3,800 E (energy is moved, not made);
#  G  DIFFERENTIAL: a generator beside a furnace with no cable between them
#     makes energy that never arrives: the furnace stays empty and idle.
#
# SELFTEST=1 flips C's 4,000 assertion (it must FAIL), proving the scenario can fail.
set -Eeuo pipefail
export LC_ALL=C
[ -z "${TR_DEBUG:-}" ] || set -x

CLONE=${CLONE:-$(cd "$(dirname "$0")/../../.." && pwd)}
PORT=$CLONE/external/techreborn
CTL=${CTL:-}
GAME=${GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
SOCK=${SOCK:-/tmp/opencode/techreborn-e2e.sock}
SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}
WORLD=TechRebornE2E
# Where the arena stands: the End's void, far from the central island.
X0=200
SX=$((X0 + 2))
GENERATOR=techreborn:solid_fuel_generator
CABLE=techreborn:copper_cable
FURNACE=techreborn:electric_furnace

command -v jq >/dev/null || { echo "missing jq (every assertion parses a ctl reply)" >&2; exit 1; }
[ -n "$CTL" ] || {
    echo "CTL is unset: eval \"\$(ports/tools/launcher-game.sh env techreborn)\"" >&2
    exit 1
}
[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -n "$GAME" ] || {
    echo "GAME is unset: the SDK package is compiled into the game by the launcher." >&2
    echo "  ports/tools/launcher-game.sh build techreborn --port external/techreborn" >&2
    echo "  eval \"\$(ports/tools/launcher-game.sh env techreborn)\"" >&2
    exit 1
}
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/techreborn-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
say() { ctl chat "/$1" >/dev/null; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 60 >&2 || true; exit 1; }
on_error() { echo "FAIL: line $1: '$2' exited $?" >&2; ctl logs --lines 40 >&2 || true; }
trap 'on_error "$LINENO" "$BASH_COMMAND"' ERR

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "$KEEP" = 1 ]; then
        echo "kept scratch: $SCRATCH" >&2
    else
        rm -rf "$SCRATCH" 2>/dev/null || true
    fi
    # Last line of every run: `check-e2e-logs.py` reads it as the verdict.
    echo "EXIT:$status"
}
trap cleanup EXIT

launch() { # launch [WORLD]
    local world=${1:-$WORLD}
    rm -f "$SOCK"
    cd "$SCRATCH"
    "$CTL" --socket "$SOCK" launch --game "$GAME" --world "$world" --seed 20261105 --creative \
        --headless --wait --resources "$RES" >/dev/null
    ctl wait-ready 120 >/dev/null
    ctl wait 40 >/dev/null
}

block_name() { ctl block "$1" "$2" "$3" | jq -r '.block // empty'; }
block_state() { ctl block "$1" "$2" "$3" | jq -r '.state // empty'; }
state_field() { tr ',' '\n' <<<"$1" | grep -m1 -o "$2=[^],]*" | cut -d= -f2; }

# The End's void at X0: a stone floor and clear air above it. The player is
# teleported first, so the chunks the fills touch are loaded.
arena() {
    ctl command "execute in minecraft:the_end run tp @s $X0.5 104 0.5" >/dev/null
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5 >/dev/null
        if [ "$(ctl block "$X0" 100 0 | jq -r '.loaded // false')" = true ]; then
            loaded=true
            break
        fi
    done
    [ "$loaded" = true ] || fail "the End arena chunk never loaded"
    ctl command "execute in minecraft:the_end run fill $((X0 - 2)) 100 -20 $((X0 + 4)) 100 20 minecraft:stone" >/dev/null
    ctl wait 6 >/dev/null
    ctl command "execute in minecraft:the_end run fill $((X0 - 2)) 101 -20 $((X0 + 4)) 108 20 minecraft:air" >/dev/null
    ctl wait 6 >/dev/null
    ctl command "execute in minecraft:the_end run kill @e[type=!player]" >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $X0.5 101 0.5" >/dev/null
    ctl wait 20
    [ "$(block_name "$X0" 100 0)" = "minecraft:stone" ] || fail "the arena floor is not stone"
    [ "$(block_name "$X0" 101 0)" = "minecraft:air" ] || fail "the arena air above the floor is not air"
}

# `/techreborn info X Y Z`: the line, then the parsed fields in ENERGY,
# CAPACITY, ACTIVE, BURN, BURN_TOTAL, COOK, COOK_TOTAL, INPUT, OUTPUT, TIME.
info() { # info X Y Z
    say "techreborn info $1 $2 $3"
    ctl wait 12 >/dev/null
    INFO_LINE=$(ctl messages 12 | jq -r '.chat[]' | grep -m1 'Tech Reborn ' || true)
    [ -n "$INFO_LINE" ] || fail "no /techreborn info reply"
    eval "$(python3 "$PORT/tools/parse_info.py" "$INFO_LINE")"
    [ "${ok:-0}" = 1 ] || fail "the info line did not parse: $INFO_LINE"
    echo "  $INFO_LINE" >&2
}

expect_energy() { [ "$ENERGY" = "$1" ] || fail "the block holds $ENERGY E, expected $1"; }
expect_active() { [ "$ACTIVE" = "$1" ] || fail "the block reads active=$ACTIVE, expected $1"; }

# A block placed with `setblock`, read back to prove the id resolved.
place() { # place X Y Z ID
    ctl command "execute in minecraft:the_end run setblock $1 $2 $3 $4" >/dev/null || true
    ctl wait 4 >/dev/null
    local name
    name=$(block_name "$1" "$2" "$3")
    [ "$name" = "$4" ] || fail "the block at $1,$2,$3 is '$name', expected $4"
}

# `ctl container X Y Z ITEM COUNT` fills the block entity's first empty slot.
give_to() { # give_to X Y Z ITEM [COUNT]
    ctl container "$1" "$2" "$3" "$4" "${5:-1}" >/dev/null
    ctl wait 6 >/dev/null
}

echo "== launch (The End arena, the one package carries content and code) =="
launch
arena

ctl command "item replace entity @s hotbar.0 with $GENERATOR 1" >/dev/null
ctl command "item replace entity @s hotbar.1 with $CABLE 1" >/dev/null
ctl command "item replace entity @s hotbar.2 with $FURNACE 1" >/dev/null
ctl wait 4 >/dev/null
ctl inventory | jq -e --arg g "$GENERATOR" --arg c "$CABLE" --arg f "$FURNACE" \
    '[.hotbar[], .main[] | .item] | (index($g) != null) and (index($c) != null) and (index($f) != null)' >/dev/null \
    || fail "the three items did not register: $(ctl inventory | jq -c '{hotbar,main}')"
echo "  the generator, cable and furnace items registered"

echo "== P: a machine placed from its item faces the placer =="
# `BlockMachineBase.setPlacedBy`: facing = `placer.getDirection().getOpposite()`.
# Standing west of the block and looking east at it, the front ends up west.
place_facing() { # place_facing X Z FROM_DX FROM_DZ EXPECTED
    local x=$1 z=$2 dx=$3 dz=$4 expected=$5
    ctl command "execute in minecraft:the_end run tp @s $((x + dx)).5 101 $((z + dz)).5" >/dev/null
    ctl wait 6 >/dev/null
    ctl look-at-block "$x" 100 "$z" up >/dev/null
    ctl wait 2 >/dev/null
    ctl command "item replace entity @s hotbar.0 with $GENERATOR 1" >/dev/null
    ctl select 1 >/dev/null
    ctl place "$x" 101 "$z" --item "$GENERATOR" >/dev/null
    ctl wait 6 >/dev/null
    local placed facing
    placed=$(block_state "$x" 101 "$z")
    [ "$(block_name "$x" 101 "$z")" = "$GENERATOR" ] || fail "the generator did not place at $x,101,$z: $placed"
    facing=$(state_field "$placed" facing)
    [ "$facing" = "$expected" ] || fail "the generator at $x,101,$z faces '$facing', expected $expected ($placed)"
    [ "$(state_field "$placed" active)" = "false" ] || fail "a fresh generator reads active: $placed"
    echo "  placed from [$dx, $dz] looking at it: facing $facing, $placed"
}
place_facing "$SX" 0 -2 0 west
place_facing "$SX" -8 2 0 east
place_facing "$SX" -12 0 -2 north

echo "== A: a generator is empty and inactive =="
place "$SX" 101 4 "$GENERATOR"
info "$SX" 101 4
expect_energy 0
expect_active false
[ "$CAPACITY" = 10000 ] || fail "the generator's buffer is $CAPACITY E, expected 10,000"
[ "$INPUT" = empty ] || fail "a fresh generator holds $INPUT"

echo "== B: a non-fuel item in the fuel slot makes no energy and is kept =="
give_to "$SX" 101 4 minecraft:dirt
info "$SX" 101 4
[ "$INPUT" = "minecraft:dirtx1" ] || fail "the fuel slot holds $INPUT, expected one dirt"
ctl wait 40 >/dev/null
info "$SX" 101 4
expect_energy 0
[ "$INPUT" = "minecraft:dirtx1" ] || fail "the dirt was used up: $INPUT"
expect_active false
echo "  still $ENERGY E after 40 ticks, fuel $INPUT"

echo "== C: one coal is 400 burn steps, exactly 4,000 E =="
place "$SX" 101 -4 "$GENERATOR"
give_to "$SX" 101 -4 minecraft:coal
ctl wait 460 >/dev/null
info "$SX" 101 -4
if [ "$SELFTEST" = 1 ]; then
    echo "(selftest: asserting the generator holds 4,001)"
    expect_energy 4001
else
    expect_energy 4000
fi
[ "$BURN" = 0 ] || fail "the coal has $BURN steps left, expected 0"
[ "$BURN_TOTAL" = 0 ] || fail "the burn total is $BURN_TOTAL, expected 0 once the fuel is spent"
expect_active false
[ "$INPUT" = empty ] || fail "the fuel slot still holds $INPUT"
echo "  one coal: $ENERGY E, $BURN/$BURN_TOTAL steps, active=$ACTIVE"

echo "== D: control — a furnace with no energy idles =="
place "$SX" 101 8 "$FURNACE"
give_to "$SX" 101 8 minecraft:raw_iron
ctl wait 120 >/dev/null
info "$SX" 101 8
expect_energy 0
# Upstream reads `active` as "a recipe is current", not "burning": the recipe
# and its 200 ticks are known the moment the input arrives, so `active=true`
# here is faithful. Nothing cooks without energy.
[ "$COOK_TOTAL" = 200 ] || fail "the unpowered furnace's recipe total is $COOK_TOTAL, expected the recipe's 200 ticks"
[ "$COOK" = 0 ] || fail "the unpowered furnace cooked $COOK ticks"
[ "$OUTPUT" = empty ] || fail "the unpowered furnace produced $OUTPUT"
ctl wait 100 >/dev/null
info "$SX" 101 8
[ "$COOK" = 0 ] || fail "the unpowered furnace reached cook $COOK after another 100 ticks"
[ "$OUTPUT" = empty ] || fail "the unpowered furnace produced $OUTPUT"
ctl container "$SX" 101 8 | jq -e '.slots[1] == null' >/dev/null \
    || fail "the unpowered furnace's output slot is not empty"
echo "  raw iron with no energy: cook $COOK/$COOK_TOTAL after 220 ticks, out $OUTPUT"

echo "== E: 1,000 E smelts one raw iron in the recipe's 200 ticks for 200 E =="
say "techreborn energy $SX 101 8 1000"
ctl wait 12 >/dev/null
info "$SX" 101 8
START=$ENERGY
[ "$START" -le 1000 ] && [ "$START" -ge 980 ] || fail "the harness set $START E, expected about 1,000"
ctl wait 240 >/dev/null
info "$SX" 101 8
[ "$OUTPUT" = "minecraft:iron_ingotx1" ] || fail "the furnace produced $OUTPUT, expected one iron ingot"
[ "$INPUT" = empty ] || fail "the furnace still holds $INPUT in its input slot"
expect_energy 800
expect_active false
ctl container "$SX" 101 8 | jq -e '.slots[1].item == "minecraft:iron_ingot"' >/dev/null \
    || fail "the output slot does not hold an iron ingot: $(ctl container "$SX" 101 8 | jq -c .slots)"
echo "  one iron ingot, $START -> $ENERGY E (200 ticks at 1 E a tick)"

echo "== F: generator -> cable -> furnace smelts, and the energy balances =="
place "$SX" 101 12 "$GENERATOR"
place "$SX" 101 13 "$CABLE"
place "$SX" 101 14 "$FURNACE"
give_to "$SX" 101 14 minecraft:raw_iron
give_to "$SX" 101 12 minecraft:coal
ctl wait 560 >/dev/null
info "$SX" 101 14
[ "$OUTPUT" = "minecraft:iron_ingotx1" ] || fail "the cable did not feed the furnace: out $OUTPUT"
FURNACE_E=$ENERGY
info "$SX" 101 12
GENERATOR_E=$ENERGY
info "$SX" 101 13
CABLE_E=$ENERGY
TOTAL=$((GENERATOR_E + CABLE_E + FURNACE_E))
# One coal is 4,000 E and one iron ingot costs 200: 3,800 is left in the three
# buffers, wherever the split fell. Energy is moved, never made.
[ "$TOTAL" -eq 3800 ] || fail "the three buffers hold $TOTAL E ($GENERATOR_E + $CABLE_E + $FURNACE_E), expected 3,800 (4,000 - 200)"
echo "  one iron ingot: generator $GENERATOR_E + cable $CABLE_E + furnace $FURNACE_E = $TOTAL E"

echo "== G: differential — no cable, no energy arrives =="
place "$SX" 101 18 "$GENERATOR"
place "$SX" 101 19 "$FURNACE"
give_to "$SX" 101 19 minecraft:raw_iron
give_to "$SX" 101 18 minecraft:coal
ctl wait 300 >/dev/null
info "$SX" 101 18
GENERATOR_E=$ENERGY
[ "$GENERATOR_E" -gt 2000 ] || fail "the lone generator made $GENERATOR_E E in 300 ticks, expected more than 2,000"
info "$SX" 101 19
expect_energy 0
[ "$OUTPUT" = empty ] || fail "the furnace smelted $OUTPUT without a cable"
echo "  generator $GENERATOR_E E beside a furnace holding $ENERGY E: nothing crossed without the cable"

echo "PASS: a placed machine faces the placer, one coal is exactly 4,000 E over 400 burn steps, a non-fuel item makes nothing, an unpowered furnace idles, 1,000 E smelts one raw iron in 200 ticks for exactly 200 E, a cable carries 10 E a tick from a generator to a furnace and the three buffers balance at 3,800 E, and a generator beside a furnace with no cable delivers nothing"
