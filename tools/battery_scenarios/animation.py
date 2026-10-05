"""CPU geometry regressions for individually reviewed aircraft animation rigs.

This lane checks actual render-path geometry, without creating a window.
Source artifacts stay in the battery's ignored per-scenario output directory.
"""
import json
from pathlib import Path

from battery import Scenario

# Expand only after independent source attachments and pose sheets are reviewed.
REVIEWED_AIRCRAFT = ("a7", "f4b", "f4j", "f4e", "f4g", "f15", "mig17", "f16c", "f104", "a310", "c130", "ac130", "e3", "a10", "av8", "ah64", "mi24", "ch47")


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
    aircraft = report.get("aircraft")
    rotor_poses = {"AH64.PT": 1200, "MI24.PT": 1200, "CH47.PT": 6000}.get(aircraft)
    if rotor_poses:
        try:
            combined = json.loads((work / "poses/rotor-combinations.json").read_text())
            if combined.get("poses") != rotor_poses or not combined.get("checks_passed"):
                problems.append("combined rotor attachment/separation checks did not pass")
        except (OSError, ValueError) as error:
            problems.append(f"unreadable combined rotor report: {error}")
    csv_name = ("flap-roll-combinations.csv" if aircraft in {"F16C.PT", "F104.PT", "E3.PT"}
                else "nozzle-combinations.csv" if aircraft == "AV8.PT" else None)
    if csv_name:
        try:
            if len((work / "poses" / csv_name).read_text().splitlines()) != 26:
                problems.append("combined control sweep omitted expected poses")
        except OSError as error:
            problems.append(f"missing combined control sweep: {error}")
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
