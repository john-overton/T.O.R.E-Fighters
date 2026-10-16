"""Lane: replay. The replay's HUD camera and Sensor View (nr-replayviews,
docs/REPLAYS.md, "HUD camera" and "Sensor View").

`replay-taped-views` records the F-16C's laser pass (`--a2g-run` on Ukraine's
`~QUCOL`: its Pave Spike lases a T-80 and a GBU-10 rides the spot, the
`replay-a2g` laser pass) and draws the director's views 4 to 8 offscreen at
one tick while the bomb falls: the HUD camera, then the Sensor View in white
hot, black hot, TV and night vision. It draws the same plan twice: every still
must be written and the two draws must match byte for byte (the tape's noise,
tracking band and dropouts come from the replay tick, never the clock), and
the HUD camera and the white hot Sensor View must differ (each view draws its
own picture).
"""
import hashlib
from pathlib import Path

from battery import Scenario, Step

ARGS = [
    "--a2g-run", "UKR", "QUCOL", "--aircraft", "f16c", "--load", "GBU10", "--station", "2", "--count", "2",
    "--attack", "laser", "--altitude", "15000", "--speed", "480", "--from", "6", "--seconds", "60",
    "--record", "{work}/laser.tore-replay",
]
# Tick 3000: the laser is on and the GBU-10 is in the air (released at 1824).
PLAN = (
    "import sys; open(sys.argv[1], 'w').write(''.join("
    "f'3000 0 0 0 0 0 0 100 60 -1 {v} 1 -1\\n' for v in (4, 5, 6, 7, 8)))"
)
VIEWS = ("hud", "whot", "bhot", "tv", "nvg")


def digests(folder: Path) -> list[str]:
    return [hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(folder.glob("still-*.png"))]


def problems(work: Path, output: str) -> list[str]:
    out: list[str] = []
    first, second = digests(work / "a"), digests(work / "b")
    if len(first) != len(VIEWS) or len(second) != len(VIEWS):
        return [f"stills written: {len(first)} and {len(second)}, want {len(VIEWS)} each"]
    for name, x, y in zip(VIEWS, first, second):
        if x != y:
            out.append(f"the {name} still differs between two draws of the same tick")
    if len(set(first)) != len(first):
        out.append("two views drew the same picture")
    return out


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="replay-taped-views", lane="replay",
            args=ARGS,
            timeout=600,
            then=[
                Step(["python3", "-c", PLAN, "{work}/plan.txt"], app=False, timeout=30),
                Step(["--replay-stills", "{work}/laser.tore-replay", "{work}/plan.txt", "{work}/a"], timeout=600),
                Step(["--replay-stills", "{work}/laser.tore-replay", "{work}/plan.txt", "{work}/b"], timeout=600),
            ],
            expect=[r"a2g-run: done"],
            check_work=problems,
            notes="nr-replayviews: the HUD camera and the four Sensor View pictures draw offscreen from a "
                  "recording, the same at a tick every time.",
        ),
    ]
