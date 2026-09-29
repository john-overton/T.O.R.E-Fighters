"""Scenarios that press keys in a real window (see _replay_drive.py).

Flights and the replay viewer are driven by key presses sent to their own
window. The checks are that nothing crashes or hangs, the game exits on
Alt+F4, and whatever the flight recorded reads back cleanly.
"""
from __future__ import annotations

import re
from pathlib import Path

from battery import Scenario, Step
from battery_scenarios import _replay_checks as rc
from battery_scenarios import _replay_drive as drive
from battery_scenarios import _replay_tools as tools
from battery_scenarios._replay_record import invariant_problems, sections, semantic_log_problems

PY = "python3"
DRIVER = drive.__file__


def drive_step(keys: str, game: list[str], *, random: str = "", recording: bool = True, timeout: float = 240.0) -> Step:
    args = [PY, DRIVER, "--data", "{work}/data"]
    if keys:
        args += ["--keys", keys]
    if random:
        args += ["--random", random]
    return Step([*args, "--", *game], app=False, window=True, timeout=timeout)


def flight_checks(work: Path, output: str, *, min_bookmarks: int = 0, ended: str = "") -> list[str]:
    problems: list[str] = []
    s = sections(output)
    text = s.get(1, "")
    if "driver: game pid" not in text:
        return [f"the driver never found the game: {text.strip()[-200:]}"]
    for bad in ("panicked at", "did not exit on Alt+F4", "the game exited before"):
        if bad in text:
            problems.append(f"driver reports: {bad}")
    rest = "\n".join(s.get(i, "") for i in (2, 3, 4, 5))
    if "recordings: 1 finished, 0 partial" not in s.get(2, "") and "recordings:" in s.get(2, ""):
        problems.append(f"recordings folder: {s.get(2, '').strip().splitlines()[0]}")
    info = s.get(3, "")
    if "State       finished normally" not in info:
        problems.append("the recording does not say it finished normally")
    if "Problem " in info:
        problems.append("recording-info reports a problem")
    found = re.search(r"(\d+) player\.bookmark", info)
    marks = int(found.group(1)) if found else 0
    if marks < min_bookmarks:
        problems.append(f"{marks} bookmarks recorded, expected {min_bookmarks}")
    if ended and f"end={ended}" not in info:
        problems.append(f"expected the recording to end with '{ended}'")
    problems += rc.file_problems(work, "log/log.jsonl", rc.check_jsonl)
    log = work / "log" / "log.jsonl"
    if log.exists():
        body = log.read_text()
        problems += semantic_log_problems(body)
        problems += invariant_problems(body)
        problems += pause_problems(body)
    problems += rc.file_problems(work, "live.acmi", rc.check_acmi)
    return problems


def pause_problems(text: str) -> list[str]:
    """Pauses and resumes must alternate."""
    import json

    problems = []
    paused = False
    for line in text.splitlines():
        d = json.loads(line)
        if d["type"] != "event":
            continue
        if d["kind"] == "system.pause":
            if paused:
                problems.append(f"two pauses in a row at {d['t']}s")
            paused = True
        elif d["kind"] == "system.resume":
            if not paused:
                problems.append(f"a resume without a pause at {d['t']}s")
            paused = False
    return problems


def events_of(work: Path) -> list[dict]:
    import json

    log = work / "log" / "log.jsonl"
    if not log.exists():
        return []
    return [d for d in map(json.loads, log.read_text().splitlines()) if d["type"] == "event"]


def expect_events(*wanted: tuple[str, int, int | None]):
    """A check that the recording holds at least `count` events of each kind, optionally from one aircraft."""

    def check(work: Path, output: str) -> list[str]:
        events = events_of(work)
        problems = []
        for kind, count, subject in wanted:
            found = sum(1 for e in events if e["kind"] == kind and (subject is None or e.get("subject") == subject))
            if found < count:
                problems.append(f"expected at least {count} {kind} events{'' if subject is None else f' from aircraft {subject}'}, found {found}")
        return problems

    return check


def keyed_flight(name: str, game: list[str], *, keys: str = "", random: str = "", min_bookmarks: int = 0, ended: str = "", more=None) -> Scenario:
    return Scenario(
        name=name,
        lane="replay",
        args=["--version"],
        # A window on a hidden workspace loses focus at the first key, which pauses a flight;
        # TORE_PERF_ACTIVE keeps a timing run unpaused, and a large frame count keeps it going
        # until Alt+F4. TORE_RECORD_MISSIONS=1 makes such a run record.
        env={"TORE_RECORD_MISSIONS": "1", "TORE_PERF_FRAMES": "100000", "TORE_PERF_ACTIVE": "1"},
        then=[
            drive_step(keys, game, random=random),
            Step([PY, tools.__file__, "newest", "{work}/data", "{work}/live.tore-replay"], app=False),
            Step(["--recording-info", "{work}/live.tore-replay"]),
            Step(["--recording-log", "{work}/live.tore-replay", "--out", "{work}/log", "--rate", "5"]),
            Step(["--recording-acmi", "{work}/live.tore-replay", "--out", "{work}/live.acmi", "--guns"]),
        ],
        check_work=lambda work, output: flight_checks(work, output, min_bookmarks=min_bookmarks, ended=ended) + (more(work, output) if more else []),
        timeout=120,
    )


def scenarios() -> list[Scenario]:
    out: list[Scenario] = []
    free = ["--free-flight", "--no-audio"]
    out.append(keyed_flight("replay-keys-bookmarks", free, keys="wait 2;ctrl+B;ctrl+P;wait 1;ctrl+B;ctrl+P;wait 2;ctrl+B;ctrl+P;wait 1;ctrl+P;wait 2;ctrl+B;wait 1", min_bookmarks=4))
    out.append(keyed_flight("replay-keys-end-flight", free, keys="wait 3;ctrl+Q;wait 2", ended="end flight"))
    quick_fight = ["--launch-quick-mission", "--separation", "2", "--no-audio"]
    out.append(
        keyed_flight(
            "replay-keys-countermeasures",
            free,
            keys="wait 2;Insert;wait 1;Insert;wait 1;Delete;wait 1;Delete;wait 2",
            more=expect_events(("combat.countermeasure", 4, 0)),
        )
    )
    out.append(
        keyed_flight(
            "replay-keys-designate-and-fire",
            quick_fight,
            keys="wait 2;t;wait 1;bracketright;wait 1;space;wait 1;space;wait 6",
            more=expect_events(("player.command", 1, None)),
        )
    )
    ground = ["--launch-quick-mission", "--ground-start", "1", "--probe-wing-size", "3", "--no-audio"]
    quick = ["--launch-quick-mission", "--separation", "5", "--no-audio"]
    for seed in range(1, 6):
        out.append(keyed_flight(f"replay-keys-fuzz-free-{seed}", free, keys="wait 1", random=f"{seed}:60:flight"))
    for seed in range(11, 14):
        out.append(keyed_flight(f"replay-keys-fuzz-quick-{seed}", quick, keys="wait 1", random=f"{seed}:60:flight"))
    for seed in range(21, 24):
        out.append(keyed_flight(f"replay-keys-fuzz-ground-{seed}", ground, keys="wait 1", random=f"{seed}:60:flight"))
    # Alt+F4 must quit over the controls, graphics and sound screens (it once did nothing there).
    screens = {"controls": "Escape;Right;Return", "graphics": "Escape;Right;Right;Return", "sound": "Escape;Right;Right;Down;Return"}
    for name, path in screens.items():
        out.append(keyed_flight(f"replay-keys-alt-f4-over-{name}-flight", free, keys=f"wait 1;{path};wait 1"))
        out.append(
            Scenario(
                name=f"replay-keys-alt-f4-over-{name}-viewer",
                lane="replay",
                args=["--ai-probe-ticks", "1200", "--separation", "5", "--record-mission", "{work}/rec.tore-replay", "--no-audio"],
                then=[drive_step(f"wait 1;{path};wait 1", ["--watch-replay", "{work}/rec.tore-replay", "--no-audio"])],
                check_work=lambda work, output: viewer_checks(output),
                timeout=240,
            )
        )
    out.append(
        keyed_flight(
            "replay-keys-alt-f4-after-long-sequence",
            free,
            keys="wait 1;F8;m;shift+t;Down;k;ctrl+F2;shift+Up;space;shift+l;Return;F5;m;x;bracketright;shift+m;1;F4;1;space;space;2;shift+Left;shift+m;m;v;shift+Left;comma;Up;Escape;v;shift+Right;ctrl+1;ctrl+P;g;shift+9;j;ctrl+B;Return;Return;semicolon;5;m;Insert;h;3;shift+1;ctrl+shift+i;semicolon;shift+1;shift+5;Up;F8;F5;m;alt+F1;shift+2;F4;F3",
        )
    )
    # The replay viewer.
    for seed in range(1, 7):
        out.append(
            Scenario(
                name=f"replay-keys-fuzz-viewer-{seed}",
                lane="replay",
                args=["--ai-probe-ticks", "3600", "--separation", "5", "--probe-attack", "600:10", "--record-mission", "{work}/rec.tore-replay", "--no-audio"],
                then=[drive_step("", ["--watch-replay", "{work}/rec.tore-replay", "--no-audio"], random=f"{seed}:80:replay")],
                check_work=lambda work, output: viewer_checks(output),
                timeout=240,
            )
        )
    return out


def viewer_checks(output: str) -> list[str]:
    text = sections(output).get(1, "")
    problems = []
    if "driver: game pid" not in text:
        return [f"the driver never found the game: {text.strip()[-200:]}"]
    for bad in ("panicked at", "did not exit on Alt+F4", "the game exited before"):
        if bad in text:
            problems.append(f"driver reports: {bad}")
    return problems
