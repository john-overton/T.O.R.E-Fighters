"""Dedicated feedback form through real input, capture conversion and viewer."""
from __future__ import annotations

import json
import re
import time
from battery import Drive, Scenario
from battery_scenarios.net import (
    GAME_FLAGS, LOCALHOST, fresh_data, server_log, start_server, stop_server, write_server_files,
)


def drive_feedback(d: Drive, enabled: bool) -> None:
    port = d.port()
    folder = d.work / "server-replays"
    server = start_server(d, port, record_replays="on", feedback="on" if enabled else "off",
                          replays_kept=0 if enabled else 1, replays_folder=folder,
                          after_end="restart", empty_timeout=60)
    script = d.work / "feedback.txt"
    steps = ["waittick 360 90", "key Escape", "wait 0.3", "snapshot escape.ppm"]
    if enabled:
        steps += ["key Up", "key Enter", "wait 0.3", "snapshot form-empty.ppm",
                  "key x", "key Escape", "key Enter", "wait 0.3"]
        text = "runway bounce"
        steps += [f"key {c if c != ' ' else 'Space'}" for c in text]
        steps += ["wait 2", "snapshot form-filled.ppm", "key Tab", "key Enter", "wait 2"]
    steps += ["key Escape", "waittick 1500 90", "key Ctrl+q", "wait 180", "exit"]
    script.write_text("\n".join(steps) + "\n")
    game = d.start("game", [d.app, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Viper",
                   *GAME_FLAGS, "--input-script", script], window=True)
    if not server.wait_for(r"left: left", 120):
        d.problem("the script did not leave through End Mission")
    stop_server(d, server)
    # Conversion is deliberately asynchronous. Keep the menu process alive
    # until it finishes, rather than killing its writer after a fixed wait.
    deadline = time.monotonic() + 120 * d.scale
    while game.alive() and not list((d.data / "replays").glob("*.tore-replay")) and time.monotonic() < deadline:
        d.sleep(0.2)
    game.stop()

    recordings = list(folder.glob("*.tore-replay"))
    if "Recording failed:" in server_log(d):
        d.problem("server recording failed during the mission")
    if len(recordings) != 1:
        d.problem(f"expected one server recording, got {recordings}")
        return
    replay = recordings[0]
    d.run("export-server", [d.app, "--recording-log", replay, "--out", d.work / "server-export", "--rate", "1"], timeout=90)
    flags = [json.loads(line) for line in (d.work / "server-export/feedback.jsonl").read_text().splitlines()]
    if enabled:
        if len(flags) != 1 or flags[0]["text"] != "runway bounce":
            d.problem(f"expected one submitted report and no cancelled report: {flags}")
        elif flags[0]["tick"] >= flags[0]["submit_tick"]:
            d.problem("multiplayer stopped while the feedback form was open")
        if "Feedback from Viper at" not in server_log(d):
            d.problem("feedback did not reach the server log")
        local = list((d.data / "replays").glob("*.tore-replay"))
        if len(local) != 1:
            d.problem(f"expected one local recording: {local}")
        else:
            d.run("export-local", [d.app, "--recording-log", local[0], "--out", d.work / "local-export", "--rate", "1"], timeout=90)
            local_flags = [json.loads(line) for line in (d.work / "local-export/feedback.jsonl").read_text().splitlines()]
            if [{k: v for k, v in f.items() if k != "file"} for f in local_flags] != [{k: v for k, v in f.items() if k != "file"} for f in flags]:
                d.problem("local and server feedback differ")
    elif flags:
        d.problem("disabled server acquired feedback")
    if not replay.with_suffix(".log").exists():
        d.problem("server recording has no companion log")
    d.log(f"Recording size: {replay.stat().st_size} bytes; mission log: {replay.with_suffix('.log').stat().st_size} bytes")
    view = d.work / "viewer.txt"
    view_steps = ["wait 4", "shot viewer-before.ppm"]
    if enabled:
        view_steps += ["movemenu 550 14", "click", "wait 0.5", "movemenu 100 50", "click", "wait 0.5", "shot viewer-feedback.ppm", "key Escape"]
    view_steps += ["exit"]
    view.write_text("\n".join(view_steps) + "\n")
    d.run("viewer", [d.app, "--watch-replay", replay, *GAME_FLAGS, "--input-script", view], timeout=90, window=True)


def drive_recorder_lifecycle(d: Drive) -> None:
    """Headless recording across a restart and quit, including the last chunk."""
    fresh_data(d)
    folder = d.work / "server-replays"
    config = write_server_files(d, d.port(), start="now", record_replays="on",
                                replays_folder=folder, replays_kept=10,
                                after_end="restart", empty_timeout=120,
                                status_interval=1, broadcast="off", telemetry="off")
    server = d.start("server", [d.server, "--config", config], stdin=True)
    if not server.wait_for(r"tick [3-9][0-9]{2} .*worst [0-9.]+ ms", 60):
        d.problem("the first recording did not reach its third chunk with worst-tick status")
    server.send("restart")
    if not server.wait_for(r"Replay saved:", 30):
        d.problem("restart did not finish the first recording")
    d.sleep(3)
    server.send("status")
    stop_server(d, server)
    if "Recording failed:" in server_log(d):
        d.problem("recording failed during the headless lifecycle")
    paths = sorted(folder.glob("*.tore-replay"))
    if len(paths) != 2 or list(folder.glob("*.partial")):
        d.problem(f"expected two finished recordings and no pending writers, got {paths}")
    for n, path in enumerate(paths):
        info = d.run(f"info-{n}", [d.app, "--recording-info", path], timeout=60)
        info.expect(r"^State +finished normally$", "a completed recording")
        if not path.with_suffix(".log").exists():
            d.problem("a finished replay lost its companion log")
            continue
        log = path.with_suffix(".log").read_text()
        if not re.search(r"MissionEnded|Stopped", log):
            d.problem("the final pending frame omitted its lifecycle event")


def scenarios() -> list[Scenario]:
    return [Scenario(name=f"net-replay-feedback-{name}", lane="net", args=[], window=True,
                     uses=("server",), timeout=360, env={"TORE_RECORD_MISSIONS": "1", "TORE_SCRIPT_OUT": "{work}"},
                     driver=lambda d, enabled=enabled: drive_feedback(d, enabled),
                     notes="Real dedicated feedback form, local and server replay exports, retention and viewer.")
            for name, enabled in [("on", True), ("off", False)]] + [
                Scenario(name="net-replay-feedback-recorder-lifecycle", lane="net", args=[],
                         uses=("server",), timeout=180, driver=drive_recorder_lifecycle,
                         notes="Headless dedicated recorder flush, restart, quit, companion logs and worst tick.")
            ]
