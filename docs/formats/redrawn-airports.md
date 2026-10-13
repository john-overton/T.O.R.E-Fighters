# Redrawn airports

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-10 (experiment AP1); the default everywhere
since 2026-10-11 (slice FX-A, protocol 24). John asked for "a test redraw of an airport using airport textures
but redrawn to be accurate instead of reusing the existing airports", then,
after reviewing the redrawn Kiev ("The airport looks good"), for one plan per
runway shape type, each theater's own runway textures with the dark asphalt
as the fallback, larger airports keeping their strips with parking and
taxiways on both sides, simple taxiway and edge lines, and the extra hangars
and vehicles as targets (all 2026-10-10), then for the redraw on for
everyone online (2026-10-10) and as the default (2026-10-11). Every airport
of the 75 layouts is drawn and flown from its type's plan in single player,
the Quick Mission, missions, online and on the dedicated server.
`TORE_REDRAWN_AIRPORTS=0` is a developer escape hatch for single player and
the offline tools: every airport is the retail one and nothing below runs.
Online games never read it ([online play and recordings](#online-play-and-recordings)).

Why: each retail runway shape is a whole airfield drawn about three times real
size across the runway (runways 368 to 760 ft wide, taxiways 160 to 290 ft),
while aircraft and buildings are drawn at real size
([placed object scale](objects-and-shapes.md#placed-object-scale-2026-10-10)).
The redraw keeps each runway where retail put it and redraws the rest at real
size.

## What changes and what stays

| Part | With the switch on | Source |
| --- | --- | --- |
| Runway position, heading, length, elevation | Unchanged: the frame is the retail runway's near threshold (STRIP anchor 0x11) and its far end. A second tile's runway object moves onto the tile's own main runway (below) | retail import, plan for tiles |
| ILS, approach line, landing service, short-strip rule | Unchanged (they read only the above); a second tile's follow its main runway (STRIP5A and STRIP6A in a pair: the joined crossing runway) | retail import, plan for tiles |
| Drawn pavement | Generated from the plan's runways, taxiways and aprons, tiled with retail runway art, markings painted in | plan, retail art |
| Landable and contact box | The redrawn field's extent plus a grass margin; grass between runways stays landable | plan |
| AI taxi, takeoff, landing and parking points | The plan's (the `AirfieldAnchors` roles of [native-strip](native-strip.md)), second tiles included since slice CL-AP; dirt strips (vertical pads) have none | plan |
| Ground-start slots | Follow the plan's takeoff point and taxi-out legs | plan |
| Airport buildings | Moved by rule onto the plan's building lines; extra hangars and vehicles added by rule | retail shapes, rules |
| Terrain recess under the airport | Follows the landable box | plan |

A Quick Mission ground target template that brings its own runway piece
(`~QBFAIR`, `~QGRSAIR`, `~QTBUNK` a DTSTRP dirt strip, `~QUSTRIP` a STRIP,
`~QFFACT` a STRIP1) is redrawn like a layout runway since slice BG (B14,
2026-10-12): the template's strip gets its plan's pavement, landable box, AI
points, lights, extra hangars and vehicles, keyed by the template object's
id, and the template's parked aircraft snap to its aprons. The template's own
buildings and units stay where the template stands them (they are surface
units and objectives; on the retail data none of them stands on the redrawn
pavement). Before, only the layout's placements were scanned, so those five
targets kept the retail strip shape while the rest of the world was redrawn.

A plan applies only to the runway shape it was drawn for: a placement whose
runway length differs from the plan's by more than a foot keeps its retail
airfield (and so does its second tile). Being the default, the redraw never
refuses a mission; with the retail import all 1,458 runway objects of the 75
layouts are redrawn (FX-A survey, 2026-10-11). The unit tests' synthetic map
has an 8,000 ft runway, so it keeps its retail airfield unless a test passes
its own plan.

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
9,700 ft) and the pair's link taxiways; the tile keeps its own runway object
and box but draws nothing itself. A tile with no base nearby (ten in the
`~*F` developer layouts) uses its own plan alone.

A tile's STRIP anchors are copies of its base's, so its retail approach line
lies beside its own pavement (on grass between its runways, or off the
field). Its runway object therefore flies from the tile's main runway, the
longest in its plan: the approach line, heading and length are that
runway's, so the ILS, a ground start and the AI's takeoff and landing are on
pavement (agent decision, FX-A, 2026-10-11).

Since slice CL-AP (2026-10-11) every tile plan has AI points of its own, the
base plans' pattern turned to its main runway (below), so the AI at a tile
taxis, takes off, lands and parks on the tile's pavement. In a pair the
tile's main runway continues one of the base's runways (`[pair] continues`):
STRIP3A's and STRIP7A's the base's main runway, STRIP5A's and STRIP6A's the
base's crossing runway. Layouts place the tile up to about 65 ft off that
line, so the tile is moved across the base runway by that much and the two
centre lines are one line (the move is only across it, so the link
taxiways still meet). STRIP5A's and STRIP6A's main runways are crosswise to
their frame and run on into the base's crossing runway, whose near end is
left unmarked: the tile's runway object flies the two as one runway
(`joined`), from the tile runway's near threshold to the base crossing
runway's far threshold (8,274 to 8,377 ft at STRIP5A fields, 14,572 to
14,597 ft at STRIP6A fields), so both ILS thresholds stand at real, marked
runway ends instead of one in the middle of the pavement where FX-A put it
(agent decision, CL-AP: the ILS along the real main runway, as asked). Its
ILS is still 90 degrees from the retail tile's line, which lay on no
runway. A lone tile (the `~*F` layouts) has no base to run into: all its
runway ends are marked.

Larger airports keep the strips retail drew, at their retail centrelines, with
a parallel taxiway on each side, links at both ends and midfield, an apron on
each side (the one on retail's apron side holding the nine parking slots), and
a building line along each apron's outer edge. Primary runways are 200 ft
wide, others 150 ft, taxiways 75 ft (fitted).

### The AI's points

Every plan but the dirt strip gives the AI one pattern along the runway it
flies (the base's retail runway, a tile's main runway), all `fitted`, agent
choices (FX-A for the bases, 2026-10-10; slice CL-AP for the tiles and the
corrections, 2026-10-11):

- the parallel taxiway 450 ft to one side of the centre line;
- a cross taxiway 190 ft behind the near threshold, over a 220 ft paved
  pad, from the parallel taxiway to the centre line, where the last taxi-out
  point stands; the takeoff spot is the threshold;
- the nine parking slots 250 ft apart in a row 888 ft from the centre line,
  on an apron beside the parallel taxiway, the exit at the apron's near
  corner and the entry at its far corner;
- the landing rollout ends on the centre line 250 ft before the far end,
  where a cross taxiway leads back to the parallel taxiway.

Each leg from a parking slot out to the spot and from the rollout end in to
every slot is straight and on pavement. Where the AI stops before turning
(the parking exit and every taxi-in point) it runs on about 12 ft and turns
on a radius of about 30 ft, so those points stand clear of a taxiway's end:
a corner stop 30 ft short of the junction. The AI's own taxi rules are in
[AI airfield sequences](../spec/ai-airfield.md#taxiing-on-the-pavement).

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
| `lights` | `false` for an unlit field (the dirt strip) |
| `[pair]` `base`, `tile`, `unmark` | A pair plan's two types and the base runway ends (`R1:far`) that run on into the tile and lose their markings; the ILS runway (`R0`) keeps both ends marked, since its threshold is set in from the pavement's end |
| `[pair]` `continues`, `joined` | The base runway and the tile runway that continues it (`["R1", "R0"]`): the tile is moved across to line them up; `joined = true` makes the tile's runway object fly the two as one |
| `[[material]]` `name`, `pic`, `rect`, `tile_ft`, `along`, `paint`, `variants` | A texel rectangle `[x, y, w, h]` of a retail PIC; the feet one copy covers along its columns and rows (0 across a runway or taxiway: its width); which texture axis runs along the element (`v` default, `u`); markings painted into a copy (`edges`, `centre`, `threshold`, `taxiway`, `digit`); other rectangles of the same size picked per copy |
| `[[runway]]` `from`, `heading`, `length`, `width`, `pad`, `touchdown`, `numbers`, `marked_near`, `marked_far` | Near threshold, heading against the frame in degrees, length, width, paved run before each threshold, touchdown zone length, designators (`false` for none, default from the world heading), whether each end has threshold markings |
| `[[taxiway]]` `width`, `points` | A polyline of straight legs at any angle, ends squared off half a width past each point |
| `[[apron]]` `min`, `max` | A rectangular apron |
| `[[line]]` `from`, `to`, `out` | A building line and the direction away from its apron |
| `[anchors]` | `taxi_out` (4), `takeoff`, `landing`, `taxi_in` (4), `parking` (9), `parking_heading` (degrees against the frame); a base takes off and lands along the frame, a tile along its main runway |
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
- Wherever a taxiway leg meets another element's edge, at any angle, through a
  bend in its own polyline or where it crosses another taxiway, a curved corner
  piece from the atlas's fillet art (sides 0.7 of the taxiway width) fills each
  inside corner. The piece is a parallelogram along the two edges, so angled
  junctions get it too. It is left out where it would touch other pavement,
  where the two edges run within about 14 degrees of parallel, and on the
  outside of an L where the pavement met does not run on along its whole side.
- Junction mouths: where a later taxiway meets an earlier one, the earlier
  one's piece across the mouth is drawn with its centre line only, so its edge
  lines stop at the junction instead of running across it.
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
- Parked template aircraft: an airfield template's parked aircraft that now
  stands off the pavement, and belongs to the airfield by the building rule,
  moves to the nearest free apron spot, keeping its heading. Spots are two rows
  on each apron, a quarter and three quarters deep, every 120 ft, at least
  100 ft from the plan's AI parking slots. Aircraft on carrier decks never
  move; one with no free spot stays where it was.

## Lights

The retail game has no airfield lights: no light objects in any layout, no
light geometry in the runway shapes, and the world's only point lights are
burning flares, afterburners and gunfire
([surface lighting](../spec/surface-lighting.md)). A redrawn airport adds its
own, as requested by John on 2026-10-10. All placements and looks are
`fitted`, agent choices from the usual real installations.

| Lights | Where | Colour | Seen |
| --- | --- | --- | --- |
| Runway edge | every 200 ft along both edges, 3 ft out | white | all round, 6 nm |
| Threshold and end bars | across each marked end, 10 ft apart | green toward the approach, red toward the runway | within 90 degrees of their face, 6 and 4 nm |
| Approach (ALSF-II in outline) | the ILS runway's marked ends: a five-light barrette every 100 ft to 2,400 ft, a crossbar 100 ft wide at 1,000 ft, red side barrettes over the inner 1,000 ft, a flasher on each station from 1,000 ft out sequenced toward the threshold twice a second | white, red side rows | within 30 degrees of the approach, 10 nm (flashers 12) |
| PAPI | four lights on the approach's left abeam the ILS aim point (1,000 ft in), 50 ft from the edge and 30 ft apart | each white above its angle, red below: 2.5, 2.83, 3.17 and 3.5 degrees from the outermost in | within 30 degrees of the approach, 8 nm |
| Taxiway edge | every 200 ft along both edges, 3 ft out, where the edge borders grass | blue | all round, 2 nm |

On the 3 degree path the PAPI shows two white and two red. The approach light
rows are one table (`terrain::redrawn::lights::approach_rows`).

Only a base plan's main runway has approach lights and a PAPI. A lone tile
(paired or alone, all paved) keeps its edge, threshold and taxiway lights
and gets no approach lights or PAPI; the dirt strips (`lights = false`) have
no lights at all, like a real grass or dirt field (agent decision, slice
CL-AP, from John's 2026-10-11 "Lone tiles like grass or dirt runways? Fine
with no lights").

Lights are drawn as points, a little larger as they near, dimmed with
distance, haze and cloud (dimmed as a surface at a third of their distance is,
so they carry farther than terrain) and by daylight: full at night, fading
through dusk with the flares' night rule, gone by day except the PAPI,
which shows by day at full strength, 1.8 times as bright and 1.4 times as
large, so it reads on a daylight final from 2 to 3 nm (John, 2026-10-10);
at night it keeps the plain point. They are tested against the world depth without writing it, like the
flares, and drawn in two instanced batches (steady lights uploaded once per
scene, flashers rewritten each frame from the world's tick clock, so a replay
flashes as its flight did). `TORE_AIRFIELD_LIGHTS=0` hides them. Kiev's field
has about 1,000 lights; Ukraine about 13,000.

## Short strips under the redraw

The redraw keeps every retail runway length, so the 22 dirt strips (DTSTRP,
1,074 ft) stay short strips: no tower service, no AI home
([short strips](../spec/airports.md#short-strips)). Since 2026-10-11 they are
labelled STOL and a player may start there
([STOL runways](../spec/airports.md#stol-runways)). They are also vertical
pads by type (`Scene::vertical_pad`), which keeps conventional AI off them
whatever their length, so they have no AI points. Each has a dirt hardstand
beside the strip, 80 to 320 ft right of its centre line and 150 to 800 ft
along, drawn in the strip's own dirt: the helicopter templates (Cuba QCCMHQ,
Panama QAPHELO, Gulf QPGSRUN) park their helicopters on it, 252 ft right of
the centre line (they stood on the grass beside the old 120 by 200 ft pad).

Lengthening them is a future option: John chose on 2026-10-10 to leave them
as they are. The findings: all 22 lie on flat cells at
0 ft with nothing higher within 2 nm, so a 3,500 or 4,000 ft strip needs no
cut or fill. Laid forward from the retail threshold, three would cross a
water cell (Toncri, Al Turas, Deleite; Deleite whichever way it is laid, the
other two clear when laid back from the far end), and laid backward San Carlos
would cover a Roland site; the other 18 are clear of water and layout objects
every way. It would take: a plan length the scene uses for the
runway (its length, approach centre and ILS, now the retail shape's),
`terrain::strip_length_ft` (the Quick Mission's list) and the vertical-pad
rule learning the redraw, a pad plan with taxiway and apron at real size, and
the short-strip text in the airport spec. The 2,000 ft rule itself needs no
change: a 3,500 ft strip clears it.

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
| The redraw as the default everywhere, online included | opinionated (John, 2026-10-10 and 2026-10-11) |
| Fixed on online rather than a mission text line; the plan mismatch fallback; a second tile's runway on its main runway; the recording's airfield scene | opinionated (agent, FX-A, 2026-10-11) |
| A template's own runway piece redrawn too (B14, John's bug report 2026-10-12); its buildings left where the template stands them; airfield scene 2 | opinionated (agent, BG, 2026-10-12) |
| AI taxi on the pavement; STOL labels and starts; lone grass or dirt fields unlit | opinionated (John, 2026-10-11) |
| The tiles' AI points, a tile lined up with the runway it continues, the STRIP5A and STRIP6A joined runway, lone tiles marked at both ends, lone paved tiles lit without approach lights, the dirt hardstand, the audit's wheel track and run-out | fitted or opinionated (agent, CL-AP, 2026-10-11) |
| Airfield lights exist | opinionated (John, 2026-10-10) |
| Light spacing, colours, ranges, approach pattern, PAPI placement and day share | fitted (agent) |

None of this is retail parity: retail drew its airfields at about 3x.

## Online play and recordings

Built in slice FX-A (2026-10-11). The scene is not on the wire: each machine
rebuilds it from its own import and build.

1. **Protocol 24.** The redraw's version: builds with and without it never
   meet, since the added buildings are targets with ids the old scene lacks
   and buildings, parked aircraft and the AI's points stand elsewhere
   ([the protocol](net-protocol.md#the-redrawn-airports-protocol-24)).
2. **Fixed on online.** The mission text has no airfield line and always
   means the redraw, so the King's game, every player's, the dedicated
   server and the bot build the same airfields; `TORE_REDRAWN_AIRPORTS=0`
   is never read online. A text line that let a session choose was the
   alternative, left out as more surface than a developer aid needs.
3. **Recordings.** The header's airfield scene says which airfields a
   flight had: 0 (absent) the retail ones, 1 the redraw with a ground target
   template's own runway piece retail, 2 (since slice BG, 2026-10-12) the
   redraw with that piece redrawn too ([replays](../REPLAYS.md)). The viewer rebuilds what the header names,
   whatever the switch says, so a recording from before the redraw replays
   on the retail airfields and a new one on the redraw. A redrawn recording
   is a format 3 file.

## Limits

- The dirt strips have no AI points: they are vertical pads by type, which
  no conventional AI uses, and short strips, nobody's home.
- AI homes are the nearest own field by runway centre, so moving the tiles'
  lines onto their main runways changed four of the 160 homes the
  `--airport-allegiance` survey lists (FX-A: Baltic Red: Gdov and Vitebsk
  became the tiles "Airport 27" and "Airport 23"; Greece Blue: "Airport 15"
  became Iraklion; North Korea Red: Kuum Ni became "Airport 22"). The joined
  STRIP6A runway (CL-AP) moved "Airport 22"'s centre and gave North Korea
  Red's Kuum Ni back. An AI flight homed on a tile parks on the tile's apron.
- A pair's tile is placed at the measured offset, which varies by up to 250 ft
  along the runway between layouts; link taxiways overlap generously to cover
  it, and the 220 ft pad at a STRIP5A or STRIP6A main runway's far end closes
  any gap to the base runway it runs into. Across the runway the tile is
  lined up exactly.
- A junction mouth clears only the earlier taxiway's edge lines; a runway's
  edge stripes still run past a taxiway's end, as on real fields.

## Audit

`--airfield-audit` (imported media, no display; slice CL-AP) builds every
layout of the import, the `~` developer layouts with the lone tiles
included, and for every runway object:

- walks every AI route its points give it, from every parking slot out to
  the takeoff spot, the takeoff roll and from the landing aim point in to
  every slot, every 5 ft, its centre and 6 ft either side (the wheel track),
  and 45 ft on past every point the AI stops at (the run-out of its stop and
  turn), against the drawn pavement;
- checks that every drawn runway end of the airfield is reachable over
  touching pavement pieces from the parking slots;
- checks that the runway object's line, which the ILS localizer and glide
  path follow, lies on a drawn runway: both thresholds within 2 ft of its
  centre line, inside its paved length, at its heading within 0.05 degrees
  (or on two runways that are one line, for a joined tile runway);
- for a pair, checks the two runways its plan says continue each other
  (`[pair] continues`) are one straight runway on the drawn pavement (a
  `pair alignment` line): both tile ends within 10 ft of the base runway's
  centre line, headings within 0.5 degrees, and no gap wider than 10 ft
  between the paved ends (`fitted`). The layouts place a tile 150 to 560 ft
  into its base's pavement, the 220 ft overrun pads included, so the
  pavements overlap by that much: one continuous surface, reported as a
  negative `gap_ft` and not a problem. The build lines the tile up across the
  base runway; without it 224 of the 430 pairs of the import were more than
  10 ft off and up to 64 ft (John, Spassk Dalniy, 2026-10-11: the runway
  jogged sideways halfway down);
- reports drawn pavement on water (a `water` line, no failure): pavement
  samples every 50 ft where the ground picture is the water cutout (255) or
  the cell is water class, split into runway and other pavement, with whether
  the retail runway line is wet too (retail placement, kept). The import has
  8 such fields: 4 with the retail runway line on water (CUB 1 and 16,
  GRE 3, IRA 9), and 4 where only our aprons and taxiways reach a shore
  (CUB 10, GRE 4, PGU 4, WTA 11: 1 to 24 per cent of the
  pavement). Vladivostok's Lake Khanka is not seen: it is painted in
  ordinary land indices (teal in that theater's palette) over elevation 0
  cells, so neither the picture nor the cell class marks it.

It prints one line per runway object and a `PROBLEM` line for each failure;
`--svg DIRECTORY` draws each airfield overhead with its routes, and
`--track FILE` adds the ground tracks an AI probe wrote with
`--probe-track`, green on the pavement and red off it. `--parked` builds
every Quick Mission ground target of the base theaters (seed 1) instead and
checks that every template aircraft standing on a redrawn airfield stands
on its pavement. The AI probe (`--ai-probe-ticks`) counts an AI aircraft on
its wheels in an airfield phase at a redrawn airport and off the drawn
pavement as an `off the pavement` anomaly; `--probe-pavement` prints the
count. The battery runs all three ([lane menus](../testing/lane-menus.md),
[lane AI](../testing/lane-ai.md)).

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
  of each plan overhead, oblique, from short final, along the parking row, down
  the runway toward its far end (for pairs, where it meets the second tile)
  and over the first taxiway junction ([development](../DEVELOPMENT.md)).
- Round 5 (2026-10-10, after the AL1 merge): fillets at every junction,
  junction mouths, the pair ILS runway's far end marked, and template aircraft
  on aprons. Switch off: guard SAME 52 against `sf-51b2d613`, `--validate-maps`
  identical to the base build. Switch on: all 75 layouts build;
  `--validate-ils` 578 ends, 0 problems, identical to switch off; airport
  sides (`--airport-allegiance`: offered fields and AI homes) identical on and
  off. Probes at one Blue airport per plan (Berezovka and L'viv STRIP,
  Bournemouth STRIP1, Pyong Taek STRIP2, El-Nakab STRIP3 with 3A, Ras Nasrani
  STRIP4, Southampton STRIP5 with 5A, Kimpo STRIP6 with 6A, Athinai and
  Liepaja STRIP7 with 7A) all reach `Parked` with 0 anomalies and three
  ground-start slots. Polotsk, Taetan, Wonsan and Longtian now have hostile
  AAA by the field that downs takeoffs, as on retail, so they left the list.
- Default (FX-A, 2026-10-11): with the redraw on by default, a survey of
  every runway object of the 75 layouts (1,458) found every one redrawn, and
  every runway other than the dirt strips with its aim point and departure
  point on pavement, after moving the 440 second tiles' lines onto their main
  runways (before: all 430 paired and 10 lone tiles had both off the
  pavement). `--validate-ils`: 578 ends, 0 problems; it differs from the
  retail airfields only in the tiles' lengths. AI probes (a two-ship ground
  start, takeoff, land order at 75 s, through `Parked`, 0 anomalies) at one
  field per base plan (UKR 8, BAL 3, EGY 10 and 11, FRA 1 and 14, GRE 1,
  NSK 9 and 11), six paired tiles (EGY 22, FRA 24, BAL 24, GRE 13, NSK 18,
  VLA 22) and four lone tiles (`~EGYF` 2, `~VLAF` 1, `~APAF` 1, `~SPAF` 1).
- AI on the pavement (slice CL-AP, 2026-10-11): before it, `--airfield-audit`
  found 10,023 problems at the 1,458 runway objects (the 440 tiles without AI
  points, corner stops and exits whose run-out left the taxiway, two STRIP6A
  runways not reachable from parking) and `--parked` 6 helicopters on the
  grass; after it, 0 and 0 (27,284 routes walked). AI probes counting every
  AI ground tick off the drawn pavement: before, the two-ship ground start,
  land order and taxi to parking left the pavement at 10 of 12 fields and at
  the lone `~VLAF` 1 (15,664 of 74,137 ground ticks there: the AI drifted to
  the runway edge and over corners); after, 0 off-pavement ticks and every AI
  aircraft parked at 20 fields (every base plan; paired tiles EGY 22, FRA 24,
  GRE 13, BAL 24, NSK 18, VLA 22; lone tiles `~EGYF` 2, `~APAF` 1, `~VLAF` 1,
  `~BALF` 1, `~SPAF` 1) and at 11 wing-of-four or other-aircraft runs (UKR 8,
  NSK 11, FRA 14, FRA 24, GRE 13, VLA 22, `~EGYF` 2, `~BALF` 1 with four
  F/A-18Ds, three landing and parking; C-130 at EGY 10, F-16C at NSK 18,
  MiG-29 at `~APAF` 1). `--validate-ils`: 578 ends, 0 problems; only the
  STRIP5A and STRIP6A tiles' lengths change (the joined runway). Before and
  after overheads with the probe tracks: `.local/surface-close/renders/cl-ap/`.
