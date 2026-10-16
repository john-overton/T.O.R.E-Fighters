"""Lane: menus. The air-to-ground weapons in the Load Ordnance catalog
(docs/spec/ordnance-presentation.md, "Air-to-ground stores"; A2G plan decision
16).

`a2g-catalog` runs `--validate-ordnance`, whose audit now also covers the 14
bomb, cluster bomb and pod records the A2G round added (Mk 84, FAB-250, -500
and -1000, Mk 82 3-pack, Snakeye, 6-pack and 7-pack, CBU-87, CBU-97, Mk 20,
RBK-250, RBK-500 and LAU-10), the Pave Spike pod and the novelty
bombs:

* all 54 supported records are found and every compatible station takes them;
* each laser designator pod loads on every station that takes it, one to an aircraft,
  and survives the saved mission;
* the normal catalog offers exactly what some station of the aircraft accepts,
  for every one of the 36 aircraft, and never Boulder or Moose;
* Cheat loading lists every supported record and both novelty bombs, and no
  station gives the novelty bombs capacity or accepts them without Cheat
  loading, nor keeps them when Cheat loading goes off.
"""
import re

from battery import Scenario

AIRCRAFT = re.compile(
    r"^ordnance (\S+): \d+ source rows, \d+ optional weapon rows, (\d+) compatible weapon placements, "
    r"\d+ tank placements; defaults and restart passed; catalog (\d+) normal, (\d+) Cheat$",
    re.M,
)
AUDIT = re.compile(r"^ordnance audit: 36 aircraft, 12 recovered weapon stations, \d+ weapon placements, "
                   r"175 tank placements, (\d+) weapon and 9 tank records; passed$", re.M)
# Aircraft with a bomb or rocket station the A2G records fit; the cargo and
# radar aircraft carry nothing.
LOADED = ("F18.PT", "A4E.PT", "F4B.PT", "SU25.PT", "A10.PT", "F16C.PT", "F15.PT")
EMPTY = ("C130.PT", "E3.PT", "IL76.PT", "E2.PT", "CH47.PT", "B747.PT", "A310.PT")
A2G_RECORDS = 14  # the records X1 added to the 40 before it


def catalog_problems(output: str) -> list[str]:
    rows = {m[0]: (int(m[1]), int(m[2]), int(m[3])) for m in AIRCRAFT.findall(output)}
    problems: list[str] = []
    audit = AUDIT.search(output)
    if audit is None or int(audit.group(1)) < 40 + A2G_RECORDS:
        problems.append(f"the audit read {audit and audit.group(1)} supported records, want at least {40 + A2G_RECORDS}")
    if len(rows) != 36:
        problems.append(f"{len(rows)} aircraft listed a catalog, want 36")
    # Cheat loading lists every supported record and both novelty bombs (and
    # the SUU-16 gun pod only where the aircraft has one: the F-4J), so every
    # aircraft lists the same number but for one.
    cheats = sorted({cheat for _, _, cheat in rows.values()})
    if len(cheats) not in (1, 2) or (len(cheats) == 2 and cheats[1] - cheats[0] != 1):
        problems.append(f"Cheat catalogs list {cheats}, want one size, or one more with a gun pod")
    for pt, (placements, normal, cheat) in rows.items():
        if normal > cheat - 2:
            problems.append(f"{pt}: normal catalog lists {normal}, Cheat's {cheat} less the two novelty bombs is the most")
        if pt in EMPTY and (placements or normal):
            problems.append(f"{pt}: carries nothing but lists {normal} stores")
        if pt in LOADED and normal < A2G_RECORDS:
            problems.append(f"{pt}: normal catalog lists only {normal} stores, fewer than the {A2G_RECORDS} A2G records")
    return problems


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="a2g-catalog", lane="menus",
            args=["--validate-ordnance", "--no-audio"],
            timeout=600,
            expect=[
                r"ordnance audit: 36 aircraft, 12 recovered weapon stations, \d+ weapon placements, 175 tank placements, \d+ weapon and 9 tank records; passed",
                r"ordnance audit: 1 designator pods, \d+ pod placements, one to an aircraft, loaded and restored through the saved mission; passed",
                r"ordnance audit: 2 novelty records, 166 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed",
            ],
            forbid=[r"PROBLEM"],
            check=catalog_problems,
            notes="The ordnance audit in both catalog modes with the A2G records and the Cheat-only novelty bombs.",
        ),
    ]
