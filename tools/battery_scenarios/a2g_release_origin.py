"""Lane: flight. Where stores leave the aircraft (bug B3, docs/spec/stores-on-pylons.md),
through `--a2g-run`'s `release` attack and its `origin` lines (every round's start against
the aircraft, feet right, up and forward).

`a2g-release-origin` presses the trigger four times on each of (the A-4E's and F-4B's bomb
sight holds the fourth Mk 82 past the target, so three of those count):

* an A-4E's inner wing Mk 82 row (source x 8, 2.67 ft; no retail pair flag, paired as a
  wing row by agent decision);
* an A-4E's LAU-61 pods on the outer wing row (x 26, 8.67 ft): one rocket a press;
* an F-4B's fuselage Mk 82 row (x 13, 4.33 ft, retail flag 0x4);
* an F-16C's inner wing row (x 32, 10.67 ft) loaded with Mk 82s;
* an F-4B's centreline row (x 0) loaded with two Mk 82s.

A paired row's stores leave from its mirror first, then its own position, in turn
(left, right, left, right); the centreline row's from the centreline every time. Every
store starts at its row's height, and no store starts at the old single point.
"""
import re

from battery import Scenario, Step

ORIGIN = re.compile(
    r"^a2g-run: origin n=(\d+) id=\d+ right (\S+) up (\S+) forward (\S+) station-right (\S+) rounds-after (\d+)$",
    re.M,
)

# (label, station right ft, station up ft, presses, alternates)
CASES = [
    ("A-4E inner wing Mk 82", 8 / 3, -16 / 3, 4, True),
    ("A-4E LAU-61", 26 / 3, -16 / 3, 4, True),
    ("F-4B fuselage Mk 82", 13 / 3, -19 / 3, 4, True),
    ("F-16C inner wing Mk 82", 32 / 3, -10 / 3, 4, True),
    ("F-4B centreline Mk 82", 0.0, -19 / 3, 2, False),
]


def runs(output: str) -> list[str]:
    """The output of each `--a2g-run`, in order."""
    return [p for p in re.split(r"^a2g-run: done$", output, flags=re.M) if "a2g-run:" in p]


def origin_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != len(CASES):
        return [f"{len(parts)} runs finished, want {len(CASES)}"]
    problems: list[str] = []
    for part, (label, right, up, presses, alternates) in zip(parts, CASES):
        # The first round of each release (a rocket or a single bomb).
        firsts = [o for o in ORIGIN.findall(part) if o[0] == "0"]
        # The bomb sight lets the last press go past the target on some
        # carriers; three releases still show the turn of sides.
        if not min(3, presses) <= len(firsts) <= presses:
            problems.append(f"{label}: {len(firsts)} releases, want {presses}")
            continue
        sides = []
        for n, (_, r, u, _f, station, after) in enumerate(firsts):
            r, u, station = float(r), float(u), float(station)
            if abs(station - right) > 0.01:
                problems.append(f"{label}: station at {station:.2f} ft right, want {right:.2f}")
            if abs(abs(r) - right) > 0.05:
                problems.append(f"{label}: release {n} left from {r:.2f} ft right, want {right:.2f} either side")
            if abs(u - up) > 0.05:
                problems.append(f"{label}: release {n} left from {u:.2f} ft up, want {up:.2f}")
            sides.append("C" if right == 0 else ("L" if r < 0 else "R"))
        want = (["L", "R"] * presses if alternates else ["C"] * presses)[: len(sides)]
        if sides != want:
            problems.append(f"{label}: sides {''.join(sides)}, want {''.join(want)}")
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
            notes="Four presses on four paired rows and two on a centreline row: the sides alternate (B3).",
        ),
    ]
