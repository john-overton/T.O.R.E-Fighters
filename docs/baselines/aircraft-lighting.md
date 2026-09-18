# Surface lighting and shadow validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-18. Validates the
[surface lighting and geometric shadow spec](../spec/surface-lighting.md).
The shared GPU surface path supersedes the earlier aircraft-only direction fix.
All tuning is opinionated; this is not evidence of retail parity.

## Automated evidence

The explicitly invoked GPU test uses synthetic geometry and the production
shader, shadow maps and material pipelines. Pixel readbacks establish:

- An opaque object shadows a terrain receiver, even when its exterior is hidden.
- Terrain geometry shadows an opaque object receiver.
- Shadows fall west at 09:00 and east at 15:00 with east/west solar azimuths.
  The sunward control pixel remains unchanged. Fully shadowed test receivers
  are less than half as bright as the corresponding lit pixels.
- A vertical blocker casts onto an inclined receiver with the sun half visible
  at the horizon. This catches the former zero-light-at-sunset failure.
- Moving the view across a painted surface changes its solar highlight by more
  than twelve linear 8-bit red levels; the matte terrain control changes by at
  most two levels.
- Water's production solar-reflection function is tested with terrain and object
  blockers. A blocked pixel returns the 0.05 linear water base within one 8-bit
  level, while the unblocked glint exceeds 80 levels and the unoccluded control
  is unchanged. The probe isolates solar reflection from ripple noise and art.
- With sunglare disabled, a caster 500 feet above the receiver produces at least
  four more partially shadowed pixels across the tested edge than the same
  caster 2 feet above it. This tests contact-dependent softness through the
  production geometry maps and fragment shader.
- Eleven low-sun frames cover five seconds in half-second increments with small
  camera translations. A 32 by 32 pixel patch on an inclined receiver changes
  by at most two linear 8-bit levels per channel between frames. Sunglare is off.
- Low-sun land contrast is checked on a synthetic grey slope: at 17:00 the
  back slope's red channel falls from its preceding 20 levels to at most 15.
  The 07:00 sun-facing slope, noon slope and nighttime slope retain reference
  red values 90, 56 and 24 within two linear 8-bit levels. The existing water,
  painted-panel, temporal-stability and contact-shadow checks still pass.
- Stepped compatibility draws no new geometry shadows.
- Fully transparent texture cutouts, glass, emissive glints and textured flames
  do not cast opaque silhouettes.
- A grey surface facing low morning sun has a red channel at least ten linear
  8-bit levels above blue. Advancing the sun by one minute changes each channel
  by at most two levels.

A separate GPU glare test renders a synthetic color gradient through the actual
lens-flare shader with deliberately empty remap tables. Every RGB channel under,
between and outside two circles agrees with continuous optical composition to
within one 8-bit level; alpha stays unchanged. Both GPU tests passed explicitly.

CPU projection tests cover overhead and low oblique sunlight at million-foot
world coordinates, depth ordering and bounded texel snapping. Visible-disc
checks verify full/half/zero energy, downward light from partial segments,
finite directions and continuous fading over 201 horizon samples. Separate
checks verify 1, 30, 50 and 100 percent visible disc fractions within 0.002
percentage points. Additional CPU checks verify shared terrain-edge normals
across different material indices, continuity through the former overhead
projection-axis switch, and sun motion within a single whole second. Existing sun-arc
tests cover horizon crossings and noon continuity. The GPU test is ignored in
ordinary headless test runs and was explicitly run successfully on this host.

All required checks passed on Linux: formatting, workspace Clippy with warnings
denied, workspace tests, workspace build, 40 Python tests, source and both binary
asset checks, documentation checks, and the display/GPU smoke test. Cargo
validation used the locked dependency set. New documentation headers were also
checked explicitly because the standard checker enumerates tracked files.

## Display and performance evidence

An F/A-18D oblique exterior capture at 18:00 was inspected. Low-sun terrain
striping in the initial implementation was corrected by increasing shadow
raster depth bias. The corrected capture has no visible striping in that view.
Egypt terrain at 17:00, Rafale C at 08:00 and F/A-18D at 22:00 were also
captured and inspected. A straight terrain seam in Egypt is present in the
stepped control capture too; it is not introduced by geometric shadows.
Matched F/A-18D captures at 19:00, chase view with a 90-degree orbit and 2x
zoom, were inspected after the stronger-shadow request. The revised image
separates the warm sunward fuselage from cooler wings and distinguishes tail
facets while retaining the original panel art. Before/after images and the
latest checks are in `.local/shadow-contrast-validation/`; earlier captures
remain in `.local/surface-lighting-validation/`. These captures do not reproduce
the user's exact mission, aircraft paint or camera state.
The subsequent glare/reflection checks and matched 19:00 viewer captures are
in `.local/glare-reflection-validation/`. Those captures were inspected for
rendering artifacts but do not reproduce the user's exact sunset scene. The
synthetic GPU readbacks establish blocked-glint suppression and gradient
continuity, rather than claiming a retail visual comparison.
The attempted `KOR` theater capture failed because that resource name was not
available; it does not count as a successful validation.

After stabilization, a 180-frame 1280 by 720 active flight run at 16:00 on an
NVIDIA GeForce RTX 4070, Vulkan, cycled cockpit and exterior views with sunglare
disabled. It reported zero paused frames and 90 rear-mirror renders. After
excluding the first 30 frames, mean frame-start interval was 6.00 ms, p95
16.61 ms and maximum 16.89 ms. Mean UI composition was 0.96 ms and
submission/presentation 4.75 ms. The first stable-filter version averaged
17.30 ms; avoiding unnecessary cascade/filter work reduced that cost while
retaining the GPU regression results. These are short CPU wall-time runs,
not GPU timestamps, certified displayed FPS or a cross-platform benchmark.

The latest required-check logs, temporal GPU test, coastal capture and timing
log are in `.local/stable-shadow-validation/`. The 19:00 coastal capture with
glare disabled was inspected for rendering artifacts. It does not reproduce
the user's exact scene or prove the absence of every possible flicker. The
controlled GPU sequence establishes the five-second stability check. Original
terrain artwork and material-color boundaries are retained; the new shared
normals address lighting seams without repainting the source tiles.

The land-only low-sun follow-up is validated in
`.local/low-sun-land-validation/`. Matched 18:50 coastal captures were inspected:
shaded land darkens without raising sunlit brightness. The sampled 200 by 25
pixel water patch at (20, 320) is byte-identical before and after. All required
checks and the expanded GPU test passed for this follow-up. This capture is
not the user's exact hill or camera position.

Windows/macOS execution, retail comparison, long-session performance and all
possible imported material variants were not validated. Finite map coverage,
small distant silhouettes and transparent cloud volumes retain the limits in
the spec. No retail artwork or derived captures were committed.
