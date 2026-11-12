"""Builds T.O.R.E's authored equipment pod models in Blender (three shipped,
two Tornado IDS pods added 2026-11-12, TOR-13a).

John asked on 2026-11-07 for low-poly models of the AN/ASQ-153 Pave Spike
(PAVESPK.SEE), the AN/AAS-38 NITE Hawk (AAS38.SEE) and the AN/ALQ-167
(ALQ167.ECM), in place of the generic fitted pod body. Fighters Anthology names
no shape for these records, so the models are ours, not retail geometry
(docs/spec/stores-on-pylons.md, EQ-2).

This script is the source of the models: it builds each one as a Blender mesh
from the ring and part tables below, checks it, and can export the triangles
into the Rust table the game draws (crates/tore-app/src/pylons/pod_models.rs).

    blender --background --factory-startup --python assets/pods/build_pods.py -- \
        --export crates/tore-app/src/pylons/pod_models.rs \
        [--blend OUT.blend] [--thumbs OUT_DIR]

Model units: the pod's axis is Blender +Y (forward), from -0.5 (tail) to +0.5
(nose), so 1 is the pod's length; X (right) and Z (up) are in diameters, so the
main body has radius 0.5 at most and its top touches the pylon at Z = 0.5. The
game scales Y by the pod's length and X and Z by its diameter
(tore_sim::combat::store_sizes::equipment_pod_body). Every model stays under
100 triangles, flat shaded, with 6 or 8 sided cylinders as FA draws them.

TOR-13a (2026-11-12): the two RAF Tornado GR1/GR4 outer wing pylon pods, keyed
`SKYSHADOW` and `BOZ107` for now (no retail record exists for them; TOR-13
decides the mapping). John asked for them on 2026-11-12: "keep the budget under
100 triangles, and if we can add a yellow triangle graphic on the side for the
ecm pod that would be good, flare/chaff should come out the back of the
countermeasure pod", and then gave the shape: "The shape for both can look
pretty much identical, they are long teardrop shaped pod with small fins on the
back and the countermeasure pod has a little dispenser back that can just be
tiny slightly darker circles." So both share one teardrop body (`teardrop`).
The outline is John's call; the facts behind the sizes are below. He then
raised these two pods' budget to 130 triangles each (2026-11-12, "just to be
safe"; TRIANGLE_LIMITS); the three older models keep 100. Both are well
inside it anyway.

* Sky Shadow (Marconi, later Selex ES, ARI 23246/1; the GR4's is the Sky
  Shadow 2): an ECM jamming pod, "around 3 metres long" and "around 200 kg"
  (https://en.wikipedia.org/wiki/Sky_Shadow_(radar_jammer), read 2026-11-12).
  No public source found gives its diameter; 0.42 m is OUR FIT, taken from
  the length-to-width of John's photo (below), a 0.14 ratio. Pod-mounted,
  "usually on the wing's outer pylons" (search summary of the same page);
  the RAF bought about 225 for the GR1 fleet.
* BOZ-107 (Bofors Electronics, now Saab; the BOZ-100 family, the British
  variant of the Swedish BOX-9 / BOZ-100; German BOZ-101, Italian BOZ-102): a
  chaff and flare dispenser, listed "~ 4 m" long and "~ 325 kg"
  (https://www.armedconflicts.com/SWE-BOZ-107-vymetnice-klamnych-cilu-t22390,
  read 2026-11-12; a secondary Czech database, approximate), in RAF service
  from mid-1984 (same page); flares are 55 mm Chemring No.4 Mk.2 Type 1
  (https://en.wikipedia.org/wiki/BOZ_counter-measures_pod). A forum post
  quoted in search results gave 80 kg of chaff in 540 packets and 28 flares
  (https://forum.dcs.world/topic/153511-rwr-and-countermeasures-systems/; not
  verified, may be another BOZ-100 variant). Diameter: none found; 0.48 m is
  OUR FIT from the same photo (a 0.12 ratio). Saab's own sheet (BOZ EC, 2024) gives no dimensions.
* Which side (John, 2026-11-12, from his photo of a GR4's underside, labelled
  by him): BOZ-107 on the right wing's outer pylon, Sky Shadow on the left's.
  The shape also follows that photo: both long and slender, much thinner than
  the wing tanks; a pointed nose, widest a little forward of the middle, a
  short cylindrical aft end with small fins (the BOZ's aft end carries the
  dispenser outlets; the Sky Shadow has a fairing bulge on its side, which
  John, 2026-11-12, described as "small bulbs on the left and right side (as
  well as the yellow triangle in front of it)": an elongated faceted bulb
  each side, 0.56 m by 0.20 m by 0.04 m after his later "double the width,
  and quadruple the length, height is fine", the triangle just forward of
  it). Public sources agree on the sides:
  the GR4's countermeasures are "a BOZ-107 pod on the right wing
  to dispense chaff and flares and a Sky Shadow-2 electronic countermeasures
  pod on the left wing" (https://www.panavia.de/aircraft/systems-and-
  capabilities/gr44a); the Haynes RAF Tornado Owner's Workshop Manual agrees
  ("the outer two carry the BOZ-EC ... (often on the right wing), and on the
  port wing the Sky Shadow 2"). So Sky Shadow on the outer port pylon, BOZ on
  the outer starboard one. Forum posts add that a second BOZ (flare opening
  blanked) can ride to port as ballast; not modelled.
* Colour: RAF Tornado grey, taken as Medium Sea Grey (BS 381C 637), a pick:
  about sRGB (0.42, 0.47, 0.50). The game snaps it to its palette. The yellow
  warning triangle is (1.0, 0.82, 0.0); the game's palette may have no yellow
  and take its nearest entry (TOR-13 to check).
"""

import argparse
import math
import sys

import bpy
import bmesh

# Colours, sRGB 0..1. The game gives each the nearest palette entry, so it is
# lit like the airframe. Pave Spike is white: pure white takes the palette's
# brightest entry (255, where 0.92 took 235), and the lighting then shades it
# as it shades a white airframe. The Navy pods are tactical light gray, about
# FS 36375 (between it and FS 36320); details are near black.
WHITE = (1.0, 1.0, 1.0)
NAVY_GRAY = (0.5, 0.53, 0.55)
DARK = (0.13, 0.14, 0.15)
# TOR-13a. RAF Tornado grey: Medium Sea Grey, BS 381C 637, about sRGB this.
RAF_GRAY = (0.42, 0.47, 0.50)
# The BOZ's dispenser outlets: only slightly darker than the body.
DISPENSER_DARK = (0.27, 0.29, 0.31)
YELLOW = (1.0, 0.82, 0.0)

TRIANGLE_LIMIT = 100
# John raised the budget for the two Tornado pods to 130 triangles each on
# 2026-11-12 ("just to be safe"); the first three models keep 100.
TRIANGLE_LIMITS = {"SKYSHADOW": 130, "BOZ107": 130}


def lathe(sides, rings, colors, ends, dark=()):
    """A round body turned about the Y axis.

    `rings` are (y, radius) from nose to tail; a radius of 0 is a point (a
    cone's tip). `colors[i]` colours the band between rings i and i + 1;
    `ends` colours the nose and tail caps (a flat cap where the ring has a
    radius). `dark` lists (band, side) faces drawn DARK instead, where band
    -1 is the nose cap and len(rings) - 1 the tail cap. Side k spans the
    corners k and k + 1; corner 0 is on top, so the top of the body is an
    edge that meets the pylon. Returns (corners, polygons with colours).
    """
    corners = []
    ring_index = []
    for y, radius in rings:
        if radius == 0:
            ring_index.append([len(corners)] * sides)
            corners.append((0.0, y, 0.0))
            continue
        start = len(corners)
        for k in range(sides):
            angle = math.pi / 2 + k * math.tau / sides
            corners.append((radius * math.cos(angle), y, radius * math.sin(angle)))
        ring_index.append(list(range(start, start + sides)))
    polygons = []

    def color_of(band, side, base):
        return DARK if (band, side) in dark else base

    for band in range(len(rings) - 1):
        a, b = ring_index[band], ring_index[band + 1]
        for k in range(sides):
            n = (k + 1) % sides
            # Outward winding (Blender's counter-clockwise front faces).
            quad = [a[k], a[n], b[n], b[k]]
            unique = []
            for q in quad:
                if q not in unique:
                    unique.append(q)
            polygons.append((unique, color_of(band, k, colors[band])))
    nose, tail = rings[0], rings[-1]
    if nose[1] > 0:
        polygons.append((list(reversed(ring_index[0])), color_of(-1, 0, ends[0])))
    if tail[1] > 0:
        polygons.append((list(ring_index[-1]), color_of(len(rings) - 1, 0, ends[1])))
    return corners, polygons, False


def blade(points, color):
    """A thin part (a fin or an antenna) as one flat polygon; the game draws
    both sides."""
    return points, [(list(range(len(points))), color)], True


def block(x, z_bottom, z_top, y_front_top, y_back_top, y_front_bottom, y_back_bottom, color):
    """A raised strip on top of a body: half width `x`, flat top, sloped ends,
    open underneath (it sits in the body)."""
    corners = [
        (-x, y_front_bottom, z_bottom),
        (x, y_front_bottom, z_bottom),
        (x, y_back_bottom, z_bottom),
        (-x, y_back_bottom, z_bottom),
        (-x, y_front_top, z_top),
        (x, y_front_top, z_top),
        (x, y_back_top, z_top),
        (-x, y_back_top, z_top),
    ]
    faces = [
        [4, 7, 6, 5],  # top
        [0, 4, 5, 1],  # front slope
        [2, 6, 7, 3],  # back slope
        [1, 5, 6, 2],  # right side
        [3, 7, 4, 0],  # left side
    ]
    return corners, [(f, color) for f in faces], False


def fin(angle, root_front, root_back, tip_front, tip_back, root, span, color):
    """A trapezoid fin standing out from the body at `angle` (radians from
    +X towards +Z), its root on radius `root`."""
    c, s = math.cos(angle), math.sin(angle)
    out = root + span
    return blade(
        [
            (root * c, root_front, root * s),
            (out * c, tip_front, out * s),
            (out * c, tip_back, out * s),
            (root * c, root_back, root * s),
        ],
        color,
    )


def pave_spike():
    """AN/ASQ-153 Pave Spike: a straight white tube of one diameter from
    end to end (John 2026-11-07: "pretty much a straight cylinder"), a blunt
    faceted seeker head of the same diameter with a dark window on its face,
    a raised collar band at the middle where it hangs, and a blunt tail. Six
    sides: 92 triangles."""
    w = WHITE
    body = 0.45
    return [
        lathe(
            6,
            [
                (0.5, 0.3),  # the window
                (0.46, body),  # the head's faceted rim
                (0.08, body),
                (0.06, 0.5),  # the collar band under the pylon
                (-0.08, 0.5),
                (-0.1, body),
                (-0.47, body),
                (-0.5, 0.32),  # the blunt tail
            ],
            [w] * 7,
            (DARK, w),
        )
    ]


def nite_hawk():
    """AN/AAS-38 NITE Hawk: a fat cylinder with a rounded seeker head, its
    dark window low on the front, a dark band joint behind the head, and a
    blunt tail. Eight sides: 94 triangles."""
    g = NAVY_GRAY
    # Sides 3 and 4 face down (corner 0 is on top).
    window = {(0, 3), (0, 4), (1, 3), (1, 4)}
    return [
        lathe(
            8,
            [
                (0.5, 0.0),  # the head's front
                (0.46, 0.34),
                (0.39, 0.5),
                (0.2, 0.5),
                (0.17, 0.5),  # the band joint
                (-0.43, 0.5),
                (-0.5, 0.3),  # the blunt tail
            ],
            [g, g, g, DARK, g, g],
            (g, g),
            dark=window,
        )
    ]


def alq167():
    """AN/ALQ-167: a long cylinder with blunt conical caps, a raised dark
    fairing strip along the top, small dark antenna stubs under both ends,
    two dark L-shaped prongs at the front, and four small dark fins in an X
    near the tail. Eight sides: 88 triangles."""
    g = NAVY_GRAY
    radius = 0.42
    parts = [
        lathe(
            8,
            [(0.5, 0.2), (0.41, radius), (-0.41, radius), (-0.5, 0.2)],
            [g, g, g],
            (g, g),
        ),
        # The fairing strip: from the body's top ridge up to the pylon.
        block(0.12, 0.34, 0.5, 0.26, -0.1, 0.32, -0.16, DARK),
    ]
    for quarter in range(4):
        angle = math.pi / 4 + quarter * math.pi / 2
        parts.append(fin(angle, -0.2, -0.34, -0.28, -0.33, radius - 0.02, 0.2, DARK))
    # Antenna stubs under the nose and tail.
    for y in (0.36, -0.37):
        parts.append(blade([(0.0, y + 0.03, -0.4), (0.0, y - 0.01, -0.58), (0.0, y - 0.03, -0.4)], DARK))
    # The two L-shaped prongs at the front, low on either side: out from the
    # body, then forward to just short of the nose (John 2026-11-07).
    for angle in (-math.pi / 6, math.pi + math.pi / 6):
        c, s = math.cos(angle), math.sin(angle)
        outline = [(0.38, 0.33), (0.56, 0.33), (0.56, 0.49), (0.5, 0.49), (0.5, 0.37), (0.38, 0.37)]
        parts.append(blade([(r * c, y, r * s) for r, y in outline], DARK))
    return parts


# The teardrop body both Tornado pods share (John 2026-11-12, and his photo of
# a GR4's underside): (y, radius) nose to tail: a pointed nose cone, widest a
# little forward of the middle (y = 0.1), a taper, then a short cylindrical
# aft end. The tail ring sits at -0.49 so the dispenser outlets, just proud of
# the aft face, end at exactly -0.5.
TEARDROP = [(0.5, 0.0), (0.34, 0.33), (0.1, 0.5), (-0.18, 0.46), (-0.32, 0.38), (-0.49, 0.38)]

# Where chaff and flares leave the BOZ pod, for TOR-13 to emit them from: the
# centre of the aft face, in model units (x right in diameters, y forward in
# lengths, z up in diameters); they leave along -Y (straight aft). The nine
# outlets are centred on it: one on the point, eight round it.
DISPENSER_EXIT = (0.0, -0.5, 0.0)


def teardrop_radius(y):
    """The body's radius at `y`, between the rings."""
    for (y0, r0), (y1, r1) in zip(TEARDROP, TEARDROP[1:]):
        if y1 <= y <= y0:
            return r1 + (r0 - r1) * (y - y1) / (y0 - y1)
    raise ValueError(y)


def facing(points, outward):
    """`points` wound so the polygon's normal faces `outward`."""
    (ax, ay, az), (bx, by, bz), (cx, cy, cz) = points[:3]
    u = (bx - ax, by - ay, bz - az)
    v = (cx - bx, cy - by, cz - bz)
    n = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
    dot = sum(a * b for a, b in zip(n, outward))
    return list(points) if dot > 0 else list(reversed(points))


def tornado_fins(color):
    """Four small swept fins in an X on the taper, over the body's corners 1,
    3, 5 and 7 (never on top, where the pylon is)."""
    parts = []
    for quarter in range(4):
        angle = math.pi / 4 + quarter * math.pi / 2
        c, s = math.cos(angle), math.sin(angle)
        out = 0.52
        points = [
            (teardrop_radius(-0.26) * c, -0.26, teardrop_radius(-0.26) * s),
            (out * c, -0.4, out * s),
            (out * c, -0.5, out * s),
            (teardrop_radius(-0.49) * c, -0.49, teardrop_radius(-0.49) * s),
        ]
        parts.append(blade(points, color))
    return parts


def teardrop_body(color):
    """The shared teardrop: 8 sides, 5 bands, a point at the nose and a flat
    tail cap (78 triangles)."""
    return lathe(8, TEARDROP, [color] * 5, (color, color))


def facet(side):
    """The teardrop's long facet `side` (band 2, between the widest ring and
    the next): returns at(u, v), mapping u along the facet from corner `side`
    to the next (circumferential, v along the band from front 0 to back 1) to
    a point on it, and the outward unit normal."""
    (y0, r0), (y1, r1) = TEARDROP[2], TEARDROP[3]
    sides = 8

    def corner(k, ring_y, radius):
        a = math.pi / 2 + k * math.tau / sides
        return (radius * math.cos(a), ring_y, radius * math.sin(a))

    c00, c10 = corner(side, y0, r0), corner(side + 1, y0, r0)
    c01, c11 = corner(side, y1, r1), corner((side + 1) % sides, y1, r1)

    def at(u, v):
        top = [c00[i] + (c10[i] - c00[i]) * u for i in range(3)]
        bottom = [c01[i] + (c11[i] - c01[i]) * u for i in range(3)]
        return [top[i] + (bottom[i] - top[i]) * v for i in range(3)]

    e1 = [c10[i] - c00[i] for i in range(3)]
    e2 = [c01[i] - c00[i] for i in range(3)]
    n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]]
    length = math.sqrt(sum(x * x for x in n))
    n = [x / length for x in n]
    if sum(a * b for a, b in zip(n, at(0.5, 0.5))) < 0:
        n = [-x for x in n]
    return at, n


# Side 6 faces right and side 1 faces left, each 22.5 degrees above the
# horizontal. Going up the right facet runs corner 6 to 7 (u rising); on the
# left it runs corner 1 to 2, downward.
SIDE_FACETS = ((6, True), (1, False))


def facet_triangle(side, rises):
    """A yellow warning triangle (point up, base along the pod) lying on the
    long facet `side`, 0.008 proud, forward of the bulb (v 0.08 to 0.25, y 0.08 to 0.03; the bulb starts
    at y -0.007).
    `rises` is True where the corner-to-corner direction rises (side 6)."""
    at, n = facet(side)
    base_u, apex_u = (0.06, 0.94) if rises else (0.94, 0.06)
    v_low, v_high = 0.08, 0.25
    points = [at(base_u, v_low), at(base_u, v_high), at(apex_u, (v_low + v_high) / 2)]
    points = [tuple(p[i] + n[i] * 0.008 for i in range(3)) for p in points]
    points = facing(points, n)
    return points, [([0, 1, 2], YELLOW)], False


# The Sky Shadow's side bulbs (John 2026-11-12: "double the width, and
# quadruple the length, height is fine" on a 0.14 by 0.10 by 0.04 m blister):
# 0.56 m along the 3.0 m pod, 0.20 m around the 0.42 m pod, 0.04 m proud.
BULB_LENGTH_M, BULB_WIDTH_M, BULB_HEIGHT_M = 0.56, 0.20, 0.04
POD_LENGTH_M, POD_DIAMETER_M = 3.0, 0.42
BULB_CENTRE_Y = -0.10  # mid-body, just behind the widest ring


def surface(phi, y, sink=0.0):
    """The point of the 8-sided body's surface at azimuth `phi` (radians from
    +X towards +Z) and height `y`, `sink` inside it."""
    facet_width = math.tau / 8
    m = round((phi - facet_width / 2) / facet_width)
    d = phi - facet_width / 2 - m * facet_width  # from the facet's middle
    rho = teardrop_radius(y) * math.cos(facet_width / 2) / math.cos(d) - sink
    return (rho * math.cos(phi), y, rho * math.sin(phi))


def facet_bulb(side, color):
    """An elongated faceted blister on the body's side, centred on the long
    facet `side`'s middle: 10 triangles (an 8 point outline on the surface, a
    two point ridge). It is larger than a facet, so it crosses the facet edges
    and the ring at y = -0.18; the outline is kept on the surface (0.005 sunk,
    and every chord between outline points lies inside the convex body), so it
    seats with no gap and shows no floating edge. 0.56 m long, 0.20 m wide,
    0.04 m proud (the ridge) on the 3.0 by 0.42 m pod."""
    phi_c = math.pi / 2 + (side + 0.5) * math.tau / 8
    half_len = BULB_LENGTH_M / POD_LENGTH_M / 2
    # Half the width, as an angle about the axis at the pod's widest radius.
    half_width = (BULB_WIDTH_M / POD_DIAMETER_M / 2) / 0.5
    height = BULB_HEIGHT_M / POD_DIAMETER_M
    outline = []
    for j in range(8):
        t = j * math.tau / 8
        outline.append(
            surface(phi_c + half_width * math.cos(t), BULB_CENTRE_Y + half_len * math.sin(t), 0.005)
        )
    radial = (math.cos(phi_c), 0.0, math.sin(phi_c))
    ridge = []
    for sign in (1, -1):
        y = BULB_CENTRE_Y + sign * half_len * 0.5
        base = surface(phi_c, y)
        ridge.append(tuple(base[i] + radial[i] * height for i in range(3)))
    front, back = ridge
    verts = outline + [front, back]
    f, b = 8, 9
    faces = [
        (0, 1, f), (1, 2, f), (2, 3, f), (3, 4, f),
        (4, 5, b), (5, 6, b), (6, 7, b), (7, 0, b),
        (f, 4, b), (f, b, 0),
    ]
    inside = tuple(
        (front[i] + back[i]) / 2 - radial[i] * height * 0.5 for i in range(3)
    )
    polygons = []
    for face in faces:
        tri = [verts[i] for i in face]
        mid = [sum(p[i] for p in tri) / 3 for i in range(3)]
        out = [mid[i] - inside[i] for i in range(3)]
        order = facing(tri, out)
        polygons.append(([verts.index(p) for p in order], color))
    return verts, polygons, False


def dispenser_outlets(color):
    """Nine tiny hexagonal discs on the aft face, 0.006 proud of it: one on
    DISPENSER_EXIT and a ring of eight round it (radius 0.24, each disc radius
    0.065, so they touch neither each other nor the face's edge at 0.35):
    the BOZ's chaff and flare outlets (36 triangles). John first had three, then
    asked on 2026-11-12 for "about triple the little grey things in the back"."""
    x0, y0, z0 = DISPENSER_EXIT
    centres = [(x0, z0)]
    for k in range(8):
        a = math.pi / 2 + k * math.tau / 8
        centres.append((x0 + 0.24 * math.cos(a), z0 + 0.24 * math.sin(a)))
    corners, polygons = [], []
    for cx, cz in centres:
        ring = [
            (cx + 0.065 * math.cos(j * math.tau / 6), y0, cz + 0.065 * math.sin(j * math.tau / 6))
            for j in range(6)
        ]
        ring = facing(ring, (0.0, -1.0, 0.0))
        start = len(corners)
        corners.extend(ring)
        polygons.append((list(range(start, start + 6)), color))
    return corners, polygons, False


def sky_shadow():
    """Marconi/Selex Sky Shadow (ARI 23246) ECM pod, left (port) outer pylon:
    the shared teardrop in RAF grey with four small fins, and on each side a
    small faceted bulb mid-body with a yellow warning triangle just forward of
    it (John 2026-11-12). The bulbs are 0.56 m long, 0.20 m wide and 0.04 m
    proud (John 2026-11-12: first 0.14 by 0.10 by 0.04, then "double the
    width, and quadruple the length, height is fine"). 3.0 m by 0.42 m
    (fitted). Triangles: 78 body, 8 fins, 20 bulbs (2 x 10), 2 yellow =
    108."""
    g = RAF_GRAY
    parts = [teardrop_body(g)]
    parts.extend(tornado_fins(g))
    for side, rises in SIDE_FACETS:
        parts.append(facet_bulb(side, g))
        parts.append(facet_triangle(side, rises))
    return parts


def boz107():
    """BAe/Bofors (now Saab) BOZ-107 chaff and flare dispenser, starboard outer
    pylon: the shared teardrop in RAF grey with the same four small fins and,
    on the aft face, nine tiny slightly darker discs, the dispenser outlets
    (one on DISPENSER_EXIT, a ring of eight round it; John 2026-11-12 asked
    for triple the first three). 4.0 m by 0.48 m (fitted). Triangles: 78
    body, 8 fins, 36 outlets = 122."""
    g = RAF_GRAY
    parts = [teardrop_body(g)]
    parts.extend(tornado_fins(g))
    parts.append(dispenser_outlets(DISPENSER_DARK))
    return parts


# Record, object name, builder, and diameter over length (the game's sizes,
# equipment_pod_body), used only to proportion the thumbnails.
MODELS = [
    ("PAVESPK.SEE", "pave_spike", pave_spike, 1.1 / 12.3),
    ("AAS38.SEE", "nite_hawk", nite_hawk, 1.08 / 6.0),
    ("ALQ167.ECM", "alq167", alq167, 1.4 / 12.0),
    # TOR-13a placeholders: real length 3.0 m and 4.0 m, fitted diameters 0.42 m and 0.48 m.
    ("SKYSHADOW", "sky_shadow", sky_shadow, 0.42 / 3.0),
    ("BOZ107", "boz107", boz107, 0.48 / 4.0),
]


def material(color):
    name = "pod_%02x%02x%02x" % tuple(round(c * 255) for c in color)
    found = bpy.data.materials.get(name)
    if found is None:
        found = bpy.data.materials.new(name)
        found.diffuse_color = (*color, 1.0)
    return found


def build(name, parts):
    """One Blender mesh object from the parts, a material per colour."""
    mesh = bpy.data.meshes.new(name)
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(obj)
    bm = bmesh.new()
    thin = bm.faces.layers.int.new("blade")
    colors = []
    for corners, polygons, is_blade in parts:
        verts = [bm.verts.new(c) for c in corners]
        for indices, color in polygons:
            if color not in colors:
                colors.append(color)
                mesh.materials.append(material(color))
            face = bm.faces.new([verts[i] for i in indices])
            face.material_index = colors.index(color)
            face.smooth = False
            face[thin] = int(is_blade)
    bm.to_mesh(mesh)
    bm.free()
    mesh.update()
    return obj


def check(obj):
    """Triangle count, size and normals of a built model."""
    mesh = obj.data
    mesh.calc_loop_triangles()
    triangles = len(mesh.loop_triangles)
    xs = [v.co.x for v in mesh.vertices]
    ys = [v.co.y for v in mesh.vertices]
    zs = [v.co.z for v in mesh.vertices]
    # Closed bodies wind outward. Thin blades are drawn from both sides.
    inward = 0
    blades = mesh.attributes["blade"].data
    for poly in mesh.polygons:
        if blades[poly.index].value:
            continue
        c, n = poly.center, poly.normal
        # A cap faces along the axis; a band faces away from it.
        if abs(n.y) > 0.999:
            outward = n.y * c.y
        else:
            outward = n.x * c.x + n.z * c.z
        if outward < -1e-6:
            inward += 1
    return {
        "name": obj.name,
        "triangles": triangles,
        "length": max(ys) - min(ys),
        "y": [min(ys), max(ys)],
        "x": [min(xs), max(xs)],
        "z": [min(zs), max(zs)],
        "inward_faces": inward,
    }


def export(objects, path):
    """Writes the models' triangles into the game's Rust table."""
    out = [
        "//! Authored equipment pod models (John 2026-11-07, docs/spec/stores-on-pylons.md,",
        "//! EQ-2). Generated by assets/pods/build_pods.py from its Blender meshes; edit",
        "//! the script and export again, never this file.",
        "//!",
        "//! Corners are [right, up, forward]: forward in pod lengths (-0.5 tail to 0.5",
        "//! nose), right and up in pod diameters. Colours are sRGB, 0 to 1.",
        "",
        "/// One authored model: the equipment record it draws and its triangles.",
        "pub(crate) struct PodModel {",
        "    pub record: &'static str,",
        "    pub triangles: &'static [([[f32; 3]; 3], [f32; 3])],",
        "}",
        "",
        "pub(crate) const MODELS: [PodModel; %d] = [" % len(objects),
    ]

    def num(v):
        text = "%.4f" % v
        text = text.rstrip("0")
        if text.endswith("."):
            text += "0"
        return "0.0" if text in ("-0.0", "0.0") else text

    for record, obj in objects:
        mesh = obj.data
        mesh.calc_loop_triangles()
        out.append("    PodModel {")
        out.append('        record: "%s",' % record)
        out.append("        triangles: &[")
        for tri in mesh.loop_triangles:
            corners = []
            for i in tri.vertices:
                co = mesh.vertices[i].co
                corners.append("[%s, %s, %s]" % (num(co.x), num(co.z), num(co.y)))
            color = mesh.materials[tri.material_index].diffuse_color
            out.append(
                "            ([%s], [%s, %s, %s]),"
                % (", ".join(corners), num(color[0]), num(color[1]), num(color[2]))
            )
        out.append("        ],")
        out.append("    },")
    out.append("];")
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(out) + "\n")


def thumbnails(objects, directory, views=False):
    """A small flat-shaded render of each model from the front quarter. With
    `views`, also side, other-side, underside, rear and close-up pictures
    (NAME-side.png and so on)."""
    import os
    from mathutils import Vector

    os.makedirs(directory, exist_ok=True)
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "MATERIAL"
    scene.display.shading.show_object_outline = True
    scene.render.resolution_x = 640
    scene.render.resolution_y = 320
    scene.render.film_transparent = False
    world = bpy.data.worlds.new("pods") if scene.world is None else scene.world
    scene.world = world
    camera = bpy.data.cameras.new("pods")
    camera.type = "ORTHO"
    camera.ortho_scale = 1.15
    eye = bpy.data.objects.new("pods_camera", camera)
    scene.collection.objects.link(eye)
    scene.camera = eye

    def shoot(obj, suffix, direction, scale, target=(0.0, 0.0, 0.0)):
        """`direction` points from the target to the eye."""
        camera.ortho_scale = scale
        d = Vector(direction).normalized()
        eye.location = Vector(target) + d * 3.0
        eye.rotation_euler = (-d).to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.join(directory, obj.name + suffix + ".png")
        bpy.ops.render.render(write_still=True)

    def centre_of(obj, color):
        """World centre of the faces painted `color`, or None."""
        mesh = obj.data
        found = [
            i
            for i, m in enumerate(mesh.materials)
            if all(abs(x - y) < 1e-4 for x, y in zip(m.diffuse_color, color))
        ]
        points = [
            Vector(mesh.vertices[v].co)
            for p in mesh.polygons
            if p.material_index in found
            for v in p.vertices
        ]
        if not found or not points:
            return None
        mean = sum(points, Vector()) / len(points)
        return Vector((mean.x * obj.scale.x, mean.y, mean.z * obj.scale.z))

    for _, obj in objects:
        for _, other in objects:
            other.hide_render = other is not obj
        # Drawn at the real pod's proportions, as the game scales it.
        obj.scale = (obj["slender"], 1.0, obj["slender"])
        shoot(obj, "", (1.6, 1.0, 0.6), 1.15)
        if not views:
            continue
        shoot(obj, "-side", (1.0, 0.0, 0.36), 1.15)
        shoot(obj, "-side-left", (-1.0, 0.0, 0.36), 1.15)
        shoot(obj, "-under", (0.25, 0.0, -1.0), 1.15)
        shoot(obj, "-rear", (0.6, -1.0, 0.35), 0.3, (0.0, -0.45, 0.0))
        yellow = centre_of(obj, YELLOW)
        if yellow is not None:
            shoot(obj, "-triangle", (1.0, 0.0, 0.41), 0.2, (yellow.x, yellow.y - 0.04, yellow.z))
        outlets = centre_of(obj, DISPENSER_DARK)
        if outlets is not None:
            shoot(obj, "-dispenser", (0.0, -1.0, 0.0), 0.2, (0.0, -0.5, 0.0))


def main(argv):
    parser = argparse.ArgumentParser()
    parser.add_argument("--export")
    parser.add_argument("--blend")
    parser.add_argument("--thumbs")
    parser.add_argument("--views", action="store_true", help="more angles with --thumbs")
    args = parser.parse_args(argv)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    objects = []
    report = []
    for record, name, parts, slender in MODELS:
        obj = build(name, parts())
        obj["slender"] = slender
        info = check(obj)
        info["record"] = record
        report.append(info)
        limit = TRIANGLE_LIMITS.get(record, TRIANGLE_LIMIT)
        if info["triangles"] >= limit:
            raise SystemExit("%s has %d triangles (limit %d)" % (name, info["triangles"], limit))
        if info["inward_faces"]:
            raise SystemExit("%s has %d faces wound inward" % (name, info["inward_faces"]))
        objects.append((record, obj))
    if args.export:
        export(objects, args.export)
    if args.thumbs:
        thumbnails(objects, args.thumbs, args.views)
    if args.blend:
        bpy.ops.wm.save_as_mainfile(filepath=args.blend)
    return report


if __name__ == "__main__":
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    for line in main(argv):
        print("POD", line)
