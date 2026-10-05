"""Scenarios for converting a networked flight's capture into a replay.

A bot flies a real `tore-server` for 25 seconds and keeps its capture
(`_replay_net_run.py capture`); the game converts it with
`--convert-capture`; the replay is read back by the exports like any other
recording; converting twice gives the same bytes; a capture cut short still
converts and says so. The windowless half of the work: no scenario here opens
a window. See docs/testing/lane-replay.md.

The `net` lane (docs/testing/lane-net.md), when it exists, should take this
scenario, since it starts a server and a bot as processes of their own.
"""
from __future__ import annotations

import sys
from pathlib import Path

from battery import Scenario, Step

ROOT = Path(__file__).resolve().parents[2]
PY = sys.executable
RUN = str(ROOT / "tools" / "battery_scenarios" / "_replay_net_run.py")
CAPTURE = "{work}/replays/2026-10-05_1500_NET_127001.tore-capture"
REPLAY = "{work}/replays/2026-10-05_1500_UKR_F18.tore-replay"


def check(output: str) -> list[str]:
    problems: list[str] = []
    stats = output.count("net.stats")
    if stats < 1:
        problems.append("the replay holds no net.stats events")
    for line in output.splitlines():
        if line.startswith("Length ") and "no frames" in line:
            problems.append("the converted replay has no frames")
    return problems


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="replay-net-convert-capture",
            lane="replay",
            args=["--version"],
            timeout=420,
            then=[
                Step([PY, RUN, "capture", "{work}"], app=False, timeout=240),
                Step(["--convert-capture", CAPTURE], timeout=120),
                Step(["--recording-info", REPLAY], timeout=60),
                Step(["--recording-log", REPLAY, "--out", "{work}/log"], timeout=60),
                Step(["--recording-acmi", REPLAY, "--out", "{work}/net.txt.acmi"], timeout=60),
                Step(["--convert-capture", CAPTURE, "--out", "{work}/a.tore-replay"], timeout=120),
                Step(["--convert-capture", CAPTURE, "--out", "{work}/b.tore-replay"], timeout=120),
                Step([PY, RUN, "same", "{work}/a.tore-replay", "{work}/b.tore-replay"], app=False),
                Step([PY, RUN, "cut", CAPTURE, "{work}/cut.tore-capture", "60"], app=False),
                Step(["--convert-capture", "{work}/cut.tore-capture", "--out", "{work}/cut.tore-replay"], timeout=120),
                Step(["--recording-info", "{work}/cut.tore-replay"], timeout=60),
            ],
            expect=[
                r"capture written: \d+ bytes",
                r"Replay: .*_UKR_F18\.tore-replay \(\d+ frames, \d+\.\d s, 4 aircraft\)",
                r"State +finished normally",
                r"Mission +Network flight",
                r"Setting +net\.callsign = Viper",
                r"Aircraft +0 +You +F/A-18D",
                r"\d+ net\.stats",
                r"Recording log: ",
                r"Tacview file: ",
                r"identical: yes",
                r"is cut short: converted up to its last whole record",
                r"Result +end=cut, .*capture=cut short at byte",
            ],
            forbid=[r"INCOMPLETE", r"^Problem", r"identical: NO"],
            check=check,
            outputs=["net.txt.acmi", "log/summary.txt", "log/log.jsonl"],
        ),
    ]
