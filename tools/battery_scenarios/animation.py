"""CPU geometry regressions for individually reviewed aircraft animation rigs.

This lane checks actual render-path geometry, without creating a window.
Source artifacts stay in the battery's ignored per-scenario output directory.
"""
import json
from pathlib import Path

from battery import Scenario

# Expand only after independent source attachments and pose sheets are reviewed.
REVIEWED_AIRCRAFT = ("a7", "f4b", "f4j", "f4e", "f4g", "f15", "mig17", "f16c", "f104", "a310")


def check_report(work: Path, _output: str) -> list[str]:
    try:
        report = json.loads((work / "poses" / "report.json").read_text())
    except (OSError, ValueError) as error:
        return [f"unreadable animation report: {error}"]
    problems = []
    if report.get("validation_scope") != "reviewed-controls-and-attachments":
        problems.append("animation regressed to an unreviewed motion survey")
    controls = report.get("controls", [])
    names = {control.get("control") for control in controls}
    if not {"elevator", "aileron", "rudder", "flaps", "gear"} <= names:
        problems.append("animation report omitted primary controls")
    for control in controls:
        if not control.get("checks_passed"):
            problems.append(f"animation checks failed: {control.get('control')}")
    return problems


def scenarios() -> list[Scenario]:
    return [Scenario(
        name=f"flight-animation-{aircraft}", lane="flight",
        args=["--aircraft", aircraft, "--animation-probe", "{work}/poses",
              "--no-audio", "--no-controllers"],
        expect=[r"0 required/check failures"],
        outputs=["poses/report.json", "poses/index.txt", "poses/gear.ppm"],
        check_work=check_report, timeout=120,
    ) for aircraft in REVIEWED_AIRCRAFT]
