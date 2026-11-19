#!/usr/bin/env python3
"""Parse one `/techreborn info` line into shell assignments.

The command answers one line:

    Tech Reborn <id> at [x, y, z]: energy=<e>/<cap> active=<bool> burn=<b>/<t> \
        cook=<c>/<ct> in=<item>x<count>|empty out=<item>x<count>|empty

and the e2e shells out to this to turn it into `STATION`-style variables, so an
assertion reads `[ "$ENERGY" = 4000 ]` rather than a regex per phase.

The retired 0.1.0 `.hnmod` line ended in `t=<game time>`; the SDK API has no server game-time
answer, so 0.2.0 prints no `t=` and TIME is always 0 (no assertion reads it).
"""

from __future__ import annotations

import re
import sys

FIELDS = (
    "energy",
    "capacity",
    "active",
    "burn",
    "burn_total",
    "cook",
    "cook_total",
    "input",
    "output",
    "time",
)

PATTERN = re.compile(
    r"Tech Reborn (?P<block>\S+) at \[(?P<x>-?\d+), (?P<y>-?\d+), (?P<z>-?\d+)\]: "
    r"energy=(?P<energy>\d+)/(?P<capacity>\d+) "
    r"active=(?P<active>\w+) "
    r"burn=(?P<burn>-?\d+)/(?P<burn_total>-?\d+) "
    r"cook=(?P<cook>-?\d+)/(?P<cook_total>-?\d+) "
    r"in=(?P<input>\S+) out=(?P<output>\S+)"
    r"(?: t=(?P<time>\d+))?$"
)


def main() -> int:
    if len(sys.argv) < 2:
        print("usage: parse_info.py '<info line>'", file=sys.stderr)
        return 2
    line = sys.argv[1].strip()
    match = PATTERN.match(line)
    if not match:
        print("ok=0")
        return 1
    print("ok=1")
    print(f"BLOCK={match.group('block')}")
    for field in FIELDS:
        value = match.group(field)
        print(f"{field.upper()}={sh_quote(value if value is not None else '0')}")
    return 0


def sh_quote(value: str) -> str:
    return "'" + value.replace("'", "'\\''") + "'"


if __name__ == "__main__":
    raise SystemExit(main())
