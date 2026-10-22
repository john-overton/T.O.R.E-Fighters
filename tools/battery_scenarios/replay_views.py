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
    jet = _events(work / "jet-log" / "log.jsonl", "aircraft.hud")
    if not any("target_x" in (row.get("fields") or {}) and row.get("object", 0) >= 0x50000000 for row in jet):
        out.append("the F-16C ground designation never reached the recorded HUD")
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
    player_sights = [row for row in sights if row.get("subject") == 0]
    ticks = [row["tick"] for row in player_sights]
    if len(ticks) < 24 or any(b - a != 5 for a, b in zip(ticks, ticks[1:])):
        out.append("the player sensor was not recorded at a steady 24 Hz")
    if not any("impact0_x" in (row.get("fields") or {}) for row in player_sights):
        out.append("the sensor never recorded its live gun impact")
    if not any((row.get("fields") or {}).get("radar_dots") for row in player_sights):
        out.append("the console never recorded its A-G radar contacts")
    ground_hud = _events(work / "ac130" / "log.jsonl", "aircraft.hud")
    if not any("target_x" in (row.get("fields") or {}) and row.get("object", 0) >= 0x50000000 for row in ground_hud):
        out.append("the ground designation never reached the recorded HUD")
    for name in ("hud", "sight"):
        stills = list((work / name).glob("still-*.png"))
        if len(stills) != 1 or stills[0].stat().st_size < 10_000:
            out.append(f"the {name} still was not written")
    if "Replay still:" not in output:
        out.append("the stills step did not report a frame")
    return out


WEAPON_PLAN = (
    "import sys; open(sys.argv[1], 'w').write(''.join("
    "f'{t} 0 0 0 0 0 0 100 60 -1 {v} 1 -1\\n' "
    "for t,v in [(3000,10),(5298,10),(5299,10),(5300,10),(3000,10),(3000,9),(5299,9)]))"
)


def weapon_problems(work: Path, output: str) -> list[str]:
    frames = digests(work / "weapon")
    if len(frames) != 7:
        return [f"weapon chase wrote {len(frames)} frames, want 7"]
    from battery_scenarios._replay_tools import ppm_problems
    problems = ppm_problems(str(work / "f12-bomb.ppm"), expect_ui=False)
    if frames[0] != frames[4]:
        problems.append("weapon chase differs after reverse seek")
    if frames[0] == frames[5]:
        problems.append("weapon chase and seeker nose pictures are identical")
    if frames[1] == frames[3]:
        problems.append("weapon chase did not change across impact")
    return problems


# Keep a real live rig and a real replay F12 selected across the bomb's end.
# The release probe runs requested post-impact frames before it finishes.
HOLD_TICKS = (2533, 2593, 2623, 2652, 2653, 2713, 2773, 2832, 2833)
HOLD_SCRIPT = r"""
import pathlib, sys
work = pathlib.Path(sys.argv[1])
ticks = (2533, 2593, 2623, 2652, 2653, 2713, 2773, 2832, 2833)
lines = ["wait 1"]
previous = ticks[0]
for tick in ticks:
    lines += ["key Right"] * (tick - previous)
    lines += ["wait 1", f"shot {work}/replay-{tick}.ppm"]
    previous = tick
lines += ["key Left"] * (2833 - 2713)
lines += ["wait 1", f"shot {work}/reverse.ppm", "key F10", "wait 1",
          f"shot {work}/interrupted.ppm", "exit"]
(work / "hold-input.txt").write_text("\n".join(lines) + "\n")
"""


def hold_problems(work: Path, output: str) -> list[str]:
    import re
    out = []
    poses = {int(tick): eye for tick, eye in re.findall(r"a2g-run: F12 tick (\d+) eye (\[[^]]+\])", output)}
    stopped = re.search(r"probe F12 stop: tick (\d+) eye (\[[^]]+\])", output)
    if not stopped or not 2533 < int(stopped[1]) < 2653:
        out.append("live F12 did not stop on the approach before impact")
    if any(t not in poses for t in HOLD_TICKS):
        out.append("live F12 did not capture every hold boundary")
    elif stopped:
        stopped_at, eye = int(stopped[1]), stopped[2]
        if any(poses[t] != eye for t in HOLD_TICKS if stopped_at <= t < 2833) or poses[2833] == eye:
            out.append("live F12 did not hold its stopped pose for exactly 180 game ticks after impact")
        if poses[2533] == eye:
            out.append("live F12 stopped before entering the approach")
    paths = [work / f"replay-{t}.ppm" for t in HOLD_TICKS] + [work / "reverse.ppm", work / "interrupted.ppm"]
    if any(not p.is_file() for p in paths):
        out.append("replay F12 hold captures are missing")
    else:
        shots = {Path(path).name: pose for path, pose in re.findall(r"replay F12 shot: (.+?) eye (.+)", output)}
        if "reverse.ppm" not in shots or shots.get("reverse.ppm") != shots.get("replay-2713.ppm"):
            out.append("replay F12 camera changed after reverse seek")
        # Interactive rendering can differ by a fraction of a
        # colour level after a seek. Camera poses above must still be exact.
        from battery_scenarios._replay_tools import read_ppm
        aw, ah, a = read_ppm(str(work / "reverse.ppm"))
        bw, bh, b = read_ppm(str(work / "replay-2713.ppm"))
        if (aw, ah) != (bw, bh) or sum(abs(x - y) for x, y in zip(a, b)) / max(1, len(a)) > 0.5:
            out.append("replay F12 picture changed after reverse seek")
        if (work / "interrupted.ppm").read_bytes() == (work / "reverse.ppm").read_bytes():
            out.append("F10 did not interrupt the replay hold")
        if (work / "replay-2832.ppm").read_bytes() == (work / "replay-2833.ppm").read_bytes():
            out.append("replay F12 did not return at the hold boundary")
    return out


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="flight-f12-hold", lane="replay",
            args=["--a2g-run", "UKR", "QUCOL", "--attack", "release", "--from", "1",
                  "--aircraft", "a4e", "--load", "MK82", "--count", "1", "--altitude", "3000",
                  "--speed", "450", "--seconds", "26", "--record", "{work}/hold.tore-replay",
                  "--render-scene", "{work}/live", "--scene-at",
                  ",".join(str((t - 962 - 0.01) / 120) for t in HOLD_TICKS)],
            env={"TORE_SCENE_F12": "1"}, timeout=600,
            then=[
                Step(["python3", "-c", HOLD_SCRIPT, "{work}"], app=False),
                Step(["--watch-replay", "{work}/hold.tore-replay", "--replay-tick", "2533",
                      "--flight-view", "11", "--replay-clean", "--window-size", "960x540",
                      "--input-script", "{work}/hold-input.txt", "--no-audio"], window=True, timeout=300),
            ],
            check_work=hold_problems,
            notes="Live and replay F12 stop on the 328-foot approach and hold a bomb impact for 180 game ticks, restore it on reverse seek, and allow view interruption.",
        ),
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
                    "--console-cycle", "55", "--console-fire", "60,68", "--orbit-slew", "70", "--seconds", "120",
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
            name="replay-weapon-chase", lane="replay",
            args=ARGS,
            timeout=600,
            then=[
                Step(["python3", "-c", WEAPON_PLAN, "{work}/weapon-plan.txt"], app=False, timeout=30),
                Step(["--replay-stills", "{work}/laser.tore-replay", "{work}/weapon-plan.txt", "{work}/weapon"], timeout=600),
                Step(["--watch-replay", "{work}/laser.tore-replay", "--capture-replay", "{work}/f12-bomb.ppm",
                      "--replay-tick", "3000", "--flight-view", "11", "--replay-clean", "--no-audio"], window=True, timeout=120),
            ],
            expect=[r"a2g-run: done"],
            check_work=weapon_problems,
            notes="Weapon chase follows a guided bomb to impact, restores it on reverse seek, and keeps the seeker nose view distinct.",
        ),
        Scenario(
            name="replay-taped-views", lane="replay",
            args=ARGS,
            timeout=600,
            then=[
                Step(["--recording-log", "{work}/laser.tore-replay", "--out", "{work}/jet-log"], timeout=180),
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
