# F/A-XX original-format export contract

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-18. John requested exporting the concept for use in
original Fighters Anthology and reusable tools for future exports. The
[gameplay concept](fa-xx.md) remains the design target. This document specifies
the **fitted export adapter**, with agent-authored choices and explicit limits.
It does not change T.O.R.E gameplay or any flight-adapter defaults.

## Delivered candidate

The default `--identity faxx` emits a separate aircraft: FAXX.PT plus FAXX.SH
and FAXX_A/B/C/D/S.SH. The PT names it F/A-XX / F/A-XX Concept and identifies
itself as FAXX.PT. Shape and shadow references point into the new family. All
other definition fields, including flight settings, cockpit/HUD, equipment and
availability, remain donor values. The package has no F22-named resource entries.

The original catalog enumerates `*.PT` definitions, so a per-aircraft PT supplies
the identity rather than requiring a replacement of an existing aircraft slot.
Evidence and the shadow-derived damage-name contract are recorded in the
[packaging baseline](../baselines/fa-xx-packaging.md). This establishes the
registration mechanism. John subsequently reported successful original FA flight
and supplied the screenshot recorded in the packaging baseline.

The B/D fragments and S shadow are unchanged donor aliases. Copying the shadow
under FAXX_S.SH is necessary because original setup derives related damage names
from that shadow filename. Keep textures and equipment shared with the recipient's
stock files. Automatic campaign roster inclusion, custom cockpit discovery and
Kapset-specific conflicts/load order remain outside validation.

Also produce a LIB containing exactly those seven resources, reports and a ZIP.
Static startup inspection confirms arbitrary .LIB names are indexed from the
working directory. Copy FAXX.LIB beside FA.EXE and launch with that folder as
the working directory. Install the LIB alone, not its duplicate loose payloads.
John confirmed successful original FA flight and the decal-removal revision on
2026-09-18. Detailed control/damage behavior and Kapset compatibility remain
unverified.
Retain `--identity f22` as the explicit earlier three-shape replacement mode.
The separate default follows John's request on 2026-09-18; retaining donor
settings and sharing resources are agent implementation choices.

The intact model omits the five reviewed fin faces and both separate fin decals; both damaged bodies omit
their reviewed fin masks. Addresses have one home in
[objects and shapes](../formats/objects-and-shapes.md). Detached-fragment and shadow geometry remain unchanged under the new aliases. Bypass the original C8 LOD branches
so unreviewed distant fin geometry is not selected through those branches.
Original-game behavior at distance has not been validated.

## Control states and presentation

The source files bind the authored branches to the existing `_PLrudder`,
`_PLhook`, `_PLleftFlap` and `_PLrightFlap` symbols. F31 and F14 donor import
tables establish the existence of rudder and hook symbol references. OpenFA's
state table supplies the endpoint interpretation used for this candidate.
Live original-game producer values, sign and hook availability on the F-22
remain **unknown** until reviewed or tested by the recipient.

The candidate's data contract is:

| State | Authored output |
| --- | --- |
| Rudder 0 | One closed skin per inboard flap face |
| Rudder +1 | Right leaves at midpoint minus/plus 0.6 radians |
| Rudder -1 | Left leaves at midpoint minus/plus 0.6 radians |
| Corresponding flap -1 | Midpoint 0.4 radians |
| Other supported flap endpoints (-2, 0, +1) | Midpoint zero |
| Hook 0 | No hook faces |
| Hook 1 | Twelve deployed hook faces, lowest vertex at source z=-23 |

Use the geometry, hinge and hook-angle rules in the concept contract. Neutral
geometry and texture coordinates, except the omitted fins and their decals, match the stock
donor. Restore shared flap vertex slots after drawing each authored leaf group.

**Fitted differences:** only discrete endpoint poses are exported. There is no
continuous rudder opening or three-second hook travel. Integer SH vertices round
positions to source units. Hook x coordinates have minimum magnitude one source
unit to prevent the thin shank collapsing; the shoe and shank consequently have
the same width. At the existing scale, one source unit is four inches. Hook
bottom z=-23 remains exact. The exported hook geometry has no arrestment logic.

Hold the donor's original flap-state skin switches at their neutral selection;
the seven inboard concept flap faces supply the authored flap movement. This
bypasses the donor's alternative flap/brake skins, including changes outside
those seven faces. Preserve other donor device blocks and resource references.
Do not claim original FA reproduces T.O.R.E handling merely because its F-22 PT
is unchanged. No aerodynamic model is exported.

## Export boundary and acceptance

Read only the exact reviewed donor hashes recorded in the
[packaging baseline](../baselines/fa-xx-packaging.md); reject another edition or
modified donor. Never edit source media in place. Require a fresh output directory
and the patched static OpenFA build. Disable upstream's x86 analysis pass for
all conversion. Preserve existing drawing-record addresses, append the new
endpoint routines, relocate affected absolute code references and import aliases,
and compile the complete shape through OpenFA. Imported machine code is treated
as data, never executed by the corrected toolchain.

Acceptance for this **experimental candidate** includes a parsed PT check that
only names and geometry references differ, matching B/D/S alias bytes, no F22
resource collisions, and decoded geometry agreement for
24 gear/flap/rudder/hook combinations, finless damage-body agreement, and exact
LIB payload recovery. Synthetic tests cover jump bounds, geometry constants and
failure detection. These checks do not establish original-game loading, draw
order, palette appearance, key availability, timing or Kapset compatibility.
The recipient must check those before treating the package as a working mod.
