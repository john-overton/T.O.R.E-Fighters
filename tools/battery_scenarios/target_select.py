"""Human air targeting through the real window handlers, locally and online."""
from __future__ import annotations

import re

from battery import Drive, Scenario
from battery_scenarios.net import GAME_FLAGS, LOCALHOST, dummy_enemies, guide_mission, start_server, stop_server, weapons_hold

SCRIPT = """wait 1
waittick 120 120
shot SHOTS/before.ppm
key t
wait 0.5
shot SHOTS/radar-first.ppm
key t
wait 0.5
shot SHOTS/radar-next.ppm
key Shift+t
wait 0.5
shot SHOTS/radar-previous.ppm
key r
key semicolon
wait 0.5
key t
wait 0.5
shot SHOTS/radar-off.ppm
VISUAL_WAIT
key Enter
wait 0.5
shot SHOTS/visual-first.ppm
key Enter
wait 0.5
shot SHOTS/visual-next.ppm
key quote
wait 0.5
shot SHOTS/visual-centre.ppm
key Enter
wait 0.5
shot SHOTS/visual-retained.ppm
CHAT
exit
"""
PICTURES = ("before", "radar-first", "radar-next", "radar-previous", "radar-off",
            "visual-first", "visual-next", "visual-centre", "visual-retained")


def selection_problems(text: str) -> list[str]:
    """Read each next key's authoritative before-state, not just its request."""
    problems = []
    radar = re.findall(r"Target selection: radar before=(None|Some\(\d+\)) requested=(None|Some\(DesignateTarget\(\d+\)\))", text)
    visual = re.findall(r"Target selection: visual centre=(false|true) before=(None|Some\(\d+\)) requested=(None|Some\(\d+\))", text)
    if len(radar) != 4:
        problems.append(f"expected four radar presses, got {radar}")
    else:
        for i in range(3):
            if radar[i][1] == "None":
                problems.append(f"radar press {i} selected nothing")
        if radar[3][1] != "None":
            problems.append("T selected a contact with radar off")
        accepted = 0
        for i in (1, 2):
            expected = radar[i - 1][1].replace("DesignateTarget(", "")[:-1]
            if radar[i][0] == expected:
                accepted += 1
            elif radar[i][0] != "None":
                problems.append(f"radar selection changed to an unrequested target: {radar}")
            # A nearby friendly may leave the scan while the capture blocks
            # the client. Radar selection is supposed to drop in that case.
        if not accepted:
            problems.append(f"no radar selection survived to the next key: {radar}")
    if len(visual) != 4 or [v[0] for v in visual] != ["false", "false", "true", "false"]:
        problems.append(f"expected Enter, Enter, Apostrophe, Enter, got {visual}")
    else:
        for i, (_, before, picked) in enumerate(visual):
            if picked == "None":
                problems.append(f"visual press {i} selected nothing with radar off")
            if i and before != visual[i - 1][2]:
                problems.append(f"visual selection {i - 1} did not survive the host ticks: {visual}")
        if visual[0][2] == visual[1][2]:
            problems.append("Enter did not advance between visible aircraft")
    return problems


def fly(d: Drive, args: list[str], chat: bool = False) -> None:
    shots = d.work / "shots"
    shots.mkdir(exist_ok=True)
    script = d.work / "target-select.txt"
    # A GPU capture can block this process for several seconds. Let the
    # client receive fresh cockpit lists after each shot before pressing the
    # next key, rather than testing the network's cold-baseline recovery.
    steps = "\n".join(line + ("\nwait 0.5" if line.startswith("shot ") else "")
                      for line in SCRIPT.splitlines()) + "\n"
    script.write_text(steps.replace("SHOTS", str(shots))
                      .replace("VISUAL_WAIT", "waittick 6000 120" if chat else "")
                      .replace("CHAT", "key grave\nkey Enter\nwait 0.5" if chat else ""))
    game = d.start("game", [d.app, *args, *GAME_FLAGS, "--window-size", "960x720", "--scope-range", "25" if chat else "10",
                            "--input-script", script], window=True)
    game.finish(240, 0)
    text = "\n".join(p.read_text(errors="replace") for p in (d.data / "logs").glob("tore-*.log"))
    for problem in selection_problems(text):
        d.problem(problem)
    for name in PICTURES:
        if not (shots / f"{name}.ppm").exists():
            d.problem(f"missing capture {name}")


def single_player(d: Drive) -> None:
    # Scripted CLI launches bypass saved/random creator choices: this is
    # the deterministic F/A-18D four-against-four Quick Mission fixture.
    fly(d, ["--launch-quick-mission", "--aircraft", "f18", "--separation", "5",
            "--probe-fight", "4:4", "--ai-mission", "hold"])


def multiplayer(d: Drive) -> None:
    port = d.port()
    # Unlike single player, the host keeps flying during the window's loading
    # and captures. Start further apart, use straight-flight enemy fixtures,
    # and wait for the host's 50-second tick before the visual picks.
    server = start_server(d, port, dummy_enemies(weapons_hold(guide_mission(20))))
    bot = d.start("bot", [d.bot, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Friend",
                          "--slot", "2", "--seconds", "120"])
    fly(d, ["--connect", f"{LOCALHOST}:{port}", "--callsign", "TargetCheck", "--slot", "1"], chat=True)
    stop_server(d, server)
    bot.finish(30, None)


def scenarios() -> list[Scenario]:
    return [Scenario(name="flight-sensor-target-select", lane="flight", window=True, args=[], driver=single_player, timeout=300),
            Scenario(name="net-window-target-select", lane="net", window=True, args=[], driver=multiplayer, timeout=330)]
