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
    ]
