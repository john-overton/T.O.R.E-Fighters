"""CPU geometry regressions for individually reviewed aircraft animation rigs.

This lane checks actual render-path geometry, without creating a window.
Source artifacts stay in the battery's ignored per-scenario output directory.
"""
import json
import re
from pathlib import Path

from battery import Scenario

# Expand only after independent source attachments and pose sheets are reviewed.
REVIEWED_AIRCRAFT = ("a7", "f4b", "f4j", "f4e", "f4g", "f15", "mig17", "f16c", "f104", "a310", "c130", "ac130", "e3", "a10", "av8", "ah64", "mi24", "ch47",
                     "yak141", "v22", "il76", "e2", "b747",
                     "f18", "rafale", "mig29", "su27", "su35", "mig21", "su25", "f22", "f22n", "faxx", "x31", "mig23", "f14", "a4e")


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
    aircraft = report.get("selection_key", report.get("aircraft"))
    rotor_poses = {"AH64.PT": 1200, "MI24.PT": 1200, "CH47.PT": 6000}.get(aircraft)
    if rotor_poses:
        try:
            combined = json.loads((work / "poses/rotor-combinations.json").read_text())
            if combined.get("poses") != rotor_poses or not combined.get("checks_passed"):
                problems.append("combined rotor attachment/separation checks did not pass")
        except (OSError, ValueError) as error:
            problems.append(f"unreadable combined rotor report: {error}")
    sweeps = {
        "F16C.PT": {"flap-roll-combinations.csv": 26},
        "F104.PT": {"flap-roll-combinations.csv": 26},
        "E3.PT": {"flap-roll-combinations.csv": 26},
        "AV8.PT": {"nozzle-combinations.csv": 13},
        "YAK141.PT": {"nozzle-combinations.csv": 13},
        "V22.PT": {"flaperon-combinations.csv": 26, "conversion-rotor-combinations.csv": 26},
        "IL76.PT": {"flap-roll-combinations.csv": 26},
        "E2.PT": {"flap-roll-combinations.csv": 26, "pitch-yaw-combinations.csv": 26},
        "B747.PT": {"flap-roll-combinations.csv": 26},
        "RAFALE.PT": {"flap-pitch-roll-combinations.csv": 126},
        "MIG29.PT": {"flap-roll-combinations.csv": 26},
        "SU27.PT": {"flaperon-combinations.csv": 26},
        "SU35.PT": {"flap-roll-combinations.csv": 26},
        "MIG21.PT": {"flap-roll-combinations.csv": 26},
        "A4E.PT": {"dense-gear.csv": 203, "dense-hook.csv": 203, "dense-brake.csv": 203,
                   "flap-roll-combinations.csv": 26, "pitch-roll-combinations.csv": 26},
        "F14.PT": {"sweep-flap-roll-combinations.csv": 126, "pitch-roll-combinations.csv": 26},
        "MIG23.PT": {"sweep-flap-roll-combinations.csv": 126, "rudder-material-correspondence.csv": 7},
        "F31.PT": {"prototype-vector-combinations.csv": 26},
        "F22.PT": {"pitch-roll-combinations.csv": 26},
        "F22N.PT": {"pitch-roll-combinations.csv": 26},
        "faxx": {"pitch-roll-combinations.csv": 26, "flap-yaw-combinations.csv": 26},
        "SU25.PT": {"flap-roll-combinations.csv": 26, "rudder-material-correspondence.csv": 6},
    }.get(aircraft, {})
    for csv_name, row_count in sweeps.items():
        try:
            if len((work / "poses" / csv_name).read_text().splitlines()) != row_count:
                problems.append(f"combined control sweep omitted expected poses: {csv_name}")
        except OSError as error:
            problems.append(f"missing combined control sweep: {error}")
    return problems


def selection_key(aircraft: str) -> str:
    # Keep the concept identity separate from its F22N retail donor.
    return {"f4g": "F4.PT", "mig17": "MIG17F.PT", "x31": "F31.PT", "faxx": "faxx"}.get(
        aircraft, aircraft.upper() + ".PT")


def scenarios() -> list[Scenario]:
    return [Scenario(
        name=f"flight-animation-{aircraft}", lane="flight",
        args=["--aircraft", aircraft, "--animation-probe", "{work}/poses",
              "--no-audio", "--no-controllers"],
        expect=[rf"animation probe {re.escape(selection_key(aircraft))}: \d+ neutral faces, 0 required/check failures;"],
        outputs=["poses/report.json", "poses/index.txt", "poses/gear.ppm"],
        check_work=check_report, timeout=120,
    ) for aircraft in REVIEWED_AIRCRAFT]
