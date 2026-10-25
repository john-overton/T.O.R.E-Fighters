# Bridge placement and destruction

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. This review addresses John's bridge placement and asset
request of 2026-10-25. Retail comparison is unavailable. The source geometry
and damage branches are evidence about assets, not proof of retail placement.
See [source evidence](../formats/objects-and-shapes.md#destroyed-looks-of-buildings-and-bridges-2026-10-12).

## Placement

Bridge types retain their source position, heading, pitch, bank and full map
scale. Roads retain their existing map-height rule. Bridges stand on the
bottom of their complete shape through `tore_world::grounding`. Using the
shape origin as ground height buries the decks that lie below that origin.
The correction is a **fitted agent choice**, not a recovered retail datum.
Joined END/MID pieces of the same family share a support datum. Their
horizontal shape bounds may overlap or be at most 16 ft apart; their forward
axes must have an absolute dot product above 0.99. Whole BRD1 through BRD4
bridges are independent. Footing includes complete geometry and samples face
interiors at intervals no larger than 64 ft, capped at 256 subdivisions.
Authored relative height is added after the correction. Source angles remain.

Level bridge pieces at authored height zero also receive a terrain corridor.
The complete shape footprint gets 2 ft of clearance on each side. Adjoining
footprints merge through the existing foundation builder. A merged footprint
containing bridge support uses the lowest of nine imported terrain samples
(corners, edge midpoints and centre), clamped to sea level. Its 512 ft skirt
blends into the unmodified terrain. Existing airport support takes precedence.
This cuts a high bank instead of lifting an entire crossing above the low bank.
The correction changes contact and drawing together. It leaves the source
water mask and road artwork in place. The world-axis rectangle can also grade
bare ground beside an irregular bridge; this is a known fitted difference.
Tilted or explicitly raised pieces keep their authored pose and receive only
the shared support translation. The long suspension bridge's cables do not
enlarge its solid hit box. A reviewed flat colour-6 shadow decal also stays
out of bridge contact bounds. It remains part of the drawing and footing,
so a shot below a suspension deck passes through air rather than hitting
the shadow's enclosing box. Other objects retain their existing contact policy.

## Damage and destruction

The imported OT supplies hit points, category, explosion and crater words.
All twelve use structure category `0x0100`, which selects weapon damage class
index 2.
The twelve types and the exact imported asset inventory are measured by
`--bridge-audit`, without supplementing the imported pack from retail archives.

A bridge with positive hit points retains its intact geometry, including at
half health. At zero hit points, the eight span types with an internal damage
branch draw that branch: BR1MID, BR2MID, BR3MID, BRDMID and BRD1 through BRD4.
The four END types have no separate damaged geometry and keep their intact
appearance. These are the existing accepted destruction rules from
[surface defenses](surface-defenses.md#destroyed-looks). There is no additional
intermediate damaged model. The original game's threshold for selecting its
damage branch remains **unknown**. Next research: the executable's consumer
of the shape damage word, using the build identity in the source evidence.

Broken geometry retains the intact bridge's placed datum. It must not be
lifted again to put the bottom of a fallen span on ground. Replay seeking
back to a living bridge restores the intact geometry at the same datum.
Bridge destruction uses explosion type 35 (the existing presentation varies
it among types 35, 36 and 37), crater type 0 and no persistent
wreck fire. A weapon impact can independently create its own effects and
marks. No bridge record names DEBRIS.SH or a separate bridge ruin object.
The source contains no separate falling animation in the reviewed branches;
this host swaps geometry immediately.

The strike controller treats a bridge as a normal surface target and checks
its current HP before continuing an attack. The flight map uses the existing
structure filter and labels a known destroyed contact `DESTROYED`. No new
strike behavior or map symbol is added.

There is no bridge-specific river obstruction or convoy rerouting system.
Destroyed scenery ceases to be a living target. This does not claim that a
convoy is stopped by a broken span, or that a river becomes navigable. Those
original behaviors are **unknown** and need a bounded behavior investigation
before implementation. This review does not change autonomous movement.

## Validation contract

The development audit reports every selected imported MM bridge placement,
its authored orientation, sampled road/ramp clearances at both ends and
across the span, lowest-shape support clearances, sea-plane clearance and
nearby object overlap candidates. Painted road and river alignment requires
visual inspection; source heading alone does not prove alignment. The
imported terrain has no separate surveyed riverbed beneath painted rivers.

Captures use the ordinary scenery and destroyed-geometry paths. Numeric
clearance is measured against the same world terrain as contact, rather
than inferred from a screenshot. Asset coverage and visual fidelity are
separate results. Missing geometry or an unknown deck must not count as a
passing placement. A clear audit row means no burial beyond 1 ft and no
lowest-geometry support gap above 30 ft. It does not mean every ramp vertex
is flush: the source pieces have small differences between their lowest
points and their road edges, which remain visible in the endpoint columns.

The bridge correction changes deterministic world geometry and target positions.
It adds no HP state, network field, replay event or checkpoint field. Existing
host-authoritative HP snapshots select the same intact/broken art for every
client. Recordings retain those HP values; capture validation compares live
and replay geometry at intact, partial-health and zero-health states. Rebuilding
an older recording uses the corrected terrain and bridge geometry. The lead
must account for that behavior change when integrating the release; this
change does not bump any version.
