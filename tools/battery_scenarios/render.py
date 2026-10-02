"""Lane: flight. Windowed aircraft, runway and HUD regression captures.

They live apart from flight.py because their names start with `render-`, not
`flight-`. Each only checks that the frame is not blank; the picture needs a
human eye. See docs/testing/lane-flight.md#render-captures.
"""
from battery import Scenario
from battery_scenarios.flight import frame_problems


def scenarios() -> list[Scenario]:
    """Windowed regression captures, each checked for a
    blank frame. Whether the fault shows needs a human eye (docs/testing/lane-flight.md)."""
    return [
        Scenario(
            name="render-f14-exhaust-top",
            lane="flight",
            args=["--free-flight", "--aircraft", "f14", "--flight-devices", "0,0,0,0,1", "--flight-view", "1", "--flight-look", "0,89", "--flight-zoom", "3", "--capture-flight", "{work}/f14-exhaust-top.ppm", "--no-audio"],
            window=True,
            outputs=["f14-exhaust-top.ppm"],
            expect=["Scene capture:"],
            check=frame_problems,
            timeout=180,
            notes="Both exhaust collars and afterburner roots must have matching widths and centerlines.",
        ),
        Scenario(
            name="render-f14-rear-belly",
            lane="flight",
            args=["--free-flight", "--aircraft", "f14", "--flight-devices", "0,0,0,0,1", "--flight-view", "1", "--flight-look", "0,-30", "--flight-zoom", "3", "--capture-flight", "{work}/f14-rear-belly.ppm", "--no-audio"],
            window=True,
            outputs=["f14-rear-belly.ppm"],
            expect=["Scene capture:"],
            check=frame_problems,
            timeout=180,
            notes="Aft belly and tail fairing seams must be closed, with matching exhaust outlets.",
        ),
        Scenario(
            name="render-f14-nose-belly",
            lane="flight",
            args=["--free-flight", "--aircraft", "f14", "--flight-devices", "0,0,0,0,0", "--flight-view", "1", "--flight-look", "180,-55", "--flight-zoom", "3", "--capture-flight", "{work}/f14-nose-belly.ppm", "--no-audio"],
            window=True,
            outputs=["f14-nose-belly.ppm"],
            expect=["Scene capture:"],
            check=frame_problems,
            timeout=180,
            notes="The nose side and both under-cockpit panels must be closed without covering the air intakes.",
        ),
        Scenario(
            name="render-f14-devices-belly",
            lane="flight",
            args=["--free-flight", "--aircraft", "f14", "--flight-devices", "1,1,1,1,0", "--flight-view", "1", "--flight-look", "180,-55", "--flight-zoom", "3", "--capture-flight", "{work}/f14-devices-belly.ppm", "--no-audio"],
            window=True,
            outputs=["f14-devices-belly.ppm"],
            expect=["Scene capture:"],
            check=frame_problems,
            timeout=180,
            notes="Repaired body seams stay closed while gear, hook and brakes remain free to deploy.",
        ),
        Scenario(
            name="render-f14-roots-top",
            lane="flight",
            args=["--free-flight", "--aircraft", "f14", "--maneuver", "overspeed", "--flight-probe-ticks", "1", "--flight-devices", "0,0,0,0,1", "--flight-view", "1", "--flight-look", "0,89", "--flight-zoom", "3", "--capture-flight", "{work}/f14-top.ppm", "--no-audio"],
            window=True,
            outputs=["f14-top.ppm"],
            expect=["Scene capture:"],
            check=frame_problems,
            timeout=180,
            notes="Fully swept F-14: both wing roots and horizontal tail roots should meet the body symmetrically. The one-tick speed probe only sets the pose.",
        ),
        Scenario(
            name="render-f14-roots-side",
            lane="flight",
            args=["--free-flight", "--aircraft", "f14", "--maneuver", "overspeed", "--flight-probe-ticks", "1", "--flight-devices", "0,0,0,0,1", "--flight-view", "1", "--flight-look", "90,0", "--flight-zoom", "3", "--capture-flight", "{work}/f14-side.ppm", "--no-audio"],
            window=True,
            outputs=["f14-side.ppm"],
            expect=["Scene capture:"],
            check=frame_problems,
            timeout=180,
            notes="Wing panels should sit close to the outer root and tuck under the fixed glove, not float above the fuselage deck.",
        ),
        Scenario(
            name="render-nosewheel-hud-inactive",
            lane="flight",
            args=["--free-flight", "--aircraft", "f18", "--ground-start", "1", "--maneuver", "takeoff", "--flight-probe-ticks", "600", "--flight-view", "0", "--capture-flight", "{work}/nsw-inactive.ppm", "--no-audio"],
            window=True,
            outputs=["nsw-inactive.ppm"],
            expect=["Scene capture:"],
            check=frame_problems,
            timeout=180,
            notes="Rolling above the nosewheel steering cutoff, the NSW label should be absent even with the gear down.",
        ),
        Scenario(
            name="render-mig21-upper-skin",
            lane="flight",
            args=["--free-flight", "--aircraft", "mig21", "--flight-view", "2", "--flight-look", "0,-12", "--capture-flight", "{work}/mig21.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "11:00"},
            window=True,
            outputs=["mig21.ppm"],
            expect=["Scene capture:"],
            check=frame_problems,
            timeout=180,
            notes="Look at the upper wing: the differently divided underside skin must not show through it. Uses the MiG-21 identity.",
        ),
        Scenario(
            name="render-runway-paint-calm",
            lane="flight",
            args=["--free-flight", "--aircraft", "f18", "--theater", "UKR", "--ground-start", "1", "--maneuver", "takeoff", "--flight-probe-ticks", "1300", "--flight-view", "1", "--capture-flight", "{work}/runway.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "07:15", "TORE_WIND": "0,0"},
            window=True,
            outputs=["runway.ppm"],
            expect=["Scene capture:"],
            check=frame_problems,
            timeout=180,
            notes="Calm runway start and a moving view of the pavement: the paint must be whole, with no flicker or z-fighting. The ignored GPU airport tests check depth and aircraft occlusion separately.",
        ),
    ]
