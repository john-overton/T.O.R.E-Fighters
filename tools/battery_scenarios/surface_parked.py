"""Lane: ai. Parked aircraft (docs/spec/surface-defenses.md, "Parked aircraft").

`surface-parked-aircraft` runs `--surface-parked`, which builds a Quick Mission
ground target, lists every parked aircraft and can put the player's bomb,
Maverick or gun burst on one. On the Falklands airstrip `~QLFFAIR` the five
Super Etendard targets and four other fighters stand gear down on the ground
as surface-role targets on the ground (radar cannot see them); a Mk 82, an
AGM-65G (whose seeker locks the aircraft first) and a gun burst destroy three
Super Etendards, each with damage by section, a type-30 family explosion, a
crash crater and a fire, a thrown fragment and a kill in the Fighter row (the
airfield's whole-site contact box would stop every round, so the run knocks it
down first, `--clear-shelters`). On the Ukraine strip `~QUSTRIP` a bomb on the
parked MiG-29 does the same with no shelter in the way, and a gun burst on a
MiG-25 damages its left wing without a kill. On the Clemenceau fleet `~QFFLT`
all eight Rafale M and Super Etendards stand on the carrier deck (268 ft at
today's placed scale); on the Kiev fleet `~QBFLT` the four Yak-141s stay out.
"""
import re

from battery import Scenario, Step

AIRCRAFT = re.compile(
    r"^surface-parked: aircraft (0x[0-9a-f]+) (\S+) class (0x[0-9a-f]+) hp (\d+) gear (\S+) scale (\S+) "
    r"ground (\S+) (\S+) (\S+) terrain (\S+) origin-up (\S+) heading (\S+) deck (\S+) target (\d) side (\d+)$",
    re.M,
)
OUTCOME = re.compile(
    r"^surface-parked: outcome (0x[0-9a-f]+) (\S+) hp (\d+)/(\d+) sections (\S+) structural (\S+) "
    r"craters (\d+) fires (\d+) fragment (\d)$",
    re.M,
)
KILL = re.compile(r"^surface-parked: kill (0x[0-9a-f]+) by (\d+) class (0x[0-9a-f]+) row Some\((\d+)\) aircraft (\d)$", re.M)
EXPLOSION = re.compile(r"^surface-parked: explosion (\d+) at (-?\d+) (-?\d+) (-?\d+)$", re.M)
ROW = re.compile(r"^surface-parked: row (0x[0-9a-f]+) role (\S+) on-ground (\d) airborne (\d) parked (\d) sheltered (\S+)$", re.M)

# Damage sections in the order the outcome prints them.
SECTIONS = ["cockpit", "core", "nose", "left wing", "right wing", "tail"]


def _killed(output: str, ids: list[str], problems: list[str]) -> None:
    outcomes = {m.group(1): m for m in OUTCOME.finditer(output)}
    kills = {m.group(1): m for m in KILL.finditer(output)}
    for id in ids:
        o = outcomes.get(id)
        if o is None:
            problems.append(f"no outcome for {id}")
            continue
        if o.group(3) != "0":
            problems.append(f"{id} {o.group(2)} survived with {o.group(3)} hit points")
        sections = [int(v) for v in o.group(5).split(",")]
        if sum(1 for v in sections if v > 0) == 0 or o.group(6) == "None":
            problems.append(f"{id} took no damage by section: {o.group(5)} {o.group(6)}")
        if (o.group(7), o.group(8), o.group(9)) != ("1", "1", "1"):
            problems.append(f"{id} left craters {o.group(7)} fires {o.group(8)} fragment {o.group(9)}, want 1 1 1")
        k = kills.get(id)
        if k is None:
            problems.append(f"no kill for {id}")
        elif (k.group(2), k.group(3), k.group(4), k.group(5)) != ("0", "0x8000", "0", "0"):
            problems.append(f"{id} kill {k.group(0)}: want by the player, class 0x8000, Fighter row 0, not a roster aircraft")
    blasts = [int(m.group(1)) for m in EXPLOSION.finditer(output)]
    if len(blasts) < len(ids) or any(not 24 <= b <= 33 for b in blasts):
        problems.append(f"explosions {blasts}: want one aircraft explosion (24 to 33) per kill")


def lffair_problems(output: str) -> list[str]:
    problems = []
    aircraft = AIRCRAFT.findall(output)
    if len(aircraft) != 9:
        problems.append(f"{len(aircraft)} parked aircraft on ~QLFFAIR, want 9")
    spe = [a for a in aircraft if a[1] == "SPE.PT"]
    if len(spe) != 5 or any(a[13] != "1" for a in spe):
        problems.append("the five Super Etendards should be the targets")
    if any(a[13] != "0" for a in aircraft if a[1] != "SPE.PT"):
        problems.append("only the Super Etendards are targets")
    for a in aircraft:
        if a[4] == "none" or a[12] != "none" or float(a[10]) <= 0:
            problems.append(f"{a[0]} {a[1]}: gear {a[4]} deck {a[12]} origin-up {a[10]}, want gear down on the ground")
    for m in ROW.finditer(output):
        if (m.group(2), m.group(3), m.group(4), m.group(5)) != ("surface", "1", "0", "1"):
            problems.append(f"{m.group(1)}: {m.group(0)}, want a surface-role parked row on the ground")
    if "maverick 0x50000009 accepts 1 seeker-lock 1" not in output:
        problems.append("the Maverick's seeker did not lock the parked Super Etendard")
    _killed(output, ["0x50000008", "0x50000009", "0x5000000a"], problems)
    return problems


def ustrip_problems(output: str) -> list[str]:
    problems = []
    _killed(output, ["0x50000022"], problems)
    outcomes = {m.group(1): m for m in OUTCOME.finditer(output)}
    mig25 = outcomes.get("0x50000021")
    if mig25 is None or mig25.group(3) == "0" or mig25.group(9) != "0":
        problems.append(f"the gun burst on the MiG-25 should damage it without a kill: {mig25 and mig25.group(0)}")
    elif [int(v) for v in mig25.group(5).split(",")][3] == 0:
        problems.append(f"the MiG-25's damage should be on its left wing: {mig25.group(5)}")
    return problems


def fleet_problems(output: str) -> list[str]:
    problems = []
    aircraft = AIRCRAFT.findall(output)
    clem = [a for a in aircraft if a[12] == "0x50000000"]
    if len(clem) != 8 or len(aircraft) != 8 or any(abs(float(a[7]) - 268.0) > 0.5 for a in clem):
        problems.append(f"{len(clem)} of {len(aircraft)} aircraft on the Clemenceau's deck at 268 ft, want 8")
    return problems


def fleet_problems_kiev(output: str) -> list[str]:
    problems = []
    if AIRCRAFT.findall(output):
        problems.append("no aircraft should stand in the Kiev fleet")
    left = re.findall(r"^surface-parked: left-out (\d+) YAK141\.PT on no carrier's deck$", output, re.M)
    if left != ["16", "17", "18", "19"]:
        problems.append(f"the four Kiev fleet Yak-141s should stay out, got {left}")
    return problems


def runs(output: str) -> list[str]:
    """The output of each `--surface-parked` run, in order."""
    return re.split(r"^surface-parked: done at .*$", output, flags=re.M)


def problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) < 4:
        return [f"{len(parts) - 1} runs finished, want 4"]
    return lffair_problems(parts[0]) + ustrip_problems(parts[1]) + fleet_problems(parts[2]) + fleet_problems_kiev(parts[3])


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="surface-parked-aircraft", lane="ai",
            args=["--surface-parked", "LFA", "QLFFAIR", "--clear-shelters",
                  "--strike", "8:bomb", "--strike", "9:maverick", "--strike", "10:gun"],
            timeout=300,
            then=[
                Step(["--surface-parked", "UKR", "QUSTRIP", "--strike", "34:bomb", "--strike", "33:gun"], timeout=300),
                Step(["--surface-parked", "FRA", "QFFLT", "--seconds", "0"], timeout=120),
                Step(["--surface-parked", "BAL", "QBFLT", "--seconds", "0"], timeout=120),
            ],
            expect=[
                r"^surface-parked: parked 9 placed 9 targets 5 parked-targets 5 unreadable 0$",
                r"^surface-parked: cleared shelter 0x4000000b$",
            ],
            check=problems,
            notes="Falklands airstrip strikes, the Ukraine strip MiG-29 and MiG-25, the Clemenceau deck and the "
                  "Kiev fleet's left-out Yak-141s.",
        ),
    ]
