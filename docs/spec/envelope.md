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

Manual identity: local `.local/missile-update/manual.pdf`, SHA-256
`1a082378a8e8cd163ed6b398efcc1df80b67c2f104f6b90ac0733c88d58e26c3`.
PDF pages 94-96 carry printed pages 90-92. The manual illustration is blue-gray;
the three screenshots supplied by John on 2026-09-21 show teal fills, pale
readouts and yellow/cyan square markers. Their executable build and aircraft
identities are unknown. They establish appearance, not exact scale or timing.
No claim of measured retail execution parity is made.

## Fitted presentation rules

Agent choices, 2026-09-21, unless explicitly stated otherwise:

- Plot only within raster x=11..148, y=21..134 in the existing 160x156 instrument.
  Use the selected aircraft's own imported PT polygons. Axis maxima are 1.06
  times maximum speed and 1.12 times ceiling over its nonempty positive-G rows.
  Compare mode uses the union of both aircraft's bounds. Zero is the lower left.
  This keeps the entire envelope inside the window without a universal
  2,000 ft/s or 70,000 ft scale. Mode changes do not rescale U versus A.
- Use filled scanline spans. Available G is the largest positive row containing
  the point. Current mode chooses the nearest available row to rounded live G,
  including negative rows. Its fill is the dark 1G color.
- RGB colors for 1..9G: (44,91,107), (48,101,117), (56,114,131),
  (67,131,149), (78,144,161), (86,151,167), (94,158,174), (101,163,178),
  (108,169,183). Clamp higher rows to the lightest band. Background below stall
  is (81,145,161), beyond maximum speed (128,177,188), above ceiling
  (177,207,213). Readouts use (198,225,228). These are screenshot color fits,
  not recovered palette indices.
- John requested a multicolor shifting square on 2026-09-21. Use a 4x4 square,
  cycling yellow (232,248,40), cyan (96,207,233), white (239,249,246),
  one color per 20 simulation ticks at 120 Hz. Clamp the entire square inside
  the plot when the aircraft is outside chart bounds. Pausing freezes its phase.
  The manual describes a white dot; exact retail color sequence and cadence
  are unknown. Next research step: inspect the original marker palette update.
- Round feet, knots (ft/s divided by 1.68781) and live G to whole numbers.
  Default to A, retaining the previously visible collection of curves.
- Compare requires a selected acquired sensor contact and that exact aircraft's
  available profile. No lock or missing data produces an explicit status.
  Red (175,72,73) means the target's highest containing positive-G row is larger
  than the player's at that point. Exact retail advantage blending is unknown.

The imported PT geometry and manual behavior are spec-derived. RGB values,
scales, rounding, comparison predicate and marker cadence are fitted.
