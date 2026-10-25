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
            problems.append("released wrong-class devices")
    else:
        problems.append("missing study runs.csv")
    for line in output.splitlines():
        if "activity=Defending" not in line:
            continue
        bank = re.search(r" bank=(-?[\d.]+)", line)
        if bank and abs(float(bank[1])) > 90.0:
            problems.append("defensive bank exceeded 90 degrees")
            break
    if re.search(r"maneuver: Break,.*\}\), (?:None|Some\(BurstRequest \{.*?\}\)), None, true", output):
        problems.append("last-ditch break continued without a closing estimate")
    if re.search(r"^missile-study: device .*defense=Some\(\(.*true\)\)$", output, re.M):
        problems.append("device released after the threat passed")
    if runs.exists() and any(int(row["devices"]) + int(row["other_devices"]) > 8 for row in rows):
        problems.append("single-threat defense exceeded its eight-device budget")
    return problems


def head_break_problems(work: Path, output: str) -> list[str]:
    problems = defense_problems(work, output, minimum_turn=5)
    first = re.search(r"^missile-study: trace .*? yaw=(-?[\d.]+).*?activity=Defending defense=Some\(\(Some\(MotionSuggestion \{ maneuver: Break, heading_deg: (-?[\d.e+]+)", output, re.M)
    if first is None:
        problems.append("no head-on last-ditch break observed")
    elif abs((float(first[2]) - float(first[1]) + 180) % 360 - 180) < 75:
        problems.append("head-on break straightened toward the missile")
    return problems


def chaff_timing_problems(work: Path, output: str) -> list[str]:
    problems = defense_problems(work, output, minimum_turn=5)
    runs = work / "study/runs.csv"
    if not runs.exists():
        return problems
    with runs.open() as source:
        rows = list(csv.DictReader(source))
    if not rows or not rows[0]["first_device_s"]:
        return problems + ["no chaff release observed"]
    first_device = float(rows[0]["first_device_s"])
    samples = []
    for line in output.splitlines():
        match = re.search(r" t=([\d.]+).*maneuver: (\w+)", line)
        if match:
            samples.append((float(match[1]), match[2]))
    after_release = next((phase for at, phase in samples if first_device <= at <= first_device + 0.26), None)
    if after_release != "Notch":
        problems.append("first chaff waited for the last-ditch break")
    if "reduce_heat: true" in output:
        problems.append("radar threat requested an IR heat cut")
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
    for model in ("standard", "hybrid"):
        turns.append(Scenario(
            name=f"ai-regress-missile-bank-low-{model}", lane="ai",
            args=["--missile-study", "{work}/study", "--ai", "--trace", "--skill", "ace",
                  "--ai-model", model, "--missiles", "AIM120", "--ranges", "medium",
                  "--aspects", "tail", "--altitudes", "down", "--seeds", "1", "--threads", "1"],
            timeout=180, outputs=["study/runs.csv", "study/cells.csv"],
            check_work=defense_problems,
            notes="Radar warning ensures the 3,000 ft target defends; the overhead IR shot is not seen. Check upright bank and the end of the break.",
        ))
    for skill in ("novice", "average"):
        turns.append(Scenario(
            name=f"ai-regress-missile-chaff-{skill}", lane="ai",
            args=["--missile-study", "{work}/study", "--ai", "--trace", "--skill", skill,
                  "--missiles", "AIM120", "--ranges", "medium", "--aspects", "tail",
                  "--altitudes", "co", "--seeds", "1", "--threads", "1"],
            timeout=180, outputs=["study/runs.csv", "study/cells.csv"],
            check_work=chaff_timing_problems,
            notes="Chaff starts in the notch phase for lower skills; no IR throttle cut for a radar threat.",
        ))
    return turns
