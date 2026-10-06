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


def weapons_hold(mission: str) -> str:
    """The mission with every AI wing on weapons hold and the objective lines gone, so that nothing but the players
    shoots: a PvP kill limit is then the players' to reach, not a race with the AI's missiles (which kill a bot
    on the AI's side about 15 seconds in, before the players' pass)."""
    mission = mission.replace("preset free", "preset hold")
    return "".join(line for line in mission.splitlines(keepends=True) if not line.startswith("objective "))


def dummy_enemies(mission: str) -> str:
    """The mission with every enemy wing's skill set to `dummy`: straight, level aircraft at 400 knots that do not
    evade or fire."""
    return re.sub(r"(?m)^(wing enemy \d+ \S+ \d+) \w+$", r"\1 dummy", mission)


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
    # Stage L (slice L3): the import's source, its counts and one line per item.
    if not re.search(r"^Content: .*Fighters Anthology.*, imported by .*T\.O\.R\.E", text, re.M):
        problems.append("the content's source line is missing")
    if not re.search(r"^Content items: \d+ aircraft, \d+ theaters?, \d+ weapons?, the shared data$", text, re.M):
        problems.append("the content's counts line is missing or has no shared data")
    items = content_item_lines(text)
    for kind in ("aircraft", "theater", "weapon", "shared"):
        if not any(line.startswith(kind) for line in items):
            problems.append(f"the content lists no {kind} item")
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


def results_problems(text: str, callsigns: list[str]) -> list[str]:
    """What each bot printed of the Results message (slice F2-D): exactly one line, before the mission's end, naming
    every aircraft of the mission with the players' callsigns on their planes and the AI on the rest, and, in PvP,
    the winner of the final scores."""
    problems = []
    for name in callsigns:
        lines = re.findall(rf"^{name}: results: (.*)$", text, re.M)
        if len(lines) != 1:
            problems.append(f"{name} printed {len(lines)} results lines, not one")
            continue
        line = lines[0]
        head = re.match(r"(\d+) aircraft, (\d+) flown by players: ", line)
        if not head:
            problems.append(f"{name}'s results line has no aircraft count: {line[:80]}")
            continue
        if int(head.group(2)) != len(callsigns):
            problems.append(f"{name}'s results count {head.group(2)} players, not {len(callsigns)}")
        if int(head.group(1)) <= len(callsigns):
            problems.append(f"{name}'s results list only {head.group(1)} aircraft, none of them the AI's")
        if not re.search(r"\d+ AI (alive|dead|ejected|retired) \d+k", line):
            problems.append(f"{name}'s results list no AI aircraft")
        for other in callsigns:
            if not re.search(rf"\d+ {other} (alive|dead|ejected|retired) \d+k", line):
                problems.append(f"{name}'s results list no row for {other}")
        if not re.search(r"; (a draw|the \w+ side wins|\w+ wins)$", line):
            problems.append(f"{name}'s results name no winner of the final scores")
        ended = text.find(f"{name}: Mission ended")
        if ended >= 0 and text.find(f"{name}: results: ") > ended:
            problems.append(f"{name}'s results came after the mission's end")
    return problems


def pvp_end_problems(text: str, callsigns: list[str]) -> list[str]:
    """A PvP mission with a kill limit (slice F2-1's server keys, F2-S's scoring), from what the bots printed: the
    scores name the enemy side and the limit, a kill ends the mission by the kill limit, and the last scores name
    the winner. The scripted pilot lands gun kills (slice BOT), so the time limit's draw is a failure here: the
    kill limit's end is covered end to end, not only by the simulator's `host::score_tests`. Two bots that shoot each
    other down on one pass both have kills and finish level, a draw."""
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
    if not re.search(r"^\w+: Mission ended: the kill limit", text, re.M):
        if re.search(r"^\w+: Mission ended: the time limit", text, re.M):
            problems.append("nobody shot anyone down: the time limit ended the mission, not the kill limit")
        else:
            problems.append("the kill limit did not end the mission")
    if not any(kills.values()):
        problems.append("no player scored a kill")
    last = lines[-1]
    if "; a draw" in last:
        if sum(1 for count in kills.values() if count) < 2:
            problems.append(f"the last scores are a draw with fewer than two players who scored: {last}")
    elif not re.search(r"; (the \w+ side|\w+) wins$", last):
        problems.append(f"the kill limit's last scores name no winner: {last}")
    return problems


def hunt_problems(text: str, name: str) -> list[str]:
    """A lone bot against enemies that do not evade (slice BOT), from what it printed: the kill limit ended the mission,
    the bot's own side won, and the bot's tally holds the kill that ended it."""
    problems = []
    lines = re.findall(rf"^{name}: scores: (players ranked by kills: .*)$", text, re.M)
    if not lines:
        return [f"{name} printed no scores"]
    if not re.search(rf"^{name}: Mission ended: the kill limit", text, re.M):
        problems.append("the kill limit did not end the mission: the bot shot nothing down in time")
    last = lines[-1]
    tally = re.search(rf"\d+ {name} \((?:friendly|enemy)\) (\d+)/(\d+)", last)
    if not tally or int(tally.group(1)) < 1:
        problems.append(f"the bot's tally has no kill: {last}")
    if not last.endswith("; the friendly side wins"):
        problems.append(f"the last scores do not give the bot's side the win: {last}")
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
        d, log, r"joined as Bot1 \(path: local network\)", r"joined as Bot2 \(path: local network\)", r"Bot1( \(plane \d+\))? left: left",
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


def drive_results(d: Drive) -> None:
    """The results (slice F2-D): a PvP server with a one-minute time limit and a bot on each side; at the end each
    bot hears one Results message, before the mission's end, with a row for every aircraft (the players' callsigns on
    their planes, the AI on the rest) and the winner of the final scores."""
    port = d.port()
    server = start_server(d, port, guide_mission(separation_nm=5), mode="pvp", time_limit=1)
    blue = start_bots(d, port, "blue", 100, "--callsign", "Blue", "--slot", "0")
    red = start_bots(d, port, "red", 100, "--callsign", "Red", "--slot", "6")
    if not server.wait_for(r"^mission ended: the time limit$", 150):
        d.problem("the time limit did not end the mission")
    blue.finish(60, None)
    red.finish(60, None)
    server.finish(40, 0)
    for problem in results_problems(blue.text() + red.text(), ["Blue", "Red"]):
        d.problem(problem)
    for bot in (blue, red):
        bot.forbid(NET_BAD, "a network problem")
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


def rate_problems(text: str, name: str, rate: int, setting_seen: bool = True) -> list[str]:
    """What a bot printed of the snapshot rate (slice R1): the lobby's settings carrying `rate` when the setting is not
    the default, and the rate of each flight as it was seated."""
    problems = []
    rates = re.findall(rf"^{name}: snapshots: (\d+) a second$", text, re.M)
    if not rates:
        problems.append(f"{name} printed no snapshot rate for its flight")
    elif set(rates) != {str(rate)}:
        problems.append(f"{name}'s flight ran at {sorted(set(rates))} snapshots a second, expected {rate}")
    if setting_seen and not re.search(rf"^{name}: settings: .*snapshot-rate {rate} a second", text, re.M):
        problems.append(f"{name}'s lobby never showed snapshot-rate {rate} a second")
    return problems


def drive_rate_server(d: Drive) -> None:
    """A dedicated server whose file sets `snapshot-rate 30` (slice R1): the lobby state carries it, a bot joins, and
    its flight runs at 30 snapshots a second, though a server's own rate is its operator's (the King could not turn
    it)."""
    port = d.port()
    server = start_server(d, port, guide_mission(separation_nm=5), snapshot_rate=30)
    bot = start_bots(d, port, "bot", 20, "--callsign", "Bot")
    bot.finish(90, 0)
    server.finish(40, 0)
    for problem in rate_problems(bot.text(), "Bot", 30):
        d.problem(problem)
    bot.expect(r"^Bot: seat \d+, plane \d+, at tick \d+$", "a seating")
    bot.forbid(NET_BAD, "a network problem")
    server.forbid(NET_BAD, "a network problem")


def drive_rate_host(d: Drive) -> None:
    """A game a player hosts (a hosting bot, the house and the King, slice R1): the King turns the snapshot rate to 30 in
    the lobby, a second bot joins, and both flights run at 30 snapshots a second. (Whether the joiner's Accepted packet
    already said 30 depends on which came first; the stale Accepted is `host::king_tests`' to check on the simulator.)"""
    port = d.port()
    mission = write_mission(d)
    host = d.start(
        "host",
        [d.bot, "--host", mission, "--port", port, "--callsign", "Lead", "--slot", "0", "--seconds", 40, "--players", "2",
         "--standby", "off", "--king", "snapshot-rate=30"],
    )
    if not host.wait_for(r"^Lead: hosting ", 60):
        raise DriveError("the hosting bot never began to host")
    pilot = d.start(
        "pilot", [d.bot, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Pilot", "--slot", "1", "--seconds", 25],
    )
    if not host.wait_for(r"^Lead: host: mission started", 90):
        raise DriveError("the hosting bot never started the mission")
    pilot.finish(120, 0)
    host.finish(120, 0)
    host.expect(r"^Lead: as the King, changing the settings: snapshot-rate 30 a second$", "the King's change")
    for problem in rate_problems(host.text(), "Lead", 30) + rate_problems(pilot.text(), "Pilot", 30):
        d.problem(problem)
    pilot.expect(r"^Pilot: seat \d+, plane 1, at tick \d+$", "the joiner's seat")
    host.forbid(NET_BAD, "a network problem")
    pilot.forbid(NET_BAD, "a network problem")


def drive_pvp(d: Drive) -> None:
    """PvP from the server's file (slice F2-1's keys): `mode pvp`, a kill limit of one kill in all, four minutes at
    most (the first pass kills in about 20 seconds; the bots merge again every 20 seconds or so, which leaves a
    busy machine chances to spare). One bot flies for each side and shoots at the other (slice BOT's pursuit and gun aiming land a kill in
    about 20 seconds on the guide's mission 5 nm apart, its AI on weapons hold so that the AI's missiles do not kill a
    bot first); the scores name both sides and the limit, and the kill limit ends the mission."""
    port = d.port()
    server = start_server(
        d, port, weapons_hold(guide_mission(separation_nm=5)), mode="pvp", kill_limit=1, kill_owner="total",
        time_limit=4,
    )
    blue = start_bots(d, port, "blue", 250, "--callsign", "Blue", "--slot", "0")
    red = start_bots(d, port, "red", 250, "--callsign", "Red", "--slot", "6")
    if not server.wait_for(r"^mission ended: the kill limit$", 250):
        d.problem("the kill limit did not end the mission (the time limit is four minutes)")
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


def drive_hunt(d: Drive) -> None:
    """A bot shoots down a target that does not evade (slice BOT, the pursuit and gun aiming the net lane needs to
    end a PvP mission by its kill limit): the guide's mission 5 nm apart with every enemy a dummy (straight, level,
    400 knots) and every AI wing on weapons hold, so that the kill is the bot's own. The bot flies for the friendly
    side; its first kill ends the mission by the kill limit within about 20 seconds."""
    port = d.port()
    mission = weapons_hold(dummy_enemies(guide_mission(separation_nm=5)))
    server = start_server(d, port, mission, mode="pvp", kill_limit=1, kill_owner="total", time_limit=2)
    bot = start_bots(d, port, "hunter", 100, "--callsign", "Hunter", "--slot", "0")
    if not server.wait_for(r"^mission ended: the kill limit$", 60):
        d.problem("the kill limit did not end the mission within a minute")
    bot.finish(60, None)
    server.finish(40, 0)
    bot.expect(r"^Hunter: seat \d+, plane 0, at tick \d+$", "a seating")
    for problem in hunt_problems(bot.text(), "Hunter"):
        d.problem(problem)
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


def revive_problems(text: str, name: str, new_plane: int) -> list[str]:
    """What a `tore-bot --revive` printed (slice F2-V): it ejected, heard it may fly again, was told of its new
    plane before it was seated in it, and was seated twice, the second time in that plane."""
    problems = []
    lines = text.splitlines()

    def first(pattern: str) -> int | None:
        return next((i for i, line in enumerate(lines) if re.search(pattern, line)), None)

    ejected = first(rf"^{name}: ejected$")
    revival = first(rf"^{name}: revival: Press Enter to fly again")
    spawned = first(rf"^{name}: spawned plane {new_plane} in Friendly wing \d, member \d+$")
    seatings = [i for i, line in enumerate(lines) if re.search(rf"^{name}: seat \d+, plane \d+, at tick \d+$", line)]
    reseated = first(rf"^{name}: seat \d+, plane {new_plane}, at tick \d+$")
    if ejected is None:
        problems.append(f"{name} never ejected")
    if revival is None:
        problems.append(f"{name} never heard it may fly again")
    if spawned is None:
        problems.append(f"{name} was not told of plane {new_plane}")
    if len(seatings) < 2 or reseated is None:
        problems.append(f"{name} was not seated again in plane {new_plane}")
    order = [ejected, revival, spawned, reseated]
    if None not in order and order != sorted(order):
        problems.append(f"{name}'s ejection, revival, new plane and seating came out of order")
    return problems


def drive_revive(d: Drive) -> None:
    """Revival (slice F2-V): a server whose King's `respawn` is retail's revival; a bot ejects 8 seconds into its
    flight, flies again in a new plane of its wing, flies on and leaves cleanly. The server's file sets the respawn rule
    by its registry name (slice F2-1)."""
    port = d.port()
    server = start_server(d, port, guide_mission(), respawn="revive")
    bot = start_bots(d, port, "bot", 40, "--callsign", "Phoenix", "--revive", "8")
    bot.finish(90, 0)
    server.finish(40, 0)
    # The guide's mission has twelve aircraft: the revival's is plane 12.
    for problem in revive_problems(bot.text(), "Phoenix", 12):
        d.problem(problem)
    bot.expect(r"^Phoenix: debrief: ", "a debrief")
    bot.expect(r"^Phoenix: The connection ended: the player left\.$", "a clean leave")
    bot.forbid(NET_BAD, "a network problem")
    server.forbid(NET_BAD, "a network problem")
    log_must(d, server_log(d), r"Phoenix took plane 0\b", r"Phoenix took plane 12\b", forbid=NET_BAD)


def drive_replies(d: Drive) -> None:
    """Orders to human wingmen and their replies (slice F2-R): two bots in the first friendly wing of the guide's
    mission, the AI on weapons hold. The lead bot (plane 0) orders "break left" and, later, presses a reply key, which
    a lead has no one to answer; the wingman bot (plane 1) replies "Winchester". The wingman hears the lead's order as a
    radio call, the lead hears the reply under the wingman's place, the wingman hears its own as itself, and the
    lead's own reply is refused with its line and heard by no one."""
    port = d.port()
    server = start_server(d, port, weapons_hold(guide_mission()))
    lead = start_bots(
        d, port, "lead", 45, "--callsign", "Lead", "--slot", "0", "--order", "14,break-left", "--reply", "26,engaging",
    )
    if not lead.wait_for(r"^Lead: seat \d+, plane 0, at tick \d+$", 90):
        raise DriveError("the lead bot was never seated in plane 0")
    wing = start_bots(d, port, "wing", 40, "--callsign", "Wing", "--slot", "1", "--reply", "12,winchester")
    lead.finish(120, 0)
    wing.finish(120, 0)
    server.finish(40, 0)
    wing.expect(r"^Wing: seat \d+, plane 1, at tick \d+$", "the wingman flies plane 1")
    wing.expect(r"^Wing: radio: Red one: '.+'$", "the lead's order as a radio call")
    lead.expect(r"^Lead: radio: Red two: 'Winchester'$", "the wingman's reply, under its place")
    wing.expect(r"^Wing: radio: YOU: 'Winchester'$", "the wingman's own reply, as itself")
    lead.expect(r"^Lead: line: You lead this flight\.$", "the lead's refusal")
    lead.forbid(r"^Lead: radio: YOU: '.*[Ee]ngag", "a call from a lead's reply")
    wing.forbid(r"^Wing: radio: Red one: '.*[Ee]ngag", "a call from a lead's reply")
    for bot in (lead, wing):
        bot.forbid(NET_BAD, "a network problem")
    server.forbid(NET_BAD, "a network problem")


def drive_datalink(d: Drive) -> None:
    """The flight data link on the wire (slice G7, protocol 15): two bots in the first friendly wing of the guide's
    mission, the AI on weapons hold. The lead bot (plane 0) sorts its wing (Alt+A); the wingman bot (plane 1), a human,
    is given a bandit by data link. Both bots hear the assignment as a Link event about their flight, the wingman's
    readout carries it, and the wingman hears the lead's assignment call."""
    port = d.port()
    server = start_server(d, port, weapons_hold(guide_mission()))
    lead = start_bots(d, port, "lead", 45, "--callsign", "Lead", "--slot", "0", "--order", "20,sort")
    if not lead.wait_for(r"^Lead: seat \d+, plane 0, at tick \d+$", 90):
        raise DriveError("the lead bot was never seated in plane 0")
    wing = start_bots(d, port, "wing", 40, "--callsign", "Wing", "--slot", "1")
    lead.finish(120, 0)
    wing.finish(120, 0)
    server.finish(40, 0)
    wing.expect(r"^Wing: seat \d+, plane 1, at tick \d+$", "the wingman flies plane 1")
    lead.expect(r"^Lead: line: Sort: \d+ assigned", "the lead's sort was given")
    given = r"link: plane 0 assigned plane 1 bandit (\d+) \(Sort\)$"
    wing.expect(r"^Wing: " + given, "the wingman's assignment as a Link event")
    lead.expect(r"^Lead: " + given, "the assignment as a Link event about the lead's flight")
    m = re.search(r"(?m)^Wing: " + given, wing.text())
    if m:
        wing.expect(
            rf"^Wing: link: assigned: bandit {m.group(1)} by plane 0\b", "the wingman's readout holds the assignment"
        )
    wing.expect(r"^Wing: radio: Red one: '.*[Aa]ttack bandit", "the lead's assignment call")
    for bot in (lead, wing):
        bot.forbid(r"^\w+: link: plane ([4-9]|\d\d+) ", "a Link event about another flight")
        bot.forbid(NET_BAD, "a network problem")
    server.forbid(NET_BAD, "a network problem")


def drive_datalink_lead(d: Drive) -> None:
    """An AI lead's data link work reaching a human wingman over the wire (slices G4, G7 and G11): one bot flies plane
    1 of the guide's mission, weapons free, so the AI leads plane 0 and flies planes 2 and 3. Once the AI lead commits
    it shares or sorts, and the bot, its number Two, is dealt a bandit like its AI wingmen: it hears the assignment as
    a Link event about its flight, its readout holds it, and it hears the lead's call to Two. An AI wingman the lead
    dealt too shows as a flightmate's mark in the bot's readout."""
    port = d.port()
    server = start_server(d, port, guide_mission())
    wing = start_bots(d, port, "wing", 45, "--callsign", "Wing", "--slot", "1")
    wing.finish(120, 0)
    server.finish(40, 0)
    wing.expect(r"^Wing: seat \d+, plane 1, at tick \d+$", "the bot flies plane 1")
    own = r"^Wing: link: plane 0 assigned plane 1 bandit (\d+) \((Sort|EngageMyTarget)\)$"
    m = re.search(own, wing.text(), re.M)
    if not m:
        d.problem(f"wing: missing the AI lead's assignment to the human as a Link event /{own}/")
    else:
        wing.expect(
            rf"^Wing: link: assigned: bandit {m.group(1)} by plane 0\b", "the readout holds the human's assignment"
        )
        wing.expect(r"^Wing: radio: Red one: 'Two, attack bandit", "the AI lead's call to Two")
    flightmate = r"^Wing: link: plane 0 assigned plane ([23]) bandit (\d+) \((Sort|EngageMyTarget)\)$"
    m = re.search(flightmate, wing.text(), re.M)
    if m:
        member = int(m.group(1)) + 1
        wing.expect(
            rf"^Wing: link: marked: bandit {m.group(2)} assigned to (\d+, )*{member}\b",
            "the readout's mark for the flightmate's bandit",
        )
    wing.forbid(r"^Wing: link: plane ([4-9]|\d\d+) ", "a Link event about another flight")
    wing.forbid(NET_BAD, "a network problem")
    server.forbid(NET_BAD, "a network problem")


def away_problems(text: str, name: str, plane: int) -> list[str]:
    """What a `tore-bot --away` printed (slice F2-A): seated in `plane`, the AI took it and kept it while the bot
    watched it, the bot asked for it back and was seated in it again, in that order."""
    problems = []
    lines = text.splitlines()

    def first(pattern: str, after: int = -1) -> int | None:
        return next((i for i, line in enumerate(lines) if i > after and re.search(pattern, line)), None)

    seat = rf"^{name}: seat \d+, plane {plane}, at tick \d+$"
    seated = first(seat)
    away = first(rf"^{name}: away: the AI flies plane {plane}$")
    watching = first(rf"^{name}: observing from tick \d+, 0 s behind$")
    back = first(rf"^{name}: back at the controls$")
    reseated = first(seat, back) if back is not None else None
    if seated is None:
        problems.append(f"{name} was never seated in plane {plane}")
    if away is None:
        problems.append(f"the AI never flew {name}'s plane {plane}")
    if watching is None:
        problems.append(f"{name} never watched its plane while away")
    if back is None:
        problems.append(f"{name} never asked for its plane back")
    if reseated is None:
        problems.append(f"{name} was not seated again in plane {plane}")
    order = [seated, away, back, reseated]
    if None not in order and order != sorted(order):
        problems.append(f"{name}'s seating, away, back and seating again came out of order")
    return problems


def drive_away(d: Drive) -> None:
    """The AI flies an idle player's aircraft (slice F2-A): a bot's game says it is away 8 seconds into its flight;
    the AI flies its plane, kept for it, while it watches; 6 seconds later it says it is back, flies on in the same
    plane and leaves cleanly. The server's file leaves `idle-ai` at its default, 5 minutes (the bot says it is away itself, so the setting only has to allow it)."""
    port = d.port()
    server = start_server(d, port, guide_mission())
    bot = start_bots(d, port, "bot", 40, "--callsign", "Viper", "--slot", "0", "--away", "8,6")
    bot.finish(90, 0)
    server.finish(40, 0)
    for problem in away_problems(bot.text(), "Viper", 0):
        d.problem(problem)
    bot.expect(r"^Viper: debrief: ", "a debrief")
    bot.expect(r"^Viper: The connection ended: the player left\.$", "a clean leave")
    bot.forbid(NET_BAD, "a network problem")
    server.forbid(NET_BAD, "a network problem")
    log_must(
        d,
        server_log(d),
        r"Viper is away: the AI flies plane 0$",
        r"Viper is back: takes plane 0 from the AI$",
        r"Viper took plane 0\b",
        forbid=NET_BAD,
    )


def rejoin_problems(text: str, name: str, plane: int) -> list[str]:
    """What a `tore-bot --token-file` printed when it was started again after it was killed in flight (slice K5): it
    sent its token, the host welcomed it back with its aircraft waiting, and it was seated in `plane` again, in
    that order."""
    problems = []
    lines = text.splitlines()

    def first(pattern: str, after: int = -1) -> int | None:
        return next((i for i, line in enumerate(lines) if i > after and re.search(pattern, line)), None)

    sent = first(rf"^{name}: rejoining with its token \(session [0-9a-f]{{16}}\)$")
    welcome = first(rf"^{name}: Welcome back, {name}: your aircraft is waiting\.$")
    seated = first(rf"^{name}: seat \d+, plane {plane}, at tick \d+$")
    if sent is None:
        problems.append(f"{name} never sent its token")
    if welcome is None:
        problems.append(f"{name} was not welcomed back with its aircraft waiting")
    if seated is None:
        problems.append(f"{name} was not seated in plane {plane} again")
    order = [sent, welcome, seated]
    if None not in order and order != sorted(order):
        problems.append(f"{name}'s token, welcome and seating came out of order")
    return problems


def drive_rejoin(d: Drive) -> None:
    """Rejoin (slice K5): a bot flies plane 0 and keeps its token in a file; it is killed in flight (SIGKILL); the
    server drops it after 5 seconds and keeps plane 0 for it (the AI flies it, nobody else may take it); the bot
    started again with the same file sends its token and is back in plane 0, welcomed back, and leaves cleanly. A
    second bot keeps the mission going meanwhile."""
    port = d.port()
    server = start_server(d, port, guide_mission(), empty_timeout=90)
    token_file = d.work / "viper.token"
    stay = start_bots(d, port, "stay", 75, "--callsign", "Stay", "--slot", "1")
    first = start_bots(
        d, port, "viper", 60, "--callsign", "Viper", "--slot", "0", "--token-file", str(token_file)
    )
    if not server.wait_for(r"seat \d+ Viper took plane 0", 90):
        raise DriveError("Viper never took plane 0")
    if not server.wait_for(r"seat \d+ Stay took plane 1", 30):
        raise DriveError("Stay never took plane 1")
    d.sleep(4)
    if not token_file.exists():
        raise DriveError("the bot kept no token file")
    # Killed in flight: no Leave, no goodbye.
    d.log("killing Viper (SIGKILL)")
    first.stopped = True
    first.popen.kill()
    first.wait(10)
    if not server.wait_for(r"Viper dropped out: the AI flies plane 0, kept for it$", 40):
        d.problem("the server did not keep plane 0 for the dropped Viper")
    d.sleep(1)
    second = d.start(
        "viper2",
        [d.bot, "--connect", f"{LOCALHOST}:{port}", "--seconds", "20", "--callsign", "Viper", "--slot", "0",
         "--token-file", str(token_file)],
    )
    second.finish(90, 0)
    for problem in rejoin_problems(second.text(), "Viper", 0):
        d.problem(problem)
    second.expect(r"^Viper: debrief: ", "a debrief")
    second.expect(r"^Viper: The connection ended: the player left\.$", "a clean leave")
    second.forbid(r"a protocol error|too many bad packets|the game data differs|No answer from the server", "a network problem")
    stay.finish(120, 0)
    server.send("end")
    server.finish(40, 0)
    log_must(
        d,
        server_log(d),
        r"Viper dropped out: the AI flies plane 0, kept for it$",
        r"Viper rejoined with its token: plane 0 is waiting$",
        r"Viper took plane 0\b.*(?:\n.*)*Viper took plane 0\b",
        forbid=r"protocol error|bad packets|\bfault\b",
    )
    if re.search(r"Stay took plane 0\b", server_log(d)):
        d.problem("another player took the plane kept for Viper")


# --------------------------------------------------------------------------
# Host migration (stage K, slice K9): a hosting `tore-bot --host`, bots that stand by
# --------------------------------------------------------------------------

# How long after a bot noticed the loss its snapshots must come again: the plan's 5 seconds from the loss, less the
# 1.5 seconds of silence a client waits before it notices (docs/ARCHITECTURE.md, "Losing the host").
SNAPSHOTS_AGAIN_MS = 3500  # times the runner's --timeout-scale: a loaded machine steps the fast-forward slower
# After a handover every client races the new host at once, so the gap is a round trip or two and the fast-forward.
SNAPSHOTS_AGAIN_HANDOVER_MS = 2500
# The resume window the new host holds the clock for at most (docs/ARCHITECTURE.md, "Losing the host"; since slice
# K10 it ends once the last pilot it waits for has resumed, the old host's own player not waited for), and the cost of
# a fast-forward tick above which a build or machine is too slow to judge the 5 second target by; then each pilot's
# snapshots must come within FOLLOW_LIVE_MS of the new host going live.
RESUME_WINDOW_MS = 1500
SLOW_TICK_MS = 2.0
FOLLOW_LIVE_MS = 1500


def write_mission(d: Drive, separation_nm: int = 5) -> Path:
    """The guide's mission, with the enemy `separation_nm` away, in the scenario's work folder."""
    path = d.work / "mission.txt"
    path.write_text(guide_mission(separation_nm))
    return path


def world_lines(text: str, who: str | None = None) -> list[dict]:
    """The `host: world:` lines a hosting bot prints once a second while the mission flies: the tick, the guided
    missiles in flight, the aircraft kills in the world and the players."""
    name = who or r"\w+"
    found = []
    for m in re.finditer(
        rf"^({name}): host: world: tick (\d+), (\d+) missiles in flight, (\d+) aircraft kills, (\d+) players$", text, re.M
    ):
        found.append(
            {"who": m.group(1), "tick": int(m.group(2)), "missiles": int(m.group(3)), "kills": int(m.group(4)),
             "players": int(m.group(5))}
        )
    return found


def results_kills(text: str, who: str) -> int | None:
    """The aircraft kills the last `results:` line of `who` adds up (every plane's `Nk`), or None when it has none."""
    lines = re.findall(rf"^{who}: results: (.*)$", text, re.M)
    if not lines:
        return None
    return sum(int(k[:-1]) for k in re.findall(r"\d+k\b", lines[-1]))


def snapshots_again(text: str, who: str) -> list[int]:
    """The milliseconds after `who` noticed the loss of its host that its snapshots came again, one for each loss."""
    return [int(ms) for ms in re.findall(rf"^{who}: migrate: snapshots again (\d+) ms after the loss was noticed$", text, re.M)]


def migrate_problems(
    pilots: str, callsigns: list[str], before: dict | None, limit_ms: float, handover: bool = False
) -> list[str]:
    """What the pilots (bots that stand by) printed of a migration: exactly one of them took the game over, and the
    host's own lines say it replayed from the old host's last tick; the others resumed with it ("The game moved
    to"); each one's snapshots came again within `limit_ms` of noticing the loss; the new host's world carries on
    from the old host's last tick with at least the kills the old host had; and the new host's Results keep them."""
    problems = []
    took = re.findall(r"^(\w+): migrate: taking the game over$", pilots, re.M)
    if len(took) != 1:
        problems.append(f"{len(took)} games took the game over, expected exactly one: {took}")
        return problems
    new = took[0]
    if not re.search(rf"^{new}: host: took the game over at tick \d+: replayed \d+ ticks in \d+ ms, \d+ players expected back$", pilots, re.M):
        problems.append(f"{new} has no host line saying it took the game over")
    if not re.search(rf"^{new}: host: live at tick \d+, \d+ ms after the takeover, \d+ ticks fast-forwarded$", pilots, re.M):
        problems.append(f"{new}'s host never went live")
    live = re.search(rf"^{new}: host: live at tick \d+, (\d+) ms after the takeover, (\d+) ticks fast-forwarded$", pilots, re.M)
    live_ms = int(live.group(1)) if live else None
    slow_note = ""
    if live and int(live.group(2)) > 0:
        resumed = [int(ms) for ms in re.findall(rf"^{new}: host: \w+ resumed (\d+) ms after the takeover", pilots, re.M)]
        window = min(RESUME_WINDOW_MS, max(resumed)) if resumed else RESUME_WINDOW_MS
        cost = (live_ms - window) / int(live.group(2))
        if cost > SLOW_TICK_MS:
            # The new host stepped its fast-forward at more than SLOW_TICK_MS a tick (a debug build on a busy machine,
            # not the plan's release build): the 5 second target cannot be judged here, so the pilots must follow the
            # host going live closely instead.
            limit_ms = max(limit_ms, live_ms + FOLLOW_LIVE_MS)
            slow_note = f" (the fast-forward cost {cost:.1f} ms a tick)"
    for name in callsigns:
        # The game that took over hosts it: it is told nothing of a move to itself (after a loss its own client
        # still says "Lost contact"; after a handover it says nothing).
        if name != new and not re.search(rf"^{name}: Lost contact with the host\. Moving the game to \w+\.\.\.$", pilots, re.M) and not handover:
            problems.append(f"{name} printed no notice of the loss")
        if name != new and not re.search(rf"^{name}: The game moved to {new}\.$", pilots, re.M):
            problems.append(f"{name} was never told \"The game moved to {new}.\"")
        times = snapshots_again(pilots, name)
        if not times:
            problems.append(f"{name}'s snapshots never came again")
        elif min(times) > limit_ms:
            problems.append(f"{name}'s snapshots came again after {min(times)} ms, over {limit_ms:.0f} ms{slow_note}")
        elif live_ms is not None and min(times) > live_ms + FOLLOW_LIVE_MS:
            problems.append(f"{name}'s snapshots came {min(times) - live_ms:.0f} ms after the new host went live, over {FOLLOW_LIVE_MS} ms")
    if not handover:
        for name in callsigns:
            if name != new and not re.search(rf"^{new}: host: {name} resumed \d+ ms after the takeover", pilots, re.M):
                problems.append(f"the new host never saw {name} resume")
    if before is not None:
        after = [w for w in world_lines(pilots, new)]
        if not after:
            problems.append(f"the new host {new} printed no world lines")
        else:
            if after[0]["tick"] < before["tick"]:
                problems.append(f"the new host's world starts at tick {after[0]['tick']}, before the old host's {before['tick']}")
            if after[0]["kills"] < before["kills"]:
                problems.append(f"the new host's world holds {after[0]['kills']} kills, the old host's had {before['kills']}")
            if after[-1]["tick"] <= before["tick"] + 120:
                problems.append("the new host's world did not carry on past the old host's last tick")
        kills = results_kills(pilots, new)
        if kills is None:
            problems.append(f"{new} printed no Results at the mission's end")
        elif kills < before["kills"]:
            problems.append(f"the Results hold {kills} aircraft kills, the old host's world had {before['kills']}")
    return problems


def wait_world(d: Drive, host: Proc, seconds: float, ok) -> dict | None:
    """Polls the host's world lines until `ok(line)` holds for the newest; returns that line, or None on time out."""
    end = time.time() + seconds * d.scale
    while time.time() < end:
        d._check_time()
        lines = world_lines(host.text())
        if lines and ok(lines[-1]):
            return lines[-1]
        if not host.alive():
            return None
        time.sleep(0.05)
    return None


def start_migration(d: Drive, host_seconds: int, pilot_seconds: int, standbys: int = 2):
    """A hosting bot (Lead, plane 0) and three pilots that stand by (Pilot1 to Pilot3, planes 1 to 3); the host
    starts the mission once all four are in the lobby and `standbys` standbys are ready."""
    port = d.port()
    mission = write_mission(d)
    host = d.start(
        "host",
        [d.bot, "--host", mission, "--port", port, "--callsign", "Lead", "--slot", "0", "--seconds", host_seconds,
         "--players", "4", "--wait-standbys", standbys],
    )
    if not host.wait_for(r"^Lead: hosting ", 60):
        raise DriveError("the hosting bot never began to host")
    pilots = d.start(
        "pilots",
        [d.bot, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Pilot", "--count", "3", "--slot", "1",
         "--seconds", pilot_seconds, "--standby", "on"],
    )
    if not host.wait_for(r"^Lead: host: mission started", 150):
        raise DriveError("the hosting bot never started the mission (no two standbys ready?)")
    return host, pilots, port


def drive_migrate_kill(d: Drive) -> None:
    """Host migration (slice K9): a hosting bot and three pilots that stand by fly the guide's mission 5 nm apart.
    Once a kill is booked (and, when one flies, a guided missile) the hosting bot is killed with SIGKILL, in the
    fight: the first standby takes the game over, every pilot's snapshots come again within 5 seconds of the loss, the
    new host's world carries on from the old host's last tick with the kills the old host had, and its Results keep
    them."""
    fresh_data(d)
    host, pilots, _ = start_migration(d, 400, 110)
    first = wait_world(d, host, 90, lambda w: w["kills"] >= 1 and w["missiles"] >= 1)
    if first is None:
        d.log("no guided missile flew with a kill booked; killing the host with the kill alone")
        first = wait_world(d, host, 90, lambda w: w["kills"] >= 1)
    if first is None:
        raise DriveError("the AI booked no kill in 3 minutes of flight")
    before = world_lines(host.text())[-1]
    d.log(f"killing the host (SIGKILL) at {before}")
    host.stopped = True
    host.popen.kill()
    host.wait(10)
    if not pilots.wait_for(r"^Pilot3: migrate: snapshots again \d+ ms", 30):
        d.problem("a pilot's snapshots never came again after the host was killed")
    pilots.finish(200, 0)
    for problem in migrate_problems(pilots.text(), ["Pilot1", "Pilot2", "Pilot3"], before, SNAPSHOTS_AGAIN_MS * d.scale):
        d.problem(problem)
    pilots.forbid(NET_BAD + r"|No other game could take over", "a network problem or a session given up")
    pilots.forbid(r"migrations resumed \d+, failed [1-9]", "a failed migration")
    pilots.forbid(r"corrected [1-9]", "a corrected plane at the resume")
    pilots.expect(r"^Pilot\d: migrate: migrations resumed 1, failed 0, corrected 0$", "the migration's counts")


def drive_migrate_handover(d: Drive) -> None:
    """Host migration (slice K9): the hosting bot leaves on purpose when its time is up. It hands the game over to the
    first standby (no kill, no wait for a timeout): that pilot takes the game over at once, the other pilots follow
    it within about a second, the old host exits 0, and the world carries on from the old host's last tick."""
    fresh_data(d)
    host, pilots, _ = start_migration(d, 80, 120)
    pilots.wait_for(r"^Pilot3: seat \d+, plane 3", 30)
    host.finish(120, 0)
    host.expect(r"^Lead: host: handing the game over to player \d+$", "the handover")
    host.expect(r"^Lead: host: the game was handed over$", "the handover's end")
    host.forbid(r"no handover|the host left the game", "a host that left instead of handing over")
    before = world_lines(host.text())[-1] if world_lines(host.text()) else None
    pilots.finish(200, 0)
    for problem in migrate_problems(
        pilots.text(), ["Pilot1", "Pilot2", "Pilot3"], before, SNAPSHOTS_AGAIN_HANDOVER_MS * d.scale, handover=True
    ):
        d.problem(problem)
    pilots.forbid(NET_BAD + r"|No other game could take over", "a network problem or a session given up")
    pilots.forbid(r"migrations resumed \d+, failed [1-9]", "a failed migration")


def drive_migrate_relay(d: Drive) -> None:
    """Host migration through the master (slice K9, with K8): a hosting bot lists its game on a `tore-master` on this
    machine; two pilots that stand by join it directly and a third bot joins through the master's relay (`--path
    relay`; a relayed player is never a standby). The host is killed (SIGKILL) in the fight: the first standby takes
    the game over and resumes the listing from the part the old host journaled, the master moves the relay channel to
    the new host's address, and the relayed bot, which keeps its channel, flies on with the new host."""
    fresh_data(d)
    master, mport = start_master(d)
    port = d.port()
    mission = write_mission(d)
    name = "Migrate relay"
    host = d.start(
        "host",
        [d.bot, "--host", mission, "--port", port, "--master", f"{LOCALHOST}:{mport}", "--name", name,
         "--callsign", "Lead", "--slot", "0", "--seconds", 400, "--players", "4", "--wait-standbys", "2"],
    )
    if not host.wait_for(r"^Lead: hosting ", 60):
        raise DriveError("the hosting bot never began to host")
    if not host.wait_for(r"^Lead: listing: Listed\b", 30):
        raise DriveError("the hosting bot's game was never listed on the master")
    pilots = d.start(
        "pilots",
        [d.bot, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Pilot", "--count", "2", "--slot", "1",
         "--seconds", 130, "--standby", "on"],
    )
    relay = d.start(
        "relay",
        [d.bot, "--master", f"{LOCALHOST}:{mport}", "--listing", name, "--path", "relay", "--callsign", "Relay",
         "--slot", "3", "--seconds", 130],
    )
    if not host.wait_for(r"^Lead: host: mission started", 150):
        raise DriveError("the hosting bot never started the mission")
    if not wait_world(d, host, 90, lambda w: w["kills"] >= 1):
        raise DriveError("the AI booked no kill in 90 seconds of flight")
    before = world_lines(host.text())[-1]
    d.log(f"killing the host (SIGKILL) at {before}")
    host.stopped = True
    host.popen.kill()
    host.wait(10)
    if not relay.wait_for(r"^Relay: migrate: snapshots again \d+ ms", 30):
        d.problem("the relayed bot's snapshots never came again after the host was killed")
    pilots.finish(200, 0)
    relay.finish(200, 0)
    together = pilots.text() + "\n" + relay.text()
    for problem in migrate_problems(together, ["Pilot1", "Pilot2", "Relay"], before, SNAPSHOTS_AGAIN_MS * d.scale):
        d.problem(problem)
    relay.expect(r"^Relay: joined through the Internet Lobby, path relay$", "the relayed join")
    relay.forbid(r"The relay closed|No other game could take over|relay is (busy|full|switched off)", "a lost relay")
    together_text = together + "\n" + host.text()
    if re.search(r"standby Relay", together_text):
        d.problem("the relayed bot was appointed a standby")
    if not re.search(r"^Pilot\d: listing: resumed from the old host's part$", pilots.text(), re.M):
        d.problem("the new host never resumed the listing from the old host's part")
    master.send("quit")
    master.finish(20, 0)
    master.expect(
        r"^relay moved listing=[0-9a-f]{16} from=127\.0\.0\.1:\d+ to=127\.0\.0\.1:\d+ channels=1$",
        "the relay channel moved with the listing",
    )
    master.expect(r"^moved id=[0-9a-f]{16} from=127\.0\.0\.1:\d+ to=127\.0\.0\.1:\d+$", "the listing moved")
    master.forbid(r"relay refused|reason=(idle|over its rate)", "a channel closed by the master or refused")
    for text in (pilots.text(), relay.text()):
        if re.search(NET_BAD, text):
            d.problem("a network problem: " + re.search(NET_BAD, text).group(0))


def drive_reach_upload(d: Drive) -> None:
    """Host selection (slices K3 and K6) on loopback: a hosting bot waits for one ready standby before it starts the
    mission. Aa stands by (`--standby on`), Bb does not (`--standby off`: its game says it may not host). The host
    runs its reach tests and the upload test on Aa, appoints it first standby and streams it the mission; Bb is
    never appointed; Aa's standby is warm and its checks come out equal. When its time is up the host hands over to Aa,
    and Bb follows: it resumes with Aa however late it joined (slice K10: a player who joined after the host last sent
    its Succession never heard of the standby and dropped)."""
    fresh_data(d)
    port = d.port()
    mission = write_mission(d)
    host = d.start(
        "host",
        [d.bot, "--host", mission, "--port", port, "--callsign", "Lead", "--slot", "0", "--seconds", 70,
         "--players", "3", "--wait-standbys", "1"],
    )
    if not host.wait_for(r"^Lead: hosting ", 60):
        raise DriveError("the hosting bot never began to host")
    aa = d.start(
        "aa", [d.bot, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Aa", "--slot", "1", "--seconds", 100, "--standby", "on"]
    )
    bb = d.start(
        "bb", [d.bot, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Bb", "--slot", "2", "--seconds", 100, "--standby", "off"]
    )
    if not host.wait_for(r"^Lead: host: mission started", 150):
        raise DriveError("the hosting bot never started: no standby passed the reach and upload tests")
    if not host.wait_for(r"^Lead: host: standby Aa First, warm, Warm, checks [1-9]\d* equal 0 differ", 90):
        d.problem("Aa never showed as a warm first standby with equal checks")
    host.finish(120, 0)
    host.forbid(r"standby Bb", "Bb, whose game said it may not host, appointed")
    host.forbid(r"standby Aa .* [1-9]\d* differ", "a check that differed")
    aa.expect(r"^Aa: standby: appointed, warm$", "Aa's appointment")
    bb.forbid(r"standby:", "a standby on Bb")
    aa.finish(150, 0)
    bb.finish(150, 0)
    aa.expect(r"^Aa: host: Bb resumed \d+ ms after the takeover", "Bb resuming with Aa after the handover")
    bb.expect(r"^Bb: migrate: migrations resumed 1, failed 0", "Bb following the handover")
    for text in (host.text(), aa.text(), bb.text()):
        if re.search(NET_BAD, text):
            d.problem("a network problem: " + re.search(NET_BAD, text).group(0))


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
    log_must(d, server_log(d), r"joined as Bot \(path: punched\)", r"Bot( \(plane \d+\))? left: left", forbid=NET_BAD)


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


def drive_master_relay(d: Drive) -> None:
    """A `tore-bot --master --listing --path relay` joins a `tore-server` listed on a `tore-master` on this machine
    through the master's relay (slice J3): on one machine every direct path works, so the bot asks for the relay at
    once and never races. It joins the channel's relayed address, flies 30 seconds with no drop and leaves; the
    master logs the channel opened and closed by an end with the bytes each way, and its status line counts the
    channel and the month's relayed bytes."""
    master, mport = start_master(d)
    port = d.port()
    server = start_server(d, port, broadcast="on", master=f"{LOCALHOST}:{mport}")
    if not server.wait_for(r"Broadcasting: listed on the Internet Lobby", 10):
        d.problem("the server never said it was listed")
    bot = d.start(
        "bot",
        [d.bot, "--master", f"{LOCALHOST}:{mport}", "--listing", "T.O.R.E server", "--path", "relay", "--seconds", "30"],
    )
    bot.finish(120, 0)
    server.finish(40, 0)
    master.send("status")
    master.send("quit")
    master.finish(20, 0)
    master.expect(r"^relay ACTIVE: up to 64 channels, ", "the start line saying the relay is active")
    master.expect(r"^relay this month \(\d{4}-\d{2}\): \d+B of 800 GB relayed$", "the month's figure at start")
    bot.expect(rf'^found "T\.O\.R\.E server" on the Internet Lobby at 127\.0\.0\.1:{mport}$', "the listing found")
    bot.expect(r"^Bot: introduced; not racing its [1-8] address(es)?$", "the introduction, not raced")
    bot.expect(r"^Bot: --path relay; asking for the relay\.\.\.$", "the relay asked for")
    bot.expect(r"^Bot: the relay is open; joining through it$", "the channel open")
    bot.expect(r"^Bot: joined through the Internet Lobby, path relay$", "the join through the relay")
    bot.expect(r"^Bot: seat \d+, plane \d+, at tick \d+$", "a seating")
    bot.expect(r"^Bot: round trip \d+ ms, loss", "flight figures")
    bot.expect(r"^Bot: debrief: (success|failure), \d+ kills, \d+ seconds$", "a debrief")
    bot.expect(r"^Bot: The connection ended: the player left\.$", "a clean leave")
    bot.forbid(NET_BAD, "a network problem")
    bot.forbid(r"trying \d+ address|path punched|The relay closed|relay is (busy|full|switched off)", "a race or a lost relay")
    server.expect(r"^mission ended: everyone left$", "the end")
    server.forbid(NET_BAD, "a network problem")
    opened = rf"^relay opened channel=[0-9a-f]{{8}} host=127\.0\.0\.1:{port} player=127\.0\.0\.1:\d+ channels=1$"
    master.expect(opened, "the channel opened")
    m = re.search(
        r"^relay closed channel=[0-9a-f]{8} .* reason=closed by an end to-host=(\d+) to-player=(\d+) channels=0$",
        master.text(),
        re.M,
    )
    if not m:
        d.problem("the master never logged the channel closed by an end")
    elif int(m.group(1)) < 10_000 or int(m.group(2)) < 100_000:
        d.problem(f"too few relayed bytes for 30 seconds of flight: {m.group(1)} to the host, {m.group(2)} to the bot")
    master.expect(
        r"^status listings=\d+ sources=\d+ .* relayed=1 channels=0 relay-month=[\d.]+(KB|MB) ",
        "the status line counting the channel and the month's bytes",
    )
    master.forbid(r"reason=(idle|over its rate)|relay refused", "a channel closed by the master or refused")
    master.expect(r"^Stopped$", "the stop line")
    figure = sorted((d.work / "state").glob("relay-*.txt"))
    if not figure or int(figure[-1].read_text().strip() or 0) < 100_000:
        d.problem(f"the month's relay figure was not written at the stop: {figure}")
    log_must(d, server_log(d), r"joined as Bot \(path: relay\)", r"Bot( \(plane \d+\))? left: left", forbid=NET_BAD)


# The script of a relayed join from the Internet Lobby: open the screen, select the listed server, Join, take a
# plane, Ready, fly a few seconds and exit.
INTERNET_RELAY_SCRIPT = """wait 10
movemenu 150 48
wait 0.6
click
wait 0.6
movemenu 150 90
wait 0.6
click
wait 5
movemenu 100 194
wait 0.6
click
wait 2
movemenu 200 432
wait 0.6
click
wait 12
snapshot SHOTS/relay-joined.ppm
movemenu 150 176
wait 0.6
click
click
wait 2
movemenu 460 434
wait 0.6
click
wait 3
waittick 720 40
shot SHOTS/relay-flight.ppm
exit
"""


def drive_internet_relay(d: Drive) -> None:
    """The game's Internet Lobby joins a `tore-server` listed on a master on this machine through the master's relay
    (slice J5): `TORE_JOIN_PATH=relay` makes the game ask for the relay at once and send nothing to the server's own
    address, since on one machine every direct path works. The screen's and the session's lines say each step, the
    game joins the channel's relayed address, takes a plane, readies and flies a few seconds; the master logs the
    channel and the net log names the path."""
    master, mport = start_master(d)
    port = d.port()
    server = start_server(d, port, broadcast="on", master=f"{LOCALHOST}:{mport}")
    if not wait_count(d, master, rf'^listed id=[0-9a-f]{{16}} from=127\.0\.0\.1:{port} name="T\.O\.R\.E server"', 1, 10):
        d.problem("the master never listed the server")
    (d.data / "network-v1.conf").write_text(
        f"tore-network 1\ncallsign Viper\nport {d.port()}\nmaster {LOCALHOST}:{mport}\n"
    )
    shots = d.work / "shots"
    shots.mkdir(exist_ok=True)
    opened = r"Internet Lobby: Asking the Internet Lobby at"
    d.env["TORE_JOIN_PATH"] = "relay"
    # A scripted click on a window that is slow to come up can land before the menu answers; one more try,
    # starting later, tells that from a real failure.
    for attempt, start_wait in enumerate((10, 25), start=1):
        for old in (d.data / "logs").glob("tore-*.log"):
            old.unlink()
        script = d.work / f"relay{attempt}.txt"
        script.write_text(INTERNET_RELAY_SCRIPT.replace("SHOTS", str(shots)).replace("wait 10\n", f"wait {start_wait}\n", 1))
        game = d.start(f"game{attempt}", [d.app, *GAME_FLAGS, "--input-script", script], window=True)
        game.finish(200, 0)
        log = "\n".join(p.read_text(errors="replace") for p in sorted((d.data / "logs").glob("tore-*.log")))
        if opened in log:
            break
        d.log(f"attempt {attempt}: the scripted clicks did not open the screen")
    for pattern, what in (
        (r"Internet Lobby: Asking the Internet Lobby to introduce you to 'T\.O\.R\.E server'\.\.\.", "Join asking"),
        (r"Internet Lobby: Joining 'T\.O\.R\.E server' through the relay only \(TORE_JOIN_PATH=relay\)\.\.\.", "the introduction, not raced"),
        (r"Network: Asking for the relay\.\.\.", "the relay asked for"),
        (r"Network: The relay is open; joining through it\.\.\.", "the channel open"),
        (r"Network: Connected through the relay\.", "the path the join took"),
        (r"Network: joined \[100::1:[0-9a-f]+:[0-9a-f]+\]:0, path relay", "the relayed address joined"),
        (r"Network: seated in plane 0", "a seating after Join, a slot and Ready"),
        (r"Network: lobby: Flying, .* Viper plane 0", "the mission flying"),
    ):
        if not re.search(pattern, log):
            d.problem(f"the game's log lacks {what}: /{pattern}/")
    if re.search(r"master\.jroverton\.com|master\.invalid", log):
        d.problem("the game talked about a master other than the scenario's own")
    if re.search(r"Connected directly|path punched", log):
        d.problem("the game joined directly although only the relay was allowed")
    tsv = "\n".join(p.read_text(errors="replace") for p in sorted((d.data / "logs").glob("net-*.tsv")))
    if not re.search(r"^[\d.]+\tpath\trelay\t\[100::1:", tsv, re.M):
        d.problem("the net log has no `path relay` line")
    for name in ("relay-joined", "relay-flight"):
        if not (shots / f"{name}.ppm").exists():
            d.problem(f"the script's {name}.ppm was not written")
    game.forbid(NET_BAD, "a network problem")
    stop_server(d, server)
    master.send("status")
    master.send("quit")
    master.finish(20, 0)
    master.expect(
        rf"^relay opened channel=[0-9a-f]{{8}} host=127\.0\.0\.1:{port} player=127\.0\.0\.1:\d+ channels=1$",
        "the channel opened",
    )
    master.expect(r"^status listings=\d+ sources=\d+ .* relayed=1 ", "the status line counting the channel")
    master.forbid(r"relay refused|reason=over its rate", "a refused or flooded channel")
    log_must(d, server_log(d), r"joined as Viper", r"Viper took the slot of plane 0", forbid=NET_BAD)


def drive_convert(d: Drive) -> None:
    """A bot's capture of a real flight converts into a replay (`--convert-capture`): the replay reads back through
    the exports, carries the contrails, motor smoke and gun rounds the game makes again (slice E2), converting twice
    gives the same bytes, and a copy cut short still converts and says so."""
    port = d.port()
    # At 40,000 feet, above every aircraft's contrail onset (30,000 to 35,000), so the conversion has contrails to make.
    mission = guide_mission(separation_nm=5).replace("start airborne 20000", "start airborne 40000")
    if "start airborne 40000" not in mission:
        raise DriveError("the guide's example mission no longer starts airborne at 20000 feet")
    server = start_server(d, port, mission)
    replays = d.work / "replays"
    replays.mkdir(exist_ok=True)
    capture = replays / "2026-10-05_1500_NET_127001.tore-capture"
    bots = start_bots(d, port, "bot", 25, "--callsign", "Viper", "--capture", capture)
    bots.finish(90, 0)
    server.finish(40, 0)
    bots.forbid(NET_BAD, "a network problem")
    if not capture.exists() or capture.stat().st_size < 100_000:
        raise DriveError("the bot wrote no capture of a flight")
    run = d.run("convert", [d.app, "--convert-capture", capture], timeout=120)
    run.expect(r"^Replay: .*_UKR_F18\.tore-replay \(\d+ frames, \d+\.\d s, 12 aircraft\)$", "the replay's line")
    made = re.search(r"^Made again: (\d+) smoke puffs, (\d+) contrail puffs, (\d+) gun rounds$", run.text(), re.M)
    if not made:
        d.problem("the conversion did not say what it made again (smoke, contrails, gun rounds)")
        smoke = contrails = rounds = -1
    else:
        smoke, contrails, rounds = (int(n) for n in made.groups())
    replay = next(replays.glob("*_UKR_F18.tore-replay"), None)
    if replay is None:
        raise DriveError("the conversion wrote no replay beside the capture")
    info = d.run("info", [d.app, "--recording-info", replay], timeout=60)
    info.expect(r"^State +finished normally", "a finished replay")
    info.expect(r"^Mission +Network flight", "the network flight")
    info.expect(r"^Setting +net\.callsign = Viper", "the callsign")
    info.expect(r"^Aircraft +0 +You +F/A-18D", "the player as You")
    info.expect(r"^\s+\d+ net\.stats$", "the network figures")
    info.expect(r"^Result +end=end flight, net\.seconds=", "the footer's figures")
    info.forbid(r"INCOMPLETE|^Problem", "damage")
    # The host sent launches and gun bursts; the smoke of the motors, the contrails at that height and the rounds of
    # every burst are made again, which only the game's conversion does (the host sends none of them).
    count = lambda event: int((re.search(rf"^\s+(\d+) {re.escape(event)}$", info.text(), re.M) or [0, 0])[1])
    if made and count("weapon.launch") and smoke < 1:
        d.problem(f"{count('weapon.launch')} launches and no missile smoke")
    if made and contrails < 1:
        d.problem("a flight at 40,000 feet and no contrails")
    if made and (count("weapon.gun_burst") > 0) != (rounds > 0):
        d.problem(f"{count('weapon.gun_burst')} gun burst events and {rounds} gun rounds made")
    log = d.run("log", [d.app, "--recording-log", replay, "--out", d.work / "log"], timeout=60)
    log.expect(r"^Recording log: ", "the debug log")
    acmi = d.run("acmi", [d.app, "--recording-acmi", replay, "--out", d.work / "net.txt.acmi", "--guns"], timeout=60)
    acmi.expect(r"^Tacview file: ", "the Tacview file")
    for path in (d.work / "log" / "summary.txt", d.work / "log" / "log.jsonl", d.work / "net.txt.acmi"):
        if not path.exists() or path.stat().st_size == 0:
            d.problem(f"{path.name} was not written")
    if rounds > 0 and "Projectile+Bullet" not in (d.work / "net.txt.acmi").read_text(errors="replace"):
        d.problem("the Tacview file with --guns shows none of the gun rounds the conversion made")
    twice = []
    for name in ("a", "b"):
        out = d.work / f"{name}.tore-replay"
        d.run(f"convert-{name}", [d.app, "--convert-capture", capture, "--out", out], timeout=120)
        twice.append(out.read_bytes() if out.exists() else b"")
    if not twice[0] or twice[0] != twice[1]:
        d.problem("converting the same capture twice did not give the same bytes")
    cut = d.work / "cut.tore-capture"
    data = capture.read_bytes()
    cut.write_bytes(data[: len(data) * 60 // 100])
    short = d.run("convert-cut", [d.app, "--convert-capture", cut, "--out", d.work / "cut.tore-replay"], timeout=120)
    short.expect(r"is cut short: converted up to its last whole record", "the cut report")
    cut_info = d.run("info-cut", [d.app, "--recording-info", d.work / "cut.tore-replay"], timeout=60)
    cut_info.expect(r"^Result +end=cut, capture=cut short at byte", "the footer's note")


# --------------------------------------------------------------------------
# Stage L: content and gaps (slice L3)
# --------------------------------------------------------------------------

# The 1.0 disc, which net-content-builds imports for its bot.
DISC_10 = ROOT / "gameassets" / "fighters-anthology" / "disc1"


def content_item_lines(text: str) -> list[str]:
    """The item lines of a content report (`--check` or `tore-bot --content-report`): "aircraft F18.PT <digest>"."""
    return [m.group(1) for m in re.finditer(r"(?m)^(?:\[\w-]+\] )?  ((?:aircraft|theater|weapon) \S+ [0-9a-f]{16}|shared data [0-9a-f]{16})$", text)]


def drive_content_builds(d: Drive) -> None:
    """A server on the profile's 1.02F import and a bot on a 1.0 import made here: every item is the same, the bot
    says nothing about the build (John, 2026-10-06: the audit found no difference a player sees), and flies."""
    if not (DISC_10 / "SETUP.ESA").exists():
        raise DriveError(f"the 1.0 disc is missing: {DISC_10} (net-content-builds needs it)")
    fa10 = d.work / "fa10"
    d.run("import", [d.server, "--import", DISC_10, "--data-dir", fa10], timeout=600)
    report = d.run("report", [d.bot, "--content-report", "--data-dir", fa10], timeout=120)
    report.expect(r"^Content: Fighters Anthology 1\.0, imported by T\.O\.R\.E ", "the 1.0 import's source")
    port = d.port()
    config = write_server_files(d, port, guide_mission(separation_nm=5))
    check = d.run("check", [d.server, "--config", config, "--check"], timeout=120)
    check.expect(r"^Content: Fighters Anthology 1\.02F, imported by ", "the server's 1.02F source")
    ours, theirs = content_item_lines(check.text()), content_item_lines(report.text())
    if not ours:
        d.problem("tore-server --check listed no content items")
    elif ours != theirs:
        differing = sorted(set(ours) ^ set(theirs))
        d.problem(f"the 1.0 and 1.02F imports differ in {len(differing)} item lines, such as {differing[:3]}")
    server = start_server(d, port, guide_mission(separation_nm=5))
    bot = start_bots(d, port, "bot", 30, "--callsign", "Old", "--data-dir", fa10)
    bot.finish(90, 0)
    server.finish(40, 0)
    bot.forbid(r"imported Fighters Anthology|differs from the host's", "a build or difference line")
    bot.expect(r"^Old: gaps: none$", "no gaps")
    bot.expect(r"^Old: seat \d+, plane \d+, at tick \d+$", "a seating")
    bot.expect(r"^Old: debrief: ", "a debrief")
    bot.expect(r"^Old: The connection ended: the player left\.$", "a clean leave")
    bot.forbid(NET_BAD, "a network problem")
    server.expect(r"^Content: Fighters Anthology 1\.02F, imported by ", "the server's content line")
    server.forbid(NET_BAD, "a network problem")
    log_must(
        d, server_log(d), r"joined as Old",
        r"content Old: Fighters Anthology 1\.0, imported by T\.O\.R\.E \S+ \([0-9a-f]+\); the same items as the host",
        forbid=NET_BAD + r"|^.*gaps: (?!none)",
    )
    # The 1.0 import is about 170 MB; the run's log keeps what it showed.
    shutil.rmtree(fa10, ignore_errors=True)


def drive_content_missing(d: Drive) -> None:
    """A bot whose import lacks the Su-27 joins the guide's mission (which flies it): every player gets the gap, the
    bot is unable with the words and leaves, another bot flies, and the server's log names the gap and its end."""
    port = d.port()
    server = start_server(d, port, guide_mission(separation_nm=5), empty_timeout=3)
    hawk = start_bots(d, port, "hawk", 15, "--callsign", "Hawk", "--drop-resource", "SU27.PT", "--expect-unable")
    viper = start_bots(d, port, "viper", 35, "--callsign", "Viper")
    hawk.finish(60, 0)
    viper.finish(90, 0)
    server.finish(40, 0)
    hawk.expect(r"^dropped SU27\.PT from the import$", "the dropped profile")
    hawk.expect(
        r"^Hawk: Your game has no Su-27[^,]*, which this mission flies\. .*Re-import Fighters Anthology \(Pref, "
        r"Re-import\) to add it\. \(",
        "its own words",
    )
    hawk.expect(r"^Hawk: gaps: aircraft SU27\.PT \(Hawk lacks it\)$", "the gap")
    hawk.expect(r"^Hawk: Your game differs from the host's: no Su-27[^.]*\.$", "its own difference line")
    hawk.expect(r"^Hawk: The connection ended: the player left\.$", "a clean leave")
    hawk.forbid(r"^Hawk: seat \d+", "a seating")
    viper.expect(r"^Viper: gaps: aircraft SU27\.PT \(Hawk lacks it\)$", "the gap")
    viper.expect(r"^Viper: Hawk's game differs from the host's: no Su-27[^.]*\.$", "Hawk's difference line")
    viper.expect(r"^Viper: lobby: .*Hawk no slot unable", "Hawk unable in the lobby")
    viper.expect(r"^Viper: seat \d+, plane \d+, at tick \d+$", "a seating")
    viper.expect(r"^Viper: debrief: ", "a debrief")
    viper.forbid(NET_BAD, "a network problem")
    log_must(
        d, server_log(d), r"joined as Hawk", r"joined as Viper",
        r"content Hawk: .*; lacks aircraft SU27\.PT$",
        r"gaps: aircraft SU27\.PT \(Hawk lacks it\)$",
        r"Hawk cannot play the mission: Hawk's game has no Su-27[^,]*, which this mission flies\.",
        r"gaps: none$",
    )


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
            name="net-server-results", lane="net", args=[], driver=drive_results, uses=("server", "bot"), timeout=300,
            notes="PvP with a one-minute time limit: each bot hears the results, a row for every aircraft, and the winner",
        ),
        Scenario(
            name="net-server-king", lane="net", args=[], driver=drive_king, uses=("server", "bot"), timeout=300,
            notes="`king first-player`: the King bot changes the settings to PvP and starts; a bot takes an enemy plane",
        ),
        Scenario(
            name="net-server-hunt", lane="net", args=[], driver=drive_hunt, uses=("server", "bot"), timeout=240,
            notes="a bot shoots down dummy enemies on the guide's mission 5 nm apart: the kill limit ends the mission",
        ),
        Scenario(
            name="net-server-rate", lane="net", args=[], driver=drive_rate_server, uses=("server", "bot"), timeout=240,
            notes="a server whose file sets snapshot-rate 30: the lobby carries it and a bot's flight runs at 30 a second "
            "(slice R1)",
        ),
        Scenario(
            name="net-host-rate", lane="net", args=[], driver=drive_rate_host, uses=("bot",), timeout=300,
            notes="a game a hosting bot hosts: the King turns the snapshot rate to 30 in the lobby, a second bot joins, "
            "and both flights run at 30 a second (slice R1)",
        ),
        Scenario(
            name="net-server-pvp", lane="net", args=[], driver=drive_pvp, uses=("server", "bot"), timeout=420,
            notes="PvP from the server's file with a kill limit: a bot on each side, the scores, the end by the kills",
        ),
        Scenario(
            name="net-server-delay", lane="net", args=[], driver=drive_delay, uses=("server", "bot"), timeout=240,
            notes="`observer-delay 10` in PvP: an observer bot watches two bots fight 10 seconds behind",
        ),
        Scenario(
            name="net-server-revive", lane="net", args=[], driver=drive_revive, uses=("server", "bot"), timeout=200,
            notes="retail's revival: a bot ejects, flies again in a new plane of its wing (slice F2-V) and leaves",
        ),
        Scenario(
            name="net-server-replies", lane="net", args=[], driver=drive_replies, uses=("server", "bot"), timeout=300,
            notes="a lead bot orders its wing and a wingman bot replies: the order is a radio call for the human "
            "wingman, the reply reaches the lead, and a lead's own reply is refused (slice F2-R)",
        ),
        Scenario(
            name="net-server-datalink", lane="net", args=[], driver=drive_datalink, uses=("server", "bot"),
            timeout=300,
            notes="a lead bot sorts its wing and the wingman bot, a human, is given a bandit by data link: the Link "
            "event, its readout's assignment and the call reach it over the wire (slice G7)",
        ),
        Scenario(
            name="net-server-datalink-lead", lane="net", args=[], driver=drive_datalink_lead,
            uses=("server", "bot"), timeout=200,
            notes="a bot flies as Two under an AI lead, weapons free: the lead's share or sort deals the bot a bandit "
            "like its AI wingmen, and it gets the Link event, its readout's assignment and the call (slices G4, G7 and G11)",
        ),
        Scenario(
            name="net-server-away", lane="net", args=[], driver=drive_away, uses=("server", "bot"), timeout=200,
            notes="a bot's game is away: the AI flies its plane, kept for it, until it is back and flies on in it "
            "(slice F2-A)",
        ),
        Scenario(
            name="net-server-rejoin", lane="net", args=[], driver=drive_rejoin, uses=("server", "bot"), timeout=300,
            notes="a bot killed in flight is dropped, its plane kept for it; started again with its token file it is "
            "back in that plane (slice K5)",
        ),
        Scenario(
            name="net-migrate-kill", lane="net", args=[], driver=drive_migrate_kill, uses=("bot",), timeout=420,
            notes="a hosting bot and three pilots that stand by fly the guide's mission; the host is killed (SIGKILL) in the "
            "fight once a kill is booked: the first standby takes the game over, every pilot flies on within 5 seconds, the "
            "world carries on with the kills, and the Results keep them (slice K9)",
        ),
        Scenario(
            name="net-migrate-handover", lane="net", args=[], driver=drive_migrate_handover, uses=("bot",), timeout=420,
            notes="the hosting bot leaves on purpose and hands the game over: the first standby hosts at once, the other "
            "pilots follow within about a second, the old host exits 0 (slice K9)",
        ),
        Scenario(
            name="net-migrate-relay", lane="net", args=[], driver=drive_migrate_relay, uses=("bot",), timeout=480,
            notes="a hosting bot listed on a master on this machine, two direct pilots that stand by and one relayed bot; "
            "the host is killed: the first standby resumes the listing, the master moves the relay channel, and the relayed "
            "bot flies on with the new host (slices K8 and K9)",
        ),
        Scenario(
            name="net-reach-upload", lane="net", args=[], driver=drive_reach_upload, uses=("bot",), timeout=420,
            notes="host selection on loopback: the host's reach and upload tests pass the bot that may host, which is "
            "appointed a warm first standby with equal checks; the bot that may not is never appointed (slices K3 and K6, K9)",
        ),
        Scenario(
            name="net-convert-capture", lane="net", args=[], driver=drive_convert, uses=("server", "bot"), timeout=360,
            notes="a bot's capture of a flight at 40,000 ft converts into a replay: read back, with the contrails, motor smoke and "
            "gun rounds the game makes again, the same bytes twice, a cut copy",
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
        Scenario(
            name="net-master-relay", lane="net", args=[], driver=drive_master_relay, uses=("server", "bot"),
            timeout=180,
            notes="tore-bot --path relay joins a listed tore-server through the relay of a tore-master on this "
            "machine, flies 30 seconds with no drop, and the master counts the channel and its bytes (slice J3)",
        ),
        Scenario(
            name="net-content-builds", lane="net", args=[], driver=drive_content_builds, uses=("server", "bot"),
            timeout=900,
            notes="a server on the profile's 1.02F import and a bot on a 1.0 import of gameassets' disc1 made in the "
            "run: every content item is the same, the bot says nothing about the build and flies (slices L3, L5)",
        ),
        Scenario(
            name="net-content-missing", lane="net", args=[], driver=drive_content_missing, uses=("server", "bot"),
            timeout=240,
            notes="a bot whose import lacks SU27.PT (tore-bot --drop-resource) joins the guide's mission: the gap, its "
            "unable words, another bot flies, the server's content and gaps lines (slice L3)",
        ),
        Scenario(
            name="net-window-internet-relay", lane="net", args=[], driver=drive_internet_relay, uses=("server",),
            window=True, timeout=360,
            notes="the Internet Lobby screen joins a listed tore-server through the relay of a master on this machine "
            "(TORE_JOIN_PATH=relay): the steps in Messages, a plane, a few seconds of flight, the path in the net log "
            "(slice J5)",
        ),
    ]
