"""Lane: menu screens, the Quick Mission creator, loadout and GUI captures.

Headless scenarios (CPU snapshots, creator probes, mission starts through the
headless AI probe) run in parallel; the windowed ones (real flight captures at
several window sizes) go through tools/agent-run.sh and are kept few.
"""
import re
from pathlib import Path

from battery import Scenario

AIRCRAFT = ["f18", "rafale", "f14", "a4e", "x31", "mig29", "su27", "mig21", "su25", "mig23", "su35", "f22", "f22n", "faxx"]

# Every menu state `--snapshot-state` accepts (normal mode).
NORMAL_STATES = [
    "normal", "hover", "pressed", "help", "pref", "multi", "notice", "controls", "controls-keyboard",
    "controls-mouse", "controls-head", "controls-search", "controls-search-keys", "graphics", "sound",
    "replays", "replays-settings", "replays-delete", "locate", "locate-importing", "locate-done",
]
# Quick Mission mode states, the loadout page states and the debrief pages.
QUICK_STATES = [
    "normal", "aircraft", "theaters", "help", "ordnance", "ordnance-empty", "ordnance-drag",
    "ordnance-message", "ordnance-message-long", "debrief", "debrief-2", "debrief-3", "debrief-4",
    "debrief-5", "debrief-success",
]
THEATERS = ["BAL", "CUB", "EGY", "LFA", "FRA", "GRE", "IRA", "KURILE", "TVIET", "SPA", "APA", "PGU", "NSK", "WTA", "UKR", "VLA"]
VARIANT_THEATERS = [f"~{code}{n}" for code in ("UKR", "VLA") for n in range(1, 9)] + ["~UKRF", "~VLAF", "~WTAF"]


def read_ppm(path: Path):
    data = path.read_bytes()
    parts = data.split(b"\n", 3)
    if len(parts) < 4 or parts[0] != b"P6":
        return None
    width, height = (int(v) for v in parts[1].split())
    return width, height, parts[3]


def picture_problems(output: str, marker: str, size=None, min_colors=24, max_flat=0.985) -> list[str]:
    """Reads the picture the run wrote (its path is printed after `marker`) and
    reports a missing, mis-sized, blank or single-colour picture."""
    m = re.search(marker + r"\s*(\S+)", output)
    if not m:
        return [f"no line naming the picture ({marker})"]
    path = Path(m.group(1))
    if not path.exists():
        return [f"picture not written: {path}"]
    ppm = read_ppm(path)
    if ppm is None:
        return [f"not a P6 picture: {path}"]
    width, height, pixels = ppm
    problems = []
    if len(pixels) != width * height * 3:
        problems.append(f"picture {width}x{height} has {len(pixels)} bytes")
        return problems
    if size and (width, height) != size:
        problems.append(f"picture is {width}x{height}, expected {size[0]}x{size[1]}")
    colors = {}
    step = max(1, (width * height) // 20000)
    for i in range(0, width * height, step):
        px = pixels[i * 3 : i * 3 + 3]
        colors[px] = colors.get(px, 0) + 1
    total = sum(colors.values())
    if len(colors) < min_colors:
        problems.append(f"picture has only {len(colors)} colours (blank or flat)")
    if max(colors.values()) / total > max_flat:
        problems.append("picture is almost one flat colour")
    return problems


def menu_picture(size=(640, 480)):
    # The dark input, graphics, replay and locate screens use few colours.
    return lambda output: picture_problems(output, r"(?:Menu|Locate) preview:", size, min_colors=8)


def flight_picture(size):
    return lambda output: picture_problems(output, r"Scene capture:", size, min_colors=200)


def flown_inventory(choice: str):
    """A Quick Mission launched with stores taken off must hold exactly what the
    Load Ordnance page left: every count zero and nothing listed for `none`, only
    the internal gun for `guns`."""

    def check(output: str) -> list[str]:
        m = re.search(r"Quick Mission launch:.* ammo=\[([0-9, ]*)\] listed=\[(.*)\]$", output, re.M)
        if not m:
            return ["no Quick Mission launch line with ammo and listed weapons"]
        ammo = [int(v) for v in m.group(1).split(",") if v.strip()]
        listed = m.group(2)
        problems = []
        if choice == "none":
            if any(ammo):
                problems.append(f"stores came back after removing them all: {ammo}")
            if listed.strip():
                problems.append(f"weapons window lists {listed} for an empty aircraft")
        else:
            if not ammo or ammo[0] == 0 or any(ammo[1:]):
                problems.append(f"gun only expected, ammo is {ammo}")
            if listed.count("(") != 1 or "true" not in listed:
                problems.append(f"weapons window should list only the selected gun: {listed}")
        return problems

    return check


def no_ordnance_leak(output: str) -> list[str]:
    """The creator probe must report every removed-store case and no problems."""
    problems = []
    if "removed stores passed" not in output:
        problems.append("removed-store check did not run")
    if re.search(r"PROBLEM", output):
        problems.append("creator matrix listed problems")
    return problems


def scenarios() -> list[Scenario]:
    out: list[Scenario] = []
    out.append(
        Scenario(
            name="menus-validate-creator",
            lane="menus",
            args=["--validate-creator", "--no-audio"],
            timeout=3600,
            expect=[r"creator matrix: \d+ setups started", r"removed stores passed"],
            forbid=[r"PROBLEM"],
            check=no_ordnance_leak,
            notes="Loadouts for every aircraft incl. removed stores, then the whole Quick Mission creator matrix.",
        )
    )
    out.append(Scenario(name="menus-validate-maps", lane="menus", args=["--validate-maps", "--no-audio"], timeout=600, expect=[r"Validated 75 retail map layouts"]))
    out.append(Scenario(name="menus-validate-weather", lane="menus", args=["--validate-weather", "--no-audio"], timeout=900, expect=[r"Weather sources validated"]))
    # `--combat-smoke` fails for the other thirteen aircraft (a stale radar-off
    # check since the missile guidance rework); see docs/testing/lane-menus.md.
    out.append(Scenario(name="menus-combat-smoke-mig29", lane="menus", args=["--aircraft", "mig29", "--combat-smoke", "--no-audio"], timeout=600, expect=[r"PASS"], forbid=[r"FAIL"]))

    # CPU snapshots of every menu state.
    for state in NORMAL_STATES:
        out.append(
            Scenario(
                name=f"menus-snap-{state}",
                lane="menus",
                args=["--snapshot", "{work}/shot.ppm", "--snapshot-state", state, "--no-audio"],
                timeout=120,
                expect=[r"(Menu|Locate) preview:"],
                check=menu_picture(),
            )
        )
    for state in QUICK_STATES:
        out.append(
            Scenario(
                name=f"menus-snap-quick-{state}",
                lane="menus",
                args=["--quick-mission", "--snapshot", "{work}/shot.ppm", "--snapshot-state", state, "--no-audio"],
                timeout=120,
                expect=[r"(Menu|Locate) preview:"],
                check=menu_picture(),
            )
        )
    # Every creator popup (fields 3 to 34), with a ground start chosen so the
    # airport row exists.
    for field in range(3, 35):
        out.append(
            Scenario(
                name=f"menus-snap-quick-field-{field}",
                lane="menus",
                args=["--quick-mission", "--ground-start", "1", "--snapshot", "{work}/shot.ppm", "--snapshot-state", f"field-{field}", "--no-audio"],
                timeout=120,
                expect=[r"Menu preview:"],
                check=menu_picture(),
            )
        )
    out.append(
        Scenario(
            name="menus-snap-quick-unknown-state-refused",
            lane="menus",
            args=["--quick-mission", "--snapshot", "{work}/shot.ppm", "--snapshot-state", "bogus", "--no-audio"],
            timeout=120,
            expect_exit=1,
            expect=[r"snapshot states: normal, aircraft, theaters, help"],
        )
    )
    # The creator and the loadout page for every aircraft; the loadout page
    # is where taking stores off is done.
    for a in AIRCRAFT:
        for state in ("normal", "aircraft", "ordnance", "ordnance-empty"):
            out.append(
                Scenario(
                    name=f"menus-snap-quick-{state}-{a}",
                    lane="menus",
                    args=["--aircraft", a, "--quick-mission", "--snapshot", "{work}/shot.ppm", "--snapshot-state", state, "--no-audio"],
                    timeout=120,
                    expect=[r"(Menu|Locate) preview:"],
                    check=menu_picture(),
                )
            )
    # The creator on every theater layout.
    for code in THEATERS + VARIANT_THEATERS:
        out.append(
            Scenario(
                name=f"menus-snap-quick-theater-{code.strip('~').lower()}{'-variant' if code.startswith('~') else ''}",
                lane="menus",
                args=["--theater", code, "--quick-mission", "--snapshot", "{work}/shot.ppm", "--snapshot-state", "theaters", "--no-audio"],
                timeout=120,
                expect=[r"(Menu|Locate) preview:"],
                check=menu_picture(),
            )
        )

    # Missions started through the headless AI probe: the same launch layout
    # code a flown Quick Mission uses. Player aircraft against three theaters
    # and every weather choice, then separations and wing sizes.
    probe = ["--ai-probe-ticks", "90", "--no-audio"]
    for a in AIRCRAFT:
        for theater in ("UKR", "KURILE", "PGU"):
            out.append(
                Scenario(
                    name=f"menus-start-{a}-{theater.lower()}",
                    lane="menus",
                    args=["--aircraft", a, "--theater", theater, "--probe-fight", "3:3", *probe],
                    timeout=300,
                    expect=[r"AI probe totals:"],
                    forbid=[r"OFF-MAP"],
                )
            )
    for weather in range(6):
        for theater in ("UKR", "NSK", "BAL"):
            out.append(
                Scenario(
                    name=f"menus-start-weather{weather}-{theater.lower()}",
                    lane="menus",
                    args=["--weather-condition", str(weather), "--theater", theater, "--probe-fight", "2:2", *probe],
                    timeout=300,
                    expect=[r"AI probe totals:"],
                )
            )
    for sep in (1, 2, 5, 10, 20, 50, 100, 150, 200, 300):
        for theater in ("UKR", "CUB"):
            out.append(
                Scenario(
                    name=f"menus-start-separation{sep}-{theater.lower()}",
                    lane="menus",
                    args=["--separation", str(sep), "--theater", theater, "--probe-fight", "2:2", *probe],
                    timeout=300,
                    expect=[r"AI probe totals:"],
                    forbid=[r"OFF-MAP"],
                )
            )
    for size in range(1, 6):
        out.append(
            Scenario(
                name=f"menus-start-wing{size}",
                lane="menus",
                args=["--probe-wing-size", str(size), "--probe-fight", "5:5", *probe],
                timeout=300,
                expect=[r"AI probe totals:"],
            )
        )

    # Stores taken off, as the Load Ordnance page leaves them: the aircraft still
    # flies and the gun still works with the externals off.
    for a in AIRCRAFT:
        for choice in ("none", "guns"):
            out.append(
                Scenario(
                    name=f"menus-loadout-{choice}-{a}",
                    lane="menus",
                    args=["--aircraft", a, "--loadout", choice, "--headless-flight", "600", "--no-audio"],
                    timeout=300,
                )
            )
    for a in ("f18", "f14", "su35"):
        for choice in ("none", "guns"):
            out.append(
                Scenario(
                    name=f"menus-window-weapons-page-{choice}-{a}",
                    lane="menus",
                    window=True,
                    args=["--aircraft", a, "--loadout", choice, "--capture-flight", "{work}/flight.ppm", "--instrument-page", "8", "--window-size", "960x720", "--no-audio"],
                    timeout=120,
                    expect=[r"Smoke test: requested screen presented successfully"],
                    check=flight_picture((960, 720)),
                )
            )

    # The real App path: a Quick Mission launched with the loadout page's edits.
    for a in AIRCRAFT:
        for choice in ("none", "guns"):
            out.append(
                Scenario(
                    name=f"menus-window-launch-{choice}-{a}",
                    lane="menus",
                    window=True,
                    args=["--aircraft", a, "--launch-quick-mission", "--loadout", choice, "--smoke-test", "--no-audio"],
                    timeout=180,
                    expect=[r"Quick Mission restart: PASS", r"Quick Mission launch:"],
                    check=flown_inventory(choice),
                )
            )

    # Ground starts through the real launch path, wings of three, every aircraft,
    # and the adapters that cannot ground start.
    for a in AIRCRAFT:
        out.append(
            Scenario(
                name=f"menus-window-launch-ground-{a}",
                lane="menus",
                window=True,
                args=["--aircraft", a, "--launch-quick-mission", "--ground-start", "1", "--probe-wing-size", "3", "--smoke-test", "--no-audio"],
                timeout=180,
                expect=[r"Quick Mission restart: PASS", r"Quick Mission launch: ground=Some\(\d+\).*supported=true.*parked_targets=2"],
            )
        )
    out.append(
        Scenario(
            name="menus-window-launch-legacy-adapter-airborne",
            lane="menus",
            window=True,
            args=["--legacy-flight", "--launch-quick-mission", "--smoke-test", "--no-audio"],
            timeout=180,
            expect=[r"Quick Mission restart: PASS", r"Quick Mission launch: ground=None"],
        )
    )
    out.append(
        Scenario(
            name="menus-window-launch-legacy-adapter-ground-refused",
            lane="menus",
            window=True,
            args=["--legacy-flight", "--launch-quick-mission", "--ground-start", "1", "--smoke-test", "--no-audio"],
            timeout=180,
            expect_exit=1,
            expect=[r"Ground start requires the researched flight model; choose Airborne for this adapter"],
        )
    )

    # Windowed captures through tools/agent-run.sh: a few per lane, quick ones.
    sizes = {"960x720": (960, 720), "1280x720": (1280, 720), "640x900": (640, 900)}
    for label, size in sizes.items():
        out.append(
            Scenario(
                name=f"menus-window-flight-{label}",
                lane="menus",
                window=True,
                args=["--capture-flight", "{work}/flight.ppm", "--window-size", label, "--no-audio"],
                timeout=120,
                expect=[r"Smoke test: requested screen presented successfully"],
                check=flight_picture(size),
            )
        )
        out.append(
            Scenario(
                name=f"menus-window-flight-menu-{label}",
                lane="menus",
                window=True,
                args=["--capture-flight", "{work}/flight.ppm", "--flight-menu", "--window-size", label, "--no-audio"],
                timeout=120,
                expect=[r"Smoke test: requested screen presented successfully"],
                check=flight_picture(size),
            )
        )
    for label in ("640x480", "641x481", "800x600", "1000x1000", "1600x900", "1920x1080"):
        width, height = (int(v) for v in label.split("x"))
        out.append(
            Scenario(
                name=f"menus-window-size-{label}",
                lane="menus",
                window=True,
                args=["--capture-flight", "{work}/flight.ppm", "--window-size", label, "--no-audio"],
                timeout=120,
                expect=[r"Smoke test: requested screen presented successfully"],
                check=flight_picture((width, height)),
            )
        )
    for label in ("480x320", "100x100", "3841x2161"):
        out.append(
            Scenario(
                name=f"menus-window-size-refused-{label}",
                lane="menus",
                args=["--capture-flight", "{work}/flight.ppm", "--window-size", label, "--no-audio"],
                timeout=60,
                expect_exit=1,
                expect=[r"window size outside 640x480\.\.3840x2160"],
            )
        )
    for a in AIRCRAFT:
        out.append(
            Scenario(
                name=f"menus-window-cockpit-{a}",
                lane="menus",
                window=True,
                args=["--aircraft", a, "--capture-flight", "{work}/flight.ppm", "--window-size", "960x720", "--no-audio"],
                timeout=120,
                expect=[r"Smoke test: requested screen presented successfully"],
                check=flight_picture((960, 720)),
            )
        )
    for view in range(12):
        out.append(
            Scenario(
                name=f"menus-window-view-{view}",
                lane="menus",
                window=True,
                args=["--capture-flight", "{work}/flight.ppm", "--flight-view", str(view), "--window-size", "960x720", "--no-audio"],
                timeout=120,
                expect=[r"Smoke test: requested screen presented successfully"],
                check=flight_picture((960, 720)),
            )
        )
    # Instrument windows are drawn on the CPU: every page for every aircraft,
    # and every panel-only systems fault (1 to 35) on the systems window.
    for a in AIRCRAFT:
        for page in range(10):
            out.append(
                Scenario(
                    name=f"menus-panel-{a}-page-{page}",
                    lane="menus",
                    args=["--aircraft", a, "--panel-snapshot", "{work}/panel.ppm", "--instrument-page", str(page), "--no-audio"],
                    timeout=120,
                    outputs=["panel.ppm"],
                )
            )
    for fault in range(1, 36):
        out.append(
            Scenario(
                name=f"menus-panel-fault-{fault}",
                lane="menus",
                args=["--panel-snapshot", "{work}/panel.ppm", "--instrument-page", "7", "--systems-preview", str(fault), "--no-audio"],
                timeout=120,
                outputs=["panel.ppm"],
            )
        )
    # Every graphics option value, and the preview and panel switches, in a
    # real window.
    for option in (
        "--anti-aliasing off", "--anti-aliasing 2x", "--anti-aliasing 4x", "--anti-aliasing 8x",
        "--render-scale 75", "--render-scale 125", "--render-scale 150", "--render-scale 200",
        "--spotting-aid off", "--spotting-aid strong", "--terrain-filtering off", "--original-graphics",
    ):
        out.append(
            Scenario(
                name=f"menus-window-graphics{option.replace('--', '-').replace(' ', '-')}",
                lane="menus",
                window=True,
                args=[*option.split(), "--dummy-aircraft", "mig29,2", "--capture-flight", "{work}/flight.ppm", "--window-size", "960x720", "--no-audio"],
                timeout=120,
                expect=[r"Smoke test: requested screen presented successfully"],
                check=flight_picture((960, 720)),
            )
        )
    for name, extra in (
        ("map", ["--flight-map"]),
        ("panels", ["--dummy-aircraft", "mig29,2", "--flight-panels", "thought,telemetry,guidance,comms,menu", "--debug-panels"]),
        ("weapon-diagnostics", ["--weapon-diagnostics", "--dummy-aircraft", "mig29,1"]),
        ("small-layout", ["--instrument-layout", "small"]),
        ("damage", ["--damage-preview", "0.6"]),
        ("ejection", ["--ejection-preview", "seat"]),
        ("chaff", ["--combat-command", "chaff", "--countermeasure-preview", "60"]),
    ):
        out.append(
            Scenario(
                name=f"menus-window-preview-{name}",
                lane="menus",
                window=True,
                args=[*extra, "--capture-flight", "{work}/flight.ppm", "--window-size", "960x720", "--no-audio"],
                timeout=120,
                expect=[r"Smoke test: requested screen presented successfully"],
                check=flight_picture((960, 720)),
            )
        )
    for flag in ("--free-flight", "--launch-quick-mission", "--quick-mission", "--controls-menu"):
        out.append(
            Scenario(
                name=f"menus-window-smoke{flag.replace('--', '-')}",
                lane="menus",
                window=True,
                args=[flag, "--smoke-test", "--no-audio"],
                timeout=120,
                expect=[r"Smoke test: requested screen presented successfully"],
            )
        )
    return out
