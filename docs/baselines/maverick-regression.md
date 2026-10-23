# Maverick scenario regression

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-15, nr-mav from `f805054a`, protocol 38 pending
the lead's compatibility bump. Behavior: [solid contact](../formats/objects-and-shapes.md#grounding-every-object-stands-on-the-bottom-of-its-shape).

## Cause and correction

`ccba1db2`, integrated by `c8787c1c` (nr-objg), made line endpoints contribute
to `Placements::stance` bounds as well as footing. The bounds become
`StaticObject.bounds`, weapon targets through `Combat::register_airport_object`,
aircraft collision through `Terrain::solid_contact`, and flat roof contact.
This created empty solid volumes around thin line primitives. It predates
all three fox merges. Surface IR observation already bypasses aircraft aspect,
and the missile driver waits for READY with an acquired lock in these shots.

The fix separates complete drawn geometry (`grounding::points`) from solid
face/sprite geometry (`grounding::solid_points`). Placement lift, compact
slope lift, drawn geometry and the order of grounding points are preserved.
The shared contact box uses faces and sprites only. A frame can touch ground
below the body's contact box; the grounding audit now checks that the solid
box does not penetrate the ground rather than requiring it to reach the
frame's lowest point. Polygon towers remain solid. This is a fitted contact
rule, requested by the lead on 2026-10-15, not a retail collision reconstruction.

The initial attempt to re-aim the scenario was rejected by the lead: it
avoided the invisible wall rather than fixing it. The original BORE shot
(4 nmi, 3,000 ft) and Enter shot (3 nmi, 2,000 ft, original heading) are now
restored. The scenario asserting collisions with the inflated bridge box
was removed. Synthetic tests still require a genuinely solid intervening
body to intercept a locked missile.

## Complete surface audit

The local probe uses the repository's bounded archive and shape readers on
the imported profile plus FA_2.LIB and FA_1.LIB. It enumerates every OT/NT
main shape through `Placements::of_types`, the same projection used for
scene contact. Archive identities (SHA-256):

- FA_1.LIB: `657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f`.
- FA_2.LIB: `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`.

**250 decoded surface types; nine changed types, eight unique
shapes.** CRATER.SH reports the existing unsupported-native-reentry error
and has no projected contact box to change. No other unreadable shape was
reported. No line-only contact body appeared in this inventory. A post-fix probe
asserted actual `Placements::stance` min/max against the independently
computed face/sprite bounds for every decoded type: all 250 pass. This audit
covers the normal projected main shapes used by current collision, not
unexecuted retail callbacks or every possible animation branch.

Below are all changes, ordered by largest bound displacement. Values are
**feet at placement scale, relative to the authored shape origin**, before
any grounding translation. Each entry is `[min right, min up, min forward];
[max right, max up, max forward]`. The fixed local bounds equal the pre-change
face/sprite bounds, but keep current footing and slope translation.

| Type / main shape | Before ccba1db2, and fixed | ccba1db2 expanded box |
| --- | --- | --- |
| BRDMID.OT / BRDMID.SH | [-128.00, -488.00, -2456.00]; [128.00, 104.00, 2456.00] | [-136.00, -488.00, -2456.00]; [128.00, 1248.00, 2456.00] |
| OSCAR.NT / OSCAR.SH | [-33.33, 0.00, -289.33]; [33.33, 45.33, 202.67] | [-33.33, 0.00, -289.33]; [33.33, 80.00, 202.67] |
| CRANE.OT / CRANE.SH | [-14.67, -58.00, -77.33]; [14.67, 47.33, 122.67] | [-14.67, -58.00, -77.33]; [14.67, 81.33, 122.67] |
| KING.OT / KING.SH | [-38.67, -17.33, -1.33]; [36.00, 25.33, 17.33] | [-38.67, -18.67, -1.33]; [36.00, 25.33, 34.67] |
| GCI.NT / KING.SH | [-38.67, -17.33, -1.33]; [36.00, 25.33, 17.33] | [-38.67, -18.67, -1.33]; [36.00, 25.33, 34.67] |
| RES1.OT / RES1.SH | [-98.67, -33.33, -52.00]; [89.33, 80.00, 80.00] | [-98.67, -33.33, -52.00]; [89.33, 92.00, 80.00] |
| CGRP3.OT / CGRP3.SH | [-117.33, -26.67, -98.67]; [133.33, 53.33, 96.00] | [-117.33, -26.67, -98.67]; [133.33, 58.67, 96.00] |
| M2.NT / M2.SH | [-5.33, -5.67, -10.33]; [5.67, 4.33, 10.67] | [-5.33, -5.67, -10.33]; [5.67, 8.33, 10.67] |
| CLEM.NT / CLEM.SH | [-168.00, 2.67, -430.67]; [174.67, 89.33, 661.33] | [-168.00, 2.67, -430.67]; [174.67, 92.00, 661.33] |

BRDMID's decoded face top is z=13 and cable top z=156 at scale 8, giving
104 ft and 1,248 ft. BRDEND's polygon tower already reaches z=163 at scale 8,
**1,304 ft**, and its bounds do not change: `[-872, -488, -504];
[704, 1304, 1896]` feet. The middle section's cable does not fill the space
under it; the end's real tower remains in its fitted solid box.

## What the line primitives represent

The decoded `Line` contains endpoints, color and fog, with no thickness or
semantic label. A CPU projection of the complete polygons and lines was
inspected, with faces grey and lines red. The geometry establishes:

- BRDMID/BRDEND: segmented suspension cables with vertical hangers above
  the deck. BRDEND also has a filled polygon tower, independently reaching
  1,304 ft. The middle's filled deck/body tops out at 104 ft.
- CRANE: rigging and open framework around the polygon boom/body. Rigging
  raises the bound by 34 ft, without filling that space.
- KING/GCI: thin radar frame/support segments. They still establish the
  18.67 ft footing lift; the polygon body's bottom is 1.33 ft above it.
- OSCAR and M2: slender vertical segments above the hull/turret, consistent
  with mast/periscope or antenna detail. RES1 has a short rooftop vertical
  segment. These semantic names are geometric interpretations, not tagged
  fields in the resource.
- CLEM and CGRP3: short deck/roof/edge detail segments. Whether the author
  intended particular lights, aerials or outlines is unknown; the grammar
  only establishes thin lines. That distinction does not affect the contact
  rule. Further naming would require authored asset documentation.

The baseline table is measured decoded data. Local evidence is retained in
`.local/tmp-nr-mav/contact-audit-raw.log`, `contact_audit.rs`,
`contact-shapes.svg` and `contact-shapes.png`. No extracted art or geometry
is committed. Retail gameplay comparison was unavailable.

## Before evidence

- Exported `49938b4c`, the parent of ccba1db2: original five-shot Maverick
  scenario passes. BORE kills 0x40000090; Enter hits tank 0x50000006,
  leaving 20/200 HP.
- Exported `fef782db`, before fox3, and freshly built `f805054a`: original
  scenario fails with the same three reported errors. BORE remains locked
  on 0x40000090 but hits inflated BRDMID 0x40000085. Enter now selects that
  inflated middle section and hits near end 0x40000091. Selected HP stays
  100/100. Misses are 2,367 and 3,443 ft.
- Source attribution is the parent run plus isolated decoded bounds and
  the ccba1db2 change; its immediate post-change executable was not built.

## Validation

The original five-shot `a2g-maverick` passes with unchanged scenario geometry.
BORE kills `0x40000090` at t=16.700 s (0/100 HP), and Enter hits
`0x50000006` at t=13.575 s (20/200 HP). Both match the pre-regression
export exactly. The other CUED/FLIR/6-nmi shots also pass.

Clippy and the locked workspace binary build pass. World tests: 987 passed,
3 ignored. Simulation tests: 1,996 passed, 8 ignored, plus one ignored doc
test. App testing: 1,846 passed, 69 socket-startup-related failures, 37
ignored, and two unbounded lobby-startup tests excluded after interrupting
their socket-blocked run. Python: 472 passed, two socket environment errors.
These failures are the sandbox's UDP restriction, not contact regressions.
All **41/41 selected scenarios pass**: the full `a2g-*` set, AI Maverick,
surface-grounding, surface-grounding-sites and surface-start-placement.
GPU-only surface-grounding-radar-render was not run because GPU access is
unavailable. The quick guard ran all 32 commands successfully on a private profile copy,
with JOBS=3 and nice 10. Against guard-base-334a70e7: SAME 34 / DIFFERENT 18 /
MISSING 0. Direct comparisons with both tmp-nr-fox2/guard-final and
tmp-nr-bombphys/guard-final give SAME 52 / DIFFERENT 0 / MISSING 0. Thus all
18 older-baseline differences are inherited; this change adds none, and
there are no additional bomb-rework differences in the curated guard.
Logs: tmp-nr-mav/guard-contact-fix.log, guard-contact-vs-fox2.log and
guard-contact-vs-bombphys.log. No goldens or baseline recordings were changed.

Protocol remains 38 in the worktree for the lead to bump. Peers must share
the corrected contact-box construction and imported geometry even though
no serialized fields changed. The final report is
`.local/next-round/reports/nr-mav.md`, section "Contact bounds fix".
The extra SLAM, Shrike, AS-15 and AS-30L scenarios and the synthetic
intervening-solid missile tests are retained.
