# Retail weather parity plan

Requested 2026-09-14. This is a code and existing-evidence review plus an
implementation plan. The initial planning pass established no new native behavior
or runtime parity.
Follow-up: [clock/turbulence research](formats/weather.md) now establishes
continuous time and distinguishes physical turbulence from maneuver sound.
Contrails and vapor trails are user-confirmed scope, including broader
wing-induced vapor beyond wingtips; their native contracts remain to recover.
Target the supplied Fighters Anthology build first, then validate every supported
base theater. Follow the roadmap's behavioral/asset fidelity definition; original
resolution and pixel-identical rasterization are not acceptance requirements.

## Current implementation

Updated 2026-09-15 after the [full implementation review](baselines/weather-review.md).
[Evidence](baselines/weather.md).

## Numbered dependency sequence — 2026-09-15

This is the current execution order requested after the implementation review.
The older W1–W7 sections below retain their contract detail and existing links;
their numbers no longer describe the order of remaining work. Each step finishes
the existing implementation where one exists, rather than replacing it.

| Step | Remaining deliverable | Prerequisite | Acceptance for the slice |
| --- | --- | --- | --- |
| **1** | **Fog, palette shading and sky foundation:** translate reviewed callbacks, persistent fog tint and its palette consumer; recover shade/remap tables, sky projection and horizon clipping | Existing LAY reader, clock and palette path | Fog evolution with fixed inputs; dawn/dusk boundaries; horizon/zenith and land/water comparisons |
| **2** | **Sun, moon and stars:** bounded original shapes, time-driven placement, scale, visibility, materials and horizon clipping | 1; shared CLI/app celestial dependencies | Day/night and transition captures looking toward/away from the sun, upward and across the horizon |
| **3** | **Cloud geometry:** source deck shapes, scattered-cloud defaults/distribution, mission deck altitude, coverage, transparency and draw order; source ocean/horizon deck where required | 1; finish visual composition with 2 | Below/inside/above cloud and fog bands, deck crossings, clear versus cloudy, second LAY family |
| **4** | **Weather in every camera:** sample altitude and visibility for cockpit, exterior, mirrors and camera instruments from one environment instant | 1–3 | Different-altitude simultaneous views; camera count/order cannot change clock, fog RNG or trail histories; wide/tall and performance checks |
| **5** | **Wind:** native omitted-wind defaults, explicit overrides, common wind/atmosphere input to flight and air data, verified wind-audio dispatch | Stable environment state from 1; follow camera acceptance in 4 | Calm/cardinal winds, air-relative versus ground-relative readings, both flight adapters without double drift |
| **6** | **Turbulence:** surface/daylight and preference gates, nearby-aircraft wake inputs, native coupling and rounding; retain per-aircraft event state and actual-output haptics | 5; explicit surface and nearby-aircraft inputs | Ground/airborne and wake probes, both aircraft and complete loops, fixed RNG and pause/restart; distinguish maneuver buffet |
| **7** | **Finish wingtip vapor; investigate broader wing-induced vapor:** decode patterned translucent fills, roll gate, scale and integer sample rounding; continue static aircraft/caller research for broader vapor | 1, 4 and verified aircraft response in 6 | G/roll/day/night boundaries, both aircraft, all camera paths, reset and material comparisons |
| **8** | **Serialized weather replay and final parity acceptance:** complete launch/restart identity and state, theater/condition matrix and retail/platform comparisons | 1–7 | Reproduce clock/RNG/callback/turbulence/trail state; all six conditions and 16 base theater profiles; Linux/Windows/macOS runtime evidence |
| **9** | **Future engine contrails:** continue evidence search; if unverified, implement only as an optional authored feature under W7 | Accepted retail weather; 4/5/7/8 for rendering, advection and replay | Separate engine attachments and formation policy, bounded lifetime, both aircraft, classic-mode behavior preserved |

**First batch: 1 → 2 → 3. Then: 4 → 5 → 6 → 7 → 8.** Step 9 remains a
future extension unless source evidence places contrails in the retail scope.
The first three reorder the previous gap list to put the shared rendering
foundation ahead of the objects that depend on it.

These are delivery checkpoints, not permission gates. Keep simulation state and
view sampling separate from step 1; step 4 integrates and verifies every camera.
Define seed, configuration identity and mutable-state ownership as each feature
lands; step 8 completes serialization and the combined replay gate. Collect
matched retail evidence throughout, not only at step 8. Wake probes can use
explicit supplied aircraft geometry without scheduling combat AI. Sensor
visibility consumers remain a separate source audit, not an inferred rule that
all sensors are blocked by visible fog.

### Step 1: next implementation slice

- [x] Resolve reviewed LAY callback imports to typed behavior with bounds checks;
  preserve the no-op horizon callback and reject unsupported behavior explicitly.
- [x] Keep mutable fog-record state and RNG outside immutable configuration.
  Translate the callback before time selection/blending, with reviewed cadence;
  sampling additional cameras must not invoke it.
- [x] Finish the tint consumer contract before applying the scalar to rendered
  pixels. The callback alone is insufficient: native view update publishes tint,
  and a later palette pass smooths and applies it to selected palette ranges.
- [x] Translate reviewed palette/remap operations, including indexed aircraft
  and selective cockpit tint; render normal full-detail horizon bands.
- [x] Recover deck/ground transition boundaries, above-sky transition/underside,
  lower solid size/inversion gates and per-shape fog control.
- [ ] Finish per-normal light remaps and assess special display-mode effects;
  compare native scanline rounding and horizon filtering with retail.
- [ ] Add synthetic boundary/state tests, imported-module diagnostics and matched
  fog/transition captures; run creator/viewer/flight checks after rendering edits.

Status: callbacks, live six-bit tint/smoothing and imported indexed haze remaps
are implemented. Named sky/ocean decks now use world-anchored ray/plane
projection, source altitude/tile scale and deterministic load-time wildcard
selection. The GPU now composes target-layer then view-layer indexed remaps, with
integer distance splitting and overlap restrictions. Normal full-detail Gouraud horizon bands and above-sky selection now render.
Textured transition polygons and special horizon branches remain open; these are recorded acceptance
gaps, not prerequisites for decoding the celestial and cloud primitives next.
The 60 ms palette cadence and separate RNG streams are deterministic host
adaptations. [Evidence](baselines/weather-foundation.md).

## Existing implementation coverage

| Area | Implemented | Gap |
| --- | --- | --- |
| Mission inputs | `layer` name and choice index, `clouds` altitude, `wind` degrees and feet per second, and time all recovered and consumed | Campaign mission sources; generated and explicit cloud altitudes now draw; campaign sources remain open |
| Weather records | Reviewed fields of the 352-byte record in `tore-formats::weather`, all 24 supplied modules parsing | Vapor fill-pattern tables and native comparison |
| Environment state | `tore-sim::environment` advances a 120 Hz to 256-unit clock, selects and blends records by time and altitude, and answers pure queries | Pause, compression and long-session audit against the original |
| Terrain and sky | Artwork uploads as source palette indices; the live palette resolves on the GPU each frame | Special horizon branches, low-detail cloud gates and celestial comparison/glare |
| Visibility | Recovered ramps, altitude haze and ordered cross-layer indexed remaps | Retail crossing comparison; sensor consumers |
| Creator | Six source weather choices launch; duplicate overcast removed | Label mapping remains inferred; serialized weather replay |
| Wind | Resolved to world feet per second and applied by both adapters | Missing-wind native defaults; wind audio |
| Air data | `telemetry::EnvironmentReading` accepts wind and atmosphere | Still no live producer |
| Turbulence | Reviewed event generator, with corrected duration/priority, driving authored coupling | Nearby-aircraft wake strength; the daytime ground-query flag |
| Wing vapor | Shape-supplied attachment, position history, trigger, roll shortening and night suppression | Patterned fills, roll gate, scale and integer sample rounding remain open |

Source foundations: [theater format](formats/theater.md),
[creator contract](formats/quick-mission.md),
[native flight research](formats/native-flight.md). SUN.SH has no identified
literal PIC dependency; do not assume a missing sun texture or author replacement
art. Cloud imagery in SKY0 does not establish a cloud system.

## W1 — Recover the environment contract and retail baselines

Build a repeatable weather static-extraction pass using the existing PE/SMS
research infrastructure. Gate fixed addresses on reviewed executable/symbol
hashes and identify module hashes. Preserve source-build distinctions. Importers
only interpret bounded data; they never load native weather callbacks.

Trace these existing leads through producers, consumers and callers:

- LAY selection/interpolation at FA `0x4b3820`, helpers `0x4b3b60` and
  `0x4b3b80`, palette copy/write spans documented in the theater specification.
- FOG callback `0x4b4320`: known random adjustment is insufficient without
  initialization, record-field meaning, RNG source and caller cadence.
- SUN/MOON/STARS references at `0x50c42c` / `0x50c434` / `0x50c43c`;
  SKY0–8 selection and cloud draw/update callers.
- Creator condition field 15: retain the seven-label source inventory as evidence,
  but expose six editor rows with overcast removed as a duplicate of cloudy.
  Validate the explicit editor-to-native mapping and launch metadata.
- `_windSpeed` / `_windH` producers and `_FMTurbulence` at `0x477590`.
  Establish units, direction convention, altitude dependence and ground behavior.

Inventory all supplied LAY variants and mission environment combinations. Keep
absent values distinct until native defaults/override precedence are recovered.
Continuous time and day wrap are now statically confirmed; finish the remaining
pause/compression, long-session and exact scheduling audit. Determine
whether celestial placement depends on theater, date or merely a source clock.

Audit additional weather-related behavior: stars, cloud bases/tops/coverage/drift,
inside-cloud visibility, ground/water/object shading, cockpit/HUD/night palettes,
wind audio, visibility/seeker effects and turbulence. Investigate precipitation,
lightning, icing and other candidates only as evidence warrants. Mark each
verified-present, verified-absent in the reviewed scope, or unresolved; lack of a
string reference is not proof of absence.

Wingtip vapor is confirmed in the dedicated streamer subsystem and in both
reviewed aircraft shapes. The new static aircraft-code pass resolves device
imports and afterburner branches, but finds no additional contrail or broad
wing-vapor trigger in those blocks. Whole-game absence remains unproved. Recover
the patterned fills, roll-rate gate, native scale and sample rounding before
calling the port exact. See [review findings](baselines/weather-review.md).

Create matched retail scenarios recording build, theater, condition, mission
time, position, altitude, heading, settings and elapsed time. Start with Ukraine
and a theater using a distinct LAY variant. Store media and derived traces under
ignored `.local/weather/`; publish procedures and conclusions in
`docs/baselines/weather.md` when evidence exists.

**Gate:** a field/caller/status matrix and reproducible retail baseline protocol;
unresolved contracts remain explicit. No generic weather algorithm substitutes
for missing native evidence.

## W2 — Bounded records and deterministic environment state

Extend `tore-formats` with typed reviewed weather records and narrow sky/cloud SH
support. Validate RVAs, sections, record/sentinel counts, arithmetic, resource
dependencies and palette ranges with synthetic fixtures. Keep CLI/app dependency
resolution, cache requirements and provenance aligned; update format coverage.

Put environment evolution in renderer-independent `tore-sim::environment`
(proposed module), using immutable validated configuration, caller-owned mutable
clock/RNG/state and explicit spatial queries. Separate world evolution from
camera-altitude-dependent presentation. Sampling for a mirror or another aircraft
must not advance weather or consume new random draws.

Advance authoritative state on the existing 120 Hz service. Translate native
arithmetic where verified; document clock adaptation and RNG policy rather than
claiming native scheduler parity. Persist configuration identity, initial time,
seed and required state in restart/replay contracts. Preserve pause/focus-loss
behavior without catch-up, and permit headless operation without an aircraft.

**Gate:** bounded malformed-input tests and identical state for identical tick
inputs, independent of rendering rate, camera count and query order. Compare
translated arithmetic against reviewed source expectations at boundaries.

## W3 — Time, palettes, sky, sun, moon and stars

Recover and implement record selection, altitude/time interpolation and native
sky mapping. Replace static midday construction with dynamic palette evaluation.
Prefer retaining source indices with GPU palette lookup over rebuilding the
terrain mesh or uploading all RGBA artwork every weather tick; verify filtering,
transparency and palette roles against retail before settling the GPU layout.

Render original celestial shapes/textures with recovered angular placement,
scale, visibility, horizon clipping and blend rules. Reuse the reviewed SH grammar
where applicable and extend only for needed commands. Preserve zenith coverage
without square-art pole collapse. Apply recovered dawn/dusk/night behavior to
ground, water, aircraft and cockpit where their native paths actually use it.
Do not assume modern directional lights, shadows or astronomical moon phases.

**Gate:** matched dawn, noon, sunset and night captures; boundary samples before,
at and after every recovered transition; full clock-wrap test if supported.
Verify front/back/up/exterior views, mirrors and camera instruments share the
same environment instant.

## W4 — Clouds, horizon and visibility

Implement recovered cloud geometry/layers, coverage and distribution, movement,
altitude, clipping, transparency and draw ordering. Distinguish sky-image cloud
art from independently rendered clouds. Replace authored distance fog with the
native recovered visibility and horizon rules, including FOG evolution.

Audit any visibility contribution to targeting or weapons separately from visual
fog; integrate only verified consumers. Do not equate obscured imagery with a
universal sensor-blocking rule.

**Gate:** clear/cloudy/foggy comparisons below, within and above cloud
or fog layers; horizon and land/water distance comparisons; deterministic
evolution and identical main/mirror/panel behavior at matching viewpoints.

## W5 — Wind and turbulence, separate from aircraft movement

Resolve raw mission wind into explicit world-space units using recovered native
semantics. Give flight and `telemetry::AirData` a consistent environment sample;
verify air-relative versus ground-relative velocity and avoid applying drift
twice. Address both the legacy and researched adapters explicitly, preserving
their provenance and F18/Rafale model separation.

Recover turbulence's inputs, recurrence, RNG draws, cadence and aircraft
`turbulencePercent` consumer before deciding the generator's exact API. A
headless service outside aircraft integration is the proposed architecture, but
native behavior may require per-aircraft state rather than a global wind field.
Follow-up establishes per-aircraft timed state, ground proximity, nearby-aircraft
geometry and daylight scaling; direct LAY/wind coupling remains unestablished.
Keep aircraft response and any view-only perturbation separate. Do not turn a
native angular perturbation into an invented gust-force model.

Use actual turbulence output for haptics. Recover wind/environment audio dispatch
before connecting original samples; a WIND resource name alone does not prove a
weather-dependent loop. Preserve instrument channel distinctions and explicit
atmosphere inputs. Additional weather features require W1 evidence and their own
acceptance cases.

**Gate:** calm/cardinal wind and ground/airborne probes; F18 and Rafale regressions,
complete loops, restart/pause/replay; independent generator tests and retail
response comparisons. Aircraft count, camera changes and haptic/audio enablement
must not accidentally alter weather random sequencing.

## W6 — Creator integration and parity acceptance

Pass recovered conditions into one validated launch environment shared by creator,
direct diagnostics, viewer, flight and restart. Remove each clear-only launch
restriction only when its supported contract is implemented. Preserve existing
setup/loadout checks and original labels; this work schedules no unrelated menus.

Add bounded diagnostic controls for condition/time, seed, elapsed ticks and
environment-state output (proposed flags, not available commands yet). Extend
input/combat replay metadata or version their formats as required so weather is
reproducible and incompatible recordings fail explicitly.

Acceptance matrix:

- All six supported conditions across all 16 base theater resource profiles;
  detailed native comparisons for each distinct weather behavior/LAY family.
- Representative low/high altitudes, transition boundaries, cloud crossings,
  cardinal headings, horizon and zenith; both reviewed aircraft.
- Wide/tall flight captures, cockpit/exterior, mirrors and camera panels;
  creator and viewer GPU smoke tests after rendering changes.
- Repeatable frame-time measurements using
  [existing bounded diagnostics](baselines/flight-performance.md), with matched
  clear and dense-weather runs. No live blocking readback or post-render sleeps.
- Formatting, warnings-denied Clippy, locked tests/build, Python checks and asset
  guards per [development instructions](DEVELOPMENT.md); real Linux, macOS and
  Windows checks recorded separately from compilation or headless success.

**Exit:** every in-scope weather matrix row has source evidence, implementation
coverage and retail acceptance evidence. Unknown behavior stays an open parity
gate. Publish tolerances and comparison limitations before calling a row accepted;
passing static helpers or attractive screenshots alone cannot close 1:1 parity.

## Execution and evidence

Use the numbered dependency sequence above for remaining work. W1 source
recovery, W2 state ownership and W6 acceptance apply within every numbered
step; they are not separate phases to postpone until rendering is finished.

## W7 — Optional engine contrails after retail weather

Scheduled as future work at the user’s request, not implemented by this review.
If further retail shape/caller research or matched captures establishes a native
contrail contract, move that behavior into the parity milestones. Otherwise ship
it as an explicitly authored optional feature, off in classic mode.

1. Recover engine nozzle attachments separately for F18 and Rafale from their
   shape/afterburner geometry. Do not reuse wingtip points or guess engine count.
2. Define the authored formation policy using explicit atmosphere/altitude and
   engine-running inputs. Existing weather lacks humidity/temperature producers;
   add validated configuration for those inputs or document a simpler selectable
   approximation. Do not label invented thresholds as recovered retail behavior.
3. Keep bounded trail histories in `tore-sim`, with fixed-tick emission, lifetime,
   wind advection, restart and replay state. Long-lived engine trails remain
   separate from the short G-triggered wingtip streamers and broad wing vapor.
4. Add transparent trail rendering with matching main/mirror/panel state and a
   bounded memory/frame-time budget. Select original suitable material only where
   evidence supports it; any new material must be clearly authored.
5. Test engine off/on, atmosphere boundaries, calm/crosswind, pause/restart,
   replay and both aircraft. Compare enabled/disabled captures and performance;
   confirm classic mode keeps its accepted retail behavior.

Before visual acceptance, close callback/tint application, scattered-cloud
defaults, per-camera altitude sampling and native visibility/remap behavior.
Wind defaults, turbulence coupling and serialized replay have their own later
delivery checkpoints; all remain required for whole-system acceptance.
Corrected helper tests and source-art screenshots alone do not close these gates.

### Step 2 implementation checkpoint

Original sun circles/glow remap, moon texture and 94 stars now render with source
time/flag/angle selection. Weather SH decoding is bounded and independent of
aircraft animation. The continuation implements sun-view whitening, nine original
lens-flare circles/remaps and the moon bank fix. Lower Gouraud horizon masking
is applied. Exact textured horizon clipping, pixel rounding and matched retail
acceptance remain open. See the continuation evidence in weather-foundation.md.

### Step 3 implementation checkpoint

Original CLOUD1 top/bottom geometry, cutout texture, runtime-imported nine-entry
layout, high-detail 4×4 repeats, generated altitude defaults and explicit mission
altitude now render. Crossings and a second theater were captured. The sixteen
CLOUDS billboards are decoded but have no established active producer; native
low-detail, forward-sector relocation and SH coordinate-range rejection now
work. GPU triangle clipping replaces native sphere/frustum work rejection;
integer edge rounding and matched retail comparisons remain open. The static
resource/producer audit still establishes no active CLOUDS.SH placement. Cloud
bands are source fog records, not evidence for an authored volumetric deck.
