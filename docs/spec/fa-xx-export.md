# F/A-XX original-format export contract

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-18, donor changed 2026-09-22. John requested
exporting the concept for use in original Fighters Anthology and reusable tools
for future exports, then moved the donor to the retail F-22N. The
[gameplay concept](fa-xx.md) remains the design target. This document specifies
the **fitted export adapter**, with agent-authored choices and explicit limits.
It does not change T.O.R.E gameplay or any flight-adapter defaults.

## Delivered candidate

The exporter emits a separate aircraft: FAXX.PT plus FAXX.SH and
FAXX_A/B/C/D/S.SH, all derived from the F-22N donors F22N.PT and
F22N.SH/_A/_B/_C/_D/_S.SH. The PT names it F/A-XX / F/A-XX Concept and
identifies itself as FAXX.PT. Shape and shadow references point into the new
family. The donor's hook capability bit is already set, so the PT flags are
copied unchanged. All other definition fields, including aerodynamic settings,
cockpit/HUD, equipment and availability, remain donor values. The package has
no F22- or F22N-named resource entries.

The original catalog enumerates `*.PT` definitions, so a per-aircraft PT supplies
the identity rather than requiring a replacement of an existing aircraft slot.
Evidence and the shadow-derived damage-name contract are recorded in the
[packaging baseline](../baselines/fa-xx-packaging.md). John reported successful
original FA flight of the earlier F-22A-based package on 2026-09-18; the F-22N
based package has not yet been flown in original FA.

The B/D fragments and S shadow are unchanged donor aliases. Copying the shadow
under FAXX_S.SH is necessary because original setup derives related damage names
from that shadow filename. Keep textures and equipment shared with the recipient's
stock files: the shapes reference the stock `_F22N` textures. Automatic campaign
roster inclusion, custom cockpit discovery and Kapset-specific conflicts/load
order remain outside validation.

Also produce a LIB containing exactly those seven resources, reports and a ZIP.
Static startup inspection confirms arbitrary .LIB names are indexed from the
working directory. Copy FAXX.LIB beside FA.EXE and launch with that folder as
the working directory. Install the LIB alone, not its duplicate loose payloads.

The earlier `--identity f22` replacement mode, which overwrote the stock F-22A
in place, was removed on 2026-09-22 at John's request: with the F-22N donor it
would have replaced the retail carrier Raptor.

The intact model omits the five reviewed fin faces and both separate fin decals;
both damaged bodies omit their reviewed fin masks. Addresses have one home in
[objects and shapes](../formats/objects-and-shapes.md). Detached-fragment and
shadow geometry remain unchanged under the new aliases. Bypass the original C8
LOD branches so unreviewed distant fin geometry is not selected through those
branches. Original-game behavior at distance has not been validated.

## Control states and presentation

The source files bind the authored branches to the existing `_PLrudder`,
`_PLleftFlap` and `_PLrightFlap` symbols; `_PLhook` is already imported by the
F-22N donor and its branch is left untouched. OpenFA's state table supplies the
endpoint interpretation used for this candidate. Static original-code review
establishes that the hook capability permits the hook command, which sets/clears
the deployed state consumed as `_PLhook` 0/1; the F-22N PT carries that
capability as shipped.

The candidate's data contract is:

| State | Authored output |
| --- | --- |
| Rudder 0 | One closed skin per inboard flap face |
| Rudder +1 | Right leaves at midpoint minus/plus 0.6 radians |
| Rudder -1 | Left leaves at midpoint minus/plus 0.6 radians |
| Corresponding flap -1 | Midpoint 0.4 radians |
| Other supported flap endpoints (-2, 0, +1) | Midpoint zero |
| Hook 0 | No hook faces (donor behavior) |
| Hook 1 | The donor's two native hook faces, lowest vertex at source z=-23 |

Use the geometry and hinge rules in the concept contract. Neutral geometry and
texture coordinates, except the omitted fins and their decals, match the stock
donor. Restore shared flap vertex slots after drawing each authored leaf group.

**Fitted differences:** only discrete endpoint poses are exported. There is no
continuous rudder opening or three-second hook travel; the hook shows the
donor's stowed/deployed endpoints. Integer SH vertices round leaf positions to
source units. At the existing scale, one source unit is four inches. No
arrestment logic is exported.

Hold the donor's original flap-state skin switches at their neutral selection;
the seven inboard concept flap faces supply the authored flap movement. This
bypasses the donor's alternative flap/brake skins, including changes outside
those seven faces. Preserve other donor device blocks and resource references.
Do not claim original FA reproduces T.O.R.E handling merely because its F-22N
PT is unchanged. No aerodynamic model is exported.

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
only names and geometry references differ and that the donor already carries
the hook capability bit, matching B/D/S alias bytes, no F22/F22N resource
collisions, decoded geometry agreement for 24 gear/flap/rudder/hook
combinations including the donor's own hook faces, finless damage-body
agreement, and exact LIB payload recovery. Synthetic tests cover jump bounds,
geometry constants and failure detection. These checks do not establish
original-game loading, draw order, palette appearance, key availability, timing
or Kapset compatibility. The recipient must check those before treating the
package as a working mod.
