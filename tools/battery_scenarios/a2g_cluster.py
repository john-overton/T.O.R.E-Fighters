"""Lane: flight. Cluster bombs (docs/spec/ordnance-presentation.md, "Cluster
bombs"; A2G plan decision 13), through `--a2g-run`'s `cluster` attack.

`a2g-cluster` drops a cluster bomb through the real trigger on the Ukraine
armored column `~QUCOL` three times: a CBU-87 from a level pass (splash 2,200
ft), a Mk 20 Rockeye II from the same pass (1,600 ft) and an RBK-500 from a 30
degree dive (2,500 ft). After each burst the run lists every vehicle near it
and checks:

* the retail wide splash: a vehicle whose surface is inside the record's splash
  radius loses hit points by the original's rule (the record's damage, 70
  percent of it for a ground burst, falling in a straight line to nothing at the
  radius) and one outside loses none;
* the burst is the type 18 family the five cluster records name and there is no
  crater;
* the presentation-only bomblet flashes the renderer derives from the burst:
  28, the same on every derivation, inside 1,200 ft of the burst (inside the
  smallest cluster splash, 1,500 ft), starting within the burst's first half
  second so the last ends with its second.
"""
import re

from battery import Scenario, Step

RECORD = re.compile(
    r"^a2g-run: record (\S+) flags (0x[0-9a-f]+) land-explosion (\d+) object-explosion (\d+) water-explosion (\d+) "
    r"crater (\d+) splash-radius (\d+) splash-percent (\d+) fuze (\d+) damage (\S+)$",
    re.M,
)
BURST = re.compile(r"^a2g-run: cluster burst at (\S+) (\S+) (\S+) blast Some\((\d+)\) ticks (\d+)$", re.M)
TARGET = re.compile(
    r"^a2g-run: cluster target (0x[0-9a-f]+) distance (\d+) radius (\d+) lost (-?\d+) hp (\d+)/(\d+) inside (yes|no)$",
    re.M,
)
FLASHES = re.compile(
    r"^a2g-run: cluster flashes (\d+) deterministic (\w+) first-at-tick (\d+) last-at-tick (\d+) farthest (\d+) "
    r"spread (\d+) splash (\d+)$",
    re.M,
)
FLASH = re.compile(r"^a2g-run: cluster flash n=(\d+) at (\S+) (\S+) (\S+) start (\d+) width (\d+)$", re.M)

SMALL_AIR = {18, 19, 20, 24, 25, 26, 28, 29}
CLUSTER_FLAG = 0x800000
SMALLEST_SPLASH = 1500
GROUND_SHARE = 0.7  # a bomb's ground burst passes on 70 percent of the share
SLOP_FT = 40  # a vehicle's box makes its surface a few feet nearer than its centre minus its radius
SLOP_HP = 4


def runs(output: str) -> list[str]:
    return [p for p in re.split(r"^a2g-run: done$", output, flags=re.M) if "a2g-run:" in p]


def _run_problems(run: str, label: str, want: str) -> list[str]:
    problems: list[str] = []
    record = RECORD.search(run)
    if record is None:
        return [f"{label}: no record line"]
    source, flags, land, obj, water, crater, radius, percent, _fuze, damage = record.groups()
    radius = int(radius)
    if source != want:
        problems.append(f"{label}: flew {source}, want {want}")
    if not int(flags, 16) & CLUSTER_FLAG:
        problems.append(f"{label}: {source} lacks the cluster flag 0x800000 ({flags})")
    if (int(land), int(obj), int(water)) != (18, 18, 18) or int(crater) != 0:
        problems.append(f"{label}: explosion types {land}/{obj}/{water} crater {crater}, want 18 for all and no crater")
    if percent != "100":
        problems.append(f"{label}: splash percent {percent}, want 100")
    burst = BURST.search(run)
    if burst is None:
        return problems + [f"{label}: no burst"]
    if int(burst.group(4)) not in SMALL_AIR:
        problems.append(f"{label}: burst is explosion type {burst.group(4)}, want the type 18 family")
    top = max(int(d) for d in damage.split(";"))
    targets = TARGET.findall(run)
    inside = [t for t in targets if t[6] == "yes"]
    outside = [t for t in targets if t[6] == "no"]
    if len(inside) < 3 or not outside:
        problems.append(f"{label}: {len(inside)} vehicles inside the splash and {len(outside)} outside, want 3 and 1 at least")
    for _id, distance, size, lost, _hp, _initial, _in in targets:
        gap = int(distance) - int(size)
        lost = int(lost)
        if gap >= radius + SLOP_FT and lost != 0:
            problems.append(f"{label}: a vehicle {gap} ft from the burst, beyond the {radius} ft splash, lost {lost} hp")
        if gap <= radius - SLOP_FT:
            expect = top * GROUND_SHARE * (1 - gap / radius)
            if lost <= 0 or abs(lost - expect) > SLOP_HP:
                problems.append(f"{label}: a vehicle {gap} ft from the burst lost {lost} hp, the splash rule gives {expect:.1f}")
    flashes = FLASHES.search(run)
    if flashes is None:
        return problems + [f"{label}: no flashes"]
    count, determinist, first, last, farthest, spread, splash = (
        int(g) if g.isdigit() else g for g in flashes.groups()
    )
    if count != 28:
        problems.append(f"{label}: {count} bomblet flashes, want 28")
    if determinist != "yes":
        problems.append(f"{label}: the flashes differ between two derivations of one burst")
    if not 0 <= first <= last <= 60:
        problems.append(f"{label}: flashes start between ticks {first} and {last}, want 0 to 60")
    if farthest > spread or spread >= SMALLEST_SPLASH or splash < SMALLEST_SPLASH:
        problems.append(f"{label}: flashes reach {farthest} ft of a {spread} ft spread; smallest splash is {SMALLEST_SPLASH}")
    if len(FLASH.findall(run)) != 28:
        problems.append(f"{label}: {len(FLASH.findall(run))} flash lines, want 28")
    return problems


def cluster_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != 3:
        return [f"{len(parts)} runs finished, want 3"]
    problems: list[str] = []
    for part, label, want in zip(parts, ("CBU-87 level", "Mk 20 level", "RBK-500 dive"), ("CBU87.JT", "MK20.JT", "RBK500.JT")):
        problems += _run_problems(part, label, want)
    radii = [int(RECORD.search(p).group(7)) for p in parts if RECORD.search(p)]
    if radii != [2200, 1600, 2500]:
        problems.append(f"splash radii {radii}, want the records' 2,200, 1,600 and 2,500")
    return problems


def scenarios() -> list[Scenario]:
    run = ["--a2g-run", "UKR", "QUCOL", "--attack", "cluster", "--from", "1"]
    return [
        Scenario(
            name="a2g-cluster", lane="flight",
            args=run + ["--aircraft", "a4e", "--load", "CBU87", "--count", "2", "--altitude", "5000", "--speed", "450"],
            timeout=300,
            then=[
                Step(run + ["--aircraft", "a4e", "--load", "MK20", "--count", "2", "--altitude", "5000",
                            "--speed", "450"], timeout=300),
                Step(run + ["--aircraft", "f4b", "--load", "RBK500", "--count", "2", "--altitude", "3000",
                            "--speed", "450", "--dive", "30"], timeout=300),
            ],
            check=cluster_problems,
            notes="CBU-87, Mk 20 and RBK-500 on QUCOL: the retail wide splash and the derived bomblet flashes; a few seconds a run in debug.",
        ),
    ]
