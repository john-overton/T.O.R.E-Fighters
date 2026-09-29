"""Live recordings: flights run in a real window that record themselves, read back afterwards.

`TORE_RECORD_MISSIONS=1` forces recording on for a frame-timing run, so
`TORE_PERF_FRAMES` ends the flight after a fixed number of frames and the
recording is finished the way a quit would finish it. The recording is then
read back with the same commands as a probe recording and checked the same way.
"""
from __future__ import annotations

import re
from pathlib import Path

from battery import Scenario, Step
from battery_scenarios import _replay_checks as rc
from battery_scenarios import _replay_tools as tools
from battery_scenarios._replay_record import AIRCRAFT, invariant_problems, sections, semantic_log_problems

PY = "python3"


def live_checks(work: Path, output: str, *, tower: bool = False, ai: int | None = None) -> list[str]:
    problems: list[str] = []
    s = sections(output)
    if "frame interval: mean" not in s[0]:
        problems.append("the timed flight did not finish")
    if "recordings: 1 finished, 0 partial" not in s.get(1, ""):
        problems.append(f"expected one finished recording and no partial one: {s.get(1, '').strip()[:120]}")
    info = s.get(2, "")
    if "State       finished normally" not in info:
        problems.append("recording-info does not say the recording finished normally")
    if "Problem " in info:
        problems.append("recording-info reports a problem")
    m = re.search(r"Length +(\d+):([\d.]+) \((\d+) frames", info)
    if not m or int(m.group(3)) < 60:
        problems.append("the recording is nearly empty")
    n_info = len(re.findall(r"^Aircraft +\d+ ", info, re.M))
    if ai is not None and n_info < ai:
        problems.append(f"only {n_info} aircraft recorded, expected at least {ai}")
    problems += rc.file_problems(work, "log/log.jsonl", rc.check_jsonl)
    problems += rc.file_problems(work, "log/summary.txt", lambda t: rc.check_summary(t, expect_aircraft=n_info or None))
    problems += rc.file_problems(work, "live.acmi", rc.check_acmi)
    log = work / "log" / "log.jsonl"
    if log.exists():
        try:
            text = log.read_text()
            problems += semantic_log_problems(text)
            problems += invariant_problems(text)
            if tower and '"kind": "comms.tower"' not in text and '"kind":"comms.tower"' not in text:
                problems.append("a ground start produced no tower radio")
            if tower and "Ground start:" not in text:
                problems.append("a ground start showed no start hint")
            if "PageUp adds throttle" in text:
                problems.append("the ground start hint names PageUp, which no longer sets throttle")
        except (ValueError, KeyError) as e:
            problems.append(f"log.jsonl could not be read for state checks: {e!r}")
    if log.exists():
        problems += rc.info_vs_log(info, log.read_text())
    acmi = work / "live.acmi"
    if acmi.exists() and log.exists():
        problems += rc.acmi_vs_log(acmi.read_text(), log.read_text())
    if "Verdict: the recordings match" not in s.get(5, ""):
        problems.append("recording-diff of a recording against itself does not say they match")
    return problems


def live_scenario(name: str, args: list[str], *, frames: int = 240, audio: bool = False, tower: bool = False, ai: int | None = None, extra_env: dict | None = None) -> Scenario:
    steps = [
        Step([PY, tools.__file__, "newest", "{work}/data", "{work}/live.tore-replay"], app=False),
        Step(["--recording-info", "{work}/live.tore-replay"]),
        Step(["--recording-log", "{work}/live.tore-replay", "--out", "{work}/log", "--rate", "5"]),
        Step(["--recording-acmi", "{work}/live.tore-replay", "--out", "{work}/live.acmi", "--guns"]),
        Step(["--recording-diff", "{work}/live.tore-replay", "{work}/live.tore-replay"]),
    ]
    return Scenario(
        name=name,
        lane="replay",
        args=[*args, *([] if audio else ["--no-audio"])],
        window=True,
        timeout=300,
        env={"TORE_RECORD_MISSIONS": "1", "TORE_PERF_FRAMES": str(frames), **(extra_env or {})},
        then=steps,
        check_work=lambda work, output: live_checks(work, output, tower=tower, ai=ai),
    )


def scenarios() -> list[Scenario]:
    out: list[Scenario] = []
    for ac in AIRCRAFT:
        out.append(live_scenario(f"replay-live-free-{ac}", ["--free-flight", "--aircraft", ac, "--researched-flight"], frames=240))
    out.append(live_scenario("replay-live-free-legacy", ["--free-flight", "--legacy-flight"], frames=240))
    for sep in (1, 5, 20):
        out.append(live_scenario(f"replay-live-quick-sep{sep}", ["--launch-quick-mission", "--separation", str(sep)], frames=600, ai=3))
    for airport in (1, 2, 5):
        for wing in (1, 3, 5):
            out.append(
                live_scenario(
                    f"replay-live-ground-{airport}-wing{wing}",
                    ["--launch-quick-mission", "--ground-start", str(airport), "--probe-wing-size", str(wing)],
                    frames=900,
                    tower=True,
                    ai=wing,
                )
            )
    out.append(live_scenario("replay-live-quick-hold", ["--launch-quick-mission", "--ai-mission", "hold"], frames=600, ai=3))
    out.append(
        Scenario(
            name="replay-live-record-off",
            lane="replay",
            args=["--free-flight", "--no-audio"],
            window=True,
            timeout=180,
            env={"TORE_RECORD_MISSIONS": "0", "TORE_PERF_FRAMES": "120"},
            expect=[r"frame interval: mean"],
            then=[Step([PY, tools.__file__, "newest", "{work}/data", "{work}/x.tore-replay"], app=False, expect_exit=1)],
            check_work=lambda work, output: [] if "no finished recording" in output else ["TORE_RECORD_MISSIONS=0 still recorded"],
        )
    )
    out.append(
        Scenario(
            name="replay-live-record-bad-value",
            lane="replay",
            args=["--free-flight", "--no-audio"],
            window=True,
            env={"TORE_RECORD_MISSIONS": "2"},
            expect_exit=1,
            expect=[r"TORE_RECORD_MISSIONS needs 0 or 1"],
        )
    )
    out.append(live_scenario("replay-live-audio-quick", ["--launch-quick-mission", "--separation", "5"], frames=600, audio=True, ai=3))
    out.append(live_scenario("replay-live-audio-ground", ["--launch-quick-mission", "--ground-start", "1", "--probe-wing-size", "3"], frames=900, audio=True, tower=True, ai=3))
    return out
