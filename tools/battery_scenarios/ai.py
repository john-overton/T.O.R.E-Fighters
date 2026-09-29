"""Lane: AI fights, wings, damage and missiles (headless probes).

Every scenario is one `--ai-probe-ticks` run (or the roster and matrix probes).
`probe_problems` reads the probe's own lines and reports impossible states: the
probe's `AI probe anomaly:` lines (written by crates/tore-app/src/probe_invariants.rs
from every tick), actors whose final state disagrees with the events the probe
printed, debrief kill counts that disagree with the fight, dropped launches,
open ground hazards and radio lines that repeat. See docs/testing/lane-ai.md.
"""
from __future__ import annotations

import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Callable, Optional

from battery import ROOT, Scenario

AIRCRAFT = ["f18", "rafale", "f14", "a4e", "x31", "mig29", "su27", "mig21", "su25", "mig23", "su35", "f22", "f22n", "faxx"]
SKILLS = ["novice", "average", "experienced", "ace"]
MISSIONS = ["free", "cap", "intercept", "escort", "self-defense", "hold"]
GEOMETRIES = ["head", "side", "rear"]
ADAPTERS = ["legacy", "researched"]
# Base theaters (--validate-maps lists them); every one has airports 1 and 3.
THEATERS = ["APA", "BAL", "CUB", "EGY", "FRA", "GRE", "IRA", "KURILE", "LFA", "NSK", "PGU", "SPA", "TVIET", "UKR", "VLA", "WTA"]
# Known-good airport for ground starts in the default theater (Ukraine).
GROUND_AIRPORT = "2"

ACTOR = re.compile(r"^actor=(\d+) (\S+ \d-\d) (\S+) activity=(.*?) alive=(true|false) rounds=(\d+) ")
INVARIANTS = re.compile(r"^AI probe invariants: .*anomalies=(\d+)", re.M)
ANOMALY = re.compile(r"^AI probe anomaly: (.*)$", re.M)
TOTALS = re.compile(r"^AI probe totals: wings=(\d+) ticks=(\d+) shots=(\d+) dropped=(\d+) warnings=(\d+) live_projectiles=(\d+) player_hp=(-?\d+)", re.M)
DEBRIEF = re.compile(r"^AI probe debrief: (\S+) \[(.*?)\] elapsed=(\d+)s player\[(\S+) damage=(\d+)% kills=\[([\d, ]*)\] ff=(\d+)", re.M)
ATTACK = re.compile(r"^AI probe attack: clicks=.*? missiles=(\d+) .*?player_kills=(\d+) hits=(\d+) destroyed=(\d+) lost_friendly=(\d+)/(\d+) lost_enemy=(\d+)/(\d+) player_alive=(true|false) .*?ejections=(\d+) ", re.M)
RADIO_LINE = re.compile(r"^  (\d+\.\d)s (.+?): '(.*)' \[", re.M)
EVENT = re.compile(r"^t=(\d+) \(([\d.]+)s\) (?:destroyed: (\S+ \d-\d)|(\S+ \d-\d) pilot ejected)", re.M)
HAZARDS = re.compile(r"^AI probe liftoff gaps: \[(.*?)\] hazards_open=(\d+)", re.M)
PHASES = re.compile(r"^AI probe phases: (\S+ \d-\d): (.*)$", re.M)


# Anomaly kinds that are real but wait on a decision (docs/testing/lane-ai.md,
# "Found, not fixed" and "Needs a decision"): a fighter at the edge of a
# missile's employment zone alternates every few ticks between its maneuver and
# gun tracking, and AI aircraft collide in the air (leaders of different wings
# have no deconfliction; wingmen breaking out of formation can turn into each
# other). Scenarios pass `strict=True` to fail on them anyway.
KNOWN_ANOMALIES = ("activity flapping", "pitch stick oscillating", "mid-air collision")


def probe_problems(
    output: str,
    *,
    strict: bool = False,
    allow_anomalies: tuple[str, ...] = (),
    ground: bool = False,
    need_takeoff: bool = False,
    need_landing: bool = False,
) -> list[str]:
    """Impossible or inconsistent states in one AI probe's output."""
    problems: list[str] = []
    inv = INVARIANTS.search(output)
    if not inv:
        problems.append("no 'AI probe invariants' line (probe did not finish?)")
    allowed = allow_anomalies + (() if strict else KNOWN_ANOMALIES)
    for line in ANOMALY.findall(output):
        if not any(kind in line for kind in allowed):
            problems.append(f"anomaly: {line[:200]}")
    actors: dict[str, tuple[str, bool, int]] = {}
    for raw in output.splitlines():
        m = ACTOR.match(raw)
        if not m:
            continue
        _, label, _, activity, alive, rounds = m.groups()
        actors[label] = (activity, alive == "true", int(rounds))
        if alive == "true" and activity == "Destroyed":
            problems.append(f"{label} alive but its activity says Destroyed")
        if alive == "false" and activity != "Destroyed":
            problems.append(f"{label} dead but its activity says {activity}")
    problems.extend(ground_collisions(output, actors))
    for m in EVENT.finditer(output):
        label = m.group(3) or m.group(4)
        if label in actors and actors[label][1]:
            what = "destroyed" if m.group(3) else "ejected"
            problems.append(f"{label} {what} at {m.group(2)}s but alive at the end")
    totals = TOTALS.search(output)
    if totals and int(totals.group(4)) > 0:
        problems.append(f"dropped launches: {totals.group(4)}")
    debrief = DEBRIEF.search(output)
    attack = ATTACK.search(output)
    if debrief and attack:
        kills = sum(int(k) for k in debrief.group(6).split(",") if k.strip())
        # The combat counter counts only destructions by the player's own
        # weapons; the debrief also credits aircraft the player damaged that
        # then went down (ejection, crash), so it may be larger, never smaller.
        if kills < int(attack.group(2)):
            problems.append(f"debrief player kills {kills} but combat counted {attack.group(2)}")
        lost_enemy, enemies = int(attack.group(7)), int(attack.group(8))
        objective = re.search(r"Destroy \{ destroyed: (\d+), total: (\d+) \}", debrief.group(2))
        # Only free engagement makes every enemy aircraft the objective.
        if objective and re.search(r"^AI probe: .* mission=free$", output, re.M) and (int(objective.group(1)), int(objective.group(2))) != (lost_enemy, enemies):
            problems.append(
                f"debrief objective {objective.group(1)}/{objective.group(2)} but {lost_enemy}/{enemies} enemies lost"
            )
        player_alive = attack.group(9) == "true"
        if player_alive != (debrief.group(4) != "Dead") and debrief.group(4) not in ("Ejected", "Captured", "Rescued"):
            problems.append(f"debrief says player {debrief.group(4)} but probe says alive={player_alive}")
    hazards = HAZARDS.search(output)
    if ground and hazards and int(hazards.group(2)) > 0:
        problems.append(f"ground hazards still open at the end: {hazards.group(2)}")
    if ground:
        for label, phases in PHASES.findall(output):
            names = [p.split("@")[0] for p in phases.split()]
            if need_takeoff and "ClimbOut" not in names and "Airborne" not in names:
                problems.append(f"{label} never took off: {phases[:160]}")
            if need_landing and label in actors and actors[label][1] and not any(n in names for n in ("Rollout", "Landed", "Parked", "TaxiIn")):
                problems.append(f"{label} never landed: {phases[:160]}")
    problems.extend(radio_problems(output))
    return problems


DEATH = re.compile(r"^t=\d+ \(([\d.]+)s\) (\S+ \d-\d) \S+: (?:phase=\S+ activity=\"[^\"]*\" )?alive=false agl=(-?\d+) kt=(\d+)", re.M)
DAMAGED = re.compile(r"^AI damage: actor=(\d+) ", re.M)


def ground_collisions(output: str, actors: dict) -> list[str]:
    """AI aircraft that flew into the ground undamaged: they died on the
    ground with no weapon kill, no ejection and no system fault."""
    destroyed = set(re.findall(r"destroyed: (\S+ \d-\d)", output))
    ejected = set(re.findall(r"(\S+ \d-\d) pilot ejected", output))
    damaged = set(DAMAGED.findall(output))
    ids = {m.group(2): m.group(1) for m in (ACTOR.match(line) for line in output.splitlines()) if m}
    problems = []
    for m in DEATH.finditer(output):
        at, label, agl = m.group(1), m.group(2), int(m.group(3))
        if agl < 40 and label not in destroyed and label not in ejected and ids.get(label) not in damaged:
            problems.append(f"{label} flew into the ground undamaged at {at}s")
    return problems


def radio_problems(output: str) -> list[str]:
    """Radio lines that flood: the same speaker and words three times within five seconds."""
    problems = []
    recent: dict[tuple[str, str], list[float]] = {}
    for at, speaker, words in RADIO_LINE.findall(output):
        times = recent.setdefault((speaker, words), [])
        times.append(float(at))
        window = [t for t in times if float(at) - t <= 5.0]
        if len(window) >= 3:
            problems.append(f"radio repeats {speaker}: '{words}' {len(window)} times in 5 s")
            times.clear()
    return problems


def checker(**kw) -> Callable[[str], list[str]]:
    return lambda output: probe_problems(output, **kw)


def rerun_same(args: list[str]) -> Callable[[str], list[str]]:
    """Determinism: run the same probe again on a fresh profile copy and compare."""

    def strip(text: str) -> list[str]:
        # Log lines carry wall-clock stamps; everything else must match.
        return [line for line in text.splitlines() if not re.match(r"^\d+\.\d+Z ", line)]

    def check(output: str) -> list[str]:
        problems = probe_problems(output)
        profile = Path(os.environ.get("TORE_BATTERY_PROFILE", ROOT / ".local" / "bugbash-data"))
        binary = os.environ.get("TORE_BATTERY_BIN", str(ROOT / "target" / "debug" / "tore-app"))
        with tempfile.TemporaryDirectory(prefix="bb-ai-det-") as tmp:
            data = Path(tmp) / "data"
            if subprocess.run(["cp", "-a", "--reflink=auto", str(profile), str(data)]).returncode != 0:
                shutil.copytree(profile, data)
            env = dict(os.environ, TORE_DATA_DIR=str(data), TORE_NO_ERROR_DIALOG="1")
            again = subprocess.run([binary, *args], env=env, capture_output=True, text=True, errors="replace", timeout=900)
        a, b = strip(output), strip(again.stdout + again.stderr)
        if a != b:
            first = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
            left = a[first] if first < len(a) else "<end>"
            right = b[first] if first < len(b) else "<end>"
            problems.append(f"not deterministic at line {first}: {left[:120]!r} vs {right[:120]!r}")
        return problems

    return check


def recordings_match(args: list[str]) -> Callable[[str], list[str]]:
    """Determinism of recordings: record the probe twice on fresh profile
    copies and require `--recording-diff` to say they match."""

    def check(output: str) -> list[str]:
        problems = probe_problems(output)
        profile = Path(os.environ.get("TORE_BATTERY_PROFILE", ROOT / ".local" / "bugbash-data"))
        binary = os.environ.get("TORE_BATTERY_BIN", str(ROOT / "target" / "debug" / "tore-app"))
        with tempfile.TemporaryDirectory(prefix="bb-ai-rec-") as tmp:
            paths = []
            for n in range(2):
                data = Path(tmp) / f"data{n}"
                if subprocess.run(["cp", "-a", "--reflink=auto", str(profile), str(data)]).returncode != 0:
                    shutil.copytree(profile, data)
                path = Path(tmp) / f"run{n}.tore-replay"
                env = dict(os.environ, TORE_DATA_DIR=str(data), TORE_NO_ERROR_DIALOG="1")
                subprocess.run([binary, *args, "--record-mission", str(path)], env=env, capture_output=True, timeout=900)
                paths.append(str(path))
            diff = subprocess.run([binary, "--recording-diff", *paths], capture_output=True, text=True, errors="replace", timeout=300)
        if "Verdict: the recordings match" not in diff.stdout:
            tail = [line for line in diff.stdout.splitlines() if line.strip()][-3:]
            problems.append(f"two recordings of the same probe differ: {' / '.join(tail)[:200]}")
        return problems

    return check


def probe(name: str, args: list[str], *, ticks: int = 7200, timeout: float = 900, check=None, expect=None, outputs=None, notes: str = "") -> Scenario:
    return Scenario(
        name=f"ai-{name}",
        lane="ai",
        args=["--ai-probe-ticks", str(ticks), *args, "--no-audio"],
        timeout=timeout,
        expect=[r"AI probe totals:", *(expect or [])],
        check=check or checker(),
        outputs=outputs or [],
        notes=notes,
    )


def fight(f: int, e: int, *extra: str) -> list[str]:
    return ["--probe-fight", f"{f}:{e}", *extra]


def scenarios() -> list[Scenario]:
    out: list[Scenario] = []
    attack = ["--probe-attack", "600:10"]

    # 1. Fight sizes, even and lopsided, the default F/A-18D against itself.
    sizes = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (7, 7), (8, 8), (10, 10), (12, 12), (15, 15),
             (1, 15), (15, 1), (5, 10), (10, 5), (3, 12), (12, 3), (1, 5), (6, 2)]
    for f, e in sizes:
        out.append(probe(f"fight-{f}v{e}-default", fight(f, e, "--separation", "5", *attack)))
        out.append(probe(f"fight-{f}v{e}-noattack", fight(f, e, "--separation", "10"), ticks=9600))

    # 2. Every aircraft against every aircraft, two a side.
    for i, fa in enumerate(AIRCRAFT):
        for j, ea in enumerate(AIRCRAFT):
            skill = SKILLS[(i + j) % 4]
            out.append(probe(
                f"pair-{fa}-vs-{ea}",
                fight(2, 2, "--aircraft", fa, "--probe-friendly-aircraft", fa, "--probe-enemy-aircraft", ea,
                      "--probe-enemy-skill", skill, "--separation", "5", *attack),
                ticks=6000,
            ))

    # 3. Each aircraft as the enemy, bigger fights, researched adapter.
    for i, ea in enumerate(AIRCRAFT):
        fa = AIRCRAFT[(i + 5) % len(AIRCRAFT)]
        out.append(probe(
            f"big-{fa}-vs-{ea}-researched",
            fight(6, 6, "--aircraft", fa, "--probe-friendly-aircraft", fa, "--probe-enemy-aircraft", ea,
                  "--probe-flight-model", "researched", "--separation", "10", *attack),
            ticks=9600,
        ))

    # 4. Skills x geometry x adapter, one against one and three against three.
    for skill in SKILLS:
        for geo in GEOMETRIES:
            for adapter in ADAPTERS:
                out.append(probe(
                    f"skill-{skill}-{geo}-{adapter}-1v1",
                    ["--aircraft", "f22", "--probe-enemy-aircraft", "su27", "--probe-enemy-skill", skill,
                     "--probe-geometry", geo, "--probe-flight-model", adapter, "--separation", "2", *attack],
                    ticks=6000,
                ))
                out.append(probe(
                    f"skill-{skill}-{geo}-{adapter}-3v3",
                    fight(3, 3, "--aircraft", "mig29", "--probe-friendly-aircraft", "mig29", "--probe-enemy-aircraft", "f14",
                          "--probe-enemy-skill", skill, "--probe-geometry", geo, "--probe-flight-model", adapter,
                          "--separation", "5", *attack),
                    ticks=6000,
                ))

    # 5. Missions at three sizes, with and without the leader attacking.
    for mission in MISSIONS:
        for f, e in [(1, 1), (4, 4), (10, 10)]:
            out.append(probe(f"mission-{mission}-{f}v{e}", fight(f, e, "--ai-mission", mission, "--separation", "5", *attack)))
            out.append(probe(f"mission-{mission}-{f}v{e}-passive", fight(f, e, "--ai-mission", mission, "--separation", "10"), ticks=9600))

    # 6. Separations.
    for sep, ticks in [(1, 4800), (2, 6000), (10, 12000), (20, 18000), (50, 36000), (100, 48000)]:
        out.append(probe(f"separation-{sep}nm-3v3", fight(3, 3, "--separation", str(sep), *attack), ticks=ticks, timeout=1800))

    # 7. Guns only.
    for fa, ea in [("f18", "mig21"), ("a4e", "su25"), ("f14", "mig23"), ("x31", "rafale"), ("su35", "f22n")]:
        out.append(probe(f"guns-{fa}-vs-{ea}", fight(2, 2, "--aircraft", fa, "--probe-friendly-aircraft", fa, "--probe-enemy-aircraft", ea,
                                                       "--probe-guns", "--probe-ai-guns-only", "--separation", "2", *attack), ticks=9600))
        out.append(probe(f"guns-ai-only-{fa}-vs-{ea}", ["--aircraft", fa, "--probe-enemy-aircraft", ea, "--probe-ai-guns-only",
                                                        "--probe-geometry", "rear", "--separation", "1"], ticks=6000))

    # 8. Fault injection into the first enemy, every index, and controlled threats.
    for index in range(45):
        out.append(probe(f"fault-{index:02d}", ["--probe-enemy-aircraft", "su27", "--probe-fault", f"240:{index}",
                                                 "--separation", "5", "--ai-mission", "hold"], ticks=4800))
    for threat in ["hit", "gun", "aaa"]:
        for geo in GEOMETRIES:
            out.append(probe(f"threat-{threat}-{geo}", ["--aircraft", "f22", "--probe-enemy-aircraft", "mig29", "--probe-geometry", geo,
                                                        "--probe-threat", f"120:{threat}", "--probe-threat", f"1200:{threat}",
                                                        "--separation", "1", "--ai-mission", "hold"], ticks=4800))
    out.append(probe("threat-many-hits", ["--probe-enemy-aircraft", "su35"] + sum((["--probe-threat", f"{120 + 60 * k}:hit"] for k in range(64)), []),
                     ticks=6000))

    # 9. Wing orders in the air.
    for order in ["bug-out", "land-selected", "attack-on-contact", "engage-my-target"]:
        for size in [2, 5]:
            # Engage-my-target needs the leader's designation, made at tick 600.
            at = 700 if order == "engage-my-target" else 600
            out.append(probe(f"order-{order}-wing{size}", ["--probe-wing-size", str(size), "--probe-wing-order", f"{at}:{order}",
                                                           "--separation", "5", "--probe-attack", "600:10"], ticks=12000,
                             expect=[r"order=\S+ (reply|refused)"]))

    # 10. Ground starts: takeoff, formation, landing orders.
    for size in [1, 2, 3, 4, 5]:
        out.append(probe(f"ground-takeoff-wing{size}", ["--ground-start", GROUND_AIRPORT, "--probe-wing-size", str(size), "--maneuver", "takeoff",
                                                         "--separation", "50"], ticks=6000 + 6000 * size,
                         check=checker(ground=True, need_takeoff=True)))
    for size in [2, 4]:
        out.append(probe(f"ground-land-selected-wing{size}", ["--ground-start", GROUND_AIRPORT, "--probe-wing-size", str(size), "--maneuver", "takeoff",
                                                              "--probe-wing-order", "9000:land-selected", "--separation", "200", "--probe-wing-only"],
                         # A wing's approach over the hills south of Simferopol takes about
                         # 400 s per aircraft (lane doc, "Needs a decision"), so only the
                         # pair is expected to be down inside the run.
                         ticks=90000, timeout=1800,
                         check=checker(ground=True, need_takeoff=True, need_landing=size == 2)))
        # Bug out is ignored while taking off (spec), so order it once the wing is up.
        out.append(probe(f"ground-bug-out-wing{size}", ["--ground-start", GROUND_AIRPORT, "--probe-wing-size", str(size), "--maneuver", "takeoff",
                                                        "--probe-wing-order", f"{6000 * size}:bug-out", "--separation", "200", "--probe-wing-only"],
                         ticks=48000, timeout=1800, check=checker(ground=True, need_takeoff=True)))
        out.append(probe(f"ground-fight-wing{size}", ["--ground-start", GROUND_AIRPORT, "--probe-wing-size", str(size), "--maneuver", "takeoff",
                                                      "--separation", "20", "--probe-attack", "6000:10"], ticks=24000, timeout=1800,
                         check=checker(ground=True, need_takeoff=True)))
    out.append(probe("ground-idle-wing4", ["--ground-start", GROUND_AIRPORT, "--probe-wing-size", "4", "--separation", "50", "--probe-wing-only"],
                     ticks=7200, check=checker(ground=True)))

    # 11. Long runs: 30 simulated minutes, looking for slow leaks and stuck aircraft.
    for name, args in [
        ("long-1v1", fight(1, 1, "--separation", "10", *attack)),
        ("long-5v5", fight(5, 5, "--separation", "20", *attack)),
        ("long-15v15", fight(15, 15, "--separation", "20", *attack)),
        ("long-hold-4v4", fight(4, 4, "--ai-mission", "hold", "--separation", "20")),
        ("long-guns-3v3", fight(3, 3, "--probe-guns", "--probe-ai-guns-only", "--separation", "5", *attack)),
        ("long-ground-land", ["--ground-start", GROUND_AIRPORT, "--probe-wing-size", "4", "--maneuver", "takeoff",
                              "--probe-wing-order", "30000:land-selected", "--separation", "200", "--probe-wing-only"]),
    ]:
        # With no route an aircraft holds its heading (B48) and can fly off the
        # map in half an hour; recorded under "Needs a decision" in the lane doc.
        out.append(probe(name, args, ticks=216000, timeout=3600,
                         check=checker(ground="ground" in name, need_takeoff="ground" in name,
                                       allow_anomalies=("outside the world",))))

    # 12. Recordings: render verification and a readable log.
    for name, args in [
        ("record-2v2", fight(2, 2, "--separation", "5", *attack)),
        ("record-15v15", fight(15, 15, "--separation", "5", *attack)),
        ("record-guns", fight(2, 2, "--probe-guns", "--probe-ai-guns-only", "--separation", "2", *attack)),
        ("record-researched", fight(4, 4, "--probe-flight-model", "researched", "--separation", "5", *attack)),
        ("record-fault", ["--probe-fault", "300:11", "--separation", "2", *attack]),
        ("record-threat-aaa", ["--probe-threat", "120:aaa", "--probe-geometry", "rear", "--separation", "1", "--ai-mission", "hold"]),
        ("record-ground", ["--ground-start", GROUND_AIRPORT, "--probe-wing-size", "3", "--maneuver", "takeoff", "--separation", "20"]),
    ]:
        out.append(probe(name, [*args, "--record-mission", "{work}/a.tore-replay", "--verify-render"],
                         ticks=7200, expect=[r"AI probe verify-render: PASS"], outputs=["a.tore-replay"],
                         check=checker(ground="ground" in name)))

    # 13. Determinism: the same arguments twice give the same output.
    for name, args in [
        ("determinism-1v1", ["--ai-probe-ticks", "6000", *fight(1, 1, "--separation", "2", *attack), "--no-audio"]),
        ("determinism-8v8", ["--ai-probe-ticks", "6000", *fight(8, 8, "--separation", "5", *attack), "--no-audio"]),
        ("determinism-researched", ["--ai-probe-ticks", "6000", *fight(4, 4, "--probe-flight-model", "researched", *attack), "--no-audio"]),
        ("determinism-ground", ["--ai-probe-ticks", "9000", "--ground-start", GROUND_AIRPORT, "--probe-wing-size", "4", "--maneuver", "takeoff", "--no-audio"]),
    ]:
        out.append(Scenario(name=f"ai-{name}", lane="ai", args=args, timeout=1800,
                            expect=[r"AI probe totals:"], check=rerun_same(args)))

    recorded = ["--ai-probe-ticks", "4800", *fight(5, 5, "--separation", "5", *attack), "--no-audio"]
    out.append(Scenario(name="ai-determinism-recordings", lane="ai", args=recorded, timeout=1800,
                        expect=[r"AI probe totals:"], check=recordings_match(recorded)))

    # 13b. Regressions for fixed defects, checked strictly.
    out.append(probe("regress-visual-incoming-flap", fight(6, 6, "--aircraft", "f22", "--probe-friendly-aircraft", "f22",
                                                           "--probe-enemy-aircraft", "mig29", "--separation", "10", *attack),
                     ticks=14400, check=checker(strict=True),
                     notes="wingman flipped between missile defense and formation every tick (fixed 2026-09-28)"))
    def wingman_back_in_fight(output: str) -> list[str]:
        problems = probe_problems(output)
        if re.search(r"^actor=3 Enemy 1-2 \S+ activity=(Landing|Holding at marshal) alive=true", output, re.M):
            problems.append("Enemy 1-2 is still following its ejected leader in to land")
        return problems

    out.append(probe("regress-wingman-dead-leader", fight(2, 2, "--aircraft", "f18", "--probe-friendly-aircraft", "f18",
                                                          "--probe-enemy-aircraft", "su27", "--probe-enemy-skill", "ace",
                                                          "--separation", "5", *attack),
                     ticks=6000, check=wingman_back_in_fight,
                     notes="wingman kept landing after its damaged leader ejected (fixed 2026-09-28)"))
    out.append(probe("regress-envelope-edge-flap", fight(8, 8, "--separation", "5", *attack), ticks=7200,
                     check=checker(strict=True, allow_anomalies=("mid-air collision",)),
                     notes="Friendly 2-3 swapped between its missile tactic and gun tracking every few ticks (fixed 2026-09-29)"))
    out.append(probe("regress-decoy-over-100", fight(2, 2, "--aircraft", "su25", "--probe-friendly-aircraft", "su25",
                                                     "--probe-enemy-aircraft", "mig21", "--separation", "5", *attack),
                     ticks=6000, notes="mission aborted: decoy percentages exceed 100 (fixed 2026-09-28)"))
    out.append(probe("regress-terrain-f14-dive", ["--probe-fight", "3:3", "--aircraft", "mig29", "--probe-friendly-aircraft", "mig29",
                                                  "--probe-enemy-aircraft", "f14", "--probe-enemy-skill", "novice", "--probe-geometry", "head",
                                                  "--probe-flight-model", "legacy", "--separation", "5", *attack],
                     ticks=6000, notes="undamaged F-14 flew into the ground turning at 72 degrees of bank (fixed 2026-09-28)"))
    out.append(probe("regress-terrain-f18-hill", fight(1, 1, "--separation", "10", *attack), ticks=24000,
                     notes="undamaged F/A-18D eased into a hillside at 2 G (fixed 2026-09-28)"))

    # 15. Other theaters: fights, ground starts, takeoff and landing, and
    # every aircraft's ground-start takeoff at a few airports.
    for theater in THEATERS:
        where = ["--theater", theater]
        out.append(probe(f"theater-{theater.lower()}-fight-4v4", where + fight(4, 4, "--separation", "5", *attack)))
        out.append(probe(f"theater-{theater.lower()}-fight-8v8-noattack", where + fight(8, 8, "--separation", "10"), ticks=9600))
        for airport in ("1", "3"):
            out.append(probe(f"theater-{theater.lower()}-takeoff-a{airport}", where + [
                "--ground-start", airport, "--probe-wing-size", "3", "--maneuver", "takeoff", "--separation", "50"],
                ticks=24000, timeout=1800, check=checker(ground=True, need_takeoff=True)))
        out.append(probe(f"theater-{theater.lower()}-land-pair", where + [
            "--ground-start", "1", "--probe-wing-size", "2", "--maneuver", "takeoff",
            "--probe-wing-order", "12000:land-selected", "--separation", "200", "--probe-wing-only"],
            ticks=108000, timeout=2400, check=checker(ground=True, need_takeoff=True, need_landing=True)))
    for theater, airport in (("UKR", "1"), ("PGU", "2"), ("FRA", "3"), ("NSK", "5")):
        for aircraft in AIRCRAFT:
            out.append(probe(f"takeoff-{theater.lower()}-a{airport}-{aircraft}", [
                "--theater", theater, "--ground-start", airport, "--aircraft", aircraft,
                "--probe-friendly-aircraft", aircraft, "--probe-wing-size", "2", "--maneuver", "takeoff",
                "--separation", "100", "--probe-wing-only"],
                ticks=18000, timeout=1800, check=checker(ground=True, need_takeoff=True)))
    for theater in ("PGU", "VLA"):
        out.append(probe(f"long-15v15-{theater.lower()}", ["--theater", theater, *fight(15, 15, "--separation", "20", *attack)],
                         ticks=216000, timeout=3600, check=checker(allow_anomalies=("outside the world",))))

    # 14. The fixed acceptance probes.
    out.append(Scenario(name="ai-roster-probe", lane="ai", args=["--ai-roster-probe-ticks", "3600", "--no-audio"], timeout=1800))
    out.append(Scenario(name="ai-probe-matrix", lane="ai",
                        args=["--probe-matrix", "{work}/matrix", "--ai-probe-ticks", "360", "--separation", "1", "--no-audio"],
                        timeout=3600, check=matrix_problems))
    return out


def matrix_problems(output: str) -> list[str]:
    problems = []
    cases = len(re.findall(r"^AI probe matrix: ", output, re.M))
    if cases < 1008:
        problems.append(f"probe matrix ran {cases} of 1008 cases")
    for line in ANOMALY.findall(output):
        problems.append(f"anomaly: {line[:200]}")
    return problems[:40]
