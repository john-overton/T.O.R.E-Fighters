# Working with objects and SH shapes

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

The [F/A-XX concept](../spec/fa-xx.md) is built on the F-22N since 2026-09-22
and omits F22N.SH fin faces 361d/3670/38e4/3903/3926 at runtime. The reviewed
base layout is still validated by the F-22N rig before use. Its split leaves
reuse the existing inboard flap faces 4477/4496/44f3/450e/466e/4691/46ee and
their texture coordinates; the imported source data is unchanged. Damaged-body
fin faces are F22N_A.SH 3366/3389 and F22N_C.SH 2ac0/2ae3/2c8b/2cae.
The concept inherits the F-22N's C/D damage selection; D contains no vertical fin.
The original-format exporter also removes the indexed decal faces 3644 and 3954
from the intact F22N.SH. They use texture-index slots 1 and 0 respectively, with
source z=9..25 on the fin planes. The gameplay projection omits indexed materials;
export validation retains their geometry with unresolved material labels so
that separate fin artwork cannot evade its removal checks. The F-22N's own hook
is the `_PLhook` word 5e9a branch, two coplanar faces 40a1/40c0 with root at
source y=-11..-7, z=-9 and deployed tip at z=-23. The superseded F-22A
addresses were fins 34f7/354a/37ff/381e/3841, decals 351e/386f, flaps
437f/439e/43fb/4416/4576/4599/45f6, F22_A 3365/3388 and F22_C
2abf/2ae2/2c8a/2cad; they were matched to the F-22N faces by identical geometry.

> **Research notes, research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Requirements, gates and remaining
> work described here are research-mode scope; they are not acceptance gates
> for gameplay. Parity is measured by expression of feature, see
> [AGENTS.md](../../AGENTS.md). Player-visible behaviour is specified in
> [docs/spec/](../spec/).


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

The [Ukraine airport inventory](airport-placements.md) now identifies the first
runway/building dependency set and its placement limits.

The runtime importer follows base-layout placements through their OBJ_TYPE
prefix to explicit SH and projected PIC references. Static geometry uses the SH
CODE header exponent for rendering and contact. Unsupported shape opcodes are
diagnosed while their placement identity remains available.

The environment reader still handles environment/tmap fields independently.
The bounded mission reader now parses object blocks for static scene construction;
see [airport placements](airport-placements.md). The separate bounded STRIP reader can inspect one isolated
eight-field placement record ([contract](native-strip.md#bounded-isolated-placement-ne-011b));
that diagnostic remains separate from the runtime scene loader. The current
integration resolves type, visual dependencies, placement and mutable simulation
state separately. Asset presence alone does not establish complete shape or
behavior coverage; this is TORE's object model, not the original object manager.

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
No such general writer is implemented here. The external OpenFA compiler has
passed exact F22 and V22 round trips in the [F/A-XX packaging review](../baselines/fa-xx-packaging.md).
The [F/A-XX export adapter](../spec/fa-xx-export.md) now writes reviewed donor
modifications and validates decoded poses. Original-game loading remains unverified.

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

### Supported-aircraft vapor correction, 2026-09-15

Native CE attachment vectors use **right/up/forward**, while mesh vertices use
right/forward/up. Applying this distinction to F18.SH and RAFALE.SH gives exact
vertex matches for all four attachments, with the existing one-third-foot scale.
Heading hinges rotate the right/forward plane, preserving up. This fixes vapor
placement; exterior-store placement remains a separate open investigation.
[Evidence](../baselines/wind-turbulence-vapor.md).

### Contact-offset reader, 2026-09-15

The bounded `shape::contact_offset` reader follows the F2 relative link used by
FA 0x42e0c0 and reads only its signed word +8. Absent F2 and malformed links
remain distinct. This does not decode full collision bounds or establish a
contact surface from mesh faces. [Source contract](native-land-contact.md#shape-relative-contact-offset-e007),
[validation](../baselines/native-land-geometry.md).

### STRIP contact boxes, 2026-09-15

`shape::contact_boxes` bounds the F2 subrecord list used by COLGetBox. STRIP
initialization uses ten midpoint positions and two orientation records; these
are separate from visible mesh vertices. `native_strip RUNWAY.SH` diagnoses the
required IDs and partial static texture references. Full world/collision and
drawing acceptance remain open. [Contract](native-strip.md),
[commands and validation](../baselines/native-strip.md).

`native_strip RUNWAY.SH STRIP.OT` also validates the bounded STRIP/166 definition
and matches its explicit shape filename. Unknown source tokens remain preserved;
extra shape slots, different selectors/classes and unsupported layouts fail.
This is metadata inspection, not object placement or full resource resolution.
[Definition-reader evidence](../baselines/native-strip-definition.md).

## Whitecap shape boundary

WAVE1.SH/WAVE2.SH contain embedded frame-selection code, outside the static SH
reader. [The ocean contract](ocean.md) records the recovered 16-frame effect.
The trial runtime effect was removed at the user's request; no imported code
was executed and the reader's animated-shape coverage is unchanged.

## Additional FA aircraft rigs

The [additional-aircraft spec](../spec/additional-aircraft.md) owns fitted
presentation behavior. The base FA shapes project without extending the SH
reader. CODE lengths and neutral face counts are F14 29462/313, A4 24506/237,
and F31 21974/225. Header word +6 is 10 for F14 and 8 for A4/F31.

| Shape | Burner word / added faces | Brake word / added faces | Gear word / added faces | Hook |
| --- | --- | --- | --- | --- |
| F14 | 0x82e0 / 8 | 0x82e6 / 4 | 0x82ec / 16 | 0x82f8 adds 2 degenerate triangles |
| A4 | none | 0x6f90 / 12 | 0x6f96 / 18 | 0x6fa2 swaps 2 faces |
| F31 | 0x65a0 / 4 | 0x65a6 / 4 | 0x65b2 / 22 | none |

F14 flap neutral subcalls are guarded by 0x82fe/0x8304; A4 by
0x6fa8/0x6fae; F31 by 0x65be/0x65c4. Keep their neutral branches and apply
fitted hinges. F31's 0x65ca selects a rudder alternative. The rigs validate
CODE length, state-word sets, face counts and texture names before using their
own address mappings. Gear/brake/burner groups come from bounded per-word
projection differences. No old ATF offsets or SWPATCH shapes are substituted.

Inspect user-owned data with `cargo run --locked -p tore-formats --example
shape_inspect -- FILE.SH HEX_WORD=1`. Raw geometry output stays local.
[Source identities and evidence](../baselines/aircraft-fa-expansion.md).

The X-31 port's existing afterburner plume now follows live auxiliary pitch/yaw
rates with a fitted 15-degree maximum cue. The three source paddles also follow this demand about fitted hinges at their
forward edges, independently of afterburner visibility. See the [aircraft behavior spec](../spec/additional-aircraft.md)
for the control and animation contract.

FA F31.SH paddle faces are paired at 0x44b2/0x44da (upper),
0x4406/0x442e (left) and 0x4342/0x4381 (right). Each has two forward
vertices at source y=-41; these define the fitted hinge. Addresses refer to the
reviewed base FA shape, not an ATF variant. Face identity, texture coordinates
and neutral geometry are retained. The cold nozzle face 0x29f7 stays fixed.

A4.SH horizontal-tail faces 0x4345/0x4361/0x44b5/0x44d5 and
0x4967/0x4982/0x49dd/0x49f8 contain diagonals crossing the fitted elevator
strip. Clip all eight at y=-54 before rotating the aft portion. Selecting only
the rear polygon pairs leaves a stationary diagonal section in the elevator.

The optional user-supplied [engine material](../spec/engine-material.md) replaces reviewed burner
face materials at runtime. Its throttle glow is separate from the retail atlas
and does not change A-4E presentation or flame geometry.

## Seven-aircraft roster geometry

The base FA shapes were projected with `shape_inspect`, then each observed
state word was set independently to 1. Added face identities and geometry
bounds establish device endpoints, not original continuous schedules.
[Behavior and limitations](../spec/roster-aircraft.md),
[source build and validation](../baselines/aircraft-roster-expansion.md).

| Shape | CODE bytes | Neutral faces | Flame word / faces | Brake word / faces | Gear word / faces |
| --- | ---: | ---: | --- | --- | --- |
| MIG29 | 29290 | 328 | 8240 / 16 | 8246 / 4 | 824c / 16 |
| SU27 | 12838 | 146 | 41f0 / 16 | 41f6 / 2 | 41fc / 8 |
| MIG21 | 14952 | 159 | 4a50 / 8 | none | 4a56 / 6 |
| SU25 | 29626 | 334 | none | 8390 / 8 | 8396 / 18 |
| MIG23 | 23312 | 219 | 6ae0 / 6 | none | 6ae6 / 18 |
| SU35 | 27114 | 329 | 79c0 / 8 | 79c6 / 2 | 79cc / 18 |
| F22 | 20012 | 245 | 5df0 / 8 | 5e02 / 4 | 5e0e / 12 |
| F22N | 20146 | 248 | 5e70 / 8 | 5e82 / 4 | 5e8e / 12 |

Words are hexadecimal. The loader checks CODE length, complete observed word
set, neutral count, added face counts and texture identity before applying any
rig. F22 word 5dfc adds 14 main-bay belly details, handled independently of gear.
F22N (added 2026-09-22) has the same F-22 devices at its own words: bay 5e7c
adds 12 belly details and hook 5e9a adds the two native blade faces 40a1/40c0.
MiG23 header exponent is 9; the others are 8.
Flame forward roots are respectively -41, -60, -56, absent, -29, -59, -48 and
-48 source units. Gear upper bounds are 0, 0, -1, -9, -3, -1, -1 and -1. These own-shape
bounds establish source endpoints; the [animation spec](../spec/aircraft-animation.md)
defines fitted rigid travel. Local projection logs: `.local/*-shape.txt` and
`.local/<aircraft>-<word>.txt`. Unreviewed controls retain neutral geometry.

The subsequent [animation pass](../baselines/aircraft-animation.md) reviews
round nozzle faces at MIG29 468b/46a6/46d7/4706, SU27 2117/22fe,
MIG21 22e8, MIG23 34af/34d6 and SU35 2aee/366c/3690/3a9b/3af0.
Addresses are hexadecimal CODE offsets, not transferable between aircraft.
F22 outer glazing is 2136/2153/2170/219c/2220/2255/228a/22a4/22bc/
2462/247c/2494/2c36/2c6c. Its main-bay belly spans base faces
35d7/3601/362f/3cc0/3d03 and switched details under word 5dfc.
F22N outer glazing is 2215/2232/224f/227b/22ff/2334/2369/2383/239b/
2541/255b/2573/2c8d/2e57; ailerons 44b5/44d4/46b0/46cf; elevators
31eb/31fd/36d3/36e7/3ada/3cff; rudder faces 361d/3670/3903/3926. Its
main-bay belly spans 3755/36fd/3716/39ae plus 39dc, the remodelled right aft
belly panel chosen (fitted, 2026-09-22) as the counterpart of F22 3d03, with
switched details under word 5e7c.
All new hinge and material choices follow the linked fitted/opinionated contracts.

## Combat damage and smoke resource review

Local FA media review, 2026-09-17. `FA_1.LIB` SHA-256:
`657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f`;
`FA_2.LIB`: `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`.
The bounded SH parser reads A/B/C/D variants for all twelve supported aircraft.
A/C are body variants; B/D contain separated pieces. Example: F18_A is 296 faces,
shortened from intact longitudinal bounds -66..102 to -63..44; F18_B is 206 faces
with narrow lateral bounds -14..14. F18_C is 270 faces with reduced left extent;
F18_D is a 33-face near-flat piece. Thus A/B and C/D are compatible with two
breakup pairs, not evidence of four increasing damage levels. That pairing and
threshold selection are not established native consumers.

All nonempty texture references resolve to the matching variant PIC except
F22_D, which has no texture reference. The current import closure already selects
these aircraft-prefixed resources. Local inventory, bounds and texture previews:
`.local/damage-smoke/`. No retail bytes or previews are committed.

Local geometry/texture inspection on 2026-09-20 also confirms Rafale A removes
the left wing area and C reduces vertical-fin height. F-22 A visibly damages
left-wing geometry. These observations identify suitable appearances, not the
original hit-selection rules. The 256x418 `_F18_A.PIC` contains a dark damage
patch in the top-origin rectangle x=141..193, y=180..236. Local wireframe and
texture comparisons are in `.local/damage-profile-review/`. Reusing that patch
on other aircraft is a fitted rendering choice described in the damage spec.

`SMOKE.PIC` is 256x43 with no private palette, containing dark, grey and pale puff
art in three cells. Visually inspected 43x43 crops begin at x=0, 47 and 94.
The sheet uses palette index 255 for its background despite the generic PIC mask
being fully opaque. The host explicitly keys that index transparent.
The existing bounded general SH reader rejects `SMOKE.SH`;
its runtime program is not executed or enabled by this work. The host uses the
original PIC with fitted billboard placement. PIC SHA-256:
`9058efbbfac301c0d72d096fcedd7ec18df9f0d3e44dcfdae03fc8a19d7e655e`;
SH: `d4fa9f4374679ab9fe3da7926af75456f38f557035cd75a4889775a5f3838b51`.
The local FA manual describes effects of damage but does not supply these smoke
or mesh thresholds. [Implementation rules](../spec/damage-smoke.md) keep tuning
separate from this resource evidence.

`GRDLRGA.PIC` was also decoded and visually inspected: 256x252, twelve apparent
ground-explosion frames in three columns and four rows. The host samples 80x63
cells and scales this down for debris contact. This is a reviewed original asset
with fitted use; no claim is made that retail bullets selected this sheet.

Opaque imported world geometry participates in the shared
[surface lighting and shadow pass](../spec/surface-lighting.md). Smooth mode
submits complete animated meshes so camera-hidden faces can cast shadows;
stepped mode keeps the earlier face rejection and light maps. This does not
interpret or execute original shadow-shape commands.
