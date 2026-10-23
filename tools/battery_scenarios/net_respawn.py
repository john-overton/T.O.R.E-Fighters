"""Persistent respawn menus over a live network flight, with explicit confirmation."""
from __future__ import annotations

import re

from battery import Drive, Scenario
from battery_scenarios.net import (
    GAME_FLAGS, NET_BAD, guide_mission, server_log, start_bots, start_server, weapons_hold,
)
from battery_scenarios.net_screens import game_log

# The settings registry permits whole-minute delays, so use its shortest nonzero value.
# Ejection drives the real loss path. The second player keeps the mission populated.
SCRIPT = """waittick 240 120
key Shift+e
wait 0.4
key Shift+e
wait 5
shot SHOTS/01-waiting.ppm
key m
wait 0.5
shot SHOTS/02-mission.ppm
key Escape
key ArrowRight
key ArrowDown
key Enter
key Enter
movemenu 80 460
click
wait 0.5
key Enter
wait 0.5
shot SHOTS/03-early-enter.ppm
key Escape
movemenu 20 12
click
key ArrowUp
key Enter
wait 0.5
shot SHOTS/04-reopened-loadout.ppm
waittick 18000 240
shot SHOTS/05-still-waiting.ppm
key Escape
key Escape
movemenu 20 12
click
wait 0.5
shot SHOTS/06-escape-choices.ppm
key ArrowUp
key ArrowUp
key Enter
wait 0.5
shot SHOTS/07-reopened-aircraft.ppm
key ArrowDown
key ArrowDown
key Enter
key Enter
movemenu 80 460
click
key Escape
movemenu 20 12
click
key ArrowUp
key Enter
wait 0.5
shot SHOTS/08-last-loadout.ppm
key m
wait 0.5
shot SHOTS/09-mission-again.ppm
key Escape
key Escape
key Enter
wait 5
shot SHOTS/10-respawned.ppm
key Escape
movemenu 20 12
click
wait 0.5
shot SHOTS/11-alive-menu.ppm
exit
"""
PICTURES = (
    "01-waiting", "02-mission", "03-early-enter", "04-reopened-loadout", "05-still-waiting",
    "06-escape-choices", "07-reopened-aircraft", "08-last-loadout", "09-mission-again",
    "10-respawned", "11-alive-menu",
)


def drive_respawn_menu(d: Drive) -> None:
    port = d.port()
    server = start_server(
        d, port, weapons_hold(guide_mission()) + "wing-choice friendly 1\n", respawn="revive",
        revive_delay=1, lives=2, time_limit=5, empty_timeout=0, min_players=2, idle_ai=1,
    )
    start_bots(d, port, "bot", 300, "--callsign", "Witness", "--slot", "1", "--players", "2")
    shots = d.work / "shots"
    shots.mkdir(exist_ok=True)
    script = d.work / "respawn.txt"
    script.write_text(SCRIPT.replace("SHOTS", str(shots)))
    game = d.start(
        "game", [d.app, "--connect", f"127.0.0.1:{port}", "--callsign", "Viper", "--slot", "0",
                 *GAME_FLAGS, "--original-graphics", "--window-size", "800x600", "--input-script", script], window=True,
    )
    game.finish(360, 0)
    server.send("quit")
    server.finish(30, None)
    log = game_log(d)
    for pattern in (
        r"Respawn: saved choice Rafale", r"Respawn: opened Loadout for Rafale",
        r"Respawn: opened Aircraft for Rafale", r"Respawn: saved choice F14",
        r"Respawn: opened Loadout for F14", r"Respawn: confirmed choice F14",
    ):
        if not re.search(pattern, log):
            d.problem(f"missing live respawn step: {pattern}")
    confirms = list(re.finditer(r"Respawn: confirmed", log))
    seats = list(re.finditer(r"Network: seated in plane (\d+)", log))
    if len(confirms) != 1 or len(seats) != 2 or not seats[0].start() < confirms[0].start() < seats[1].start():
        d.problem("expected exactly one explicit confirmation between the initial seat and the revival")
    if "Viper is away" in server_log(d):
        d.problem("idle AI took the dead player's slot")
    for name in PICTURES:
        if not (shots / f"{name}.ppm").exists():
            d.problem(f"missing capture: {name}")
    game.forbid(NET_BAD, "a network problem")


def scenarios() -> list[Scenario]:
    return [Scenario(
        name="net-window-respawn-menu", lane="net", args=[], driver=drive_respawn_menu,
        uses=("server", "bot"), window=True, timeout=420,
        notes="Eject, edit and reopen Choice aircraft and ordnance across a revival delay, reopen Mission, then confirm once.",
    )]
