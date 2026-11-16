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
                     "f18", "rafale", "mig29", "su27", "su35", "mig21", "su25", "f22", "f22n", "faxx", "x31", "mig23", "f14", "a4e", "mig25",
                     "f16a", "mig31", "m2000", "f15e", "gripen", "f5e", "mf1", "tornado", "tornadoc", "b52", "b2", "f117", "f111")


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
        "F16.PT": {"flap-roll-combinations.csv": 26},
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
        "M25.PT": {"dense-gear.csv": 203, "flap-roll-combinations.csv": 26},
        "MIG31.PT": {"dense-gear.csv": 203, "flap-roll-combinations.csv": 26},
        "M2000.PT": {"dense-gear.csv": 203, "pitch-roll-combinations.csv": 26},
        "F15E.PT": {"dense-gear.csv": 203, "pitch-roll-combinations.csv": 26,
                    "dense-elevator.csv": 22, "dense-aileron.csv": 22, "dense-flaps.csv": 22},
        "GRIPEN.PT": {"dense-gear.csv": 203, "pitch-roll-combinations.csv": 26},
        "F5E.PT": {"dense-gear.csv": 203, "pitch-roll-combinations.csv": 26},
        "MF1.PT": {"dense-gear.csv": 203, "pitch-roll-combinations.csv": 26},
        # B-2 (B2-3, B2-4): the elevons, the drag rudders against the speed brake, the gear.
        "B2.PT": {"dense-gear.csv": 203, "pitch-roll-combinations.csv": 26,
                  "yaw-brake-combinations.csv": 26},
        "TOR.PT": {"dense-gear.csv": 203, "pitch-roll-combinations-25deg.csv": 26,
                   "pitch-roll-combinations-67deg.csv": 26, "sweep-flap-combinations.csv": 36,
                   "brake-sweep.csv": 8, "exhaust-sweep.csv": 8},
        # The B-52 (B52-3, B52-4): the pitch against yaw grid (25 poses) and the dense gear sweep
        # (202 values); no roll surface, brake or plume.
        "B52.PT": {"dense-gear.csv": 203, "pitch-yaw-combinations.csv": 26},
        # The F-117 (F117-3 and F117-4): the ruddervator pitch/yaw grid (25 poses), the dense gear
        # sweep (202 values) and the interim bay sweep (8 values); no elevon, flap, brake or plume.
        "F117.PT": {"dense-gear.csv": 203, "pitch-yaw-combinations.csv": 26, "bay-sweep.csv": 9},
        # The F-111F (F111-3): the pitch/roll grid at the spread and the fully swept wing (25 poses each),
        # the sweep against speed and flaps (40 poses), the root seam, swept span and taileron clearance
        # at nine sweep angles, the taileron corner swing, and the dense gear, dorsal brake and plume sweeps.
        "F111.PT": {"dense-gear.csv": 203, "pitch-roll-combinations-16deg.csv": 26,
                    "pitch-roll-combinations-72deg.csv": 26, "sweep-flap-combinations.csv": 41,
                    "sweep-seam-span-taileron.csv": 19, "taileron-corner-swing.csv": 25,
                    "brake-sweep.csv": 9, "exhaust-sweep.csv": 8},
        # The Tornado IDS (Clean) (TOR-13) drives the Tornado's own rig: the same sweeps.
        "tornadoc": {"dense-gear.csv": 203, "pitch-roll-combinations-25deg.csv": 26,
                     "pitch-roll-combinations-67deg.csv": 26, "sweep-flap-combinations.csv": 36,
                     "brake-sweep.csv": 8, "exhaust-sweep.csv": 8},
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
    return {"f4g": "F4.PT", "mig17": "MIG17F.PT", "x31": "F31.PT", "faxx": "faxx", "mig25": "M25.PT",
            "f16a": "F16.PT", "tornado": "TOR.PT", "tornadoc": "tornadoc"}.get(
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
