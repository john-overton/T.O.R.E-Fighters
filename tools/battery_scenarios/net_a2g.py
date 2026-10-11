"""Lane: net. The air-to-ground round online (protocol 23, slice NA), over real UDP on 127.0.0.1.

`net-a2g-loopback`: a `tore-server` flies the Vladivostok radar site `~QVRDRI` (seed 4, heavy defenses, the
enemy 5 nm from Blue at 10,000 ft, Easy targeting and invulnerable players, every AI on weapons hold) with a
King who waits for two players holding slots. Two bots fly it:

- `Laser`, the F-16C (its Pave Spike pod), turns the A-G radar on, designates the nearest enemy ground object
  with Ctrl+X, which its sight then tracks, and lases it by hand (Ctrl+L) while circling it (`tore-bot --a2g
  lase`).
- `Buddy`, an F/A-18D with no designator, hangs a GBU-10 and an AGM-88 on its stations in a Cheat loading
  (`--load`, the server's `loadouts any`), follows the Laser bot until it sees its spot, releases the bomb when
  its guided sight says IN RNG on that buddy spot, follows the bomb to its end, then fires the HARM in BORE at
  the cue's candidate (`--a2g buddy`).

Both screens must agree: the spot the Buddy reads as the buddy's is the Laser bot's own (owner plane 0, the
same target, the same point); the Buddy releases on it and the bomb guides (its time to impact counts down)
and ends; the Laser bot draws the Buddy's GBU-10 and its AGM-88, the missile flying at the candidate the
Buddy's HARM cue chose. See docs/testing/lane-net.md.
"""
from __future__ import annotations

import re

from battery import Drive, DriveError, Scenario
from battery_scenarios.net import NET_BAD, server_log, start_bots, start_server

MISSION = """tore-mission 1
theater VLA
condition clear
start airborne 10000
separation-nm 5
preset hold
guns-only no
wing friendly 1 F16C.PT 1 experienced
wing friendly 2 F18.PT 1 experienced
wing enemy 1 MIG29.PT 1 dummy
ground-target QVRDRI
defenses aaa heavy sam heavy
surface-seed 4
cheats easy-targeting damage=invulnerable
"""

SPOT = re.compile(
    r"^(?P<name>\w+): a2g: (?:laser Lasing spot|buddy spot) owner (?P<owner>\d+) target (?P<target>0x[0-9a-f]+) "
    r"at (?P<x>-?\d+) (?P<y>-?\d+) (?P<z>-?\d+) range \d+$",
    re.M,
)
RELEASE = re.compile(
    r"^Buddy: a2g: release GBU10\.JT buddy yes reach \S+ range \S+ spot owner (?P<owner>\d+) "
    r"target (?P<target>0x[0-9a-f]+) at",
    re.M,
)
WEAPON = re.compile(r"^Buddy: a2g: weapon \d+ Laser guiding (?P<guiding>yes|no) tti (?P<tti>\S+)$", re.M)
FIRE = re.compile(r"^Buddy: a2g: fire AGM88\.JT at (?P<target>0x[0-9a-f]+)$", re.M)
SAW = re.compile(
    r"^Laser: a2g: saw (?P<weapon>\S+) \d+ from plane (?P<owner>\d+) target (?P<target>\S+)$", re.M
)


def spots(text: str, name: str) -> list[dict[str, str]]:
    return [m.groupdict() for m in SPOT.finditer(text) if m.group("name") == name]


def a2g_problems(laser: str, buddy: str) -> list[str]:
    """What the two bots printed against the scenario's rules."""
    problems: list[str] = []
    own = spots(laser, "Laser")
    seen = spots(buddy, "Buddy")
    if not own:
        problems.append("the Laser bot never lased (no `laser Lasing spot` line)")
    if not seen:
        problems.append("the Buddy never read a buddy spot")
    if own and seen:
        lased = {(s["owner"], s["target"]): (int(s["x"]), int(s["y"]), int(s["z"])) for s in own}
        for s in seen:
            key = (s["owner"], s["target"])
            if s["owner"] != "0":
                problems.append(f"the Buddy's spot belongs to plane {s['owner']}, not the Laser bot's plane 0")
                break
            if key not in lased:
                problems.append(f"the Buddy's spot on {s['target']} is not one the Laser bot lased")
                break
            point = (int(s["x"]), int(s["y"]), int(s["z"]))
            gap = max(abs(a - b) for a, b in zip(point, lased[key]))
            if gap > 100:
                problems.append(f"the two screens put the spot on {s['target']} {gap} ft apart")
                break
    release = RELEASE.search(buddy)
    if release is None:
        problems.append("the Buddy released no GBU-10 on a buddy spot")
    elif release.group("owner") != "0":
        problems.append(f"the Buddy released on plane {release.group('owner')}'s spot")
    tti = [float(m.group("tti")) for m in WEAPON.finditer(buddy) if m.group("guiding") == "yes" and m.group("tti") != "none"]
    if not tti:
        problems.append("the GBU-10 never guided (no `guiding yes` weapon line)")
    elif any(b >= a for a, b in zip(tti, tti[1:])):
        problems.append(f"the GBU-10's time to impact did not count down: {tti}")
    if not re.search(r"^Buddy: a2g: bomb ended at tick \d+$", buddy, re.M):
        problems.append("the Buddy never saw its bomb end")
    fire = FIRE.search(buddy)
    if fire is None:
        problems.append("the Buddy fired no HARM in BORE")
    saw = [m.groupdict() for m in SAW.finditer(laser) if m.group("owner") == "1"]
    if not any(s["weapon"] == "GBU10.JT" for s in saw):
        problems.append("the Laser bot never drew the Buddy's GBU-10")
    harms = [s for s in saw if s["weapon"] == "AGM88.JT"]
    if fire is not None:
        if not harms:
            problems.append("the Laser bot never drew the Buddy's AGM-88")
        elif not any(s["target"] == fire.group("target") for s in harms):
            problems.append(
                f"the Laser bot draws the AGM-88 at {harms[0]['target']}, the Buddy fired at {fire.group('target')}"
            )
    return problems


def drive_loopback(d: Drive) -> None:
    """A laser bot and a buddy bomber over the radar site; their screens agree on the spot, the bomb and the HARM."""
    port = d.port()
    server = start_server(d, port, MISSION, king="first-player", loadouts="any")
    laser = start_bots(
        d, port, "laser", 110, "--callsign", "Laser", "--slot", "0", "--a2g", "lase", "--king", "loadouts=any",
        "--players", "2",
    )
    # The Laser bot wears the crown: the King's start waits for both bots.
    if not server.wait_for(r"Laser wears the crown", 60):
        raise DriveError("the Laser bot never wore the crown")
    buddy = start_bots(
        d, port, "buddy", 110, "--callsign", "Buddy", "--slot", "1", "--a2g", "buddy", "--load", "GBU10.JT,AGM88.JT",
    )
    laser.finish(220, 0)
    buddy.finish(220, 0)
    server.finish(40, 0)
    laser.expect(r"^Laser: seat \d+, plane 0, at tick \d+$", "the Laser bot flies plane 0")
    buddy.expect(r"^Buddy: seat \d+, plane 1, at tick \d+$", "the Buddy flies plane 1")
    laser.expect(r"^Laser: a2g: tracking 0x[0-9a-f]+, laser on$", "the sight tracking the designation")
    for bot, name in ((laser, "Laser"), (buddy, "Buddy")):
        bot.expect(rf"^{name}: The connection ended: the player left\.$", "a clean leave")
        bot.forbid(NET_BAD, "a network problem")
        bot.forbid(rf"^{name}: line: cannot hang", "a loadout the bot could not build")
    for problem in a2g_problems(laser.text(), buddy.text()):
        d.problem(problem)
    server.forbid(NET_BAD, "a network problem")
    log = server_log(d)
    if not re.search(r"joined as Laser", log) or not re.search(r"joined as Buddy", log):
        d.problem("the server log does not show both bots joining")


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="net-a2g-loopback", lane="net", args=[], driver=drive_loopback, uses=("server", "bot"),
            timeout=360,
        ),
    ]
