"""Hand-flown coverage: windowed missions driven by `--input-script` files.

Each script in scripts/ presses the game's own keys through the same handlers
the window's events use (see crates/tore-app/src/input_script.rs), so the real
input path, the flight, the weapons and the mission recorder all run. The
recording the flight leaves is then read back and checked for the events the
keys should have caused.
"""
from __future__ import annotations

import json
from pathlib import Path

from battery import Scenario
from battery_scenarios._replay_keys import pause_problems
from battery_scenarios._replay_live import live_scenario

SCRIPTS = Path(__file__).resolve().parent / "scripts"


def load(work: Path) -> tuple[list[dict], list[dict]]:
    """(events, samples of the player) from the log the live steps wrote."""
    log = work / "log" / "log.jsonl"
    if not log.exists():
        return [], []
    lines = [json.loads(line) for line in log.read_text().splitlines()]
    events = [d for d in lines if d["type"] == "event"]
    samples = [d for d in lines if d["type"] == "sample" and d["id"] == 0]
    return events, samples


def first_time(samples: list[dict], test, after: float = 0.0) -> float | None:
    for d in samples:
        if d["t"] >= after and test(d):
            return d["t"]
    return None


def device_cycle(samples: list[dict], name: str, opened_after: float, closed_after: float) -> list[str]:
    """A device opens after one key press and closes again after the next."""
    up = first_time(samples, lambda d: d["devices"][name] > 0.9, opened_after)
    if up is None:
        return [f"{name} never opened after the key"]
    down = first_time(samples, lambda d: d["devices"][name] < 0.1, max(up, closed_after))
    return [] if down is not None else [f"{name} never closed again after the second key"]


def count(events: list[dict], kind: str, subject: int | None = 0, **fields) -> int:
    return sum(
        1
        for e in events
        if e["kind"] == kind
        and (subject is None or e.get("subject") == subject)
        and all(e.get("fields", {}).get(k) == v for k, v in fields.items())
    )


def systems_check(work: Path, output: str) -> list[str]:
    events, samples = load(work)
    problems = []
    if not samples:
        return ["no samples of the player"]
    at = first_time(samples, lambda d: d["controls"]["throttle"] > 0.99)
    if at is None or at > 4:
        problems.append("key 5 did not bring the throttle to 100 percent within 4 s")
    problems += device_cycle(samples, "gear", 1.5, 5.8)
    problems += device_cycle(samples, "flaps", 7.5, 12.5)
    problems += device_cycle(samples, "hook", 7.5, 12.5)
    problems += device_cycle(samples, "brake", 7.5, 12.5)
    burner = first_time(samples, lambda d: "afterburner" in d["flags"], 17.0)
    if burner is None:
        problems.append("key 6 did not light the afterburner")
    elif first_time(samples, lambda d: "afterburner" not in d["flags"], burner) is None:
        problems.append("key 5 did not put the afterburner out again")
    if count(events, "combat.countermeasure", 0, decoy="chaff") != 1:
        problems.append("expected exactly one chaff release from the player")
    if count(events, "combat.countermeasure", 0, decoy="flare") != 1:
        problems.append("expected exactly one flare release from the player")
    return problems


def missile_check(work: Path, output: str) -> list[str]:
    events, samples = load(work)
    problems = []
    if count(events, "player.command", 0, command="designate") < 1:
        problems.append("key t did not designate a target")
    if count(events, "player.command", 0, command="selection-next") < 1:
        problems.append("key ] did not change the selection")
    if count(events, "weapon.launch", 0, **{"class": "missile"}) < 1:
        problems.append("the player fired no missile")
    if count(events, "combat.countermeasure", 0) < 2:
        problems.append("chaff and flare were not both released")
    return problems


def gun_check(work: Path, output: str) -> list[str]:
    events, _ = load(work)
    rounds = count(events, "weapon.launch", 0, **{"class": "gun"})
    return [] if rounds >= 20 else [f"the held trigger fired {rounds} gun rounds, expected at least 20"]


def eject_check(work: Path, output: str) -> list[str]:
    events, samples = load(work)
    problems = []
    if count(events, "aircraft.ejected", 0) < 1:
        problems.append("Shift+E twice did not eject the player")
    if count(events, "audio.ejection", 0) < 1:
        problems.append("no ejection sound event")
    if samples and "ejected" not in samples[-1]["flags"]:
        problems.append("the last sample does not show the pilot ejected")
    return problems


def takeoff_check(work: Path, output: str) -> list[str]:
    events, samples = load(work)
    problems = []
    took = [e for e in events if e["kind"] == "aircraft.took_off" and e.get("subject") == 0]
    if len(took) != 1:
        return ["expected exactly one takeoff by the player"]
    t = took[0]["t"]
    if not 5 <= t <= 16:
        problems.append(f"the player took off at {t} s, outside the expected 5 to 16 s")
    gear_up = first_time(samples, lambda d: d["devices"]["gear"] < 0.1, t)
    if gear_up is None:
        problems.append("the gear did not come up after takeoff")
    if samples[-1]["pos_ft"][1] < 150:
        problems.append("the player is still low at the end of the climb")
    if not any("Airborne" in (e.get("text") or "") for e in events if e["kind"] == "comms.hud"):
        problems.append("the tower did not call the player airborne")
    after = [e for e in events if e["kind"] == "comms.tower" and e["t"] > 15 and e["fields"].get("outcome") == "delivered"]
    if not after:
        problems.append("the landing request (Shift+N, Shift+L) got no tower reply")
    return problems


def nav_check(work: Path, output: str) -> list[str]:
    events, samples = load(work)
    return [] if samples and len(samples) > 5 else ["no flight recorded"]


def views_check(work: Path, output: str) -> list[str]:
    return []


def pause_check(work: Path, output: str) -> list[str]:
    events, _ = load(work)
    problems = []
    if count(events, "player.bookmark", 0) != 3:
        problems.append("expected three bookmarks")
    if count(events, "system.pause", None) < 2:
        problems.append("Ctrl+P and the menu should each have paused the flight")
    log = work / "log" / "log.jsonl"
    if log.exists():
        problems += pause_problems(log.read_text())
    return problems


CHEATS = [
    "unlimited_ammo", "unlimited_fuel", "easy_aiming", "no_crashes", "no_spins", "no_turbulence", "extra_g",
    "ignore_weapon_weights", "no_sun_whiteout", "no_g_effects", "no_screen_shake", "ignore_midair_collisions",
    "easy_targeting", "guns_only",
]


def cheats_check(work: Path, output: str) -> list[str]:
    events, _ = load(work)
    problems = []
    state: dict[str, bool] = {}
    seen: dict[str, list[bool]] = {}
    for e in events:
        if e["kind"] == "system.cheat":
            f = e["fields"]
            seen.setdefault(f["cheat"], []).append(f["on"])
            state[f["cheat"]] = f["on"]
    for name in [*CHEATS, "invulnerable", "realistic_damage", "enemy_ai"]:
        got = seen.get(name)
        if not got:
            problems.append(f"cheat {name} never changed")
        elif got[0] is not True or state[name]:
            problems.append(f"cheat {name} did not go on and then off again: {got}")
    return problems


def scenarios() -> list[Scenario]:
    free = ["--free-flight", "--no-audio"]
    quick = ["--launch-quick-mission", "--no-audio"]

    def build(name, script, args, check, **kw):
        return live_scenario(
            f"replay-script-{name}",
            [*args, "--input-script", str(SCRIPTS / script)],
            timed=False,
            more=check,
            **kw,
        )

    return [
        build("systems", "systems.txt", free, systems_check),
        build("missile", "missile.txt", [*quick, "--separation", "10", "--ai-mission", "hold"], missile_check, ai=3),
        build("gun", "gun.txt", [*quick, "--separation", "10", "--ai-mission", "hold"], gun_check, ai=3),
        build("eject", "eject.txt", free, eject_check),
        build("takeoff-and-landing-request", "takeoff.txt", [*quick, "--ground-start", "1", "--probe-wing-size", "3"], takeoff_check, tower=True),
        build("navigation", "nav.txt", free, nav_check),
        build("views", "views.txt", free, views_check),
        build("pause-menu-bookmarks", "pause.txt", free, pause_check),
        build("cheats", "cheats.txt", free, cheats_check),
    ]
