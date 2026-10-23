"""The mission countdown in a six-minute dedicated flight and its Escape menu."""
from __future__ import annotations

from battery import Drive, Scenario
from battery_scenarios.net import GAME_FLAGS, LOCALHOST, start_server, start_bots

MISSION = """tore-mission 1
theater UKR
condition clear
start airborne 20000
separation-nm 200
preset hold
guns-only no
wing friendly 1 F18.PT 2 experienced
wing enemy 1 MIG29.PT 1 dummy
cheats none
"""


def drive_timer(d: Drive) -> None:
    port = d.port()
    server = start_server(d, port, MISSION, time_limit=6, idle_ai="never", empty_timeout=120)
    bot = start_bots(d, port, "observer", 400, "--observe", "none", "--callsign", "ClockWitness")
    shots = d.work / "shots"
    shots.mkdir()
    script = d.work / "timer.txt"
    script.write_text(
        f"wait 1\nwaittick 1200 90\nkey a\nkey Escape\nwait 0.3\nshot {shots}/menu-550.ppm\n"
        f"key Escape\nwait 0.3\nshot {shots}/cockpit-550.ppm\nwaittick 8400 100\nshot {shots}/cockpit-450.ppm\n"
        f"key F10\nwait 0.3\nshot {shots}/external-450.ppm\n"
        f"waittick 39600 300\nshot {shots}/external-030.ppm\n"
        f"key Escape\nwait 0.3\nshot {shots}/menu-030.ppm\n"
        "wait 35\nexit\n"
    )
    game = d.start("game", [d.app, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Timer",
                            "--slot", "0", *GAME_FLAGS, "--window-size", "1280x720",
                            "--input-script", script], window=True)
    game.finish(460, 0)
    server.expect(r"mission ended: the time limit", "the host ends at the deadline")
    bot.expect(r"[Tt]ime limit", "the observer receives the time-limit end")
    for name in ("menu-550", "cockpit-550", "cockpit-450", "external-450", "external-030", "menu-030"):
        if not (shots / f"{name}.ppm").exists():
            d.problem(f"missing {name} capture")
    # The capture log records the final authoritative Scores, including zero.
    bot.expect(r"0:00 left", "the final host clock is zero")


def scenarios() -> list[Scenario]:
    return [Scenario(
        name="net-window-mission-timer", lane="net", args=[], driver=drive_timer,
        uses=("server", "bot"), window=True, timeout=540,
        notes="A six-minute mission: Escape countdown before five minutes, cockpit and external countdown "
                    "after five minutes and in the last minute, and the host's time-limit end at zero.",
    )]
