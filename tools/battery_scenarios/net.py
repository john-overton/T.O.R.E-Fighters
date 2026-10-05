"""Lane: multiplayer on this machine, over real UDP on 127.0.0.1.

Every scenario is a driver (`Scenario.driver`) that owns several processes: a
`tore-server` and `tore-bot`s, or a hosting `tore-app --host` and a bot. The
scenarios whose processes are headless run with the rest; the two that need the
game itself (a joined game that stalls, a hosted game) open a window through
tools/agent-run.sh like any windowed scenario, because the stall script and the
hosting thread live in the game's window loop.

Each scenario takes its own ports from `Drive.port()`, so they run in parallel.
The mission is the example in docs/DEDICATED-SERVER.md, read from the guide so
the two stay in step. See docs/testing/lane-net.md.
"""
from __future__ import annotations

import errno
import re
import shutil
import socket
import time
from pathlib import Path

from battery import ROOT, Drive, DriveError, Proc, Scenario

GUIDE = ROOT / "docs" / "DEDICATED-SERVER.md"
LOCALHOST = "127.0.0.1"

# Output that means the network went wrong, in a bot's words or the server's.
NET_BAD = (
    r"a protocol error|too many bad packets|no packets for 5 seconds|the game data differs|"
    r"No answer from the server|[Rr]efused|\bfault\b|silent"
)


def guide_mission(separation_nm: int | None = None) -> str:
    """The example mission of the dedicated server guide, optionally with the enemy closer."""
    text = GUIDE.read_text()
    m = re.search(r"```text\n(tore-mission 1\n.*?)```", text, re.S)
    if not m:
        raise DriveError("the guide's example mission is missing from docs/DEDICATED-SERVER.md")
    mission = m.group(1)
    if separation_nm is not None:
        mission = re.sub(r"(?m)^separation-nm .*$", f"separation-nm {separation_nm}", mission)
    return mission


def fresh_data(d: Drive) -> None:
    """Empties the scenario's copy of the logs, replays and remembered settings, so every file the checks find
    was written by this run (the copy is the scenario's own; the profile it came from is not touched)."""
    for name in ("logs", "replays"):
        shutil.rmtree(d.data / name, ignore_errors=True)
    (d.data / "network-v1.conf").unlink(missing_ok=True)


def write_server_files(d: Drive, port: int, mission: str | None = None, **settings) -> Path:
    """Writes `server.conf` and `mission.txt` into the work folder; returns the configuration path."""
    (d.work / "mission.txt").write_text(mission if mission is not None else guide_mission())
    lines = {"port": port, "mission": "mission.txt", "status-interval": 5, "empty-timeout": 3, "after-end": "quit"}
    lines.update({k.replace("_", "-"): v for k, v in settings.items()})
    config = d.work / "server.conf"
    config.write_text("".join(f"{k} {v}\n" for k, v in lines.items()))
    return config


def start_server(d: Drive, port: int, mission: str | None = None, **settings) -> Proc:
    """Starts a `tore-server` with a console on its standard input and waits until it listens."""
    fresh_data(d)
    config = write_server_files(d, port, mission, **settings)
    server = d.start("server", [d.server, "--config", config], stdin=True)
    if not server.wait_for(r"Waiting for players", 90):
        raise DriveError("the server never said it was waiting for players")
    return server


def start_bots(d: Drive, port: int, label: str = "bot", seconds: int = 30, *extra: str) -> Proc:
    return d.start(label, [d.bot, "--connect", f"{LOCALHOST}:{port}", "--seconds", str(seconds), *extra])


def server_log(d: Drive) -> str:
    """The server's own log file in the data folder (`logs/server-DATE.log`)."""
    return "\n".join(p.read_text(errors="replace") for p in sorted((d.data / "logs").glob("server-*.log")))


def log_must(d: Drive, text: str, *patterns: str, forbid: str = "") -> None:
    """Problems for patterns the log file lacks, or one it holds that it must not."""
    if not text.strip():
        d.problem("the server wrote no log file")
        return
    for pattern in patterns:
        if not re.search(pattern, text, re.M):
            d.problem(f"server log: missing /{pattern}/")
    if forbid:
        m = re.search(forbid, text, re.M)
        if m:
            d.problem(f"server log: forbidden /{forbid}/: {m.group(0)[:200]}")


def port_in_use(port: int) -> bool:
    """True when something on this machine already holds the UDP port (the game that hosts there)."""
    for family, host in ((socket.AF_INET6, "::"), (socket.AF_INET, "0.0.0.0")):
        try:
            probe = socket.socket(family, socket.SOCK_DGRAM)
        except OSError:
            continue
        try:
            probe.bind((host, port))
        except OSError as e:
            if e.errno == errno.EADDRINUSE:
                return True
        finally:
            probe.close()
    return False


def wait_for_listener(d: Drive, port: int, proc: Proc, seconds: float) -> None:
    """Waits until the game `proc` holds the port. The search itself is not used to ask: it takes the game port
    when it is free, and a hosting game that starts listening a moment later would find it taken."""
    end = time.time() + seconds * d.scale
    while not port_in_use(port):
        if not proc.alive():
            raise DriveError(f"{proc.label} ended before it listened on port {port}")
        if time.time() > end:
            raise DriveError(f"{proc.label} did not listen on port {port} within {seconds:.0f}s")
        d.sleep(0.25)


def stop_server(d: Drive, server: Proc) -> None:
    """The console's `quit`: the server says it stopped and exits 0."""
    server.send("quit")
    server.finish(20, 0)
    server.expect(r"^Stopped$", "the stop line")


# --------------------------------------------------------------------------
# Pure checks (unit tested in tools/test_battery_net.py)
# --------------------------------------------------------------------------


def check_report_problems(text: str) -> list[str]:
    """What `tore-server --check` must print for the guide's example mission."""
    problems = []
    if not re.search(r"^Mission: UKR \(clear\), airborne at 20000 ft, enemy 20 nm away; .*12 aircraft$", text, re.M):
        problems.append("the mission summary line is missing or wrong")
    planes = re.findall(r"^\s+(\d+)\s+(F18\.PT|F14\.PT|MIG29\.PT|SU27\.PT)\s+(friendly|enemy) wing \d, member \d, \w+$", text, re.M)
    if [int(n) for n, _, _ in planes] != list(range(12)):
        problems.append(f"expected planes 0 to 11 in order, found {[n for n, _, _ in planes]}")
    if sum(1 for _, _, side in planes if side == "friendly") != 6:
        problems.append("expected six friendly planes")
    if not re.search(r"^Runways of UKR", text, re.M):
        problems.append("the runway list is missing")
    if not re.search(r"^Content manifest: \d+ resources, digest [0-9a-f]{16}$", text, re.M):
        problems.append("the content manifest line is missing")
    return problems


def players_table(text: str) -> list[dict]:
    """The rows of the console's `players` table: lobby id, seat, callsign and plane."""
    rows = []
    for m in re.finditer(r"^(\d+)\s+(\d+|-)\s+(\S+)\s+(\d+|-)\s+\d+ ms", text, re.M):
        rows.append({"id": int(m.group(1)), "seat": m.group(2), "callsign": m.group(3), "plane": m.group(4)})
    return rows


def net_log_problems(text: str) -> list[str]:
    """A game's `logs/net-DATE.tsv`: the header, the join, the seating, once-a-second figures, and the end."""
    lines = [line.split("\t") for line in text.splitlines() if line.strip()]
    if not lines or lines[0][:2] != ["seconds", "kind"]:
        return ["net log has no header line"]
    kinds = [row[1] for row in lines[1:] if len(row) > 1]
    problems = [f"net log has no `{k}` line" for k in ("connect", "joined", "mission", "seated") if k not in kinds]
    if kinds.count("stats") < 5:
        problems.append(f"net log has only {kinds.count('stats')} stats lines")
    width = len(lines[0])
    bad = [row for row in lines[1:] if row[1] == "stats" and len(row) != width]
    if bad:
        problems.append("a stats line does not have the header's columns")
    return problems


def figures_problems(log: str, callsigns: list[str]) -> list[str]:
    """The once-a-minute figures a server logs for each player."""
    problems = []
    for name in callsigns:
        if not re.search(rf"figures seat \d+ {name} plane \d+: round trip \d+ ms, loss", log):
            problems.append(f"the server log has no figures line for {name}")
    return problems


# --------------------------------------------------------------------------
# Drivers
# --------------------------------------------------------------------------


def drive_check(d: Drive) -> None:
    config = write_server_files(d, d.port())
    run = d.run("server", [d.server, "--config", config, "--check"], timeout=60)
    for problem in check_report_problems(run.text()):
        d.problem(problem)


def drive_fight(d: Drive) -> None:
    """A server and two bots fly a short fight; both join, fly and leave, and the server stops by itself."""
    port = d.port()
    server = start_server(d, port, guide_mission(separation_nm=5))
    bots = start_bots(d, port, "bots", 75, "--count", "2", "--callsign", "Bot")
    bots.finish(120, 0)
    server.finish(40, 0)
    for n in (1, 2):
        bots.expect(rf"^Bot{n}: joined$", "a join")
        bots.expect(rf"^Bot{n}: seat \d+, plane \d+, at tick \d+$", "a seating")
        bots.expect(rf"^Bot{n}: debrief: (success|failure), \d+ kills, \d+ seconds$", "a debrief")
        bots.expect(rf"^Bot{n}: The connection ended: the player left\.$", "a clean leave")
        bots.expect(rf"^Bot{n}: round trip \d+ ms, loss", "flight figures")
    bots.forbid(NET_BAD, "a network problem")
    server.expect(r"^mission started$", "the start")
    server.expect(r"players 2/6 aircraft 12", "a status line with both players")
    server.expect(r"^mission ended: everyone left$", "the end")
    server.expect(r"^The last mission has ended\. Stopping\.$", "the stop")
    server.forbid(NET_BAD, "a network problem")
    log = server_log(d)
    log_must(
        d, log, r"joined as Bot1", r"joined as Bot2", r"seat \d+ Bot1 \(plane \d+\) left: left",
        r"seat \d+ Bot2 \(plane \d+\) left: left", r"mission ended: everyone left", forbid=NET_BAD,
    )
    if len(re.findall(r"^Bot\d: .* bursts [1-9]\d*,", bots.text(), re.M)) == 0:
        d.problem("neither bot fired a burst in 75 seconds with the enemy 5 nm away")
    for problem in figures_problems(log, ["Bot1", "Bot2"]):
        d.problem(problem)


def drive_chat(d: Drive) -> None:
    """Chat: free text and CHAT.TXT's quick lines go through the host to the other players, and are logged."""
    port = d.port()
    server = start_server(d, port, empty_timeout=2)
    bots = start_bots(
        d, port, "bots", 22, "--count", "2", "--callsign", "Bot",
        "--say", "5,all,Hello from the battery", "--say", "7,friendlies,Form up", "--say", "9,enemies,Anyone there",
        "--quick", "11,1",
    )
    bots.finish(90, 0)
    server.finish(40, 0)
    for to, text in (("all", "Hello from the battery"), ("friendlies", "Form up")):
        for sender in ("Bot1", "Bot2"):
            other = "Bot2" if sender == "Bot1" else "Bot1"
            bots.expect(rf"^{other}: chat: {sender} to {to}: {text}", f"{sender}'s line to {to} heard by {other}")
    # A line to the enemy side nobody holds is answered, not dropped, and logged as a refusal or as unheard.
    bots.expect(r"^Bot\d: chat: .*(no one|No one|nobody)", "the host's answer to a line nobody hears")
    bots.expect(r"^Bot\d: chat: Bot\d to all: .+ \[\^\w+\.\d+K\]$", "a quick chat line with its sound")
    bots.forbid(NET_BAD.replace("[Rr]efused|", ""), "a network problem")
    log = server_log(d)
    log_must(
        d, log, r"chat: Bot1 to all \(1 heard\): Hello from the battery",
        r"chat: Bot2 to friendlies \(1 heard\): Form up",
        r"chat: Bot1 to enemies \(no one heard\): Anyone there",
        r"chat: Bot\d to all \(1 heard\): (?!Hello)\S",
    )


def drive_kick(d: Drive) -> None:
    """The console: players, status, kick by seat, kick-player by id with a reason, end."""
    port = d.port()
    server = start_server(d, port, empty_timeout=10)
    stay = start_bots(d, port, "stay", 60, "--callsign", "Stay", "--slot", "1")
    byseat = start_bots(d, port, "byseat", 60, "--callsign", "GoneA", "--slot", "0")
    byid = start_bots(d, port, "byid", 60, "--callsign", "GoneB", "--slot", "2")
    for name in ("Stay", "GoneA", "GoneB"):
        if not server.wait_for(rf"seat \d+ {name} took plane", 60):
            raise DriveError(f"{name} never took a plane")
    d.sleep(3)
    server.send("players")
    if not server.wait_for(r"^id\s+seat\s+callsign", 10):
        raise DriveError("the console did not answer `players`")
    d.sleep(0.5)
    rows = {row["callsign"]: row for row in players_table(server.text())}
    for name, plane in (("Stay", "1"), ("GoneA", "0"), ("GoneB", "2")):
        if rows.get(name, {}).get("plane") != plane:
            d.problem(f"the players table does not show {name} in plane {plane}: {rows.get(name)}")
    server.send("status")
    # Seats are given in the order the games ask, so read GoneA's from the table.
    seat_a = rows.get("GoneA", {}).get("seat", "?")
    server.send(f"kick {seat_a}")
    if not server.wait_for(rf"seat {seat_a} GoneA \(plane 0\) left: kicked", 20):
        d.problem(f"the server did not log the kick of seat {seat_a}")
    if "GoneB" in rows:
        server.send(f"kick-player {rows['GoneB']['id']} testing the console")
        if not server.wait_for(r"GoneB was kicked: testing the console", 20):
            d.problem("the server did not log the removal of GoneB")
    byseat.finish(20, 1)
    byid.finish(20, 1)
    byseat.expect(r"^GoneA: The server ended the connection: kicked by the server\.$", "the kick's words")
    byid.expect(r"^GoneB: The server removed you from the game: testing the console$", "the removal's words")
    server.expect(r"players \d+/6 aircraft", "a status line")
    d.sleep(1)
    server.send("end")
    if not stay.wait_for(r"^Stay: debrief: ", 30):
        d.problem("the player left in the mission got no debrief from `end`")
    stay.wait_for(r"server is stopping", 30)
    stay.finish(40, None)
    server.finish(40, 0)
    stay.expect(r"^Stay: Mission ended by the host\.$", "the end notice")
    server.expect(r"^mission ended: ended from the console$", "the mission end")
    server.forbid(r"\bfault\b|protocol error|bad packets", "a network problem")


def drive_discovery(d: Drive) -> None:
    """`tore-app --find-games` lists a server on this machine, and says so when there is none."""
    port = d.port()
    server = start_server(d, port, name="Battery discovery")
    found = d.run("find", [d.app, "--find-games", "3", "--port", str(port)], timeout=40)
    found.expect(
        rf"^\S+:{port}\tsame build \([^)]+\)\tBattery discovery\tUKR, clear, airborne at 20000 ft: .*\t0/6 players\tlobby\tking -\topen\tnot full$",
        "the server's line",
    )
    nothing = d.run("empty", [d.app, "--find-games", "1.5", "--port", str(d.port())], timeout=40)
    nothing.expect(r"^No games found\.$", "the empty answer")
    stop_server(d, server)


# The script a joined game runs: wait for the flight, block the whole main loop, then leave.
STALL_SCRIPT = "wait 1\nwaittick 480 60\nstall 4\nwaittick 1500 60\nexit\n"
# The hosting game flies for twenty seconds of flight, then leaves.
HOST_SCRIPT = "wait 1\nwaittick 2400 90\nexit\n"
GAME_FLAGS = ["--no-audio", "--windowed"]


def drive_stall(d: Drive) -> None:
    """A joined game whose main loop stalls for 4 s stays connected (the keepalive), and writes its net files."""
    port = d.port()
    server = start_server(d, port)
    script = d.work / "stall.txt"
    script.write_text(STALL_SCRIPT)
    game = d.start(
        "game", [d.app, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Viper", *GAME_FLAGS, "--input-script", script],
        window=True,
    )
    game.finish(120, 0)
    game.expect(r"^Input script: stalling 4 s$", "the stall")
    game.expect(r"^Input script: stall over$", "the end of the stall")
    server.expect(r"seat 0 Viper: game stalled, flying neutral", "the stall")
    server.expect(r"seat 0 Viper: game back after [3-9]\.\d s", "the resume")
    server.expect(r"seat 0 Viper \(plane 0\) left: left", "a clean leave")
    stop_server(d, server)
    log = server_log(d)
    log_must(d, log, r"Viper: game stalled, flying neutral", r"Viper: game back after", forbid=r"silent|protocol error")
    tsv = sorted((d.data / "logs").glob("net-*.tsv"))
    if not tsv:
        d.problem("the game wrote no logs/net-DATE.tsv")
    for path in tsv:
        for problem in net_log_problems(path.read_text(errors="replace")):
            d.problem(f"{path.name}: {problem}")
    captures = [p for p in (d.data / "replays").glob("*_NET_*.tore-capture") if p.stat().st_size > 0]
    if not captures:
        d.problem("the game wrote no network capture in replays/")


def drive_host(d: Drive) -> None:
    """A hosted game: it shows up in the search, a bot joins and flies, and the host leaving ends it for the bot."""
    port = d.port()
    fresh_data(d)
    mission = d.work / "host-mission.txt"
    mission.write_text(guide_mission())
    script = d.work / "host.txt"
    script.write_text(HOST_SCRIPT)
    host = d.start(
        "host",
        [d.app, "--host", mission, "--port", str(port), "--name", "Battery host", "--callsign", "Host", *GAME_FLAGS,
         "--input-script", script],
        window=True,
    )
    # The game opens its window, loads the mission and then listens; once it does, the search must find it.
    wait_for_listener(d, port, host, 90)
    seen = d.run("find", [d.app, "--find-games", "3", "--port", str(port)], timeout=40)
    seen.expect(
        rf"^\S+:{port}\tsame build \([^)]+\)\tBattery host\t.*\t\d+/6 players\t(lobby|flying)\tking (Host|-)\topen\tnot full$",
        "the hosted game's line",
    )
    bot = start_bots(d, port, "bot", 90, "--callsign", "Joiner")
    bot.finish(120, 0)
    host.finish(60, 0)
    bot.expect(r"^Joiner: joined$", "a join")
    bot.expect(r"^Joiner: seat \d+, plane \d+, at tick \d+$", "a seating")
    bot.expect(r"^Joiner: round trip \d+ ms, loss", "flight figures")
    bot.expect(r"^Joiner: Mission ended: the host left the game\.$", "the end notice")
    bot.expect(r"^Joiner: debrief: ", "a debrief")
    bot.expect(r"^Joiner: The host left the game\.$", "the host's goodbye")
    bot.forbid(NET_BAD, "a network problem")
    tsv = sorted((d.data / "logs").glob("net-*.tsv"))
    if not tsv:
        d.problem("the hosting game wrote no logs/net-DATE.tsv")
    for path in tsv:
        for problem in net_log_problems(path.read_text(errors="replace")):
            d.problem(f"{path.name}: {problem}")
    if not [p for p in (d.data / "replays").glob("*_NET_HOSTED.tore-capture") if p.stat().st_size > 0]:
        d.problem("the hosting game wrote no network capture in replays/")


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="net-server-check", lane="net", args=[], driver=drive_check, uses=("server",), timeout=120,
            notes="`tore-server --check` on the guide's example mission",
        ),
        Scenario(
            name="net-server-fight", lane="net", args=[], driver=drive_fight, uses=("server", "bot"), timeout=300,
            notes="a server and two bots fly about 75 seconds; join, seat, debrief, clean exit, the server's log",
        ),
        Scenario(
            name="net-server-chat", lane="net", args=[], driver=drive_chat, uses=("server", "bot"), timeout=240,
            notes="free chat and quick chat reach the other player and the log",
        ),
        Scenario(
            name="net-server-kick", lane="net", args=[], driver=drive_kick, uses=("server", "bot"), timeout=240,
            notes="the console's players, status, kick, kick-player and end",
        ),
        Scenario(
            name="net-discovery", lane="net", args=[], driver=drive_discovery, uses=("server",), timeout=120,
            notes="`tore-app --find-games` finds a server on this machine and reports none on a free port",
        ),
        Scenario(
            name="net-window-stall", lane="net", args=[], driver=drive_stall, uses=("server",), window=True, timeout=300,
            notes="a joined game stalls its main loop for 4 seconds: the server logs the stall and the return",
        ),
        Scenario(
            name="net-window-host", lane="net", args=[], driver=drive_host, uses=("bot",), window=True, timeout=300,
            notes="a hosted game answers the search, a bot joins, and the host leaving ends the game for it",
        ),
    ]
