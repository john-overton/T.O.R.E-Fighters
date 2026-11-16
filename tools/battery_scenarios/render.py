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


def f117_bay_problems(output: str) -> list[str]:
    """The default GBU-27s clear the F-117's open doors and the bay's ends and trapeze rail (F117-5)."""
    problems = []
    for side in ("left", "right"):
        m = re.search(
            rf"Bay clearance: row\d GB27\.SH {side}: inner door (-?[\d.]+), outer door (-?[\d.]+), "
            r"aft end (-?[\d.]+), fore end (-?[\d.]+), roof (-?[\d.]+), hangs (-?[\d.]+) below the belly plane, "
            r"top skin (-?[\d.]+) at .*?, rail (-?[\d.]+)", output)
        if not m:
            problems.append(f"no bay clearance line for the {side} GBU-27")
            continue
        inner, outer, aft, fore, _roof, _hang, _skin, rail = map(float, m.groups())
        for label, value in (("inner door", inner), ("outer door", outer), ("aft end", aft),
                             ("fore end", fore), ("trapeze rail", rail)):
            if value < 0.05:
                problems.append(f"{side} GBU-27 clears the {label} by only {value:.2f} ft")
    return problems


def b2_bay_problems(output: str) -> list[str]:
    """The default 32 Mk 82 a bay (4 abreast, 2 long, 4 tiers) clear the B-2's open doors, both
    ends, the sloped recess roof and the top skin, and no two stores touch (B2-5)."""
    problems = []
    for side in ("left", "right"):
        m = re.search(
            rf"Bay clearance: row\d MK82\.SH {side}: inner door (-?[\d.]+), outer door (-?[\d.]+), "
            r"aft end (-?[\d.]+), fore end (-?[\d.]+), roof (-?[\d.]+), hangs (-?[\d.]+) below the belly plane, "
            r"top skin (-?[\d.]+) at", output)
        if not m:
            problems.append(f"no bay clearance line for the {side} Mk 82s")
            continue
        inner, outer, aft, fore, roof, _hang, skin = map(float, m.groups())
        for label, value in (("inner door", inner), ("outer door", outer), ("aft end", aft),
                             ("fore end", fore), ("recess roof", roof), ("top skin", skin)):
            if value < 0.05:
                problems.append(f"{side} Mk 82s clear the {label} by only {value:.2f} ft")
    m = re.search(r"Bay clearance: b2 (\d+) stores, closest pair .*?: nearest vertices (-?[\d.]+) ft apart, (\d+) pairs under", output)
    if not m or int(m.group(1)) != 64:
        problems.append("no closest-pair line for 64 stores")
    elif float(m.group(2)) < 0.05 or int(m.group(3)):
        problems.append(f"two Mk 82s are only {m.group(2)} ft apart")
    return problems


def b52_clearance_problems(output: str) -> list[str]:
    """The B-52's 27 Mk 82 and 12 Harpoons clear the open bay and each other (B52-5)."""
    problems = []
    stores = [line for line in output.splitlines() if re.match(r"Bay clearance: row\d MK82\.SH slot\d:", line)]
    if len(stores) != 27:
        problems.append(f"{len(stores)} bay stores reported, expected 27")
    for line in stores:
        for label in ("side wall", "door leaf", "aft end", "fore end", "roof", "above the belly plane", "rails and posts"):
            m = re.search(rf"{label} (-?[\d.]+)", line)
            if not m:
                problems.append(f"no {label} in {line[:48]}")
            elif float(m.group(1)) < 0.05:
                problems.append(f"{line[:46]} clears the {label} by only {m.group(1)} ft")
    m = re.search(r"Bay clearance: b52 27 stores, nearest pair (-?[\d.]+) ft", output)
    if not m or float(m.group(1)) < 0.05:
        problems.append("the bay stores touch each other")
    m = re.search(r"Pylon clearance: b52 \d+ stores, nearest pair (-?[\d.]+) ft .*, (\d+) pairs under", output)
    if not m or int(m.group(2)) != 0:
        problems.append("the pylon stores touch each other")
    for flaps in ("up", "down 15 deg"):
        m = re.search(rf"Pylon clearance: b52 row 7 flaps {flaps} .*?flap strip (-?[\d.]+) .*?wing \(stores only\) (-?[\d.]+)", output)
        if not m:
            problems.append(f"no pylon line with the flaps {flaps}")
        elif min(float(m.group(1)), float(m.group(2))) < 0.05:
            problems.append(f"the Harpoons touch the flap strip or the wing with the flaps {flaps}")
    return problems


def skin_capture_problems(output: str) -> list[str]:
    """Every aircraft skin capture drew a non-blank frame."""
    captures = [line for line in output.splitlines() if line.startswith("Scene capture:")]
    if not captures:
        return ["no Scene capture line"]
    problems = []
    for line in captures:
        problems.extend(frame_problems(line))
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
            name="render-f117-bay-clearance", lane="flight",
            args=["--stores-preview", "{work}/stores"],
            env={"TORE_STORES_PREVIEW_CPU": "1", "TORE_STORES_PREVIEW_BAYCLEAR": "1",
                 "TORE_STORES_PREVIEW_AIRCRAFT": "f117"},
            expect=["Bay clearance: f117 opening"], check=f117_bay_problems,
            notes="Headless F-117 bay clearances in feet with the doors open (F117-5): door planes, ends, roof, top skin and trapeze rail.",
        ),
        Scenario(
            name="render-b2-bay-clearance", lane="flight",
            args=["--stores-preview", "{work}/stores"],
            env={"TORE_STORES_PREVIEW_CPU": "1", "TORE_STORES_PREVIEW_BAYCLEAR": "1",
                 "TORE_STORES_PREVIEW_AIRCRAFT": "b2"},
            expect=["Bay clearance: b2 opening"], check=b2_bay_problems,
            notes="Headless B-2 bay clearances in feet with the doors open (B2-5): the default 64 Mk 82 against the door planes, ends, sloped roof and top skin.",
        ),
        Scenario(
            name="render-b52-clearance", lane="flight",
            args=["--stores-preview", "{work}/stores"],
            env={"TORE_STORES_PREVIEW_CPU": "1", "TORE_STORES_PREVIEW_B52CLEAR": "1",
                 "TORE_STORES_PREVIEW_AIRCRAFT": "b52",
                 "TORE_STORES_PREVIEW_RECORDS": "3=MK82.JT:9,5=MK82.JT:9,6=MK82.JT:9"},
            expect=["Bay clearance: b52 opening", "Pylon clearance: b52 row 7"], check=b52_clearance_problems,
            notes="Headless B-52 clearances in feet (B52-5): 27 Mk 82 in the open bay against the doors, walls, roof, floor, rails and each other; 12 default Harpoons on the pylons against the pod, the flap strip up and down, the fuselage, the gear and the ground.",
        ),
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
            name="render-mig25-skin",
            lane="flight",
            args=["--free-flight", "--aircraft", "mig25", "--flight-view", "2", "--flight-look", "0,60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/mig25-top.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "12:00"},
            window=True,
            outputs=["mig25-top.ppm", "mig25-under.ppm"],
            expect=["Scene capture:"],
            check=skin_capture_problems,
            then=[
                Step(["--free-flight", "--aircraft", "mig25", "--flight-view", "2", "--flight-look", "0,-60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/mig25-under.ppm", "--no-audio"], window=True, timeout=180),
            ],
            timeout=180,
            notes="MiG-25 skin from above and below with the gear down: the double-sided wing panels must not z-fight and the underside must not show through the top. The two E0 wing-marking decal slots stay omitted rather than filled with skin (docs/formats/objects-and-shapes.md, MiG-25 skin review).",
        ),
        Scenario(
            name="render-f16a-skin",
            lane="flight",
            args=["--free-flight", "--aircraft", "f16a", "--flight-view", "2", "--flight-look", "0,60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/f16a-top.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "12:00"},
            window=True,
            outputs=["f16a-top.ppm", "f16a-under.ppm"],
            expect=["Scene capture:"],
            check=skin_capture_problems,
            then=[
                Step(["--free-flight", "--aircraft", "f16a", "--flight-view", "2", "--flight-look", "0,-60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/f16a-under.ppm", "--no-audio"], window=True, timeout=180),
            ],
            timeout=180,
            notes="F-16A skin from above and below with the gear down: it draws the shared F16.SH, so the double-sided wing panels must not z-fight and the underside must not show through the top. All five E0 decal slots stay omitted rather than filled with skin (docs/formats/objects-and-shapes.md, F-16A skin review).",
        ),
        Scenario(
            name="render-mig31-skin",
            lane="flight",
            args=["--free-flight", "--aircraft", "mig31", "--flight-view", "2", "--flight-look", "0,60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/mig31-top.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "12:00"},
            window=True,
            outputs=["mig31-top.ppm", "mig31-under.ppm"],
            expect=["Scene capture:"],
            check=skin_capture_problems,
            then=[
                Step(["--free-flight", "--aircraft", "mig31", "--flight-view", "2", "--flight-look", "0,-60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/mig31-under.ppm", "--no-audio"], window=True, timeout=180),
            ],
            timeout=180,
            notes="MiG-31 skin from above and below with the gear down: its 25 double-sided panels are exact-vertex twins with opposite stored normals, so they must not z-fight and the underside must not show through the top. The two E0 wing-marking decal slots stay omitted rather than filled with skin (docs/formats/objects-and-shapes.md, MiG-31 skin review).",
        ),
        Scenario(
            name="render-m2000-skin",
            lane="flight",
            args=["--free-flight", "--aircraft", "m2000", "--flight-view", "2", "--flight-look", "0,60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/m2000-top.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "12:00"},
            window=True,
            outputs=["m2000-top.ppm", "m2000-under.ppm"],
            expect=["Scene capture:"],
            check=skin_capture_problems,
            then=[
                Step(["--free-flight", "--aircraft", "m2000", "--flight-view", "2", "--flight-look", "0,-60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/m2000-under.ppm", "--no-audio"], window=True, timeout=180),
            ],
            timeout=180,
            notes="Mirage 2000 skin from above and below with the gear down, at its real size (per-axis shape scale 1.3616, 1.2290, 1.5053, docs/formats/objects-and-shapes.md, Aircraft shape scale): no underside shape shows through the top and the double-sided panels do not z-fight. The shape's E0 decal slots stay omitted rather than filled with skin.",
        ),
        Scenario(
            name="render-f15e-skin",
            lane="flight",
            args=["--free-flight", "--aircraft", "f15e", "--flight-view", "2", "--flight-look", "0,60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/f15e-top.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "12:00"},
            window=True,
            outputs=["f15e-top.ppm", "f15e-under.ppm"],
            expect=["Scene capture:"],
            check=skin_capture_problems,
            then=[
                Step(["--free-flight", "--aircraft", "f15e", "--flight-view", "2", "--flight-look", "0,-60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/f15e-under.ppm", "--no-audio"], window=True, timeout=180),
            ],
            timeout=180,
            notes="F-15E skin from above and below with the gear down, at the shape's own size (no shape scale): its 17 double-sided panels are exact-vertex twins with opposite stored normals, so they must not z-fight and the underside must not show through the top (docs/formats/objects-and-shapes.md, F-15E skin review).",
        ),
        Scenario(
            name="render-gripen-skin",
            lane="flight",
            args=["--free-flight", "--aircraft", "gripen", "--flight-view", "2", "--flight-look", "0,60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/gripen-top.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "12:00"},
            window=True,
            outputs=["gripen-top.ppm", "gripen-under.ppm"],
            expect=["Scene capture:"],
            check=skin_capture_problems,
            then=[
                Step(["--free-flight", "--aircraft", "gripen", "--flight-view", "2", "--flight-look", "0,-60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/gripen-under.ppm", "--no-audio"], window=True, timeout=180),
            ],
            timeout=180,
            notes="Saab Gripen skin from above and below with the gear down, at the shape's own size (true size, no scale): no underside shape shows through the top and the 19 double-sided panels do not z-fight. The five E0 decal slots (nose art, both fin sides, both wing markings) stay omitted rather than filled with skin (docs/formats/objects-and-shapes.md, Gripen skin review).",
        ),
        Scenario(
            name="render-f5e-skin",
            lane="flight",
            args=["--free-flight", "--aircraft", "f5e", "--flight-view", "2", "--flight-look", "0,60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/f5e-top.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "12:00"},
            window=True,
            outputs=["f5e-top.ppm", "f5e-under.ppm"],
            expect=["Scene capture:"],
            check=skin_capture_problems,
            then=[
                Step(["--free-flight", "--aircraft", "f5e", "--flight-view", "2", "--flight-look", "0,-60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/f5e-under.ppm", "--no-audio"], window=True, timeout=180),
            ],
            timeout=180,
            notes="F-5E skin from above and below with the gear down: its exact-vertex twin panels carry opposite stored normals, so they must not z-fight and the underside must not show through the top. The F-5E draws its own F5E.SH and _F5E.PIC (the desert and snow records are separate). The shape's two E0 wing decal slots stay omitted rather than filled with skin (docs/formats/objects-and-shapes.md, F-5E skin review).",
        ),
        Scenario(
            name="render-mf1-skin",
            lane="flight",
            args=["--free-flight", "--aircraft", "mf1", "--flight-view", "2", "--flight-look", "0,60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/mf1-top.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "12:00"},
            window=True,
            outputs=["mf1-top.ppm", "mf1-under.ppm"],
            expect=["Scene capture:"],
            check=skin_capture_problems,
            then=[
                Step(["--free-flight", "--aircraft", "mf1", "--flight-view", "2", "--flight-look", "0,-60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/mf1-under.ppm", "--no-audio"], window=True, timeout=180),
            ],
            timeout=180,
            notes="Mirage F1 skin from above and below with the gear down, at its real size (per-axis shape scale 1.0335, 1.1804, 1.0546, docs/formats/objects-and-shapes.md, Aircraft shape scale): no underside shape shows through the top and the double-sided panels do not z-fight. The shape's two E0 wing-marking decal slots stay omitted rather than filled with skin.",
        ),
        Scenario(
            name="render-tornado-skin",
            lane="flight",
            args=["--free-flight", "--aircraft", "tornado", "--flight-view", "2", "--flight-look", "0,60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/tornado-top.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "12:00"},
            window=True,
            outputs=["tornado-top.ppm", "tornado-under.ppm"],
            expect=["Scene capture:"],
            check=skin_capture_problems,
            then=[
                Step(["--free-flight", "--aircraft", "tornado", "--flight-view", "2", "--flight-look", "0,-60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/tornado-under.ppm", "--no-audio"], window=True, timeout=180),
            ],
            timeout=180,
            notes="Tornado IDS skin from above and below with the gear down, at its header scale (2/3 ft a unit, true size, no per-axis factor): no underside shape shows through the top and the double-sided panels do not z-fight. The shape's two E0 fin decal slots stay omitted rather than filled with skin (docs/formats/objects-and-shapes.md, Tornado skin review).",
        ),
        Scenario(
            name="render-b52-skin",
            lane="flight",
            args=["--free-flight", "--aircraft", "b52", "--flight-view", "2", "--flight-look", "0,60", "--flight-zoom", "0.5", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/b52-top.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "12:00"},
            window=True,
            outputs=["b52-top.ppm", "b52-under.ppm"],
            expect=["Scene capture:"],
            check=skin_capture_problems,
            then=[
                Step(["--free-flight", "--aircraft", "b52", "--flight-view", "2", "--flight-look", "0,-60", "--flight-zoom", "0.5", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/b52-under.ppm", "--no-audio"], window=True, timeout=180),
            ],
            timeout=180,
            notes="B-52H skin from above and below with the gear down, at its header scale (2/3 ft a unit, true size, no per-axis factor): no underside shape shows through the top and the 12 double-sided panels do not z-fight. The shape's two E0 wing-marking decal slots stay omitted rather than filled with skin (docs/formats/aircraft-variety.md, B-52 shape, size and looks). The zoom is 0.5 so the 185 ft span fits.",
        ),
        Scenario(
            name="render-b2-skin",
            lane="flight",
            args=["--free-flight", "--aircraft", "b2", "--flight-view", "2", "--flight-look", "0,60", "--flight-zoom", "1.4", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/b2-top.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "12:00"},
            window=True,
            outputs=["b2-top.ppm", "b2-under.ppm"],
            expect=["Scene capture:"],
            check=skin_capture_problems,
            then=[
                Step(["--free-flight", "--aircraft", "b2", "--flight-view", "2", "--flight-look", "0,-60", "--flight-zoom", "1.4", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/b2-under.ppm", "--no-audio"], window=True, timeout=180),
            ],
            timeout=180,
            notes="B-2 skin from above and below with the gear down, at its header scale (1/3 ft a unit, true span, no per-axis factor): no underside shape shows through the top and the double-sided panels do not z-fight. The shape's one E0 decal slot (2a73, a 7 by 5 unit plate beside the cockpit) stays omitted rather than filled with skin (docs/formats/aircraft-variety.md, B-2 shape, size and looks).",
        ),
        Scenario(
            name="render-f117-skin",
            lane="flight",
            args=["--free-flight", "--aircraft", "f117", "--flight-view", "2", "--flight-look", "0,60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/f117-top.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "12:00"},
            window=True,
            outputs=["f117-top.ppm", "f117-under.ppm"],
            expect=["Scene capture:"],
            check=skin_capture_problems,
            then=[
                Step(["--free-flight", "--aircraft", "f117", "--flight-view", "2", "--flight-look", "0,-60", "--flight-zoom", "2", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/f117-under.ppm", "--no-audio"], window=True, timeout=180),
            ],
            timeout=180,
            notes="F-117A skin from above and below with the gear down, at its real size (per-axis shape scale 0.7647, 1.0463, 0.8869, docs/formats/objects-and-shapes.md, Aircraft shape scale): no underside shape shows through the top and the 19 double-sided panels do not z-fight. The shape's five E0 decal slots (two wing markings, two ruddervator markings, one fuselage side marking) stay omitted rather than filled with skin.",
        ),
        Scenario(
            name="render-b1b-skin",
            lane="flight",
            args=["--free-flight", "--aircraft", "b1b", "--flight-view", "2", "--flight-look", "0,60", "--flight-zoom", "0.5", "--flight-devices", "1,0,0,0,0", "--loadout", "none", "--capture-flight", "{work}/b1b-top.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "12:00"},
            window=True,
            outputs=["b1b-top.ppm", "b1b-under.ppm"],
            expect=["Scene capture:"],
            check=skin_capture_problems,
            then=[
                Step(["--free-flight", "--aircraft", "b1b", "--flight-view", "2", "--flight-look", "0,-60", "--flight-zoom", "0.5", "--flight-devices", "1,0,0,0,0", "--loadout", "none", "--capture-flight", "{work}/b1b-under.ppm", "--no-audio"], window=True, timeout=180),
            ],
            timeout=180,
            notes="B-1B skin from above and below with the gear down, at its header scale (4/3 ft a unit, true size, no per-axis factor): no underside shape shows through the top and the double-sided panels do not z-fight, spread (the free-flight start is below the 280 knot sweep threshold). The shape's two E0 glove decal slots stay omitted rather than filled with skin (docs/formats/aircraft-variety.md, B-1B shape and rig). The zoom is 0.5 so the 137 ft span fits. The scenario starts stripped (`--loadout none`) because the B-1B default loadout is refused at start (payload over the mass limit, B1B-6/B1B-8 own it).",
        ),
        Scenario(
            name="render-tu160-skin",
            lane="flight",
            args=["--free-flight", "--aircraft", "tu160", "--flight-view", "2", "--flight-look", "0,60", "--flight-zoom", "0.5", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/tu160-top.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "12:00"},
            window=True,
            outputs=["tu160-top.ppm", "tu160-under.ppm"],
            expect=["Scene capture:"],
            check=skin_capture_problems,
            then=[
                Step(["--free-flight", "--aircraft", "tu160", "--flight-view", "2", "--flight-look", "0,-60", "--flight-zoom", "0.5", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/tu160-under.ppm", "--no-audio"], window=True, timeout=180),
            ],
            timeout=180,
            notes="Tu-160 skin from above and below with the gear down and the wing at its spread 20 degrees, at its header scale (8/3 ft a unit, true size within 1 percent on span and length, no per-axis factor): no underside shape shows through the top and the double-sided panels (tailplane, fin, flap strips, keel plates) do not z-fight. The shape's E0 decal slots stay omitted rather than filled with skin (docs/formats/aircraft-variety.md, Tu-160 shape and rig). The zoom is 0.5 so the 181 ft span fits.",
        ),
        Scenario(
            name="render-f111-skin",
            lane="flight",
            args=["--free-flight", "--aircraft", "f111", "--loadout", "none", "--flight-view", "2", "--flight-look", "0,60", "--flight-zoom", "2", "--flight-speed", "200", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/f111-top.ppm", "--no-audio"],
            env={"TORE_WEATHER_TIME": "12:00"},
            window=True,
            outputs=["f111-top.ppm", "f111-under.ppm"],
            expect=["Scene capture:"],
            check=skin_capture_problems,
            then=[
                Step(["--free-flight", "--aircraft", "f111", "--loadout", "none", "--flight-view", "2", "--flight-look", "0,-60", "--flight-zoom", "2", "--flight-speed", "200", "--flight-devices", "1,0,0,0,0", "--capture-flight", "{work}/f111-under.ppm", "--no-audio"], window=True, timeout=180),
            ],
            timeout=180,
            notes="F-111F skin from above and below with the wing spread (200 knots) and the gear down, at its real size (per-axis shape scale 0.859, 1.0, 0.827 on the header scale, docs/formats/objects-and-shapes.md, Aircraft shape scale): no underside shape shows through the top and the 19 double-sided panels do not z-fight; the fixed glove plates lie inside the spread wing and show nothing. The shape has no E0 decal faces. The default load is not used: with the full 32,663 lb tank it is over the 100,000 lb maximum take-off weight and free flight refuses to start (F111-6 and F111-8).",
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
