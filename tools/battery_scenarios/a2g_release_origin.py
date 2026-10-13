"""Lane: flight. Where stores leave the aircraft (bug B3, slices BP2 and PY2,
docs/spec/stores-on-pylons.md), through `--a2g-run`'s `release` attack and its `origin` lines
(every round's start against the aircraft, feet right, up and forward, the point the drawing
hangs the store at, and how deep its pylon is).

`a2g-release-origin` presses the trigger four times on each of (the A-4E's and F-4B's bomb
sight holds the fourth Mk 82 past the target, so three of those count):

* an A-4E's Mk 82 row (source x 8, 2.67 ft, no retail pair flag): a centreline rack of two
  pairs one behind another (PY2), the front pair first, right then left;
* an A-4E's LAU-61 pods on the outer wing row (x 26, 8.67 ft): one rocket a press;
* an F-4B's fuselage Mk 82 row (x 13, 4.33 ft, retail flag 0x4): one behind another on each
  side (PY2), the front pair first;
* an F-16C's inner wing row (x 32, 10.67 ft) loaded with Mk 82s: two pylons a wing (BP2),
  1.4 ft either side of the row's position, the outboard pair first;
* an F-4B's centreline row (x 0) loaded with two Mk 82s, side by side, right then left.

Every store leaves from the point it hangs at (within 0.05 ft across and up, half a foot
along for the tick flown). A wing pylon is 2 ft deep; a fuselage or centreline launcher rail
4 inches (John 2026-10-13), with a rack's 0.3 ft beam between rail and stores. Paired
rows alternate left and right.
"""
import re

from battery import Scenario, Step

ORIGIN = re.compile(
    r"^a2g-run: origin n=(\d+) id=\d+ right (\S+) up (\S+) forward (\S+) station-right (\S+) "
    r"rounds-after (\d+) hung (\S+) (\S+) (\S+) depth (\S+)$",
    re.M,
)

# How far either side of a wing row's position its two pylons stand, feet
# (tore_sim::combat::hardpoints::DUAL_PYLON_SPREAD).
DUAL = 1.4
# A fuselage or centreline launcher rail's depth (4 inches) and a rack beam's, feet
# (tore_sim::combat::hardpoints::RAIL_DEPTH, RACK_DEPTH).
RAIL = 4 / 12
RACK = 0.3
# A Mk 82's shoulder on a rack: 0.55 of its 1.27 ft fin span.
SHOULDER = 0.55 * 1.27

# (label, station right ft, presses, the releases' sides, how far out each leaves,
# whether each leaves ahead of the row (+1), behind (-1) or either (0), pylon depth ft)
CASES = [
    ("A-4E Mk 82 centreline rack", 0.0, 4, "RLRL", [SHOULDER] * 4, [1, 1, -1, -1], RAIL + RACK),
    ("A-4E LAU-61", 26 / 3, 4, "LRLR", [26 / 3] * 4, [0] * 4, 2.0),
    ("F-4B fuselage Mk 82", 13 / 3, 4, "LRLR", [13 / 3] * 4, [1, 1, -1, -1], RAIL),
    ("F-16C inner wing Mk 82", 32 / 3, 4, "LRLR",
     [32 / 3 + DUAL] * 2 + [32 / 3 - DUAL] * 2, [0] * 4, 2.0),
    ("F-4B centreline Mk 82", 0.0, 2, "RL", [SHOULDER] * 2, [0] * 2, RAIL + RACK),
]


def runs(output: str) -> list[str]:
    """The output of each `--a2g-run`, in order."""
    return [p for p in re.split(r"^a2g-run: done$", output, flags=re.M) if "a2g-run:" in p]


def origin_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != len(CASES):
        return [f"{len(parts)} runs finished, want {len(CASES)}"]
    problems: list[str] = []
    for part, (label, right, presses, sides_want, outs, alongs, depth_want) in zip(parts, CASES):
        # The first round of each release (a rocket or a single bomb).
        firsts = [o for o in ORIGIN.findall(part) if o[0] == "0"]
        # The bomb sight lets the last press go past the target on some
        # carriers; three releases still show the turn of sides.
        if not min(3, presses) <= len(firsts) <= presses:
            problems.append(f"{label}: {len(firsts)} releases, want {presses}")
            continue
        sides = []
        for n, (_, r, u, f, station, _after, hr, hu, hf, depth) in enumerate(firsts):
            r, u, f, station = float(r), float(u), float(f), float(station)
            hr, hu, hf = float(hr), float(hu), float(hf)
            if abs(station - right) > 0.01:
                problems.append(f"{label}: station at {station:.2f} ft right, want {right:.2f}")
            if abs(r - hr) > 0.05 or abs(u - hu) > 0.05 or abs(f - hf) > 0.5:
                problems.append(
                    f"{label}: release {n} left from {r:.2f} {u:.2f} {f:.2f}, "
                    f"hung at {hr:.2f} {hu:.2f} {hf:.2f}"
                )
            if abs(abs(r) - outs[n]) > 0.05:
                problems.append(f"{label}: release {n} left from {r:.2f} ft right, want {outs[n]:.2f} either side")
            if depth == "none" or abs(float(depth) - depth_want) > 0.01:
                problems.append(f"{label}: release {n} pylon depth {depth}, want {depth_want:.2f}")
            sides.append("L" if r < 0 else "R")
        # One behind another: the front stores leave first, a store length ahead.
        ahead = [float(o[8]) for o, a in zip(firsts, alongs) if a > 0]
        behind = [float(o[8]) for o, a in zip(firsts, alongs) if a < 0]
        if ahead and behind and min(ahead) - max(behind) < 5:
            problems.append(f"{label}: front stores at {ahead} ft forward, rear at {behind}: want 5 ft or more apart")
        if "".join(sides) != sides_want[: len(sides)]:
            problems.append(f"{label}: sides {''.join(sides)}, want {sides_want[: len(sides)]}")
    return problems


def scenarios() -> list[Scenario]:
    run = ["--a2g-run", "UKR", "QUCOL", "--attack", "release", "--from", "1", "--altitude", "3000",
           "--speed", "420", "--repeat", "4", "--gap", "0.5"]
    return [
        Scenario(
            name="a2g-release-origin", lane="flight",
            args=run + ["--aircraft", "a4e", "--load", "MK82", "--count", "4"],
            timeout=300,
            then=[
                Step(run + ["--aircraft", "a4e", "--load", "LAU61"], timeout=300),
                Step(run + ["--aircraft", "f4b", "--load", "MK82", "--count", "4"], timeout=300),
                Step(run + ["--aircraft", "f16c", "--station", "1", "--load", "MK82", "--count", "4"],
                     timeout=300),
                Step(["--a2g-run", "UKR", "QUCOL", "--attack", "release", "--from", "1", "--altitude",
                      "3000", "--speed", "420", "--repeat", "2", "--gap", "0.5", "--aircraft", "f4b",
                      "--station", "0", "--load", "MK82", "--count", "2"], timeout=300),
            ],
            check=origin_problems,
            notes="Four presses on four rows and two on a centreline row: each store leaves from where it hangs, under a 2 ft wing pylon or a 4 inch body rail (B3, BP2, PY2).",
        ),
    ]
