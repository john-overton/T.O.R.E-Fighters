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
Buddy's HARM cue chose.

`net-ai-strike-handback` (slice ST-K, docs/spec/ai-strike.md "Takeover and handback"): a `tore-server` flies the
Ukraine column `~QUCOL` (no added defenses, Easy targeting) with two F-16Cs and no enemy aircraft (a hostile
fighter within 15 nm makes the takeover unsafe, so the AI would stand off instead). `Strike` (plane 0, Mk 82s in a
Cheat loading, a token file) sets up a level pass (`tore-bot --a2g strike`: A-G page, Ctrl+X, the Mk 82s, LVL)
and is killed in flight before it releases; the server keeps plane 0 for it and the AI flies it. `Stay` (plane
1, `--a2g watch`) keeps the mission going and sees the Mk 82s leave plane 0: the AI continued the pass. Strike's
game then joins again with its token (`--a2g watch`) and finds its cockpit as it left it: the same station, LVL,
fewer bombs, and the designation on the same target while it stands.

`net-a2g-stores` (protocol 27, slice BP3, docs/spec/stores-on-pylons.md): the same column with two enemy MiG-29s
free to fire and invulnerable players. `Bomber` (plane 0, Mk 82s in a Cheat loading, `tore-bot --a2g bomb`) drops
a Mk 82 in CCIP every four seconds; `Watch` (plane 1, `--a2g watch`) flies on. Each bot prints its `stores:` line:
Watch must draw the other aircraft's stores and see plane 0's Mk 82 station and an AI aircraft's missile station
empty as they release; Bomber must see an AI aircraft's stores empty too. See docs/testing/lane-net.md.
"""
from __future__ import annotations

import re

from battery import Drive, DriveError, Scenario
from battery_scenarios.net import (
    LOCALHOST,
    NET_BAD,
    log_must,
    rejoin_problems,
    server_log,
    start_bots,
    start_server,
)

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


STRIKE_MISSION = """tore-mission 1
theater UKR
condition clear
start airborne 5000
separation-nm 5
preset free
guns-only no
wing friendly 1 F16C.PT 2 average
ground-target QUCOL
surface-seed 7
cheats easy-targeting
"""

STRIKE_SET = re.compile(
    r"^Strike: a2g: strike set target (?P<target>0x[0-9a-f]+) station (?P<station>\d+) mode LVL "
    r"laser \S+ rounds (?P<rounds>\d+)$",
    re.M,
)
COCKPIT = re.compile(
    r"^Strike: a2g: cockpit designated (?P<designated>\S+) station (?P<station>\d+) mode (?P<mode>\S+) "
    r"laser \S+ rounds (?P<rounds>\d+)$",
    re.M,
)


def handback_problems(
    strike: str, watch: str, back: str, weapon: str = "MK82.JT", same_station: bool = True
) -> list[str]:
    """What the three bots printed against `net-ai-strike-handback`'s rules: the pass set up, the AI's release
    seen from plane 1, and the rejoined cockpit as left with fewer bombs."""
    problems: list[str] = []
    set_up = STRIKE_SET.search(strike)
    if set_up is None:
        return ["the Strike bot never set up its pass (no `strike set` line)"]
    if re.search(rf"^Stay: a2g: saw {re.escape(weapon)} \d+ from plane 0 ", watch, re.M) is None:
        problems.append(f"Stay never saw a {weapon} leave plane 0: the AI did not continue the pass")
    found = [m.groupdict() for m in COCKPIT.finditer(back)]
    if not found:
        return problems + ["the rejoined game never read its cockpit (no `cockpit` line)"]
    first = found[0]
    # A B-2's ripple hands the selection to its other bay after a release, and a bay holds 32: the
    # rejoined cockpit then reads the other station at full count (BMB-AI).
    if not same_station:
        return problems
    if first["station"] != set_up["station"]:
        problems.append(f"the rejoined cockpit has station {first['station']}, the human left {set_up['station']}")
    if first["mode"] != "LVL":
        problems.append(f"the rejoined cockpit is in {first['mode']}, the human left LVL")
    if not int(first["rounds"]) < int(set_up["rounds"]):
        problems.append(f"the rejoined cockpit has {first['rounds']} bombs, as many as the human left")
    if first["designated"] not in (set_up["target"], "none"):
        problems.append(
            f"the rejoined cockpit designates {first['designated']}, the AI attacked {set_up['target']}"
        )
    return problems


def drive_strike_handback(
    d: Drive, mission: str = STRIKE_MISSION, load: str = "MK82.JT", doors: bool = False
) -> None:
    """A bomber's game dropped mid-pass: the AI finishes the pass, the game rejoins and finds its cockpit. With
    `doors` (a bay aircraft) the other client also sees the AI's bay doors open for the release (BMB-AI)."""
    port = d.port()
    server = start_server(d, port, mission, king="first-player", loadouts="any")
    token_file = d.work / "strike.token"
    # Strike wears the crown and its start waits for both players, so its Cheat loading goes with its ready.
    strike = start_bots(
        d, port, "strike", 120, "--callsign", "Strike", "--slot", "0", "--a2g", "strike", "--load", load,
        "--token-file", str(token_file), "--king", "loadouts=any", "--players", "2",
    )
    if not server.wait_for(r"Strike wears the crown", 60):
        raise DriveError("the Strike bot never wore the crown")
    stay = start_bots(d, port, "stay", 330, "--callsign", "Stay", "--slot", "1", "--a2g", "watch")
    if not strike.wait_for(r"^Strike: a2g: strike set target", 90):
        raise DriveError("the Strike bot never set up its pass")
    # On a straight line at the target before the drop.
    d.sleep(8)
    if re.search(rf"^Stay: a2g: saw {re.escape(load)} \d+ from plane 0 ", stay.text(), re.M):
        d.problem(f"a {load} left plane 0 before the drop")
    d.log("killing Strike (SIGKILL)")
    strike.stopped = True
    strike.popen.kill()
    strike.wait(10)
    if not server.wait_for(r"Strike dropped out: the AI flies plane 0, kept for it$", 40):
        d.problem("the server did not keep plane 0 for the dropped Strike")
    # The pass the human set up, or the re-attack after it (the level run's cue does not always come
    # inside the gate on the first pass from the human's line).
    if not stay.wait_for(rf"^Stay: a2g: saw {re.escape(load)} \d+ from plane 0 ", 240):
        d.problem("the AI never released on the human's target")
    d.sleep(15)
    back = d.start(
        "strike2",
        [d.bot, "--connect", f"{LOCALHOST}:{port}", "--seconds", "15", "--callsign", "Strike", "--slot", "0",
         "--token-file", str(token_file), "--a2g", "watch"],
    )
    back.finish(90, 0)
    for problem in rejoin_problems(back.text(), "Strike", 0):
        d.problem(problem)
    back.expect(r"^Strike: The connection ended: the player left\.$", "a clean leave")
    stay.finish(330, 0)
    server.send("end")
    server.finish(40, 0)
    for bot in (stay, back):
        bot.forbid(NET_BAD, "a network problem")
    for problem in handback_problems(strike.text(), stay.text(), back.text(), load, same_station=not doors):
        d.problem(problem)
    if doors:
        for problem in doors_problems(stay.text(), "Stay", 0, "the AI bomber"):
            d.problem(problem)
    log_must(
        d,
        server_log(d),
        r"Strike dropped out: the AI flies plane 0, kept for it$",
        r"Strike rejoined with its token: plane 0 is waiting$",
        forbid=r"protocol error|bad packets|\bfault\b",
    )


STORES_MISSION = STRIKE_MISSION.replace(
    "wing friendly 1 F16C.PT 2 average\n", "wing friendly 1 F16C.PT 2 average\nwing enemy 1 MIG29.PT 2 average\n"
).replace("cheats easy-targeting\n", "cheats easy-targeting damage=invulnerable\n")

# B2-9: the same two-player stores mission with both players in B-2s (one wing of two), so each client
# must draw the other's bay stores (a gridded magazine: 32 Mk 82 a bay) and see them leave.
B2_STORES_MISSION = STORES_MISSION.replace("wing friendly 1 F16C.PT 2 average\n", "wing friendly 1 B2.PT 2 average\n")

# BMB-AI (2026-11-16): the same two-player stores mission with both players in F-117s, bomb-only jets
# whose bomb is a GBU-27 (`tore-bot --a2g bomb` takes the first bomb of its load, not the Mk 82 by
# name). Each client must see the other's bay doors open for the release.
F117_STORES_MISSION = STORES_MISSION.replace("wing friendly 1 F16C.PT 2 average\n", "wing friendly 1 F117.PT 2 average\n")
# B52-9 (2026-11-16): the same mission with both players in B-52s. `tore-bot --load` hangs each store on the first
# station after the others that takes it, two of each: `AGM84E.JT,MK82.JT` puts two Harpoons on the wing pylons
# (row 7, station 1) and two Mk 82 in the forward bay row (station 2), and `--a2g bomb` takes the first bomb, the
# Mk 82 (a Harpoon is not on the bomb equation). The watching client must draw the pylon Harpoons and the bay
# stores, and see a Mk 82 leave with the bay doors open.
B52_STORES_MISSION = STORES_MISSION.replace("wing friendly 1 F16C.PT 2 average\n", "wing friendly 1 B52.PT 2 average\n")
# The stores round with both players in B-1Bs (B1B-9): the Bomber's `--load MK82.JT` puts Mk 82 on the first bay row (an
# unguided store is the first bomb), the other bay rows keep the default seven-packs, and the watching client must draw
# the bay stores, see a Mk 82 leave and see the bay doors open for it.
B1B_STORES_MISSION = STORES_MISSION.replace("wing friendly 1 F16C.PT 2 average\n", "wing friendly 1 B1.PT 2 average\n")
# T160-9 (2026-11-16): the same mission with both players in Tu-160s. `tore-bot --load MK82.JT` hangs two Mk 82 in the
# forward bay row (station 0, the default load's row; the pylon-less Tu-160 has bay rows only), and `--a2g bomb` takes
# the Mk 82. The watching client must draw the bay stores (a gridded magazine) and see a Mk 82 leave with the bomber's
# bay doors open.
TU160_STORES_MISSION = STORES_MISSION.replace("wing friendly 1 F16C.PT 2 average\n", "wing friendly 1 TU160.PT 2 average\n")
# F111-9 (2026-11-16): the same mission with both players in F-111Fs. `tore-bot --load` hangs each store on the first
# station after the others that takes it, two of each, in the station order: the four wing rows (5 to 8), then the bay,
# then the rails. Four Harpoons (`AGM84E.JT`, a surface missile that is not on the bomb equation) fill the wing rows
# (the outer pair carrying them holds the wing at 26 degrees), so the Mk 82 after them takes the bay row, and
# `--a2g bomb` (the first bomb station with a round) takes it from there: the watching client must draw the Harpoons
# on the pylons, see the Mk 82 leave the bay and see the bay doors open for it.
F111_STORES_MISSION = STORES_MISSION.replace("wing friendly 1 F16C.PT 2 average\n", "wing friendly 1 F111.PT 2 average\n")
F111_LOAD = "AGM84E.JT,AGM84E.JT,AGM84E.JT,AGM84E.JT,MK82.JT"
# T26-9 (2026-11-16): the same mission with both players in Tu-26s. `tore-bot --load` hangs each store on the first
# station after the others that takes it, two of each: the Tu-26's stations run gun, intake pair (row 6), inner
# glove (7), outer glove (8), belly bay (5), and every pylon row takes the Mk 82, so `AS16.JT,MK82.JT` would hang
# the bomb on the inner glove with the doors shut. `AS16.JT,AS16.JT,AS16.JT,MK82.JT` fills the three pylon rows
# with AS-16 first and leaves the bay for the two Mk 82; `--a2g bomb` takes the first bomb, the Mk 82. The
# watching client must draw the pylon and bay stores, and see a Mk 82 leave the bay with the belly doors open.
TU26_STORES_MISSION = STORES_MISSION.replace("wing friendly 1 F16C.PT 2 average\n", "wing friendly 1 TU26.PT 2 average\n")
# The strike handback with a B-2 (the F-117 has no radar to set up a pass with), airborne at 20,000 ft so the heavy needs no climb to its release floor: the human's pass is
# continued by the AI, whose bay doors must open for the release and be seen to open from the other client.
B2_STRIKE_MISSION = STRIKE_MISSION.replace(
    "wing friendly 1 F16C.PT 2 average\n", "wing friendly 1 B2.PT 2 average\n"
).replace("start airborne 5000\n", "start airborne 20000\n")

DOORS_LINE = re.compile(r"^(?P<name>\w+): bay doors \[(?P<doors>[\d.= ]*)\]$", re.M)


def last_doors(text: str) -> dict[str, dict[int, float]]:
    """Each bot's last `bay doors [ID=WIDEST ...]` line (BMB-AI), by callsign: the widest each other aircraft's
    doors stood open on that bot's screen, 0 to 1."""
    found: dict[str, dict[int, float]] = {}
    for m in DOORS_LINE.finditer(text):
        found[m.group("name")] = {
            int(k): float(v) for k, v in (pair.split("=") for pair in m.group("doors").split())
        }
    return found


def doors_problems(text: str, watcher: str, plane: int, who: str) -> list[str]:
    """`watcher` saw `plane`'s (`who`) bay doors open wide (0.95 or more) on its screen."""
    seen = last_doors(text).get(watcher)
    if seen is None:
        return [f"{watcher} printed no bay doors line"]
    if seen.get(plane, 0.0) < 0.95:
        return [f"{watcher} saw {who}'s (plane {plane}) bay doors open at most {seen.get(plane, 0.0):.2f}, want 0.95 or more"]
    return []


STORES_LINE = re.compile(
    r"^(?P<name>\w+): stores: told (?P<told>\d+), bytes (?P<bytes>\d+), largest (?P<largest>\d+), "
    r"drawn (?P<drawn>\d+), emptied \[(?P<emptied>[\d ]*)\]$",
    re.M,
)


def last_stores(text: str) -> dict[str, dict[str, str]]:
    """Each bot's last stores line, by callsign."""
    return {m.group("name"): m.groupdict() for m in STORES_LINE.finditer(text)}


def stores_problems(text: str, players: tuple[int, ...]) -> list[str]:
    """What the bots' stores lines say against `net-a2g-stores`: Watch drew the other aircraft's stores and saw
    plane 0's and an AI aircraft's empty, Bomber saw an AI aircraft's empty, and a Stores event stayed small."""
    problems = []
    seen = last_stores(text)
    for name in ("Bomber", "Watch"):
        if name not in seen:
            problems.append(f"{name} printed no stores line")
    watch = seen.get("Watch")
    if watch:
        emptied = {int(n) for n in watch["emptied"].split()}
        if int(watch["drawn"]) < 2:
            problems.append(f"Watch drew stores on {watch['drawn']} other aircraft at most, not the Bomber and the AI")
        if int(watch["told"]) == 0:
            problems.append("Watch was told no Stores event")
        if 0 not in emptied:
            problems.append("Watch never drew the Bomber's (plane 0) Mk 82 station emptying")
        if not emptied - set(players):
            problems.append("Watch never drew an AI aircraft's stores emptying")
        if int(watch["largest"]) > 40:
            problems.append(f"a Stores event took {watch['largest']} bytes, more than a fighter's stations need")
    bomber = seen.get("Bomber")
    if bomber and not {int(n) for n in bomber["emptied"].split()} - set(players):
        problems.append("Bomber never drew an AI aircraft's stores emptying")
    return problems


def drive_strike_handback_b2(d: Drive) -> None:
    """`net-ai-strike-handback` with a B-2 (BMB-AI): the human's Mk 82 pass is finished by the AI, which opens
    the bay doors for the release, and the other client sees the doors open on the AI's plane."""
    drive_strike_handback(d, B2_STRIKE_MISSION, "MK82.JT", doors=True)


def drive_stores(d: Drive, mission: str = STORES_MISSION, load: str = "MK82.JT", doors: bool = False,
                 bomb: str | None = None) -> None:
    """Another player's and an AI's stores empty on a client's screen as they release (protocol 27). With
    `doors` (a bay aircraft) the watching client also sees the bomber's bay doors open (BMB-AI). `bomb` is the
    store the watcher must see leave when it is not the whole `load` (a comma list of stores)."""
    bomb = bomb or load
    port = d.port()
    server = start_server(d, port, mission, king="first-player", loadouts="any")
    bomber = start_bots(
        d, port, "bomber", 120, "--callsign", "Bomber", "--slot", "0", "--a2g", "bomb", "--load", load,
        "--king", "loadouts=any", "--players", "2",
    )
    if not server.wait_for(r"Bomber wears the crown", 60):
        raise DriveError("the Bomber bot never wore the crown")
    watch = start_bots(d, port, "watch", 120, "--callsign", "Watch", "--slot", "1", "--a2g", "watch")
    watch.finish(240, 0)
    bomber.finish(240, 0)
    server.send("end")
    server.finish(40, 0)
    bomber.expect(r"^Bomber: seat \d+, plane 0, at tick \d+$", "plane 0")
    watch.expect(r"^Watch: seat \d+, plane 1, at tick \d+$", "plane 1")
    watch.expect(rf"^Watch: a2g: saw {re.escape(bomb)} \d+ from plane 0 ", f"a {bomb} leaving plane 0")
    for bot in (bomber, watch):
        bot.forbid(NET_BAD, "a network problem")
    for problem in stores_problems(bomber.text() + watch.text(), (0, 1)):
        d.problem(problem)
    if doors:
        for problem in doors_problems(watch.text(), "Watch", 0, "the Bomber"):
            d.problem(problem)
    log_must(d, server_log(d), r"joined as Bomber", r"joined as Watch", forbid=r"protocol error|bad packets|\bfault\b")


def drive_stores_f117(d: Drive) -> None:
    """`net-a2g-stores` with both players in F-117s (BMB-AI): `tore-bot --a2g bomb` takes the first bomb of its
    load, the GBU-27, so a bomb-only jet bombs over the wire, and the watching client sees the bomber's bay
    doors open for each release."""
    drive_stores(d, F117_STORES_MISSION, "PAVEWA3.JT", doors=True)


def drive_stores_b2(d: Drive) -> None:
    """`net-a2g-stores` with both players in B-2s: the same Mk 82 leaves the bomber's left bay on the watcher's
    screen, and the AI's stores still empty."""
    drive_stores(d, B2_STORES_MISSION)


def drive_stores_b52(d: Drive) -> None:
    """`net-a2g-stores` with both players in B-52s (B52-9): a Mk 82 leaves the bomber's bay on the watcher's
    screen, the watcher sees the bomber's bay doors open for it, and the AI's stores still empty."""
    drive_stores(d, B52_STORES_MISSION, "AGM84E.JT,MK82.JT", doors=True, bomb="MK82.JT")


def drive_stores_b1b(d: Drive) -> None:
    """`net-a2g-stores` with both players in B-1Bs (B1B-9): a Mk 82 leaves the bomber's bay on the watcher's
    screen, the watcher sees the bomber's bay doors open for it, and the AI's stores still empty."""
    drive_stores(d, B1B_STORES_MISSION, "MK82.JT", doors=True)


def drive_stores_tu160(d: Drive) -> None:
    """`net-a2g-stores` with both players in Tu-160s (T160-9): a Mk 82 leaves the bomber's forward bay row on the
    watcher's screen, the watcher sees the bomber's bay doors open for it, and the AI's stores still empty."""
    drive_stores(d, TU160_STORES_MISSION, "MK82.JT", doors=True)


def drive_stores_f111(d: Drive) -> None:
    """`net-a2g-stores` with both players in F-111Fs (F111-9): a Mk 82 leaves the bomber's bay on the watcher's
    screen, the watcher sees the bomber's bay doors open for it, and the AI's stores still empty."""
    drive_stores(d, F111_STORES_MISSION, F111_LOAD, doors=True, bomb="MK82.JT")


def drive_stores_tu26(d: Drive) -> None:
    """`net-a2g-stores` with both players in Tu-26s (T26-9): a Mk 82 leaves the bomber's belly bay or glove row on
    the watcher's screen, the watcher sees the stores empty, and the AI's stores still empty."""
    drive_stores(d, TU26_STORES_MISSION, "AS16.JT,AS16.JT,AS16.JT,MK82.JT", doors=True, bomb="MK82.JT")


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="net-a2g-loopback", lane="net", args=[], driver=drive_loopback, uses=("server", "bot"),
            timeout=360,
        ),
        Scenario(
            name="net-ai-strike-handback", lane="net", args=[], driver=drive_strike_handback,
            uses=("server", "bot"), timeout=600,
        ),
        Scenario(
            name="net-a2g-stores", lane="net", args=[], driver=drive_stores, uses=("server", "bot"), timeout=420,
        ),
        Scenario(
            name="net-a2g-stores-b2", lane="net", args=[], driver=drive_stores_b2, uses=("server", "bot"),
            timeout=420,
        ),
        Scenario(
            name="net-a2g-stores-b52", lane="net", args=[], driver=drive_stores_b52, uses=("server", "bot"),
            timeout=420,
        ),
        Scenario(
            name="net-a2g-stores-b1b", lane="net", args=[], driver=drive_stores_b1b, uses=("server", "bot"),
            timeout=420,
        ),
        Scenario(
            name="net-a2g-stores-tu160", lane="net", args=[], driver=drive_stores_tu160, uses=("server", "bot"),
            timeout=420,
        ),
        Scenario(
            name="net-a2g-stores-f111", lane="net", args=[], driver=drive_stores_f111, uses=("server", "bot"),
            timeout=420,
        ),
        Scenario(
            name="net-a2g-stores-tu26", lane="net", args=[], driver=drive_stores_tu26, uses=("server", "bot"),
            timeout=420,
        ),
        Scenario(
            name="net-a2g-stores-f117", lane="net", args=[], driver=drive_stores_f117, uses=("server", "bot"),
            timeout=420,
        ),
        Scenario(
            name="net-ai-strike-handback-b2", lane="net", args=[], driver=drive_strike_handback_b2,
            uses=("server", "bot"), timeout=900,
        ),
    ]
