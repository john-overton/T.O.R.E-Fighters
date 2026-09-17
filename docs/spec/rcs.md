# RCS instrument and aircraft exposure

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-16. This records the RCS panel requested by John. The
supplied screenshot is a visual reference; behaviour comes from the manual and
inspected FA build. [Evidence](../formats/radar.md#rcs-panel-and-signature-coupling),
[validation](../baselines/radar.md),
[the authored component](../radar.md#rcs-instrument-and-shared-aspect-model).

## Player-visible retail function

The [Fighters Anthology manual, pages 94-95](https://pdfcoffee.com/famanual-pdf-free.html)
describes a passive display of surrounding radar emitters. Bearings are relative
to ownship: 0 ahead, 90 right, 180 behind, 270 left. Its central contour indicates
exposure; an emitter inside is more likely to detect ownship, not guaranteed to
have a lock. Manoeuvring and configuration changes affect exposure. Symbols are
shared with the warning receiver; the manual associates squares with ground
sources. The screenshot's three squares are therefore consistent with ground
radar sources, but it does not identify their type or current lock state.

## Static FA findings

The Rust app's page 0 now draws the authored exposure contour, emitter symbols
and view scale described in the
[component guide](../radar.md#rcs-instrument-and-shared-aspect-model); the
placeholder it replaced is gone. The findings here are what that authored panel
was written against. The inspected FA helper has two
uses: contour dimensions and adjustment of a nonzero base radar signature.
It is called by both scope drawing and COSig. Do not treat the retail contour
as unrelated decoration, or equate its radii directly with detection distance.

For contour drawing, the base front/side dimensions in native design units are:

| PT radar signature | Front | Side |
| --- | ---: | ---: |
| Below 10 | 1 | 3 |
| 10 through 20 | 2 | 5 |
| Above 20 through 50 | 3 | 8 |
| Above 50 | 4 | 10 |

These are display units, not square metres or nautical miles. Angle conversion
uses 182 source units per degree with integer truncation. Pitch outside +/-10
degrees enables an addition of floor(abs(pitch_degrees)/20) to the front value;
the first nonzero addition is at 20 degrees. Bank adds floor(folded_bank/15)
to the side value, where folded_bank rises 0..90 from level to knife-edge and
falls back to zero at inverted level. Thus a 60-degree bank adds 4 side units,
a 90-degree bank adds 6, and a 180-degree bank adds none. Reviewed gear flag
0x40 contributes 2 front and 4 side units.

Other configuration flags and player globals contribute additional terms; their
complete meanings are unresolved. With a nonzero base signature the helper
returns an adjusted signature rather than those drawing dimensions, with
alternative weights under a type/player flag branch. COSig adds further
configuration effects. Do not copy the drawing buckets as the detection model.

## Boundary with the authored panel

The authored replacement uses one aircraft-aspect calculation for detection and
the exposure display. Its observer-relative response and smooth contour are
opinionated choices in the [component guide](../radar.md), not these original
integer rules. Active transmissions and physical reflection stay distinguishable
in that model. No autonomous threat behaviour is included.

Remaining retail questions: precise RCS zoom steps, full contact eligibility and
symbol-state mapping, all configuration modifiers, and how closely the contour
predicts each emitter's detection range. The shipped panel's 5/10/20/30/50 nmi
scales are an agent reuse of the warning receiver's set, not a recovered answer
to the first of those. This source evidence was sufficient to scope the panel
without claiming those contracts are complete.
