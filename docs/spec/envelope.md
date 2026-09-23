# Flight envelope instrument

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Player behavior and evidence

The Fighters Anthology retail manual, printed pages 57, 59 and 90-92, describes
clean-aircraft speed/altitude polygons. Darker shading means fewer available Gs.
Altitude increases upward; speed increases rightward. Altitude in feet is at the
upper left, current G at the upper right, airspeed in knots at the lower right.
The aircraft marker follows the same coordinates as the envelope.

U selects the current integer-G curve. A displays all positive-G curves, with
higher G bands drawn lighter. C compares against a locked target, shading regions
where it can sustain more G than the player red. The curves do not predict
performance with the current fuel, weapons or damage load.

In U mode the single dark region tightens as the pilot pulls. Two retail frames
supplied by John on 2026-09-21 show one aircraft before a pull (1 G, 494 KTS)
and during a pull of about 5.7 G (HUD `5.70`, 393 KTS). During the pull the stall
edge moves right, the ceiling drops and the fast edge moves in, on the same axes.
Everything outside the region shows the background shades laid out around the
smaller region: the space it gave up is not a separate shade. The G readout in
the pull frame is too blurred to read the digit, but it shows two glyphs and no
decimal point, so whole-G display is kept.

Manual identity: local `.local/missile-update/manual.pdf`, SHA-256
`1a082378a8e8cd163ed6b398efcc1df80b67c2f104f6b90ac0733c88d58e26c3`.
PDF pages 94-96 carry printed pages 90-92. The manual illustration is blue-gray;
the three screenshots supplied by John on 2026-09-21 show teal fills, pale
readouts and yellow/cyan square markers. Their executable build and aircraft
identities are unknown. They establish appearance, not exact scale or timing.
No claim of measured retail execution parity is made.

## Fitted presentation rules

Agent choices, 2026-09-21, unless explicitly stated otherwise:

- Plot only within the 138 by 114 instrument screen: window x=12..149, y=20..133
  in the 162 by 160 window ([bezel geometry](instrument-bezel.md)).
  Use the selected aircraft's own imported PT polygons. Axis maxima are 1.06
  times maximum speed and 1.12 times ceiling over its nonempty positive-G rows.
  Compare mode uses the union of both aircraft's bounds. Zero is the lower left.
  This keeps the entire envelope inside the window without a universal
  2,000 ft/s or 70,000 ft scale. Mode changes do not rescale U versus A.
- Use filled scanline spans. Available G is the largest positive row containing
  the point.
- U is the default mode (John, 2026-09-23). It shows one row, stepping per whole
  G with no interpolation (John, 2026-09-23): the row nearest the live load
  factor rounded to a whole number, so 1.4 G shows 1 G and 1.6 G shows 2 G. The
  load factor is the flight model's achieved normal G, read every frame. Rows
  run contiguously from the aircraft's lowest to highest PT row, so G above the
  top row keeps the top row and the region never jumps back to a larger curve;
  G below the bottom row keeps the bottom row. Between -0.5 and 0.5 G the 0 G
  row shows when the aircraft has one, and a push shows the nearest negative
  row. The retail frames cover only 1 G and about 5.7 G; low, negative and
  beyond-maximum G presentation is an agent choice, and the next research step
  is a retail frame during a push and one above the top row.
- Backgrounds divide everything outside an outline into three shades. Stall
  lies left of the outline and straight up from the slow end of its ceiling.
  High lies above the outline, from that ceiling point across to the outline's
  fastest point. Fast lies right of the outline below its fastest point. U uses
  the selected row as the outline; A and C use all positive rows together. A
  flat ceiling takes its slowest point; a vertical fast edge takes its lowest
  point, matching the one retail frame with a near-vertical fast edge. Exact
  retail handling of a perfectly vertical edge is unknown.
- RGB colors for 1..9G in A mode: (44,91,107), (48,101,117), (56,114,131),
  (67,131,149), (78,144,161), (86,151,167), (94,158,174), (101,163,178),
  (108,169,183). Clamp higher rows to the lightest band. The U fill is
  (48,99,118), measured in both the 1 G and 5.7 G retail frames. Stall is
  (81,145,161), fast (128,177,188), high (177,207,213); the retail frames read
  within 4 of these. Readouts use (198,225,228). These are screenshot color
  fits, not recovered palette indices.
- John requested a multicolor shifting square on 2026-09-21. Use a 4x4 square,
  cycling yellow (232,248,40), cyan (96,207,233), white (239,249,246),
  one color per 20 simulation ticks at 120 Hz. Clamp the entire square inside
  the plot when the aircraft is outside chart bounds. Pausing freezes its phase.
  The manual describes a white dot; exact retail color sequence and cadence
  are unknown. Next research step: inspect the original marker palette update.
- Round feet, knots (ft/s divided by 1.68781) and live G to whole numbers. A
  light push reads 0 G, never -0 G.
- Compare requires a selected acquired sensor contact and that exact aircraft's
  available profile. No lock or missing data produces an explicit status.
  Red (175,72,73) means the target's highest containing positive-G row is larger
  than the player's at that point. Exact retail advantage blending is unknown.

The imported PT geometry and manual behavior are spec-derived. RGB values,
scales, rounding, background layout, comparison predicate and marker cadence are
fitted. The U default and whole-G stepping are opinionated, requested by John on
2026-09-23.
