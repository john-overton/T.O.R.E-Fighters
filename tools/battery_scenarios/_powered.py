"""Flight lane: the six powered-lift aircraft (VTOL overhaul), flown by short scripted tapes.

The helicopters, the V-22 and the two vectoring jets fly their own physics, and the
app's headless pilot-input replay (`--replay-input TAPE`) is the simplest way to put
a scripted pilot on one. `--maneuver hover` starts an aircraft at rest in the air in
its own hover trim (hands off it holds), and the tapes fly the rest: a stick pulse and
hover hold, the transition to forward flight, the conversion corridor, an
autorotation, the vortex ring state and the Easy flight physics cheat. The tapes are
written at run time by `write_tapes` (a follow-up step), so nothing here is a data
file, and each is open loop: a short script on a fixed point, so the outcomes are
bands, not numbers. The checks read the probe's result line (`ticks=... speed_kt=...
altitude_ft=... crashed=...`, `heading_deg`, `bank_deg`, `non_finite`).

Scenario names are `flight-powered-<what>-<aircraft>`; see docs/testing/lane-flight.md.
"""
from __future__ import annotations

import re
from pathlib import Path

from battery import Scenario, Step
from battery_scenarios._replay_record import sections

ROOT = Path(__file__).resolve().parents[2]
PY = "python3"

HELICOPTERS = ["ah64", "mi24", "ch47"]
ROTORCRAFT = HELICOPTERS + ["v22"]
JETS = ["av8", "yak141"]
POWERED = ["av8", "yak141", "v22", "ah64", "mi24", "ch47"]

# A full-fuel Yak-141 has a 1.2 percent hover margin at sea level and none at the probe's 5,000 ft;
# with 6,000 lb of internal fuel it holds a hover (docs/baselines/vtol-overhaul.md).
EXTRA = {"yak141": ["--flight-fuel", "6000"]}

# The probe starts at 5,000 ft over terrain near 500 ft, and its start heading is 0.3 rad.
START_ALTITUDE = 5000.0
START_HEADING = 17.19


# ---------------------------------------------------------------------------
# The tapes
# ---------------------------------------------------------------------------


def tape_text(ticks: int, holds=(), commands=None) -> str:
    """A `tore-pilot 3` tape of `ticks` frames.

    `holds` are `(first, last, {field: value})` tick ranges setting `pitch`, `roll`, `yaw`, `throttle`,
    `collective` (a lever position) or `conversion` (a rate, 1 toward helicopter mode). `commands` maps a tick
    to the tape's command words (`toggle:hover-hold`, `trim:pitch:-0.04`, `lift-command:nozzle-step-up`...)."""
    commands = commands or {}
    lines = ["tore-pilot 3"]
    for tick in range(1, ticks + 1):
        field = {"pitch": 0.0, "roll": 0.0, "yaw": 0.0, "throttle": "-"}
        lift = {}
        for first, last, values in holds:
            if first <= tick <= last:
                for key, value in values.items():
                    if key == "collective":
                        lift["collective"] = f"0:{value}"
                    elif key == "conversion":
                        lift["conversion"] = f"{value}:-"
                    else:
                        field[key] = value
        words = [str(tick), str(field["pitch"]), str(field["roll"]), str(field["yaw"]), "0", str(field["throttle"])]
        words += [f"lift:{name}:{value}" for name, value in lift.items()]
        words += commands.get(tick, [])
        lines.append(" ".join(words))
    return "\n".join(lines) + "\n"


def _taps(first: int, count: int, word: str, every: int = 10) -> dict:
    return {first + every * k: [word] for k in range(count)}


ATTITUDE = "lift-command:stability-level=attitude"
OFF = "lift-command:stability-level=off"
FORWARD_TAP = "trim:pitch:-0.04"

# Taps of forward cyclic trim at the Attitude level that take each helicopter from a hover to 60 to 100 kt.
TAPS = {"ah64": 6, "mi24": 5, "ch47": 4}


def spec(kind: str, aircraft: str) -> tuple[int, list, dict]:
    """(ticks, holds, commands) of the tape `kind` for `aircraft`."""
    if kind == "quiet":
        return 1800, [], {}
    if kind == "pulse":  # a hard stick pulse from the hover, nothing after: the drift keeps growing
        return 1800, [(60, 150, {"pitch": -1.0, "roll": 0.6})], {}
    if kind == "hold":  # the same pulse, hover hold engaged while the aircraft is still drifting
        return 1800, [(60, 150, {"pitch": -1.0, "roll": 0.6})], {420: ["toggle:hover-hold"]}
    if kind == "transition":
        if aircraft in TAPS:
            return 3600, [], {1: [ATTITUDE]} | _taps(60, TAPS[aircraft], FORWARD_TAP)
        if aircraft == "v22":  # the conversion keys held toward airplane mode at 85 percent lever, nose a little down
            holds = [(1, 5000, {"collective": 0.85}), (60, 4800, {"conversion": -1}), (60, 120, {"pitch": -0.1})]
            return 4800, holds, {}
        if aircraft == "av8":  # the manual: nozzles three steps up, forward at 90 kt
            return 4800, [(1, 6000, {"throttle": 1})], {1: ["lift-command:nozzle-step-up"] * 3, 1000: ["lift-command:nozzle-preset-forward"]}
        return 4800, [(1, 7000, {"throttle": 1})], {1: ["lift-command:nozzle-step-up"] * 3, 1800: ["lift-command:nozzle-preset-forward"]}
    if kind == "corridor":  # airplane-mode start, the nacelles asked toward helicopter mode at 175 knots
        return 3000, [(60, 2400, {"conversion": 1})], {}
    if kind == "autorotation":  # engine out at the start speed, lever down, the Attitude level holding the attitude
        return 6000, [(60, 9000, {"collective": 0.35})], {1: [ATTITUDE], 60: ["set:engine:0"]}
    if kind == "vrs":  # a sinking hover (lever 0.4), then full collective
        return 1560, [(1, 9000, {"collective": 0.4}), (600, 9000, {"collective": 1.0})], {1: [ATTITUDE]}
    if kind == "torque":  # stability Off, a 30 percent collective step from the hover
        return 300, [], {1: [OFF], 60: ["axis-adjust:collective:0.3"]}
    if kind == "roll":  # stability Off, a roll pulse in a jet's hover
        return 300, [(60, 90, {"roll": 1.0})], {1: [OFF]}
    raise KeyError(kind)


def write_tapes(work: str, aircraft: str, *kinds: str) -> None:
    """Writes `{work}/{kind}.tape` for each kind (run as a follow-up step)."""
    for kind in kinds:
        ticks, holds, commands = spec(kind, aircraft)
        Path(work, f"{kind}.tape").write_text(tape_text(ticks, holds, commands))


def _write_step(aircraft: str, *kinds: str) -> Step:
    code = (
        f"import sys; sys.path.insert(0, {str(ROOT / 'tools')!r}); "
        "from battery_scenarios._powered import write_tapes; write_tapes(sys.argv[1], sys.argv[2], *sys.argv[3:])"
    )
    return Step([PY, "-c", code, "{work}", aircraft, *kinds], app=False)


def _fly(aircraft: str, kind: str, *extra: str, maneuver: str = "hover") -> Step:
    args = ["--replay-input", f"{{work}}/{kind}.tape", "--maneuver", maneuver, "--aircraft", aircraft, "--researched-flight", "--no-audio"]
    return Step(args + EXTRA.get(aircraft, []) + list(extra), timeout=180)


# ---------------------------------------------------------------------------
# Reading the result line
# ---------------------------------------------------------------------------


def _result(text: str) -> dict | None:
    found = re.search(r"ticks=(\d+) speed_kt=(\S+) altitude_ft=(\S+) fuel_lb=\S+ crashed=(\w+)", text)
    if not found:
        return None
    heading = re.search(r"heading_deg=(\S+)", text)
    bank = re.search(r"bank_deg=(\S+)", text)
    overspeed = re.search(r"overspeed_ticks=(\d+)", text)
    non_finite = re.search(r"non_finite=(\d+)", text)
    return {
        "ticks": int(found[1]),
        "speed": float(found[2]),
        "altitude": float(found[3]),
        "crashed": found[4] == "true",
        "heading": float(heading[1]) if heading else float("nan"),
        "bank": float(bank[1]) if bank else float("nan"),
        "overspeed_ticks": int(overspeed[1]) if overspeed else 0,
        "non_finite": int(non_finite[1]) if non_finite else 0,
    }


def _flights(output: str, runs: int) -> tuple[list[dict], list[str]]:
    """The results of the app runs among a scenario's steps (step 1 writes the tapes, steps 2.. fly them)."""
    parts = sections(output)
    results, problems = [], []
    for step in range(2, 2 + runs):
        result = _result(parts.get(step, ""))
        if result is None:
            problems.append(f"flight {step - 1} printed no result line")
            continue
        results.append(result)
        if result["crashed"]:
            problems.append(f"flight {step - 1} crashed")
        if result["non_finite"]:
            problems.append(f"flight {step - 1} had {result['non_finite']} non-finite samples")
    return results, problems


def _angle(a: float, b: float) -> float:
    return (a - b + 180.0) % 360.0 - 180.0


# ---------------------------------------------------------------------------
# The scenarios
# ---------------------------------------------------------------------------


def check_hover(output: str) -> list[str]:
    results, problems = _flights(output, 1)
    for r in results:
        if r["ticks"] != 1800:
            problems.append(f"the hover flew {r['ticks']} ticks, not 1,800")
        if r["speed"] > 3.0:
            problems.append(f"hands off, the hover drifted to {r['speed']:.1f} kt")
        if abs(r["altitude"] - START_ALTITUDE) > 30:
            problems.append(f"hands off, the hover left its height: {r['altitude']:.0f} ft")
    return problems


def hover_scenarios() -> list[Scenario]:
    return [
        Scenario(
            name=f"flight-powered-hover-{ac}", lane="flight", args=["--version"],
            then=[_write_step(ac, "quiet"), _fly(ac, "quiet")],
            check=check_hover, timeout=240,
        )
        for ac in POWERED
    ]


def check_hover_hold(output: str) -> list[str]:
    results, problems = _flights(output, 2)
    if len(results) == 2:
        held, loose = results
        if held["speed"] > 3.0:
            problems.append(f"hover hold left a drift of {held['speed']:.1f} kt")
        if not 4950 <= held["altitude"] <= 5010:
            problems.append(f"hover hold left its height: {held['altitude']:.0f} ft")
        if loose["speed"] < 15.0:
            problems.append(f"without hover hold the stick pulse drifted only {loose['speed']:.1f} kt: the scenario proves nothing")
    return problems


def hover_hold_scenarios() -> list[Scenario]:
    """Hover hold is the helicopters' and the V-22's (the jets refuse it by design, unit test A2)."""
    return [
        Scenario(
            name=f"flight-powered-hover-hold-{ac}", lane="flight", args=["--version"],
            then=[_write_step(ac, "hold", "pulse"), _fly(ac, "hold"), _fly(ac, "pulse")],
            check=check_hover_hold, timeout=300,
        )
        for ac in ROTORCRAFT
    ]


# (minimum speed, minimum altitude, maximum altitude) after the transition script.
TRANSITION_BANDS = {
    "ah64": (40.0, 4600.0, 5500.0),
    "mi24": (40.0, 4600.0, 5500.0),
    "ch47": (40.0, 4400.0, 5500.0),
    "v22": (100.0, 3000.0, 6500.0),
    "av8": (250.0, 3000.0, 5200.0),
    "yak141": (200.0, 4000.0, 5200.0),
}


def transition_check(aircraft: str):
    minimum_speed, low, high = TRANSITION_BANDS[aircraft]

    def check(output: str) -> list[str]:
        results, problems = _flights(output, 1)
        for r in results:
            if r["speed"] < minimum_speed:
                problems.append(f"the transition ended at {r['speed']:.0f} kt, under {minimum_speed:.0f}")
            if not low <= r["altitude"] <= high:
                problems.append(f"the transition ended at {r['altitude']:.0f} ft, outside {low:.0f} to {high:.0f}")
            if r["overspeed_ticks"] > 600:
                problems.append(f"the transition ran {r['overspeed_ticks']} ticks over the limit")
        return problems

    return check


def transition_scenarios() -> list[Scenario]:
    """Hover to forward flight: the helicopters by cyclic trim at the Attitude level, the V-22 by converting its
    nacelles, the jets by the manual's nozzle steps."""
    return [
        Scenario(
            name=f"flight-powered-transition-{ac}", lane="flight", args=["--version"],
            then=[_write_step(ac, "transition"), _fly(ac, "transition")],
            check=transition_check(ac), timeout=300,
        )
        for ac in POWERED
    ]


def check_corridor(output: str) -> list[str]:
    results, problems = _flights(output, 1)
    for r in results:
        if r["speed"] < 150.0:
            problems.append(f"asked for helicopter mode at 175 kt the V-22 slowed to {r['speed']:.0f} kt")
        if not 4500 <= r["altitude"] <= 5600:
            problems.append(f"the V-22 left its height: {r['altitude']:.0f} ft")
        if r["overspeed_ticks"] > 0:
            problems.append(f"{r['overspeed_ticks']} ticks over the overspeed limit: the corridor did not protect the nacelles")
    return problems


def corridor_scenarios() -> list[Scenario]:
    return [Scenario(
        name="flight-powered-corridor-v22", lane="flight", args=["--version"],
        then=[_write_step("v22", "corridor"), _fly("v22", "corridor", maneuver="level")],
        check=check_corridor, timeout=240,
    )]


def check_autorotation(output: str) -> list[str]:
    results, problems = _flights(output, 1)
    for r in results:
        drop = 4999.0 - r["altitude"]
        if not 1500 <= drop <= 3500:
            problems.append(f"the autorotation lost {drop:.0f} ft in 50 s, outside 1,500 to 3,500")
        if not 90 <= r["speed"] <= 160:
            problems.append(f"the autorotation ended at {r['speed']:.0f} kt, outside 90 to 160")
    return problems


def autorotation_scenarios() -> list[Scenario]:
    return [Scenario(
        name="flight-powered-autorotation-ah64", lane="flight", args=["--version"],
        then=[_write_step("ah64", "autorotation"), _fly("ah64", "autorotation", maneuver="level")],
        check=check_autorotation, timeout=240,
    )]


def check_vrs(output: str) -> list[str]:
    results, problems = _flights(output, 2)
    if len(results) == 2:
        plain, easy = results
        if easy["altitude"] - plain["altitude"] < 100.0:
            problems.append(
                f"full collective from a sinking hover lost {plain['altitude']:.0f} ft with the hazards and "
                f"{easy['altitude']:.0f} ft with Easy flight physics: the vortex ring state shows no difference"
            )
    return problems


def vrs_scenarios() -> list[Scenario]:
    return [Scenario(
        name="flight-powered-vrs-ah64", lane="flight", args=["--version"],
        then=[_write_step("ah64", "vrs"), _fly("ah64", "vrs"), _fly("ah64", "vrs", "--flight-cheat", "easy-physics")],
        check=check_vrs, timeout=300,
    )]


def easy_check(aircraft: str):
    def check(output: str) -> list[str]:
        results, problems = _flights(output, 2)
        if len(results) != 2:
            return problems
        plain, easy = results
        if aircraft in JETS:
            if plain["bank"] < 15.0:
                problems.append(f"at stability Off the roll pulse banked the jet only {plain['bank']:.1f} degrees: no hazard to remove")
            if abs(easy["bank"]) > 12.0:
                problems.append(f"with Easy flight physics the jet still banked {easy['bank']:.1f} degrees")
        else:
            torque = abs(_angle(plain["heading"], START_HEADING))
            easy_turn = abs(_angle(easy["heading"], START_HEADING))
            if aircraft in ("ah64", "mi24") and torque < 15.0:
                problems.append(f"the collective step turned the nose only {torque:.1f} degrees without the cheat: no torque to remove")
            if easy_turn > 5.0:
                problems.append(f"with Easy flight physics a collective step turned the nose {easy_turn:.1f} degrees")
        return problems

    return check


def easy_physics_scenarios() -> list[Scenario]:
    """The Easy flight physics cheat removes the hazards: torque on the single-rotor helicopters, the low-speed
    roll-off and undamped puffers on the jets; the CH-47 and V-22 have no torque to remove and must not turn."""
    out = []
    for ac in POWERED:
        kind = "roll" if ac in JETS else "torque"
        out.append(Scenario(
            name=f"flight-powered-easy-{ac}", lane="flight", args=["--version"],
            then=[_write_step(ac, kind), _fly(ac, kind), _fly(ac, kind, "--flight-cheat", "easy-physics")],
            check=easy_check(ac), timeout=300,
        ))
    return out


def powered_scenarios() -> list[Scenario]:
    return (
        hover_scenarios()
        + hover_hold_scenarios()
        + transition_scenarios()
        + corridor_scenarios()
        + autorotation_scenarios()
        + vrs_scenarios()
        + easy_physics_scenarios()
    )
