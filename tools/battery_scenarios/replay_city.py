"""Lane: replay. A recorded city strike (nr-fx3, docs/REPLAYS.md, "Versions
and damage").

`replay-city-strike` runs `--surface-objective` on Ukraine's `~QUCOL` with
`--city-strike 12`: twelve Mk 84s, four a pass, on the city fill buildings
nearest the target, with `--record` and `--verify-render`. The run prints how
many fill buildings fell; the recording's info must say the same number were
knocked down (its city section, by where each building stands), in one change
a pass. A second step replays the recording's stills offscreen at three ticks
(before the strike, after the first pass, at the end) to prove the viewer
opens it and sets the city from it.

`replay-city-smoke-seek` (nr-fx4) records the same strike and replays stills of
the burning city three ways, each ending at the same tick: played through
(frames every few hundred ticks), opened cold on the last tick, and reached by
jumps forward and back. A burning block's smoke column depends on the playhead
and the tick the building fell on alone, so the three must print the same
`Replay city smoke` line (the same puff digest) at that tick, and the columns
must have grown during the play-through.
"""
import re

from battery import Scenario, Step

FLATTENED = re.compile(r"^surface-objective: city flattened (\d+) buildings$", re.M)
RECORDED = re.compile(r"^City +(\d+) buildings knocked down in (\d+) changes, first at tick (\d+)$", re.M)
VERIFY = re.compile(r"^surface-objective: verify-render: PASS", re.M)
# A director plan: night (23:00), from 2,500 ft south of the strike, before
# it, after the first pass and at the end (anchor 4294967294 is the world).
PLAN = (
    "import sys; open(sys.argv[1], 'w').write(''.join("
    "f'{t} 4294967294 1090500 2500 830000 1090500 0 839000 50 1380 0 1 -1\\n' for t in (2300, 2500, 5000)))"
)
STILL = re.compile(r"^Replay still: .*still-\d+-t\d+\.png$", re.M)

SMOKE = re.compile(
    r"^Replay city smoke: tick (\d+) burning (\d+) fires (\d+) puffs (\d+) tallest ([\d.]+) ft digest ([0-9a-f]+)$", re.M)
# Three plans (day, a fixed eye 1,500 ft off the strike) ending at tick 5000.
SEEK_PLANS = (
    "import sys\n"
    "def plan(name, ticks):\n"
    "    open(sys.argv[1] + '/' + name, 'w').write(''.join("
    "f'{t} 4294967294 1090400 450 837000 1090400 250 838500 60 720 0 1 -1\\n' for t in ticks))\n"
    "plan('through.txt', [2300, 2500, 2900, 3300, 3700, 4100, 4500, 5000])\n"
    "plan('cold.txt', [5000])\n"
    "plan('shuffle.txt', [5000, 1000, 2450, 5000, 2000, 5000])\n"
)

ARGS = [
    "--surface-objective", "UKR", "QUCOL", "--surface-seed", "1", "--from", "2",
    "--run-on", "--seconds", "40", "--city-strike", "12",
    "--record", "{work}/city.tore-replay", "--verify-render",
]


def problems(output: str) -> list[str]:
    out: list[str] = []
    flat, recorded = FLATTENED.search(output), RECORDED.search(output)
    if not flat:
        return ["no 'city flattened' line"]
    if not recorded:
        return ["the recording's info has no City line: the knocked-down buildings were not recorded"]
    fell, kept, changes = int(flat.group(1)), int(recorded.group(1)), int(recorded.group(2))
    if fell < 12:
        out.append(f"only {fell} buildings fell to twelve Mk 84s")
    if kept != fell:
        out.append(f"the recording keeps {kept} knocked-down buildings, the run flattened {fell}")
    if changes != 3:
        out.append(f"{changes} recorded city changes, want 3 (one a pass of four bombs)")
    if not VERIFY.search(output):
        out.append("verify-render did not pass")
    if len(STILL.findall(output)) != 3:
        out.append("the replay stills were not all written")
    return out


def smoke_problems(output: str) -> list[str]:
    out: list[str] = []
    lines = [(int(m[0]), int(m[1]), int(m[3]), float(m[4]), m[5]) for m in SMOKE.findall(output)]
    # through: 7 burning frames (2500..5000), cold: 1, shuffle: 4 (5000, 2450, 5000, 5000).
    if len(lines) != 12:
        return [f"{len(lines)} 'Replay city smoke' lines, want 12"]
    through, cold, shuffle = lines[:7], lines[7:8], lines[8:]
    finals = {through[-1][4], cold[0][4], shuffle[0][4], shuffle[2][4], shuffle[3][4]}
    if len(finals) != 1:
        out.append(f"the smoke at tick 5000 differs by how the playhead got there: {sorted(finals)}")
    if any(t[0] != 5000 for t in (through[-1], cold[0], shuffle[0], shuffle[2], shuffle[3])):
        out.append("a final frame is not at tick 5000")
    heights = [t[3] for t in through]
    if heights != sorted(heights) or heights[-1] < 3 * heights[0]:
        out.append(f"the columns did not grow through the play-through: {heights}")
    if through[-1][2] < 400:
        out.append(f"only {through[-1][2]} puffs at the end: the columns did not stand")
    # The jump back to tick 2450 shows only the first pass's buildings and its young columns.
    back = shuffle[1]
    if not (back[1] < through[-1][1] and back[3] < through[-1][3] / 4):
        out.append(f"the jump back to tick 2450 shows burning {back[1]}, puffs {back[2]}, tallest {back[3]}")
    return out


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="replay-city-strike", lane="replay",
            args=ARGS,
            timeout=600,
            then=[
                Step(["--recording-info", "{work}/city.tore-replay"], timeout=120),
                Step(["python3", "-c", PLAN, "{work}/plan.txt"], app=False, timeout=30),
                Step(["--replay-stills", "{work}/city.tore-replay", "{work}/plan.txt", "{work}/stills"], timeout=600),
            ],
            expect=[r"^surface-objective: city flattened \d+ buildings$"],
            check=problems,
            notes="nr-fx3: a city strike's knocked-down fill buildings are recorded (city section, by place) "
                  "and the replay viewer opens the recording and draws its stills offscreen.",
        ),
        Scenario(
            name="replay-city-smoke-seek", lane="replay",
            args=ARGS,
            timeout=900,
            then=[
                Step(["python3", "-c", SEEK_PLANS, "{work}"], app=False, timeout=30),
                Step(["--replay-stills", "{work}/city.tore-replay", "{work}/through.txt", "{work}/through"], timeout=600),
                Step(["--replay-stills", "{work}/city.tore-replay", "{work}/cold.txt", "{work}/cold"], timeout=600),
                Step(["--replay-stills", "{work}/city.tore-replay", "{work}/shuffle.txt", "{work}/shuffle"], timeout=600),
            ],
            expect=[r"^surface-objective: city flattened \d+ buildings$"],
            check=smoke_problems,
            notes="nr-fx4: after a replay seek, a burning city block's smoke column stands at the height playing "
                  "through would have given it: the same puff digest at the same tick however the playhead got there.",
        ),
    ]
