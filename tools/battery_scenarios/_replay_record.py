"""Recording scenarios: probes recorded, read back every way, damaged files rejected.

Each recording scenario runs one AI probe with `--record-mission` and
`--verify-render`, then reads the recording back with `--recording-info`,
`--recording-log`, `--recording-acmi` and `--recording-diff`, and checks every
file it wrote. Scenarios that ask for a repeat also run the probe twice more
(once recorded, once not) to prove that identical runs record identically and
that recording changes nothing the probe prints.
"""
from __future__ import annotations

import json
import re
from pathlib import Path

from battery import Scenario, Step
from battery_scenarios import _replay_checks as rc

TOOLS = "tools/battery_scenarios/_replay_tools.py"
PY = "python3"
ROOT = Path(__file__).resolve().parents[2]

# The imported aircraft by command-line name.
AIRCRAFT = ["f18", "rafale", "f14", "a4e", "x31", "mig29", "su27", "mig21", "su25", "mig23", "su35", "f22", "f22n", "faxx"]


def sections(output: str) -> dict[int, str]:
    """Splits a scenario's output into the main run (0) and each follow-up step."""
    parts = re.split(r"\n\$ then (\d+): [^\n]*\n", output)
    found = {0: parts[0]}
    for i in range(1, len(parts) - 1, 2):
        found[int(parts[i])] = parts[i + 1]
    return found


_NOISE = re.compile(r"^(\d{10}\.\d+Z|Session log|Stage:|T\.O\.R\.E -|T\.O\.R\.E-Fighters )|verify-render|^Recording|^Tacview")


def normalise(text: str) -> list[str]:
    return [line for line in text.splitlines() if line.strip() and not _NOISE.search(line)]


def semantic_log_problems(text: str) -> list[str]:
    """Impossible states in the state samples and comms events of log.jsonl."""
    problems: list[str] = []
    world = None
    dead_at: dict[int, float] = {}
    delivered: dict[tuple, float] = {}
    for line in text.splitlines():
        d = json.loads(line)
        kind = d["type"]
        if kind == "header":
            world = d.get("world", {})
        elif kind == "sample":
            flags = d.get("flags", [])
            hp, max_hp = d["hp"], d["max_hp"]
            who = f"aircraft {d['id']} at {d['t']}s"
            if "alive" in flags and hp <= 0:
                problems.append(f"{who} is alive with {hp} hit points")
            if "alive" not in flags and hp > 0:
                problems.append(f"{who} is dead with {hp} hit points")
            if hp > max_hp:
                problems.append(f"{who} has more hit points than its maximum")
            if d["fuel_lb"] < 0:
                problems.append(f"{who} has negative fuel")
            x, y, z = d["pos_ft"]
            if world and world.get("extent_ft"):
                ex, ez = world["extent_ft"]
                if not (0 <= x <= ex and 0 <= z <= ez):
                    problems.append(f"{who} is outside the world ({x:.0f}, {z:.0f})")
            if abs(d["g"]) > 20:
                problems.append(f"{who} shows {d['g']} G")
        elif kind == "event":
            k = d["kind"]
            f = d.get("fields", {})
            if k in ("combat.destroyed", "aircraft.crashed", "aircraft.ejected") and d.get("subject") is not None:
                dead_at.setdefault(d["subject"], d["t"])
            if k in ("comms.radio", "comms.tower", "comms.hud") and f.get("outcome") == "delivered":
                text_ = d.get("text", "")
                if not text_.strip() or (k != "comms.hud" and not f.get("speaker")):
                    problems.append(f"empty delivered line at {d['t']}s: {f}")
                key = (f.get("speaker"), text_)
                last = delivered.get(key)
                if last is not None and d["t"] - last < 0.2 and k != "comms.hud":
                    problems.append(f"{f.get('speaker')} said '{text_}' twice within 0.2 s at {d['t']}s")
                delivered[key] = d["t"]
                s = d.get("subject")
                if s in dead_at and d["t"] > dead_at[s] + 0.5 and k == "comms.radio":
                    problems.append(f"{f.get('speaker')} spoke at {d['t']}s after aircraft {s} was lost at {dead_at[s]}s")
        if len(problems) > 20:
            problems.append("... more problems suppressed")
            break
    return problems


def invariant_problems(text: str) -> list[str]:
    """Event-stream invariants: order, ownership, one outcome per shot, decoy counts, no revivals."""
    problems: list[str] = []
    lines = [json.loads(line) for line in text.splitlines()]
    events = [e for e in lines if e["type"] == "event"]
    ids = {a["id"] for a in lines if a["type"] == "aircraft"}
    last = -1
    for e in events:
        if e["tick"] < last:
            problems.append(f"events run backwards at tick {e['tick']} (after {last})")
            break
        last = e["tick"]
    footer = [e for e in lines if e["type"] == "footer"]
    if footer and any(e["tick"] > footer[0]["end_tick"] for e in events):
        problems.append("an event is later than the recording's end")
    for e in events:
        s = e.get("subject")
        if s is not None and s < 1000 and s not in ids:
            problems.append(f"{e['kind']} names unknown aircraft {s}")
            break
    dead: dict[int, float] = {}
    for e in events:
        if e["kind"] in ("combat.destroyed", "aircraft.crashed") and e.get("subject") is not None:
            dead.setdefault(e["subject"], e["t"])
    launched = set()
    outcomes: dict[int, int] = {}
    left: dict[tuple, int] = {}
    for e in events:
        f = e.get("fields", {})
        if e["kind"] == "weapon.launch":
            launched.add(f["projectile"])
            if e["subject"] in dead and e["t"] > dead[e["subject"]] + 0.01:
                problems.append(f"aircraft {e['subject']} fired at {e['t']}s after it was lost at {dead[e['subject']]}s")
        elif e["kind"] == "weapon.outcome":
            outcomes[f["projectile"]] = outcomes.get(f["projectile"], 0) + 1
        elif e["kind"] == "combat.countermeasure" and f.get("left") is not None:
            key = (e.get("subject"), f.get("decoy"))
            if f["left"] < 0 or f["left"] > left.get(key, 10**9):
                problems.append(f"aircraft {key[0]} {key[1]} count went to {f['left']} at {e['t']}s")
            left[key] = f["left"]
    for shot, count in outcomes.items():
        if count > 1:
            problems.append(f"shot {shot} has {count} outcomes")
        if shot not in launched:
            problems.append(f"shot {shot} has an outcome but no launch")
    revived: set[int] = set()
    zero: set[int] = set()
    for e in lines:
        if e["type"] == "sample":
            if e["hp"] <= 0:
                zero.add(e["id"])
            elif e["id"] in zero and e["id"] not in revived:
                revived.add(e["id"])
                problems.append(f"aircraft {e['id']} came back to life at {e['t']}s")
    return problems[:20]


def recorded_probe_checks(work: Path, output: str, *, same_run: bool) -> list[str]:
    """Everything a recording of a probe run should satisfy, given the standard steps."""
    problems: list[str] = []
    s = sections(output)
    main = s[0]
    m = re.search(r"AI probe verify-render: (PASS|FAIL)[^\n]*", main)
    if not m:
        problems.append("no verify-render line")
    elif m.group(1) != "PASS" or "missing=0 differing=0" not in m.group(0):
        problems.append(f"verify-render did not pass cleanly: {m.group(0)}")
    info = s.get(1, "")
    if "State       finished normally" not in info:
        problems.append("recording-info does not say the recording finished normally")
    for bad in ("Problem ", "did not finish", "unknown:"):
        if bad in info:
            problems.append(f"recording-info reports: {bad}")
    n_info = len(re.findall(r"^Aircraft +\d+ ", info, re.M))
    actors = re.search(r"AI probe: aircraft=\S+ actors=(\d+)", main)
    if actors and n_info != int(actors.group(1)) + 1:
        problems.append(f"recording lists {n_info} aircraft but the probe had {actors.group(1)} actors plus the player")
    outcome = re.search(r"AI probe debrief: (SUCCESS|FAILURE)", main)
    result = re.search(r"^Result .*outcome=(\w+)", info, re.M)
    if outcome and result and outcome.group(1).lower() != result.group(1):
        problems.append(f"probe debrief says {outcome.group(1)} but the recording's result says {result.group(1)}")
    problems += rc.file_problems(work, "log/log.jsonl", rc.check_jsonl)
    problems += rc.file_problems(work, "log/summary.txt", lambda t: rc.check_summary(t, expect_aircraft=n_info or None))
    problems += rc.file_problems(work, "a.acmi", rc.check_acmi)
    log = work / "log" / "log.jsonl"
    if log.exists():
        try:
            problems += semantic_log_problems(log.read_text())
            problems += invariant_problems(log.read_text())
        except (ValueError, KeyError) as e:
            problems.append(f"log.jsonl could not be read for state checks: {e!r}")
    if log.exists():
        problems += rc.info_vs_log(info, log.read_text())
    acmi = work / "a.acmi"
    if acmi.exists() and log.exists():
        problems += rc.acmi_vs_log(acmi.read_text(), log.read_text())
    if acmi.exists() and n_info:
        ids = set(re.findall(r"^(1[0-9a-f]{10}),T=", acmi.read_text(), re.M))
        if len(ids) != n_info:
            problems.append(f"Tacview file has {len(ids)} aircraft objects, the recording has {n_info}")
    if "Verdict: the recordings match" not in s.get(4, ""):
        problems.append("recording-diff of a recording against itself does not say they match")
    if same_run:
        if "Verdict: the recordings match" not in s.get(6, ""):
            problems.append("two identical runs made recordings that differ")
        if normalise(s.get(5, "")) != normalise(main):
            problems.append("the same probe run twice printed different output")
        bare, recorded = normalise(s.get(7, "")), normalise(main)
        if bare != recorded:
            first = next((i for i, (x, y) in enumerate(zip(bare, recorded)) if x != y), min(len(bare), len(recorded)))
            problems.append(f"recording changed the probe's output (first difference at line {first})")
    return problems


def record_scenario(
    name: str,
    probe: list[str],
    *,
    timeout: float = 240.0,
    same_run: bool = True,
    expect: list[str] | None = None,
    extra_check=None,
    notes: str = "",
) -> Scenario:
    """One probe recorded and read back; with `same_run`, run twice more to prove determinism."""
    steps = [
        Step(["--recording-info", "{work}/a.tore-replay"]),
        Step(["--recording-log", "{work}/a.tore-replay", "--out", "{work}/log", "--rate", "5"]),
        Step(["--recording-acmi", "{work}/a.tore-replay", "--out", "{work}/a.acmi", "--guns"]),
        Step(["--recording-diff", "{work}/a.tore-replay", "{work}/a.tore-replay"]),
    ]
    if same_run:
        steps += [
            Step([*probe, "--record-mission", "{work}/b.tore-replay", "--verify-render", "--no-audio"], timeout=timeout),
            Step(["--recording-diff", "{work}/a.tore-replay", "{work}/b.tore-replay"]),
            Step([*probe, "--no-audio"], timeout=timeout),
        ]

    def check(work: Path, output: str) -> list[str]:
        problems = recorded_probe_checks(work, output, same_run=same_run)
        if extra_check:
            problems += extra_check(work, output)
        return problems

    return Scenario(
        name=name,
        lane="replay",
        args=[*probe, "--record-mission", "{work}/a.tore-replay", "--verify-render", "--no-audio"],
        timeout=timeout,
        expect=[r"AI probe totals:", *(expect or [])],
        outputs=["a.tore-replay"],
        then=steps,
        check_work=check,
        notes=notes,
    )


def scenarios() -> list[Scenario]:
    out: list[Scenario] = []

    # AI fights of every size.
    for f, e in [(1, 1), (2, 2), (3, 3), (5, 5), (8, 8), (15, 15), (1, 15), (15, 1), (4, 9)]:
        out.append(
            record_scenario(
                f"replay-rec-fight-{f}v{e}",
                ["--ai-probe-ticks", "2400", "--probe-fight", f"{f}:{e}", "--separation", "5"],
                same_run=f + e <= 16,
                timeout=500,
            )
        )

    # Every imported aircraft as the enemy and as the player.
    for ac in AIRCRAFT:
        out.append(
            record_scenario(
                f"replay-rec-enemy-{ac}",
                ["--ai-probe-ticks", "2400", "--probe-enemy-aircraft", ac, "--separation", "2", "--probe-attack", "300:10"],
                same_run=False,
            )
        )
        out.append(
            record_scenario(
                f"replay-rec-player-{ac}",
                ["--ai-probe-ticks", "2400", "--aircraft", ac, "--separation", "2", "--probe-attack", "300:10", "--probe-flight-model", "researched"],
                same_run=False,
            )
        )

    # Skills, geometries and missions.
    for skill in ("novice", "average", "experienced", "ace"):
        for geometry in ("head", "side", "rear"):
            out.append(
                record_scenario(
                    f"replay-rec-skill-{skill}-{geometry}",
                    ["--ai-probe-ticks", "2400", "--probe-enemy-skill", skill, "--probe-geometry", geometry, "--separation", "1", "--probe-attack", "300:10"],
                    same_run=False,
                )
            )
    for mission in ("free", "cap", "intercept", "escort", "self-defense", "hold"):
        out.append(
            record_scenario(
                f"replay-rec-mission-{mission}",
                ["--ai-probe-ticks", "3000", "--ai-mission", mission, "--separation", "5", "--probe-fight", "3:3"],
                same_run=False,
            )
        )

    # Guns, missiles, chaff and flares, threats, faults.
    out.append(record_scenario("replay-rec-guns", ["--ai-probe-ticks", "3600", "--separation", "2", "--probe-guns", "--probe-ai-guns-only", "--probe-attack", "300:5"]))
    out.append(record_scenario("replay-rec-missiles", ["--ai-probe-ticks", "7200", "--separation", "5", "--probe-attack", "600:10"], timeout=400))
    for kind in ("hit", "gun", "aaa"):
        out.append(
            record_scenario(
                f"replay-rec-threat-{kind}",
                ["--ai-probe-ticks", "1800", "--probe-enemy-aircraft", "su27", "--probe-geometry", "rear", "--separation", "1", "--ai-mission", "hold", "--probe-threat", f"120:{kind}", "--probe-flight-model", "researched"],
                same_run=False,
            )
        )
    for fault in (0, 4, 7, 11, 12, 29, 30, 34, 44):
        out.append(record_scenario(f"replay-rec-fault-{fault}", ["--ai-probe-ticks", "1800", "--separation", "1", "--probe-fault", f"100:{fault}"], same_run=False))

    # Ground starts, takeoffs, wings, orders.
    for airport in (1, 2, 5):
        for wing in (1, 3, 5):
            out.append(
                record_scenario(
                    f"replay-rec-ground-{airport}-wing{wing}",
                    ["--ai-probe-ticks", "6000", "--ground-start", str(airport), "--maneuver", "takeoff", "--probe-wing-size", str(wing), "--probe-wing-only"],
                    same_run=False,
                    timeout=300,
                )
            )
    for order in ("bug-out", "attack-on-contact", "engage-my-target", "land-selected"):
        out.append(
            record_scenario(
                f"replay-rec-order-{order}",
                ["--ai-probe-ticks", "9000", "--ground-start", "1", "--maneuver", "takeoff", "--probe-wing-size", "3", "--probe-wing-order", f"3000:{order}"],
                same_run=False,
                timeout=400,
            )
        )

    out += failure_scenarios()
    return out


def check_rejected(output: str, message: str) -> list[str]:
    problems = []
    s = sections(output)
    for i in (2, 3, 4):
        text = s.get(i, "")
        if message not in text:
            problems.append(f"step {i}: expected the message '{message}', got: {text.strip()[-160:]}")
        if "panicked" in text:
            problems.append(f"step {i}: panic")
    return problems


def check_soft_damage(work: Path, output: str) -> list[str]:
    """Damaged but readable files: a warning or a clear message, never a panic or a bare error."""
    problems = []
    s = sections(output)
    if "Problem " not in s.get(2, ""):
        problems.append("recording-info does not report a problem with a damaged file")
    for i in (2, 3, 4, 5):
        text = s.get(i, "")
        if "panicked" in text:
            problems.append(f"step {i}: panic")
        if not text.strip():
            problems.append(f"step {i}: no output at all")
    return problems


def killed_probe_scenario() -> Scenario:
    """A recording whose writer is killed mid-flight must still read back as incomplete."""
    binary = str(ROOT / "target" / "debug" / "tore-app")
    return Scenario(
        name="replay-rec-killed-probe",
        lane="replay",
        args=["--version"],
        then=[
            Step(
                [PY, TOOLS, "killrun", "6", binary, "--ai-probe-ticks", "90000", "--separation", "5", "--record-mission", "{work}/k.tore-replay", "--no-audio"],
                app=False,
                timeout=60,
            ),
            Step(["--recording-info", "{work}/k.tore-replay.partial"]),
            Step(["--recording-log", "{work}/k.tore-replay.partial", "--out", "{work}/log", "--rate", "2"]),
            Step(["--recording-acmi", "{work}/k.tore-replay.partial", "--out", "{work}/k.acmi"]),
        ],
        check_work=check_killed,
    )


def check_killed(work: Path, output: str) -> list[str]:
    s = sections(output)
    problems = []
    if not (work / "k.tore-replay.partial").exists():
        problems.append("no .partial recording was left")
    if (work / "k.tore-replay").exists():
        problems.append("a killed run left a finished-looking recording")
    if "INCOMPLETE" not in s.get(2, ""):
        problems.append("recording-info does not call it incomplete")
    if "Length      0:" not in s.get(2, ""):
        problems.append("the partial recording has no length")
    problems += rc.file_problems(work, "log/log.jsonl", rc.check_jsonl)
    problems += rc.file_problems(work, "k.acmi", rc.check_acmi)
    return problems


def failure_scenarios() -> list[Scenario]:
    out: list[Scenario] = [killed_probe_scenario()]
    make = ["--ai-probe-ticks", "1200", "--separation", "2", "--record-mission", "{work}/src.tore-replay", "--no-audio"]

    out.append(
        Scenario(
            name="replay-rec-refuses-existing-path",
            lane="replay",
            args=[*make[:-3], "--record-mission", "{work}/a.tore-replay", "--no-audio"],
            outputs=["a.tore-replay"],
            then=[Step([*make[:-3], "--record-mission", "{work}/a.tore-replay", "--no-audio"], expect_exit=1)],
            check_work=lambda work, output: (
                [] if "already exists" in sections(output).get(1, "") else ["no clear 'already exists' message"]
            )
            + ([] if (work / "a.tore-replay").stat().st_size > 1000 else ["existing recording was damaged"]),
        )
    )

    hard = {
        "empty": "too short",
        "head100": "header",
        "text": "damaged recording",
        "random": "not a T.O.R.E recording",
        "bad-magic": "not a T.O.R.E recording",
        "bad-version": "unsupported recording",
    }
    for kind, message in hard.items():
        steps = [
            Step([PY, TOOLS, "mangle", kind, "{work}/src.tore-replay", "{work}/bad.tore-replay"], app=False),
            Step(["--recording-info", "{work}/bad.tore-replay"], expect_exit=1),
            Step(["--recording-log", "{work}/bad.tore-replay", "--out", "{work}/o-log"], expect_exit=1),
            Step(["--recording-acmi", "{work}/bad.tore-replay", "--out", "{work}/o.acmi"], expect_exit=1),
        ]
        out.append(
            Scenario(
                name=f"replay-corrupt-{kind}",
                lane="replay",
                args=make,
                then=steps,
                check_work=lambda work, output, message=message: check_rejected(output, message),
            )
        )
    for kind in ("trunc-third", "trunc-half", "no-tail", "no-last-byte", "flip", "zero-fill", "duplicate-tail"):
        steps = [
            Step([PY, TOOLS, "mangle", kind, "{work}/src.tore-replay", "{work}/bad.tore-replay"], app=False),
            Step(["--recording-info", "{work}/bad.tore-replay"], expect_exit=None),
            Step(["--recording-log", "{work}/bad.tore-replay", "--out", "{work}/o-log"], expect_exit=None),
            Step(["--recording-acmi", "{work}/bad.tore-replay", "--out", "{work}/o.acmi"], expect_exit=None),
            Step(["--recording-diff", "{work}/src.tore-replay", "{work}/bad.tore-replay"], expect_exit=None),
        ]
        out.append(
            Scenario(
                name=f"replay-corrupt-{kind}",
                lane="replay",
                args=make,
                then=steps,
                check_work=check_soft_damage,
            )
        )
    return out
