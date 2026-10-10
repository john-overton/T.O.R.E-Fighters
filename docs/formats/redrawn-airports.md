# Redrawn airports (experiment AP1)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-10. A feasibility prototype, off by default.
John asked for "a test redraw of an airport using airport textures but redrawn
to be accurate instead of reusing the existing airports" (2026-10-10). One
airport, Kiev in the Ukraine theater, has a redrawn plan. With
`TORE_REDRAWN_AIRPORTS=1` the game draws and flies Kiev from that plan; unset
or `0`, every airport is the retail one and nothing below runs.

Why: each retail runway shape is a whole airfield drawn about three times real
size across the runway (runways 368 to 760 ft wide, taxiways 160 to 290 ft),
while aircraft and buildings are drawn at real size
([placed object scale](objects-and-shapes.md#placed-object-scale-2026-10-10)).
The redraw keeps the runway where retail put it and redraws everything else at
real size.

## What changes and what stays

| Part | With the switch on | Source |
| --- | --- | --- |
| Runway position, heading, length, elevation | Unchanged: the frame is the retail runway's near threshold (STRIP anchor 0x11) and its far end | retail import |
| ILS, approach line, landing service, short-strip rule | Unchanged (they read only the above) | retail import |
| Drawn pavement | Generated from the plan's runways, taxiways and apron, tiled with retail runway art | plan, retail art |
| Landable and contact box | The redrawn field's extent plus a grass margin; grass between runways stays landable, as today's whole-airfield box allows | plan |
| AI taxi, takeoff, landing and parking points | The plan's (the nine-slot `AirfieldAnchors` roles of [native-strip](native-strip.md)) | plan |
| Ground-start slots | Follow the plan's takeoff point and taxi-out legs | plan |
| Airport buildings | Retail placements moved by alias to the plan's spots; extra hangars, fuel tanks and vehicles added | plan, retail shapes |
| Terrain recess under the airport | Follows the landable box | plan |

A plan whose runway length differs from the retail shape's by more than a foot,
or whose AI points leave its landable box, is refused and the mission does not
build, so a wrong plan is never flown silently.

## The plan file

One file per airport in `crates/tore-world/airports/`, compiled into the build
(no file is read at run time, so every machine of a build has the same plan).
The format is a small subset of TOML: `# comments`, `key = value`, `[table]`,
`[[array of tables]]`, numbers, quoted strings, booleans and arrays.

Coordinates are feet in the **runway frame**: `x` to the right of the retail
runway's centreline, `z` along it from its near threshold. The file holds no
retail coordinates; the import supplies the frame.

| Key | Meaning |
| --- | --- |
| `layout`, `strip` | Layout code (`UKR`) and the STRIP placement's name (`Kiev`) |
| `runway_length_ft` | The retail runway length the plan was drawn for (Kiev 5,532) |
| `grass_margin_ft` | Landable grass around the pavement |
| `[[material]]` `name`, `pic`, `rect`, `tile_ft` | A texel rectangle `[x, y, w, h]` of a retail PIC and the feet one copy covers along its columns and rows; a runway material's first `tile_ft` of 0 spans the runway's width |
| `[[runway]]` `x`, `threshold`, `length`, `width`, `pad`, `touchdown`, `numbers` | Centreline offset, near threshold, length, width, paved run before each threshold, touchdown zone length, near and far designators. The first runway is the retail one (`x` 0, `threshold` 0) |
| `[[taxiway]]` `width`, `from`, `to` | A straight taxiway along `x` or `z`, ends squared off half its width past each point |
| `[[apron]]` `min`, `max` | A rectangular apron |
| `[anchors]` | `taxi_out` (4), `takeoff`, `landing`, `taxi_in` (4), `parking` (9), `parking_heading` (degrees against the runway) |
| `[[building]]` `type`, `at`, `heading`, `replaces` | A placed object at real size; `replaces` names the retail placement's `alias` it moves, otherwise it is added |

Required materials: `runway_plain`, `runway_threshold`, `runway_touchdown`,
`runway_centreline`, `taxiway`, `apron`; `digit_0` to `digit_9` are optional
(a designator with a missing digit stays plain).

## Generation rules

All `fitted`, agent choices of 2026-10-10 from usual real layouts:

- Runway bands from each threshold: the pad (plain), threshold bars for 150 ft,
  plain to 190 ft, the designation digits 60 ft tall and 20 ft wide with a
  10 ft gap, plain to 300 ft, the touchdown texture for `touchdown` feet, then
  the centreline texture to the far end's mirror image. The near end's art
  reads upright on approach; the far end's is turned half round.
- Overlaps are cut away, not layered: runways first, then taxiways, then
  aprons, so no two pavement pieces share a spot and nothing z-fights.
- Textures repeat on a grid from each element's start, one copy per cell,
  inset half a texel so atlas neighbours never bleed in.
- Added buildings take the side and flags of the first moved retail building.
  Their object ids are `0x40F0_0000` up, inside the layout range.

## Retail runway art (inventory)

The 13 retail runway shapes each use one texture atlas of their own name
(`_RUNWAY.PIC` 256 x 211, `_RNWY1.PIC` to `_RNWY7A.PIC` 256 x 324 to 678,
`_DTSTRP.PIC` 256 x 126). Each face maps a whole rectangle of the atlas once
(no repeating UVs), at about 5 to 12 ft per texel on the 3x shape, so at real
size the same art covers about 1.5 to 4 ft per texel.

| Reusable tile | Where | Notes |
| --- | --- | --- |
| Runway asphalt with a centreline dash | `_RNWY7`, `_RNWY7A` (dark) | Dash about 120 ft at 2 ft per texel, near the real stripe |
| Runway with tyre marks | `_RNWY7`; tan slab runways in `_RUNWAY`, `_RNWY3`, `_RNWY4`, `_RNWY3A` | |
| Threshold bars | `_RNWY7` only (three white bars) | |
| Designation digits | Runway number boards on `_RNWY1` to `_RNWY7A` and `_DTSTRP`: 1, 2, 3, 5, 6, 7, 8, 9 | No 0 or 4; a dark board behind each digit |
| Taxiway asphalt | `_RNWY7`, `_RNWY2`, `_RNWY6`, grey in `_RUNWAY` | No taxiway edge or centreline markings in any atlas |
| Apron concrete slabs | `_RNWY7`, `_RNWY1`, `_RNWY5`, `_RNWY6A`; hexagonal blocks in `_RUNWAY` | |
| Fillet curves | Corner pieces in most atlases | Not used by the prototype |
| Grass | None (only `_DTSTRP`'s dirt with grass edges) | The terrain shows between runways |

The Kiev plan uses `_RNWY7.PIC` for its surfaces and digits from `_RNWY1`,
`_RNWY3`, `_RNWY5` and `_RNWY7`; Kiev's own `_RUNWAY.PIC` has no markings.

## Provenance

| Component | Label |
| --- | --- |
| Retail art, building shapes, runway frame and length | retail import |
| The redraw exists | opinionated (John, 2026-10-10) |
| Kiev's layout: widths 200 and 150 ft, 75 ft taxiways, parallel runway 1,068 ft left, apron, building spots, extra buildings | fitted (agent) |
| Marking bands, cut-away order, texel scale per material | fitted (agent) |
| `TORE_REDRAWN_AIRPORTS` honoured by replays (recordings do not keep it) | opinionated (agent) |

None of this is retail parity: retail drew its airfields at about 3x.

## Limits

- One airport. Variants (`~UKR1` and the others) and every other theater keep
  their retail airfields.
- Single player only. A multiplayer server builds its own scene and does not
  read the switch; both ends must agree before it can fly online.
- Square taxiway junctions (no fillets), no taxiway or runway edge lines, a dark
  board behind each digit.

## Validation (2026-10-10, surface-data import)

- Switch off: the single-player guard is unchanged (SAME 52), and the Kiev AI
  probe below gives the same log as before the change.
- `--validate-ils`: 578 runway ends, 0 problems, the same output on and off.
- Switch on, Kiev (`--ground-start 7`): an AI F/A-18D takes off, holds,
  lands on the redrawn 200 ft runway, taxis back by taxiway N and A and parks
  on the apron (probe phases through `Parked`, 0 anomalies); a four-ship
  ground start puts the player on the runway and three wingmen on taxiway S,
  and all three take off.
- Unit tests: the plan's AI points and taxi legs lie on its pavement, no two
  pavement pieces overlap, texture cells tile each piece exactly, the runway
  and approach line do not move, and wrong plans are refused.
