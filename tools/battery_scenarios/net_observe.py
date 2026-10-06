"""Lane: net. The observer screen in the real window (stage F phase 2, slices F2-O2 and F2-O3).

One windowed scenario, `net-window-observe`: a `tore-server` flies the guide's mission with two `tore-bot` players, and
the game joins it from Direct Connection with no plane: its script selects the server in the games list, presses
Join, and in the lobby presses Watch, which opens the replay viewer in its live mode on the stream. The script looks at
the live view (a picture), changes the aircraft watched, leaves live with Home and returns with End (pictures of each),
then opens the Escape menu and leaves with its Stop Watching row, back in the lobby. What the game logs is what
its screens show in Messages; the server's log says the game watched and stopped watching. The pictures are for a
person to look at.

The scenario takes its own port, so it runs beside the others; it opens one window (`--windows`).

`net-window-away-watch` (slice F2-O3) is the other way in: the game flies plane 0 of a server's guide mission, the
script opens the flight menu and waits out the King's `idle-ai` (10 seconds), the host gives the plane to the AI, and
the game opens the observer screen on its own plane. The script looks at it, holds the Up arrow (the first flight
input) and is seated in the plane again; then it does the same and leaves the observer screen by its Escape menu's
Stop Watching row, which in a game with no lobby screen takes the plane back too.
"""
from __future__ import annotations

import re
import shutil
import time

from battery import Drive, DriveError, Scenario

from battery_scenarios.net import (
    GAME_FLAGS, LOCALHOST, NET_BAD, guide_mission, log_must, server_log, start_bots, start_server, weapons_hold,
)

# The script's clicks, in the 640 by 480 menu layer's pixels: the Multi menu, Direct Connection; the Connect to
# field, where Up brings back the address the settings file remembers, and Enter presses Join (the search finds
# nothing: the server holds the port it would search from); then the lobby's Watch (the second button of a player who is not the King, where Loadout is
# while the mission is not flying). Escape and Enter in the live view are the viewer's own keys.
OBSERVE_SCRIPT = """wait 10
movemenu 150 48
wait 4
click
wait 4
movemenu 190 72
wait 4
click
wait 10
snapshot SHOTS/watch-1-direct.ppm
movemenu 200 145
wait 4
click
wait 2
key Up
wait 2
snapshot SHOTS/watch-2-address.ppm
key Enter
wait 20
snapshot SHOTS/watch-3-lobby.ppm
movemenu 398 429
wait 4
click
wait 15
shot SHOTS/watch-4-live.ppm
key Tab
wait 3
shot SHOTS/watch-5-next.ppm
key Home
wait 3
shot SHOTS/watch-6-scrubbed.ppm
key End
wait 3
shot SHOTS/watch-7-live-again.ppm
key Escape
wait 2
key Enter
wait 6
snapshot SHOTS/watch-8-lobby.ppm
exit
"""

PICTURES = (
    "watch-1-direct", "watch-2-address", "watch-3-lobby", "watch-4-live", "watch-5-next", "watch-6-scrubbed",
    "watch-7-live-again", "watch-8-lobby",
)


def game_log(d: Drive) -> str:
    """The game's own log files in the data folder (`logs/tore-DATE.log`)."""
    return "\n".join(p.read_text(errors="replace") for p in sorted((d.data / "logs").glob("tore-*.log")))


def drive_observe_window(d: Drive) -> None:
    """The game joins a flying mission with no plane and watches it: Watch opens the live view, which follows the
    newest frame, can be scrubbed and returns to live with End; Stop Watching returns to the lobby."""
    port = d.port()
    server = start_server(d, port, guide_mission(separation_nm=5))
    (d.data / "network-v1.conf").write_text(
        f"tore-network 1\ncallsign Viper\nport {d.port()}\naddress {LOCALHOST}:{port}\n"
    )
    flyers = start_bots(d, port, "flyers", 400, "--count", "2", "--callsign", "Bot")
    if not server.wait_for(r"^mission started$", 60):
        raise DriveError("the mission never started")
    shots = d.work / "shots"
    shots.mkdir(exist_ok=True)
    # A scripted click on a window that is slow to come up (the machine is shared with a person) can land before
    # the menu answers; one more try, starting later, tells that from a real failure.
    game = None
    pictures = shots
    for attempt, start_wait in enumerate((20, 30, 45), start=1):
        for old in (d.data / "logs").glob("tore-*.log"):
            if not old.name.startswith("server-"):
                old.unlink()
        script = d.work / f"observe{attempt}.txt"
        pictures = shots / f"attempt{attempt}"
        pictures.mkdir(exist_ok=True)
        script.write_text(OBSERVE_SCRIPT.replace("SHOTS", str(pictures)).replace("wait 10\n", f"wait {start_wait}\n", 1))
        game = d.start(f"game{attempt}", [d.app, *GAME_FLAGS, "--input-script", script], window=True)
        started = time.time()
        # Milestones a loaded machine may miss: the Direct Connection screen opened (the clicks landed), then the
        # screen of the watch. A miss ends the attempt at once and starts the next.
        while not re.search(r"Observer screen: watching the mission", game_log(d)):
            waited = (time.time() - started) / d.scale
            opened = "Direct Connection:" in game_log(d)
            if not game.alive() or waited > start_wait + 100 or (not opened and waited > start_wait + 40):
                break
            d.sleep(0.5)
        else:
            break
        d.log(f"attempt {attempt}: the scripted clicks did not open the observer screen")
        game.stop()
        shutil.copytree(d.data / "logs", d.work / f"attempt{attempt}-logs", dirs_exist_ok=True)
    else:
        raise DriveError("the game never opened the observer screen")
    assert game is not None
    game.finish(150, 0)
    # The bots fly on; the scenario does not need the rest of their time.
    flyers.stop()
    server.send("quit")
    server.finish(30, None)
    log = game_log(d)
    # What the game's Messages and log said: the join without a plane, Watch, the screen opening and leaving.
    for pattern, what in (
        (r"Watching the mission\. Stop Watch ends it", "Watch pressed in the lobby"),
        (r"Observer screen: watching the mission", "the observer screen opening"),
        (r"Observer screen: back to the lobby", "Stop Watching returning to the lobby"),
        (r"You stopped watching\.", "the lobby's line after Stop Watching"),
    ):
        if not re.search(pattern, log):
            d.problem(f"the game's log lacks {what}: /{pattern}/")
    if re.search(r"Could not show the mission", log):
        d.problem("the observer screen could not open")
    for name in PICTURES:
        if not (pictures / f"{name}.ppm").exists():
            d.problem(f"the script's {name}.ppm was not written")
    game.forbid(NET_BAD, "a network problem")
    flyers.forbid(NET_BAD, "a network problem")
    log_must(d, server_log(d), r"Viper is watching the mission", r"Viper stopped watching", forbid=NET_BAD)


# The script of the away watch: four seconds of flight, the flight menu (the controls are neutral behind it), the
# time the King's idle-ai takes and the handoff and the first frames, then pictures of the observer screen, Up
# held (the stick is a flight input) and the plane flown again; the same once more and Stop Watching (Escape opens
# the viewer's menu, Enter its first row) before the last picture and the end. Waits are long: the machine is shared.
AWAY_SCRIPT = """wait 1
waittick 480 60
shot SHOTS/away-1-flying.ppm
key Escape
wait 2
shot SHOTS/away-2-menu.ppm
wait 16
shot SHOTS/away-3-watching.ppm
wait 3
shot SHOTS/away-4-watching-later.ppm
down Up
wait 1
up Up
wait 8
shot SHOTS/away-5-back.ppm
key Escape
wait 18
key Escape
wait 2
shot SHOTS/away-6-menu.ppm
key Enter
wait 8
shot SHOTS/away-7-back-again.ppm
exit
"""

AWAY_PICTURES = (
    "away-1-flying", "away-2-menu", "away-3-watching", "away-4-watching-later", "away-5-back", "away-6-menu",
    "away-7-back-again",
)


def away_watch_problems(game: str, server: str) -> list[str]:
    """What the game's log and the server's log must hold after two handoffs, the first ended by a flight input and
    the second by Stop Watching (pure, unit tested in tools/test_battery_net.py)."""
    problems = []
    for pattern, count, what in (
        (r"Network: away for the idle-ai seconds; the AI flies the plane", 2, "the game saying it is away"),
        (r"Observer screen: watching the player's own aircraft", 2, "the observer screen opening on its own plane"),
        (r"Network: a flight input; taking the aircraft back from the AI", 1, "a flight input taking the plane back"),
        (r"Network: Stop Watching; taking the aircraft back from the AI", 1, "Stop Watching taking the plane back"),
        (r"Observer screen: back to the flight", 2, "the observer screen closing onto the flight, once for each return"),
        (r"Network: seated in plane 0", 3, "a seating in plane 0, and two more after the handoffs"),
    ):
        found = len(re.findall(pattern, game))
        if found < count:
            problems.append(f"the game's log holds {found} of {count} for {what}: /{pattern}/")
    if re.search(r"Could not show the mission", game):
        problems.append("the game's log holds the observer screen failing to open")
    if re.search(r"The AI lost your aircraft", game):
        problems.append("the game's log says the AI lost the aircraft")
    for pattern, what in (
        (r"Viper is away: the AI flies plane 0$", "the handoffs"),
        (r"Viper is back: takes plane 0 from the AI$", "the returns"),
    ):
        found = len(re.findall(pattern, server, re.M))
        if found != 2:
            problems.append(f"the server's log holds {found} of 2 for {what}: /{pattern}/")
    return problems


def drive_away_watch(d: Drive) -> None:
    """The game flies plane 0, goes away behind its flight menu, watches the AI fly the plane on the observer screen
    and takes it back by a flight input; then again, and takes it back by Stop Watching."""
    port = d.port()
    # Every AI wing on weapons hold: the AI flying the idle plane must not be shot down while the script watches.
    server = start_server(d, port, weapons_hold(guide_mission()))
    shots = d.work / "shots"
    shots.mkdir(exist_ok=True)
    script = d.work / "away.txt"
    script.write_text(AWAY_SCRIPT.replace("SHOTS", str(shots)))
    game = d.start(
        "game", [d.app, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Viper", *GAME_FLAGS, "--input-script", script],
        window=True,
    )
    game.finish(240, 0)
    server.send("quit")
    server.finish(30, None)
    for problem in away_watch_problems(game_log(d), server_log(d)):
        d.problem(problem)
    for name in AWAY_PICTURES:
        if not (shots / f"{name}.ppm").exists():
            d.problem(f"the script's {name}.ppm was not written")
    game.forbid(NET_BAD, "a network problem")
    log_must(d, server_log(d), r"Viper is away", forbid=NET_BAD)


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="net-window-observe", lane="net", args=[], driver=drive_observe_window, uses=("server", "bot"),
            window=True, timeout=700,
            notes="the observer screen in the window: the game joins a server flown by two bots with no plane, presses "
            "Watch, looks at the live view, changes the aircraft, leaves live and returns with End, and stops watching",
        ),
        Scenario(
            name="net-window-away-watch", lane="net", args=[], driver=drive_away_watch, uses=("server",),
            window=True, timeout=400,
            notes="an away player watches its own plane: the game flies, opens its flight menu, goes away after the "
            "King's idle-ai seconds, shows the AI flying its plane on the observer screen, and takes it back by "
            "the Up arrow; then the same, taken back by Stop Watching (slice F2-O3)",
        ),
    ]
