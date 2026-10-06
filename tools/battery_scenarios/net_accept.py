"""Lane: net. Stage F phase 2's acceptance: the idle-AI menu's remaining paths in the real window (slice F2-X).

`net-window-away-watch` (slices F2-O3 and F2-O4) covers a joined player's Take Back Flight and Leave Game. The
scenarios here cover what it cannot:

`net-window-host-leave-handover` and `net-window-host-leave-confirm`: the game hosts from Direct Connection's New
(so it is the King), the script turns the lobby's "AI flies idle aircraft after" to 1 minute, a `tore-bot` that only
watches (`--observe none`) joins, and the King flies. The script opens the flight menu (the controls go neutral),
waits out the minute, and on the observer screen of its own aircraft chooses Leave Game. With a ready standby (the bot
stands by) the game hands the game over to it; with none (the bot does not stand by) the menu asks twice and the
second Enter ends the game for everyone.

`net-window-away-lost`: a game joins a `tore-server` whose AI wings fly free and whose King allows revival; the game
goes away, the AI flies its aircraft until the enemy shoots it down, and the observer menu's first row is Spawn in
Aircraft, which flies the player again under the revival rules (co-op; the idle AI in PvP is `net-window-host-leave-*`).

Each scenario opens one window (`--windows`). The pictures the scripts write are for a person to look at; the menu
screens' text is drawn on the GPU in the window and is not in a capture, so the logs are the checks.
"""
from __future__ import annotations

import re
import time

from battery import Drive, DriveError, Scenario

from battery_scenarios.net import (
    GAME_FLAGS, LOCALHOST, NET_BAD, away_problems, fresh_data, guide_mission, log_must, revive_problems, server_log,
    start_bots, start_server, weapons_hold,
)
from battery_scenarios.net_screens import game_log, wait_log

# The menu layer's clicks (640 by 480): Multi, Direct Connection, New; then the lobby's Settings... (the King's second
# button), the Game type (a click moves to the next value: PvP), the "AI flies idle aircraft after" row (a right click
# moves to the value before: 5 minutes, 2, 1), the Realism page's Damage (invulnerable, so that the enemy cannot shoot
# down the aircraft the AI flies while the game is away), the panel closed by Escape, the first plane taken; later
# Ready and Fly. The waits are long: the machine is shared.
HOST_LEAVE_SCRIPT = """wait 20
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
wait 25
snapshot SHOTS/hl-1-lobby.ppm
movemenu 160 431
wait 0.6
click
wait 2
movemenu 430 197
wait 0.6
click
wait 3
movemenu 430 344
wait 0.6
click right
wait 2
click right
wait 2
key Tab
wait 0.5
key Tab
wait 0.5
key Tab
wait 0.5
movemenu 275 175
wait 0.6
click
wait 2
snapshot SHOTS/hl-2-settings.ppm
key Escape
wait 2
movemenu 150 175
wait 0.6
click
wait 2
wait JOINWAIT
movemenu 393 429
wait 0.6
click
wait 3
snapshot SHOTS/hl-3-ready.ppm
movemenu 487 429
wait 0.6
click
waittick 480 150
wait 1
shot SHOTS/hl-4-flying.ppm
key Escape
wait 2
shot SHOTS/hl-5-menu.ppm
wait 75
shot SHOTS/hl-6-watching.ppm
key Escape
wait 2
shot SHOTS/hl-7-observer-menu.ppm
key Down
wait 1
key Enter
wait 3
shot SHOTS/hl-8-after-first-enter.ppm
CONFIRM
wait 8
shot SHOTS/hl-9-left.ppm
exit
"""

HOST_LEAVE_PICTURES = (
    "hl-1-lobby", "hl-2-settings", "hl-3-ready", "hl-4-flying", "hl-5-menu", "hl-6-watching", "hl-7-observer-menu",
    "hl-8-after-first-enter", "hl-9-left",
)


def host_leave_script(shots: str, standby: bool) -> str:
    """The script, with the second Enter only where the menu asks twice (a host with no standby)."""
    return (
        HOST_LEAVE_SCRIPT.replace("SHOTS", shots)
        .replace("JOINWAIT", "60")
        .replace("CONFIRM\n", "wait 1\nkey Enter\n" if not standby else "")
    )


def host_leave_problems(game: str, bot: str, standby: bool) -> list[str]:
    """What the hosting game's log and the pilot bot's lines must hold after the host's Leave Game on the observer
    menu, with a ready standby (it hands the game over) or with none (the menu asks twice and the game ends for
    everyone). Pure, unit tested in tools/test_battery_net.py."""
    problems = []
    for pattern, what in (
        (r"Lobby: Settings: idle-ai 1 minute\.", "the idle time turned to 1 minute"),
        (r"Network: away for the idle-ai time; the AI flies the plane", "the game saying it is away"),
        (r"Observer screen: watching the player's own aircraft", "the observer screen opening on its own plane"),
    ):
        if not re.search(pattern, game):
            problems.append(f"the game's log lacks {what}: /{pattern}/")
    leaves = re.findall(r"Network: Leave Game \(([^)]*)\)", game)
    want = "handing the game over" if standby else "ending the game"
    if leaves != [want]:
        problems.append(f"the game's Leave Game lines are {leaves}, not [{want!r}]")
    if standby and not re.search(r"Host: a ready standby: true", game):
        problems.append("the game's log lacks a ready standby: /Host: a ready standby: true/")
    if not standby and re.search(r"Host: a ready standby: true", game):
        problems.append("the game's log holds a ready standby with none standing by")
    if standby:
        for pattern, what in (
            (r"Host: handing the game over to player \d+", "the host handing over"),
            (r"Host: handed the game over to player \d+ after tick \d+", "the hand-over done"),
        ):
            if not re.search(pattern, game):
                problems.append(f"the game's log lacks {what}: /{pattern}/")
        for pattern, what in (
            (r"^Pilot: migrate: taking the game over$", "the standby taking the game over"),
            (r"^Pilot: host: took the game over at tick \d+", "the new host's resume"),
            (r"^Pilot: host: live at tick \d+", "the new host live"),
        ):
            if not re.search(pattern, bot, re.M):
                problems.append(f"the pilot lacks {what}: /{pattern}/")
        # The world carries on under the new host: its tick advances in the lines it prints once a second.
        after = bot.split("host: took the game over at tick", 1)[-1]
        ticks = [int(t) for t in re.findall(r"^Pilot: host: world: tick (\d+),", after, re.M)]
        if len(ticks) < 5 or ticks != sorted(set(ticks)):
            problems.append(f"the new host's world did not carry on: ticks {ticks[:8]}")
    else:
        for pattern, what in (
            (r"Host: tick \d+: mission ended: the host left the game", "the host ending the mission"),
        ):
            if not re.search(pattern, game):
                problems.append(f"the game's log lacks {what}: /{pattern}/")
        for pattern, what in (
            (r"^Pilot: Mission ended: the host left the game\.$", "the pilot's end notice"),
            (r"^Pilot: The host left the game\.$", "the host's goodbye"),
        ):
            if not re.search(pattern, bot, re.M):
                problems.append(f"the pilot lacks {what}: /{pattern}/")
        if re.search(r"handed the game over|taking the game over", game + bot):
            problems.append("a hand-over happened with no standby")
    return problems


def drive_host_leave(d: Drive, standby: bool) -> None:
    port = d.port()
    fresh_data(d)
    (d.data / "network-v1.conf").write_text(f"tore-network 1\ncallsign Viper\nport {port}\n")
    shots = d.work / "shots"
    shots.mkdir(exist_ok=True)
    # A scripted click on a window that is slow to come up can land before the menu answers; one more try, starting
    # later, tells that from a real failure (as `net-window-lobby` does).
    game = None
    for attempt, start_wait in enumerate((20, 45), start=1):
        for old in (d.data / "logs").glob("tore-*.log"):
            old.unlink()
        script = d.work / f"hostleave{attempt}.txt"
        script.write_text(host_leave_script(str(shots), standby).replace("wait 20\n", f"wait {start_wait}\n", 1))
        game = d.start(f"game{attempt}", [d.app, *GAME_FLAGS, "--input-script", script], window=True)
        if wait_log(d, r"Hosting Viper's game on UDP port", start_wait + 60):
            break
        d.log(f"attempt {attempt}: the scripted clicks did not open the lobby")
        game.stop()
    else:
        raise DriveError("the game never hosted from Direct Connection's New")
    assert game is not None
    # The default mission has room for the hosting player alone: the Game type turned to PvP opens the enemy planes
    # and the game's player limit, and the bot may then join.
    if not wait_log(d, r"Lobby: Settings: mode pvp", 90):
        raise DriveError("the script never turned the Game type to PvP")
    # A pilot in an enemy plane (the invulnerable host takes no harm from it) keeps the mission flying after the
    # host leaves. It stands by, or not, as the scenario says.
    bot = start_bots(
        d, port, "bot", 270, "--callsign", "Pilot", "--slot", "1", "--standby", "on" if standby else "off",
    )
    game.finish(420, 0)
    bot.finish(150, None)
    for problem in host_leave_problems(game_log(d), bot.text(), standby):
        d.problem(problem)
    game.forbid(NET_BAD, "a network problem")
    for name in HOST_LEAVE_PICTURES:
        if not (shots / f"{name}.ppm").exists():
            d.problem(f"the script's {name}.ppm was not written")
    d.log("game log:\n" + game_log(d)[-6000:])


def drive_host_leave_handover(d: Drive) -> None:
    drive_host_leave(d, True)


def drive_host_leave_confirm(d: Drive) -> None:
    drive_host_leave(d, False)


# The player's F/A-18D alone against ten aces 50 nm away: the AI that flies it for the away player is shot down about
# 50 seconds after the handoff (the same loss every run, 110 seconds into the mission), where the guide's mission's
# AI wings sometimes win. At 20 nm the enemy would arrive before the idle minute is up.
LONE_MISSION = """tore-mission 1
theater UKR
condition clear
start airborne 20000
separation-nm 50
preset free
guns-only no
wing friendly 1 F18.PT 1 experienced
wing enemy 1 SU27.PT 5 ace
wing enemy 2 MIG29.PT 5 ace
cheats none
"""

# The game flies plane 0 for four seconds and opens its flight menu (the controls go neutral); once the server's
# minute is up the AI flies the plane and the game watches it; the enemy arrives about 50 seconds later and shoots it
# down. The observer menu's first row is then Spawn in Aircraft: Enter flies the player again.
AWAY_LOST_SCRIPT = """wait 1
waittick 480 60
shot SHOTS/lost-1-flying.ppm
key Escape
wait 2
shot SHOTS/lost-2-menu.ppm
wait 150
shot SHOTS/lost-3-watching.ppm
key Escape
wait 2
shot SHOTS/lost-4-menu.ppm
key Enter
wait 12
shot SHOTS/lost-5-flying-again.ppm
exit
"""

AWAY_LOST_PICTURES = (
    "lost-1-flying", "lost-2-menu", "lost-3-watching", "lost-4-menu", "lost-5-flying-again",
)


def away_lost_problems(game: str, server: str) -> list[str]:
    """What the game's log and the server's log must hold after the AI lost the away player's aircraft and the observer
    menu's Spawn in Aircraft flew the player again. Pure, unit tested in tools/test_battery_net.py."""
    problems = []
    for pattern, what in (
        (r"Network: away for the idle-ai time; the AI flies the plane", "the game saying it is away"),
        (r"Observer screen: watching the player's own aircraft", "the observer screen opening on its own plane"),
        (r"Network: Spawn in Aircraft", "Spawn in Aircraft chosen"),
    ):
        if not re.search(pattern, game):
            problems.append(f"the game's log lacks {what}: /{pattern}/")
    if re.search(r"Take Back Flight", game):
        problems.append("the menu's first row was Take Back Flight: the AI did not lose the aircraft")
    seats = [int(n) for n in re.findall(r"Network: seated in plane (\d+)", game)]
    if len(seats) < 2 or seats[0] != 0 or seats[-1] == 0:
        problems.append(f"the game's seatings were {seats}, not plane 0 and then a plane of the revival")
    for pattern, count, what in (
        (r"Viper is away: the AI flies plane 0$", 1, "the handoff"),
        (r"Viper lost plane 0 while the AI flew it$", 1, "the loss"),
        (r"seat \d+ Viper took plane (?!0\b)\d+", 1, "Viper taking a new plane by Revive"),
    ):
        found = len(re.findall(pattern, server, re.M))
        if found != count:
            problems.append(f"the server's log holds {found} of {count} for {what}: /{pattern}/")
    if re.search(r"Viper is back", server):
        problems.append("the server's log holds the player taking the old plane back")
    return problems


def drive_away_lost(d: Drive) -> None:
    port = d.port()
    # Co-op against aces flying free, revival on (the registry's `respawn revive`), the idle time at its shortest and
    # no empty timeout, as a game a player hosts has it: without slice F2-X's fix the mission ended when the AI lost
    # the plane of its only player.
    server = start_server(d, port, LONE_MISSION, respawn="revive", idle_ai=1, empty_timeout=0)
    shots = d.work / "shots"
    shots.mkdir(exist_ok=True)
    script = d.work / "awaylost.txt"
    script.write_text(AWAY_LOST_SCRIPT.replace("SHOTS", str(shots)))
    game = d.start(
        "game", [d.app, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Viper", "--slot", "0", *GAME_FLAGS,
                 "--input-script", script],
        window=True,
    )
    game.finish(420, 0)
    server.send("quit")
    server.finish(30, None)
    for problem in away_lost_problems(game_log(d), server_log(d)):
        d.problem(problem)
    for name in AWAY_LOST_PICTURES:
        if not (shots / f"{name}.ppm").exists():
            d.problem(f"the script's {name}.ppm was not written")
    game.forbid(NET_BAD, "a network problem")
    log_must(d, server_log(d), r"Viper is away", forbid=NET_BAD)


def smoke_problems(texts: dict[str, str], mode: str) -> list[str]:
    """What the smoke test's bots printed (slice F2-X): the lobby's settings in every bot's words, a revival, an
    away player's handoff and return, and an observer that watched (with a delay in PvP only). Pure, unit tested in
    tools/test_battery_net.py."""
    problems = []
    for name, text in texts.items():
        if not re.search(rf"^{name}: settings: .*{'mode pvp' if mode == 'pvp' else 'idle-ai 1 minute'}", text, re.M):
            problems.append(f"{name} never saw the {mode} settings")
    problems.extend(revive_problems(texts["Phoenix"], "Phoenix", 12))
    # A player who is away sees what an observer sees: in PvP, with the King's observer delay behind.
    viper = texts["Viper"]
    if mode == "pvp":
        if not re.search(r"^Viper: observing from tick \d+, 10 s behind$", viper, re.M):
            problems.append("the away player was not watching 10 seconds behind, as every observer is in PvP")
        viper = viper.replace(", 10 s behind", ", 0 s behind")
    problems.extend(away_problems(viper, "Viper", 0 if mode == "coop" else 7))
    owl = texts["Owl"]
    if mode == "pvp":
        everyone = "\n".join(texts.values())
        if not re.search(r"^\w+: scores: .*; ends at 2 kills in all$", everyone, re.M):
            problems.append("no scores line names the kill limit (2 kills in all)")
        if not re.search(r"^\w+: Mission ended: the kill limit\.$", everyone, re.M):
            problems.append("the kill limit did not end the mission")
        if not re.search(r"^Owl: results: ", owl, re.M):
            problems.append("the observer was not given the results")
        if not re.search(r"^Owl: observing from tick \d+, 10 s behind$", owl, re.M):
            problems.append("the observer was not 10 seconds behind")
    elif not re.search(r"^Owl: observing from tick \d+, 0 s behind$", owl, re.M):
        problems.append("the observer was not live (observer delay is a PvP setting)")
    if not re.search(r"^Owl: watching: frames [1-9]\d*, aircraft [1-9]\d*$", owl, re.M):
        problems.append("the observer never drew frames with aircraft in them")
    if re.search(r"^Owl: (seat \d+|debrief)", owl, re.M):
        problems.append("the observer was given a plane or a debrief")
    return problems


def drive_smoke(d: Drive, mode: str) -> None:
    """The lead's smoke test without a window (slice F2-X): bots in the lobby's rules, a revival, an away player and
    an observer, in co-op or PvP. PvP adds a kill limit that ends the mission and an observer delay."""
    port = d.port()
    if mode == "pvp":
        server = start_server(
            d, port, weapons_hold(guide_mission(separation_nm=5)), mode="pvp", respawn="revive", observer_delay=10,
            idle_ai=1, kill_limit=2, kill_owner="total", time_limit=4,
        )
        phoenix_slot, viper_slot = "1", "7"
    else:
        server = start_server(d, port, weapons_hold(guide_mission()), respawn="revive", idle_ai=1)
        phoenix_slot, viper_slot = "1", "0"
    seconds = 200 if mode == "pvp" else 60
    fighters = []
    if mode == "pvp":
        # The two who fight for the kill limit: one in each side's first plane (the ejecting and the away player's
        # planes are beside them, so a kill is what ends the mission, as in `net-server-pvp`).
        fighters = [
            start_bots(d, port, "blue", seconds, "--callsign", "Blue", "--slot", "0"),
            start_bots(d, port, "red", seconds, "--callsign", "Red", "--slot", "6"),
        ]
    phoenix = start_bots(d, port, "phoenix", seconds, "--callsign", "Phoenix", "--slot", phoenix_slot, "--revive", "8")
    viper = start_bots(d, port, "viper", seconds, "--callsign", "Viper", "--slot", viper_slot, "--away", "10,6")
    if not server.wait_for(r"^mission started$", 60):
        raise DriveError("the mission never started")
    owl = start_bots(d, port, "owl", 35, "--callsign", "Owl", "--observe", "0")
    # In PvP the kill limit may end the mission while the observer, ten seconds behind, still watches.
    owl.finish(90, None if mode == "pvp" else 0)
    if mode == "pvp":
        if not server.wait_for(r"^mission ended: the kill limit$", 200):
            d.problem("the kill limit did not end the mission (the time limit is four minutes)")
    for bot in (phoenix, viper, *fighters):
        bot.finish(150, None)
    server.finish(60, None)
    texts = {"Phoenix": phoenix.text(), "Viper": viper.text(), "Owl": owl.text()}
    if mode == "pvp":
        texts["Blue"] = fighters[0].text()
        texts["Red"] = fighters[1].text()
    for problem in smoke_problems(texts, mode):
        d.problem(problem)
    for bot in (phoenix, viper, owl, *fighters):
        bot.forbid(NET_BAD, "a network problem")
    server.forbid(NET_BAD, "a network problem")


def drive_smoke_pvp(d: Drive) -> None:
    drive_smoke(d, "pvp")


def drive_smoke_coop(d: Drive) -> None:
    drive_smoke(d, "coop")


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="net-server-smoke-pvp", lane="net", args=[], driver=drive_smoke_pvp, uses=("server", "bot"),
            timeout=420,
            notes="the lead's smoke test, PvP: a server with a kill limit, revival, an observer delay and a one minute "
            "idle time; a bot ejects and flies again, one goes away and comes back, an observer watches 10 seconds "
            "behind (slice F2-X)",
        ),
        Scenario(
            name="net-server-smoke-coop", lane="net", args=[], driver=drive_smoke_coop, uses=("server", "bot"),
            timeout=300,
            notes="the lead's smoke test, co-op: revival, an away player and a live observer on the guide's mission "
            "(slice F2-X)",
        ),
        Scenario(
            name="net-window-host-leave-handover", lane="net", args=[], driver=drive_host_leave_handover,
            uses=("bot",), window=True, timeout=700,
            notes="a hosting game's away player chooses Leave Game on the observer menu with a ready standby: the "
            "game hands the game over (slice F2-X)",
        ),
        Scenario(
            name="net-window-host-leave-confirm", lane="net", args=[], driver=drive_host_leave_confirm,
            uses=("bot",), window=True, timeout=700,
            notes="a hosting game's away player chooses Leave Game with no standby: the menu asks twice and the game "
            "ends for everyone (slice F2-X)",
        ),
        Scenario(
            name="net-window-away-lost", lane="net", args=[], driver=drive_away_lost, uses=("server",),
            window=True, timeout=700,
            notes="an away player's aircraft is shot down while the AI flies it (co-op, revival on): the observer "
            "menu's first row is Spawn in Aircraft, which flies the player again in a new plane (slice F2-X)",
        ),
    ]
