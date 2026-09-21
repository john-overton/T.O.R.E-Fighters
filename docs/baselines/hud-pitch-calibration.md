# Pitch ladder calibration validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-21. John reported that compressed ladder labels
advanced ahead of actual pitch and disappeared near vertical. The corrected
behavior is specified in [HUD layout](../spec/hud-layout.md#flight-display).
No retail comparison or retail-parity claim is made.

## Findings and checks

The previous transform compressed around the world-projected horizon. Its
center-meridian 85-degree mark crossed the forward point at approximately
57.44 degrees actual pitch. As the horizon projection diverged near vertical,
visible marks moved outside the clip; a forward-depth threshold then switched
back to uncompressed projection. World-bearing rungs also narrowed toward zero
width at the poles, and the drawn range excluded +/-90 degrees.

The new local attitude scale compresses relative pitch and motion together.
Numbered rungs use that calibrated scale. The zero bar is now a separate
world-projected horizon reference so it meets the level-flight velocity marker;
see the [horizon and creator follow-up](horizon-creator.md).
Aircraft orientation, simulation and world-projected flight-path/weapon cues
are unchanged. Five-degree steps span 27.2763 reference pixels near the forward
point at unit zoom, independent of absolute pitch. Rungs retain finite width.

Synthetic tests cover:

- Every five-degree pitch from -90 through +90, seven banks from -180 to +180,
  and zoom 0.5, 1 and 4. Actual pitch is centered; five-degree motion and spacing
  agree, and rung widths match the level-attitude reference.
- A 1,440-step full orientation loop, including both poles and inverted flight.
  Every sampled orientation has visible rungs and pole markers move continuously.
- The actual HUD drawing path with a synthetic aircraft and blank synthetic
  font: every five-degree pitch, including +/-90, draws its corresponding rung
  at the forward reference.
- Existing projection, bank, flight-path marker and weapon cue regressions.

All required checks passed on Linux:

- Formatting, warnings-denied workspace/all-target Clippy and locked build.
- Locked workspace tests: 968 passed, two existing GPU unit tests ignored.
- Python tools: 68 passed.
- Source and both binary asset guards, documentation headers, diff whitespace.
- Explicit application smoke test on NVIDIA RTX 4070 / Vulkan.

## Rendered evidence and limits

Ignored `.local/hud-pitch-calibration/` contains command logs and captures.
An isolated raster harness uses the current projection, Paint and ladder-drawing
source with the user's runtime-decoded HUD11 font. Inspected frames cover level,
+/-70, +/-85, +/-90 and +85 with 45-degree bank. Yellow side ticks in the review
sheet mark the forward reference only; they are not added to the product HUD.
The review sheet is `ladder-review.png`; no retail font or generated derivative
is committed.

A full GPU cockpit capture, `loop.ppm`, was inspected after
`--capture-flight ... --flight-probe-ticks 720 --maneuver loop`. It verifies the
integrated HUD, cockpit clipping and steep/inverted presentation. The synthetic
tests, rather than an inferred screenshot angle, establish exact calibration.
Windows, macOS and interactive pilot acceptance were not run in this pass.
