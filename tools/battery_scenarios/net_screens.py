"""Lane: net. The game's screens through a rejoin and a host migration (stage K, slice K7b), in the real window.

Two windowed scenarios, each opening one window at a time (`--windows 1`):

`net-window-rejoin`: the game joins a `tore-server` with `--connect` and flies; it is killed in flight (SIGKILL, so no
Leave); the server keeps its aircraft for it; the game started again with the same data folder finds the token it kept
in `rejoin-v1.conf`, sends it, and is welcomed back in its aircraft.

`net-window-migrate`: a hosting `tore-bot` (Lead) and two pilots that stand by (`tore-bot --standby on`) fly with the game,
whose "Let my game take over hosting" switch is off (so a pilot, never the game, takes over); the hosting bot is killed
in flight. The game's HUD says "Lost contact with the host. Moving the game to Pilot1..." and then "The game moved to
Pilot1."; the pictures the script writes show the HUD for a person to look at. It needs the hosting bot of slice K9.

`net-window-migrate-smoke`: the lead's smoke test of slice K10, the shape of John's three-machine check. The game hosts in
the window, listed on a `tore-master` on this machine; two pilots that stand by join it directly, a bot joins through the
master's relay and Viper keeps its rejoin token in a file. The hosting game is killed in the fight (SIGKILL); a pilot
takes the game over, the relayed bot keeps its channel, and Viper, killed after the migration and started again
through the master with its token, is welcomed back into the aircraft the new host kept for it.
"""
from __future__ import annotations

import os
import re
import signal
import time
from pathlib import Path

from battery import Drive, DriveError, Scenario

from battery_scenarios.net import (
    GAME_FLAGS, LOCALHOST, NET_BAD, SNAPSHOTS_AGAIN_MS, fresh_data, guide_mission, log_must, migrate_problems,
    rejoin_problems, server_log, start_master, start_server,
)

# The game flies about four seconds and leaves (only reached when it is not killed first).
REJOIN_SCRIPT = "wait 1\nwaittick 600 90\nexit\n"
# The game flies about ten seconds, then pictures of the flight every half second for seven, then leaves. The driver
# kills the host nine seconds after the game is seated, so the loss is noticed (1.5 s) just after the pictures begin.
MIGRATE_SCRIPT = (
    "wait 1\nwaittick 1200 120\n"
    + "".join(f"shot SHOTS/migrate-{i:02d}.ppm\nwait 0.5\n" for i in range(1, 15))
    + "exit\n"
)


def game_log(d: Drive) -> str:
    """The game's own log files in the data folder (`logs/tore-DATE.log`)."""
    return "\n".join(p.read_text(errors="replace") for p in sorted((d.data / "logs").glob("tore-*.log")))


def wait_log(d: Drive, pattern: str, seconds: float, after: int = 0) -> bool:
    """Waits until the game's log holds `pattern` past its first `after` characters."""
    end = time.time() + seconds * d.scale
    while time.time() < end:
        d._check_time()
        if re.search(pattern, game_log(d)[after:]):
            return True
        time.sleep(0.25)
    return False


def kill_game(d: Drive, proc) -> None:
    """Kills a windowed game hard (SIGKILL, so no Leave). Under Hyprland `tools/agent-run.sh` starts the game from the
    compositor, so killing the wrapper would leave the game flying; the game's own log file is named with its process
    id, so the game itself is killed and the wrapper ends with it (slice K10)."""
    pids = set()
    for path in (d.data / "logs").glob("tore-*.log"):
        m = re.search(r"-(\d+)-\d+\.log$", path.name)
        if not m:
            continue
        pid = int(m.group(1))
        try:
            if Path(f"/proc/{pid}/comm").read_text().strip() == "tore-app":
                pids.add(pid)
        except OSError:
            pass
    if not pids:
        # Not Linux, or the log is not there: the wrapper is the game.
        proc.popen.kill()
    for pid in pids:
        os.kill(pid, signal.SIGKILL)
    proc.stopped = True
    proc.wait(15)


def drive_rejoin_window(d: Drive) -> None:
    """The window's rejoin: killed in flight, started again, welcomed back in plane 0."""
    port = d.port()
    server = start_server(d, port, guide_mission(), empty_timeout=90)
    script = d.work / "rejoin.txt"
    script.write_text(REJOIN_SCRIPT)
    argv = [d.app, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Viper", "--slot", "0", *GAME_FLAGS,
            "--input-script", script]
    first = d.start("game", argv, window=True)
    if not wait_log(d, r"Network: seated in plane 0", 120):
        raise DriveError("the game was never seated")
    d.sleep(3)
    kept = d.data / "rejoin-v1.conf"
    if not kept.exists() or "token " not in kept.read_text():
        raise DriveError("the game kept no token in rejoin-v1.conf")
    # Killed in flight: no Leave, no goodbye.
    d.log("killing the game (SIGKILL)")
    kill_game(d, first)
    if not server.wait_for(r"Viper dropped out: the AI flies plane 0, kept for it$", 40):
        d.problem("the server did not keep plane 0 for the dropped game")
    if not server.wait_for(r"Viper \(plane 0\) left: no packet for 5 seconds$", 20):
        d.problem("the server did not see the killed game go silent (did it leave cleanly instead?)")
    d.sleep(1)
    before = len(game_log(d))
    second = d.start("game2", argv, window=True)
    second.finish(150, 0)
    log = game_log(d)[before:]
    for pattern, what in (
        (r"Network: Welcome back, Viper: your aircraft is waiting\.", "the host's welcome"),
        (r"Network: seated in plane 0", "the aircraft taken back"),
    ):
        if not re.search(pattern, log):
            d.problem(f"the game's log lacks {what}: /{pattern}/")
    if re.search(NET_BAD + r"|does not know your rejoin token", log):
        d.problem("a network problem or a token refused in the game's log")
    server.send("end")
    server.finish(40, 0)
    log_must(
        d,
        server_log(d),
        r"Viper dropped out: the AI flies plane 0, kept for it$",
        r"Viper rejoined with its token: plane 0 is waiting$",
        forbid=r"protocol error|bad packets|\bfault\b",
    )


def drive_migrate_window(d: Drive) -> None:
    """The window's migration: the hosting bot is killed in flight and the game's HUD says so."""
    fresh_data(d)
    # The game does not stand by: a pilot takes over, and the switch's own file is what says so.
    (d.data / "network-v1.conf").write_text("tore-network 1\ncallsign Viper\nmay-host no\n")
    port = d.port()
    mission = d.work / "mission.txt"
    mission.write_text(guide_mission(5))
    shots = d.work / "shots"
    shots.mkdir(exist_ok=True)
    script = d.work / "migrate.txt"
    script.write_text(MIGRATE_SCRIPT.replace("SHOTS", str(shots)))
    host = d.start(
        "host",
        [d.bot, "--host", mission, "--port", port, "--callsign", "Lead", "--slot", "0", "--seconds", 400,
         "--players", "4", "--wait-standbys", 1],
    )
    if not host.wait_for(r"^Lead: hosting ", 60):
        raise DriveError("the hosting bot never began to host")
    pilots = d.start(
        "pilots",
        [d.bot, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Pilot", "--count", "2", "--slot", "1",
         "--seconds", 120, "--standby", "on"],
    )
    game = d.start(
        "game",
        [d.app, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Viper", "--slot", "3", *GAME_FLAGS,
         "--input-script", script],
        window=True,
    )
    if not host.wait_for(r"^Lead: host: mission started", 150):
        raise DriveError("the hosting bot never started the mission (no standby ready?)")
    if not wait_log(d, r"Network: seated in plane \d+", 150):
        raise DriveError("the game was never seated")
    d.sleep(9)
    d.log("killing the host (SIGKILL)")
    host.stopped = True
    host.popen.kill()
    host.wait(10)
    game.finish(150, 0)
    log = game_log(d)
    for pattern, what in (
        (r"Network: Lost contact with the host\. Moving the game to Pilot\d\.\.\.", "the loss line"),
        (r"Network: The game moved to Pilot\d\.", "the moved line"),
    ):
        if not re.search(pattern, log):
            d.problem(f"the game's log lacks {what}: /{pattern}/")
    for pattern, what in (
        (NET_BAD + r"|No other game could take over", "a network problem or a session given up"),
        (r"Host: .*took the game over", "the game taking over (its switch is off)"),
    ):
        if re.search(pattern, log):
            d.problem(f"the game's log holds {what}: /{pattern}/")
    pilots.finish(150, 0)
    pilots.expect(r"^Pilot\d: migrate: snapshots again \d+ ms", "a pilot's snapshots again")
    pictures = sorted(shots.glob("migrate-*.ppm"))
    if len(pictures) < 10:
        d.problem(f"the script wrote {len(pictures)} pictures, expected 14")


# The hosting game flies until the driver kills it; pictures of its fight every two seconds once it flies.
SMOKE_SCRIPT = (
    "wait 1\nwaittick 600 120\n"
    + "".join(f"shot SHOTS/host-{i:02d}.ppm\nwait 2\n" for i in range(1, 31))
    + "exit\n"
)


def drive_migrate_smoke(d: Drive) -> None:
    """The lead's smoke test (slice K10): the hosting game killed in a fight, one player relayed, one rejoin."""
    fresh_data(d)
    master, mport = start_master(d)
    port = d.port()
    name = "Migrate smoke"
    mission = d.work / "mission.txt"
    mission.write_text(guide_mission(5))
    shots = d.work / "shots"
    shots.mkdir(exist_ok=True)
    script = d.work / "smoke.txt"
    script.write_text(SMOKE_SCRIPT.replace("SHOTS", str(shots)))
    game = d.start(
        "game",
        [d.app, "--host", mission, "--port", port, "--name", name, "--list", "--master", f"{LOCALHOST}:{mport}",
         "--callsign", "Host", "--slot", "0", *GAME_FLAGS, "--input-script", script],
        window=True,
    )
    if not wait_log(d, r"Network: seated in plane 0", 150):
        raise DriveError("the hosting game was never seated")
    if not master.wait_for(r'^listed id=[0-9a-f]{16} from=127\.0\.0\.1:\d+ name="Migrate smoke"', 60):
        raise DriveError("the master never listed the hosted game")
    pilots = d.start(
        "pilots",
        [d.bot, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Pilot", "--count", "2", "--slot", "1",
         "--seconds", 150, "--standby", "on"],
    )
    relay = d.start(
        "relay",
        [d.bot, "--master", f"{LOCALHOST}:{mport}", "--listing", name, "--path", "relay", "--callsign", "Relay",
         "--slot", "3", "--seconds", 150],
    )
    # Viper may not host but answers the host's reach tests, as a game with its switch off does (a bot with no
    # --standby runs no peers router, and a player who never reaches the candidates leaves none eligible).
    token_file = d.work / "viper.token"
    viper = d.start(
        "viper",
        [d.bot, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Viper", "--slot", "4", "--seconds", 150,
         "--token-file", token_file, "--standby", "off"],
    )
    for who in (r"Pilot\d", "Relay", "Viper"):
        proc = relay if who == "Relay" else viper if who == "Viper" else pilots
        if not proc.wait_for(rf"^{who}: seat \d+, plane \d+, at tick \d+$", 120):
            raise DriveError(f"{who} was never seated")
    if not pilots.wait_for(r"^Pilot\d: standby: checkpoint \{ tick: \d+, restored: true \}$", 90):
        raise DriveError("no pilot's standby became ready")
    # The fight: the enemy is 5 nm away, so missiles fly within seconds.
    d.sleep(15)
    d.log("killing the hosting game (SIGKILL)")
    kill_game(d, game)
    if not relay.wait_for(r"^Relay: migrate: snapshots again \d+ ms", 30):
        d.problem("the relayed bot's snapshots never came again after the host was killed")
    if not viper.wait_for(r"^Viper: migrate: snapshots again \d+ ms", 30):
        d.problem("Viper's snapshots never came again after the host was killed")
    took = re.search(r"^(Pilot\d): migrate: taking the game over$", pilots.text(), re.M)
    if not took:
        raise DriveError("no pilot took the game over")
    new = took.group(1)
    d.sleep(3)
    # Viper drops out of the new host's game and comes back with its token through the master.
    d.log("killing Viper (SIGKILL)")
    viper.stopped = True
    viper.popen.kill()
    viper.wait(10)
    d.sleep(8)
    viper2 = d.start(
        "viper2",
        [d.bot, "--master", f"{LOCALHOST}:{mport}", "--listing", name, "--callsign", "Viper", "--slot", "4",
         "--seconds", 20, "--token-file", token_file],
    )
    viper2.finish(120, 0)
    for problem in rejoin_problems(viper2.text(), "Viper", 4):
        d.problem(problem)
    pilots.finish(240, 0)
    relay.finish(240, 0)
    together = pilots.text() + "\n" + relay.text() + "\n" + viper.text()
    for problem in migrate_problems(together, ["Pilot1", "Pilot2", "Relay", "Viper"], None, SNAPSHOTS_AGAIN_MS * d.scale):
        d.problem(problem)
    relay.expect(r"^Relay: joined through the Internet Lobby, path relay$", "the relayed join")
    relay.forbid(r"The relay closed|No other game could take over", "a lost relay")
    if not re.search(rf"^{new}: listing: resumed from the old host's part$", pilots.text(), re.M):
        d.problem("the new host never resumed the listing from the old host's part")
    if not re.search(rf"^{new}: host: Viper .*no packet for 5 seconds", pilots.text(), re.M):
        d.problem("the new host never saw Viper drop out")
    for text in (pilots.text(), relay.text(), viper2.text()):
        if re.search(NET_BAD, text):
            d.problem("a network problem: " + re.search(NET_BAD, text).group(0))
    master.send("quit")
    master.finish(20, 0)
    master.expect(r"^relay moved listing=[0-9a-f]{16} from=127\.0\.0\.1:\d+ to=127\.0\.0\.1:\d+ channels=\d+$",
                  "the relay channel moved with the listing")
    if len(sorted(shots.glob("host-*.ppm"))) < 3:
        d.problem("the hosting game wrote fewer than 3 pictures of its fight")


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="net-window-rejoin", lane="net", args=[], driver=drive_rejoin_window, uses=("server",), window=True,
            timeout=360,
            notes="the game's rejoin in the window: it joins a server, is killed in flight (SIGKILL), the server keeps "
            "its aircraft, and the game started again sends the token it kept in rejoin-v1.conf and is welcomed back",
        ),
        Scenario(
            name="net-window-migrate", lane="net", args=[], driver=drive_migrate_window, uses=("bot",), window=True,
            timeout=480,
            notes="the game's HUD through a host migration: a hosting bot and two standby pilots fly with the game "
            "(its take-over switch off); the host is killed; the HUD says the game is moving and then that it moved "
            "(slice K7b; needs the hosting bot of K9)",
        ),
        Scenario(
            name="net-window-migrate-smoke", lane="net", args=[], driver=drive_migrate_smoke, uses=("bot",),
            window=True, timeout=600,
            notes="the lead's smoke test (slice K10): the game hosts, listed on a local master; it is killed in the "
            "fight; a standby pilot takes over, a relayed bot keeps its channel, and a bot killed after the migration "
            "rejoins its kept aircraft through the master with its token",
        ),
    ]
