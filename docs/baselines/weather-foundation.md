# Weather foundation implementation — 2026-09-15

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


First bounded implementation slice of dependency step 1. Steps 1–3 are not
complete. Existing uncommitted review changes were preserved; nothing was
committed or pushed. Source identity is the reviewed FA.EXE/SMS pair in
[weather research](weather-research.md).

## Changes

- LAY callbacks resolve through bounded CODE aliases and import records to
  typed Rust behavior. Unsupported symbols, libraries, ordinals and aliases
  fail explicitly. Imported native code is never loaded or executed.
- Environment state owns mutable record copies and a validated, separately
  seeded RNG. Matching fog records mutate before selection/blending and retain
  their changes. Selection uses one second after a callback, ten otherwise,
  with early reselection on leaving the first active interval. Sampling cannot
  consume randomness or change time. Seed 1 and the dedicated RNG stream are
  authored defaults, not retail shared-RNG or scheduler parity.
- Altitude haze uses the source tint RGB at `+0xfb`. The previous implementation
  incorrectly used the remap shade at `+0x36`.
- Pure palette helpers preserve the reviewed selective ranges, signed rounding,
  odd-strength truncation, capped instrument range and smoothing overshoot.
  **These helpers are diagnostic: rendered palettes still ignore tint scalar.**
  View-dependent reduction, complete palette ordering/cadence and native remaps
  must be recovered and integrated before visible fog tint is complete.
- Static extraction includes altitude haze, tint helpers and celestial dispatch.
  `--validate-weather` now reports callback/tint fields and shape coverage.

## Validation

- Formatting, warnings-denied workspace/all-target Clippy, locked workspace
  tests and build passed: **261 Rust tests**, **24 Python tests**.
- Asset guards passed for source and debug app/extractor binaries.
- Synthetic checks cover callback aliases/truncations, persistent fog mutation,
  immutable configuration, camera-query purity, selection intervals and early
  interval exit, haze color source, and palette range/rounding boundaries.
- Imported validation parsed all **24 LAY modules**. Full-day probes for DAY2
  and FOG1 passed; all six condition launch/altitude checks passed. Six FOG
  variants reference the fog callback; other supplied records have null callbacks.
- Linux Vulkan / RTX 4070: creator and fog viewer smoke tests passed. F/A-18D
  fog at 1280×720 after 240 probe ticks and Rafale dusk at 720×960 captured
  successfully and were visually inspected. These confirm composition and
  existing weather rendering, not the unintegrated tint helpers or retail parity.
- Bounded active fog run: 330 frames, first 30 excluded, zero paused frames,
  frame interval mean **1.27 ms**, p95 **1.40 ms**; simulation/cameras mean
  **0.09 ms**; 150 mirror renders, zero completed readbacks. CPU wall intervals
  include presentation backpressure. No matched before-run was made; these are
  neither GPU timings nor proof of a performance improvement.

Captures, provenance and GPU logs are under ignored `.local/weather-foundation/`.
Build/test logs use `/tmp/weather-foundation-*.log` on the reviewed host.
The hash-gated static pass is reproducible with:

```sh
python3 tools/extract_native_flight.py --domain weather --source gameassets/fighters-anthology --out .local/weather-foundation/native
target/debug/tore-app --weather-condition 2 --validate-weather
target/debug/tore-app --viewer --weather-condition 2 --no-audio --smoke-test
target/debug/tore-app --quick-mission --no-audio --smoke-test
```

The initial selective extraction encountered the unrelated WB installer LIB;
rerunning with both `disc1/LHX/*` and `disc1/WB/*` archive exclusions avoids
treating those unrelated formats as EALIB resources.

## Remaining dependency work

1. Finish view-dependent tint reduction/state, palette ordering/remaps and native
   sky/horizon projection. Keep fixed-tick state independent of camera queries.
2. Decode and render original celestial primitives. Current static projection
   stops at SUN opcode `0x13` / VA `0x1034`; MOON and STARS yield no accepted
   geometry. Source dispatch flags/angles are traced, but full clipping,
   materials and rendered movement remain open.
3. Decode cloud primitives and recover distribution/deck composition.
   CLOUD1 yields two static faces; CLOUDS yields no accepted geometry. Neither
   result establishes native transparency, placement or deck crossings.

No matched retail capture, Windows runtime or macOS runtime check was available
in this pass. None of the numbered weather parity gates is marked complete.

## Live palette and deck foundation — 2026-09-15

- Bounded root `+0x6c` reader decodes 48-byte shade headers and up to ten
  256-entry index remaps. FA `0x4b3ad0` chooses the first minimum Manhattan RGB
  distance; `0x4b3410` quantizes density and saturates the final level.
- Tint reduction at `0x4b36ae` is gated by layer overlap. The selected object's
  `+0x34` is signed 24.8 speed, confirmed by the object/context copy into
  `0x50ceb4`. Fields `+0x12e/+0x132` supply maximum reduction/speed cap.
- The palette worker at `0x486e80` invokes the palette pass every fourth 15 ms
  iteration. Host presentation owns smoothing/reduction/RNG and uses nominal
  60 ms fixed-tick passes; pause behavior, startup phase and independent seeded
  streams are authored scheduling, not native replay parity.
- Imported terrain remaps run before palette lookup/filtering. Aircraft retain
  their own palette and fitted RGB haze. Native cross-altitude ray composition
  (`0x4b31f0`) remains open; GPU distance/filtering are not the integer rasterizer.
- Named sky/ocean decks use the plane altitude, `2^exponent` feet tile size and
  reversed Z texture coordinate verified in `0x447aa5` and `0x448400`. Wildcards
  resolve once in record/deck order. CLI/app extraction now includes OCEAN PICs
  and rejects old caches missing them. The native special horizon fill and
  above-sky branches remain open; horizon minification is visibly aliased.
- Validation: 263 Rust tests, Clippy, build and 24 Python tests passed. Linux
  viewer and F18 capture passed; `.local/weather-foundation/planes.png` was
  visually inspected. These establish a working GPU path, not retail equality.
- Follow-up creator smoke and hash-gated static extraction passed. Bounded active
  fog sample: 330 frames, 300 measured, zero paused frames; mean 1.35 ms, p95
  1.53 ms, simulation/cameras 0.13 ms. CPU intervals include presentation;
  this is not a matched before/after performance claim. Artifact guards passed.

## Celestial slice — 2026-09-15

- Separate bounded, straight-line weather SH reader: source vertex slots/axes,
  fill changes, circles, point stars, UVs and textured billboards. It rejects
  executable opcodes; the sun's projected-point publication is recorded without
  writing imported pointers. The aircraft shape reader is unchanged.
- Import validation now requires successful weather primitive decoding: SUN
  seven concentric circles, MOON one billboard, STARS 94 points, CLOUDS sixteen
  billboards. CLOUD1 retains two polygon faces. The moon uses `_MOON.PIC` (41²).
- Sun placement translates the source inclusive time/flag gate, integer arc and
  signed-WORD reflection. Moon uses LAY angles; stars retain source directions.
  Rendering is independent of camera translation. The GPU uses floating-point
  projection, one-pixel star quads and horizontal horizon clipping; native
  screen rounding/horizon dip remain comparison work.
- Sun fill 267 resolves through LAY root `+0x50`. Its six outer circles repeatedly
  remap the background index (native `0x497836`); the solid inner source fill
  remains indexed. No replacement sun texture or fitted glow color is used.
  Glare through the separately published `_sunPoint` remains unimplemented.
- Linux captures: `celestial-sun.png`, `celestial-night.png`, `moon-viewer.png`
  inspected under `.local/weather-foundation/`. The first moon cockpit poses
  placed it behind the canopy frame; the unobstructed viewer confirms the
  original textured disc and stars. This is not a retail side-by-side gate.
- Celestial checkpoint checks: 265 Rust tests, Clippy with warnings denied,
  locked build, 24 Python tests, creator/viewer/flight GPU checks and asset
  guards passed. Night active sample: 330 frames/300 measured, zero paused;
  mean 1.38 ms, p95 1.57 ms. No matched retail or Windows/macOS runtime check.

## Cloud geometry slice — 2026-09-15

- Reviewed EXE hash-gated reader imports nine 26-byte cloud descriptors from
  `0x50c298`; only inert placement/rotation/mask fields enter `TORE_CLOUDS_V1`.
  The original executable and shape modules are never loaded as code.
- `0x4a8b90/0x4a8ba0` load `CLOUD1.SH`, replace descriptor Y with mission cloud
  altitude in 24.8 feet, then call the repeat helper. High detail uses a 4×4
  supercell: 144 placements, base period 131,072 feet. The renderer translates
  nearest-period wrapping and replaces native frustum optimizations with GPU
  clipping. Low-detail preference dispatch remains an integration gap.
- Source CLOUD1 header exponent 10 gives `2^(10-8)` geometry scale. Its two
  coincident horizontal faces supply opposite winding/UVs. Select the visible
  side and use `_CLOUD1.PIC` (256×79), palette indices and index-255 cutout.
  GPU bilinear half-coverage cutout is an adaptation; no volumetric density is invented.
  The shape's bounded native reentry was statically reviewed: it ORs bit 2 into
  `_effectsAllowed` before returning to shape data. No imported code executes.
- Native generated choices 0/3/4 make a 50% draw, then choose 7,000–19,999 feet.
  Generated launches now use that rule with a dedicated seed-one host stream.
  Ordinary MM launches preserve explicit cloud altitude, including zero; omitted
  MM cloud altitude defaults to zero. `TORE_CLOUD_ALTITUDE` is a bounded diagnostic
  override. Resolved altitude stays in mission identity across weather resets.
- `CLOUD1.LAY` has no named texture decks. Its fog/whiteout bands remain distinct
  from the scattered-cloud producer. `CLOUDS.SH`'s 16 billboards decode, but an
  active global producer has not been established; they are not placed arbitrarily.
- Linux captures below/above and one foot either side of a 10,000-foot sheet
  passed. Ukraine and Egypt use the original asset over their respective terrain.
  F18 wide and Rafale dusk tall composition captures passed. `cloud-below.png`,
  `cloud-above.png`, `cloud-egy.png` and `cloud-flight-tall.png` were inspected.
  All are under ignored `.local/weather-foundation/`; these are host evidence,
  not matched retail acceptance.
- Synthetic checks cover cache truncation/values/provenance rejection, periodic
  placement, half-period ties, zero altitude, query purity and generated defaults.
  Shared CLI extraction fixtures now exercise sky/ocean/cloud dependencies.
- Cloud checkpoint: 267 Rust tests, Clippy, locked build, 24 Python tests,
  hash-gated static extraction, creator smoke, 24-module validation and asset
  guards passed. Active clear/cloud sample: 330 frames, 300 measured, zero
  paused; mean 1.83 ms, p95 2.02 ms. This differs in scene from the earlier fog
  sample, so it is not a matched regression measurement. The final cloud pass
  writes opaque depth after a half-coverage cutout test, preventing farther
  vapor from drawing through cloud pixels; its GPU smoke passed.

## Ordered cross-layer fog — 2026-09-15

- Translated `0x4b31f0` into pure CPU ray queries and the indexed terrain/cloud
  shader. Both preserve target-before-view remap ordering, adjacent-boundary
  distance splitting, nonadjacent saturation, overlap restrictions and signed
  WORD distance units. Normal-view bias is explicitly zero. Source indices are
  remapped before palette lookup; cutout tests retain the original texel index.
- Synthetic checks distinguish noncommuting map order and cover upward/downward
  crossings, same/nonadjacent layers, overlap, distance rounding and signed wrap,
  invalid distance and query purity. GPU projection/interpolation/filtering remain
  adaptations; own-palette aircraft fog and sensor visibility are separate work.
- Linux RTX 4070/Vulkan captures `ray-clear-cloud.png`, `ray-in-cloud.png`,
  `ray-above-cloud.png` and `ray-night.png` were inspected. Clear sheets, band
  whiteout and stars remain visible in their respective cases. Wide F18/cloudy
  and tall Rafale/dusk captures, creator smoke and all 24 module diagnostics pass.
  Artifacts are under ignored `.local/weather-foundation/`.
- Final checks: 269 Rust tests, 24 Python tests, formatting, Clippy with warnings
  denied, locked workspace build and repository/binary asset guards passed.
  The additional signed-wrap/NaN assertions passed in the focused ray test rerun.
- Repeatable active sample: `TORE_PERF_FRAMES=330 TORE_PERF_ACTIVE=1
  TORE_CLOUD_ALTITUDE=10000 target/debug/tore-app --free-flight
  --weather-condition 0 --no-audio --window-size 1280x720`. 300 measured frames,
  zero paused frames/readbacks; frame interval mean 1.54 ms, p95 1.71 ms.
  This is host CPU/presentation evidence, not GPU timing or a retail comparison.
- Special horizon/above-sky branches, celestial glare/native clipping, low-detail
  cloud dispatch and unresolved CLOUDS placement remain open. Windows/macOS and
  matched retail acceptance were not exercised; steps 1–3 are not marked 1:1.

## Continuation: horizon bands and aircraft/cockpit palettes

The merged starting head was `a43db12`, with a clean tree and no unpushed commits.
Normal full-detail background bands and above-sky selection now use the recovered
Gouraud contract. Exterior indices share live weather/fog; cockpit indices retain
their 64-color prefix and receive the selective native tint. Original art remains
runtime imported. See [contract and limits](../formats/weather.md#horizon-and-shared-aircraft-palettes--continuation-2026-09-15).

The user confirmed that the retail Windows comparison machine is still being
built. Matched retail captures are unavailable; local GPU captures cannot close
that gate. Required retail cases are listed in the continuation acceptance section.

Continuation foundation checks: **270 Rust tests**, **24 Python tests**, fmt,
warnings-denied Clippy, locked build and repository/binary asset guards passed.
Creator/viewer Vulkan smoke and inspected above-sky, F18 dusk wide and Rafale
dawn tall captures passed on RTX 4070. Artifacts: `.local/weather-continuation/`.
Active clear/cloud benchmark (same command as the preceding checkpoint): 330
frames, 300 measured, no paused frames/readbacks, 330 rear renders; mean **2.26
ms**, p95 **2.57 ms**. The earlier recorded 1.54 ms was not rerun as a paired
measurement; no unchanged-frame-time or regression-isolation claim is made.
Windows/macOS runtime and matched retail acceptance remain unavailable.

### Celestial continuation — 2026-09-15

- Fixed the reported moon bank distortion: both textured-quad axes now use the
  world celestial rotation. The old mixed camera-right/world-up basis sheared
  the disc during banking. Camera rotation still rotates the whole world in
  the image; it no longer changes the moon's own basis. A synthetic geometry
  regression covers five rolls, translation, orthogonality and edge lengths.
- FA `0x4b4170` / `0x4cd8b0` consume sun/view alignment for whitening. The
  recovered target uses the Q15 dot result, threshold `0x3ccc`, division by three
  and 0..255 saturation, with the source daylight/elevation gates. Presentation
  smooths it alongside fog tint; `0x4c8e6c` whitens palette entries 0..254 before
  fog tint. Fixed flight ticks supply the authoritative current view alignment;
  render queries do not advance it. Float view-to-Q15 conversion remains an
  adapter, not native scheduler or whole-frame rounding parity.
- FA `0x4b4990` consumes `_sunPoint` for nine original lens-flare circles from
  `0x50c8d8`. Runtime import preserves their offsets, radii and fills. LAY root
  pointers +0x48/+0x4c provide indexed remaps 265/266. The source one-degree
  elevation, screen bounds and center dead-zone gates are applied. Glare is
  composed after the world and before cockpit/UI. GPU filtered RGB is resolved
  to the nearest live palette index inside circles before source remapping;
  this and drawable-size projection are explicit GPU adaptations. No CPU
  readback or imported machine-code execution is involved.
- The lower native Gouraud band also masks celestial fragments through its
  five-unit upper edge. Special textured horizon transitions and native
  screen/pixel rounding still need work; no Earth-curvature model was invented.
- Every record in all 24 supplied LAY modules has moon angles 8190/3640,
  sunrise/sunset 25200/68400 and sun azimuths 18200/-18200. The traced angle/draw
  paths consume these fields and time, with no theater latitude or calendar
  input. This establishes the reviewed placement contract, not absence from
  every executable subsystem.

Original moon art, sun circles/glow remap and 94 stars remain intact. Host glare
preference defaults on and has a diagnostic override; the native preference UI
is not implemented. Matched retail captures remain unavailable while the user
builds the Windows test machine.

Validation: 274 Rust tests and 24 Python tests pass, as do formatting, locked
build, warnings-denied Clippy and all asset guards. Linux RTX 4070 Vulkan
creator/viewer, F18 1280x720 exterior and Rafale 720x960 dawn flight captures
passed. Ignored captures are under `.local/weather-continuation/`: `lens-flare`,
`moon-bank-{0,45,-45}`, `glare-f18`, `celestial-rafale`. The exterior F18 capture
is an integration check, not evidence of visible glare. Normal 330-frame active
flight measured 2.10 ms mean / 2.20 ms p95, with 330 mirrors and no readbacks.
A head-look sunward run measured 1.54 / 1.82 ms with mirrors outside the view;
these differ in workload and are CPU frame intervals, not GPU timings or a
paired performance comparison. Retail/platform parity is not closed.

### Cloud visibility continuation — 2026-09-15

FA `0x4a8bd0` dispatches detail >=2 through the 4x4 repeat helper; lower detail
passes the base period directly, yielding nine candidates rather than 144.
`0x4a9660` publishes forward heading sectors: +Z [-10920,10920], -Z outside
(-21840,21840), +X [5460,27300], -X [-27300,-5460]. At pitch >=-8190,
`0x4a8130` moves a periodic representative one whole period forward if its
shape radius lies strictly behind a selected sector. Steeper downward views
skip that relocation. `CLOUD1.SH` bounds are radius 1889, exponent 10: 7556 feet.
The pure view query now applies these rules; roll and repeated sampling do not
advance weather/RNG. Host radians-to-word rounding is an explicit adaptation.

Cloud callers use first-bucket mask 3, maximum signed distance and terrain class
-1; subsequent zero masks do not remove these candidates. They overwrite the
Y coordinate with mission cloud altitude and bypass ground lookup. The queue
at `0x4a8c30` stores the anonymous shape record; it is not a frustum test. Its
consumer `0x4a7c26` passes the shape into `0x4d057c`. That entry rejects any
absolute camera-minus-center component >=32767 after the signed SH exponent
shift. This observable coordinate-range gate is now applied before upload,
including its asymmetric negative rounding boundary. For CLOUD1 this is roughly
131068 feet per axis. The following `0x4d028c` sphere/frustum tests avoid work on
out-of-view shapes; GPU triangle clipping remains the host adaptation for them.
Native integer edge rounding and finite native queue exhaustion are not claimed.

Audited 1654 FA_2.LIB resources: 1275 SH, 145 PT, 135 JT, 75 MM and 24 LAY.
No direct CLOUDS.SH or zero-terminated CLOUDS reference occurs. All LAY shape
fields name wave1.SH; the reviewed cloud initializer/descriptor producer loads
CLOUD1.SH. This bounds the negative evidence; constructed names, other paths
and runtime appearance are not disproven. The 16 CLOUDS.SH billboards remain
decoded but unplaced. No time/wind input appears in the traced cloud placement;
camera-relative periodic relocation is not cloud drift.

Original cutout pixels remain tested before fog remapping, opposite faces are
selected across the altitude plane, and depth resolves world/cloud occlusion.
The host's filtered alpha cutoff is an adaptation, not source pixel acceptance.
`TORE_CLOUD_DETAIL=0|1|2` exposes cloud preference behavior (default 2) without
claiming the deferred graphics menu or lower-detail terrain raster is complete.

Validation: 276 Rust tests, 24 Python tests and required locked/lint/asset checks
pass. Linux Vulkan low/high viewer captures, altitude crossing, creator smoke,
F18 1280x720 and Rafale 720x960 flight smoke pass. The forward low/high captures
are visually identical after coordinate-range rejection; synthetic tests cover
candidate-count differences and the downward exception. Active 330-frame F18
flight: 2.09 ms mean, 2.30 ms p95, 330 mirrors, zero paused/readback frames.
This is CPU interval evidence, not GPU timing or retail comparison. Ignored
producer-audit JSON and captures are in `.local/weather-continuation/`. The
broad extraction reported five unrelated non-EALIB installer archives; the
FA_2 resource inventory above completed and its provenance is retained.

### Horizon transition and shape fog continuation — 2026-09-15

Recovered `0x447f2f` / `0x4481a0` / `0x448585`: the texture boundary uses deck
height at two million feet along the unrolled view direction, while the other
transition boundary uses ground height at eight million feet. Original indexed
endpoints F3/F4 (F1 in the single-deck cases) interpolate to F0. The GPU now
projects these boundaries and interpolates palette indices; the old radial cap
and flat fallback strip did not represent this geometry. Native 16-iteration
scanline searches, integer trigonometry and one/two-pixel padding are replaced
by analytic float projection and remain pixel-parity qualifications.

The above-sky empty-name mode-1 call at `0x4ab00c` draws only a transition, then
the upper Gouraud and celestial primitives, then the lower solid horizon and
actual sky underside. The port now represents that virtual transition, lower
solid selection and actual underside. The unusual virtual altitude argument is
25,600,000 **feet**: `0x447ac9` compares the passed value to view Y shifted down
by eight. It is not a 100,000-foot fixed-point altitude.

`0x4aad7e` selects the lower solid Q15-plane offset using clip dimensions and
integer roll/182. Large views use 20 normally and 90 inverted; medium views
0/200; small 0/300. The pure helper preserves these gates. Shader plane scaling
is a float-camera adaptation. Celestial clipping follows the lower solid/deck
or Gouraud consumer instead of imposing zero elevation everywhere. The lower
Gouraud also overlays sky texels through Y=5, matching native draw order.

SH opcode CA at `0x4d4288` modifies fog bit 0x10: zero disables, two disables
when weather flag 0x40 is set, other nonzero values enable. `0x4b3541` publishes
that flag. The static reader now retains a typed fog mode per face, and aircraft
vertices carry it through indexed color and texture remapping. Ordinary terrain
and cloud vertices retain their existing enabled mode. Source per-normal light
remaps and other display-mode effect masks remain separate work; this does not
claim complete native shape execution or authored HUD parity.

Validation: 278 Rust tests, 24 Python tests, locked build, warnings-denied Clippy,
formatting and source/binary asset guards pass. Imported neutral F18/RAF/CLOUD1
faces all retain enabled fog (282/206/2); this audit found no neutral face that
needed a disabled mode. Synthetic fixtures exercise the recovered disabled and
conditional cases. Linux creator/viewer GPU checks passed, with dusk, dawn,
above-sky bank and horizon captures in ignored
`.local/weather-continuation/`. A 330-frame run had presentation stalls (2.99 ms
mean / 12.29 p95); a 630-frame repeat measured 2.09 / 2.34 ms, max 5.21, with
630 mirrors and no paused frames/readbacks. Neither result is GPU timing. Retail
comparison and Windows/macOS execution remain unavailable in this session.


### Per-normal aircraft lighting — 2026-09-15

LAY root +14/+18 supplies shade count/pointers; +40/+44 supplies highlight
count/pointers. All 24 reviewed modules contain seven shade and six highlight
rows. The bounded reader retains these original 256-index maps. FA 0x4cd854
shifts each Q15 normal/light product before summation; 0x4cc4b4 selects a shade
row for nonnegative amounts or a highlight row below -192. The reviewed object
path enables negative highlights. Polygon subtype bit 0x20 applies these maps
to flat and textured faces (0x4d4619 / 0x4d46b7), before fog. Subtype bits 0x60
carry the source normal. Neutral F18 has 275 lit faces of 282, Rafale 201 of
206; CLOUD1's two faces do not request this light operation.

Weather light direction (0x4b35b8..0x4b364a) uses the inverted daytime sun arc
or night moon angles, independently of the sun draw flag. Tests cover both
inclusive day boundaries. The app packs light-row and fog selectors into the
existing indexed vertex field and shares the original maps with solid/texture
shading. Animated normals and directions are quantized from the existing float
orientation; native integer matrix parity remains unclaimed.

Validation: 281 Rust tests, 24 Python tests, formatting, warnings-denied Clippy,
locked build and source/binary asset guards pass. Linux Vulkan creator/viewer
checks and actual F18 1280x720/Rafale 720x960 flight captures pass, including
Rafale cockpit at dusk. Correct flight images use `--capture-flight` and are
named `lighting-{f18,rafale,cockpit}-flight.ppm` in ignored
`.local/weather-continuation/`. Earlier `horizon-f18`, `horizon-rafale`,
`lighting-f18` and `lighting-rafale` files used `--capture-terrain`, which selects
the viewer even after `--free-flight`; those are viewer evidence only. The
corrected flight captures validate the cumulative horizon/fog/lighting path.
A 630-frame active F18 run measured 2.17 ms mean / 2.56 p95, max 5.38, with
630 mirrors and zero paused frames/readbacks. CPU intervals are not GPU timing.
Matched retail and Windows/macOS checks remain pending external access.


### HUD palette consumer — continuation 2026-09-15

FA `_HUDDraw` at 0x406ad6 reads 0x5213d2 and sends the byte directly to the
indexed raster color setter (0x497680); there is no object distance/light remap
in that call. HUD initialization at 0x406193 copies 0x2b2 bytes from the loaded
HUD CODE root to 0x521360. The bounded HUD reader therefore reads CODE+0x72,
not file offset 0x72. Both F18.HUD and RAFALE.HUD select index 40 in the private
cockpit palette. Authored HUD geometry and other instrument colors are separate.

`HUDBrightness` (0x40aac0) changes a signed value by 16, clamping -256..256;
0x4b2f24 initializes it to zero. The palette worker at 0x4b3f74 changes just
index 40: positive `c += (63-c)*amount >> 8`, negative
`c = c*(256+amount) >> 8`. Endpoints give white/black. This occurs before sun
whitening, and index 40 is outside fog's 47..60 / 64..254 tint ranges. The app
now shares one resolved palette between cockpit artwork and primary HUD ink.
Brightness controls use the recovered range/step instead of the fitted green
formula. Preferences version 2 stores the signed amount; version 1 is accepted
with explicit host migration `(old-7)*16`, preserving step distance from the old
default without claiming equivalent fitted colors. Other preferences survive.

Tests cover bounded module extraction, brightness endpoints, order/tint exclusion,
control saturation and preference migration. The complete ordered native palette
pipeline still includes non-weather effects beyond this slice.

Validation: 284 Rust tests, 24 Python tests, formatting, warnings-denied Clippy,
locked build and source/binary asset guards pass. Imported diagnostics read both
HUD indices. Linux Vulkan creator/viewer smoke and actual F18 noon wide / Rafale
dawn tall cockpit captures pass and were inspected (`hud-*.ppm` in ignored
`.local/weather-continuation/`). A 630-frame active F18 run measured 1.63 ms mean,
1.76 p95, max 5.98, with 630 mirrors and no paused/readback frames. This short
CPU interval sample does not establish a causal performance improvement.

### Final weather sampling and batch checkpoint — 2026-09-15

The reviewed ordinary flight/viewer implementation for batch 1–3 has landed.
This is **implementation evidence, not retail or all-platform acceptance**.
The user's Windows retail setup is not ready; no imported native module was run.

Source scanline 0x449cea..0x449e68 copies one original sky index at each integer
texture position, then 0x449f2d..0x44a04d remaps the temporary bitmap. Final
G_AcTexture dispatch at 0x44a064 reaches the indexed byte-copy raster. Sky/ocean,
moon and cloud GPU textures now use single-index samples and original cutout
indices, removing the added bilinear weather filter. The shared dark sRGB
conversion now includes its linear segment so palette black stays black.
The native intermediate sky bitmap, fixed-point scanlines, edge padding and
matrix rounding are still adapted to direct GPU projection. Terrain/aircraft
and cockpit filtering outside these weather textures remains qualified.
The roadmap excludes pixel identity/original resolution: compare visible art,
coverage and behavior, not exact scanline bytes.

Special display audit: all 24 LAY ordinary map-0 rows are identity. CPDraw's
alternate map selection is view-specific. INFO2Draw sets the solid C6/C1/C7
horizon override and clears it after its draw. Low-detail terrain horizon DF/D4
selection also depends on a ground-query flag; ordinary full-detail uses ED/FC.
These are identified display consumers, not missing ordinary weather defaults.
Their integration stays with step 4 or the separately scheduled INFO2 screen;
cloud-only detail settings do not enable a lower-detail terrain renderer.

Validation on Linux / RTX 4070 / Vulkan:

- Formatting, warnings-denied all-targets Clippy, locked workspace tests/build:
  **284 Rust tests** pass. **24 Python tests** pass; static extraction completes.
  Source, debug app/extractor and newly built release-extractor asset guards pass.
- Both imported aircraft pass the same **13-scenario flight suite** via
  `tools/extract_assets.py --aircraft f18|rafale --validate-flight`, using the
  documented LHX exclusion and separate ignored extraction directories.
- **42 GPU capture cases** and a creator smoke pass. Commands, weather inputs,
  poses and results are retained in ignored
  `.local/weather-continuation/final/manifest.json`. Coverage includes ten
  dawn/dusk times, eleven cloud-fog overlap altitudes, FOG1 weather, sky zenith,
  above-sky bank, moon at -60/0/+60 degrees, sun glare off/on, original cloud
  patch below/on/above the sheet at detail 0/2 and an overhead cutout overview.
  Actual flight captures include F18 wide cockpit and Rafale tall exterior in
  Egypt (EGY), with earlier final-code HUD captures covering both cockpits.
- Inspected representative montages/captures confirm host day/night transitions,
  lower and upper fog blends, original cloud cutouts, opposite cloud faces,
  horizon/zenith coverage, visible sun flare circles and unwarped banked moon.
  The initial generic sheet positions did not lie under a patch; the additional
  crossing views use a recovered descriptor center at X=1,073,152/Z=614,400 ft.
  Host screenshots alone cannot establish native equivalence.
- Three sequential active 630-frame F18 runs, 1280x720, no audio, first 30
  frames excluded, 630 mirror renders each, zero paused frames/live readbacks:

  | Condition | Mean ms | p95 ms | Max ms |
  | --- | ---: | ---: | ---: |
  | Clear | 1.70 | 2.05 | 5.30 |
  | Cloudy | 1.63 | 1.67 | 12.48 |
  | Clear repeat | 1.65 | 1.75 | 3.85 |

  These measure CPU frame intervals including presentation backpressure;
  the cloudy maximum includes a submission/presentation stall. They are not
  GPU timestamps, sustained maximum-load or verified displayed FPS evidence.

#### Precise remaining dependencies

1. Matched retail scenarios once runtime access exists: same build, mission,
   theater, pose, time and preference values across dawn/dusk; fog overlap
   boundaries; cloud crossings/cutouts; sun toward/away; moon bank; above-sky,
   horizon and zenith. Repeat Ukraine with a distinct LAY family. Assess
   coverage, scale, transitions and visible aliasing under the roadmap's scope.
2. CLOUDS.SH contains 16 decoded billboards but has no established active
   producer after the source queue/frustum trace, nine-descriptor audit, all
   LAY shape fields and 1,654 FA_2 resource audit. A retail appearance scenario
   or resource-load/caller trace is needed to discriminate additional paths.
   No speculative placement or game-wide absence claim is warranted.
3. Alternate camera/display maps, low-detail terrain/sky preferences and INFO2
   override integration require their actual consumers. Step 4/its separately
   scheduled screen owns this work; the standard full-detail path is covered.
4. Windows and macOS runtime/build/capture acceptance is unavailable here.
   Linux validation does not close those platform rows.

The weather plan and top progress checklist now supersede stale earlier entries
that still listed callbacks, glare, light maps, HUD ink or cloud detail as absent.
Steps 4–9 remain scheduled; this batch does not claim serialized weather replay,
wind/air-data/audio completion, turbulence or wing-vapor/contrail acceptance.
