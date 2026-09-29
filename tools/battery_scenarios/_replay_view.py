"""Playback scenarios: the replay viewer captured at many ticks, views and options.

Each scenario records one headless probe, then captures frames of it with
`--watch-replay --capture-replay` (windowed, through tools/agent-run.sh). Every
frame must be a real picture (not blank or black), must say "Scene capture:",
must not make the viewer fail, and frames that ought to differ must differ.
A person still has to look at the frames for taste; see the lane doc.
"""
from __future__ import annotations

import hashlib
import re
from dataclasses import dataclass, field
from pathlib import Path

from battery import Scenario, Step
from battery_scenarios import _replay_tools as tools

# Headless probes that make the recordings the captures use.
PROBES = {
    "attack": ["--ai-probe-ticks", "3600", "--separation", "5", "--probe-attack", "600:10"],
    "ground": ["--ai-probe-ticks", "3600", "--ground-start", "1", "--maneuver", "takeoff", "--probe-wing-size", "3", "--probe-wing-only"],
    "big": ["--ai-probe-ticks", "1500", "--probe-fight", "8:8", "--separation", "5"],
    "f22": ["--ai-probe-ticks", "2400", "--aircraft", "f22", "--separation", "2", "--probe-attack", "300:10", "--probe-flight-model", "researched"],
}


@dataclass
class Shot:
    name: str
    tick: int
    args: list[str] = field(default_factory=list)
    ui: bool = True  # the timeline strip is expected
    group: str = ""  # shots in one group must all look different
    env: dict[str, str] = field(default_factory=dict)


def capture_scenario(name: str, probe: str, shots: list[Shot], *, timeout: float = 600.0, notes: str = "") -> Scenario:
    steps = []
    for shot in shots:
        steps.append(
            Step(
                [
                    "--watch-replay", "{work}/rec.tore-replay",
                    "--capture-replay", f"{{work}}/{shot.name}.ppm",
                    "--replay-tick", str(shot.tick),
                    "--no-audio",
                    *shot.args,
                ],
                window=True,
                timeout=120,
            )
        )
    by_name = {s.name: s for s in shots}

    def check(work: Path, output: str) -> list[str]:
        problems: list[str] = []
        if output.count("Scene capture:") != len(shots):
            problems.append(f"{output.count('Scene capture:')} captures reported for {len(shots)} shots")
        hashes: dict[str, dict[str, str]] = {}
        for shot in shots:
            f = work / f"{shot.name}.ppm"
            if not f.exists():
                problems.append(f"{shot.name}: no frame written")
                continue
            problems += tools.ppm_problems(str(f), expect_ui=shot.ui)
            if shot.group:
                digest = hashlib.sha1(f.read_bytes()).hexdigest()
                for other, h in hashes.setdefault(shot.group, {}).items():
                    if h == digest:
                        problems.append(f"{shot.name} and {other} are identical frames")
                hashes[shot.group][shot.name] = digest
        if re.search(r"(?i)replay[^\n]*(failed|error)|panicked", output):
            problems.append("the viewer reported a failure")
        return problems

    return Scenario(
        name=name,
        lane="replay",
        args=[*PROBES[probe], "--record-mission", "{work}/rec.tore-replay", "--no-audio"],
        timeout=timeout,
        outputs=["rec.tore-replay"],
        then=steps,
        check_work=check,
        notes=notes,
    )


def scenarios() -> list[Scenario]:
    out: list[Scenario] = []

    # Every flight view, on a fight with missiles, chaff and flares.
    out.append(
        capture_scenario(
            "replay-view-attack-all-views",
            "attack",
            [Shot(f"v{v}", 1000, ["--flight-view", str(v)], group="views" if v in (0, 1, 2, 3, 4, 8, 9, 10) else "") for v in range(12)],
        )
    )
    # Views during a ground start: a wingman exists, the runway is close.
    out.append(
        capture_scenario(
            "replay-view-ground-start",
            "ground",
            [Shot(f"t{t}v{v}", t, ["--flight-view", str(v)]) for t in (0, 400, 1500) for v in (0, 1, 3, 7, 10)],
        )
    )
    # Extreme ticks: first, second, last, one past the end, far past the end.
    out.append(
        capture_scenario(
            "replay-ticks-extremes",
            "attack",
            [Shot(f"tick{t}", t) for t in (0, 1, 2, 119, 120, 121, 3599, 3600, 3601, 99999)],
        )
    )
    out.append(
        capture_scenario(
            "replay-ticks-sweep",
            "attack",
            [Shot(f"tick{t}", t, group="sweep") for t in range(300, 3600, 330)],
        )
    )
    # Following each aircraft, the drone, the object view.
    out.append(
        capture_scenario(
            "replay-aircraft-each",
            "attack",
            [Shot(f"ac{i}", 1200, ["--replay-aircraft", str(i)]) for i in range(5)],
        )
    )
    out.append(
        capture_scenario(
            "replay-aircraft-big-fight",
            "big",
            [Shot(f"ac{i}", 800, ["--replay-aircraft", str(i)]) for i in (0, 1, 8, 9, 15)],
        )
    )
    out.append(
        capture_scenario(
            "replay-drone-and-object",
            "attack",
            [
                Shot("drone", 1200, ["--replay-drone"]),
                Shot("drone-t0", 0, ["--replay-drone"]),
                Shot("drone-end", 3600, ["--replay-drone"]),
                Shot("look-aircraft1", 1200, ["--replay-look-at", "aircraft:1"]),
                Shot("look-aircraft3", 1200, ["--replay-look-at", "aircraft:3"]),
                Shot("look-weapon", 1000, ["--replay-look-at", "weapon:16777216"]),
                Shot("look-missing-aircraft", 1200, ["--replay-look-at", "aircraft:99"]),
                Shot("look-missing-ground", 1200, ["--replay-look-at", "ground:99"]),
            ],
        )
    )
    # Interface parts and panels.
    ui_parts = ["labels", "timer", "trails", "comms", "subtitles"]
    out.append(
        capture_scenario(
            "replay-ui-parts",
            "attack",
            [Shot(f"ui-{p}", 1400, ["--replay-ui", p]) for p in ui_parts]
            + [Shot("ui-all", 1400, ["--replay-ui", ",".join(ui_parts)]), Shot("ui-trails-comms", 2400, ["--replay-ui", "trails,comms"]), Shot("clean", 1400, ["--replay-clean"], ui=False)],
        )
    )
    out.append(
        capture_scenario(
            "replay-panels",
            "attack",
            [Shot(f"panel-{p}", 1400, ["--replay-panels", p]) for p in ("thought", "telemetry", "guidance", "comms", "menu")]
            + [Shot("panel-all", 1400, ["--replay-panels", "thought,telemetry,guidance,comms"]), Shot("panel-thought-ai", 1400, ["--replay-panels", "thought", "--replay-aircraft", "3"])],
        )
    )
    # The Escape menu, every page.
    out.append(
        capture_scenario(
            "replay-menu-pages",
            "attack",
            [Shot(f"menu-{'q' if m == '?' else m}", 1400, ["--replay-menu", m]) for m in ("?", "pref", "time", "help", "graphics", "sound", "controls")],
        )
    )
    # The other recordings.
    out.append(
        capture_scenario(
            "replay-view-big-fight",
            "big",
            [Shot(f"big-t{t}", t) for t in (0, 500, 1000, 1499)] + [Shot("big-labels", 800, ["--replay-ui", "labels,trails"]), Shot("big-comms", 800, ["--replay-ui", "comms,subtitles"])],
        )
    )
    out.append(
        capture_scenario(
            "replay-view-f22",
            "f22",
            [Shot(f"f22-t{t}", t, ["--replay-aircraft", "0"]) for t in (0, 500, 900, 1500)] + [Shot("f22-v0", 900, ["--flight-view", "0"]), Shot("f22-v10", 900, ["--flight-view", "10"])],
        )
    )
    out += speed_scenarios()
    out += flight_panel_scenarios()
    out += model_scenarios()
    out += bad_recording_scenarios()
    return out


def model_scenarios() -> list[Scenario]:
    """Each aircraft as the player and as every enemy, seen from outside in the viewer."""
    from battery_scenarios._replay_record import AIRCRAFT

    out = []
    for ac in AIRCRAFT:
        out.append(
            Scenario(
                name=f"replay-view-model-{ac}",
                lane="replay",
                args=["--ai-probe-ticks", "600", "--aircraft", ac, "--probe-enemy-aircraft", ac, "--separation", "1", "--ai-mission", "hold", "--probe-flight-model", "researched", "--record-mission", "{work}/rec.tore-replay", "--no-audio"],
                then=[
                    Step(["--watch-replay", "{work}/rec.tore-replay", "--capture-replay", "{work}/you.ppm", "--replay-tick", "240", "--replay-aircraft", "0", "--flight-view", "1", "--replay-ui", "labels", "--no-audio"], window=True, timeout=120),
                    Step(["--watch-replay", "{work}/rec.tore-replay", "--capture-replay", "{work}/enemy.ppm", "--replay-tick", "240", "--replay-aircraft", "3", "--flight-view", "1", "--replay-ui", "labels", "--no-audio"], window=True, timeout=120),
                ],
                check_work=lambda work, output: tools.ppm_problems(str(work / "you.ppm")) + tools.ppm_problems(str(work / "enemy.ppm")),
                timeout=300,
            )
        )
    return out


def flight_panel_scenarios() -> list[Scenario]:
    """The debug panels over a live flight (GPU capture): thought, telemetry, guidance, comms, right-click menu."""
    out = []
    combos = ["thought", "telemetry", "guidance", "comms", "menu", "thought,telemetry,guidance,comms,menu"]
    for panels in combos:
        for label, extra in (("air", []), ("ground", ["--ground-start", "1"])):
            if extra and panels == "guidance":
                continue
            name = panels.replace(",", "-")
            out.append(
                Scenario(
                    name=f"replay-flight-panels-{name}-{label}",
                    lane="replay",
                    args=["--launch-quick-mission", *extra, "--flight-panels", panels, "--flight-probe-ticks", "240", "--capture-flight", "{work}/c.ppm", "--no-audio"],
                    window=True,
                    timeout=200,
                    outputs=["c.ppm"],
                    check_work=lambda work, output: tools.ppm_problems(str(work / "c.ppm")),
                )
            )
    return out


def speed_scenarios() -> list[Scenario]:
    out: list[Scenario] = []
    for speed in ("0.125", "1", "4", "16", "-1", "-16"):
        tick = "3000" if speed.startswith("-") else "300"
        out.append(
            Scenario(
                name=f"replay-speed-{speed}",
                lane="replay",
                args=[*PROBES["attack"], "--record-mission", "{work}/rec.tore-replay", "--no-audio"],
                then=[
                    Step(
                        ["--watch-replay", "{work}/rec.tore-replay", "--replay-speed", speed, "--replay-tick", tick, "--no-audio"],
                        window=True,
                        timeout=180,
                    )
                ],
                check_work=lambda work, output: [] if re.search(r"frame interval: mean", output) else ["no frame timing report"],
                env={"TORE_PERF_FRAMES": "150"},
            )
        )
    return out


def bad_recording_scenarios() -> list[Scenario]:
    out: list[Scenario] = []
    kinds = {
        "empty": 1,
        "head100": 1,
        "text": 1,
        "random": 1,
        "bad-magic": 1,
        "bad-version": 1,
        "trunc-third": None,
        "trunc-half": None,
        "no-tail": None,
        "flip": None,
        "zero-fill": None,
        "duplicate-tail": None,
    }
    for kind, code in kinds.items():
        cap = "{work}/c.ppm"
        steps = [
            Step(["python3", tools.__file__, "mangle", kind, "{work}/src.tore-replay", "{work}/bad.tore-replay"], app=False),
            Step(["--watch-replay", "{work}/bad.tore-replay", "--capture-replay", cap, "--replay-tick", "1200", "--no-audio"], window=True, expect_exit=code, timeout=120),
        ]

        def check(work: Path, output: str, code=code) -> list[str]:
            problems = []
            if "panicked" in output:
                problems.append("the viewer panicked on a damaged recording")
            if code == 1 and not re.search(r"damaged recording|unsupported recording|holds no frames|too short", output):
                problems.append("no clear message about the damaged recording")
            frame = work / "c.ppm"
            if code is None and frame.exists():
                problems += tools.ppm_problems(str(frame))
            return problems

        out.append(
            Scenario(
                name=f"replay-watch-corrupt-{kind}",
                lane="replay",
                args=["--ai-probe-ticks", "1500", "--separation", "2", "--probe-attack", "300:5", "--record-mission", "{work}/src.tore-replay", "--no-audio"],
                then=steps,
                check_work=check,
            )
        )
    return out
