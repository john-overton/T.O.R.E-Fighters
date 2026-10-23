"""Lane: ai. Surface units fighting each other (John, 2026-10-11: "If units are in range of opposing sides
they should shoot at each other"). See docs/spec/surface-defenses.md, "Surface against surface".

`surface-battle-survey` builds every theater's base layout and every offered ground target at heavy defenses,
seed 1, and checks that retail never stands opposing units in reach of each other in a Quick Mission (the
nearest opposing pair is 27,000 ft apart, in the Iraq withdrawal): the ground war changes nothing in an
ordinary mission, so the single-player recordings stay as they were.

`surface-battle-column` is the test mission: the Ukraine armored column (`~QUCOL`) with its first four tanks
handed to Blue (and the Quick Mission front line left out, `--no-front`), so a Blue column stands among Red
armour and the template's Red SAM and AAA sites. Both sides engage, halt to fight, hit and kill; the tanks fire,
and so do the template's land AAA guns at the Blue tanks in reach (John, 2026-10-11: "If it can shoot at ground
targets let it"), while no SAM, flak or ship gun aims at the ground; the run repeats identically.
`surface-battle-fleet` hands the Baltic fleet's Kiev and a Krivak to Blue: the Saranchas' SS-N-9s fly at the two
ships, hit and sink them, and nothing else fires at a ship. `surface-battle-front` is the Quick Mission as the game
builds it: the Ukraine column with its front line, four Blue and four Red units 2,400 ft apart that fight each
other, tanks and an AAA gun among them, while nothing of the template's reaches them; it repeats identically.
`surface-battle-front-survey` builds every offered ground target with its front line (seed 1) and checks it
stands in contact and clear of the targets. All read the templates from the retail media until the import keeps
them.
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
# The records of the land AAA guns that now also fire at the ground, and the tanks' gun.
AAA_GUNS = ("ZSU23.JT", "ZSU57.JT", "2S6.JT", "AAA30.JT", "AAA30BAD.JT")
FRONT = re.compile(r"^surface-battle: front (.+?) (0x5e[0-9a-f]{6}) (red|blue) at (-?\d+) (-?\d+) heading (-?\d+)$", re.M)
FRONT_ID = "0x5e"


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
        record = m.group(5)
        if m.group(2).startswith(TANKS):
            if record != "T72.JT":
                problems.append(f"a tank fired {record}: {m.group(0)}")
        elif record not in AAA_GUNS:
            problems.append(f"only the tanks' guns and the land AAA guns aim at the ground: {m.group(0)}")
    # The Blue tanks also take on the template's Red SAM and AAA sites.
    if not any(not m.group(7).startswith(TANKS) for m in SHOT.finditer(output) if m.group(4) == "blue"):
        problems.append("no tank fired at a Red site that is not a tank")
    # The template's land AAA guns fire at the Blue tanks in reach, after aircraft (none flies here).
    if not any(m.group(5) in AAA_GUNS and m.group(9) == "blue" and m.group(7).startswith(TANKS)
               for m in SHOT.finditer(output)):
        problems.append("no AAA gun fired at a Blue tank")
    return problems


def front_problems(output: str) -> list[str]:
    problems = opposed_problems(output)
    front = FRONT.findall(output)
    sides = [f[2] for f in front]
    if sides.count("blue") != 4 or sides.count("red") != 4:
        problems.append(f"the front line should hold 4 Blue and 4 Red units, found {sides.count('blue')} and {sides.count('red')}")
    s = summary(output)
    if s is None:
        return problems + ["no summary line"]
    for key, least in (("engagements", 4), ("shots", 10), ("hits", 1), ("kills", 1)):
        if s[key] < least:
            problems.append(f"{key} {s[key]}, expected at least {least}")
    seen = {m.group(4) for m in SHOT.finditer(output)}
    if seen != {"red", "blue"}:
        problems.append(f"only {sorted(seen)} fired: both lines should")
    for m in SHOT.finditer(output):
        # Nothing of the template (0x50...) reaches the front, and the front reaches nothing of the template's.
        if not (m.group(3).startswith(FRONT_ID) and m.group(8).startswith(FRONT_ID)):
            problems.append(f"a shot to or from something that is not the front line: {m.group(0)}")
    # A land AAA gun of the front fires at the ground unit in reach.
    if not any(m.group(5) in AAA_GUNS for m in SHOT.finditer(output)):
        problems.append("no AAA gun of the front fired at the ground")
    return problems


FRONT_SURVEY = re.compile(
    r"^surface-battle: front-survey (\S+) (\S+) seed (\d+) units (\d+) red (\d+) blue (\d+) seen-pairs (\d+) "
    r"nearest-pair (\S+) from-target-nm (\S+)$", re.M)
FRONT_DONE = re.compile(
    r"^surface-battle: front-survey done builds (\d+) with-front (\d+) without (\d+) nearest-seen (\S+) farthest-seen (\S+)$",
    re.M)


def front_survey_problems(output: str) -> list[str]:
    problems = []
    done = FRONT_DONE.search(output)
    if done is None:
        return ["no front-survey done line"]
    builds, with_front, without = (int(done.group(i)) for i in (1, 2, 3))
    # Only naval targets far from any shore stand without a front (the land band reaches 20 nm).
    if with_front < 0.8 * builds:
        problems.append(f"only {with_front} of {builds} builds stand a front line")
    for m in FRONT_SURVEY.finditer(output):
        where = f"{m.group(1)} {m.group(2)}"
        if (m.group(4), m.group(5), m.group(6)) != ("8", "4", "4"):
            problems.append(f"{where}: front units {m.group(4)} (red {m.group(5)}, blue {m.group(6)}), expected 8 as 4 and 4")
        if int(m.group(7)) < 8:
            problems.append(f"{where}: only {m.group(7)} pairs in sight and reach")
        if m.group(8) == "none" or float(m.group(8)) > 3_600:
            problems.append(f"{where}: nearest opposing pair {m.group(8)} ft, expected within 3,600")
        if float(m.group(9)) < 3.5:
            problems.append(f"{where}: front {m.group(9)} nm from the targets, expected at least 3.5")
    if len(FRONT_SURVEY.findall(output)) != with_front:
        problems.append("a front-survey line is missing")
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
            args=["--surface-battle", "UKR", "QUCOL", "--blue", "0,1,2,3", "--no-front", "--seconds", "120", "--twice"],
            timeout=600,
            expect=[r"^surface-battle: start UKR QUCOL seed 1 player parked pairs [1-9]\d* seen [1-9]\d*$",
                    r"^surface-battle: repeat same digest "],
            check=column_problems,
            notes="The test mission: four of the column's nine tanks handed to Blue among the Red tanks and the "
                  "template's Red sites; the template's land AAA guns fire at the Blue tanks too. Run twice; both "
                  "runs must end in the same state.",
        ),
        Scenario(
            name="surface-battle-fleet", lane="ai",
            args=["--surface-battle", "BAL", "QBFLT", "--blue", "3,1", "--no-front", "--seconds", "90", "--twice"],
            timeout=600,
            expect=[r"^surface-battle: start BAL QBFLT seed 1 player parked pairs [1-9]\d* seen [1-9]\d*$",
                    r"^surface-battle: repeat same digest "],
            check=fleet_problems,
            notes="The Baltic fleet's Kiev and a Krivak handed to Blue: the Saranchas' SS-N-9s sink them. No "
                  "theater sails opposing armed ships in reach of each other, so this is a test mission too.",
        ),
        Scenario(
            name="surface-battle-front", lane="ai",
            args=["--surface-battle", "UKR", "QUCOL", "--seconds", "120", "--twice"],
            timeout=600,
            expect=[r"^surface-battle: start UKR QUCOL seed 1 player parked pairs [1-9]\d* seen [1-9]\d*$",
                    r"^surface-battle: repeat same digest "],
            check=front_problems,
            notes="The Quick Mission as the game builds it: the Ukraine column with its front line. Four Blue "
                  "and four Red units 2,400 ft apart engage, hit and kill, an AAA gun fires at the ground, "
                  "and run twice both runs end in the same state.",
        ),
        Scenario(
            name="surface-battle-front-survey", lane="ai",
            args=["--surface-battle", "--front-survey", "--seeds", "1"], timeout=900,
            expect=[r"^surface-battle: front-survey done builds [1-9]\d* with-front [1-9]\d* "],
            forbid=[r"^surface-battle: front-survey .* error "],
            check=front_survey_problems,
            notes="Every offered ground target built the way a Quick Mission builds it (seed 1): a front line of "
                  "four Blue and four Red units, in sight and reach of each other, 3.5 nm or more from the "
                  "targets; only far-out naval targets stand without one.",
        ),
    ]
