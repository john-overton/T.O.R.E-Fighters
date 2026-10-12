"""Lane: ai. Surface units fighting each other (John, 2026-10-11: "If units are in range of opposing sides
they should shoot at each other"). See docs/spec/surface-defenses.md, "Surface against surface".

`surface-battle-survey` builds every theater's base layout and every offered ground target at heavy defenses,
seed 1, and checks that retail never stands opposing units in reach of each other in a Quick Mission (the
nearest opposing pair is 27,000 ft apart, in the Iraq withdrawal): the ground war changes nothing in an
ordinary mission, so the single-player recordings stay as they were.

`surface-battle-column` is the test mission: the Ukraine armored column (`~QUCOL`) with its first four tanks
handed to Blue, so a Blue column stands among Red armour and the template's Red SAM and AAA sites. Both sides
engage, halt to fight, hit and kill; only the tanks fire (AAA and SAMs aim at aircraft only, by their records'
target flags); the run repeats identically. `surface-battle-fleet` hands the Baltic fleet's Kiev and a Krivak
to Blue: the Saranchas' SS-N-9s fly at the two ships, hit and sink them, and nothing else fires at a ship.
Both read the templates from the retail media until the import keeps them.
"""
import re

from battery import Scenario

SHOT = re.compile(r"^surface-battle: t (\S+) shot (.+?) (0x[0-9a-f]+) (red|blue) (\S+) x(\d+) -> (.+?) (0x[0-9a-f]+) (red|blue)$", re.M)
ENGAGE = re.compile(r"^surface-battle: t (\S+) engage (.+?) (0x[0-9a-f]+) (red|blue) weapon \d+ -> (.+?) (0x[0-9a-f]+) (red|blue)$", re.M)
KILL = re.compile(r"^surface-battle: t (\S+) kill (.+?) (0x[0-9a-f]+) (red|blue) by (.+?) (0x[0-9a-f]+) (red|blue)$", re.M)
SUMMARY = re.compile(
    r"^surface-battle: summary engagements (\d+) shots (\d+) rounds (\d+) hits (\d+) kills (\d+) halts (\d+) "
    r"first-shot (\S+) digest (0x[0-9a-f]+)$",
    re.M,
)


def summary(output: str):
    m = SUMMARY.search(output)
    return None if m is None else {
        "engagements": int(m.group(1)), "shots": int(m.group(2)), "hits": int(m.group(4)),
        "kills": int(m.group(5)), "halts": int(m.group(6)),
    }


def opposed_problems(output: str) -> list[str]:
    """Every shot, engagement and kill is between the two sides, never within one."""
    problems = []
    for pattern, what, a, b in ((SHOT, "shot", 4, 9), (ENGAGE, "engagement", 4, 7), (KILL, "kill", 4, 7)):
        for m in pattern.finditer(output):
            if m.group(a) == m.group(b):
                problems.append(f"a {what} within one side: {m.group(0)}")
    return problems


TANKS = ("T-72", "T-80", "T-90")


def column_problems(output: str) -> list[str]:
    problems = opposed_problems(output)
    s = summary(output)
    if s is None:
        return problems + ["no summary line"]
    for key, least in (("engagements", 2), ("shots", 4), ("hits", 1), ("kills", 1), ("halts", 1)):
        if s[key] < least:
            problems.append(f"{key} {s[key]}, expected at least {least}")
    sides = {m.group(4) for m in SHOT.finditer(output)}
    if sides != {"red", "blue"}:
        problems.append(f"only {sorted(sides)} fired: both sides should")
    for m in SHOT.finditer(output):
        if not m.group(2).startswith(TANKS) or m.group(5) != "T72.JT":
            problems.append(f"only the tanks' guns aim at the ground: {m.group(0)}")
    # The Blue tanks also take on the template's Red SAM and AAA sites.
    if not any(not m.group(7).startswith(TANKS) for m in SHOT.finditer(output)):
        problems.append("no tank fired at a Red site that is not a tank")
    return problems


def fleet_problems(output: str) -> list[str]:
    problems = opposed_problems(output)
    s = summary(output)
    if s is None:
        return problems + ["no summary line"]
    for key, least in (("shots", 2), ("hits", 1), ("kills", 1)):
        if s[key] < least:
            problems.append(f"{key} {s[key]}, expected at least {least}")
    if s["halts"]:
        problems.append("a ship halted: ships fight under way")
    for m in SHOT.finditer(output):
        if m.group(2) != "Sarancha" or m.group(5) != "SSN9.JT":
            problems.append(f"only the Saranchas' SS-N-9s aim at ships: {m.group(0)}")
        if m.group(7) not in ("Kiev", "Krivak"):
            problems.append(f"an SS-N-9 aimed at something not a Blue ship: {m.group(0)}")
    return problems


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="surface-battle-survey", lane="ai",
            args=["--surface-battle", "--survey"], timeout=600,
            expect=[r"^surface-battle: survey done builds 124 with-pairs 0$"],
            forbid=[r"^surface-battle: pair ", r"^surface-battle: survey .* error "],
            notes="Every theater's base layout and offered ground target, heavy defenses, seed 1: no opposing "
                  "pair stands in reach, so the ground war never wakes in an ordinary Quick Mission.",
        ),
        Scenario(
            name="surface-battle-column", lane="ai",
            args=["--surface-battle", "UKR", "QUCOL", "--blue", "0,1,2,3", "--seconds", "120", "--twice"],
            timeout=600,
            expect=[r"^surface-battle: start UKR QUCOL seed 1 player parked pairs [1-9]\d* seen [1-9]\d*$",
                    r"^surface-battle: repeat same digest "],
            check=column_problems,
            notes="The test mission: four of the column's nine tanks handed to Blue among the Red tanks and the "
                  "template's Red sites. Run twice; both runs must end in the same state.",
        ),
        Scenario(
            name="surface-battle-fleet", lane="ai",
            args=["--surface-battle", "BAL", "QBFLT", "--blue", "3,1", "--seconds", "90", "--twice"],
            timeout=600,
            expect=[r"^surface-battle: start BAL QBFLT seed 1 player parked pairs [1-9]\d* seen [1-9]\d*$",
                    r"^surface-battle: repeat same digest "],
            check=fleet_problems,
            notes="The Baltic fleet's Kiev and a Krivak handed to Blue: the Saranchas' SS-N-9s sink them. No "
                  "theater sails opposing armed ships in reach of each other, so this is a test mission too.",
        ),
    ]
