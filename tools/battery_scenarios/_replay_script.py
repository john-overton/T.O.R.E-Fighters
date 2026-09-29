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


# Carrier aircraft have a tailhook and only these two have no afterburner (an A-4E, an Su-25).
HOOK = {"f18", "f14", "a4e", "f22n", "faxx"}
NO_BURNER = {"a4e", "su25"}


def systems_check(work: Path, output: str, ac: str = "f18") -> list[str]:
    events, samples = load(work)
    problems = []
    if not samples:
        return ["no samples of the player"]
    at = first_time(samples, lambda d: d["controls"]["throttle"] > 0.99)
    if at is None or at > 4:
        problems.append("key 5 did not bring the throttle to 100 percent within 4 s")
    problems += device_cycle(samples, "gear", 1.5, 5.8)
    problems += device_cycle(samples, "flaps", 7.5, 12.5)
    if ac in HOOK:
        problems += device_cycle(samples, "hook", 7.5, 12.5)
    elif max(d["devices"]["hook"] for d in samples) > 0.1:
        problems.append(f"the {ac} has no tailhook but its hook moved")
    problems += device_cycle(samples, "brake", 7.5, 12.5)
    burner = first_time(samples, lambda d: "afterburner" in d["flags"], 17.0)
    if ac in NO_BURNER:
        if burner is not None:
            problems.append(f"the {ac} has no afterburner but lit one")
    elif burner is None:
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
    problems = []
    steering = [e["fields"] for e in events if e["kind"] == "flight.effect" and e["fields"].get("effect") == "Autopilot steering"]
    modes = [f["reason"] for f in steering if f["on"]]
    if len(modes) != 2 or "heading" not in modes[0] or "waypoint" not in modes[1]:
        problems.append(f"expected heading then waypoint autopilot, got {modes}")
    if [f["on"] for f in steering] != [True, False, True, False]:
        problems.append("the autopilot did not go on and off twice")
    if not any("Navigation mode selected" in (e.get("text") or "") for e in events):
        problems.append("key n did not select the navigation mode")
    return problems


def maneuvers_check(work: Path, output: str) -> list[str]:
    events, samples = load(work)
    problems = []
    if max(d["g"] for d in samples) < 5:
        problems.append("the held pull never reached 5 G")
    if max(abs(d["att_deg"][2]) for d in samples) < 90:
        problems.append("the held roll never passed 90 degrees of bank")
    if count(events, "flight.g_limit", None) < 1:
        problems.append("the pull reached the stick stop but no flight.g_limit event was recorded")
    if any(d["g"] > 12 or d["g"] < -8 for d in samples):
        problems.append("a G outside the aircraft's possible range")
    return problems


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


def info_of(path: Path) -> str:
    import os
    import subprocess

    binary = Path(__file__).resolve().parents[2] / "target" / "debug" / "tore-app"
    env = dict(os.environ, TORE_NO_ERROR_DIALOG="1")
    return subprocess.run([str(binary), "--recording-info", str(path)], capture_output=True, text=True, env=env).stdout


def pause_menu_mouse_check(work: Path, output: str) -> list[str]:
    # A flight started in the same minute gets -2 after its stem, which sorts before the dot.
    files = sorted((work / "data" / "replays").glob("*.tore-replay"), key=lambda p: p.stem.endswith("-2"))
    problems = []
    if len(files) != 2:
        return [f"expected two recordings (the restart starts a new one), found {len(files)}"]
    first, second = (info_of(f) for f in files)
    if "end=restart" not in first:
        problems.append("the first recording does not end with the restart")
    if "system.restart" not in second:
        problems.append("the second recording does not begin with a restart event")
    for text in (first, second):
        if "finished normally" not in text or "Problem " in text:
            problems.append("a recording is not finished normally")
    if "system.cheat" not in first:
        problems.append("the mouse click on the Cheat tab's Unlimited ammo row changed no cheat")
    return problems


def snapshots(work: Path) -> dict[str, Path]:
    return {p.stem: p for p in sorted((work / "shots").glob("*.ppm"))}


def debrief_mouse_check(work: Path, output: str) -> list[str]:
    from battery_scenarios import _replay_tools as tools

    shots = snapshots(work)
    names = ["page1", "page2", "page3", "page4", "page5", "back4", "creator"]
    problems = [f"no snapshot {n}" for n in names if n not in shots]
    if problems:
        return problems
    digest = {n: shots[n].read_bytes() for n in names}
    for n in names:
        problems += tools.ppm_problems(str(shots[n]), min_colors=8)
    pages = [digest[f"page{i}"] for i in range(1, 6)]
    if len(set(pages)) != 5:
        problems.append("the five debrief pages are not all different: NEXT did not page")
    if digest["back4"] != digest["page4"]:
        problems.append("PREV did not return to page 4")
    if digest["creator"] in pages:
        problems.append("OK did not leave the debrief")
    return problems


def replays_mouse_check(work: Path, output: str) -> list[str]:
    from battery_scenarios import _replay_tools as tools

    problems = []
    shots = snapshots(work)
    for n in ("list", "confirm", "after-delete", "panel", "final"):
        if n not in shots:
            problems.append(f"no snapshot {n}")
        else:
            problems += tools.ppm_problems(str(shots[n]), min_colors=8)
    folder = work / "data" / "replays"
    names = sorted(p.name for p in folder.iterdir())
    conf = (work / "data" / "replays-v1.conf").read_text()
    if "2026-09-18_1200_UKR_F18.tore-replay" in names:
        problems.append("Delete and its confirmation did not remove the recording")
    if "2026-09-18_1200_UKR_F18.txt.acmi" not in names:
        problems.append("the Tacview button wrote no file")
    if "2026-09-18_1200_UKR_F18-log" not in names:
        problems.append("the Debug log button wrote no folder")
    if "keep 2026-09-17_1200_UKR_F18.tore-replay" not in conf:
        problems.append("the Keep button did not mark the selected recording")
    if "auto-delete off" not in conf or "rule older-than" not in conf:
        problems.append("the auto-delete panel's choices were not saved")
    for n in ("2026-09-10_1200_UKR_F18.tore-replay", "2026-09-11_1200_UKR_F18.tore-replay", "notes.txt"):
        if n not in names:
            problems.append(f"{n} should be untouched")
    if list(shots).count("confirm") and shots["confirm"].read_bytes() == shots["list"].read_bytes():
        problems.append("the Delete button did not open the confirmation")
    return problems


def mouse_scenarios() -> list[Scenario]:
    from battery import Step
    from battery_scenarios import _replay_tools as tools

    out = []
    out.append(
        Scenario(
            name="replay-script-pause-menu-mouse",
            lane="replay",
            args=["--free-flight", "--no-audio", "--input-script", str(SCRIPTS / "pause-menu-mouse.txt")],
            window=True,
            timeout=240,
            env={"TORE_RECORD_MISSIONS": "1"},
            check_work=pause_menu_mouse_check,
        )
    )
    out.append(
        Scenario(
            name="replay-script-debrief-mouse",
            lane="replay",
            args=["--launch-quick-mission", "--separation", "2", "--no-audio", "--input-script", str(SCRIPTS / "debrief.txt")],
            window=True,
            timeout=240,
            env={"TORE_SCRIPT_OUT": "{work}/shots"},
            check_work=debrief_mouse_check,
        )
    )
    out.append(
        Scenario(
            name="replay-script-replays-screen-mouse",
            lane="replay",
            args=["--ai-probe-ticks", "600", "--separation", "2", "--probe-attack", "100:5", "--record-mission", "{work}/src.tore-replay", "--no-audio"],
            env={"TORE_SCRIPT_OUT": "{work}/shots"},
            then=[
                Step(["python3", tools.__file__, "seed", "{work}/data", "{work}/src.tore-replay"], app=False),
                Step(["--no-audio", "--input-script", str(SCRIPTS / "replays-screen.txt")], window=True, timeout=200),
            ],
            check_work=replays_mouse_check,
            timeout=300,
        )
    )
    return out


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

    from battery_scenarios._replay_record import AIRCRAFT

    every = [
        build(f"systems-{ac}", "systems.txt", ["--free-flight", "--no-audio", "--aircraft", ac, "--researched-flight"], lambda work, output, ac=ac: systems_check(work, output, ac))
        for ac in AIRCRAFT
    ]
    return [
        *every,
        build("systems", "systems.txt", free, systems_check),
        build("missile", "missile.txt", [*quick, "--separation", "10", "--ai-mission", "hold"], missile_check, ai=3),
        build("gun", "gun.txt", [*quick, "--separation", "10", "--ai-mission", "hold"], gun_check, ai=3),
        build("eject", "eject.txt", free, eject_check),
        build("takeoff-and-landing-request", "takeoff.txt", [*quick, "--ground-start", "1", "--probe-wing-size", "3"], takeoff_check, tower=True),
        build("navigation", "nav.txt", free, nav_check),
        build("views", "views.txt", free, views_check),
        build("maneuvers", "maneuvers.txt", free, maneuvers_check),
        build("pause-menu-bookmarks", "pause.txt", free, pause_check),
        build("cheats", "cheats.txt", free, cheats_check),
        *mouse_scenarios(),
    ]
