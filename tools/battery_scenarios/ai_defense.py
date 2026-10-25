"""Missile defense regressions flown through the real AI and flight models."""
from __future__ import annotations

import csv
import re
from pathlib import Path

from battery import Scenario

TRACE = re.compile(r"^missile-study: trace .*? g=(-?[\d.]+).*?activity=Defending defense=Some\(\(Some\(MotionSuggestion \{ maneuver: (\w+), heading_deg: (-?[\d.e+]+)", re.M)


def defense_problems(work: Path, output: str, *, minimum_turn: float = 20) -> list[str]:
    problems = []
    samples = TRACE.findall(output)
    if len(samples) < 4:
        problems.append("too few defensive flight samples")
    if any(float(g) <= 0 for g, _, _ in samples):
        problems.append("defense flew non-positive G")
    for (_, phase, heading), (_, next_phase, next_heading) in zip(samples, samples[1:]):
        delta = abs((float(next_heading) - float(heading) + 180) % 360 - 180)
        if phase == next_phase and phase in ("Beam", "Notch") and delta > 120:
            problems.append("beam/notch side flipped")
    runs = work / "study" / "runs.csv"
    if runs.exists():
        with runs.open() as source:
            rows = list(csv.DictReader(source))
        if not rows or any(row["fired"] != "1" for row in rows):
            problems.append("study did not fire every shot")
        if any(float(row["target_turn_deg"]) < minimum_turn for row in rows):
            problems.append(f"defensive heading change below {minimum_turn:g} degrees")
        if any(int(row["other_devices"]) for row in rows):
            problems.append("released wrong-class devices for the IR threat")
    else:
        problems.append("missing study runs.csv")
    return problems


def head_break_problems(work: Path, output: str) -> list[str]:
    problems = defense_problems(work, output, minimum_turn=5)
    first = re.search(r"^missile-study: trace .*? yaw=(-?[\d.]+).*?activity=Defending defense=Some\(\(Some\(MotionSuggestion \{ maneuver: Break, heading_deg: (-?[\d.e+]+)", output, re.M)
    if first is None:
        problems.append("no head-on last-ditch break observed")
    elif abs((float(first[2]) - float(first[1]) + 180) % 360 - 180) < 75:
        problems.append("head-on break straightened toward the missile")
    return problems


def scenarios() -> list[Scenario]:
    turns = [Scenario(
        name=f"ai-regress-missile-positive-g-{model}", lane="ai",
        args=["--missile-study", "{work}/study", "--ai", "--trace", "--skill", "ace",
              "--ai-model", model, "--missiles", "AIM9M", "--ranges", "medium",
              "--aspects", "tail", "--altitudes", "co", "--seeds", "1", "--threads", "1"],
        timeout=180, outputs=["study/runs.csv", "study/cells.csv"],
        check_work=defense_problems,
        notes="Committed defensive turns keep positive G and dispense only flares against a visually observed IR missile.",
    ) for model in ("standard", "hybrid")]

    turns.append(Scenario(
        name="ai-regress-missile-head-on-break", lane="ai",
        args=["--missile-study", "{work}/study", "--ai", "--trace", "--skill", "ace",
              "--missiles", "AIM9M", "--ranges", "medium", "--aspects", "head",
              "--altitudes", "co", "--seeds", "1", "--threads", "1"],
        timeout=180, outputs=["study/runs.csv", "study/cells.csv"],
        check_work=head_break_problems,
        notes="A missile directly ahead prompts a hard crossing break, not a straightening command.",
    ))
    return turns
