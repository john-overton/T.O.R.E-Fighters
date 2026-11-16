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
  for every one of the 54 aircraft, and never Boulder or Moose;
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
# B5-2 (2026-11-10): 18 recovered stations, 233 tank and 220 Cheat-only placements
# (the pins in crates/tore-app/src/ordnance_audit.rs carry the per-aircraft notes).
# Update (TOR-6b, 2026-11-11, m3-tornado): 222 Cheat-only placements, the Tornado's AIM-9 side
# rail row takes the two novelty bombs under Cheat loading as the retired outer row did.
# Update (Tornado merge, 2026-11-11): the merged tree measures 4,581 weapon placements; the
# weapon count here matches any number, and 222 Cheat-only placements stay as measured.
# BMB-A1 (2026-11-15): 48 aircraft, 20 recovered stations (the B-52's two empty bay rows) and
# 240 Cheat-only placements after the B-52, B-2 and F-117 joined; tank placements stay 233.
# B52-5 (2026-11-15): 21 recovered stations (the B-52's third empty bay row) and 242 Cheat-only
# placements (the B-52's underwing pylon row).
# Batch A integration (2026-11-15): 4,732 weapon placements measured after the B-2 bay refusals and
# the B-52 fold onto the shared grid; the recovered (21) and Cheat-only (242) counts are unchanged.
# BMB-B1 (2026-11-15): 51 aircraft and 249 tank placements after the B-1B, Tu-160 and F-111F joined
# (the F-111F's wing rows take 16 tank placements). Batches A and B integration (2026-11-15): 5,093
# weapon placements measured; 260 Cheat-only placements (242 plus batch B's 18); 21 recovered.
# BMB-C1 and BMB-C2 (2026-11-15): 54 aircraft and 257 tank placements after the Tu-26, Tu-95 and Su-34
# joined (the Su-34's two 0x07e5 rows take 8 tank placements). Batches A, B and C integration
# (2026-11-16): 23 recovered stations (21 plus the Tu-26 and Tu-95 empty rows) and 278 Cheat-only
# placements (260 plus batch C's 18).
# T26-5 (2026-11-16): 5,350 weapon placements measured (the Tu-26's bay, intake and glove refusals, 129 to 84);
# the recovered (23) and Cheat-only (278) counts are unchanged.
AUDIT = re.compile(r"^ordnance audit: 54 aircraft, 23 recovered weapon stations, \d+ weapon placements, "
                   r"257 tank placements, (\d+) weapon and 9 tank records; passed$", re.M)
# Aircraft with a bomb or rocket station the A2G records fit; the cargo and
# radar aircraft carry nothing.
LOADED = ("F18.PT", "A4E.PT", "F4B.PT", "SU25.PT", "A10.PT", "F16C.PT", "F15.PT", "F16.PT")
EMPTY = ("C130.PT", "E3.PT", "IL76.PT", "E2.PT", "CH47.PT", "B747.PT", "A310.PT")
A2G_RECORDS = 14  # the records X1 added to the 40 before it (the roster is 54 now)


def catalog_problems(output: str) -> list[str]:
    rows = {m[0]: (int(m[1]), int(m[2]), int(m[3])) for m in AIRCRAFT.findall(output)}
    problems: list[str] = []
    audit = AUDIT.search(output)
    if audit is None or int(audit.group(1)) < 40 + A2G_RECORDS:
        problems.append(f"the audit read {audit and audit.group(1)} supported records, want at least {40 + A2G_RECORDS}")
    if len(rows) != 54:
        problems.append(f"{len(rows)} aircraft listed a catalog, want 54")
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
                r"ordnance audit: 54 aircraft, 23 recovered weapon stations, \d+ weapon placements, 257 tank placements, \d+ weapon and 9 tank records; passed",
                r"ordnance audit: 1 designator pods, \d+ pod placements, one to an aircraft, loaded and restored through the saved mission; passed",
                r"ordnance audit: 2 novelty records, 278 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed",
            ],
            forbid=[r"PROBLEM"],
            check=catalog_problems,
            notes="The ordnance audit in both catalog modes with the A2G records and the Cheat-only novelty bombs.",
        ),
    ]
