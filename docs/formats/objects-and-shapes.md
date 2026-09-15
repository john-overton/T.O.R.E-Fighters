# Working with objects and SH shapes

Research checkpoint: 2026-09-15. This guide adds practical object/shape guidance
from Plurry's **The Explanation of FA Shape files, v2.01 (2026/09)** in the
user-supplied `.local/fa-shape-file-explination/` folder. It combines that
community research with checks of its F18C examples and our current readers.
It does not implement an object editor or expand supported aircraft.
[Review evidence and exact input identity](../baselines/shape-reference-review.md).

Read [aircraft](aircraft.md), [weapons](weapons.md), and [theater](theater.md)
for their respective runtime contracts. F18C is an example asset here;
the supported Hornet remains **F18.PT, F/A-18D**.

## 1. Choose the layer that owns the change

An object is more than its visible mesh. Keep these concerns separate when
investigating or eventually editing it:

| Intended change | Starting point | What the shape alone cannot establish |
| --- | --- | --- |
| Appearance, skin, moving visual parts | SH drawing program and referenced PICs | Flight response or equipment operation |
| Aircraft identity, equipment, hardpoints | PT and its referenced definitions | Native animation timing or muzzle rendering |
| Weapon behavior and sensor properties | JT / SEE / ECM and native consumers | Whole lifecycle parity from a decoded scalar |
| Damaged appearance or ground shadow | Explicit definition references and relevant SH variants | Damage thresholds, variant selection or collision geometry |
| Put a building, ship or aircraft in a mission | M/MM object records, type resolution and native placement consumers | Position, allegiance, objectives or spawning from an SH filename |
| Wing vapor attachment | SH CE record plus streamer consumers | Emission conditions and trail lifetime from coordinates alone |

Our M/MM reader currently reads environment/tmap fields and skips indented
object fields. Importing a theater or finding its object names does not place
those objects in the world. An eventual object integration must resolve the
type, its visual dependencies, placement and mutable simulation state separately.
This is a proposed development workflow, not a recovered native object manager.

### Main, damaged and shadow resources

The introductory guide describes common families: one main shape for simple
objects; a main/damaged pair for some ships and ground targets; and an aircraft
family with main, `_A` through `_D` damage shapes and `_S` shadow shape. It gives
RIG2/RIG2_A, TICON/TICON_A and shared TANK_A as ground/ship examples.
These are **reference naming patterns, not a universal resolver rule**.

The supplied textual F18C.PT has separate `shape` and `shadowShape` references.
Follow actual definition references and verified native selection rules. Do not
infer a damage progression from alphabetical suffixes, require six files for
every aircraft, or use a visual mesh as a validated collision/carrier surface.
The guide's suggestion that some buildings contain damage geometry internally
is explicitly a hypothesis. Carrier internals remain outside this review.

Keep archive provenance and title/version identity with each resource. Identical
filenames in different editions need not contain the same model. A visually
preferred model from another title is not a faithful replacement for FA data.

## 2. Understand the shape before flattening it

SH is an inert PL/PE module with a drawing program, data, and sometimes embedded
x86 re-entry blocks. It is not just a list of polygons. Read section tables;
the guide's 512/1024-byte header categories are observations, not a parser rule.
Never load or execute the imported module.

The guide uses **LOD** for distinct distance/detail models and **sub-LOD** for
component groups within one such model. A sub-LOD can be a door, flap, or fixed
piece of fuselage; it is not necessarily another distance threshold.

The supplied OBJ examples contain:

| Export | Named objects | Vertices | Faces |
| --- | ---: | ---: | ---: |
| F18C_LOD_0.obj | 17 | 452 | 326 |
| F18C_LOD_1.obj | 3 | 215 | 172 |
| F18C_LOD_2.obj | 1 | 97 | 60 |

These are counts of exported records, **not simultaneously visible retail
polygons**. The near export includes both raised/lowered flap groups and enabled
device geometry. It has no `vt` or `vn` records; it is useful for spatial study,
not a complete texture/normal/animation round trip. Seven near-detail groups
also have offset comments that must be understood before moving their vertices.
Distance thresholds and state-dependent visibility still need native validation.

### Fixed, switched and rotating parts

The guide distinguishes three useful cases:

- A fixed component contributes geometry at the body origin.
- A switched component appears/disappears or selects an alternate mesh. An
  extended flap mesh does not prove that retail smoothly rotates the closed one.
- A transformed component has local geometry and a connection point. Landing
  gear and its doors can each have separate pivots and native angle arithmetic.

For the supplied F18C, the left main gear's C4 record starts at file offset
15374. Its translation tuple in record order is `(-8, -6, -7)` and its relative
target is 16398. The target begins the gear vertex block after a separate
14-byte header. The guide labels C4 coordinates X/Z/Y; the current projector
maps its second and third words into the corresponding internal axes. Do not
apply that permutation indiscriminately to vertex, PT, normal and world data:
document the conversion for each record type.

Current `shape.rs` applies C4 translation and restores the calling transform
and texture on return. It does not interpret the full native rotation program.
The app's F18 and Rafale rigs supply fitted animation separately. A future port
must recover per-part axis, sign, scale and state inputs; the presence of
`_PLgearPos` alone does not establish the animation law.

The guide claims animated face centroids already include attachment offsets.
Treat that as a per-record research lead: verify the native consumer before
offsetting a centroid a second time. Positions translate; normal directions
rotate but do not translate.

## 3. Geometry, materials and shading

### Vertex buffers and face lengths

Opcode 82 has a vertex count and destination field, followed by signed 16-bit
coordinate triples. The guide calls the destination unknown. Our reader already
uses it as a byte offset into eight-byte vertex slots: slot = destination / 8.
It rejects misaligned destinations. Component blocks can populate shared slots;
do not restart face indices at zero for every component.

For FC faces, the second flag byte controls the stored widths:

| Mask in second flag byte | Clear | Set |
| --- | --- | --- |
| 0x04 | 8-bit vertex indices | 16-bit vertex indices |
| 0x02 | 16-bit signed face center components | 8-bit signed face center components |
| 0x01 | 16-bit UV components | 8-bit UV components |

These are index widths, not the width of the polygon vertex-count field. Our
reader consumes a byte count and bounds it to 3–64. The guide's 3–8 range is not
adopted as a universal limit. Normal/center and texture payload presence also
depend on the first flag byte; widths alone cannot determine the record length.

Changing an index from 255 to 256, or moving a UV beyond its byte range, can
therefore change a record's size. Recompute offsets and links, rather than
patching a larger value into the old field. The reference's material combinations
are useful examples, but not proof that arbitrary flag combinations are valid.

### Texture state and decals

E2 selects a named texture for following faces (14-byte bounded name payload in
our reader). E0 selects a runtime texture slot. The guide identifies slots
0/1 as left/right tail art, 2 as nose art, and 3/4 as left/right wing markings;
their names need not describe where every aircraft uses them.

An E2 after an E0 restores the main skin. Keep this state transition when
reordering geometry. Our projector clears its named texture at E0 and omits
textured faces with no resolved texture; it does not implement those five decal
slots. Do not fill an unresolved decal with a piece of the main skin.

Keep palette indices and original atlas dimensions. Texture cutout, painted
background fill, and partial-opacity shadows are different mechanisms. The
guide associates shadow groups with B8 enable/disable records plus an FC shadow
byte. Our static projector skips B8 and does not preserve that shadow byte as a
complete shadow material. Importing `_S.SH` is not shadow-rendering acceptance.

### Normals: an important export discrepancy

The face guide and normals workbook correct the older YAML export's split into
three 8-bit normals and three residuals. FC stores **three signed 16-bit normal
components**. The workbook gives 32765 as the unit scale, with axis-aligned
examples at ±32765. Our reader already retains signed 16-bit FC values; do not
replace them with the old YAML interpretation. Exact generation/rounding rules
are not established by this workbook alone.

F6 is a different record: vertex index, palette color, and three signed byte
normal components. The guide describes a 127 scale and warns that inconsistent
vertex colors can produce unwanted color interpolation. Our projector retains
the color but skips the three normal bytes. Current face lighting is not full
native Gouraud shading. Preserve both kinds of normals in future tooling rather
than treating them as interchangeable or discarding the original integers.

## 4. Internal connections and editing hazards

The examples make offset bookkeeping much clearer. Distinguish file offsets,
CODE offsets, and module RVAs. For the supplied sample only, CODE starts at
file 0x400 with RVA 0x1000, so `file = RVA - 0x1000 + 0x400` within that section.
Use the real section mapping for other modules.

Relative links are based at specified positions, often the end of the relevant
record. Verified sample examples are:

| Record | Calculation in file offsets | Target |
| --- | --- | ---: |
| F2 at 1038 | 1038 + 4 + 26974 | 28016 |
| C8 at 1115 | 1115 + 8 + 24102 | 25225 |
| C8 at 1123 | 1123 + 8 + 17370 | 18501 |
| C4 at 15374 | 15374 + 16 + 1008 | 16398 |

The guide calls some markers E1 in prose but prints **1E** in its examples.
Use the bytes. Do not use marker searching as a substitute for bounded parsing:
the gear example targets beyond an intervening header, and native return C3
bytes belong to a different grammar.

The so-called “Z-buffer structures” chapter proposes visibility/order planes
and links between groups. It expressly leaves their complete operation unknown.
Its proposed regrouping of 12/38 and 6C export rows is a research lead, not a
proven correction to our interpreter. Our reader follows 12/38 scopes and skips
several plane records while GPU depth resolves overlap. Recover native branch
conditions/order before claiming software visibility parity.

For any future writer, changing geometry can require updating shared vertex
slots, face lengths, relative branch targets, native re-entry addresses,
relocations, section sizes, normals, centers and visibility planes. An OBJ
re-export alone cannot preserve that contract. A safe first writer would need
an unchanged-input byte round trip and explicit rejection of unhandled records.
No such general writer is implemented here.

## 5. Attachment points are not interchangeable

The user reports misplaced vapor attachments and exterior ordnance in the
implementation (2026-09-15). Vapor attachment axes are now corrected as recorded
below; exterior-store placement remains open. Check source units, record-specific
axis order, model scale and body/component transforms for each supported aircraft. The sample discrepancy below does not
by itself identify the cause or justify copying F18C coordinates to another model.

**Wing vapor:** chapter 03-10 guesses at 16-bit coordinates and separator/sign
bytes. Our existing CE reader and native streamer research supersede that
guess: it reads signed 32-bit 24.8 fixed-point positions, a hinge pivot/scale,
and two attachment points with explicit side mirroring. The supplied SH's raw
points decode to `(-54, 1, -17)` and `(55, 0, -16)` before hinge/side processing.
Those differ from the chapter's printed `(-64, 1, -27)` and `(63, 1, -27)`.
The document example and sample must not be treated as the same revision.
See [weather research](weather.md) for the recovered emission subsystem.

**Gun origin:** chapter 03-11 points to the cannon's PT hardpoint, not SH
vertices. It then proposes changing coordinates to match a real aircraft. That
is an authored modification, not corrected retail data. Preserve source values
for classic fidelity and trace the launch consumer before claiming muzzle-origin
parity. Also note that this folder's F18C.PT is a textual BRF representation;
its presence does not mean arbitrary retail PT bytes can be edited as text.

## 6. Practical investigation workflow

1. Identify the requested object and title. Record definition/shape hashes and
   archive boundaries before comparing exports or borrowing another variant.
2. Extract through [the shared extraction tool](../EXTRACTION.md). Resolve
   explicit main/shadow/equipment/texture dependencies; record missing references.
3. Inspect the module as data. For the supplied example:

   ```sh
   mkdir -p .local/shape-doc-review
   python3 tools/inspect_shape_effects.py ".local/fa-shape-file-explination/03 - Sample Files/F18C.SH" > .local/shape-doc-review/effects.json
   ```

   This inventories imports and candidate re-entry sites, not a control-flow
   proof. The bundled Windows tools were not run during this review.
4. Compare binary bytes, annotated workbook, raw CSV, semantic YAML and OBJ.
   Keep an offset map and a list of disagreements. Validate record boundaries
   against bytes before accepting a tool's labels.
5. Describe the requested change at its owning layer: geometry, texture,
   visual state, equipment, or mission placement. Keep mutable damage/device
   state separate from the imported definition and rendering interpolation.
6. Before implementation acceptance, use synthetic malformed-input tests and
   known native views/states. Check neutral/deployed parts, multiple detail
   levels, damage/shadow selection and texture edges as applicable. Rendering
   work also needs creator/viewer smoke tests and aircraft camera checks.

This review schedules no new aircraft, ground-object renderer, editor, AI or
mission systems. Those remain subject to their existing roadmap gates.

### Supported-aircraft vapor correction — 2026-09-15

Native CE attachment vectors use **right/up/forward**, while mesh vertices use
right/forward/up. Applying this distinction to F18.SH and RAFALE.SH gives exact
vertex matches for all four attachments, with the existing one-third-foot scale.
Heading hinges rotate the right/forward plane, preserving up. This fixes vapor
placement; exterior-store placement remains a separate open investigation.
[Evidence](../baselines/wind-turbulence-vapor.md).
