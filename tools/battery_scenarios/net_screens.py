"""Lane: net. The game's screens through a rejoin and a host migration (stage K, slice K7b), in the real window.

Two windowed scenarios, each opening one window at a time (`--windows 1`):

`net-window-rejoin`: the game joins a `tore-server` with `--connect` and flies; it is killed in flight (SIGKILL, so no
Leave); the server keeps its aircraft for it; the game started again with the same data folder finds the token it kept
in `rejoin-v1.conf`, sends it, and is welcomed back in its aircraft.

`net-window-migrate`: a hosting `tore-bot` (Lead) and two pilots that stand by (`tore-bot --standby on`) fly with the game,
whose "Let my game take over hosting" switch is off (so a pilot, never the game, takes over); the hosting bot is killed
in flight. The game's HUD says "Lost contact with the host. Moving the game to Pilot1..." and then "The game moved to
Pilot1."; the pictures the script writes show the HUD for a person to look at. It needs the hosting bot of slice K9.
"""
from __future__ import annotations

import re
import time

from battery import Drive, DriveError, Scenario

from battery_scenarios.net import (
    GAME_FLAGS, LOCALHOST, NET_BAD, fresh_data, guide_mission, log_must, server_log, start_server,
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
    first.stopped = True
    first.popen.kill()
    first.wait(10)
    if not server.wait_for(r"Viper dropped out: the AI flies plane 0, kept for it$", 40):
        d.problem("the server did not keep plane 0 for the dropped game")
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
    ]
