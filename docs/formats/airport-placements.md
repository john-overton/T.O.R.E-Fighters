# Airport placement and resource recovery

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-20. `tore-formats::mission` reads bounded
base-layout placements and side tables while retaining source order, unknown
fields, signed aliases and authoring sections. The app resolves placed
definitions, explicit main shapes and projected texture references.
[Measured inputs and results](../baselines/ukraine-airports.md).
[Player behavior](../spec/airports.md). [Implementation stages](../ROADMAP.md#airport-and-ground-object-expansion).

Every base placement receives a stable source key and reserved scene identity.
Supported bodies render at their SH header scale and become live targets.
No-body controllers and unsupported projections remain manifest-only with a
diagnostic, without invisible contact boxes or targets. Campaign overlay replacement
semantics remain unsupported and are rejected rather than appended to a base.

## Ukraine base layout

UKR.MM contains 257 object records. Fourteen are STRIP.OT runways. Fourteen
subsequent airport-labeled comment sections contain 99 objects. The remaining
144 records describe non-airport sections. Comment groupings establish authored
layout organization, not a runtime parent-child or ownership relationship.
Do not infer airport membership solely from nearest-runway distance.

Each runway has a source name, XYZ position, angles, nationality, flags, speed
and alias. All fourteen have Y=0, angles 0/0/0, speed 0 and flags $4003.
Aliases run from -10100 through -10113. Preserve original spellings and signed
aliases. Nationalities are 12 or 137; these are source codes, not permission to
hard-code friendly/enemy status. Mission sides and overrides require resolution.
The source parser's position units are feet, with Y vertical. Zero source Y is
ground placement, not sea level. [Placement consumers and orientation evidence](native-strip.md#mission-and-current-object-state).
Do not replace the source runway locations with modern airport coordinates.

The existing top-level M/MM environment reader skips these indented fields.
The isolated STRIP parser is not a complete mission loader. A production reader
must bound records and fields, retain source identity/order and unresolved fields,
and report unsupported dependencies rather than silently omitting objects.
Map overlays, alias replacement and campaign-generated names need separate review.

## Explicit definition and visual references

The airport sections use 17 object types, plus STRIP.OT for their runways.
All eighteen OT definitions have explicit main-shape strings. Two examples show
why filename guessing is wrong: BUNKER.OT references BUNKB.SH and COMM.OT
references SHELT.SH. STRIP.OT references RUNWAY.SH.
The complete measured type/shape inventory is in the linked baseline.

All eighteen shapes produce nonempty geometry with the existing bounded gameplay
projection. Each exposes one named PIC texture, and all eighteen PICs extract
from FA_2.LIB. This establishes a usable initial visual dependency set, not full
SH branch, damage, LOD or collision coverage. No imported program is executed.
Follow definition pointers and reviewed shape records, never suffix substitution.

Runway contact boxes already provide ten position anchors and two orientation
records. Their provenance and limits belong in [STRIP metadata](native-strip.md).
Visible polygons must not become an assumed landing/collision surface. Decode
other object definitions using their actual layout before assigning health,
target eligibility or destruction behavior.

## Cross-theater requirements

The [cross-theater census](../baselines/ukraine-airports.md#cross-theater-planning-census)
identifies thirteen explicitly airport-labeled OT definitions selecting `_STRIPProc`.
The narrow `strip::Definition` parser deliberately accepts only STRIP.OT. Extend
with reviewed layouts rather than renaming other definitions to pass that reader.
STRIP3A/5A/6A/7A are separate placement types, not assumed damaged appearances.
Preserve each instance until its spatial/operational relationship is established.
FRA.MM also requires `sides2` and `nationality2`. Resolve allegiance using their
actual conversion contracts. Source record counts are not unique-airport counts.
