"""The local report across revival and early departure, over real UDP."""
from __future__ import annotations

import re
from battery import Drive, Scenario
from battery_scenarios.net import GAME_FLAGS, NET_BAD, dummy_enemies, guide_mission, server_log, start_bots, start_server
from battery_scenarios.net_screens import game_log


def drive_partial(d: Drive) -> None:
    port = d.port()
    mission = dummy_enemies(guide_mission(separation_nm=10))
    mission = re.sub(r"(?m)^wing friendly 1 .*$", "wing friendly 1 F18.PT 1 experienced", mission)
    mission = "".join(line for line in mission.splitlines(keepends=True)
                      if not line.startswith(("wing friendly 2", "wing-choice", "survive", "objective")))
    server = start_server(d, port, mission, respawn="revive", revive_distance=10,
                          lives="unlimited", time_limit=5, empty_timeout=120, min_players=1, open_planes="all")
    witness = start_bots(d, port, "witness", 240, "--callsign", "Witness", "--observe", "0")
    shots = d.work / "shots"
    shots.mkdir(exist_ok=True)
    script = d.work / "debrief.txt"
    fire = "wait 3\nkey ,\nkey ,\nwait 2\nkey t\nwait 2\nkey ]\nwait 2\n" + "key Space\nwait 1\n" * 4
    pages = "".join(f"shot {shots}/{i:02d}-report.ppm\nkey ArrowRight\nwait 0.2\n" for i in range(1, 11))
    script.write_text(
        "waittick 240 120\n" + fire + "wait 20\nkey k\n"
        + f"shot {shots}/00-first-score.ppm\nkey k\n"
        + "key Shift+e\nwait 0.4\nkey Shift+e\nwait 3\nkey Escape\nwait 0.2\nkey Enter\nwait 12\n"
        + fire + "wait 20\nkey k\n"
        + f"shot {shots}/00-second-score.ppm\nkey k\nkey Ctrl+q\nwait 4\n"
        + pages + "key End\nkey ArrowLeft\nkey ArrowLeft\n"
        + f"shot {shots}/11-kills.ppm\nexit\n"
    )
    game = d.start("game", [d.app, "--connect", f"127.0.0.1:{port}", "--callsign", "Viper", "--slot", "0",
                            *GAME_FLAGS, "--original-graphics", "--window-size", "800x600", "--input-script", script], window=True)
    game.finish(210, 0)
    # The remaining player sees the departed contribution and the AI rows.
    d.sleep(2)
    server.send("end")
    witness.finish(35, None)
    server.finish(35, 0)
    log = game_log(d)
    partial = re.search(r"Network: partial report: (\d+) kills, (\d+) sorties, tick (\d+)", log)
    if not partial:
        d.problem("the early departure did not display a local partial report")
    elif int(partial[1]) < 2:
        d.problem(f"expected kills across the lives, got {partial[1]}")
    elif int(partial[2]) != 2:
        d.problem(f"expected two sorties, got {partial[2]}")
    if not re.search(r"Viper \(left\)", witness.text()):
        d.problem("the remaining player did not retain Viper marked left")
    if "(AI)" not in witness.text():
        d.problem("the live scores did not list AI aircraft")
    seats = re.findall(r"Network: seated in plane (\d+)", log)
    if len(seats) == 2 and not re.search(
        rf"spawned plane {seats[1]} in Friendly wing 1, member 1\b", witness.text()
    ):
        d.problem("the revived human did not retain flight position 1")
    history = server_log(d)
    lost = set(re.findall(r"((?:Red|Blue) \d+-\d+) lost plane \d+: the AI respawns", history))
    for label in lost:
        if not re.search(rf"{re.escape(label)} respawned in plane", history):
            d.problem(f"the respawned AI did not retain {label}")
    for number in range(1, 11):
        if not (shots / f"{number:02d}-report.ppm").exists():
            d.problem(f"missing report capture {number}")
    game.forbid(NET_BAD, "a network problem")
    server.forbid(NET_BAD, "a network problem")
    # The complete flight is retained for visual review and the report.
    (d.work / "session-summary.txt").write_text(log + "\n" + server_log(d) + "\n" + witness.text())


def scenarios() -> list[Scenario]:
    return [Scenario(name="net-window-partial-debrief", lane="net", args=[], driver=drive_partial,
                     uses=("server", "bot"), window=True, timeout=300,
                     notes="Fly, eject, revive and leave; retain local sorties and AI/human scores, with clipboard captures.")]
