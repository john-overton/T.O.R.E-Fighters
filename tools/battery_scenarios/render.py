"""Lane: flight. Windowed captures that reproduce two drawing faults.

They live apart from flight.py because their names start with `render-`, not
`flight-`. Each only checks that the frame is not blank; the picture needs a
human eye. See docs/testing/lane-flight.md#render-captures.
"""
from battery import Scenario
from battery_scenarios.flight import frame_problems


def scenarios() -> list[Scenario]:
    """Windowed captures that reproduce two drawing faults, each checked for a
    blank frame. Whether the fault shows needs a human eye (docs/testing/lane-flight.md)."""
    return [
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
