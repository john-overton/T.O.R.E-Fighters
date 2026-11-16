"""Lane: flight. Where stores leave the aircraft (bug B3, slices BP2, PY2 and PY3,
docs/spec/stores-on-pylons.md), through `--a2g-run`'s `release` attack and its `origin` lines
(every round's start against the aircraft, feet right, up and forward, the point the drawing
hangs the store at, and how deep its pylon is).

`a2g-release-origin` presses the trigger four times on each of (the A-4E's and F-4B's bomb
sight holds the fourth Mk 82 past the target, so three of those count):

* an A-4E's Mk 82 row (source x 8, 2.67 ft, no retail pair flag): a centreline rack of two
  pairs one behind another (PY2), the front pair first, right then left, each pair either side
  of the rack's bar (PY3); the A-4E's default load carries a tank there (PY3), which the run
  takes off for the bombs;
* an A-4E's LAU-61 pods on the outer wing row (x 26, 8.67 ft): one rocket a press;
* an F-4B's fuselage Mk 82 row (x 13, 4.33 ft, retail flag 0x4): one behind another on each
  side (PY2), the front pair first;
* an F-16C's inner wing row (x 32, 10.67 ft) loaded with Mk 82s: one twin-rail pylon a wing
  (John 2026-10-14, as the F/A-18's dual launchers), the two either side of its bar, the
  racks mirrored so the outboard pair leaves first;
* an F-4B's centreline row (x 0) loaded with two Mk 82s, side by side, right then left.

Every store leaves from the point it hangs at (within 0.05 ft across and up, half a foot
along for the tick flown). A wing pylon is 1 ft deep; a fuselage or centreline launcher rail
2 inches (John 2026-10-13, half of PY2's); stores two to a pylon hang beside its bar with
their tops as deep. Paired rows alternate left and right.
"""
import re

from battery import Scenario, Step

ORIGIN = re.compile(
    r"^a2g-run: origin n=(\d+) id=\d+ right (\S+) up (\S+) forward (\S+) station-right (\S+) "
    r"rounds-after (\d+) hung (\S+) (\S+) (\S+) depth (\S+)$",
    re.M,
)

# A fuselage or centreline launcher rail's depth (2 inches) and a wing pylon's, feet
# (tore_sim::combat::hardpoints::RAIL_DEPTH, PYLON_DEPTH).
RAIL = 2 / 12
WING = 1.0
# A Mk 82 beside a rack's bar: its centre line half the pylon's 0.28 ft, a 0.1 ft gap and its
# 0.45 ft radius out (tore_sim::combat::hardpoints::shoulder).
SHOULDER = 0.14 + 0.1 + 0.45

# (label, station right ft, presses, the releases' sides, how far out each leaves,
# whether each leaves ahead of the row (+1), behind (-1) or either (0), pylon depth ft)
CASES = [
    ("A-4E Mk 82 centreline rack", 0.0, 4, "RLRL", [SHOULDER] * 4, [1, 1, -1, -1], RAIL),
    ("A-4E LAU-61", 26 / 3, 4, "LRLR", [26 / 3] * 4, [0] * 4, WING),
    ("F-4B fuselage Mk 82", 13 / 3, 4, "LRLR", [13 / 3] * 4, [1, 1, -1, -1], RAIL),
    ("F-16C inner wing Mk 82", 32 / 3, 4, "LRLR",
     [32 / 3 + SHOULDER] * 2 + [32 / 3 - SHOULDER] * 2, [0] * 4, WING),
    ("F-4B centreline Mk 82", 0.0, 2, "RL", [SHOULDER] * 2, [0] * 2, RAIL),
]


def runs(output: str) -> list[str]:
    """The output of each `--a2g-run`, in order."""
    return [p for p in re.split(r"^a2g-run: done$", output, flags=re.M) if "a2g-run:" in p]


def origin_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != len(CASES):
        return [f"{len(parts)} runs finished, want {len(CASES)}"]
    problems: list[str] = []
    for part, (label, right, presses, sides_want, outs, alongs, depth_want) in zip(parts, CASES):
        # The first round of each release (a rocket or a single bomb).
        firsts = [o for o in ORIGIN.findall(part) if o[0] == "0"]
        # The bomb sight lets the last press go past the target on some
        # carriers; three releases still show the turn of sides.
        if not min(3, presses) <= len(firsts) <= presses:
            problems.append(f"{label}: {len(firsts)} releases, want {presses}")
            continue
        sides = []
        for n, (_, r, u, f, station, _after, hr, hu, hf, depth) in enumerate(firsts):
            r, u, f, station = float(r), float(u), float(f), float(station)
            hr, hu, hf = float(hr), float(hu), float(hf)
            if abs(station - right) > 0.01:
                problems.append(f"{label}: station at {station:.2f} ft right, want {right:.2f}")
            if abs(r - hr) > 0.05 or abs(u - hu) > 0.05 or abs(f - hf) > 0.5:
                problems.append(
                    f"{label}: release {n} left from {r:.2f} {u:.2f} {f:.2f}, "
                    f"hung at {hr:.2f} {hu:.2f} {hf:.2f}"
                )
            if abs(abs(r) - outs[n]) > 0.05:
                problems.append(f"{label}: release {n} left from {r:.2f} ft right, want {outs[n]:.2f} either side")
            if depth == "none" or abs(float(depth) - depth_want) > 0.01:
                problems.append(f"{label}: release {n} pylon depth {depth}, want {depth_want:.2f}")
            sides.append("L" if r < 0 else "R")
        # One behind another: the front stores leave first, a store length ahead.
        ahead = [float(o[8]) for o, a in zip(firsts, alongs) if a > 0]
        behind = [float(o[8]) for o, a in zip(firsts, alongs) if a < 0]
        if ahead and behind and min(ahead) - max(behind) < 5:
            problems.append(f"{label}: front stores at {ahead} ft forward, rear at {behind}: want 5 ft or more apart")
        if "".join(sides) != sides_want[: len(sides)]:
            problems.append(f"{label}: sides {''.join(sides)}, want {sides_want[: len(sides)]}")
    return problems


# The Mirage 2000's default stores (M2K-6, 2026-11-10), one run each: (label, right ft, up ft,
# forward ft, sides, pylon depth ft) per release, on the per-axis scaled shape. The belly row's
# four AS-15s hang in two tandem pairs on 2 inch rails, the front pair first, left then right
# (the 0.1 ft belly drop lives in the rail surface, not in these figures); the wing rows' two
# missiles leave left then right from 1 ft pylons.
M2000_CASES = [
    ("AS-15 belly row", [(1.82, -3.18, 4.05), (1.82, -3.18, 4.05), (1.82, -2.72, -4.05),
                         (1.82, -2.72, -4.05)], "LRLR", RAIL),
    ("Super 530 inner wing", [(4.99, -3.44, -4.10)] * 2, "LR", WING),
    ("Magic 2 outer wing", [(9.08, -3.27, -8.19)] * 2, "LR", WING),
]


def m2000_origin_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != len(M2000_CASES):
        return [f"{len(parts)} runs finished, want {len(M2000_CASES)}"]
    problems: list[str] = []
    for part, (label, wants, sides_want, depth_want) in zip(parts, M2000_CASES):
        firsts = [o for o in ORIGIN.findall(part) if o[0] == "0"]
        if len(firsts) != len(wants):
            problems.append(f"{label}: {len(firsts)} releases, want {len(wants)}")
            continue
        sides = []
        for n, ((_, r, u, f, _station, _after, hr, hu, hf, depth), (wr, wu, wf)) in enumerate(zip(firsts, wants)):
            r, u, f, hr, hu, hf = (float(v) for v in (r, u, f, hr, hu, hf))
            if abs(r - hr) > 0.05 or abs(u - hu) > 0.05 or abs(f - hf) > 0.5:
                problems.append(f"{label}: release {n} left from {r:.2f} {u:.2f} {f:.2f}, hung at {hr:.2f} {hu:.2f} {hf:.2f}")
            if abs(abs(r) - wr) > 0.05 or abs(u - wu) > 0.05 or abs(f - wf) > 0.5:
                problems.append(f"{label}: release {n} at {r:.2f} {u:.2f} {f:.2f}, want +-{wr:.2f} {wu:.2f} {wf:.2f}")
            if depth == "none" or abs(float(depth) - depth_want) > 0.01:
                problems.append(f"{label}: release {n} pylon depth {depth}, want {depth_want:.2f}")
            sides.append("L" if r < 0 else "R")
        if "".join(sides) != sides_want:
            problems.append(f"{label}: sides {''.join(sides)}, want {sides_want}")
    return problems


# The F-15E's default stores (F15E-6, 2026-11-10), one run each, on the true-size shape, from the
# F15E-5 positions. The belly pair's twelve Mk 82s hang as two triple racks of six bombs a side
# on 2 inch rails (the 0.1 ft belly drop of F15E-5 round 2 is already in these figures): the
# first four leave from under the racks' bars (6.33 ft out, 5.29 ft down, 0.74 ft deep), front
# pair first then the rear pair, left then right; then the outboard column (7.02 ft out), then
# the inboard column (5.64 ft out), each front pair first. The eleven checked presses are the
# ones the bomb sight lets through before the target (the twelfth goes past it on some runs). The
# wing pair's four AIM-120s hang two a side on twin bars from 1 ft pylons at 14.33 ft: the
# outboard pair (14.96 ft) first, then the inboard (13.70 ft), left then right.
F15E_BOMBS = (
    [(6.33, -5.29, 2.48, 0.74)] * 2 + [(6.33, -5.29, -5.82, 0.74)] * 2
    + [(7.02, -4.72, 2.48, RAIL)] * 2 + [(7.02, -4.72, -5.82, RAIL)] * 2
    + [(5.64, -4.72, 2.48, RAIL)] * 2 + [(5.64, -4.72, -5.82, RAIL)]
)
F15E_CASES = [
    ("Mk 82 belly racks", F15E_BOMBS, "LR" * 5 + "L", 11),
    ("AIM-120 wing pair", [(14.96, -0.77, -4.67, WING)] * 2 + [(13.70, -0.77, -4.67, WING)] * 2, "LRLR", 4),
]


def f15e_origin_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != len(F15E_CASES):
        return [f"{len(parts)} runs finished, want {len(F15E_CASES)}"]
    problems: list[str] = []
    for part, (label, wants, sides_want, need) in zip(parts, F15E_CASES):
        firsts = [o for o in ORIGIN.findall(part) if o[0] == "0"]
        if len(firsts) < need:
            problems.append(f"{label}: {len(firsts)} releases, want at least {need}")
            continue
        sides = []
        for n, ((_, r, u, f, _station, _after, hr, hu, hf, depth), (wr, wu, wf, wd)) in enumerate(zip(firsts, wants)):
            r, u, f, hr, hu, hf = (float(v) for v in (r, u, f, hr, hu, hf))
            if abs(r - hr) > 0.05 or abs(u - hu) > 0.05 or abs(f - hf) > 0.5:
                problems.append(f"{label}: release {n} left from {r:.2f} {u:.2f} {f:.2f}, hung at {hr:.2f} {hu:.2f} {hf:.2f}")
            if abs(abs(r) - wr) > 0.05 or abs(u - wu) > 0.05 or abs(f - wf) > 0.5:
                problems.append(f"{label}: release {n} at {r:.2f} {u:.2f} {f:.2f}, want +-{wr:.2f} {wu:.2f} {wf:.2f}")
            if depth == "none" or abs(float(depth) - wd) > 0.01:
                problems.append(f"{label}: release {n} pylon depth {depth}, want {wd:.2f}")
            sides.append("L" if r < 0 else "R")
        if "".join(sides) != sides_want[: len(sides)]:
            problems.append(f"{label}: sides {''.join(sides)}, want {sides_want[: len(sides)]}")
    return problems


# The Gripen's default stores (GRI-6, 2026-11-10), one run each: (label, right ft, up ft,
# forward ft, sides, pylon depth ft) per release, at the GRI-5 station positions on the true-size
# shape. The inner wing's two AGM-65Gs hang from 1 ft pylons at +-5.00 ft, 2.21 ft down, 4.67 ft
# aft; the outer wing pair's four AIM-120s in two a side on twin bars at +-9.37 and +-10.63 ft, 1.96
# down, 7.00 aft; the tip pair's AIM-9Ms at +-13.67 ft, 0.67 down, 8.67 aft. A paired row alternates
# left then right; the outer pair's first two releases are the outboard pair (left, right).
GRIPEN_CASES = [
    ("AGM-65G inner wing", [(5.00, -2.21, -4.67)] * 2, "LR", WING),
    ("AIM-120 outer wing", [(10.63, -1.96, -7.00), (10.63, -1.96, -7.00),
                            (9.37, -1.96, -7.00), (9.37, -1.96, -7.00)], "LRLR", WING),
    ("AIM-9M wingtip", [(13.67, -0.67, -8.67)] * 2, "LR", None),
]


def gripen_origin_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != len(GRIPEN_CASES):
        return [f"{len(parts)} runs finished, want {len(GRIPEN_CASES)}"]
    problems: list[str] = []
    for part, (label, wants, sides_want, depth_want) in zip(parts, GRIPEN_CASES):
        firsts = [o for o in ORIGIN.findall(part) if o[0] == "0"]
        if len(firsts) != len(wants):
            problems.append(f"{label}: {len(firsts)} releases, want {len(wants)}")
            continue
        sides = []
        for n, ((_, r, u, f, _station, _after, hr, hu, hf, depth), (wr, wu, wf)) in enumerate(zip(firsts, wants)):
            r, u, f, hr, hu, hf = (float(v) for v in (r, u, f, hr, hu, hf))
            if abs(r - hr) > 0.05 or abs(u - hu) > 0.05 or abs(f - hf) > 0.5:
                problems.append(f"{label}: release {n} left from {r:.2f} {u:.2f} {f:.2f}, hung at {hr:.2f} {hu:.2f} {hf:.2f}")
            if abs(abs(r) - wr) > 0.05 or abs(u - wu) > 0.05 or abs(f - wf) > 0.5:
                problems.append(f"{label}: release {n} at {r:.2f} {u:.2f} {f:.2f}, want +-{wr:.2f} {wu:.2f} {wf:.2f}")
            if depth_want is not None and (depth == "none" or abs(float(depth) - depth_want) > 0.01):
                problems.append(f"{label}: release {n} pylon depth {depth}, want {depth_want:.2f}")
            sides.append("L" if r < 0 else "R")
        if "".join(sides) != sides_want:
            problems.append(f"{label}: sides {''.join(sides)}, want {sides_want}")
    return problems


# The F-5E Tiger's stores (F5E-6, 2026-11-10), one run each: (label, right ft, up ft, forward ft)
# per release, sides, pylon depth ft or None. The wing pair (row 5, empty by default, loaded here)
# hangs two stores a pylon at 7.33 ft out: the outer pair of each wing leaves first, left then
# right, then the inner pair; the wingtip AIM-9Bs leave from the tip rails (no pylon depth).
F5E_CASES = [
    ("Mk 82 wing pair", [(8.02, -2.78, 1.00), (8.02, -2.78, 1.00), (6.64, -2.78, 1.00),
                         (6.64, -2.78, 1.00)], "LRLR", WING),
    ("AGM-65G wing pair", [(8.18, -2.83, 1.00), (8.18, -2.83, 1.00), (6.48, -2.83, 1.00),
                           (6.48, -2.83, 1.00)], "LRLR", WING),
    ("AIM-9B wingtip", [(14.33, -1.17, -1.00)] * 2, "LR", None),
]


def f5e_origin_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != len(F5E_CASES):
        return [f"{len(parts)} runs finished, want {len(F5E_CASES)}"]
    problems: list[str] = []
    for part, (label, wants, sides_want, depth_want) in zip(parts, F5E_CASES):
        firsts = [o for o in ORIGIN.findall(part) if o[0] == "0"]
        if len(firsts) != len(wants):
            problems.append(f"{label}: {len(firsts)} releases, want {len(wants)}")
            continue
        sides = []
        for n, ((_, r, u, f, _station, _after, hr, hu, hf, depth), (wr, wu, wf)) in enumerate(zip(firsts, wants)):
            r, u, f, hr, hu, hf = (float(v) for v in (r, u, f, hr, hu, hf))
            if abs(r - hr) > 0.05 or abs(u - hu) > 0.05 or abs(f - hf) > 0.5:
                problems.append(f"{label}: release {n} left from {r:.2f} {u:.2f} {f:.2f}, hung at {hr:.2f} {hu:.2f} {hf:.2f}")
            if abs(abs(r) - wr) > 0.05 or abs(u - wu) > 0.05 or abs(f - wf) > 0.5:
                problems.append(f"{label}: release {n} at {r:.2f} {u:.2f} {f:.2f}, want +-{wr:.2f} {wu:.2f} {wf:.2f}")
            if depth_want is None:
                if depth != "none":
                    problems.append(f"{label}: release {n} pylon depth {depth}, want none")
            elif depth == "none" or abs(float(depth) - depth_want) > 0.01:
                problems.append(f"{label}: release {n} pylon depth {depth}, want {depth_want:.2f}")
            sides.append("L" if r < 0 else "R")
        if "".join(sides) != sides_want:
            problems.append(f"{label}: sides {''.join(sides)}, want {sides_want}")
    return problems


# The Mirage F1's stores (MF1-6, 2026-11-11), one run each, on the per-axis scaled shape: the
# wing Magics leave left then right from 1 ft pylons; the wingtip Magics from the shifted tip
# rail (1.1 ft outboard, MF1-5b) with no pylon depth; the belly Mk 82 pair, loaded for the
# check, from the 3.24 ft pylon slot (the default belly pair is tanks).
MF1_CASES = [
    ("Magic wing", [(7.23, -1.12, -1.18)] * 2, "LR", WING),
    ("Magic wingtip", [(14.88, 0.70, -4.33)] * 2, "LR", None),
    ("Mk 82 belly pair", [(3.24, -1.45, 0.00)] * 2, "LR", WING),
]


def mf1_origin_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != len(MF1_CASES):
        return [f"{len(parts)} runs finished, want {len(MF1_CASES)}"]
    problems: list[str] = []
    for part, (label, wants, sides_want, depth_want) in zip(parts, MF1_CASES):
        firsts = [o for o in ORIGIN.findall(part) if o[0] == "0"]
        if len(firsts) != len(wants):
            problems.append(f"{label}: {len(firsts)} releases, want {len(wants)}")
            continue
        sides = []
        for n, ((_, r, u, f, _station, _after, hr, hu, hf, depth), (wr, wu, wf)) in enumerate(zip(firsts, wants)):
            r, u, f, hr, hu, hf = (float(v) for v in (r, u, f, hr, hu, hf))
            if abs(r - hr) > 0.05 or abs(u - hu) > 0.05 or abs(f - hf) > 0.5:
                problems.append(f"{label}: release {n} left from {r:.2f} {u:.2f} {f:.2f}, hung at {hr:.2f} {hu:.2f} {hf:.2f}")
            if abs(abs(r) - wr) > 0.05 or abs(u - wu) > 0.05 or abs(f - wf) > 0.5:
                problems.append(f"{label}: release {n} at {r:.2f} {u:.2f} {f:.2f}, want +-{wr:.2f} {wu:.2f} {wf:.2f}")
            if depth_want is None:
                if depth != "none":
                    problems.append(f"{label}: release {n} pylon depth {depth}, want none")
            elif depth == "none" or abs(float(depth) - depth_want) > 0.01:
                problems.append(f"{label}: release {n} pylon depth {depth}, want {depth_want:.2f}")
            sides.append("L" if r < 0 else "R")
        if "".join(sides) != sides_want:
            problems.append(f"{label}: sides {''.join(sides)}, want {sides_want}")
    return problems


# The Tornado's default stores (TOR-6, 2026-11-11), one run each: (label, [(right ft, up ft,
# forward ft) per release, signed], pylon depth ft). The shoulder Paveways hang from 2 inch
# fuselage rails at 2.00 ft out, the forward pair (12.27 ft ahead) first, left then right; they
# never move with the sweep. The inner wing's Mk 82s hang from 1 ft pylons on the swinging panel
# at 9.50 ft out, 1.50 ft ahead with the wing spread (25 degrees, 200 kt); at 67 degrees (650 kt)
# the pylons swivel with the panel, so the left bomb leaves from -10.53 ft and 0.81 ft aft, the
# right from 10.36 ft and 1.26 ft aft (the left glove is a source unit further out, so the pair is
# not an exact mirror image). Left first on a paired row.
#
# Update (TOR-12, 2026-11-12, John's wing review): the wing panel moved 1.54 ft in, 0.20 ft
# forward and 0.1167 ft up and the left wing now mirrors the right, and the pylons stay at the same
# place on the panel: the Mk 82 packs hang 7.96 ft out, 1.70 ft ahead and 1.33 ft below the
# reference with the wing spread (25 degrees, 200 kt), 1.00 ft pylons; at 67 degrees (650 kt)
# both leave from 8.77 ft out and 1.39 ft aft, an exact mirror pair.
#
# Update (TOR-14, 2026-11-13, John: "all pylons need to be centered on the wing"): the pylon's
# centre is the panel's mid-chord at its span (0.38 ft aft of the reference with the wing spread),
# so the Mk 82 packs hang 7.96 ft out and 0.38 ft aft at 25 degrees and leave from 7.38 ft out and
# 2.94 ft aft at 67 degrees; the pylon is one fixed size at every sweep.
TORNADO_CASES = [
    ("Paveway shoulder row, 25 degrees",
     [(-2.00, -4.92, 12.27), (2.00, -4.92, 12.27), (-2.00, -4.55, -2.93), (2.00, -4.55, -2.93)], RAIL),
    ("Mk 82 inner wing, 25 degrees", [(-7.96, -1.33, -0.38), (7.96, -1.33, -0.38)], WING),
    ("Paveway shoulder row, 67 degrees",
     [(-2.00, -4.92, 12.27), (2.00, -4.92, 12.27), (-2.00, -4.55, -2.93), (2.00, -4.55, -2.93)], RAIL),
    ("Mk 82 inner wing, 67 degrees", [(-7.38, -1.33, -2.94), (7.38, -1.33, -2.94)], WING),
]


# The Tornado's AIM-9 side rails (row 8, TOR-6b, 2026-11-11): the AIM-9M on the rail of the
# inner pylon, level with the Mk 82 pack's pylon fore and aft at every sweep (the 1.25 ft is not
# turned with the wing); 0.53 ft from the wing's underside (the rail's own depth: the pylon's 1 ft
# less the rail's rise). Update (TOR-11, 2026-11-12, John): the rail is on the pylon's INBOARD
# flank, facing the fuselage, 1.25 ft inboard of the pack's pylon: 8.25 ft out at 25 degrees
# (200 kt, still spread: the wing starts to sweep at 304 kt), and at 67 degrees (650 kt, past the
# 600 kt full sweep) right 9.11 ft out and 1.26 ft aft, left 9.28 ft out and 0.81 ft aft (it
# was 10.75 and 11.61/11.78 on the outboard flank). Left first.
#
# Update (TOR-12, 2026-11-12): with the wing placed and mirrored the rail is 6.71 ft out and 0.62
# ft below the reference at 25 degrees (1.70 ft ahead), and 7.52 ft out, 1.39 ft aft at 67
# degrees on both sides.
#
# Update (TOR-14, 2026-11-13): the pylon is centred on the wing's mid-chord, so the rail is 6.71
# ft out and 0.38 ft aft at 25 degrees and 6.13 ft out and 2.94 ft aft at 67 degrees, still 1.25
# ft inboard of the pack's pylon.
TORNADO_RAIL_CASES = [
    ("AIM-9M side rail, 25 degrees", [(-6.71, -0.62, -0.38), (6.71, -0.62, -0.38)], 0.53),
    ("AIM-9M side rail, 67 degrees", [(-6.13, -0.62, -2.94), (6.13, -0.62, -2.94)], 0.53),
]


def tornado_rail_problems(output: str) -> list[str]:
    return tornado_origin_problems(output, TORNADO_RAIL_CASES)


def tornado_origin_problems(output: str, cases=None) -> list[str]:
    cases = TORNADO_CASES if cases is None else cases
    parts = runs(output)
    if len(parts) != len(cases):
        return [f"{len(parts)} runs finished, want {len(cases)}"]
    problems: list[str] = []
    for part, (label, wants, depth_want) in zip(parts, cases):
        firsts = [o for o in ORIGIN.findall(part) if o[0] == "0"]
        if len(firsts) != len(wants):
            problems.append(f"{label}: {len(firsts)} releases, want {len(wants)}")
            continue
        for n, ((_, r, u, f, _station, _after, hr, hu, hf, depth), (wr, wu, wf)) in enumerate(zip(firsts, wants)):
            r, u, f, hr, hu, hf = (float(v) for v in (r, u, f, hr, hu, hf))
            if abs(r - hr) > 0.05 or abs(u - hu) > 0.05 or abs(f - hf) > 0.5:
                problems.append(f"{label}: release {n} left from {r:.2f} {u:.2f} {f:.2f}, hung at {hr:.2f} {hu:.2f} {hf:.2f}")
            if abs(r - wr) > 0.05 or abs(u - wu) > 0.05 or abs(f - wf) > 0.5:
                problems.append(f"{label}: release {n} at {r:.2f} {u:.2f} {f:.2f}, want {wr:.2f} {wu:.2f} {wf:.2f}")
            if depth == "none" or abs(float(depth) - depth_want) > 0.01:
                problems.append(f"{label}: release {n} pylon depth {depth}, want {depth_want:.2f}")
    return problems


def tornado_args(speed: str, station: str, load: str, count: str) -> list[str]:
    return ["--a2g-run", "UKR", "QUCOL", "--attack", "release", "--from", "1", "--altitude", "3000",
            "--speed", speed, "--repeat", count, "--gap", "0.5", "--aircraft", "tornado", "--station", station,
            "--load", load, "--count", count]


# The B-2's bays (B2-6, 2026-11-15; B2-5's grid, `hardpoints::grid_point`): per run, (station, store,
# presses, bay centre right ft, forward ft of the rows, up ft of the first tier, how many places to
# check). A 4 abreast row leaves outermost first, alternating sides (centre -1.95, +1.95, -0.65,
# +0.65 ft); the Mk 82s hang two rows long (10.44 then 2.89 ft forward), a tier 1.3 ft up from the
# last, the Paveway III one row (6.67 ft) 1.79 ft under the reference. The AI's release uses the same
# places (`ai-strike-b2-stick`).
B2_COLUMNS = (-1.95, 1.95, -0.65, 0.65)
B2_RUNS = [
    (0, "MK82", "32", 12, -5.333, (10.44, 2.89), -2.95, 9),
    (1, "MK82", "32", 12, 4.667, (10.44, 2.89), -2.95, 9),
    (2, "PAVEWA3", "4", 4, -5.333, (6.67,), -1.79, 3),
    (3, "PAVEWA3", "4", 4, 4.667, (6.67,), -1.79, 3),
]


def b2_args(station: int, load: str, count: str, presses: int) -> list[str]:
    return ["--a2g-run", "UKR", "QUCOL", "--attack", "release", "--from", "1", "--altitude", "20000",
            "--speed", "300", "--repeat", str(presses), "--gap", "0.5", "--aircraft", "b2", "--station",
            str(station), "--load", load, "--count", count]


def b2_origin_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != len(B2_RUNS):
        return [f"{len(parts)} runs finished, want {len(B2_RUNS)}"]
    problems: list[str] = []
    for part, (station, load, _, _, centre, rows, up0, checked) in zip(parts, B2_RUNS):
        label = f"B-2 station {station} {load}"
        found = ORIGIN.findall(part)
        if len(found) < checked:
            problems.append(f"{label}: {len(found)} releases, want at least {checked}")
        for n, (_, r, u, f, _station, _after, hr, hu, hf, _depth) in enumerate(found[:checked]):
            r, u, f, hr, hu, hf = (float(v) for v in (r, u, f, hr, hu, hf))
            want_r = centre + B2_COLUMNS[n % 4]
            want_f = rows[(n // 4) % len(rows)]
            want_u = up0 + 1.3 * (n // (4 * len(rows)))
            if abs(r - hr) > 0.05 or abs(u - hu) > 0.05 or abs(f - hf) > 0.5:
                problems.append(f"{label}: release {n} left from {r:.2f} {u:.2f} {f:.2f}, hung at {hr:.2f} {hu:.2f} {hf:.2f}")
            if abs(r - want_r) > 0.05 or abs(u - want_u) > 0.06 or abs(f - want_f) > 0.5:
                problems.append(f"{label}: release {n} at {r:.2f} {u:.2f} {f:.2f}, want {want_r:.2f} {want_u:.2f} {want_f:.2f}")
    return problems


# BMB-AI2 (2026-11-16): a ripple tapped faster than the bay doors open (about 0.8 s) is held, not
# dropped. (aircraft, station, load, count, presses, gap seconds); each press is one store.
# The B-2's two bays alternate and the driver re-selects its one loaded station every tick, so its
# held presses are proved in `a_b2_ripple...` and `presses_made_while_the_bay_doors_open...` instead.
TAP_RUNS = [
    ("b52", "2", "MK82", "9", 8, "0.2"),
    ("b52", "2", "MK82", "9", 8, "0.1"),
    ("b52", "2", "MK82", "9", 8, "0.05"),
]


def tap_args(aircraft: str, station: str, load: str, count: str, presses: int, gap: str) -> list[str]:
    return ["--a2g-run", "UKR", "QUCOL", "--attack", "release", "--from", "1", "--altitude", "20000",
            "--speed", "300", "--repeat", str(presses), "--gap", gap, "--aircraft", aircraft, "--station",
            station, "--load", load, "--count", count]


def tap_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != len(TAP_RUNS):
        return [f"{len(parts)} runs finished, want {len(TAP_RUNS)}"]
    problems: list[str] = []
    for part, (aircraft, _, _, _, presses, gap) in zip(parts, TAP_RUNS):
        fired = len(re.findall(r"^a2g-run: fired t=", part, re.M))
        if fired != presses:
            problems.append(f"{aircraft} {presses} taps {gap} s apart: {fired} releases, want {presses}")
    return problems


def scenarios() -> list[Scenario]:
    base = ["--a2g-run", "UKR", "QUCOL", "--attack", "release", "--from", "1", "--altitude", "3000",
            "--speed", "420"]
    run = ["--a2g-run", "UKR", "QUCOL", "--attack", "release", "--from", "1", "--altitude", "3000",
           "--speed", "420", "--repeat", "4", "--gap", "0.5"]
    return [
        Scenario(
            name="a2g-ripple-taps", lane="flight",
            args=tap_args(*TAP_RUNS[0]),
            timeout=300,
            then=[Step(tap_args(*run), timeout=300) for run in TAP_RUNS[1:]],
            check=tap_problems,
            notes="Eight one-tick presses of a bay ripple, 0.2, 0.1 and 0.05 s apart, from the B-52's forward row: a press made while the doors are still opening is held and fires when they are open, so eight presses give eight releases (BMB-AI2; before, four, one and one).",
        ),
        Scenario(
            name="a2g-release-origin", lane="flight",
            args=run + ["--aircraft", "a4e", "--load", "MK82", "--count", "4"],
            timeout=300,
            then=[
                Step(run + ["--aircraft", "a4e", "--load", "LAU61"], timeout=300),
                Step(run + ["--aircraft", "f4b", "--load", "MK82", "--count", "4"], timeout=300),
                Step(run + ["--aircraft", "f16c", "--station", "1", "--load", "MK82", "--count", "4"],
                     timeout=300),
                Step(["--a2g-run", "UKR", "QUCOL", "--attack", "release", "--from", "1", "--altitude",
                      "3000", "--speed", "420", "--repeat", "2", "--gap", "0.5", "--aircraft", "f4b",
                      "--station", "0", "--load", "MK82", "--count", "2"], timeout=300),
            ],
            check=origin_problems,
            notes="Four presses on four rows and two on a centreline row: each store leaves from where it hangs, under a 1 ft wing pylon or a 2 inch body rail, pairs beside a rack's bar (B3, BP2, PY2, PY3).",
        ),
        Scenario(
            name="a2g-release-origin-m2000", lane="flight",
            args=base + ["--repeat", "4", "--gap", "0.5", "--aircraft", "m2000", "--station", "1", "--load", "AS15",
                         "--count", "4"],
            timeout=300,
            then=[
                Step(base + ["--repeat", "2", "--gap", "0.5", "--aircraft", "m2000", "--station", "2", "--load",
                             "R530", "--count", "2"], timeout=300),
                Step(base + ["--repeat", "2", "--gap", "0.5", "--aircraft", "m2000", "--station", "3", "--load",
                             "R550", "--count", "2"], timeout=300),
            ],
            check=m2000_origin_problems,
            notes="The Mirage 2000's default stores leave from where they hang: the belly AS-15s in two tandem pairs front first, the Super 530s and Magics from their 1 ft wing pylons, left then right (M2K-6).",
        ),
        Scenario(
            name="a2g-release-origin-f15e", lane="flight",
            args=base + ["--repeat", "12", "--gap", "0.5", "--aircraft", "f15e", "--station", "1", "--load", "MK82",
                         "--count", "12"],
            timeout=300,
            then=[
                Step(base + ["--repeat", "4", "--gap", "0.5", "--aircraft", "f15e", "--station", "2", "--load",
                             "AIM120", "--count", "4"], timeout=300),
            ],
            check=f15e_origin_problems,
            notes="The F-15E's default stores leave from where they hang: the twelve belly Mk 82s from under the two triple racks (the four under the bars first, then the outboard and inboard columns, front pair first, left then right), the four AIM-120s from the wing pair outboard pair first (F15E-6).",
        ),
        Scenario(
            name="a2g-release-origin-gripen", lane="flight",
            args=base + ["--repeat", "2", "--gap", "0.5", "--aircraft", "gripen", "--station", "1", "--load", "AGM65G",
                         "--count", "2"],
            timeout=300,
            then=[
                Step(base + ["--repeat", "4", "--gap", "0.5", "--aircraft", "gripen", "--station", "2", "--load",
                             "AIM120", "--count", "4"], timeout=300),
                Step(base + ["--repeat", "2", "--gap", "0.5", "--aircraft", "gripen", "--station", "3", "--load",
                             "AIM9M", "--count", "2"], timeout=300),
            ],
            check=gripen_origin_problems,
            notes="The Gripen's default stores leave from where they hang: the AGM-65Gs from the inner wing pylons, the four AIM-120s from the outer pair's twin bars outboard pair first, the AIM-9Ms from the tips, alternating left and right (GRI-6).",
        ),
        Scenario(
            name="a2g-release-origin-f5e", lane="flight",
            args=base + ["--repeat", "4", "--gap", "0.5", "--aircraft", "f5e", "--station", "2", "--load", "MK82",
                         "--count", "4"],
            timeout=300,
            then=[
                Step(base + ["--repeat", "4", "--gap", "0.5", "--aircraft", "f5e", "--station", "2", "--load",
                             "AGM65G", "--count", "4"], timeout=300),
                Step(base + ["--repeat", "2", "--gap", "0.5", "--aircraft", "f5e", "--station", "1", "--load",
                             "AIM9B", "--count", "2"], timeout=300),
            ],
            check=f5e_origin_problems,
            notes="The F-5E's wing-pair stores (an iron bomb and an AGM-65G, loaded on the empty default row 5) leave from their 1 ft pylons, outer pair first, left then right, and the default wingtip AIM-9Bs from the tip rails, left then right (F5E-6).",
        ),
        Scenario(
            name="a2g-release-origin-mf1", lane="flight",
            args=base + ["--repeat", "2", "--gap", "0.5", "--aircraft", "mf1", "--station", "1", "--load", "R550",
                         "--count", "2"],
            timeout=300,
            then=[
                Step(base + ["--repeat", "2", "--gap", "0.5", "--aircraft", "mf1", "--station", "2", "--load",
                             "R550", "--count", "2"], timeout=300),
                Step(base + ["--repeat", "2", "--gap", "0.5", "--aircraft", "mf1", "--station", "4", "--load",
                             "MK82", "--count", "2"], timeout=300),
            ],
            check=mf1_origin_problems,
            notes="The Mirage F1's stores leave from where they hang: the wing Magics from 1 ft pylons, the wingtip Magics from the shifted tip rail, a Mk 82 pair from the belly pylon slot at 3.24 ft, left then right (MF1-6).",
        ),
        Scenario(
            name="a2g-release-origin-tornado", lane="flight",
            args=tornado_args("200", "1", "PAVEWAY", "4"),
            timeout=300,
            then=[
                Step(tornado_args("200", "2", "MK82P3", "2"), timeout=300),
                Step(tornado_args("650", "1", "PAVEWAY", "4"), timeout=300),
                Step(tornado_args("650", "2", "MK82P3", "2"), timeout=300),
            ],
            check=tornado_origin_problems,
            notes="The Tornado's default stores leave from where they hang at both sweeps: the shoulder Paveways from fixed 2 inch rails whatever the speed, the inner wing Mk 82s from 9.50 ft out at 25 degrees (200 kt) and from the swivelled pylons at 67 degrees (650 kt), left then right (TOR-6). Update (TOR-12, 2026-11-12): 7.96 ft out and 1.70 ft ahead at 25 degrees, 8.77 ft out and 1.39 ft aft at 67 degrees, after John's wing review.",
        ),
        Scenario(
            name="a2g-release-origin-b2", lane="flight",
            args=b2_args(*B2_RUNS[0][:3], B2_RUNS[0][3]),
            timeout=300,
            then=[Step(b2_args(*r[:3], r[3]), timeout=300) for r in B2_RUNS[1:]],
            check=b2_origin_problems,
            notes="The B-2's bay grids (B2-5): the left and right bays' Mk 82 rows leave nine presses from the grid's own "
                  "places (front row of the bottom tier first, outermost first, alternating sides, then the back row, then "
                  "the next tier up) and the Paveway III rows three from their one row, each from where it hangs.",
        ),
        Scenario(
            name="a2g-release-origin-tornado-rail", lane="flight",
            args=tornado_args("200", "3", "AIM9M", "2"),
            timeout=300,
            then=[Step(tornado_args("650", "3", "AIM9M", "2"), timeout=300)],
            check=tornado_rail_problems,
            notes="The Tornado's AIM-9 side rails leave from where they hang at both sweeps: 8.25 ft out and 1.50 ft ahead at 25 degrees (200 kt), and beside the swivelled pylons at 67 degrees (650 kt), 1.25 ft inboard of the Mk 82 packs, left then right (TOR-6b; the inboard flank is TOR-11, John 2026-11-12). Update (TOR-12, 2026-11-12): 6.71 ft out and 1.70 ft ahead at 25 degrees, 7.52 ft out and 1.39 ft aft at 67 degrees, after John's wing review.",
        ),
    ]
