"""Builds T.O.R.E's three authored equipment pod models in Blender.

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

TRIANGLE_LIMIT = 100


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


# Record, object name, builder, and diameter over length (the game's sizes,
# equipment_pod_body), used only to proportion the thumbnails.
MODELS = [
    ("PAVESPK.SEE", "pave_spike", pave_spike, 1.1 / 12.3),
    ("AAS38.SEE", "nite_hawk", nite_hawk, 1.08 / 6.0),
    ("ALQ167.ECM", "alq167", alq167, 1.4 / 12.0),
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


def thumbnails(objects, directory):
    """A small flat-shaded render of each model from the front quarter."""
    import os

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
    for _, obj in objects:
        for _, other in objects:
            other.hide_render = other is not obj
        # Drawn at the real pod's proportions, as the game scales it.
        obj.scale = (obj["slender"], 1.0, obj["slender"])
        eye.location = (1.6, 1.0, 0.6)
        direction = (-eye.location[0], -eye.location[1], -eye.location[2])
        from mathutils import Vector

        eye.rotation_euler = Vector(direction).to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.join(directory, obj.name + ".png")
        bpy.ops.render.render(write_still=True)


def main(argv):
    parser = argparse.ArgumentParser()
    parser.add_argument("--export")
    parser.add_argument("--blend")
    parser.add_argument("--thumbs")
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
        if info["triangles"] >= TRIANGLE_LIMIT:
            raise SystemExit("%s has %d triangles" % (name, info["triangles"]))
        if info["inward_faces"]:
            raise SystemExit("%s has %d faces wound inward" % (name, info["inward_faces"]))
        objects.append((record, obj))
    if args.export:
        export(objects, args.export)
    if args.thumbs:
        thumbnails(objects, args.thumbs)
    if args.blend:
        bpy.ops.wm.save_as_mainfile(filepath=args.blend)
    return report


if __name__ == "__main__":
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    for line in main(argv):
        print("POD", line)
