"""Lane: net. The flight data link's cues in the real window, in a multiplayer flight (stage G, slice G10).

One windowed scenario, `net-window-datalink-cues`: a `tore-server` flies the guide's mission with its AI on weapons
free, and the game joins it in plane 1, number Two behind the AI lead of the first friendly wing (slice G11). The
lead assigns the human a bandit as it assigns its AI wingmen (a share or a sort), so the game's cockpit shows the
cues of an assignment to the player: the lead's call on the HUD, the diamond on the radar scope (blinking until the
player locks the bandit), the target window's `ASSIGNED BY LEAD` tag and the HUD's four corner brackets. The script
takes pictures of each; the checks read the game's own network capture, converted to a log, for the assignment
event and the call. The pictures are for a person to look at (the lead's part of acceptance G10).

The scenario takes its own port, so it runs beside the others; it opens one window (`--windows`).
"""
from __future__ import annotations

import json
import re

from battery import Drive, DriveError, Scenario

from battery_scenarios.net import GAME_FLAGS, LOCALHOST, NET_BAD, guide_mission, log_must, server_log, start_server

# The script: join and fly. The lead's call comes at the start of the flight, so the first picture shows its line on
# the HUD. The scope's range goes up one step (`comma`, 10 to 25 nm) so that the bandit, 20 nm out, is on it; the diamond blinks
# (lit while the tick is in the first half of a second: 120 ticks a cycle), so there is a picture of each phase.
# T designates the nearest bandit for the target window, then the lock turns the diamond steady and takes the HUD's
# brackets away.
SCRIPT = """wait 1
waittick 20 90
shot SHOTS/link-1-call.ppm
key comma
waittick 630 90
shot SHOTS/link-2-scope-lit.ppm
waittick 690 60
shot SHOTS/link-3-scope-dark.ppm
key t
waittick 750 60
shot SHOTS/link-4-target-window.ppm
waittick 810 60
shot SHOTS/link-5-target-window-dark.ppm
waittick 1500 90
shot SHOTS/link-6-later.ppm
exit
"""

PICTURES = (
    "link-1-call", "link-2-scope-lit", "link-3-scope-dark", "link-4-target-window", "link-5-target-window-dark",
    "link-6-later",
)


def capture_problems(events: list[dict], plane: int = 1, lead: int = 0) -> list[str]:
    """What the game's network capture, as the debug log reads it, must hold for an AI lead's assignment of a human
    (pure, unit tested in tools/test_battery_net.py): a `datalink.assign` from the lead to the plane, and the lead's
    call to Two as a radio line."""
    problems = []
    assigns = [e for e in events if e.get("kind") == "datalink.assign" and e.get("subject") == lead and e.get("object") == plane]
    if not assigns:
        problems.append(f"no datalink.assign from plane {lead} to plane {plane} in the capture")
    calls = [e for e in events if str(e.get("kind", "")).startswith("comms.") and re.match(r"Two, attack bandit", e.get("text") or "")]
    if not calls:
        problems.append("no call to Two (Two, attack bandit, ...) in the capture")
    return problems


def drive_cues(d: Drive) -> None:
    """The game flies as Two behind an AI lead and is dealt a bandit: the pictures of the cues, the capture's events."""
    port = d.port()
    server = start_server(d, port, guide_mission())
    shots = d.work / "shots"
    shots.mkdir(exist_ok=True)
    script = d.work / "cues.txt"
    script.write_text(SCRIPT.replace("SHOTS", str(shots)))
    game = d.start(
        "game",
        [d.app, "--connect", f"{LOCALHOST}:{port}", "--callsign", "Wing", "--slot", "1", *GAME_FLAGS, "--input-script", script],
        window=True,
    )
    game.finish(300, 0)
    server.send("quit")
    server.finish(30, None)
    for name in PICTURES:
        if not (shots / f"{name}.ppm").exists():
            d.problem(f"the script's {name}.ppm was not written")
    captures = [p for p in (d.data / "replays").glob("*_NET_*.tore-capture") if p.stat().st_size > 0]
    if not captures:
        d.problem("the game wrote no network capture in replays/")
    else:
        replay = d.work / "cues.tore-replay"
        d.run("convert", [d.app, "--convert-capture", captures[0], "--out", replay], timeout=180)
        out = d.work / "capture-log"
        d.run("log", [d.app, "--recording-log", replay, "--out", out, "--rate", "0.1"], timeout=120)
        log = out / "log.jsonl"
        if not log.exists():
            d.problem("the capture could not be turned into a replay and a log")
        else:
            events = [json.loads(line) for line in log.read_text().splitlines()]
            for problem in capture_problems([e for e in events if e.get("type") == "event"]):
                d.problem(problem)
    game.forbid(NET_BAD, "a network problem")
    log_must(d, server_log(d), r"Wing took plane 1\b", forbid=NET_BAD)


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="net-window-datalink-cues", lane="net", args=[], driver=drive_cues, uses=("server",), window=True,
            timeout=400,
            notes="the game flies as Two behind an AI lead on a server and is dealt a bandit (slice G11): pictures of "
            "the lead's call, the radar diamond lit and dark, the target window's ASSIGNED BY LEAD and the HUD brackets, "
            "and the capture holds the assignment and the call (slice G10)",
        ),
    ]
