"""Lane: replay. The air-to-ground round recorded and replayed (slice RA).

`replay-a2g` flies ten short passes through `--a2g-run --record --verify-render`
(the real trigger, the whole mission tick), one for each thing the recording has
to keep, and checks two things for each: the recording replays tick for tick as
the run drew it (every bomb's fall included), and the events and words say what
happened.

* a Mk 82 AIR 7-pack: one `weapon.release` listing seven rounds in order, the
  CCIP sight, seven bombs falling to the ground in the Tacview file;
* a Mk 82 from 300 ft in a dive, which lands before its fuze arms: a dud, with
  its words and no damage;
* an LAU-61 held for a second: a salvo of rockets of the rocket class, counted
  and with the pod running down;
* a CBU-87: a cluster burst;
* a GBU-10 on the aircraft's own laser, which kills a tank: the laser comes
  on, the bomb is released guiding, and the outcome and the loss name the laser
  they rode;
* the same bomb with the laser switched off mid-fall: it goes blind, and the
  words say whose laser went off;
* a GBU-10 dropped by an F/A-18D on an Su-25's laser spot (buddy lasing): the
  spot owner is not the shooter;
* a GBU-30 JDAM: the stored point;
* two AGM-88 HARMs at the Straight Flush of an SA-6 site, one in BORE and one
  cued from the HARM page: the launch modes, the designations, the loss.

Then one run is repeated and compared with the first (two identical runs make
identical recordings), and the format 2 recording the test crate keeps still
opens. The buddy pass is recorded without `--verify-render`: the driver's
two-seat world draws the picture from the second seat's view, which the
recording (made for the first seat) cannot redraw, and the replay of that file
is not part of the check.
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
STRAIGHT_FLUSH = "0x50000009"

RUN = ["--a2g-run"]
COLUMN = ["UKR", "QUCOL"]
TRUCK = ["TVIET", "QTTRUCK", "--target", "2"]
SITE = ["VLA", "QVRDRI", "--surface-seed", "4", "--defenses", "3", "3", "--unit", STRAIGHT_FLUSH]
GBU10 = ["--load", "GBU10", "--station", "2", "--count", "2", "--attack", "laser"]
LASER_PASS = ["--altitude", "15000", "--speed", "480", "--from", "6", "--seconds", "120"]
HARM = ["--aircraft", "f4g", "--load", "AGM88", "--attack", "harm", "--from", "12", "--altitude", "15000",
        "--speed", "450", "--seconds", "60"]

# The recordings, in order: name, arguments of the pass, whether the run replays
# its own recording, seconds each run may take.
PASSES: list[tuple[str, list[str], bool]] = [
    ("pack", COLUMN + ["--attack", "release", "--from", "1", "--aircraft", "a4e", "--load", "MK82AIR",
                       "--count", "1", "--altitude", "3000", "--speed", "450"], True),
    ("dud", TRUCK + ["--aircraft", "a4e", "--load", "MK82", "--attack", "ccip", "--dive", "30",
                     "--altitude", "300", "--speed", "450", "--from", "1"], True),
    ("rocket", TRUCK + ["--aircraft", "a4e", "--load", "LAU61", "--attack", "rocket", "--altitude", "1500",
                        "--dive", "10", "--speed", "400", "--from", "1", "--hold", "1"], True),
    ("cluster", COLUMN + ["--attack", "release", "--from", "1", "--aircraft", "a4e", "--load", "CBU87",
                          "--count", "2", "--altitude", "3000", "--speed", "450"], True),
    ("laser", COLUMN + ["--aircraft", "f16c"] + GBU10 + LASER_PASS, True),
    ("blind", COLUMN + ["--aircraft", "f16c"] + GBU10 + LASER_PASS + ["--laser-off", "20"], True),
    ("buddy", COLUMN + ["--aircraft", "f18"] + GBU10 + ["--altitude", "12000", "--speed", "450", "--from", "6",
                                                        "--seconds", "120", "--buddy", "su25"], False),
    ("jdam", ["TVIET", "QTTRUCK", "--attack", "jdam", "--aircraft", "f16c", "--load", "GBU30", "--station", "2",
              "--count", "2", "--altitude", "12000", "--speed", "450", "--from", "6", "--seconds", "120",
              "--condition", "cloudy"], True),
    ("harm-bore", SITE + HARM + ["--skill", "3"], True),
    ("harm-cued", SITE + HARM + ["--designate", "harm"], True),
]

VERIFY = re.compile(r"^a2g-run: verify-render: PASS ticks=(\d+) missing=0 differing=0", re.M)


def run_args(index: int, name: str = "") -> list[str]:
    """The arguments of pass `index`, recording to `{work}/NAME.tore-replay`."""
    base, args, verify = PASSES[index]
    out = [*RUN, *args, "--record", f"{{work}}/{name or base}.tore-replay"]
    return out + (["--verify-render"] if verify else [])


def events(work: Path, name: str) -> list[dict]:
    path = work / f"log-{name}" / "log.jsonl"
    if not path.exists():
        return []
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    return [d for d in rows if d["type"] == "event"]


def kind(rows: list[dict], wanted: str, **match) -> list[dict]:
    found = [d for d in rows if d["kind"] == wanted]
    for key, value in match.items():
        found = [d for d in found if d["fields"].get(key) == value]
    return found


def weapons(work: Path, name: str) -> dict[str, dict]:
    path = work / f"log-{name}" / "log.jsonl"
    rows = [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []
    return {d["name"]: d for d in rows if d["type"] == "weapon"}


def acmi_falls(text: str) -> list[tuple[str, float, float]]:
    """(id, first, last) altitude in metres of every bomb object in a Tacview file."""
    bombs: set[str] = set()
    heights: dict[str, list[float]] = {}
    for line in text.splitlines():
        m = rc.OBJ.match(line)
        if not m or m.group(1) or not m.group(3):
            continue
        oid, rest = m.group(2), m.group(3)
        if "Weapon+Bomb" in rest:
            bombs.add(oid)
        t = re.search(r"(?:^|,)T=([^,]*)", rest)
        if t:
            alt = t.group(1).split("|")[2:3]
            if alt and alt[0]:
                heights.setdefault(oid, []).append(float(alt[0]))
    return [(oid, h[0], h[-1]) for oid, h in heights.items() if oid in bombs and len(h) > 1]


def verify_problems(output: str, label: str, least_ticks: int) -> list[str]:
    m = VERIFY.search(output)
    if not m:
        line = re.search(r"a2g-run: verify-render:[^\n]*", output)
        return [f"{label}: the recording does not replay as the run drew it: {line.group(0) if line else 'no verdict'}"]
    if int(m.group(1)) < least_ticks:
        return [f"{label}: only {m.group(1)} ticks compared, want at least {least_ticks}"]
    return []


def pack_problems(work: Path, out: str, acmi: str) -> list[str]:
    rows = events(work, "pack")
    problems = verify_problems(out, "pack", 2_000)
    releases = kind(rows, "weapon.release")
    if len(releases) != 1:
        return problems + [f"pack: {len(releases)} releases, want one for the whole pack"]
    f = releases[0]["fields"]
    ids = f.get("projectiles", [])
    if f.get("count") != 7 or len(ids) != 7 or ids != sorted(ids) or f.get("projectile") != ids[0]:
        problems.append(f"pack: the release lists {f.get('count')} rounds {ids}, want 7 in order")
    if f.get("class") != "bomb" or f.get("delivery") != "ccip" or f.get("left") != 0:
        problems.append(f"pack: release {f}")
    launches = kind(rows, "weapon.launch", **{"class": "bomb"})
    if sorted(d["fields"]["projectile"] for d in launches) != sorted(ids):
        problems.append("pack: the launches are not the release's seven rounds")
    if any(d["fields"].get("surface") is not True for d in launches):
        problems.append("pack: a bomb launch does not say it aims at the surface")
    if len(kind(rows, "weapon.outcome")) != 7:
        problems.append("pack: not every bomb has an outcome")
    if kind(rows, "weapon.dud"):
        problems.append("pack: a bomb of the pack was a dud from 3,000 ft")
    falls = acmi_falls(acmi)
    if len(falls) != 7:
        problems.append(f"pack: {len(falls)} bombs fall in the Tacview file, want 7")
    for oid, first, last in falls:
        if first - last < 300:
            problems.append(f"pack: bomb {oid} fell {first - last:.0f} m in the Tacview file, want about 900")
    summary = (work / "log-pack" / "summary.txt")
    text = summary.read_text() if summary.exists() else ""
    if not re.search(r"You released 7 x \S+ from station \d+.*\(ccip\), 0 left", text):
        problems.append("pack: the summary does not say the pack was released")
    return problems


def dud_problems(work: Path, out: str) -> list[str]:
    rows = events(work, "dud")
    problems = verify_problems(out, "dud", 1_000)
    duds = kind(rows, "weapon.dud")
    if len(duds) != 1:
        return problems + [f"dud: {len(duds)} dud events, want 1"]
    f = duds[0]["fields"]
    if not f["flown_s"] < f["arm_s"] or "before its" not in f.get("reason", ""):
        problems.append(f"dud: {f}")
    outcome = kind(rows, "weapon.outcome")
    if len(outcome) != 1 or outcome[0]["fields"]["result"] != "missed" or \
            "before its fuze armed" not in outcome[0]["fields"].get("reason", ""):
        problems.append(f"dud: the outcome is {[o['fields'] for o in outcome]}")
    hurt = [m for m in re.findall(r"^a2g-run: target 0x\w+ hp (\d+)/(\d+)$", out, re.M) if m[0] != m[1]]
    if kind(rows, "surface.wreck") or hurt:
        problems.append("dud: a dud damaged the target")
    return problems


def rocket_problems(work: Path, out: str) -> list[str]:
    rows = events(work, "rocket")
    problems = verify_problems(out, "rocket", 1_000)
    shots = kind(rows, "weapon.release")
    if len(shots) < 4:
        return problems + [f"rocket: {len(shots)} rocket releases, want a salvo of at least 4"]
    fields = [d["fields"] for d in shots]
    if [f["salvo"] for f in fields] != list(range(1, len(fields) + 1)):
        problems.append(f"rocket: salvo numbers {[f['salvo'] for f in fields]}, want 1 to {len(fields)}")
    if any(f["class"] != "rocket" or f["delivery"] != "rocket" or f["count"] != 1 for f in fields):
        problems.append("rocket: a release is not one rocket delivered by the rocket pipper")
    left = [f["left"] for f in fields]
    if left != [left[0] - i for i in range(len(left))]:
        problems.append(f"rocket: rockets left {left}, want one fewer each time")
    if weapons(work, "rocket").get("LAU-61", {}).get("class") != "rocket":
        problems.append("rocket: the pod is not recorded as the rocket class")
    wreck = kind(rows, "surface.wreck")
    if not wreck or "LAU-61" not in wreck[0]["fields"].get("reason", ""):
        problems.append(f"rocket: the truck's loss does not name the pod: {[w['fields'] for w in wreck]}")
    return problems


def cluster_problems(work: Path, out: str) -> list[str]:
    rows = events(work, "cluster")
    problems = verify_problems(out, "cluster", 1_000)
    bursts = kind(rows, "weapon.cluster")
    if not bursts:
        return problems + ["cluster: no cluster burst recorded"]
    for burst in bursts:
        if burst["fields"].get("explosion") not in (18, 19, 20):
            problems.append(f"cluster: burst {burst['fields']}")
    if "CBU-87" not in weapons(work, "cluster"):
        problems.append("cluster: the CBU-87 is not a registered weapon")
    return problems


def laser_problems(work: Path, out: str) -> list[str]:
    rows = events(work, "laser")
    problems = verify_problems(out, "laser", 6_000)
    lasing = [d for d in kind(rows, "player.laser") if d["fields"].get("state") == "lasing"]
    released = kind(rows, "weapon.guided", released=True)
    if not lasing or not released:
        return problems + [f"laser: {len(lasing)} lasing events and {len(released)} releases"]
    g = released[0]["fields"]
    if g["guidance"] != "laser" or g["mode"] != "guiding" or g.get("own") is not True or g.get("spot_owner") != 0:
        problems.append(f"laser: released as {g}")
    if lasing[0]["tick"] > released[0]["tick"]:
        problems.append("laser: the bomb was released before the laser came on")
    if not kind(rows, "weapon.release", delivery="guided"):
        problems.append("laser: the release is not marked guided")
    if not kind(rows, "player.designate"):
        problems.append("laser: the target was never designated")
    outcome = kind(rows, "weapon.outcome")
    if not outcome or outcome[-1]["fields"]["result"] != "hit" or \
            "guided on the shooter's own laser" not in outcome[-1]["fields"].get("reason", ""):
        problems.append(f"laser: outcome {[o['fields'] for o in outcome]}")
    wreck = kind(rows, "surface.wreck")
    if not wreck or not re.search(r"hit by an? GBU-10\w* guided on the shooter's own laser",
                                  wreck[0]["fields"].get("reason", "")):
        problems.append(f"laser: the loss does not name the bomb and the laser: {[w['fields'] for w in wreck]}")
    text = (work / "log-laser" / "summary.txt")
    if text.exists() and "guided on the shooter's own laser" not in text.read_text():
        problems.append("laser: the summary does not say whose laser guided the bomb")
    problems += marks_problems(rows, "laser", near=True)
    return problems


def marks_problems(rows: list[dict], label: str, near: bool) -> list[str]:
    """What the replay viewer's laser and bomb marks draw from (slice FX-R): the spot and the aim
    followed while they move, and a landing with its miss distance."""
    problems = []
    impact = kind(rows, "weapon.impact")
    if len(impact) != 1:
        return [f"{label}: {len(impact)} weapon.impact events, want one"]
    f = impact[0]["fields"]
    for name in ("x_ft", "z_ft", "mode", "miss_ft", "aim_x_ft", "reason"):
        if name not in f:
            problems.append(f"{label}: the impact has no {name}: {f}")
    if f.get("guided") is not True:
        problems.append(f"{label}: the impact is not marked guided")
    if "miss_ft" in f and near != (f["miss_ft"] < 100):
        problems.append(f"{label}: the bomb landed {f['miss_ft']} ft from its aim point, want {'under' if near else 'over'} 100")
    spots = kind(rows, "player.laser_spot")
    aims = kind(rows, "weapon.aim")
    if not spots or not aims:
        problems.append(f"{label}: {len(spots)} laser spot entries and {len(aims)} aim entries, want some of each")
    for entries in (spots, aims):
        ticks = [d["tick"] for d in entries]
        if any(b - a < 12 for a, b in zip(ticks, ticks[1:])):
            problems.append(f"{label}: spot or aim entries closer than 12 ticks")
    return problems


def blind_problems(work: Path, out: str) -> list[str]:
    rows = events(work, "blind")
    problems = verify_problems(out, "blind", 4_000)
    changes = kind(rows, "weapon.guided", released=False)
    if not changes:
        return problems + ["blind: the bomb never changed guidance mode"]
    g = changes[0]["fields"]
    if (g.get("from"), g["mode"]) != ("guiding", "blind") or "your laser went off" not in g.get("reason", ""):
        problems.append(f"blind: change {g}")
    off = [d for d in kind(rows, "player.laser") if d["tick"] >= changes[0]["tick"]]
    if not off or off[0]["fields"]["state"] not in ("ready", "off"):
        problems.append(f"blind: the laser did not go off with the bomb: {[d['fields'] for d in off]}")
    outcome = kind(rows, "weapon.outcome")
    if not outcome or "went blind when your laser went off" not in outcome[-1]["fields"].get("reason", ""):
        problems.append(f"blind: outcome {[o['fields'] for o in outcome]}")
    problems += marks_problems(rows, "blind", near=False)
    landed = kind(rows, "weapon.impact")
    if landed and landed[0]["fields"].get("mode") != "blind":
        problems.append("blind: the impact does not say the bomb was blind")
    return problems


def buddy_problems(work: Path, out: str) -> list[str]:
    rows = events(work, "buddy")
    problems: list[str] = []
    released = kind(rows, "weapon.guided", released=True)
    if not released:
        return ["buddy: no guided release recorded"]
    g = released[0]["fields"]
    if g.get("own") is not False or g.get("spot_owner") in (None, 0) or "buddy lasing" not in g.get("reason", ""):
        problems.append(f"buddy: released as {g}")
    lasers = {d["subject"] for d in kind(rows, "player.laser") if d["fields"].get("state") == "lasing"}
    if g.get("spot_owner") not in lasers:
        problems.append(f"buddy: the spot owner {g.get('spot_owner')} never lased, only {sorted(lasers)}")
    outcome = kind(rows, "weapon.outcome")
    if not outcome or "'s laser" not in outcome[-1]["fields"].get("reason", ""):
        problems.append(f"buddy: the outcome does not name the buddy's laser: {[o['fields'] for o in outcome]}")
    return problems


def jdam_problems(work: Path, out: str) -> list[str]:
    rows = events(work, "jdam")
    problems = verify_problems(out, "jdam", 4_000)
    released = kind(rows, "weapon.guided", released=True)
    if not released:
        return problems + ["jdam: no guided release recorded"]
    g = released[0]["fields"]
    if g["guidance"] != "coordinate" or g["mode"] != "guiding" or g.get("stored") is not True or \
            not all(k in g for k in ("x_ft", "y_ft", "z_ft")):
        problems.append(f"jdam: released as {g}")
    outcome = kind(rows, "weapon.outcome")
    if not outcome or "stored at its release" not in outcome[-1]["fields"].get("reason", ""):
        problems.append(f"jdam: outcome {[o['fields'] for o in outcome]}")
    land = kind(rows, "weapon.impact")
    if len(land) != 1 or "miss_ft" not in land[0]["fields"] or land[0]["fields"]["miss_ft"] > 100 or \
            (land[0]["fields"].get("aim_x_ft"), land[0]["fields"].get("aim_z_ft")) != (g.get("x_ft"), g.get("z_ft")):
        problems.append(f"jdam: the impact does not measure from the stored point: {[d['fields'] for d in land]}")
    return problems


def harm_problems(work: Path, out: str, name: str, mode: str) -> list[str]:
    rows = events(work, name)
    problems = verify_problems(out, name, 1_500)
    launches = [d for d in kind(rows, "weapon.launch") if d.get("subject") == 0]
    if len(launches) != 1:
        return problems + [f"{name}: {len(launches)} launches by the player, want 1"]
    f = launches[0]["fields"]
    if f.get("mode") != mode or f.get("surface") is not True or not f.get("range_ft", 0) > 20_000:
        problems.append(f"{name}: launch {f}, want {mode} on the surface with its range")
    if launches[0].get("object", 0) < SURFACE_IDS:
        problems.append(f"{name}: the HARM has no radar as its target")
    wreck = kind(rows, "surface.wreck")
    if not wreck or "hit by an AGM-88" not in wreck[0]["fields"].get("reason", ""):
        problems.append(f"{name}: the radar's loss does not name the HARM: {[w['fields'] for w in wreck]}")
    if mode == "cued":
        pages = [d for d in kind(rows, "player.designate") if d["fields"].get("channel") == "harm page"]
        if not pages:
            problems.append("harm-cued: no designation from the HARM page")
        if pages and launches[0]["object"] != pages[-1]["object"]:
            problems.append("harm-cued: the HARM left at a radar the page did not hold")
    return problems


def scenarios() -> list[Scenario]:
    steps: list[Step] = []
    index: dict[str, int] = {}

    def add(args: list[str], timeout: float = 300.0) -> int:
        steps.append(Step(args, timeout=timeout))
        return len(steps)

    for n in range(1, len(PASSES)):
        index[PASSES[n][0]] = add(run_args(n))
    for name, _, _ in PASSES:
        add(["--recording-log", f"{{work}}/{name}.tore-replay", "--out", f"{{work}}/log-{name}", "--rate", "5"])
    add(["--recording-acmi", "{work}/pack.tore-replay", "--out", "{work}/pack.acmi"])
    again = add(run_args(0, "pack-again"))
    diff = add(["--recording-diff", "{work}/pack.tore-replay", "{work}/pack-again.tore-replay"])
    info = add(["--recording-info", "{work}/pack.tore-replay"])
    old = add(["--recording-info", str(FORMAT_2)])

    def check(work: Path, output: str) -> list[str]:
        s = sections(output)
        run_out = {PASSES[0][0]: s[0], **{name: s.get(i, "") for name, i in index.items()}}
        acmi = work / "pack.acmi"
        problems = pack_problems(work, run_out["pack"], acmi.read_text() if acmi.exists() else "")
        problems += dud_problems(work, run_out["dud"])
        problems += rocket_problems(work, run_out["rocket"])
        problems += cluster_problems(work, run_out["cluster"])
        problems += laser_problems(work, run_out["laser"])
        problems += blind_problems(work, run_out["blind"])
        problems += buddy_problems(work, run_out["buddy"])
        problems += jdam_problems(work, run_out["jdam"])
        problems += harm_problems(work, run_out["harm-bore"], "harm-bore", "boresight")
        problems += harm_problems(work, run_out["harm-cued"], "harm-cued", "cued")
        for name, _, _ in PASSES:
            problems += rc.file_problems(work, f"log-{name}/log.jsonl", rc.check_jsonl)
        problems += rc.file_problems(work, "pack.acmi", rc.check_acmi)
        problems += verify_problems(s.get(again, ""), "the second pack run", 2_000)
        if "Verdict: the recordings match" not in s.get(diff, ""):
            problems.append("two identical pack runs made recordings that differ")
        if "format 3" not in s.get(info, "") or not re.search(r"^Weapon +0 MK-82AIR7 \(MK82AIR\.JT, bomb\)", s.get(info, ""), re.M):
            problems.append("recording-info does not show a format 3 file with the pack's bomb class")
        if "format 2" not in s.get(old, "") or "Problem " in s.get(old, ""):
            problems.append("the format 2 recording does not open")
        return problems

    outputs = [f"{name}.tore-replay" for name, _, _ in PASSES] + ["pack-again.tore-replay"]
    return [
        Scenario(
            name="replay-a2g",
            lane="replay",
            args=run_args(0),
            timeout=300,
            expect=[r"a2g-run: done"],
            outputs=outputs,
            then=steps,
            check_work=check,
            notes="bombs, rockets, lasers, JDAMs and HARMs recorded, replayed tick for tick and read back as events",
        )
    ]
