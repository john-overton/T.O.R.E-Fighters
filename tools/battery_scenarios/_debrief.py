"""Checks on the AI probe's debrief line that hold in every scenario.

The line (`crates/tore-world/src/debrief.rs`, `Report::summary`) lists, for the
player and the wingman, the kills by row, friendly fire, and the hits and
launches of each weapon class. The rule in docs/spec/debrief.md ("Kills"):
every kill, direct or credited to the last attacker, comes from a hit the
ledger tallied, so a pilot's kills never exceed its recorded hits.
"""
from __future__ import annotations

import re

# One column of the line: `player[Dead damage=100% kills=[1, 0, ...] ff=0
# a2a=1/2 dmg=140 gun=0/0 a2g=0/0 bomb=0/0 ...]`.
COLUMN = re.compile(
    r"(player|wingman)\[\w+ damage=\d+% kills=\[([\d, ]*)\] ff=(\d+) a2a=(\d+)/\d+ dmg=\d+ gun=(\d+)/\d+"
    r"(?: a2g=(\d+)/\d+ bomb=(\d+)/\d+)?"
)


def kill_hit_problems(output: str) -> list[str]:
    """Each debrief column that credits more kills than it has recorded hits."""
    problems = []
    for line in re.findall(r"^AI probe debrief: .*$", output, re.M):
        for who, kills, friendly_fire, *hits in COLUMN.findall(line):
            credited = sum(int(k) for k in kills.split(",") if k.strip()) + int(friendly_fire)
            recorded = sum(int(h) for h in hits if h)
            if credited > recorded:
                problems.append(f"the debrief credits the {who} with {credited} kills but only {recorded} recorded hits")
    return problems
