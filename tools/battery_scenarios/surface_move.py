"""Lane: ai. Surface units that follow a route.

`surface-ucol-moving` drives the Ukraine armored column (`~QUCOL`, nine tanks,
three legs at 50 ft/s) in the whole world for 600 s and checks each tank's
place against its route; a follow-up run steps the surface alone to 1,300 s
(every tank stands on its last point) and destroys one tank at 100 s (it stops
where it died). `surface-cargo-sailing` sails the three `~QTCARGO` cargo ships
(one leg at 16 ft/s, all to one point) the same way. Positions come from
`--surface-drive`; the expected paths are the retail routes (surface-AI round
survey, 2026-10-10). See docs/spec/surface-defenses.md, "Movement".
"""
import math
import re

from battery import Scenario, Step

LINE = re.compile(
    r"^surface-drive: t (\d+) unit (0x[0-9a-f]+) (\S+) pos (\S+) (\S+) (\S+) heading (\S+) pitch (\S+) bank (\S+) "
    r"speed (\S+) leg (\d+) (moving|arrived|destroyed)$",
    re.M,
)

# `~QUCOL`: start (x, z) per tank by unit id, and the three legs they share.
UCOL_STARTS = {
    0: (999570, 789172), 1: (999570, 789572), 2: (999570, 789972), 3: (999570, 790772), 4: (999570, 791172),
    5: (999570, 791572), 6: (999570, 792372), 7: (999570, 792772), 8: (999570, 793172),
}
UCOL_LEGS = [(999553, 786403), (1015857, 770071), (1048592, 769934)]

# `~QTCARGO`: three ships to one point, 16 ft/s.
CARGO_STARTS = {0: (733800, 739908), 1: (739266, 745010), 2: (744324, 750154)}
CARGO_LEGS = [(672213, 678321)]

ACCELERATION = {"tank": 5.0, "ship": 1.0}


def travelled(kind: str, speed: float, seconds: float) -> float:
    """Feet covered from rest by `seconds`: constant acceleration to `speed`, then steady."""
    ramp = speed / ACCELERATION[kind]
    return 0.5 * ACCELERATION[kind] * seconds**2 if seconds < ramp else 0.5 * speed * ramp + speed * (seconds - ramp)


def along(start, legs, distance: float):
    """The point `distance` feet along the polyline start through legs, or its last point."""
    at = start
    for leg in legs:
        step = math.dist(at, leg)
        if distance <= step:
            f = distance / step
            return (at[0] + (leg[0] - at[0]) * f, at[1] + (leg[1] - at[1]) * f)
        distance -= step
        at = leg
    return at


def rows(output: str):
    """(seconds, unit ordinal) -> (x, y, z, heading, speed, leg, state) for every position line."""
    found = {}
    for m in LINE.finditer(output):
        found[(int(m.group(1)), int(m.group(2), 16) & 0xFFFFFF)] = (
            float(m.group(4)), float(m.group(5)), float(m.group(6)), float(m.group(7)),
            float(m.group(10)), int(m.group(11)), m.group(12),
        )
    return found


def route_problems(found, starts, legs, kind: str, speed: float, times, slack: float) -> list[str]:
    """Each unit is within `slack` feet of its place on the route, at the route speed, level."""
    problems = []
    for seconds in times:
        for ordinal, start in starts.items():
            row = found.get((seconds, ordinal))
            if row is None:
                problems.append(f"no position for unit {ordinal} at {seconds} s")
                continue
            x, y, z, _, now, _, state = row
            if state != "moving":
                problems.append(f"unit {ordinal} {state} at {seconds} s, still on its way")
            want = along(start, legs, travelled(kind, speed, seconds))
            miss = math.dist((x, z), want)
            if miss > slack:
                problems.append(f"unit {ordinal} at {seconds} s is {miss:.0f} ft from its place on the route ({(x, z)} against {want})")
            if abs(now - speed) > 0.1:
                problems.append(f"unit {ordinal} at {seconds} s runs {now} ft/s, the route says {speed}")
    return problems


def ucol_problems(output: str) -> list[str]:
    found = rows(output)
    problems = route_problems(found, UCOL_STARTS, UCOL_LEGS, "tank", 50.0, (60, 600), slack=450.0)
    # At 10 s every tank has just reached 50 ft/s on its first leg, 250 ft along.
    for ordinal, start in UCOL_STARTS.items():
        row = found.get((10, ordinal))
        if row is None or abs(row[4] - 50.0) > 0.1 or abs(math.dist((row[0], row[2]), start) - 250.0) > 15.0:
            problems.append(f"unit {ordinal} at 10 s is not 250 ft along at 50 ft/s: {row}")
    # The last point is reached exactly and held; the destroyed tank stopped where it died.
    for ordinal in UCOL_STARTS:
        late = found.get((1300, ordinal))
        if late is None:
            problems.append(f"no position for unit {ordinal} at 1300 s")
            continue
        if ordinal == 1:
            early = found.get((150, 1))
            if late[6] != "destroyed" or early is None or early[:3] != late[:3] or late[4] != 0.0:
                problems.append(f"the tank destroyed at 100 s did not stop where it died: {early} then {late}")
            if early and not (UCOL_STARTS[1][1] - 6000 < early[2] < UCOL_STARTS[1][1] - 3000):
                problems.append(f"the tank destroyed at 100 s died at {early[:3]}, expected 3,000 to 6,000 ft along")
        elif late[6] != "arrived" or (late[0], late[2]) != (float(UCOL_LEGS[-1][0]), float(UCOL_LEGS[-1][1])) or late[4] != 0.0:
            problems.append(f"unit {ordinal} did not stand on the end of its route at 1300 s: {late}")
    return problems


def cargo_problems(output: str) -> list[str]:
    found = rows(output)
    problems = route_problems(found, CARGO_STARTS, CARGO_LEGS, "ship", 16.0, (30, 300, 3000), slack=60.0)
    for ordinal in CARGO_STARTS:
        row = found.get((300, ordinal))
        if row and abs(row[3] - 225.0) > 1.0:
            problems.append(f"ship {ordinal} heads {row[3]}, the route runs to 225")
        if row and row[1] != 0.0:
            problems.append(f"ship {ordinal} left the water level: y {row[1]}")
    # All three end on the one point; the shortest route first, the longest after 6,300 s.
    for ordinal in CARGO_STARTS:
        late = found.get((6500, ordinal))
        if late is None or late[6] != "arrived" or (late[0], late[2]) != (float(CARGO_LEGS[-1][0]), float(CARGO_LEGS[-1][1])):
            problems.append(f"ship {ordinal} did not stand on the end of its route at 6500 s: {late}")
    first = found.get((5500, 0))
    if first is None or first[6] != "arrived" or found.get((5500, 1), ("", "", "", "", "", "", ""))[6] != "moving":
        problems.append("the shortest route should be done at 5,500 s and the middle one not")
    return problems


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="surface-ucol-moving", lane="ai",
            args=["--surface-drive", "UKR", "QUCOL", "--at", "10,60,600"], timeout=600,
            then=[Step(["--surface-drive", "UKR", "QUCOL", "--surface-only", "--at", "150,1300", "--kill", "1@100"], timeout=300)],
            expect=[r"^surface-drive: units \d+ moving 9$", r"^surface-drive: t 100\.0 destroyed 0x50000001$"],
            forbid=[r"^surface-drive: t \d+ unit \S+ \S+ standing$"],
            check=ucol_problems,
            notes="Whole world for 600 s, then the surface alone to 1,300 s. Reads the template from the retail "
                  "media until the import keeps it.",
        ),
        Scenario(
            name="surface-cargo-sailing", lane="ai",
            args=["--surface-drive", "TVIET", "QTCARGO", "--surface-only", "--at", "30,300,3000,5500,6500"], timeout=300,
            expect=[r"^surface-drive: units \d+ moving 3$"],
            forbid=[r"^surface-drive: t \d+ unit \S+ \S+ standing$"],
            check=cargo_problems,
            notes="The three Vietnam cargo ships sail their one leg at 16 ft/s to one point. Reads the template "
                  "from the retail media until the import keeps it.",
        ),
    ]
