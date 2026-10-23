"""Lane: net. The lobby screen's phase 2 panels (stage F, slice F2-L) driven in the real window.

One windowed scenario, `net-window-lobby`: the game hosts from Direct Connection's New (so it is the King), a
`tore-bot` joins (once the Game type is PvP, which opens the enemy planes) and takes a slot, and the game's input script works the screen as a player does: the
Settings panel (Game type, Friendly fire, the Scoring page's kill limit, the Realism page's Damage), a right click
that closes the bot's slot and opens it again, the Players panel's Give crown (the bot wears the crown); then the
bot leaves and the crown comes back. What the bot prints is what a second player
sees; what the game logs is what its Messages show. The pictures the script writes are for a person to look at.

The scenario takes its own port, so it runs beside the others; it opens one window (`--windows`).

A second windowed scenario, `net-window-gaps` (stage L, slice L4), has the game host and a `tore-bot` whose import
lacks the Rafale join as an observer, and the script shows what the game's King and every player then see: the
selected player's system line, the King's creator with the Rafale dimmed in its aircraft list, a choice of it refused
with the host's words, and the gap line in Messages.
"""
from __future__ import annotations

import re
import shutil
import time

from battery import Drive, DriveError, Scenario

from battery_scenarios.net import GAME_FLAGS, LOCALHOST, NET_BAD, fresh_data, start_bots, start_server, stop_server

# Coordinate-driven runs pin CHOOSEM: CHOOSE3 shifts the main menu bar by 109 pixels.
# The script's clicks, in the 640 by 480 menu layer's pixels. The Multi menu, Direct Connection, New; then the
# lobby: Settings... is the King's second button, Players... the third. The Settings panel's rows are at x 430,
# 19 pixels apart on the Game page (twelve rows since stage K's Host row) from y 175: Game type is the second row, at 194,
# Friendly fire the seventh, at 289 (it was 301 at 21 pixels, which now lands on Lock sides); Realism's Damage is at the top left.
LOBBY_SCRIPT = """wait 10
movemenu 150 48
wait 0.6
click
wait 0.6
movemenu 190 72
wait 0.6
click
wait 4
movemenu 140 432
wait 0.6
click
wait 14
snapshot SHOTS/lobby-1-king.ppm
movemenu 160 431
wait 0.6
click
wait 1.5
snapshot SHOTS/lobby-2-settings.ppm
movemenu 430 196
wait 0.6
click
wait 8
movemenu 430 289
wait 0.6
click
wait 2
snapshot SHOTS/lobby-3-game.ppm
key Tab
wait 0.5
snapshot SHOTS/lobby-minimum-before.ppm
movemenu 430 196
wait 0.5
click
wait 0.5
snapshot SHOTS/lobby-minimum-after.ppm
key Tab
wait 0.5
movemenu 430 238
wait 0.6
click
wait 2
snapshot SHOTS/lobby-4-scoring.ppm
key Tab
wait 0.5
movemenu 275 175
wait 0.6
click
wait 2
snapshot SHOTS/lobby-5-realism.ppm
key F1
wait 1
snapshot SHOTS/lobby-settings-help.ppm
key Escape
key F10
wait 35
movemenu 100 176
wait 0.6
click right
wait 3
snapshot SHOTS/lobby-6-closed.ppm
click right
wait 3
movemenu 450 194
wait 0.6
click
wait 0.6
movemenu 240 431
wait 0.6
click
wait 1.5
snapshot SHOTS/lobby-7-players.ppm
movemenu 200 294
wait 0.6
click
wait 4
snapshot SHOTS/lobby-8-crowned.ppm
wait 45
snapshot SHOTS/lobby-9-back.ppm
key F1
wait 1
snapshot SHOTS/lobby-help.ppm
key m
wait 3
snapshot SHOTS/lobby-main.ppm
exit
"""

# Stage L (slice L4): the same way into the lobby, the Game type turned to PvP (the default mission has room for the
# hosting player alone, and a full game refuses the bot), then the bot's row in Players (the hint), Mission..., the first
# wing's aircraft field (the list, with the Rafale dimmed), the Rafale (the tenth row of the sorted list) and OK, which
# the creator refuses, and Esc, which puts the creator away.
GAPS_SCRIPT = """wait 10
movemenu 150 48
wait 0.6
click
wait 0.6
movemenu 190 72
wait 0.6
click
wait 4
movemenu 140 432
wait 0.6
click
wait 14
snapshot SHOTS/gaps-1-lobby.ppm
movemenu 160 431
wait 0.6
click
wait 1.5
movemenu 430 196
wait 0.6
click
wait 3
key F10
wait 14
movemenu 450 194
wait 0.6
click
wait 1.5
snapshot SHOTS/gaps-2-hint.ppm
movemenu 82 431
wait 0.6
click
wait 2
movemenu 185 157
wait 0.6
click
wait 1.5
snapshot SHOTS/gaps-3-list.ppm
movemenu 300 285
wait 0.6
click
wait 0.6
movemenu 259 449
wait 0.6
click
wait 1.5
snapshot SHOTS/gaps-4-notice.ppm
key Escape
wait 1.5
snapshot SHOTS/gaps-5-back.ppm
exit
"""

GAPS_PICTURES = ("gaps-1-lobby", "gaps-2-hint", "gaps-3-list", "gaps-4-notice", "gaps-5-back")

PICTURES = (
    "lobby-settings-help", "lobby-help", "lobby-main", "lobby-minimum-before", "lobby-minimum-after",
    "lobby-1-king", "lobby-2-settings", "lobby-3-game", "lobby-4-scoring", "lobby-5-realism", "lobby-6-closed",
    "lobby-7-players", "lobby-8-crowned", "lobby-9-back",
)


def game_log(d: Drive) -> str:
    """The game's own log files in the data folder (`logs/tore-DATE.log`)."""
    return "\n".join(p.read_text(errors="replace") for p in sorted((d.data / "logs").glob("tore-*.log")))


def drive_lobby(d: Drive) -> None:
    """The King's panels in the window against a bot: settings, a slot closed and opened, the crown given away and
    taken back when the bot leaves."""
    port = d.port()
    fresh_data(d)
    (d.data / "network-v1.conf").write_text(f"tore-network 1\ncallsign Viper\nport {port}\n")
    shots = d.work / "shots"
    shots.mkdir(exist_ok=True)
    # A scripted click on a window that is slow to come up (the machine is shared with a person) can land before
    # the menu answers; one more try, starting later, tells that from a real failure.
    game = None
    for attempt, start_wait in enumerate((20, 40), start=1):
        for old in (d.data / "logs").glob("tore-*.log"):
            old.unlink()
        script = d.work / f"lobby{attempt}.txt"
        script.write_text(LOBBY_SCRIPT.replace("SHOTS", str(shots)).replace("wait 10\n", f"wait {start_wait}\n", 1))
        game = d.start(f"game{attempt}", [d.app, *GAME_FLAGS, "--background", "CHOOSEM", "--input-script", script], window=True)
        # The lobby opens some seconds after New is clicked; the hosting thread's line says it listens.
        end = time.time() + (start_wait + 45) * d.scale
        while not re.search(r"Hosting Viper's game on UDP port", game_log(d)):
            if not game.alive() or time.time() > end:
                break
            d.sleep(0.5)
        else:
            break
        d.log(f"attempt {attempt}: the scripted clicks did not open the lobby")
        game.stop()
        shutil.copytree(d.data / "logs", d.work / f"attempt{attempt}-logs", dirs_exist_ok=True)
    else:
        raise DriveError("the game never hosted from Direct Connection's New")
    # The default Quick Mission has one friendly plane, so the hosting player alone fills the game until the
    # script turns the Game type to PvP, which opens the enemy planes too: then the bot may join.
    end = time.time() + 90 * d.scale
    while not re.search(r"Lobby: Settings: mode pvp", game_log(d)):
        if not game.alive() or time.time() > end:
            raise DriveError("the script never turned the Game type to PvP")
        d.sleep(0.5)
    # The bot imports after the deferred apply. Allow that startup before locking its explicit slot,
    # then keep it long enough to receive the crown and leave during the final wait.
    bot = start_bots(d, port, "bot", 65, "--callsign", "Bot", "--slot", "0")
    assert game is not None
    game.finish(150, 0)
    if not re.search(r"Lobby: Settings: .*min-players 1", game_log(d)):
        d.problem("the deferred minimum edit did not apply with the other settings")
    bot.finish(60, None)
    log = game_log(d)
    # What the game's Messages said, from its log: the bot joining, the King's own settings and slot locks, the
    # crown given away and back, the bot's own settings change as King.
    for pattern, what in (
        (r"Lobby: Bot joined the game\.", "the bot joining"),
        (r"Lobby: Settings: mode pvp(, [^.\n]+)?\.", "the Game type turned to pvp"),
        (r"Lobby: Settings: [^\n]*friendly-fire off", "Friendly fire turned off"),
        (r"Lobby: Settings: [^\n]*kill-limit 7", "the Scoring page's kill limit turned"),
        (r"Host: tick \d+: lobby: Viper changed the settings: [^\n]*realism changed", "Realism included in the single settings apply"),
        (r"Lobby: Plane 0's slot is closed: the AI flies it\.", "the slot closed by the right click"),
        (r"Lobby: Plane 0's slot is open\.", "the slot opened by the second right click"),
        (r"Lobby: Bot is the King now\.", "the crown given to the bot"),
        (r"Lobby: Bot left the game\.", "the bot leaving"),
        (r"Lobby: Viper is the King now\.", "the crown coming back"),
    ):
        if not re.search(pattern, log):
            d.problem(f"the game's log lacks {what}: /{pattern}/")
    applies = re.findall(r"Host: tick \d+: lobby: Viper changed the settings:", log)
    if len(applies) != 1:
        d.problem(f"the King's draft produced {len(applies)} applies, expected exactly one")
    # What the bot, a second player, saw.
    bot.expect(r"^Bot: settings: .*mode pvp", "the King's change of mode")
    bot.expect(r"^Bot: settings: .*friendly-fire off", "the King's friendly fire")
    bot.expect(r"^Bot: wears the crown$", "the crown")
    bot.expect(r"^Bot: The King closed plane 0's slot: the AI flies it\.$", "the slot closed under it")
    bot.expect(r"^Bot: lobby: Lobby, mission \d+: Viper no slot; Bot \(King\)", "the lobby showing it as King")
    bot.forbid(NET_BAD, "a network problem")
    for name in PICTURES:
        if not (shots / f"{name}.ppm").exists():
            d.problem(f"the script's {name}.ppm was not written")
    game.forbid(NET_BAD, "a network problem")


def drive_gaps(d: Drive) -> None:
    """The game hosts from Direct Connection's New; a bot lacking the Rafale joins as an observer. The window shows
    the selected bot's system, the creator's dimmed Rafale and the refusal of choosing it, and the game's
    log holds the host's content and gaps lines and the lines its Messages show."""
    port = d.port()
    fresh_data(d)
    (d.data / "network-v1.conf").write_text(f"tore-network 1\ncallsign Viper\nport {port}\n")
    shots = d.work / "shots"
    shots.mkdir(exist_ok=True)
    game = None
    for attempt, start_wait in enumerate((20, 40), start=1):
        for old in (d.data / "logs").glob("tore-*.log"):
            old.unlink()
        script = d.work / f"gaps{attempt}.txt"
        script.write_text(GAPS_SCRIPT.replace("SHOTS", str(shots)).replace("wait 10\n", f"wait {start_wait}\n", 1))
        game = d.start(f"game{attempt}", [d.app, *GAME_FLAGS, "--input-script", script], window=True)
        end = time.time() + (start_wait + 45) * d.scale
        while not re.search(r"Hosting Viper's game on UDP port", game_log(d)):
            if not game.alive() or time.time() > end:
                break
            d.sleep(0.5)
        else:
            break
        d.log(f"attempt {attempt}: the scripted clicks did not open the lobby")
        game.stop()
        shutil.copytree(d.data / "logs", d.work / f"attempt{attempt}-logs", dirs_exist_ok=True)
    else:
        raise DriveError("the game never hosted from Direct Connection's New")
    # The default mission has room for the hosting player alone: the script turns the Game type to PvP, which opens
    # the enemy planes, and the bot then joins to watch from the lobby with no slot, staying past the end of the script.
    end = time.time() + 90 * d.scale
    while not re.search(r"Lobby: Settings: mode pvp", game_log(d)):
        if not game.alive() or time.time() > end:
            raise DriveError("the script never turned the Game type to PvP")
        d.sleep(0.5)
    bot = start_bots(d, port, "bot", 60, "--callsign", "Bot", "--observe", "none", "--drop-resource", "RAFALE.PT")
    assert game is not None
    game.finish(150, 0)
    bot.finish(60, None)
    log = game_log(d)
    for pattern, what in (
        (r"Lobby: Bot joined the game\.", "the bot joining"),
        (r"Lobby: Bot's game differs from the host's: no Rafale C\.", "the line about the bot's game in Messages"),
        (r"Host: tick \d+: content Bot: [^\n]*lacks aircraft RAFALE\.PT", "the host's content line for the bot"),
        (r"Host: tick \d+: gaps: aircraft RAFALE\.PT \(Bot lacks it\)", "the host's gaps line"),
    ):
        if not re.search(pattern, log):
            d.problem(f"the game's log lacks {what}: /{pattern}/")
    bot.expect(r"^dropped RAFALE\.PT from the import$", "the dropped profile")
    bot.expect(r"^Bot: gaps: aircraft RAFALE\.PT \(Bot lacks it\)$", "the gap")
    bot.forbid(NET_BAD, "a network problem")
    for name in GAPS_PICTURES:
        if not (shots / f"{name}.ppm").exists():
            d.problem(f"the script's {name}.ppm was not written")
    game.forbid(NET_BAD, "a network problem")



def drive_menu_disconnect(d: Drive) -> None:
    """Return to main, rejoin the same free seat, then Exit through the ? menu."""
    port = d.port()
    server = start_server(d, port, empty_timeout=3600)
    (d.data / "network-v1.conf").write_text(
        f"tore-network 1\ncallsign Viper\nport {d.port()}\naddress {LOCALHOST}:{port}\n"
    )
    shots = d.work / "shots"
    shots.mkdir(exist_ok=True)
    join = """movemenu 150 48
wait 1
click
wait 1
movemenu 190 72
click
wait 6
movemenu 200 145
click
key Up
key Enter
wait 18
movemenu 150 175
click
wait 3
key F1
wait 1
"""
    script = d.work / "menu-disconnect.txt"
    script.write_text("wait 20\n" + join + f"snapshot {shots}/return.ppm\nkey m\nwait 4\nsnapshot {shots}/main.ppm\n"
                      + join + f"snapshot {shots}/exit.ppm\nkey Down\nkey Enter\nwait 5\nsnapshot {shots}/exit-failed.ppm\nexit\n")
    game = d.start("game", [d.app, *GAME_FLAGS, "--background", "CHOOSEM", "--input-script", script], window=True)
    game.finish(180, 0)
    text = server.text()
    if len(re.findall(r"joined as Viper", text)) != 2:
        d.problem("Return to Main Menu did not allow joining the server a second time")
    if len(re.findall(r"Viper took the slot of plane 0", text)) != 2:
        d.problem("the first disconnect did not free plane 0 for the second join")
    if (shots / "exit-failed.ppm").exists():
        d.problem("the ? menu Exit did not close the application")
    for name in ("return", "main", "exit"):
        if not (shots / f"{name}.ppm").exists():
            d.problem(f"missing {name} menu capture")
    if not server.alive():
        d.problem("a client's menu exit stopped the server")
    game.forbid(NET_BAD, "a network problem")
    stop_server(d, server)

def drive_callsign_rejoin(d: Drive) -> None:
    """Leave, rename in the lounge, rejoin; repeat through Return to Main Menu."""
    port = d.port()
    server = start_server(d, port, empty_timeout=3600)
    (d.data / "network-v1.conf").write_text(
        f"tore-network 1\ncallsign Viper\nport {d.port()}\naddress {LOCALHOST}:{port}\n"
    )
    shots = d.work / "shots"
    shots.mkdir(exist_ok=True)
    lounge = "movemenu 150 48\nclick\nwait 1\nmovemenu 190 72\nclick\nwait 6\n"
    join = "movemenu 200 145\nclick\nkey Up\nkey Enter\nwait 30\n"
    def rename(name: str) -> str:
        return ("movemenu 140 110\nclick\nkey End\n" + "key Backspace\n" * 15
                + "".join(f"key {c}\n" for c in name))
    script = d.work / "callsign-rejoin.txt"
    script.write_text(
        "wait 20\n" + lounge + join.replace("wait 30\n", "wait 60\n") + f"shot {shots}/before-viper.ppm\n"
        + "key Escape\nwait 5\n" + rename("maverick") + join
        + f"shot {shots}/after-leave-maverick.ppm\n"
        + "key F1\nkey m\nwait 5\n" + lounge + rename("goose") + join
        + f"shot {shots}/after-main-goose.ppm\nkey F1\nkey x\n"
    )
    game = d.start("game", [d.app, *GAME_FLAGS, "--background", "CHOOSEM", "--input-script", script], window=True)
    game.finish(360, 0)
    text = server.text()
    for name in ("Viper", "maverick", "goose"):
        if not re.search(rf"joined as {name}\b", text):
            d.problem(f"the lobby did not accept the field's callsign {name}")
    lobby_lines = [line for path in sorted((d.data / "logs").glob("tore-*.log"))
                   for line in path.read_text(errors="replace").splitlines()
                   if "Network: lobby: Lobby," in line]
    (d.work / "lobby-names.txt").write_text("\n".join(lobby_lines) + "\n")
    for name in ("Viper", "maverick", "goose"):
        if not any(re.search(rf": {name} no slot$", line) for line in lobby_lines):
            d.problem(f"the client never received the renamed lobby player {name}")
    if "rejoined with its token" in text:
        d.problem("a clean leave retained the old rejoin identity")
    kept = d.data / "rejoin-v1.conf"
    if kept.exists() and "token " in kept.read_text():
        d.problem("Exit kept the session's rejoin token")
    for name in ("before-viper", "after-leave-maverick", "after-main-goose"):
        if not (shots / f"{name}.ppm").exists():
            d.problem(f"missing {name} lobby capture")
    game.forbid(NET_BAD, "a network problem")
    stop_server(d, server)


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="net-window-callsign-rejoin", lane="net", args=[], driver=drive_callsign_rejoin,
            uses=("server",), window=True, timeout=420,
            notes="clean Leave and Return to Main Menu clear identity; rejoining uses the changed lounge callsign",
        ),
        Scenario(
            name="net-window-menu-disconnect", lane="net", args=[], driver=drive_menu_disconnect,
            uses=("server",), window=True, timeout=240,
            notes="the real ? menu returns a client to main, frees its seat for rejoin, and exits on the second visit",
        ),
        Scenario(
            name="net-window-lobby", lane="net", args=[], driver=drive_lobby, uses=("bot",), window=True, timeout=420,
            notes="the lobby's Settings and Players panels and slot locks in the window: the King turns settings, "
            "closes and opens a slot, gives the crown to a bot, which changes a setting, and takes it back when it leaves",
        ),
        Scenario(
            name="net-window-gaps", lane="net", args=[], driver=drive_gaps, uses=("bot",), window=True, timeout=300,
            notes="stage L's gaps in the window: a bot lacking the Rafale joins the King's lobby; the selected player's "
            "system line, the creator's dimmed Rafale and the refusal of choosing it, Messages' difference line, the "
            "host's content and gaps lines in the game's log (slice L4)",
        ),
    ]
