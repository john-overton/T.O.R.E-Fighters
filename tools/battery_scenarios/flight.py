"""Lane: takeoff to landing, flight model, weapons, countermeasures, damage.

Every scenario is one `tore-app` run. The headless ones use the flight probe
(`--headless-flight`), which prints `extremes:`, `landing:` and
`spin_recovery:` lines this file reads for impossible states. The windowed ones
(`window=True`) go through tools/agent-run.sh and are limited by the runner.
See docs/testing/lane-flight.md.
"""
import os
import re

from battery import Scenario

# The fourteen flyable identities, `AircraftId::SELECTABLE`.
AIRCRAFT = ["f18", "rafale", "f14", "a4e", "x31", "mig29", "su27", "mig21", "su25", "mig23", "su35", "f22", "f22n", "faxx"]
# Aircraft whose PT disables spins (X-31 flag, F-22 family): they never enter one.
SPIN_IMMUNE = {"x31", "f22", "f22n", "faxx"}

# Ground-start airports per theater (`--ground-start 1..N`), counted from the imported maps.
AIRPORTS = {
    "APA": 20, "BAL": 34, "CUB": 23, "EGY": 25, "FRA": 32, "GRE": 13, "IRA": 21, "KURILE": 4,
    "LFA": 5, "NSK": 22, "PGU": 21, "SPA": 21, "TVIET": 10, "UKR": 14, "VLA": 26, "WTA": 16,
    "~APAF": 2, "~BAL0": 34, "~BAL1": 34, "~BAL2": 34, "~BAL3": 34, "~BAL4": 34, "~BAL5": 34,
    "~BAL6": 34, "~BAL7": 34, "~CUBF": 2, "~EGY1": 25, "~EGY2": 25, "~EGY3": 25, "~EGY4": 25,
    "~EGY5": 25, "~EGY6": 25, "~EGY7": 25, "~EGY8": 26, "~EGY9": 25, "~EGYF": 2, "~FRA0": 32,
    "~FRA1": 32, "~FRA2": 32, "~FRA3": 32, "~FRA4": 32, "~FRA5": 32, "~FRA6": 32, "~FRA7": 32,
    "~FRA8": 32, "~FRA9": 32, "~LFAF": 1, "~SPAF": 1, "~UKR1": 14, "~UKR2": 13, "~UKR3": 14,
    "~UKR4": 14, "~UKR5": 14, "~UKR6": 14, "~UKR7": 14, "~UKR8": 14, "~VLA1": 26, "~VLA2": 26,
    "~VLA3": 26, "~VLA4": 26, "~VLA5": 26, "~VLA6": 26, "~VLA7": 26, "~VLA8": 26, "~VLAF": 1,
}
BASE_THEATERS = [t for t in AIRPORTS if not t.startswith("~")]
VARIANT_THEATERS = [t for t in AIRPORTS if t.startswith("~")]

# Gun capacity and the other station capacities of each aircraft's default load,
# in the order `--weapon-slot 1..N` selects them (from the combat smoke).
STATIONS = {
    "f18": [570, 2, 4, 4, 2],
    "rafale": [250, 4, 2, 2, 2],
    "f14": [675, 4, 2, 2],
    "a4e": [400, 4, 2],
    "x31": [740, 2, 2, 2],
    "mig29": [150, 2, 2, 2],
    "su27": [150, 2, 2, 4, 2],
    "mig21": [200, 2, 2],
    "su25": [250, 2, 4, 4],
    "mig23": [200, 2, 2, 2],
    "su35": [150, 1, 2, 4, 2],
    "f22": [750, 2, 2, 4],
}
STATIONS["f22n"] = STATIONS["f22"]
STATIONS["faxx"] = STATIONS["f22"]


def _numbers(output: str, prefix: str) -> dict:
    """`key=value` pairs of the last output line that starts with `prefix`."""
    lines = [ln for ln in output.splitlines() if ln.startswith(prefix)]
    if not lines:
        return {}
    return dict(re.findall(r"(\w+)=(\S+)", lines[-1]))


def _plain_numbers(output: str) -> dict:
    """`key=value` pairs from the probe's plain result lines."""
    found = {}
    for line in output.splitlines():
        if line.startswith(("trace:", "landing", "spin_recovery", "systems:", "extremes:")):
            continue
        for key, value in re.findall(r"(\w+)=(\S+)", line):
            found[key] = value
    return found


def extremes_problems(output: str, engine_off_ok: bool = False, beyond_envelope_ok: bool = False) -> list[str]:
    """Impossible values in the headless flight probe's `extremes:` line.

    `beyond_envelope_ok` skips the speed and altitude limits, for the climb
    scenario that measures how far past its envelope an aircraft can go."""
    e = _numbers(output, "extremes:")
    if not e:
        return ["no extremes: line (the probe did not finish)"]
    problems = []
    try:
        if int(e["non_finite"]) != 0:
            problems.append(f"{e['non_finite']} non-finite samples")
        if float(e["fuel_rise_lb"]) > 0.01:
            problems.append(f"fuel rose by {e['fuel_rise_lb']} lb")
        if float(e["dead_stick_gain_ft"]) > 25 and not engine_off_ok:
            problems.append(f"energy gained with the engine off: {e['dead_stick_gain_ft']} ft")
        if float(e["dead_stick_gain_ft"]) > 200:
            problems.append(f"large energy gain with the engine off: {e['dead_stick_gain_ft']} ft")
        if float(e["max_speed_kt"]) > 1300 and not beyond_envelope_ok:
            problems.append(f"impossible speed {e['max_speed_kt']} kt")
        if float(e.get("speed_over_envelope_top", 0)) > 1.02 and not beyond_envelope_ok:
            problems.append(f"flew faster than the aircraft's own top speed: {e['speed_over_envelope_top']} of it")
        if not (-12 <= float(e["min_g"]) and float(e["max_g"]) <= 16):
            problems.append(f"impossible load {e['min_g']}..{e['max_g']} G")
        if float(e["max_altitude_ft"]) > 70000 and not beyond_envelope_ok:
            problems.append(f"impossible altitude {e['max_altitude_ft']} ft")
        if float(e["min_altitude_ft"]) < -100:
            problems.append(f"under the ground: {e['min_altitude_ft']} ft")
        if int(e.get("under_ground_ticks", 0)) > 0:
            problems.append(f"{e['under_ground_ticks']} ticks more than 30 ft below the surface (lowest {e.get('min_agl_ft')} ft)")
        for veil in ("max_blackout", "max_redout"):
            if not 0 <= float(e.get(veil, 0)) <= 1:
                problems.append(f"{veil} {e[veil]} outside 0..1")
        if float(e.get("max_blackout", 0)) > 0 and float(e["max_g"]) < 5:
            problems.append(f"the view greyed out at {e['max_g']} G")
        if float(e.get("max_redout", 0)) > 0 and float(e["min_g"]) > -2:
            problems.append(f"the view reddened at {e['min_g']} G")
        if float(e.get("energy_rate_over_thrust", 0)) > 1.0:
            problems.append(f"energy gained faster than the thrust allows: {e['energy_rate_over_thrust']} of thrust power")
        if float(e["max_pitch_rate_dps"]) > 120 or float(e["max_roll_rate_dps"]) > 720:
            problems.append(f"impossible rates pitch {e['max_pitch_rate_dps']} roll {e['max_roll_rate_dps']} deg/s")
    except (KeyError, ValueError) as error:
        problems.append(f"unreadable extremes line: {error}")
    return problems


# ---------------------------------------------------------------- takeoff


def check_takeoff(output: str) -> list[str]:
    problems = extremes_problems(output)
    n = _plain_numbers(output)
    if "takeoff_complete=true" not in output:
        problems.append("never reached 100 ft (stuck on the runway or crashed)")
    if n.get("crashed") != "false":
        problems.append("crashed during takeoff")
    try:
        ticks = int(n["ticks"])
        if not 300 <= ticks <= 4500:
            problems.append(f"takeoff took {ticks} ticks")
        speed = float(n["speed_kt"])
        if not 60 <= speed <= 260:
            problems.append(f"liftoff speed {speed} kt")
    except (KeyError, ValueError):
        problems.append("no result line")
    return problems


def _theater_tag(theater: str) -> str:
    return theater.lstrip("~").lower() + ("v" if theater.startswith("~") else "")


def takeoff_scenarios() -> list[Scenario]:
    out = []
    seen = set()

    def add(ac: str, theater: str, n: int) -> None:
        name = f"flight-takeoff-{ac}-{_theater_tag(theater)}-{n}"
        if name in seen:
            return
        seen.add(name)
        out.append(
            Scenario(
                name=name,
                lane="flight",
                args=["--theater", theater, "--ground-start", str(n), "--headless-flight", "9000", "--maneuver", "takeoff", "--aircraft", ac, "--no-audio"],
                check=check_takeoff,
                timeout=120,
            )
        )

    # Every aircraft from the first and the middle airport of every base theater.
    for ac in AIRCRAFT:
        for theater in BASE_THEATERS:
            add(ac, theater, 1)
            add(ac, theater, max(1, AIRPORTS[theater] // 2))
    # The Hornet from every airport of every base theater.
    for theater in BASE_THEATERS:
        for n in range(1, AIRPORTS[theater] + 1):
            add("f18", theater, n)
    # The Hornet from the first airport of every imported variant.
    for theater in VARIANT_THEATERS:
        add("f18", theater, 1)
    return out


# ---------------------------------------------------------------- landing


def _landing(output: str) -> tuple[dict, float]:
    landing = _numbers(output, "landing:")
    start = re.search(r"runway_length_ft=(\d+)", output)
    return landing, float(start.group(1)) if start else 0.0


def check_landing(output: str, overrun_ok_ft: float = 0.0) -> list[str]:
    """`overrun_ok_ft` allows a longer roll (a tailwind landing)."""
    problems = extremes_problems(output)
    landing, length = _landing(output)
    if not landing:
        return problems + ["no landing: line"]
    # The scripted landing floats to about 1,500 ft past the threshold, so only
    # a long runway is expected to hold it; a short strip may overrun.
    if length >= 5500:
        if landing.get("touchdown") != "true":
            problems.append("never touched down")
        if landing.get("crashed") != "false":
            problems.append(f"crashed on a {length:.0f} ft runway: {landing.get('unsafe')}")
        if landing.get("stopped") != "true":
            problems.append("did not come to a stop")
        elif float(landing.get("runway_left_ft", "-1")) < -overrun_ok_ft:
            problems.append(f"rolled {landing['runway_left_ft']} ft off the end of a {length:.0f} ft runway")
        if landing.get("unsafe", "none") != "none":
            problems.append(f"unsafe touchdown: {landing['unsafe']}")
        if landing.get("touchdown") == "true":
            if float(landing["touchdown_sink_fps"]) > 12:
                problems.append(f"hard touchdown, {landing['touchdown_sink_fps']} ft/s")
            if abs(float(landing["touchdown_bank_deg"])) > 5:
                problems.append(f"touchdown bank {landing['touchdown_bank_deg']} deg")
            if landing.get("left_runway") != "false" and overrun_ok_ft == 0:
                problems.append(f"rolled off the runway surface at {landing.get('left_runway_kt')} kt")
    else:
        # A short strip: landing, overrunning off the end or a crash off the
        # runway are all fine, but never a gear failure with the gear down.
        if "gear_up=true" in landing.get("unsafe", "none"):
            problems.append("gear-up crash on an approach flown with the gear down")
    return problems


def check_landing_gear_up(output: str) -> list[str]:
    problems = extremes_problems(output)
    landing, _ = _landing(output)
    if landing.get("crashed") != "true" or "gear_up=true" not in landing.get("unsafe", ""):
        problems.append(f"a gear-up landing did not crash for the gear: {landing.get('unsafe')}")
    return problems


def check_landing_off_runway(output: str) -> list[str]:
    problems = extremes_problems(output)
    landing, _ = _landing(output)
    if landing.get("crashed") != "true" or "not_landable=true" not in landing.get("unsafe", ""):
        problems.append(f"touching down beside the runway did not crash: {landing.get('unsafe')}")
    return problems


def check_landing_hard(output: str) -> list[str]:
    problems = extremes_problems(output)
    landing, _ = _landing(output)
    if landing.get("touchdown") != "true" and landing.get("crashed") != "true":
        problems.append("a steep touchdown neither landed nor crashed")
    return problems


def landing_scenarios() -> list[Scenario]:
    out = []
    seen = set()

    def add(ac: str, theater: str, n: int, tag: str = "", maneuver: str = "land", check=check_landing, env=None) -> None:
        name = f"flight-land{tag}-{ac}-{_theater_tag(theater)}-{n}"
        if name in seen:
            return
        seen.add(name)
        out.append(
            Scenario(
                name=name,
                lane="flight",
                args=["--theater", theater, "--ground-start", str(n), "--headless-flight", "60000", "--maneuver", maneuver, "--aircraft", ac, "--no-audio"],
                check=check,
                env=env or {},
                timeout=180,
            )
        )

    # The Hornet at every airport of every base theater.
    for theater in BASE_THEATERS:
        for n in range(1, AIRPORTS[theater] + 1):
            add("f18", theater, n)
    # Every other aircraft at the first airport of every base theater.
    for ac in AIRCRAFT:
        if ac != "f18":
            for theater in BASE_THEATERS:
                add(ac, theater, 1)
    # Variants: the Hornet at the first airport of each.
    for theater in VARIANT_THEATERS:
        add("f18", theater, 1)
    # Unsafe approaches, every aircraft at UKR airport 1.
    for ac in AIRCRAFT:
        add(ac, "UKR", 1, "-gearup", "land-gear-up", check_landing_gear_up)
        add(ac, "UKR", 1, "-offrunway", "land-off-runway", check_landing_off_runway)
        add(ac, "UKR", 1, "-hard", "land-hard", check_landing_hard)
    # Wind: head, tail and crosswinds of 20 to 60 ft/s (12 to 36 knots).
    for ac in AIRCRAFT:
        for heading, speed in [(0, 30), (180, 30), (90, 30), (270, 20), (45, 60)]:
            # Heading 0 is a tailwind on this runway: a longer roll is fair.
            check = (lambda out: check_landing(out, 400.0)) if heading == 0 else check_landing
            add(ac, "UKR", 1, f"-wind{heading}-{speed}", "land", check, {"TORE_WIND": f"{heading},{speed}"})
    return out


# --------------------------------------------------------------- manoeuvres

# The manoeuvres that hold the controls until the end: a rolling or banked pull
# with no pitch hold can fly into the ground, which is the script's doing.
MAY_CRASH = {"roll", "spin", "bank-left", "bank-right"}


def make_check_maneuver(maneuver: str):
    def check(output: str) -> list[str]:
        problems = extremes_problems(output, engine_off_ok=maneuver in {"stall", "spin"})
        n = _plain_numbers(output)
        if maneuver not in MAY_CRASH and n.get("crashed") != "false":
            problems.append("crashed in a manoeuvre that should stay in the air")
        try:
            ticks = int(n["ticks"])
            if maneuver in {"level", "pull", "stall"} and ticks != 7200:
                problems.append(f"ended at tick {ticks}")
            if maneuver == "loop" and n.get("loop_completed") != "true":
                problems.append("did not complete the loop")
        except KeyError:
            problems.append("no result line")
        return problems

    return check


def maneuver_scenarios() -> list[Scenario]:
    out = []
    adapters = [("default", []), ("legacy", ["--legacy-flight"]), ("researched", ["--researched-flight"])]
    for ac in AIRCRAFT:
        for maneuver in ["level", "pull", "loop", "roll", "stall", "spin", "bank-left", "bank-right"]:
            for label, flags in adapters:
                out.append(
                    Scenario(
                        name=f"flight-{maneuver}-{ac}-{label}",
                        lane="flight",
                        args=["--headless-flight", "7200", "--aircraft", ac, "--maneuver", maneuver, "--no-audio", *flags],
                        check=make_check_maneuver(maneuver),
                        timeout=120,
                    )
                )
    return out


def check_spin_recovery(ac: str):
    def check(output: str) -> list[str]:
        problems = extremes_problems(output, engine_off_ok=True)
        s = _numbers(output, "spin_recovery:")
        if not s:
            return problems + ["no spin_recovery: line"]
        entered = s.get("entered_tick") != "never"
        if ac in SPIN_IMMUNE:
            if entered:
                problems.append("spin-immune aircraft entered a spin")
            return problems
        if not entered:
            problems.append("could not be put into a spin with wrong rudder and full back stick")
            return problems
        if s.get("recovered_tick") == "never":
            problems.append("the manual's spin recovery did not recover the aircraft")
        if s.get("crashed") != "false":
            problems.append("crashed in the spin recovery")
        if float(s["revolutions_during_recovery"]) > 3:
            problems.append(f"needed {s['revolutions_during_recovery']} revolutions to recover")
        if float(s["altitude_lost_ft"]) > 8000:
            problems.append(f"lost {s['altitude_lost_ft']} ft in the spin")
        return problems

    return check


def spin_scenarios() -> list[Scenario]:
    return [
        Scenario(
            name=f"flight-spinrecover-{ac}",
            lane="flight",
            args=["--headless-flight", "12000", "--aircraft", ac, "--maneuver", "spin-recover", "--no-audio"],
            check=check_spin_recovery(ac),
            timeout=120,
        )
        for ac in AIRCRAFT
    ]


def check_stall_recovery(output: str) -> list[str]:
    problems = extremes_problems(output, engine_off_ok=True)
    s = _numbers(output, "stall_recovery:")
    if not s:
        return problems + ["no stall_recovery: line"]
    if s.get("alert_tick") == "never":
        problems.append("a full-back-stick pull at low speed never raised a stall alert")
    elif s.get("recovered_tick") == "never":
        problems.append("nose down with the afterburner in did not clear the stall")
    if s.get("crashed") != "false":
        problems.append("crashed in the stall recovery")
    return problems


def stall_scenarios() -> list[Scenario]:
    return [
        Scenario(
            name=f"flight-stallrecover-{ac}",
            lane="flight",
            args=["--headless-flight", "20000", "--aircraft", ac, "--maneuver", "stall-recover", "--no-audio"],
            check=check_stall_recovery,
            timeout=120,
        )
        for ac in AIRCRAFT
    ]


# ------------------------------------------------------------ system faults


def check_faults(output: str) -> list[str]:
    problems = extremes_problems(output, engine_off_ok=True)
    if not _numbers(output, "systems:"):
        problems.append("no systems: line")
    return problems


def fault_scenarios() -> list[Scenario]:
    out = []
    # Every fault index 0..44 on three different aircraft, in a sustained pull.
    for ac in ["f18", "su25", "f22"]:
        for index in range(45):
            out.append(
                Scenario(
                    name=f"flight-fault{index:02d}-{ac}",
                    lane="flight",
                    args=["--headless-flight", "5400", "--aircraft", ac, "--maneuver", "pull", "--flight-fault", f"240:{index}", "--no-audio"],
                    check=check_faults,
                    timeout=120,
                )
            )
    # Every fault at once, and one after another, in every aircraft.
    for ac in AIRCRAFT:
        out.append(
            Scenario(
                name=f"flight-faultall-{ac}",
                lane="flight",
                args=["--headless-flight", "7200", "--aircraft", ac, "--maneuver", "level", *[a for i in range(45) for a in ("--flight-fault", f"240:{i}")], "--no-audio"],
                check=check_faults,
                timeout=120,
            )
        )
        out.append(
            Scenario(
                name=f"flight-faultseq-{ac}",
                lane="flight",
                args=["--headless-flight", "7200", "--aircraft", ac, "--maneuver", "loop", *[a for i in range(45) for a in ("--flight-fault", f"{120 + 4 * i}:{i}")], "--no-audio"],
                check=check_faults,
                timeout=120,
            )
        )
    return out


# ------------------------------------------------------------------ combat


def check_combat_smoke(output: str) -> list[str]:
    problems = []
    passes = len(re.findall(r"\bPASS\b", output))
    if passes < 8:
        problems.append(f"only {passes} PASS lines")
    for line in output.splitlines():
        if "FAIL" in line and "PASS" not in line:
            problems.append(line.strip()[:200])
    return problems


# Aircraft with no reviewed guided air-to-air missile in the default load.
NO_MISSILES = {"a4e", "mig23"}


def make_check_missile_acceptance(ac: str):
    def check(output: str) -> list[str]:
        if "weapon,mode,launcher_fps" not in output:
            return ["no acceptance table"]
        rows = [ln for ln in output.splitlines() if ".JT," in ln]
        if not rows:
            return [] if ac in NO_MISSILES else ["no acceptance rows"]
        return check_missile_rows(rows)

    return check


def check_missile_rows(rows: list[str]) -> list[str]:
    problems = []
    for row in rows:
        cols = row.split(",")
        # weapon, mode, range, target, motion, fraction, feet, outcome, seconds, closest
        if len(cols) < 10:
            problems.append(f"short row: {row[:100]}")
            continue
        try:
            if float(cols[8]) < 0:
                problems.append(f"negative flight time: {row[:100]}")
        except ValueError:
            problems.append(f"unreadable row: {row[:100]}")
    return problems


def combat_scenarios() -> list[Scenario]:
    out = []
    for ac in AIRCRAFT:
        out.append(Scenario(name=f"flight-combatsmoke-{ac}", lane="flight", args=["--combat-smoke", "--aircraft", ac, "--no-audio"], check=check_combat_smoke, timeout=300))
        out.append(Scenario(name=f"flight-missileacceptance-{ac}", lane="flight", args=["--missile-acceptance", "--aircraft", ac, "--no-audio"], check=make_check_missile_acceptance(ac), timeout=1200))
    out.append(Scenario(name="flight-sensor-summary", lane="flight", args=["--sensor-summary", "--no-audio"], expect=[r"radar .* search"], timeout=120))
    out.append(Scenario(name="flight-validate-weather", lane="flight", args=["--validate-weather", "--no-audio"], timeout=600))
    out.append(Scenario(name="flight-validate-maps", lane="flight", args=["--validate-maps", "--no-audio"], timeout=600))
    return out


# --------------------------------------------------------------- windowed


# Stations whose weapon is a surface missile or similar and refuses the aircraft
# fixture, so the trigger must not spend a round (docs/spec/missiles.md).
REFUSED_SLOTS = {
    ("f18", 3), ("f18", 4), ("f22", 3), ("f22n", 3), ("faxx", 3), ("mig23", 3), ("mig23", 4),
    ("rafale", 2), ("su25", 3), ("x31", 3),
}


def check_slot(ac: str, slot: int):
    capacity = STATIONS[ac]
    refused = (ac, slot) in REFUSED_SLOTS

    def check(output: str) -> list[str]:
        problems = []
        m = re.search(r"Combat probe: .* shots=(\d+) hits=(\d+) kills=(\d+) active=(\d+) ammo=\[([\d, ]+)\]", output)
        if not m:
            return ["no Combat probe line"]
        shots, hits, kills = (int(m.group(i)) for i in range(1, 4))
        ammo = [int(v) for v in m.group(5).split(",")]
        if len(ammo) != len(capacity):
            return [f"{len(ammo)} stations, expected {len(capacity)}"]
        for index, (left, cap) in enumerate(zip(ammo, capacity)):
            if left < 0 or left > cap:
                problems.append(f"station {index + 1} ammo {left} outside 0..{cap}")
        if refused and shots != 0:
            problems.append(f"a surface weapon fired at the practice aircraft ({shots} shots)")
        if not refused and shots == 0:
            problems.append("the trigger fired nothing")
        fired_slot = capacity[slot - 1] - ammo[slot - 1]
        if fired_slot != shots:
            problems.append(f"fired {shots} but the station lost {fired_slot}")
        for index, (left, cap) in enumerate(zip(ammo, capacity)):
            if index != slot - 1 and left != cap:
                problems.append(f"station {index + 1} changed ({cap}->{left}) while firing station {slot}")
        if hits > shots or kills > hits:
            problems.append(f"impossible tally shots={shots} hits={hits} kills={kills}")
        return problems

    return check


def slot_scenarios() -> list[Scenario]:
    out = []
    for ac, capacity in STATIONS.items():
        if ac in {"f22n", "faxx"}:
            continue
        for slot in range(1, len(capacity) + 1):
            out.append(
                Scenario(
                    name=f"flight-livefire-{ac}-slot{slot}",
                    lane="flight",
                    args=["--live-fire", "--aircraft", ac, "--weapon-slot", str(slot), "--combat-probe-ticks", "1200", "--capture-flight", "{work}/shot.ppm", "--no-audio"],
                    window=True,
                    check=check_slot(ac, slot),
                    timeout=180,
                )
            )
    return out


def check_countermeasures(output: str) -> list[str]:
    m = re.search(
        r"Countermeasure preview: ticks=\d+ flares=(\d+) burning=(\d+) puffs=(\d+) chaff=(\d+) carried_chaff=(\d+) carried_flares=(\d+) capacity_chaff=(\d+) capacity_flares=(\d+)",
        output,
    )
    if not m:
        return ["no Countermeasure preview line"]
    flares, burning, puffs, chaff, carried_chaff, carried_flares, cap_chaff, cap_flares = (int(m.group(i)) for i in range(1, 9))
    problems = []
    if burning > flares:
        problems.append("more flares burning than flares in the air")
    if carried_chaff > cap_chaff or carried_flares > cap_flares:
        problems.append("more countermeasures carried than the aircraft's capacity")
    # Five chaff releases and three flare releases were commanded.
    if carried_chaff != max(cap_chaff - 5, 0):
        problems.append(f"5 chaff releases but chaff {cap_chaff} -> {carried_chaff}")
    if carried_flares != max(cap_flares - 3, 0):
        problems.append(f"3 flare releases but flares {cap_flares} -> {carried_flares}")
    if cap_chaff >= 5 and chaff != 5:
        problems.append(f"{chaff} chaff cartridges in the air, expected 5")
    return problems


def countermeasure_scenarios() -> list[Scenario]:
    out = []
    for ac in AIRCRAFT:
        args = ["--live-fire", "--aircraft", ac]
        args += ["--combat-command", "chaff"] * 5
        args += ["--combat-command", "flare"] * 3
        args += ["--countermeasure-preview", "240", "--capture-flight", "{work}/cm.ppm", "--no-audio"]
        out.append(Scenario(name=f"flight-countermeasures-{ac}", lane="flight", args=args, window=True, check=check_countermeasures, timeout=180))
    return out


# ---------------------------------------------------------------- climb-out


def check_climbout(output: str) -> list[str]:
    """The scripted leader takes off, raises gear and flaps and cruises."""
    problems = []
    if "player crashed=false" not in output:
        problems.append("the player crashed or the probe did not finish")
    if "player: airborne, gear and flaps up" not in output:
        problems.append("never got airborne")
    if "levelling off at" not in output:
        problems.append("never reached cruise height")
    if not re.search(r"AI probe liftoff gaps: \[\] hazards_open=0", output):
        problems.append("ground hazards or liftoff gaps reported")
    if "OFF-MAP" in output:
        problems.append("a wing member started off the map")
    return problems


def climbout_scenarios() -> list[Scenario]:
    out = []
    for ac in AIRCRAFT:
        out.append(
            Scenario(
                name=f"flight-climbout-{ac}",
                lane="flight",
                args=["--theater", "UKR", "--ground-start", "3", "--ai-probe-ticks", "16000", "--maneuver", "takeoff", "--probe-wing-size", "1", "--probe-wing-only", "--aircraft", ac, "--no-audio"],
                check=check_climbout,
                timeout=240,
            )
        )
    # A whole wing of five leaving the first airport of every base theater.
    for theater in BASE_THEATERS:
        out.append(
            Scenario(
                name=f"flight-climbout-wing5-{_theater_tag(theater)}",
                lane="flight",
                args=["--theater", theater, "--ground-start", "1", "--ai-probe-ticks", "9000", "--maneuver", "takeoff", "--probe-wing-size", "5", "--probe-wing-only", "--no-audio"],
                check=check_climbout,
                timeout=240,
            )
        )
    return out


# --------------------------------------------------------------- autopilot


def check_autopilot(output: str) -> list[str]:
    problems = extremes_problems(output)
    n = _plain_numbers(output)
    e = _numbers(output, "extremes:")
    try:
        if abs(float(n["bank_deg"])) > 2:
            problems.append(f"autopilot left {n['bank_deg']} deg of bank")
        if abs(float(n["altitude_ft"]) - 5000) > 100:
            problems.append(f"autopilot did not hold 5,000 ft: {n['altitude_ft']}")
        if float(e["min_altitude_ft"]) < 4850:
            problems.append(f"autopilot let the aircraft sink to {e['min_altitude_ft']} ft")
        if float(e["max_g"]) > 2:
            problems.append(f"autopilot pulled {e['max_g']} G")
    except (KeyError, ValueError):
        problems.append("no result line")
    return problems


def autopilot_scenarios() -> list[Scenario]:
    return [
        Scenario(
            name=f"flight-autopilot-{ac}",
            lane="flight",
            args=["--headless-flight", "3600", "--aircraft", ac, "--maneuver", "autopilot", "--no-audio"],
            check=check_autopilot,
            timeout=120,
        )
        for ac in AIRCRAFT
    ]


# ---------------------------------------------------------------- ejection


def make_check_eject(maneuver: str):
    def check(output: str) -> list[str]:
        problems = extremes_problems(output, engine_off_ok=True)
        m = re.search(r"^ejection=(\w+) pilot_alive=(\w+) pilot_position=\[([^\]]+)\]", output, re.M)
        if not m:
            return problems + ["no ejection line (the seat never fired)"]
        phase, alive, position = m.group(1), m.group(2), [float(v) for v in m.group(3).split(",")]
        if phase != "Landed":
            problems.append(f"the pilot was still {phase} after the run")
        if maneuver == "eject" and alive != "true":
            problems.append("the pilot died ejecting at 5,000 ft in level flight")
        if abs(position[1]) > 60:
            problems.append(f"the pilot came to rest at {position[1]:.0f} ft above the ground")
        return problems

    return check


def eject_scenarios() -> list[Scenario]:
    return [
        Scenario(
            name=f"flight-{maneuver}-{ac}",
            lane="flight",
            args=["--headless-flight", "30000", "--aircraft", ac, "--maneuver", maneuver, "--no-audio"],
            check=make_check_eject(maneuver),
            timeout=120,
        )
        for ac in AIRCRAFT
        for maneuver in ("eject", "eject-low")
    ]


# ----------------------------------------------------------------- weather


def frame_problems(output: str, night_ok: bool = False) -> list[str]:
    """Read the captured frame the app names and flag a blank or uniform picture."""
    m = re.search(r"Scene capture: (\S+)", output)
    if not m:
        return ["no Scene capture line"]
    path = m.group(1)
    if not os.path.isabs(path):
        path = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), path)
    try:
        with open(path, "rb") as f:
            data = f.read()
    except OSError as error:
        return [f"capture unreadable: {error}"]
    header = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", data)
    if not header:
        return ["capture is not a P6 PPM"]
    width, height = int(header.group(1)), int(header.group(2))
    pixels = data[header.end():]
    if len(pixels) < width * height * 3:
        return ["capture is truncated"]
    step = 3 * 37
    sample = pixels[: width * height * 3 : step]
    if len(set(sample[i : i + 3] for i in range(0, len(sample) - 2, 3))) < 40:
        return ["the frame is a flat colour (blank render)"]
    mean = sum(sample) / len(sample)
    if mean < 2 and not night_ok:
        return [f"the frame is black (mean {mean:.1f})"]
    return []


def weather_scenarios() -> list[Scenario]:
    out = []
    for theater in BASE_THEATERS:
        for condition in range(6):
            out.append(
                Scenario(
                    name=f"flight-weather{condition}-{_theater_tag(theater)}",
                    lane="flight",
                    args=["--free-flight", "--theater", theater, "--weather-condition", str(condition), "--capture-flight", "{work}/frame.ppm", "--no-audio"],
                    window=True,
                    check=lambda out, night=(condition == 5): frame_problems(out, night),
                    timeout=180,
                )
            )
    return out


# ------------------------------------------------------------------ cheats


def make_check_cheat(cheat: str):
    def check(output: str) -> list[str]:
        problems = extremes_problems(output, engine_off_ok=True)
        n = _plain_numbers(output)
        e = _numbers(output, "extremes:")
        try:
            if cheat == "extra-g":
                if not 8.5 <= float(e["max_g"]) <= 9.5:
                    problems.append(f"Pull extra G reached {e['max_g']} G, expected about 9")
            elif cheat == "no-g-effects":
                if float(e["max_blackout"]) != 0 or float(e["max_redout"]) != 0:
                    problems.append("No redout or blackout on but the view still greyed or reddened")
            elif cheat == "no-spins":
                if n.get("spin_direction") != "0" or "Spinning" in n.get("departure_alert", ""):
                    problems.append("No spins on but the aircraft spun")
            elif cheat == "no-crashes":
                if n.get("crashed") != "false":
                    problems.append("No crashes on but the aircraft crashed")
            elif cheat == "unlimited-fuel":
                if abs(float(e["fuel_end_lb"]) - float(e["fuel_start_lb"])) > 0.01:
                    problems.append("Unlimited fuel on but the fuel went down")
        except (KeyError, ValueError):
            problems.append("no result line")
        return problems

    return check


def cheat_scenarios() -> list[Scenario]:
    out = []
    plan = [("extra-g", "pull"), ("no-g-effects", "pull"), ("no-spins", "spin"), ("no-crashes", "roll"), ("unlimited-fuel", "loop")]
    for ac in AIRCRAFT:
        for cheat, maneuver in plan:
            out.append(
                Scenario(
                    name=f"flight-cheat-{cheat}-{ac}",
                    lane="flight",
                    args=["--headless-flight", "7200", "--aircraft", ac, "--maneuver", maneuver, "--flight-cheat", cheat, "--no-audio"],
                    check=make_check_cheat(cheat),
                    timeout=120,
                )
            )
    return out


def check_unlimited_ammo(ac: str, slot: int):
    capacity = STATIONS[ac]

    def check(output: str) -> list[str]:
        m = re.search(r"Combat probe: .* shots=(\d+) hits=(\d+) kills=(\d+) active=(\d+) ammo=\[([\d, ]+)\]", output)
        if not m:
            return ["no Combat probe line"]
        shots = int(m.group(1))
        ammo = [int(v) for v in m.group(5).split(",")]
        problems = []
        if shots == 0:
            problems.append("nothing was fired")
        # The gun's count is set by the first round; a missile station keeps its full count.
        if slot > 1 and ammo[slot - 1] != capacity[slot - 1]:
            problems.append(f"Unlimited ammo on but station {slot} went {capacity[slot - 1]} -> {ammo[slot - 1]}")
        if slot == 1 and ammo[0] < capacity[0] - 1:
            problems.append(f"Unlimited ammo on but the gun went {capacity[0]} -> {ammo[0]}")
        return problems

    return check


def make_check_damage_cheat(mode: str):
    def check(output: str) -> list[str]:
        m = re.search(r"\| HP (\d+) SYS (\S+) ", output)
        if not m:
            return ["no status line with HP"]
        hp, sys = int(m.group(1)), m.group(2)
        problems = []
        if mode == "invulnerable":
            if hp == 0 or "LAUNCHER LOST" in output:
                problems.append(f"Invulnerable but the aircraft was destroyed (HP {hp})")
        elif mode == "normal":
            if sys != "--":
                problems.append(f"Normal damage but system faults appeared: SYS {sys}")
        elif mode == "realistic":
            if sys == "--" and hp < 232:
                problems.append("Realistic damage took hit points but caused no system fault")
        return problems

    return check


def cheat_combat_scenarios() -> list[Scenario]:
    out = []
    # The first air-to-air missile station of each aircraft (the Rafale's second
    # station is a surface missile that refuses the practice aircraft).
    for ac, missile_slot in [("f18", 2), ("rafale", 3), ("f14", 2), ("mig29", 2), ("su27", 2), ("f22", 2)]:
        for slot in (1, missile_slot):
            out.append(
                Scenario(
                    name=f"flight-cheat-unlimited-ammo-{ac}-slot{slot}",
                    lane="flight",
                    args=["--live-fire", "--aircraft", ac, "--weapon-slot", str(slot), "--flight-cheat", "unlimited-ammo", "--combat-probe-ticks", "1200", "--capture-flight", "{work}/c.ppm", "--no-audio"],
                    window=True,
                    check=check_unlimited_ammo(ac, slot),
                    timeout=180,
                )
            )
    for ac in ["f18", "su27", "f22"]:
        for mode, flags in [("invulnerable", ["--flight-cheat", "invulnerable"]), ("normal", []), ("realistic", ["--flight-cheat", "realistic-damage"])]:
            if mode == "realistic" and ac != "f18":
                continue
            out.append(
                Scenario(
                    name=f"flight-cheat-damage-{mode}-{ac}",
                    lane="flight",
                    args=["--live-fire", "--aircraft", ac, "--weapon-slot", "2", *flags, "--combat-command", "incoming", "--combat-command", "incoming", "--combat-probe-ticks", "1500", "--capture-flight", "{work}/c.ppm", "--no-audio"],
                    window=True,
                    check=make_check_damage_cheat(mode),
                    timeout=180,
                )
            )
    return out


def check_waypoint(output: str) -> list[str]:
    """The waypoint autopilot turns right toward waypoint 1 (60,000 ft out, 60
    degrees right of north) from a heading of 17 degrees and holds the altitude
    it captured. After 25 seconds it has turned well toward it without
    overshooting past the bearing."""
    problems = extremes_problems(output)
    f = _numbers(output, "final_position:")
    e = _numbers(output, "extremes:")
    try:
        heading = float(f["heading_deg"])
        if not 30 <= heading <= 75:
            problems.append(f"heading {heading:.1f} after 25 s, expected a turn from 17 toward 60 degrees")
        if float(e["max_altitude_ft"]) - float(e["min_altitude_ft"]) > 80:
            problems.append("the altitude hold wandered more than 80 ft")
    except (KeyError, ValueError):
        problems.append("no result line")
    return problems


def waypoint_scenarios() -> list[Scenario]:
    return [
        Scenario(
            name=f"flight-waypoint-{ac}",
            lane="flight",
            args=["--headless-flight", "3000", "--aircraft", ac, "--maneuver", "waypoint", "--no-audio"],
            check=check_waypoint,
            timeout=120,
        )
        for ac in AIRCRAFT
    ]


def check_climb(output: str) -> list[str]:
    """A full-power climb and the dive after it. Today's aircraft go well past
    their own 1 G envelope: up to 1.65 times the ceiling and 1.95 times the top
    speed (see "Needs a decision" in the lane page). These limits are that
    behaviour with a margin, so a change for the worse is caught; they are not a
    specification."""
    problems = extremes_problems(output, engine_off_ok=True, beyond_envelope_ok=True)
    c = _numbers(output, "climb:")
    e = _numbers(output, "extremes:")
    if not c:
        return problems + ["no climb: line"]
    try:
        if float(c["ceiling_ft"]) <= 0:
            problems.append("the aircraft has no envelope ceiling")
        if float(c["over_ceiling"]) > 1.7:
            problems.append(f"climbed to {c['max_altitude_ft']} ft, {c['over_ceiling']} of its {c['ceiling_ft']} ft ceiling")
        if float(c["max_altitude_ft"]) < 0.6 * float(c["ceiling_ft"]):
            problems.append(f"could only climb to {c['max_altitude_ft']} ft of a {c['ceiling_ft']} ft ceiling")
        if float(e["speed_over_envelope_top"]) > 2.0:
            problems.append(f"reached {e['speed_over_envelope_top']} times the envelope's top speed")
    except (KeyError, ValueError):
        problems.append("no result line")
    return problems


def check_sprint(output: str) -> list[str]:
    """Full afterburner in level flight at 5,000 ft settles near the aircraft's
    own top speed instead of running away."""
    problems = extremes_problems(output, engine_off_ok=True, beyond_envelope_ok=True)
    e = _numbers(output, "extremes:")
    try:
        if float(e["speed_over_envelope_top"]) > 1.08:
            problems.append(f"level afterburner flight reached {e['speed_over_envelope_top']} times the envelope's top speed")
        # Near the top of its envelope only the 1 G row is left, and a loaded
        # aircraft cannot hold 1 G there: it sinks (see "Needs a decision"), so
        # the altitude hold is not checked once the speed is above 90% of it.
        if float(e["speed_over_envelope_top"]) <= 0.9 and float(e["max_altitude_ft"]) - float(e["min_altitude_ft"]) > 80:
            problems.append("the autopilot altitude hold wandered more than 80 ft in the sprint")
    except (KeyError, ValueError):
        problems.append("no result line")
    return problems


def climb_scenarios() -> list[Scenario]:
    return [
        Scenario(
            name=f"flight-climb-{ac}",
            lane="flight",
            args=["--headless-flight", "216000", "--aircraft", ac, "--maneuver", "climb", "--no-audio"],
            check=check_climb,
            timeout=240,
        )
        for ac in AIRCRAFT
    ]


def sprint_scenarios() -> list[Scenario]:
    return [
        Scenario(
            name=f"flight-sprint-{ac}",
            lane="flight",
            args=["--headless-flight", "36000", "--aircraft", ac, "--maneuver", "sprint", "--no-audio"],
            check=check_sprint,
            timeout=240,
        )
        for ac in AIRCRAFT
    ]


# -------------------------------------------------------------------- fuel


def check_fuel_out(output: str) -> list[str]:
    """Run out of fuel in level flight: the fuel stops at zero and the dead
    engine gives no energy."""
    problems = extremes_problems(output)
    e = _numbers(output, "extremes:")
    n = _plain_numbers(output)
    try:
        if abs(float(e["fuel_end_lb"])) > 1e-6:
            problems.append(f"25 lb of fuel did not run out in 100 seconds of level flight: {e['fuel_end_lb']} lb left")
        if float(n["fuel_lb"]) < 0:
            problems.append("negative fuel")
        if n.get("crashed") != "false":
            problems.append("crashed after running out of fuel in level flight at 5,000 ft")
    except (KeyError, ValueError):
        problems.append("no result line")
    return problems


def fuel_scenarios() -> list[Scenario]:
    return [
        Scenario(
            name=f"flight-fuelout-{ac}",
            lane="flight",
            args=["--headless-flight", "12000", "--aircraft", ac, "--maneuver", "level", "--flight-fuel", "25", "--no-audio"],
            check=check_fuel_out,
            timeout=120,
        )
        for ac in AIRCRAFT
    ]


# ----------------------------------------------------------------- devices


def check_devices(output: str) -> list[str]:
    problems = extremes_problems(output)
    d = _numbers(output, "devices:")
    if not d:
        return problems + ["no devices: line"]
    if d["violations"] != "0":
        problems.append(f"device moved wrongly: {d['first_violation']}")
    travel = float(d["deployment_seconds"])
    for name in ("gear", "flaps", "brake", "hook"):
        if name == "hook" and d["hook_available"] != "true":
            if d["hook_down_s"] != "none":
                problems.append("an aircraft with no hook lowered one")
            continue
        for way in ("down", "up"):
            value = d[f"{name}_{way}_s"]
            if value == "none":
                problems.append(f"{name} never finished going {way}")
            elif abs(float(value) - travel) > 0.1:
                problems.append(f"{name} took {value} s going {way}, the aircraft's travel is {travel} s")
    return problems


def device_scenarios() -> list[Scenario]:
    return [
        Scenario(
            name=f"flight-devices-{ac}",
            lane="flight",
            args=["--headless-flight", "3600", "--aircraft", ac, "--maneuver", "devices", "--no-audio"],
            check=check_devices,
            timeout=120,
        )
        for ac in AIRCRAFT
    ]


# ----------------------------------------------------------- damage visuals


def capture_scenarios() -> list[Scenario]:
    """Windowed captures of the damage, ejection and animation fixtures, each
    checked for a blank frame. The images themselves need a human eye."""
    out = []

    def add(name: str, args: list[str]) -> None:
        out.append(
            Scenario(
                name=f"flight-{name}",
                lane="flight",
                args=[*args, "--capture-flight", "{work}/frame.ppm", "--no-audio"],
                window=True,
                check=frame_problems,
                timeout=180,
            )
        )

    for ac in AIRCRAFT:
        for fraction in ("0.5", "1"):
            add(f"damage-{ac}-{fraction}", ["--free-flight", "--aircraft", ac, "--flight-view", "1", "--damage-preview", fraction, "--damage-preview-section", "core"])
        for pose in ("seat", "freefall", "chute"):
            add(f"ejectionpose-{ac}-{pose}", ["--free-flight", "--aircraft", ac, "--flight-view", "1", "--ejection-preview", pose])
    for section in ("nose", "cockpit", "core", "left-wing", "right-wing", "tail"):
        for fraction in ("0.3", "0.7", "1"):
            add(f"damage-f18-{section}-{fraction}", ["--free-flight", "--aircraft", "f18", "--flight-view", "1", "--damage-preview", fraction, "--damage-preview-section", section])
    for ac in AIRCRAFT:
        add(f"groundstart-{ac}", ["--free-flight", "--aircraft", ac, "--theater", "UKR", "--ground-start", "1", "--flight-view", "1", "--flight-look", "150,-15", "--flight-zoom", "1.5"])
    for bay in ("0", "0.5", "1"):
        add(f"bay-f22-{bay}", ["--free-flight", "--aircraft", "f22", "--flight-view", "1", "--flight-look", "150,-30", "--flight-bay", bay])
    return out


# ------------------------------------------------------------- world edges

# Terrain grid (columns, rows) of each base theater; the map runs 0 to
# (cells - 1) * 8,192 feet on each axis.
GRIDS = {
    "APA": (256, 256), "BAL": (256, 256), "CUB": (256, 256), "EGY": (208, 200), "FRA": (208, 200),
    "GRE": (256, 256), "IRA": (256, 256), "KURILE": (256, 256), "LFA": (256, 256), "NSK": (256, 256),
    "PGU": (256, 256), "SPA": (256, 256), "TVIET": (200, 200), "UKR": (208, 200), "VLA": (208, 200),
    "WTA": (256, 256),
}


def check_edge(output: str) -> list[str]:
    """Flying straight out over a map edge at 20,000 ft: the aircraft must keep
    flying with finite numbers. Nothing stops it leaving the map (see "Needs a
    decision")."""
    problems = extremes_problems(output)
    n = _plain_numbers(output)
    if n.get("crashed") != "false":
        problems.append("crashed flying out over the map edge at 20,000 ft")
    if "final_position:" not in output:
        problems.append("no final position")
    return problems


def edge_scenarios() -> list[Scenario]:
    out = []
    for theater, (cols, rows) in GRIDS.items():
        width, depth = (cols - 1) * 8192, (rows - 1) * 8192
        for edge, x, z, heading in [
            ("west", 20000, depth // 2, 270),
            ("east", width - 20000, depth // 2, 90),
            ("south", width // 2, 20000, 180),
            ("north", width // 2, depth - 20000, 0),
        ]:
            out.append(
                Scenario(
                    name=f"flight-edge-{edge}-{_theater_tag(theater)}",
                    lane="flight",
                    args=["--theater", theater, "--headless-flight", "24000", "--flight-start", f"{x},{z},{heading},20000", "--no-audio"],
                    check=check_edge,
                    timeout=180,
                )
            )
    return out


def check_terrain_crash(output: str) -> list[str]:
    """A spin or a roll at 90 ft over any theater hits the ground: the aircraft
    crashes, and never sinks through the surface."""
    problems = extremes_problems(output, engine_off_ok=True)
    n = _plain_numbers(output)
    e = _numbers(output, "extremes:")
    if n.get("crashed") != "true":
        problems.append("a spin or roll 90 ft above the ground did not crash")
    try:
        if int(e["under_ground_ticks"]) != 0:
            problems.append(f"{e['under_ground_ticks']} ticks more than 30 ft below the surface")
        if float(e["min_agl_ft"]) < -30:
            problems.append(f"went {e['min_agl_ft']} ft below the surface")
    except (KeyError, ValueError):
        problems.append("no terrain figures in the extremes line")
    return problems


def terrain_scenarios() -> list[Scenario]:
    out = []
    for theater, (cols, rows) in GRIDS.items():
        x, z = (cols - 1) * 4096, (rows - 1) * 4096
        for maneuver in ("spin", "roll"):
            out.append(
                Scenario(
                    name=f"flight-terrain-{maneuver}-{_theater_tag(theater)}",
                    lane="flight",
                    args=["--theater", theater, "--headless-flight", "24000", "--maneuver", maneuver, "--flight-start", f"{x},{z},300,90", "--no-audio"],
                    check=check_terrain_crash,
                    timeout=180,
                )
            )
    return out


# -------------------------------------------------------- instrument panels


def panel_scenarios() -> list[Scenario]:
    out = []
    # Every instrument page of every aircraft, and the Systems page under each
    # panel fault the preview accepts (1..35).
    for ac in AIRCRAFT:
        for page in range(10):
            out.append(
                Scenario(
                    name=f"flight-panel-{ac}-page{page}",
                    lane="flight",
                    args=["--panel-snapshot", "{work}/p.ppm", "--instrument-page", str(page), "--aircraft", ac, "--no-audio"],
                    outputs=["p.ppm"],
                    timeout=120,
                )
            )
    for fault in range(1, 36):
        out.append(
            Scenario(
                name=f"flight-panelfault{fault:02d}",
                lane="flight",
                args=["--panel-snapshot", "{work}/p.ppm", "--instrument-page", "7", "--systems-preview", str(fault), "--flight-probe-ticks", "1200", "--no-audio"],
                outputs=["p.ppm"],
                expect=[r"Damage \d+% \| TEMP \d+% OIL \d+% HYD \d+% \| Power \d+%"],
                timeout=120,
            )
        )
    return out


def daytime_scenarios() -> list[Scenario]:
    """One frame at every hour of the day in three theaters."""
    out = []
    for theater in ("UKR", "TVIET", "KURILE"):
        for hour in range(24):
            out.append(
                Scenario(
                    name=f"flight-hour{hour:02d}-{_theater_tag(theater)}",
                    lane="flight",
                    args=["--free-flight", "--theater", theater, "--capture-flight", "{work}/frame.ppm", "--no-audio"],
                    env={"TORE_WEATHER_TIME": f"{hour:02d}:00"},
                    window=True,
                    check=lambda out, night=(hour < 5 or hour >= 21): frame_problems(out, night),
                    timeout=180,
                )
            )
    return out


def scenarios() -> list[Scenario]:
    return (
        takeoff_scenarios()
        + landing_scenarios()
        + maneuver_scenarios()
        + spin_scenarios()
        + stall_scenarios()
        + fault_scenarios()
        + combat_scenarios()
        + slot_scenarios()
        + countermeasure_scenarios()
        + climbout_scenarios()
        + autopilot_scenarios()
        + eject_scenarios()
        + weather_scenarios()
        + cheat_scenarios()
        + panel_scenarios()
        + cheat_combat_scenarios()
        + device_scenarios()
        + edge_scenarios()
        + terrain_scenarios()
        + capture_scenarios()
        + daytime_scenarios()
        + fuel_scenarios()
        + waypoint_scenarios()
        + climb_scenarios()
        + sprint_scenarios()
    )
