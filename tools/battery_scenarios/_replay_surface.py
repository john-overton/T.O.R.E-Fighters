"""Lane: replay. A ground target mission recorded and replayed (slice RP1).

`replay-surface` runs `--surface-objective` on the full mission tick with
`--record` and `--verify-render`: the Ukraine armored column (`~QUCOL`, seed 1),
heavy defenses, a straight pass over the site by an invulnerable player, a Mk 82
on each flagged tank, and then eight more minutes so the supply trucks rearm the
launchers the pass emptied. The recording is a format 3 file, and the run reads
it back and compares every one of its 67,000 ticks with the picture the run drew
(poses of the tanks that drive, debris pieces, flak, craters and fires, the
aircraft, the rounds). The scenario then checks what the recording says in the
exports:

* the header holds the ground target, and the info lists its units by name;
* the column's poses change over the run, and its wrecks stay where they died;
* the launcher rails drop as SAMs fire and come back, on the tick the
  `surface.rearm` event says, after the truck's rearm time, and a gun's spare
  magazine arrives on the tick of its `surface.refill`;
* each destroyed tank has a `surface.wreck` event with its fire, and the
  summary names every unit ("SA-6 #9"), never a raw id;
* two identical runs make recordings that match.

Further steps record a vulnerable pass at `~QUCOL` (the player is shot down: the
loss says "it was shot down by an SA-6") and `~QTAAA` in Vietnam at 6,000 feet
(flak recorded with its own effect code), and open the format 2 recording the
test crate keeps (it still reads, with no surface content).
"""
from __future__ import annotations

import json
import re
from pathlib import Path

from battery import Scenario, Step
from battery_scenarios import _replay_checks as rc
from battery_scenarios._replay_record import sections

ROOT = Path(__file__).resolve().parents[2]
FORMAT_2 = ROOT / "crates" / "tore-replay" / "tests" / "golden" / "format2.tore-replay"

# The ids of surface objects start here.
SURFACE_IDS = 0x4000_0000

LONG = ["--surface-objective", "UKR", "QUCOL", "--surface-seed", "1", "--run-on", "--seconds", "560"]
SHOT_DOWN = ["--surface-objective", "UKR", "QUCOL", "--surface-seed", "1", "--vulnerable", "--seconds", "140"]
FLAK = ["--surface-objective", "TVIET", "QTAAA", "--surface-seed", "1", "--altitude", "6000", "--seconds", "140"]


def record(args: list[str], name: str) -> list[str]:
    return [*args, "--record", f"{{work}}/{name}.tore-replay", "--verify-render"]


def read_log(work: Path, folder: str) -> list[dict]:
    path = work / folder / "log.jsonl"
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


def distance(a: list[float], b: list[float]) -> float:
    return sum((x - y) ** 2 for x, y in zip(a, b)) ** 0.5


def verify_problems(run: str, label: str, least_ticks: int) -> list[str]:
    m = re.search(r"surface-objective: verify-render: PASS ticks=(\d+) missing=0 differing=0", run)
    if not m:
        line = re.search(r"surface-objective: verify-render:[^\n]*", run)
        return [f"{label}: the recording does not replay as the run drew it: {line.group(0) if line else 'no verdict'}"]
    if int(m.group(1)) < least_ticks:
        return [f"{label}: only {m.group(1)} ticks compared, want at least {least_ticks}"]
    return []


def long_run_problems(work: Path, s: dict[int, str]) -> list[str]:
    problems = verify_problems(s[0], "the column run", 60_000)
    info = s.get(1, "")
    if "format 3" not in info:
        problems.append("the recording is not a format 3 file")
    if not re.search(r"^Ground +target QUCOL with AAA 3 and SAM 3, surface seed 1", info, re.M):
        problems.append("recording-info does not name the ground target and its settings")
    named = re.search(r"^Surface +(\d+) units named, (\d+) of them enemy", info, re.M)
    if not named or int(named.group(1)) < 20:
        problems.append(f"recording-info names too few surface units: {named.group(0) if named else 'none'}")
    if "Problem " in info:
        problems.append("recording-info reports a problem")
    problems += rc.file_problems(work, "log/log.jsonl", rc.check_jsonl)
    problems += rc.file_problems(work, "log/summary.txt", lambda t: rc.check_summary(t, expect_aircraft=1))
    problems += rc.file_problems(work, "a.acmi", rc.check_acmi)
    lines = read_log(work, "log")
    header = next((d for d in lines if d["type"] == "header"), None)
    target = header and header["world"].get("ground_target")
    if not target or (target["stem"], target["seed"], target["aaa"], target["sam"]) != ("QUCOL", 1, 3, 3):
        problems.append(f"the header's ground target is {target}")
    if header and header.get("format") != 3:
        problems.append(f"the header says format {header.get('format')}")
    units = {d["id"]: d for d in lines if d["type"] == "surface_unit"}
    names = {d["name"] for d in units.values()}
    if not {"SA-6", "ZSU-23"} <= names:
        problems.append(f"the registry names {sorted(names)}, want the SA-6 and ZSU-23 of the column")
    if any(d["id"] < SURFACE_IDS for d in units.values()):
        problems.append("a surface unit has an aircraft id")

    # Moving units: a tank that drives is not where it began, and a wreck lies where it died.
    poses: dict[int, list[dict]] = {}
    for d in lines:
        if d["type"] == "surface":
            poses.setdefault(d["id"], []).append(d)
    driven = [uid for uid, rows in poses.items() if distance(rows[0]["pos_ft"], rows[-1]["pos_ft"]) > 500]
    if len(driven) < 5:
        problems.append(f"{len(driven)} of {len(poses)} recorded tracks moved by more than 500 ft, want the column")
    wrecked = [rows for rows in poses.values() if rows[-1]["wrecked"]]
    if len(wrecked) < 3:
        problems.append(f"{len(wrecked)} routed units are recorded as wrecks at the end, want the three tanks")
    for rows in wrecked:
        died = next(i for i, row in enumerate(rows) if row["wrecked"])
        if distance(rows[died]["pos_ft"], rows[-1]["pos_ft"]) > 1:
            problems.append("a wreck moved after it was destroyed")

    # Launchers: the rails drop as the SAMs fire, and a truck's rearm puts them back.
    stock = [d for d in lines if d["type"] == "surface_stock"]
    rearms = [d for d in lines if d["type"] == "event" and d["kind"] == "surface.rearm"]
    if not stock:
        problems.append("no launcher rail ever changed")
    if not rearms:
        problems.append("no supply truck rearmed a launcher in the eight minutes after the pass")
    for rearm in rearms:
        at = [d for d in stock if d["unit"] == rearm["subject"] and d["tick"] == rearm["tick"]]
        if not at:
            problems.append(f"the rearm of unit {rearm['subject']} at {rearm['t']} s has no rail change on its tick")
            continue
        if sum(d["loaded"] for d in at) != rearm["fields"]["loaded"]:
            problems.append(f"unit {rearm['subject']} was rearmed to {rearm['fields']['loaded']}, its rails hold {[d['loaded'] for d in at]}")
        for rail in at:
            earlier = [d["loaded"] for d in stock
                       if (d["unit"], d["mount"]) == (rail["unit"], rail["mount"]) and d["tick"] < rail["tick"]]
            if earlier and rail["loaded"] <= earlier[-1]:
                problems.append(f"unit {rearm['subject']} rail {rail['mount']} went from {earlier[-1]} to {rail['loaded']} at a rearm")
        if rearm["t"] < 240:
            problems.append(f"unit {rearm['subject']} was rearmed at {rearm['t']} s, before any truck could")
    refills = [d for d in lines if d["type"] == "event" and d["kind"] == "surface.refill"]
    if not refills:
        problems.append("no supply truck gave a gun a spare magazine")
    for refill in refills:
        at = [d for d in stock if d["unit"] == refill["subject"] and d["tick"] == refill["tick"]
              and d["mount"] == refill["fields"]["mount"]]
        if not at or at[0]["reserve"] != refill["fields"].get("reserve"):
            problems.append(f"the refill of unit {refill['subject']} at {refill['t']} s has no matching magazine record")
    fired = [d for d in lines if d["type"] == "event" and d["kind"] == "surface.burst"]
    if not fired or any(d["subject"] < SURFACE_IDS for d in fired):
        problems.append("the surface units' bursts are missing or owned by an aircraft")
    launches = [d for d in lines if d["type"] == "event" and d["kind"] == "weapon.launch"
                and d.get("subject", 0) >= SURFACE_IDS]
    if not launches:
        problems.append("no weapon.launch is owned by a surface unit")

    # Wrecks and their fire.
    wrecks = [d for d in lines if d["type"] == "event" and d["kind"] == "surface.wreck"]
    if len(wrecks) < 3:
        problems.append(f"{len(wrecks)} wrecks recorded, the three flagged targets fell")
    for w in wrecks:
        f = w["fields"]
        if f.get("burning") is not True or not f.get("fire_ft", 0) > 20:  # a tank burns
            problems.append(f"wreck {w['subject']} burns {f.get('burning')} with a fire of {f.get('fire_ft')} ft")
        if w["subject"] not in units:
            problems.append(f"wreck {w['subject']} is not in the registry")

    summary = (work / "log" / "summary.txt").read_text() if (work / "log" / "summary.txt").exists() else ""
    if "Surface units" not in summary or not re.search(r"SA-6 #\d+ \(SA-6; enemy; 100 hp\)", summary):
        problems.append("the summary has no section naming the SA-6s")
    if re.search(r"surface object 0x", summary):
        problems.append("the summary falls back to a raw surface id")
    if not re.search(r"rearmed \d+ times", summary):
        problems.append("the summary does not count the rearms")
    if not re.search(r"destroyed at \d+:\d\d\.\d", summary):
        problems.append("the summary does not say when the targets were destroyed")

    # The same run twice makes the same recording; a recording read against itself matches.
    if "Verdict: the recordings match" not in s.get(4, ""):
        problems.append("recording-diff of a recording against itself does not say they match")
    problems += verify_problems(s.get(5, ""), "the second column run", 60_000)
    if "Verdict: the recordings match" not in s.get(6, ""):
        problems.append("two identical runs made recordings that differ")
    return problems


def shot_down_problems(work: Path, s: dict[int, str]) -> list[str]:
    problems = verify_problems(s.get(7, ""), "the vulnerable column run", 10_000)
    lines = read_log(work, "log-down")
    lost = [d for d in lines if d["type"] == "event" and d["kind"] == "combat.destroyed" and d.get("subject") == 0]
    if not lost:
        problems.append("the vulnerable pass never lost the player")
    else:
        e = lost[0]
        if e.get("object", 0) < SURFACE_IDS:
            problems.append(f"the player's loss names killer {e.get('object')}, not a surface unit")
        if not re.fullmatch(r"it was shot down by an? \S+", e["fields"].get("reason", "")):
            problems.append(f"the loss gives the reason {e['fields'].get('reason')!r}")
    hits = [d for d in lines if d["type"] == "event" and d["kind"] == "combat.hit" and d.get("subject", 0) >= SURFACE_IDS]
    if not hits:
        problems.append("no hit on the player names a surface unit as the attacker")
    summary = (work / "log-down" / "summary.txt").read_text() if (work / "log-down" / "summary.txt").exists() else ""
    if not re.search(r"You was destroyed by (SA-6|SA-7|ZSU-\d+|SA-\d+) #\d+", summary):
        problems.append("the summary does not name the unit that destroyed the player")
    return problems


def flak_problems(s: dict[int, str]) -> list[str]:
    problems = verify_problems(s.get(8, ""), "the flak run", 10_000)
    m = re.search(r"^Effects +.*\bflak (\d+)", s.get(9, ""), re.M)
    if not m or int(m.group(1)) == 0:
        problems.append("no flak burst was recorded with its own effect code")
    return problems


def format_2_problems(s: dict[int, str]) -> list[str]:
    problems = []
    info = s.get(10, "")
    if "format 2" not in info or "State       finished normally" not in info:
        problems.append("the format 2 recording does not open as a finished format 2 file")
    if re.search(r"^Surface +|^Ground +", info, re.M) or "Problem " in info:
        problems.append("the format 2 recording shows surface content or a problem")
    if "T.O.R.E mission summary" not in s.get(11, "") and "log.jsonl" not in s.get(11, ""):
        problems.append("the format 2 recording does not export")
    return problems


def scenarios() -> list[Scenario]:
    def check(work: Path, output: str) -> list[str]:
        s = sections(output)
        problems = long_run_problems(work, s)
        problems += shot_down_problems(work, s)
        problems += flak_problems(s)
        problems += format_2_problems(s)
        return problems

    return [
        Scenario(
            name="replay-surface",
            lane="replay",
            args=record(LONG, "a"),
            timeout=600,
            expect=[r"surface-objective: done"],
            outputs=["a.tore-replay", "b.tore-replay", "c.tore-replay", "d.tore-replay"],
            then=[
                Step(["--recording-info", "{work}/a.tore-replay"]),  # 1
                Step(["--recording-log", "{work}/a.tore-replay", "--out", "{work}/log", "--rate", "5"]),  # 2
                Step(["--recording-acmi", "{work}/a.tore-replay", "--out", "{work}/a.acmi", "--guns"]),  # 3
                Step(["--recording-diff", "{work}/a.tore-replay", "{work}/a.tore-replay"]),  # 4
                Step(record(LONG, "b"), timeout=600),  # 5
                Step(["--recording-diff", "{work}/a.tore-replay", "{work}/b.tore-replay"]),  # 6
                Step(record(SHOT_DOWN, "c"), timeout=300),  # 7
                Step(record(FLAK, "d"), timeout=300),  # 8
                Step(["--recording-info", "{work}/d.tore-replay"]),  # 9
                Step(["--recording-info", str(FORMAT_2)]),  # 10
                Step(["--recording-log", str(FORMAT_2), "--out", "{work}/log-2"]),  # 11
                Step(["--recording-log", "{work}/c.tore-replay", "--out", "{work}/log-down"]),  # 12
            ],
            check_work=check,
            notes="a format 3 ground target recording replays tick for tick, names its units, and still opens format 2",
        )
    ]
