"""Lane: replay. The replay's HUD camera and Sensor View (nr-replayviews,
docs/REPLAYS.md, "HUD camera" and "Sensor View").

`replay-hud-state` (nr-replayhud) records a short scripted air-to-air attack
and an AC-130 firing pass, checks the new `aircraft.hud` and `aircraft.sight`
events, and draws the HUD camera and the Sensor View offscreen at a tick
those events name.

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
import json
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


def _events(path: Path, kind: str) -> list[dict]:
    found = []
    if not path.is_file():
        return found
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        if row.get("type") == "event" and row.get("kind") == kind:
            found.append(row)
    return found


# Standalone: the battery runs these with python3 -c, which does not share this process.
PLAN_HUD = r"""
import json, sys
log, plan = sys.argv[1], sys.argv[2]
chosen = None
for line in open(log, encoding="utf-8"):
    row = json.loads(line)
    if row.get("kind") != "aircraft.hud" or row.get("subject") != 0:
        continue
    fields = row.get("fields") or {}
    if fields.get("nav") is False and fields.get("armed") and row.get("object") is not None:
        chosen = row
        if fields.get("seeker") == "LOCK" and str(fields.get("weapon", "")).startswith("AIM"):
            break
if chosen is None:
    raise SystemExit("the player has no armed aircraft.hud sample with a target")
tick = chosen["tick"]
open(plan, "w", encoding="utf-8").write(f"{tick} 0 0 0 0 0 0 100 60 -1 4 1 -1\n")
print("replay hud plan: tick", tick, chosen["fields"].get("weapon"), chosen["fields"].get("seeker"))
"""
PLAN_SIGHT = r"""
import json, sys
log, plan = sys.argv[1], sys.argv[2]
tick = None
for line in open(log, encoding="utf-8"):
    row = json.loads(line)
    if row.get("kind") != "aircraft.sight":
        continue
    fields = row.get("fields") or {}
    rounds = [fields.get("r0"), fields.get("r1"), fields.get("r2")]
    if fields.get("gunship") and any(isinstance(n, int) and n > 0 for n in rounds):
        tick = row["tick"]
if tick is None:
    raise SystemExit("no aircraft.sight sample with gun rounds")
open(plan, "w", encoding="utf-8").write(f"{tick} 0 0 0 0 0 0 100 60 -1 5 1 -1\n")
print("replay sight plan: tick", tick)
"""


def hud_state_problems(work: Path, output: str) -> list[str]:
    out: list[str] = []
    hud = _events(work / "a2a" / "log.jsonl", "aircraft.hud")
    if not any(
        (row.get("fields") or {}).get("nav") is False
        and (row.get("fields") or {}).get("armed")
        and row.get("object") is not None
        for row in hud
    ):
        out.append("the air-to-air recording has no armed HUD sample on a target")
    if not any((row.get("fields") or {}).get("seeker") == "LOCK" for row in hud):
        out.append("the air-to-air recording never recorded a LOCK seeker")
    sights = _events(work / "ac130" / "log.jsonl", "aircraft.sight")
    if not any(
        (row.get("fields") or {}).get("gunship")
        and any(
            isinstance((row.get("fields") or {}).get(key), int) and (row.get("fields") or {})[key] > 0
            for key in ("r0", "r1", "r2")
        )
        for row in sights
    ):
        out.append("the AC-130 recording has no sight sample with rounds left")
    for name in ("hud", "sight"):
        stills = list((work / name).glob("still-*.png"))
        if len(stills) != 1 or stills[0].stat().st_size < 10_000:
            out.append(f"the {name} still was not written")
    if "Replay still:" not in output:
        out.append("the stills step did not report a frame")
    return out


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="replay-hud-state", lane="replay",
            args=[
                "--ai-probe-ticks", "4800", "--separation", "2", "--probe-attack", "400:8",
                "--record-mission", "{work}/a2a.tore-replay", "--no-audio",
            ],
            timeout=400,
            then=[
                Step(["--recording-log", "{work}/a2a.tore-replay", "--out", "{work}/a2a"], timeout=180),
                Step(["python3", "-c", PLAN_HUD, "{work}/a2a/log.jsonl", "{work}/hud-plan.txt"], app=False, timeout=30),
                Step(["--replay-stills", "{work}/a2a.tore-replay", "{work}/hud-plan.txt", "{work}/hud"], timeout=400),
                Step([
                    "--a2g-run", "UKR", "QUCOL", "--aircraft", "ac130", "--load", "C_25.JT",
                    "--attack", "orbit", "--altitude", "4000", "--speed", "200", "--from", "3",
                    "--console-fire", "55,75", "--seconds", "90",
                    "--record", "{work}/ac130.tore-replay",
                ], timeout=500),
                Step(["--recording-log", "{work}/ac130.tore-replay", "--out", "{work}/ac130"], timeout=180),
                Step(["python3", "-c", PLAN_SIGHT, "{work}/ac130/log.jsonl", "{work}/sight-plan.txt"], app=False, timeout=30),
                Step(["--replay-stills", "{work}/ac130.tore-replay", "{work}/sight-plan.txt", "{work}/sight"], timeout=400),
            ],
            expect=[r"orbit probe: PASS"],
            check_work=hud_state_problems,
            notes="nr-replayhud: a scripted air-to-air lock and an AC-130 firing pass record the HUD "
                  "and the sight, and the HUD camera and Sensor View draw from those ticks.",
        ),
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
