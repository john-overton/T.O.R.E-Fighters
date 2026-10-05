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


def scores_problems(text: str, callsigns: list[str]) -> list[str]:
    """What each bot printed of the Scores messages (slice F2-S): at least one while flying that lists every
    player with the time counting down, and the final ones, with no time left, before the mission's end."""
    problems = []
    for name in callsigns:
        lines = re.findall(rf"^{name}: scores: (players ranked by kills: .*)$", text, re.M)
        if not lines:
            problems.append(f"{name} printed no scores")
            continue
        if not any(all(re.search(rf"\d {other}\b", line) for other in callsigns) for line in lines):
            problems.append(f"no scores line of {name}'s lists every player")
        lefts = [int(m) * 60 + int(s) for m, s in (re.findall(r"; (\d+):(\d\d) left", line)[0] for line in lines
                                                   if re.search(r"; \d+:\d\d left", line))]
        if not lefts or lefts[0] == 0:
            problems.append(f"{name}'s first scores show no time left to fly")
        if lefts and lefts[-1] != 0:
            problems.append(f"{name}'s last scores are not the final ones (0:00 left)")
        ended = text.find(f"{name}: Mission ended: the time limit")
        last = text.rfind(f"{name}: scores: ")
        if ended < 0:
            problems.append(f"{name} did not hear the time limit end the mission")
        elif last > ended:
            problems.append(f"{name}'s final scores came after the end")
    return problems


def pvp_end_problems(text: str, callsigns: list[str]) -> list[str]:
    """A PvP mission with a kill limit (slice F2-1's server keys, F2-S's scoring), from what the bots printed: the
    scores name the enemy side and the limit, and the end follows the kills. A kill by any player must end the
    mission by the kill limit with a winner; with no kill (the scripted pilot rarely lands a gun kill) the time limit
    ends it in a draw, and the kill limit's own end is the simulator's test (host::score_tests)."""
    problems = []
    lines = re.findall(r"^\w+: scores: (players ranked by kills: .*)$", text, re.M)
    if not lines:
        return ["no bot printed scores"]
    if not any("(enemy)" in line for line in lines):
        problems.append("no scores line puts a player on the enemy side")
    if not any("ends at 1 kill in all" in line for line in lines):
        problems.append("no scores line names the kill limit (1 kill in all)")
    kills = {name: 0 for name in callsigns}
    for line in lines:
        for name, count in re.findall(r"\d+ (\w+)(?: \((?:friendly|enemy)\))? (\d+)/\d+", line):
            kills[name] = max(kills.get(name, 0), int(count))
    by_kill = re.search(r"^\w+: Mission ended: the kill limit", text, re.M)
    by_time = re.search(r"^\w+: Mission ended: the time limit", text, re.M)
    if any(kills.values()):
        if not by_kill:
            problems.append(f"a player scored ({kills}) but the kill limit did not end the mission")
        elif not re.search(r"; (the \w+ side|\w+) wins$", lines[-1]):
            problems.append(f"the kill limit's last scores name no winner: {lines[-1]}")
    elif not by_time:
        problems.append("nobody scored and the time limit did not end the mission")
    elif not lines[-1].endswith("; a draw"):
        problems.append(f"the time limit's last scores are not a draw: {lines[-1]}")
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
        # A bot shot down before the end leaves from the lobby, with no plane.
        d, log, r"joined as Bot1", r"joined as Bot2", r"Bot1( \(plane \d+\))? left: left",
        r"Bot2( \(plane \d+\))? left: left", r"mission ended: everyone left", forbid=NET_BAD,
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


def drive_observe(d: Drive) -> None:
    """An observer (stage F phase 2): a bot with no plane watches two bots fight, then leaves."""
    port = d.port()
    server = start_server(d, port, guide_mission(separation_nm=5))
    flyers = start_bots(d, port, "flyers", 35, "--count", "2", "--callsign", "Bot")
    if not server.wait_for(r"^mission started$", 60):
        raise DriveError("the mission never started")
    owl = start_bots(d, port, "owl", 20, "--callsign", "Owl", "--observe", "0")
    owl.finish(60, 0)
    flyers.finish(90, 0)
    server.finish(40, 0)
    owl.expect(r"^Owl: joined$", "the observer's join")
    owl.expect(r"^Owl: observing from tick \d+, 0 s behind$", "the observer flight's start")
    owl.expect(r"^Owl: watching: frames [1-9]\d*, aircraft [1-9]\d*$", "frames with aircraft in them")
    owl.expect(r"^Owl: lobby: Flying, .*Owl no slot observing", "the lobby marks the observer")
    owl.expect(r"^Owl: The connection ended: the player left\.$", "a clean leave")
    owl.forbid(r"^Owl: (seat \d+|debrief)", "a plane or a debrief for the observer")
    owl.forbid(NET_BAD, "a network problem")
    for n in (1, 2):
        flyers.expect(rf"^Bot{n}: seat \d+, plane \d+, at tick \d+$", "a seating")
    flyers.forbid(NET_BAD, "a network problem")
    server.forbid(NET_BAD, "a network problem")
    log_must(d, server_log(d), r"joined as Owl", r"Owl\b.* left: left", forbid=NET_BAD)


def drive_scores(d: Drive) -> None:
    """Scores (slice F2-S): a server with a one-minute time limit and two bots; each bot hears the scores while it
    flies and the final ones as the time limit ends the mission, and the server stops by itself."""
    port = d.port()
    server = start_server(d, port, guide_mission(separation_nm=5), time_limit=1)
    bots = start_bots(d, port, "bots", 100, "--count", "2", "--callsign", "Bot")
    if not server.wait_for(r"^mission ended: the time limit$", 150):
        d.problem("the time limit did not end the mission")
    bots.finish(60, None)
    server.finish(40, 0)
    for problem in scores_problems(bots.text(), ["Bot1", "Bot2"]):
        d.problem(problem)
    bots.forbid(NET_BAD, "a network problem")
    server.forbid(NET_BAD, "a network problem")


def drive_king(d: Drive) -> None:
    """A server whose first player wears the crown (`king first-player`, slice F2-1): the King bot changes the
    settings (PvP, friendly fire off, a one-minute time limit, three lives) and starts the mission; a second bot then
    joins and takes an enemy plane, which PvP opened. The time limit ends the mission and the server stops."""
    port = d.port()
    server = start_server(d, port, guide_mission(separation_nm=20), king="first-player")
    king = start_bots(
        d, port, "king", 100, "--callsign", "King", "--king", "mode=pvp,friendly-fire=off,time-limit=60,lives=3",
    )
    if not server.wait_for(r"King changed the settings: mode pvp", 60):
        raise DriveError("the King never changed the settings")
    wing = start_bots(d, port, "wing", 100, "--callsign", "Wing", "--slot", "6")
    if not server.wait_for(r"^mission ended: the time limit$", 150):
        d.problem("the King's time limit did not end the mission")
    king.finish(60, None)
    wing.finish(60, None)
    server.finish(40, 0)
    king.expect(r"^King: wears the crown$", "the crown")
    king.expect(
        r"^King: as the King, changing the settings: mode pvp, friendly-fire off, time-limit 1 minute, lives 3$",
        "the King's change",
    )
    king.expect(r"^King: lobby: Lobby, .*King \(King\)", "the lobby's crown")
    king.expect(r"^King: seat \d+, plane 0, at tick \d+$", "the King flies")
    for bot, name in ((king, "King"), (wing, "Wing")):
        bot.expect(rf"^{name}: settings: mode pvp, .*friendly-fire off", f"{name} sees the King's settings")
        bot.expect(rf"^{name}: Mission ended: the time limit", f"{name} hears the end")
        bot.forbid(NET_BAD, "a network problem")
    wing.expect(r"^Wing: seat \d+, plane 6, at tick \d+$", "an enemy plane, open in PvP")
    wing.forbid(r"^Wing: wears the crown$", "a second crown")
    server.forbid(NET_BAD, "a network problem")
    log_must(
        d, server_log(d), r"King wears the crown", r"King changed the settings: mode pvp, friendly-fire off",
        r"mission started", r"mission ended: the time limit", forbid=NET_BAD,
    )


def drive_pvp(d: Drive) -> None:
    """PvP from the server's file (slice F2-1's keys): `mode pvp`, a kill limit of one kill in all, two minutes at
    most. One bot flies for each side; the scores name both sides and the limit, and the end follows the kills."""
    port = d.port()
    server = start_server(
        d, port, guide_mission(separation_nm=5), mode="pvp", kill_limit=1, kill_owner="total", time_limit=2,
    )
    blue = start_bots(d, port, "blue", 170, "--callsign", "Blue", "--slot", "0")
    red = start_bots(d, port, "red", 170, "--callsign", "Red", "--slot", "6")
    if not server.wait_for(r"^mission ended: the (kill|time) limit$", 220):
        d.problem("neither the kill limit nor the time limit ended the mission")
    blue.finish(60, None)
    red.finish(60, None)
    server.finish(40, 0)
    blue.expect(r"^Blue: seat \d+, plane 0, at tick \d+$", "a friendly plane")
    red.expect(r"^Red: seat \d+, plane 6, at tick \d+$", "an enemy plane, open in PvP")
    for problem in pvp_end_problems(blue.text() + red.text(), ["Blue", "Red"]):
        d.problem(problem)
    for bot in (blue, red):
        bot.forbid(NET_BAD, "a network problem")
    server.forbid(NET_BAD, "a network problem")


def drive_delay(d: Drive) -> None:
    """A delayed observer (slice F2-O1's delay, set by slice F2-1's `observer-delay`): in a PvP mission a bot with no
    plane watches two bots fight 10 seconds behind, and the server logs that it watches."""
    port = d.port()
    server = start_server(d, port, guide_mission(separation_nm=5), mode="pvp", observer_delay=10)
    flyers = start_bots(d, port, "flyers", 50, "--count", "2", "--callsign", "Bot")
    if not server.wait_for(r"^mission started$", 60):
        raise DriveError("the mission never started")
    owl = start_bots(d, port, "owl", 30, "--callsign", "Owl", "--observe", "0")
    owl.finish(60, 0)
    flyers.finish(90, None)
    server.finish(40, 0)
    owl.expect(r"^Owl: settings: mode pvp, .*observer-delay 10 seconds", "the delay in the lobby's settings")
    owl.expect(r"^Owl: observing from tick \d+, 10 s behind$", "the observer flight, 10 seconds behind")
    owl.expect(r"^Owl: watching: frames [1-9]\d*, aircraft [1-9]\d*$", "frames with aircraft in them")
    owl.forbid(r"^Owl: (seat \d+|debrief)", "a plane or a debrief for the observer")
    owl.forbid(NET_BAD, "a network problem")
    flyers.forbid(NET_BAD, "a network problem")
    server.forbid(NET_BAD, "a network problem")
    log_must(d, server_log(d), r"Owl is watching the mission", forbid=NET_BAD)


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


def master_binary(d: Drive) -> Path:
    """`tore-master` beside the game binary (`cargo build --locked -p tore-master`)."""
    path = Path(d.app).with_name("tore-master" + (".exe" if str(d.app).endswith(".exe") else ""))
    if not path.exists():
        raise DriveError(f"{path} is not built (cargo build --locked -p tore-master)")
    return path


def master_ports(d: Drive) -> int:
    """A free main port whose next port is free too: games send the mapping test's second probe to the main
    port + 1, and the flood tool does the same."""
    for _ in range(50):
        port = d.port()
        if port < 65535 and not port_in_use(port + 1):
            return port
    raise DriveError("no free pair of UDP ports for the master")


def start_master(d: Drive, **settings) -> tuple[Proc, int]:
    """Starts a `tore-master` on 127.0.0.1 with a console and a state folder in the work folder."""
    port = master_ports(d)
    lines = {"listen": LOCALHOST, "port": port, "probe-port": port + 1, "state-dir": "state", "status-interval": 2}
    lines.update({k.replace("_", "-"): v for k, v in settings.items()})
    config = d.work / "master.conf"
    config.write_text("".join(f"{k} {v}\n" for k, v in lines.items()))
    master = d.start("master", [master_binary(d), "--config", config], stdin=True)
    if not master.wait_for(r"^Ready", 30):
        raise DriveError("the master never said it was ready")
    return master, port


def drive_master_flood(d: Drive) -> None:
    """`tore-master flood` for 10 s against a master on this machine: the limits hold, a browse from another
    address is answered all through, and the master's status lines show the drops."""
    master, port = start_master(d)
    master.expect(rf"^main port on 127\.0\.0\.1:{port}$", "the main port")
    master.expect(rf"^probe port on 127\.0\.0\.1:{port + 1}$", "the probe port")
    flood = d.run("flood", [master_binary(d), "flood", f"{LOCALHOST}:{port}", "10"], timeout=60)
    flood.expect(r"^sent \d+ datagrams \(\d+ bytes\): browse \d+, details \d+, ", "what the flood sent")
    flood.expect(r"^ports answered with more bytes than they sent: 0 ", "no port answered with more than it sent")
    m = re.search(r"^a browse from another address during the flood: answered (\d+) of (\d+)$", flood.text(), re.M)
    if not m:
        d.problem("the flood's browse check did not run (127.0.0.2 should be bindable on Linux)")
    elif int(m.group(2)) < 8 or int(m.group(1)) != int(m.group(2)):
        d.problem(f"the browse during the flood: answered {m.group(1)} of {m.group(2)}")
    flood.expect(r"^limits held$", "the verdict")
    master.send("status")
    master.send("listings")
    master.send("quit")
    master.finish(20, 0)
    master.expect(r"^status listings=0 sources=\d+ .*dropped\(limit\)=[1-9]\d* ", "a status line with the drops")
    master.expect(r"^limit source=127\.0\.0\.1 over=", "the log line for the source over a limit")
    master.expect(r"^listings=0$", "no listing made by the flood")
    master.expect(r"^Stopped$", "the stop line")
    master.forbid(r"^listed ", "a listing made by the flood")
    days = sorted((d.work / "state" / "telemetry").glob("*.tsv"))
    if not days:
        d.problem("the master wrote no state/telemetry/DATE.tsv")
    elif days[-1].read_text().splitlines()[:1] != ["installs\t0"] or "reports" in days[-1].read_text():
        d.problem(f"the flood's reports were counted: {days[-1].read_text()[:200]!r}")


def wait_count(d: Drive, proc: Proc, pattern: str, count: int, seconds: float) -> bool:
    """True once `proc` has printed `count` lines matching `pattern`; False when it exits or time runs out."""
    end = time.time() + seconds * d.scale
    while len(re.findall(pattern, proc.text(), re.M)) < count:
        if not proc.alive() or time.time() > end:
            return len(re.findall(pattern, proc.text(), re.M)) >= count
        d.sleep(0.1)
    return True


def drive_master_listing(d: Drive) -> None:
    """A `tore-server` with `broadcast on` lists itself on a `tore-master` on this machine (slice I3), judged from
    the master's own output: the listing appears from the server's game port, the console's `broadcast off` and
    `broadcast on` take it off and back, and `quit` takes it off before the server stops. Slice I4 extends this with
    `tore-app --browse`, which lists the game the way a player's Internet Lobby does (and says when the list is
    empty or the master silent), always against this scenario's own loopback master."""
    master, mport = start_master(d)
    port = d.port()
    server = start_server(d, port, broadcast="on", master=f"{LOCALHOST}:{mport}")
    listed = rf'^listed id=[0-9a-f]{{16}} from=127\.0\.0\.1:{port} name="T\.O\.R\.E server" listings=1$'
    unlisted = rf'^unlisted id=[0-9a-f]{{16}} from=127\.0\.0\.1:{port} name="T\.O\.R\.E server" reason=unregistered'
    if not wait_count(d, master, listed, 1, 10):
        d.problem("the master never listed the server")
    if not server.wait_for(r"Broadcasting: listed on the Internet Lobby", 10):
        d.problem("the server never said it was listed")
    master.send("listings")
    if not master.wait_for(r'^listing id=[0-9a-f]{16} from=127\.0\.0\.1:\d+ name="T\.O\.R\.E server" players=0/', 5):
        d.problem("the master's `listings` does not show the server")
    server.send("status")
    if not server.wait_for(rf"broadcast: listed, seen at 127\.0\.0\.1:{port}$", 5):
        d.problem("the status line does not show the listing")
    # The Internet Lobby's own listing (slice I4): `tore-app --browse` lists the server with its details.
    browsed = d.run("browse", [d.app, "--browse", "4", "--master", f"{LOCALHOST}:{mport}"], timeout=60)
    browsed.expect(
        r'^"T\.O\.R\.E server"  0/6 players, lobby, open, not full, this build, dedicated server; '
        r'mission "UKR, [^"]*"; king -; players -$',
        "the server's line in the Internet Lobby",
    )
    browsed.expect(r"^1 game listed\.$", "the count")
    server.send("broadcast off")
    if not wait_count(d, master, unlisted, 1, 5):
        d.problem("`broadcast off` did not take the server off the master's list")
    empty = d.run("browse-empty", [d.app, "--browse", "2", "--master", f"{LOCALHOST}:{mport}"], timeout=60)
    empty.expect(r"^No games listed\.$", "the empty list once the server is off it")
    silent = d.run("browse-silent", [d.app, "--browse", "4", "--master", f"{LOCALHOST}:{d.port()}"], timeout=60, expect_exit=1)
    silent.expect(r"does not answer", "the silent master's verdict")
    server.send("broadcast on")
    if not wait_count(d, master, listed, 2, 10):
        d.problem("`broadcast on` did not list the server again")
    stop_server(d, server)
    if not wait_count(d, master, unlisted, 2, 5):
        d.problem("quitting the server did not take it off the master's list")
    master.send("listings")
    master.send("quit")
    master.finish(20, 0)
    master.expect(r"^listings=0$", "an empty list once the server quit")
    master.forbid(r"reason=expired", "a listing left to expire")
    server.expect(rf"^Broadcast: on, to the Internet Lobby at 127\.0\.0\.1:{mport}; anonymous statistics on", "the start line")
    server.expect(r"^console: broadcast off: taking the server off", "the console's broadcast off")
    server.expect(r"^Broadcasting stopped: off the Internet Lobby$", "the unlisting")
    server.expect(r"^console: broadcast on: listing the server", "the console's broadcast on")
    server.forbid(NET_BAD, "a network problem")
    server.forbid(r"does not answer|cannot find the Internet Lobby", "a silent master")
    log = server_log(d)
    log_must(d, log, r"Broadcasting: listed on the Internet Lobby", r"console: quit", forbid=NET_BAD)


def drive_master_introduce(d: Drive) -> None:
    """A `tore-bot --master --listing` joins a `tore-server` listed on a `tore-master` on this machine through an
    introduction (slice J2): the bot finds the game on the master's list, runs the mapping test, is introduced,
    races the host's addresses while the server punches back, joins along the punched path, flies 30 seconds and
    leaves; the server stops by itself and the master's status line counted the introduction."""
    master, mport = start_master(d)
    port = d.port()
    server = start_server(d, port, broadcast="on", master=f"{LOCALHOST}:{mport}")
    if not server.wait_for(r"Broadcasting: listed on the Internet Lobby", 10):
        d.problem("the server never said it was listed")
    bot = d.start("bot", [d.bot, "--master", f"{LOCALHOST}:{mport}", "--listing", "T.O.R.E server", "--seconds", "30"])
    bot.finish(120, 0)
    server.finish(40, 0)
    master.send("status")
    master.send("quit")
    master.finish(20, 0)
    bot.expect(rf'^found "T\.O\.R\.E server" on the Internet Lobby at 127\.0\.0\.1:{mport}$', "the listing found")
    bot.expect(r"^Bot: asking the Internet Lobby for an introduction\.\.\.$", "the introduction asked for")
    bot.expect(r"^Bot: mapping test: (NoTranslation|SamePort)$", "the mapping test")
    bot.expect(r"^Bot: introduced; trying [1-8] address(es)?\.\.\.$", "the introduction")
    bot.expect(r"^Bot: joined through the Internet Lobby, path punched$", "the join along the punched path")
    bot.expect(r"^Bot: seat \d+, plane \d+, at tick \d+$", "a seating")
    bot.expect(r"^Bot: debrief: (success|failure), \d+ kills, \d+ seconds$", "a debrief")
    bot.expect(r"^Bot: The connection ended: the player left\.$", "a clean leave")
    bot.forbid(NET_BAD, "a network problem")
    bot.forbid(r"no direct path|only the relay", "a race that found no direct path")
    server.expect(r"^mission ended: everyone left$", "the end")
    server.forbid(NET_BAD, "a network problem")
    master.expect(r"^status listings=\d+ sources=\d+ browse/s=[\d.]+ introductions/min=[1-9]\d* ", "an introduction counted")
    master.expect(r"^Stopped$", "the stop line")
    log_must(d, server_log(d), r"joined as Bot", r"Bot( \(plane \d+\))? left: left", forbid=NET_BAD)


# The Internet Lobby driven by a script (a pointer needs a moment over a target before a click lands): the Multi
# menu's second row, the list, a game selected, New (the lobby opens as King; the driver browses meanwhile), Escape
# (leave), then Join on the listed server, a slot taken and Ready, and a few seconds of flight. Menu coordinates are
# the 640 by 480 layer's.
INTERNET_SCRIPT = """wait 10
movemenu 150 48
wait 0.6
click
wait 0.6
movemenu 150 90
wait 0.6
click
wait 5
snapshot SHOTS/internet-list.ppm
movemenu 100 194
wait 0.6
click
wait 2
snapshot SHOTS/internet-selected.ppm
movemenu 80 432
wait 0.6
click
wait 30
snapshot SHOTS/internet-lobby.ppm
key Escape
wait 1
snapshot SHOTS/internet-leave.ppm
key Tab
wait 0.5
key Enter
wait 6
movemenu 100 194
wait 0.6
click
wait 2
movemenu 200 432
wait 0.6
click
wait 12
snapshot SHOTS/internet-joined.ppm
movemenu 150 176
wait 0.6
click
click
wait 2
movemenu 460 434
wait 0.6
click
wait 3
snapshot SHOTS/internet-ready.ppm
waittick 720 40
shot SHOTS/internet-flight.ppm
exit
"""


def drive_internet(d: Drive) -> None:
    """The game's Internet Lobby screen against a master on this machine (slice I4): it lists a `tore-server` that
    is listed there and a game of its own that New lists (a second `tore-app --browse` sees it); then Join on the
    server runs the master's introduction and the race (slice J2), takes a plane, readies and flies a few seconds."""
    master, mport = start_master(d)
    port = d.port()
    server = start_server(d, port, broadcast="on", master=f"{LOCALHOST}:{mport}")
    if not wait_count(d, master, rf'^listed id=[0-9a-f]{{16}} from=127\.0\.0\.1:{port} name="T\.O\.R\.E server"', 1, 10):
        d.problem("the master never listed the server")
    # The player's own settings: a callsign, this master, the notice not yet shown.
    (d.data / "network-v1.conf").write_text(
        f"tore-network 1\ncallsign Viper\nport {d.port()}\nmaster {LOCALHOST}:{mport}\n"
    )
    shots = d.work / "shots"
    shots.mkdir(exist_ok=True)
    hosted = rf'^listed id=[0-9a-f]{{16}} from=127\.0\.0\.1:\d+ name="Viper\'s game"'
    opened = r"Internet Lobby: Asking the Internet Lobby at"
    # A scripted click on a window that is slow to come up (the machine is shared with a person) can land before
    # the menu answers; one more try, starting later, tells that from a real failure.
    for attempt, start_wait in enumerate((10, 25), start=1):
        for old in (d.data / "logs").glob("tore-*.log"):
            old.unlink()
        script = d.work / f"internet{attempt}.txt"
        script.write_text(INTERNET_SCRIPT.replace("SHOTS", str(shots)).replace("wait 10\n", f"wait {start_wait}\n", 1))
        game = d.start(f"game{attempt}", [d.app, *GAME_FLAGS, "--input-script", script], window=True)
        if wait_count(d, master, hosted, 1, 90 + start_wait):
            seen = d.run("browse", [d.app, "--browse", "4", "--master", f"{LOCALHOST}:{mport}"], timeout=60)
            seen.expect(r'^"Viper\'s game"  \d/\d players, lobby, open, not full, this build', "the hosted game in a second browse")
            seen.expect(r'^"T\.O\.R\.E server"  0/6 players, ', "the server in the same browse")
        game.finish(200, 0)
        log = "\n".join(p.read_text(errors="replace") for p in sorted((d.data / "logs").glob("tore-*.log")))
        if opened in log:
            break
        d.log(f"attempt {attempt}: the scripted clicks did not open the screen")
    # The screen's own lines are in the game's log (its text is drawn over the picture, not in the snapshots).
    for pattern, what in (
        (r"Internet Lobby: This game sends anonymous statistics to the Internet Lobby\. Turn them off in Options\.", "the one-time notice"),
        (rf"Internet Lobby: Asking the Internet Lobby at {re.escape(LOCALHOST)}:{mport} for games\.\.\.", "the browse starting"),
        (r"Internet Lobby: 1 game is listed on the Internet Lobby\.", "the count"),
        (r"Internet Lobby: Asking the Internet Lobby to introduce you to 'T\.O\.R\.E server'\.\.\.", "Join asking"),
        (r"Internet Lobby: Trying 1 address for 'T\.O\.R\.E server'\.\.\.", "the introduction"),
        (r"Network: Connected directly \(punched through\)\.", "the path the join took"),
        (r"Network: seated in plane 0", "a seating after Join, a slot and Ready"),
        (r"Network: lobby: Flying, .* Viper plane 0", "the mission flying"),
        (r"Internet Lobby: Hosting Viper's game on UDP port \d+\.\.\. Listing it on the Internet Lobby\.", "New listing the game"),
        (r"Host: Listed on the Internet Lobby, seen at 127\.0\.0\.1:\d+\.", "the host's listing"),
    ):
        if not re.search(pattern, log):
            d.problem(f"the game's log lacks {what}: /{pattern}/")
    if re.search(r"master\.jroverton\.com|master\.invalid", log):
        d.problem("the game talked about a master other than the scenario's own")
    for name in ("internet-list", "internet-selected", "internet-lobby", "internet-leave", "internet-joined", "internet-ready", "internet-flight"):
        if not (shots / f"{name}.ppm").exists():
            d.problem(f"the script's {name}.ppm was not written")
    game.forbid(NET_BAD, "a network problem")
    stop_server(d, server)
    master.send("status")
    master.send("quit")
    master.finish(20, 0)
    # The server saw the join through the master: seated, ready, flying, and the game's exit.
    log_must(d, server_log(d), r"joined as Viper", r"Viper took the slot of plane 0", r"seat 0 Viper took plane 0", forbid=NET_BAD)
    master.expect(r"^status listings=\d+ sources=\d+ browse/s=[\d.]+ introductions/min=[1-9]\d* ", "the introduction counted")


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
            name="net-server-observe", lane="net", args=[], driver=drive_observe, uses=("server", "bot"), timeout=240,
            notes="a bot with no plane watches two bots fight (stage F phase 2's observer stream) and leaves",
        ),
        Scenario(
            name="net-server-scores", lane="net", args=[], driver=drive_scores, uses=("server", "bot"), timeout=300,
            notes="a one-minute time limit with two bots: the scores while flying and the final ones at the end",
        ),
        Scenario(
            name="net-server-king", lane="net", args=[], driver=drive_king, uses=("server", "bot"), timeout=300,
            notes="`king first-player`: the King bot changes the settings to PvP and starts; a bot takes an enemy plane",
        ),
        Scenario(
            name="net-server-pvp", lane="net", args=[], driver=drive_pvp, uses=("server", "bot"), timeout=360,
            notes="PvP from the server's file with a kill limit: a bot on each side, the scores, the end by the kills",
        ),
        Scenario(
            name="net-server-delay", lane="net", args=[], driver=drive_delay, uses=("server", "bot"), timeout=240,
            notes="`observer-delay 10` in PvP: an observer bot watches two bots fight 10 seconds behind",
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
        Scenario(
            name="net-master-flood", lane="net", args=[], driver=drive_master_flood, timeout=120,
            notes="tore-master and its flood tool for 10 s: the limits hold and a browse during the flood is answered",
        ),
        Scenario(
            name="net-window-internet", lane="net", args=[], driver=drive_internet, uses=("server",), window=True, timeout=480,
            notes="the Internet Lobby screen against a master and a listed server on this machine: the list, a game "
            "selected, New (a second `--browse` sees the game), then Join through the master, a plane and a few seconds of flight",
        ),
        Scenario(
            name="net-master-listing", lane="net", args=[], driver=drive_master_listing, uses=("server",), timeout=120,
            notes="a tore-server with `broadcast on` lists itself on a tore-master on this machine; `tore-app --browse` "
            "lists it; `broadcast off`, `broadcast on` and `quit` take it off and back",
        ),
        Scenario(
            name="net-master-introduce", lane="net", args=[], driver=drive_master_introduce, uses=("server", "bot"),
            timeout=180,
            notes="tore-bot --master --listing joins a listed tore-server through an introduction from a tore-master "
            "on this machine, along the punched path, and flies 30 seconds (slice J2)",
        ),
    ]
