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


SETTINGS_FILES = {
    "preferences": ("preferences-v1.conf", "Preferences not loaded"),
    "graphics": ("graphics-v1.conf", "Graphics options not loaded"),
    "sound": ("sound-v1.conf", "Sound settings not loaded"),
    "replays": ("replays-v1.conf", "replays-v1.conf unreadable"),
}


def settings_scenarios() -> list[Scenario]:
    """A damaged settings file must never stop the game: defaults, and a warning in the session log."""
    out = []
    for name, (file, warning) in SETTINGS_FILES.items():
        code = f"import sys; open(sys.argv[1] + '/{file}', 'wb').write(b'\\xff\\xfe garbage \\x00 not a config\\nfoo=bar\\n')"
        out.append(
            Scenario(
                name=f"replay-settings-corrupt-{name}",
                lane="replay",
                args=["--version"],
                then=[
                    Step([PY, "-c", code, "{work}/data"], app=False),
                    Step([PY, DRIVER, "--data", "{work}/data", "--keys", "wait 2", "--", "--free-flight", "--no-audio"], app=False, window=True, timeout=180),
                    Step([PY, "-c", "import glob,sys; t=''.join(open(f,errors='replace').read() for f in sorted(glob.glob(sys.argv[1]+'/logs/*.log'))); print(t)", "{work}/data"], app=False),
                ],
                check_work=lambda work, output, warning=warning: check_settings(output, warning),
                timeout=240,
            )
        )
    code = "import sys; open(sys.argv[1] + '/input-v1.conf', 'wb').write(b'not a profile\\n')"
    out.append(
        Scenario(
            name="replay-settings-corrupt-input",
            lane="replay",
            args=["--version"],
            then=[
                Step([PY, "-c", code, "{work}/data"], app=False),
                Step([PY, DRIVER, "--data", "{work}/data", "--keys", "wait 1", "--", "--free-flight", "--no-audio"], app=False, window=True, expect_exit=None, timeout=180),
            ],
            check_work=lambda work, output: (
                []
                if "input-v1.conf" in sections(output).get(2, "") and "delete it to go back to the default controls" in sections(output).get(2, "") and "panicked" not in output
                else ["a damaged input-v1.conf did not give a message that names the file and says how to recover"]
            ),
            timeout=240,
        )
    )
    return out


def replays_screen_scenario() -> Scenario:
    """The Replays screen on real files: auto-delete on opening, delete asks first, only the chosen file goes."""
    listing = "REPLAYS"

    def check(work: Path, output: str) -> list[str]:
        s = sections(output)
        problems = []
        if "driver: ok" not in s.get(3, ""):
            problems.append(f"the viewer did not run and quit cleanly: {s.get(3, '').strip()[-200:]}")
        after = s.get(4, "")
        names = re.findall(r"'([^']+)'", after)
        expected = [
            "2026-09-10_1200_UKR_F18.tore-replay",  # named like a recording but not one: never touched
            "2026-09-11_1200_UKR_F18.tore-replay",  # marked Keep: survives the rule
            "2026-09-14_1200_UKR_F18.tore-replay",
            "2026-09-15_1200_UKR_F18.tore-replay",
            "2026-09-16_1200_UKR_F18.tore-replay",
            "2026-09-17_1200_UKR_F18.tore-replay",  # 12 and 13 fell to keep-last 5; 18 was deleted by hand
            "notes.txt",  # not a recording: never touched
        ]
        if names != expected:
            problems.append(f"replays folder after the run is {names}, expected {expected}")
        log = s.get(5, "")
        if log.count("Recording auto-deleted") != 2:
            problems.append("expected exactly two auto-deleted recordings in the session log")
        if log.count("Recording deleted:") != 1:
            problems.append("expected exactly one recording deleted by hand")
        return problems

    return Scenario(
        name="replay-screen-auto-delete-and-delete",
        lane="replay",
        args=["--ai-probe-ticks", "300", "--separation", "2", "--record-mission", "{work}/src.tore-replay", "--no-audio"],
        then=[
            Step([PY, tools.__file__, "seed", "{work}/data", "{work}/src.tore-replay"], app=False),
            Step(["--version"]),
            drive_step(
                "wait 2;Escape;wait 1;Return;wait 3;Delete;wait 1;Return;wait 1;Delete;wait 1;Tab;wait 0.5;Return;wait 2",
                ["--watch-replay", "{work}/src.tore-replay", "--no-audio"],
            ),
            Step([PY, tools.__file__, "list", "{work}/data"], app=False),
            Step([PY, "-c", "import glob,sys; print(''.join(open(f,errors='replace').read() for f in sorted(glob.glob(sys.argv[1]+'/logs/*.log'))))", "{work}/data"], app=False),
        ],
        check_work=check,
        timeout=240,
    )


def check_settings(output: str, warning: str) -> list[str]:
    s = sections(output)
    problems = []
    text = s.get(2, "")
    if "driver: game pid" not in text or "driver: ok" not in text:
        problems.append(f"the game did not start and quit cleanly: {text.strip()[-200:]}")
    if "panicked" in output:
        problems.append("panic with a damaged settings file")
    if warning not in s.get(3, ""):
        problems.append(f"the session log has no warning '{warning}'")
    return problems


def scenarios() -> list[Scenario]:
    out: list[Scenario] = settings_scenarios() + [replays_screen_scenario()]
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
    # Alt+F4 quits from every other screen and flight overlay.
    screens_alt = {
        "main-menu": ([], ""),
        "creator": (["--quick-mission"], ""),
        "terrain-viewer": (["--viewer"], ""),
        "flight-help": (["--free-flight"], "F11"),
        "flight-menu": (["--free-flight"], "Escape"),
        "flight-map": (["--free-flight"], "shift+m"),
    }
    for name, (args, key) in screens_alt.items():
        out.append(
            Scenario(
                name=f"replay-keys-alt-f4-on-{name}",
                lane="replay",
                args=["--version"],
                then=[drive_step(f"wait 1;{key};wait 1" if key else "wait 2", [*args, "--no-audio"])],
                check_work=lambda work, output: viewer_checks(output) if "driver: ok" in sections(output).get(1, "") else ["Alt+F4 did not quit"],
                timeout=200,
            )
        )
    # The first-run locate screen, with nothing to detect, quits on Alt+F4 like every other screen.
    out.append(
        Scenario(
            name="replay-keys-alt-f4-on-locate-screen",
            lane="replay",
            args=["--version"],
            then=[
                Step([PY, "-c", "import os,sys; os.makedirs(sys.argv[1])", "{work}/empty"], app=False),
                Step([PY, DRIVER, "--data", "{work}/fresh", "--cwd", "{work}/empty", "--keys", "wait 2", "--", "--no-audio"], app=False, window=True, timeout=180),
            ],
            check_work=lambda work, output: (
                [] if "driver: ok" in sections(output).get(2, "") and "waiting for a choice" in sections(output).get(2, "") else ["the locate screen did not appear and quit on Alt+F4"]
            ),
            timeout=240,
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
