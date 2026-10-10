# Redrawn airports (experiment AP1)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-10. An experiment behind a switch, off by
default. John asked for "a test redraw of an airport using airport textures
but redrawn to be accurate instead of reusing the existing airports", then,
after reviewing the redrawn Kiev ("The airport looks good"), for one plan per
runway shape type, each theater's own runway textures with the dark asphalt
as the fallback, larger airports keeping their strips with parking and
taxiways on both sides, simple taxiway and edge lines, and the extra hangars
and vehicles as targets (all 2026-10-10). With `TORE_REDRAWN_AIRPORTS=1`
every airport of the 75 layouts is drawn and flown from its type's plan;
unset or `0`, every airport is the retail one and nothing below runs.

Why: each retail runway shape is a whole airfield drawn about three times real
size across the runway (runways 368 to 760 ft wide, taxiways 160 to 290 ft),
while aircraft and buildings are drawn at real size
([placed object scale](objects-and-shapes.md#placed-object-scale-2026-10-10)).
The redraw keeps each runway where retail put it and redraws the rest at real
size.

## What changes and what stays

| Part | With the switch on | Source |
| --- | --- | --- |
| Runway position, heading, length, elevation | Unchanged: the frame is the retail runway's near threshold (STRIP anchor 0x11) and its far end | retail import |
| ILS, approach line, landing service, short-strip rule | Unchanged (they read only the above) | retail import |
| Drawn pavement | Generated from the plan's runways, taxiways and aprons, tiled with retail runway art, markings painted in | plan, retail art |
| Landable and contact box | The redrawn field's extent plus a grass margin; grass between runways stays landable | plan |
| AI taxi, takeoff, landing and parking points | The plan's (the `AirfieldAnchors` roles of [native-strip](native-strip.md)); second tiles and pads have none | plan |
| Ground-start slots | Follow the plan's takeoff point and taxi-out legs | plan |
| Airport buildings | Moved by rule onto the plan's building lines; extra hangars and vehicles added by rule | retail shapes, rules |
| Terrain recess under the airport | Follows the landable box | plan |

A plan whose runway length differs from the retail shape's by more than a foot
is refused and the mission does not build, so a wrong plan is never flown
silently.

## The plans

One file per runway shape type and one per pair in
`crates/tore-world/airports/`, plus `defaults.toml` (the dark asphalt
materials), compiled into the build: no file is read at run time, so every
machine of a build has the same plans.

| Plan | Type (shape) | Base airports | Runways kept | Atlas |
| --- | --- | --- | --- | --- |
| `strip.toml` | STRIP (RUNWAY.SH) | 28 | 3 parallels | `_RUNWAY` tan slabs, hexagonal apron |
| `strip1.toml` | STRIP1 (RNWY1) | 45 | 2 parallels, a short strip at 30 degrees | `_RNWY1` grey |
| `strip2.toml` | STRIP2 (RNWY2) | 21 | 1 | `_RNWY2` dark |
| `strip3.toml` | STRIP3 (RNWY3) | 26 | 2 parallels | `_RNWY3` tan |
| `strip4.toml` | STRIP4 (RNWY4) | 27 | 1 (150 ft wide) | `_RNWY4` tan |
| `strip5.toml` | STRIP5 (RNWY5) | 20 | main and a crossing runway | `_RNWY5` grey |
| `strip6.toml` | STRIP6 (RNWY6) | 12 | main and a crosswind runway | `_RNWY6` dark |
| `strip7.toml` | STRIP7 (RNWY7) | 26 | 2 parallels | `_RNWY7` dark asphalt (the defaults) |
| `strip3a.toml` | STRIP3A (RNWY3A) | 26 | 2 (continuing STRIP3's) | `_RNWY3A` tan |
| `strip5a.toml` | STRIP5A (RNWY5A) | 20 | crossing runway and one at 45 degrees | `_RNWY5A` grey |
| `strip6a.toml` | STRIP6A (RNWY6A) | 12 | crosswind runway and one at 50 degrees | `_RNWY6A` dark |
| `strip7a.toml` | STRIP7A (RNWY7A) | 26 | 1 (continuing STRIP7's) | `_RNWY7A` dark asphalt |
| `dtstrp.toml` | DTSTRP (DTSTRP.SH) | 22 | a 75 ft dirt strip | `_DTSTRP` dirt |
| `pair3.toml` to `pair7.toml` | a base and its second tile (84 airfields) | | joined | both |

Every placement of a type shares the frame of its shape, so one plan redraws
all of them. A pair plan applies when a second tile (STRIP3A, 5A, 6A or 7A)
stands within 14,000 ft of a base of its type at the same heading: the base
draws both plans (the tile's at the offset measured in the layout, 8,900 to
9,700 ft) and the pair's link taxiways; the tile keeps its own runway object,
box and ILS but draws nothing itself. A tile with no base nearby (seven in the
`~*F` layouts, one in NSK) uses its own plan alone.

Larger airports keep the strips retail drew, at their retail centrelines, with
a parallel taxiway on each side, links at both ends and midfield, an apron on
each side (the one on retail's apron side holding the nine parking slots), and
a building line along each apron's outer edge. Primary runways are 200 ft
wide, others 150 ft, taxiways 75 ft (fitted).

### Keys

The format is a small subset of TOML: `# comments`, `key = value`, `[table]`,
`[[array of tables]]`, numbers, quoted strings, booleans and arrays.
Coordinates are feet in the **runway frame**: `x` to the right of the retail
runway's centreline, `z` along it from its near threshold. The import supplies
the frame; the only retail facts a plan states are its type's runway length
and the retail runway centrelines it keeps.

| Key | Meaning |
| --- | --- |
| `applies_to`, `runway_length_ft`, `ils_runway` | The STRIP type, the retail runway length checked against its shape, and whether the first runway lies on the retail ILS line (not for second tiles) |
| `grass_margin_ft` | Landable grass around the pavement |
| `[pair]` `base`, `tile`, `unmark` | A pair plan's two types and the base runway ends (`R1:near`) that run on into the tile and lose their markings |
| `[[material]]` `name`, `pic`, `rect`, `tile_ft`, `along`, `paint`, `variants` | A texel rectangle `[x, y, w, h]` of a retail PIC; the feet one copy covers along its columns and rows (0 across a runway or taxiway: its width); which texture axis runs along the element (`v` default, `u`); markings painted into a copy (`edges`, `centre`, `threshold`, `taxiway`, `digit`); other rectangles of the same size picked per copy |
| `[[runway]]` `from`, `heading`, `length`, `width`, `pad`, `touchdown`, `numbers`, `marked_near`, `marked_far` | Near threshold, heading against the frame in degrees, length, width, paved run before each threshold, touchdown zone length, designators (`false` for none, default from the world heading), whether each end has threshold markings |
| `[[taxiway]]` `width`, `points` | A polyline of straight legs at any angle, ends squared off half a width past each point |
| `[[apron]]` `min`, `max` | A rectangular apron |
| `[[line]]` `from`, `to`, `out` | A building line and the direction away from its apron |
| `[anchors]` | `taxi_out` (4), `takeoff`, `landing`, `taxi_in` (4), `parking` (9), `parking_heading` (degrees against the runway) |
| `[extras]` `hangars`, `vehicles` | How many extra hangars the lines may take, and whether fuel trucks and trucks are added |

Material names: `runway_plain`, `runway_threshold`, `runway_touchdown`,
`runway_centreline`, `taxiway`, `apron`, `fillet` and `digit_0` to `digit_9`.
A plan's own materials replace the defaults by name; whatever its atlas lacks
comes from `defaults.toml`.

## Generation rules

All `fitted`, agent choices of 2026-10-10 from usual real layouts:

- Runway bands from each marked threshold: the pad (plain), threshold bars for
  150 ft, plain to 190 ft, the designation digits 60 ft tall and 20 ft wide with
  a 10 ft gap, plain to 300 ft, the touchdown texture for `touchdown` feet, then
  the centreline texture to the other end. The near end's art reads upright on
  approach; the far end's is turned half round. Designators come from the
  runway's world heading, a tenth of it rounded, without a leading zero (`36`,
  `9`); a designator needing a digit retail never drew (0 or 4) stays plain.
- Overlaps are cut away, not layered: runways first, then taxiways, then
  aprons, each cut into convex pieces, so no two pavement pieces share a spot
  and nothing z-fights. Angled runways and taxiways are cut the same way.
- Where a taxiway leg ends square on another element's edge, a curved corner
  piece from the atlas's fillet art (sides 0.7 of the taxiway width) fills each
  corner, if it touches no other pavement.
- Textures repeat on a grid from each element's start, one copy per cell,
  inset half a texel so atlas neighbours never bleed in. Apron copies pick one
  of the material's rectangles and mirror it from a fixed hash of the cell.
- Markings are painted into runtime copies of the retail texels in the retail
  palette's nearest white and yellow: runway edge stripes, a centreline dash
  over half of each 200 to 240 ft copy, sixteen threshold bars, yellow taxiway
  centre and edge lines, and the number boards with everything but the white
  figure replaced by the runway's own asphalt. The art lives in
  `crates/tore-app/src/scenery/redrawn_pavement.rs`; no retail bytes are
  stored.
- Buildings: every retail `.OT` building inside a runway shape's footprint
  grown by 1,500 ft, and nearer to that airfield's strips than to any other,
  moves to the building line nearest its retail spot, keeping its retail
  heading and its order along the line, its near face 20 ft behind the line and
  40 ft from its neighbours. Strips, bridges, roads, city clusters and surface
  units do not move; a building no line has room for stays where it was.
- Extras: what is left of each line takes extra hangars (HANGR, every third
  HANGRB where it fits) up to the plan's count, doors to the apron; a fuel truck
  (TANKER.NT) behind every third parking slot and a truck (TRUCK.NT) at each
  line's start. They are targets like the retail buildings (John,
  2026-10-10). Their object ids are `0x40F0_0000` up, inside the layout range.

## Retail runway art (inventory)

The 13 retail runway shapes each use one texture atlas of their own name
(`_RUNWAY.PIC` 256 x 211, `_RNWY1.PIC` to `_RNWY7A.PIC` 256 x 324 to 678,
`_DTSTRP.PIC` 256 x 126). Each face maps a whole rectangle of the atlas once
(no repeating UVs), at about 5 to 12 ft per texel on the 3x shape, so at real
size the same art covers about 1.5 to 4 ft per texel.

| Reusable tile | Where | Notes |
| --- | --- | --- |
| Runway surface | every atlas: dark asphalt `_RNWY7`/`7A`, speckle-edged dark `_RNWY2`/`6`/`6A`, grey `_RNWY1`/`5`/`5A`, tan `_RNWY3`/`3A`/`4`, tan slabs `_RUNWAY`, dirt `_DTSTRP` | |
| Centreline dash | `_RNWY7`, `_RNWY7A` (in the tile); `_RNWY3` and `_RNWY4` as separate decals | Painted elsewhere |
| Threshold bars | `_RNWY7` only (three white bars) | Painted elsewhere |
| Designation digits | Runway number boards: 1, 2, 3, 5, 6, 7, 8, 9 | No 0 or 4 |
| Taxiway surface | every atlas but `_DTSTRP` | No taxiway markings anywhere |
| Apron | slabs in most; hexagonal blocks in `_RUNWAY` | |
| Fillet curves | `_RNWY1`, `3`, `3A`, `4`, `5`, `5A`, `7`, `7A` | Used for curved corners |
| Grass | none (only `_DTSTRP`'s dirt with grass edges) | The terrain shows between runways |

## Provenance

| Component | Label |
| --- | --- |
| Retail art, building shapes, runway frames and lengths, runway centrelines | retail import |
| The redraw, per-type plans, theater textures with a dark asphalt fallback, strips kept, both-side parking and taxiways, painted lines, extras as targets | opinionated (John, 2026-10-10) |
| Widths, taxiway offsets, apron sizes, link positions, building lines, extras counts | fitted (agent) |
| Marking bands, cut-away order, fillet size, apron variation, building rule distances | fitted (agent) |
| `TORE_REDRAWN_AIRPORTS` honoured by replays (recordings do not keep it yet) | opinionated (agent) |

None of this is retail parity: retail drew its airfields at about 3x.

## Online play and recordings (planned, not built here)

John wants the redraw on for everyone online (2026-10-10). The scene is not on
the wire: each machine rebuilds it from its own import and build. The plan:

1. When the redraw becomes the default, bump the protocol (after the surface
   round's protocol 22) so builds with and without it never meet: the added
   buildings are targets with ids the old scene lacks, and buildings stand
   elsewhere. Refresh `wire-golden.txt`.
2. Until then a session could carry the switch in the mission spec (a mission
   text line and its wire field) so the host decides for every seat; not needed
   if it simply becomes the default.
3. Recordings: add the airfield set to the replay header's world identity (a
   scene version, with the replay format bump RP1 plans), so a replay rebuilds
   the airfields it was flown on; `replay::identity::terrain` then reads it
   instead of the viewer's switch.

## Limits

- Lone second tiles and pads have no AI points (as in retail, whose points for
  them lie off their pavement); the AI uses the runway fallback there.
- A pair's tile is placed at the measured offset, which varies by up to 250 ft
  between layouts; link taxiways overlap generously to cover it.
- Square corners where a taxiway meets at an angle (fillets only fit square
  junctions); a taxiway's edge lines run across the mouths of later junctions.
- The retail parked aircraft of airfield templates stand at their template
  spots, which may now be grass.

## Validation (2026-10-10, surface-data import)

- Switch off: the single-player guard is unchanged (SAME 52 against
  `sf-172a5a31`); `--validate-maps` (75 layouts) and `--validate-ils` give the
  base output.
- Switch on: all 75 layouts build; `--validate-ils` still reports 578 runway
  ends and 0 problems, byte for byte the switch-off output. Scenery vertices
  rise (Ukraine 30,327 to 91,959, the largest, North Vietnam, 268,608 to
  310,296; the cap is 838,860) and Ukraine's placements from 257 to 425 with
  the extras.
- Switch on, one airport per plan in six theaters (Kiev and Berezovka for
  STRIP, Polotsk STRIP1, Taetan STRIP2, Cairo STRIP3 with 3A, El Shirif STRIP4,
  Vilnius STRIP5 with 5A, Wonsan and Sunan STRIP6 with 6A, Longtian and Liepaja
  STRIP7 with 7A): a four-ship ground start puts the player on the takeoff
  spot and three wingmen on the taxi-out route, and an AI F/A-18D takes off,
  lands at the redrawn field on its land order, taxis in and parks (probe
  phases through `Parked`, 0 anomalies) at every one.
- Unit tests: every plan's AI points and taxi legs lie on its pavement, no two
  pavement pieces overlap (pairs included), texture cells tile each piece
  exactly, the runway and approach line do not move, buildings move onto the
  line, markings paint where they belong, and wrong plans are refused.
- `--airfield-sheets OUTPUT_DIRECTORY [--all] [THEATER ...]` renders one airport
  of each plan overhead, oblique, from short final and along the parking row
  ([development](../DEVELOPMENT.md)).
