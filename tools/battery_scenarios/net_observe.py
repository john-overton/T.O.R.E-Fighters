"""Lane: net. The observer screen in the real window (stage F phase 2, slice F2-O2).

One windowed scenario, `net-window-observe`: a `tore-server` flies the guide's mission with two `tore-bot` players, and
the game joins it from Direct Connection with no plane: its script selects the server in the games list, presses
Join, and in the lobby presses Watch, which opens the replay viewer in its live mode on the stream. The script looks at
the live view (a picture), changes the aircraft watched, leaves live with Home and returns with End (pictures of each),
then opens the Escape menu and leaves with its Stop Watching row, back in the lobby. What the game logs is what
its screens show in Messages; the server's log says the game watched and stopped watching. The pictures are for a
person to look at.

The scenario takes its own port, so it runs beside the others; it opens one window (`--windows`).
"""
from __future__ import annotations

import re
import shutil
import time

from battery import Drive, DriveError, Scenario

from battery_scenarios.net import (
    GAME_FLAGS, LOCALHOST, NET_BAD, guide_mission, log_must, server_log, start_bots, start_server,
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


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="net-window-observe", lane="net", args=[], driver=drive_observe_window, uses=("server", "bot"),
            window=True, timeout=700,
            notes="the observer screen in the window: the game joins a server flown by two bots with no plane, presses "
            "Watch, looks at the live view, changes the aircraft, leaves live and returns with End, and stops watching",
        ),
    ]
