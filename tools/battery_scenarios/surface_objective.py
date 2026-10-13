"""Lane: ai. Ground target objectives, scoring and the debrief
(docs/spec/surface-defenses.md, "Objectives, scoring and debrief").

`surface-objective-destroy` runs `--surface-objective` four times. The player
flies a scripted pass, invulnerable, at a ground target's defenses and then has
a Mk 82 placed on every target:

* `~QUCOL` (Ukraine, an armored column on the move, heavy defenses): the three
  flagged tanks are the targets; the objective reads "Destroyed 0 of 3 targets"
  at the start and "Destroyed the 3 targets" at the end, the mission is a
  SUCCESS, the debrief's SAM and AAA rows count the column's fire at the pass,
  and the three kills land in the Tank row.
* `~QLFFAIR` (the Falklands airfield): the five parked Super Etendards are the
  targets (simulated aircraft), counted in the Fighter row.
* `~QIRRETR` (Iran) with one friendly unit that is not a target destroyed too:
  every target is down and the line says so, but the mission is a FAILURE with
  one friendly-fire kill.

`surface-objective-protect` seats the human in the enemy's wing of a
multiplayer mission (a Redfor player): the targets are a Protect objective, the
column's guns and missiles fire at Blue only, and a Blue plane destroying the
column's tanks takes the line from "Protected the 3 friendly objectives" to
"Protected 0 of 3 friendly objectives".

`surface-runway-stays` (B15, 2026-10-12) bombs `~QUSTRIP` (Ukraine, an
airstrip): the Mk 82s on its hangars, shelters, tower and stores bring the
template's own runway (`STRIP.OT`, 750 hit points) down with their splash.
The runway must still be drawn (`--runway-art` counts its vertex floats), on
the redrawn airport (B14: the template's strip is redrawn with the STRIP plan)
and on the retail airfield shape (`--retail-airfields`), where it once vanished
with the whole airfield.

`surface-wrecks-burn` (B16, 2026-10-12) bombs `~QUSTRIP` (its hangars,
shelters, tower, fuel and stores) and `~QFFACT` (France, a factory: FACT1,
FACTD and its bunkers) and reads the `end wreck` lines `--runway-art` adds:
no destroyed object may draw nothing (men aside), and every one whose class
burns must have its fire. Then `~QUCOL` twice: its destroyed vehicles must
burn or not the same way both times (the pick is fixed per object and
mission, never a random stream).
"""
import re

from battery import Scenario, Step

OUTCOME = re.compile(r"^surface-objective: (start|end) outcome (\w+) objectives (\[.*\])$", re.M)
TALLY = re.compile(
    r"^surface-objective: (start|end) enemy-sam (\d+)/(\d+) enemy-aaa (\d+)/(\d+) enemy-aam (\d+)/(\d+) "
    r"enemy-gun (\d+)/(\d+) bombs (\d+)/(\d+) kills \[([\d, ]+)\] friendly-fire (\d+) shot-down-by (\S+)$",
    re.M,
)
SIDE = re.compile(r"^surface-objective: side (\w+) plane (\d+) targets (\d+) units (\d+) parked (\d+)$", re.M)

# The kill table's rows, in the debrief's order.
ROWS = ["Fighter", "Bomber", "Helicopter", "Ship", "SAM", "AAA", "Tank", "Vehicle", "Structure", "Other"]


def parts(output: str) -> list[str]:
    """The output of each `--surface-objective` run, in order."""
    return [p for p in re.split(r"^surface-objective: done$", output, flags=re.M) if "surface-objective:" in p]


def read(run: str):
    """(targets, outcomes, tallies): the target count, then `when -> (outcome, objectives)` and `when -> tally`."""
    side = SIDE.search(run)
    outcomes = {m.group(1): (m.group(2), m.group(3)) for m in OUTCOME.finditer(run)}
    tallies = {}
    for m in TALLY.finditer(run):
        sam, aaa, aam, gun, bombs = (tuple(int(m.group(i)) for i in (a, a + 1)) for a in (2, 4, 6, 8, 10))
        kills = [int(v) for v in m.group(12).split(",")]
        tallies[m.group(1)] = dict(
            sam=sam, aaa=aaa, aam=aam, gun=gun, bombs=bombs, kills=kills,
            friendly=int(m.group(13)), shot_down_by=m.group(14),
        )
    return (int(side.group(3)) if side else None), outcomes, tallies


def destroy_problems(output: str) -> list[str]:
    runs = parts(output)
    if len(runs) != 3:
        return [f"{len(runs)} runs finished, want 3"]
    problems: list[str] = []

    # The armored column.
    targets, outcomes, tallies = read(runs[0])
    if targets != 3:
        problems.append(f"~QUCOL: {targets} targets, want the three flagged tanks")
    if outcomes.get("start") != ("FAILURE", '["Destroyed 0 of 3 targets."]'):
        problems.append(f"~QUCOL start: {outcomes.get('start')}, want FAILURE and 'Destroyed 0 of 3 targets.'")
    if outcomes.get("end") != ("SUCCESS", '["Destroyed the 3 targets."]'):
        problems.append(f"~QUCOL end: {outcomes.get('end')}, want SUCCESS and 'Destroyed the 3 targets.'")
    end = tallies.get("end")
    if end is None:
        problems.append("~QUCOL: no closing tallies")
    else:
        if end["sam"][1] == 0 or end["aaa"][1] == 0:
            problems.append(f"~QUCOL: the column fired no SAM ({end['sam']}) or AAA ({end['aaa']}) at the pass")
        if end["aam"] != (0, 0) or end["gun"] != (0, 0):
            problems.append(f"~QUCOL: surface fire counted as aircraft fire: aam {end['aam']} gun {end['gun']}")
        if end["kills"][ROWS.index("Tank")] != 3:
            problems.append(f"~QUCOL: Tank row {end['kills'][ROWS.index('Tank')]}, want the three tanks")
        if end["friendly"] != 0:
            problems.append(f"~QUCOL: {end['friendly']} friendly-fire kills")
    start = tallies.get("start")
    if start and (start["sam"] != (0, 0) or start["aaa"] != (0, 0)):
        problems.append(f"~QUCOL: tallies before the pass should be empty: {start['sam']} {start['aaa']}")

    # The airfield's parked aircraft.
    targets, outcomes, tallies = read(runs[1])
    if targets != 5:
        problems.append(f"~QLFFAIR: {targets} targets, want the five Super Etendards")
    if outcomes.get("end") != ("SUCCESS", '["Destroyed the 5 targets."]'):
        problems.append(f"~QLFFAIR end: {outcomes.get('end')}, want SUCCESS and 'Destroyed the 5 targets.'")
    end = tallies.get("end")
    if end is None or end["kills"][ROWS.index("Fighter")] < 5:
        problems.append(f"~QLFFAIR: Fighter row {end and end['kills'][0]}, want the five parked aircraft")

    # A friendly unit destroyed on top of the targets.
    targets, outcomes, tallies = read(runs[2])
    if targets != 8:
        problems.append(f"~QIRRETR: {targets} targets, want 8")
    if outcomes.get("end") != ("FAILURE", '["Destroyed the 8 targets."]'):
        problems.append(
            f"~QIRRETR end: {outcomes.get('end')}, want every target down yet a FAILURE (friendly fire)"
        )
    end = tallies.get("end")
    if end is None or end["friendly"] != 1:
        problems.append(f"~QIRRETR: friendly-fire kills {end and end['friendly']}, want 1")
    return problems


def protect_problems(output: str) -> list[str]:
    runs = parts(output)
    if len(runs) != 1:
        return [f"{len(runs)} runs finished, want 1"]
    problems: list[str] = []
    side = SIDE.search(runs[0])
    if side is None or side.group(1) != "redfor" or side.group(3) != "3":
        problems.append(f"want a Redfor human and 3 targets: {side and side.group(0)}")
    _, outcomes, tallies = read(runs[0])
    start, end = outcomes.get("start"), outcomes.get("end")
    if start is None or "Protected the 3 friendly objectives." not in start[1]:
        problems.append(f"start: {start}, want 'Protected the 3 friendly objectives.'")
    if end is None or "Protected 0 of 3 friendly objectives." not in end[1] or end[0] != "FAILURE":
        problems.append(f"end: {end}, want a FAILURE and 'Protected 0 of 3 friendly objectives.'")
    final = tallies.get("end")
    if final is None or final["sam"][1] != 0 or final["aaa"][1] != 0:
        problems.append(f"the column's SAMs and guns fired at a Redfor plane: {final and (final['sam'], final['aaa'])}")
    return problems


RUNWAY = re.compile(
    r"^surface-objective: end runway (0x[0-9a-f]+) (\S+) hp (\d+)/(\d+) (retail|redrawn \S+)(?: drawn (\d+))?$",
    re.M,
)


def runway_problems(output: str) -> list[str]:
    runs = parts(output)
    if len(runs) != 2:
        return [f"{len(runs)} runs finished, want 2"]
    problems: list[str] = []
    for run, want in zip(runs, ["redrawn STRIP.OT", "retail"]):
        rows = [m for m in RUNWAY.finditer(run) if m.group(1) == "0x50000000"]
        if not rows:
            problems.append(f"{want}: no line for the template's runway 0x50000000")
            continue
        m = rows[0]
        if m.group(2) != "STRIP.OT" or m.group(3) != "0":
            problems.append(f"{want}: the bombs did not bring the runway down: {m.group(0)}")
        if m.group(5) != want:
            problems.append(f"{want}: the runway is {m.group(5)}")
        if not m.group(6) or int(m.group(6)) == 0:
            problems.append(f"{want}: the destroyed runway is not drawn: {m.group(0)}")
    return problems


WRECK = re.compile(r"^surface-objective: end wreck (0x[0-9a-f]+) (\S+) drawn (\d+) class (\w+) fire (yes|no)$", re.M)
WRECKS = re.compile(
    r"^surface-objective: end wrecks (\d+) hidden (\d+) should-burn (\d+) burning (\d+) runway-craters (\d+)$", re.M
)


def wreck_problems(output: str) -> list[str]:
    runs = parts(output)
    if len(runs) != 4:
        return [f"{len(runs)} runs finished, want 4"]
    problems: list[str] = []
    for run, name in zip(runs[:2], ["~QUSTRIP", "~QFFACT"]):
        summary = WRECKS.search(run)
        if not summary:
            problems.append(f"{name}: no end wrecks line")
            continue
        total, hidden = int(summary.group(1)), int(summary.group(2))
        if total < 5:
            problems.append(f"{name}: only {total} objects destroyed")
        if hidden:
            gone = [m.group(1) + " " + m.group(2) for m in WRECK.finditer(run) if m.group(3) == "0"]
            problems.append(f"{name}: {hidden} destroyed objects vanished: {gone}")
        for m in WRECK.finditer(run):
            if m.group(4) in ("Building", "Hard") and m.group(5) != "yes":
                problems.append(f"{name}: {m.group(1)} {m.group(2)} ({m.group(4)}) does not burn")
    picks = [
        sorted((m.group(1), m.group(5)) for m in WRECK.finditer(run) if m.group(4) in ("Vehicle", "None"))
        for run in runs[2:]
    ]
    if not picks[0]:
        problems.append("~QUCOL: no destroyed vehicle")
    elif picks[0] != picks[1]:
        problems.append(f"~QUCOL: the vehicle fires differ between runs: {picks[0]} against {picks[1]}")
    return problems


def scenarios() -> list[Scenario]:
    run = ["--surface-objective"]
    return [
        Scenario(
            name="surface-objective-destroy", lane="ai",
            args=run + ["UKR", "QUCOL"],
            timeout=600,
            then=[
                Step(run + ["LFA", "QLFFAIR", "--defenses", "0", "0", "--seconds", "150"], timeout=300),
                Step(run + ["IRA", "QIRRETR", "--defenses", "0", "0", "--kill-friendly", "--seconds", "150"], timeout=300),
            ],
            expect=[r"^surface-objective: t=\S+ every target is down$"],
            check=destroy_problems,
            notes="Reads the templates from the retail media until the import keeps them. Debug builds take "
                  "about a minute for the three runs.",
        ),
        Scenario(
            name="surface-objective-protect", lane="ai",
            args=run + ["UKR", "QUCOL", "--redfor"],
            timeout=300,
            expect=[r"^surface-objective: t=\S+ every target is down$"],
            check=protect_problems,
            notes="A multiplayer mission (open seating) with the human in the first enemy plane: Redfor defends "
                  "the target. Reads the templates from the retail media until the import keeps them.",
        ),
        Scenario(
            name="surface-runway-stays", lane="ai",
            args=run + ["UKR", "QUSTRIP", "--defenses", "0", "0", "--seconds", "150", "--runway-art"],
            timeout=300,
            then=[
                Step(run + ["UKR", "QUSTRIP", "--defenses", "0", "0", "--seconds", "150", "--runway-art",
                            "--retail-airfields"], timeout=300),
            ],
            expect=[r"^surface-objective: t=\S+ every target is down$"],
            check=runway_problems,
            notes="B15: the splash of the bombs on the airstrip's buildings destroys its runway, which stays "
                  "drawn, redrawn (B14) and retail. Builds the scenery on the CPU, no window.",
        ),
        Scenario(
            name="surface-wrecks-burn", lane="ai",
            args=run + ["UKR", "QUSTRIP", "--defenses", "0", "0", "--seconds", "150", "--runway-art"],
            timeout=300,
            then=[
                Step(run + ["FRA", "QFFACT", "--defenses", "0", "0", "--seconds", "150", "--runway-art"],
                     timeout=300),
                Step(run + ["UKR", "QUCOL", "--defenses", "0", "0", "--runway-art"], timeout=300),
                Step(run + ["UKR", "QUCOL", "--defenses", "0", "0", "--runway-art"], timeout=300),
            ],
            expect=[r"^surface-objective: end wrecks \d+ hidden 0 "],
            check=wreck_problems,
            notes="B16: destroyed buildings never vanish and burn; a vehicle's fire is a fixed pick. Builds "
                  "the scenery on the CPU, no window.",
        ),
    ]
