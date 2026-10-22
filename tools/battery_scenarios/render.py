"""Lane: flight. Windowed aircraft, runway and HUD regression captures.

They live apart from flight.py because their names start with `render-`, not
`flight-`. Art checks reject blank frames; capture determinism checks also
compare repeated images byte for byte. See docs/testing/lane-flight.md#render-captures.
"""
from pathlib import Path
import re

from battery import Scenario, Step
from battery_scenarios.flight import frame_problems


def repeated_capture_problems(work: Path, _output: str) -> list[str]:
    paths = [work / f"capture-{repeat}.ppm" for repeat in range(3)]
    # The runner reports missing files separately.
    if not all(path.is_file() for path in paths):
        return []
    first = paths[0].read_bytes()
    return [f"prepared capture changed on repeat {repeat}"
            for repeat, path in enumerate(paths[1:], 1) if path.read_bytes() != first]


CONSOLE_FORMS = ("whot", "bhot", "tv", "nvg", "free-wfov", "narrow-e4", "near-limit", "at-limit")


def console_problems(output: str) -> list[str]:
    """Every console form was drawn, and only the one at the gimbal's top says GIMBAL LIMIT."""
    lines = [line for line in output.splitlines() if line.startswith("ai-strike: CONSOLE")]
    problems = []
    if len(lines) != len(CONSOLE_FORMS):
        problems.append(f"{len(lines)} console frames, expected {len(CONSOLE_FORMS)}")
    for line in lines:
        at_limit = "-at-limit.png" in line
        if at_limit != ("status=GIMBAL LIMIT" in line):
            problems.append(f"gimbal limit status wrong: {line}")
    return problems


def stub_wing_problems(output: str) -> list[str]:
    """The Apache's actual hanger vertices meet the low stub wing, not its rotor."""
    rows = {int(row): (float(low), float(high)) for row, low, high in re.findall(
        r"Audit hanger: ah64 row=(\d+) bottom=(-?[\d.]+) top=(-?[\d.]+)", output)}
    problems = []
    for row in (5, 6):
        if row not in rows:
            problems.append(f"missing Apache hanger row {row}")
        elif not (-1.5 <= rows[row][0] < rows[row][1] <= 1.0):
            problems.append(f"Apache row {row} attaches beyond its stub wing: {rows[row]}")
    return problems


def scenarios() -> list[Scenario]:
    """Windowed art inspections and exact repeated-capture checks."""
    stable = []
    for view in ("flight", "terrain"):
        def args(repeat):
            return [f"--capture-{view}", f"{{work}}/capture-{repeat}.ppm",
                    "--window-size", "1280x720", "--no-audio", "--no-controllers"]
        stable.append(Scenario(
            name=f"render-capture-{view}-deterministic", lane="flight",
            args=args(0), window=True, timeout=180,
            env={"TORE_WEATHER_TIME": "11:00", "TORE_PERF_FRAMES": "60", "TORE_PERF_ACTIVE": "1"},
            then=[Step(args(repeat), window=True, timeout=180) for repeat in (1, 2)],
            outputs=[f"capture-{repeat}.ppm" for repeat in range(3)],
            expect=["Scene capture:"], check=frame_problems,
            check_work=repeated_capture_problems,
            notes="Bare captures hold the prepared state, independent of startup and GPU initialization time.",
        ))
    return stable + [
        Scenario(
            name="render-ah64-stub-wing-pylons", lane="flight",
            args=["--stores-preview", "{work}/stores"],
            env={"TORE_STORES_PREVIEW_CPU": "1", "TORE_STORES_PREVIEW_AUDIT": "1",
                 "TORE_STORES_PREVIEW_AIRCRAFT": "ah64"},
            expect=["Audit hanger: ah64 row=5", "Audit hanger: ah64 row=6"],
            check=stub_wing_problems,
            notes="Headless actual pylon geometry: the rotor must not hide the nearby stub wing.",
        ),
        Scenario(
            name="render-ah64-stub-wing-at12", lane="flight",
            args=["--stores-preview", "{work}/stores"],
            env={"TORE_STORES_PREVIEW_CPU": "1", "TORE_STORES_PREVIEW_AUDIT": "1",
                 "TORE_STORES_PREVIEW_AIRCRAFT": "ah64",
                 "TORE_STORES_PREVIEW_RECORDS": "5=AT12.JT:2,6=AT12.JT:2"},
            expect=["Audit hanger: ah64 row=5", "Audit hanger: ah64 row=6"],
            check=stub_wing_problems,
            notes="An off-centre missile mesh keeps its pylon under the stub wing without moving the missile.",
        ),
        Scenario(
            name="render-night-light-slots",
            lane="flight",
            args=["--night-light-slots"],
            expect=["Night light slots: 240 frames, 8 fires stable through both budgets and upload"],
            notes="Headless synthetic fires plus 0 to 64 remote AC-130-style flashes per frame; checks effect selection, scene selection and relative-position upload.",
        ),
        Scenario(
            name="render-terrain-colors",
            lane="flight",
            args=["--terrain-colors"],
            timeout=180,
            expect=["Ground colours validated against retail map palettes"],
            notes="Headless RGB statistics and exact palette checks for every theater, six weather choices and three altitudes, against each retail MM's family.",
        ),
        Scenario(
            name="render-ac130-sensor-console",
            lane="flight",
            args=["--a2g-run", "UKR", "QUCOL", "--pilot", "ai", "--aircraft", "ac130", "--load", "C_25.JT",
                  "--altitude", "6000", "--speed", "220", "--from", "3", "--seconds", "50", "--skill", "2",
                  "--console-scene", "{work}/console", "--console-at", "45"],
            window=True,
            outputs=[f"console/console-t045.0s-{form}.png" for form in CONSOLE_FORMS],
            expect=["ai-strike: CONSOLE", "status=GIMBAL LIMIT"],
            check=console_problems,
            timeout=300,
            notes="The AC-130 sensor console offscreen from an AI gunship's own sensor 45 s into its orbit on the "
                  "Ukraine column (slice nr-fltv): white hot, black hot, TV, night vision (nr-conb), wide and "
                  "narrow fields, near and at the gimbal limit. Inspect the PNGs; the symbology's rules are tore-app unit tests.",
        ),
        Scenario(
            name="render-night-effect-lights",
            lane="flight",
            args=["--impact-preview", "{work}/night", "night-range"],
            window=True,
            env={"TORE_PREVIEW_THEATER": "EGY", "TORE_CITY_FILL": "0"},
            outputs=[f"night/fire-compare-{width:03d}ft.png" for width in (24, 90, 160)]
                + ["night/night-00.3nm-t1.00s.png", "night/night-05.0nm-t0.15s.png"],
            expect=["Impact preview:"],
            timeout=240,
            notes="Night fire size and flare comparisons, plus fireballs after the initial flash. Inspect the PNGs; numeric emission and fade regressions are tore-app unit tests.",
        ),
        Scenario(
            name="render-f14-hook-deployed",
            lane="flight",
            args=["--free-flight", "--aircraft", "f14", "--flight-devices", "0,0,0,1,0", "--flight-view", "1", "--flight-look", "75,-25", "--flight-zoom", "4", "--capture-flight", "{work}/f14-hook-deployed.ppm", "--no-audio"],
            window=True,
            outputs=["f14-hook-deployed.ppm"],
            expect=["Scene capture:"],
            check=frame_problems,
            timeout=180,
            notes="The hook shows the F-22N striped shank and hooked end, fitted to the F-14 attachment instead of a solid triangular spike.",
        ),
        Scenario(
            name="render-f14-hook-reverse",
            lane="flight",
            args=["--free-flight", "--aircraft", "f14", "--flight-devices", "0,0,0,1,0", "--flight-view", "1", "--flight-look", "-75,-25", "--flight-zoom", "4", "--capture-flight", "{work}/f14-hook-reverse.ppm", "--no-audio"],
            window=True,
            outputs=["f14-hook-reverse.ppm"],
            expect=["Scene capture:"],
            check=frame_problems,
            timeout=180,
            notes="The opposite side shows the same hook artwork and outline, with no solid backing triangle.",
        ),
        Scenario(
            name="render-f14-hook-half",
            lane="flight",
            args=["--free-flight", "--aircraft", "f14", "--flight-devices", "0,0,0,0.5,0", "--flight-view", "1", "--flight-look", "75,-25", "--flight-zoom", "4", "--capture-flight", "{work}/f14-hook-half.ppm", "--no-audio"],
            window=True,
            outputs=["f14-hook-half.ppm"],
            expect=["Scene capture:"],
            check=frame_problems,
            timeout=180,
            notes="The textured hook stays rigid and attached while rising toward the belly during retraction.",
        ),
        Scenario(
            name="render-f14-hook-stowed",
            lane="flight",
            args=["--free-flight", "--aircraft", "f14", "--flight-devices", "0,0,0,0,0", "--flight-view", "1", "--flight-look", "75,-25", "--flight-zoom", "4", "--capture-flight", "{work}/f14-hook-stowed.ppm", "--no-audio"],
            window=True,
            outputs=["f14-hook-stowed.ppm"],
            expect=["Scene capture:"],
            check=frame_problems,
            timeout=180,
            notes="The hook is completely hidden at zero extension.",
        ),
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
            args=["--free-flight", "--aircraft", "f18", "--ground-start", "2", "--maneuver", "takeoff", "--flight-probe-ticks", "1200", "--flight-view", "0", "--capture-flight", "{work}/nsw-inactive.ppm", "--no-audio"],
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
            args=["--free-flight", "--aircraft", "f18", "--theater", "UKR", "--ground-start", "2", "--maneuver", "takeoff", "--flight-probe-ticks", "1300", "--flight-view", "1", "--capture-flight", "{work}/runway.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "07:15", "TORE_WIND": "0,0"},
            window=True,
            outputs=["runway.ppm"],
            expect=["Scene capture:"],
            check=frame_problems,
            timeout=180,
            notes="Calm runway start and a moving view of the pavement: the paint must be whole, with no flicker or z-fighting. The ignored GPU airport tests check depth and aircraft occlusion separately.",
        ),
    ]
