> **Frozen as of 2026-09-15. Superseded by the parity strategy (D30) — see [AGENTS.md](../../AGENTS.md) and [the parity plan](../parity-plan.md).** Kept for its recovered weather facts, dated implementation checkpoints and evidence links. Its sequencing, gates and status columns are no longer authoritative.

# Retail weather parity plan

Requested 2026-09-14. This is a code and existing-evidence review plus an
implementation plan. The initial planning pass established no new native behavior
or runtime parity.
Follow-up: [clock/turbulence research](../formats/weather.md) now establishes
continuous time and distinguishes physical turbulence from maneuver sound.
Contrails and vapor trails are user-confirmed scope, including broader
wing-induced vapor beyond wingtips; their native contracts remain to recover.
Target the supplied Fighters Anthology build first, then validate every supported
base theater. Follow the roadmap's behavioral/asset fidelity definition; original
resolution and pixel-identical rasterization are not acceptance requirements.

## Scheduling update — 2026-09-15

The next continuation is now governed by the
[living native environment/systems plan](../research/native-environment-systems-plan.md).
Its NE-01/02 sea/ocean asset discovery and NE-09 native environmental coupling
advance the dependencies needed by restricted native flight; this supersedes
the earlier blanket deferral of all weather work until after new aircraft.
This turn is planning only, with no AI scope. Remaining weather work outside
those recorded dependencies follows native environment/systems, maneuver
feedback/acceptance and the scheduled aircraft-import pass. Existing weather
behavior remains unchanged; the detailed weather gates below stay open.

## Current implementation

Updated 2026-09-15 after the continuation through steps 1–3. The ordinary
flight weather path now includes recovered horizon branches, glare, aircraft
lighting, HUD palette handling and cloud preferences/visibility. Matched retail
acceptance remains open; retail now runs through dgVoodoo, and host captures
do not establish retail acceptance. [Current evidence](../baselines/weather-foundation.md#final-weather-sampling-and-batch-checkpoint--2026-09-15),
[earlier review](../baselines/weather-review.md).

## Numbered dependency sequence — 2026-09-15

This is the internal weather dependency order; the scheduling update above
governs when its remaining work runs.
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

### Step 1 implementation checkpoint

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
- [x] Original per-normal shade/highlight maps for solid and textured aircraft,
  source day/night light selection and light-before-fog ordering.
- [x] Original HUD primary index, shared cockpit palette and source brightness
  ordering; versioned migration preserves existing preferences.
- [x] Trace single-index weather sampling and remove added bilinear color mixing
  from sky/ocean, moon and cloud textures; preserve source cutout indices.
- [x] Audit special display maps: ordinary map is identity in all 24 LAYs;
  INFO2 solid override and alternate CP view maps require their display consumers.
- [x] Synthetic boundary/state tests, imported diagnostics, Linux creator/viewer,
  both aircraft flight checks and fog/transition host captures.
- [ ] Matched retail comparison of coverage, transitions and visible aliasing;
  Windows/macOS runtime acceptance. Pixel identity is not a roadmap requirement.

Status: callbacks, live six-bit tint/smoothing, ordered target/view haze maps,
world-anchored sky/ocean planes, Gouraud and textured horizon transitions,
above-sky underside/solid clipping, original aircraft lighting and primary HUD
palette handling are implemented for ordinary full-detail views. Indexed point
sampling preserves original weather texels; sky projection uses GPU rays instead
of the native intermediate raster. The 60 ms palette cadence, separate RNG
streams, float orientation and GPU rasterization remain explicit adaptations.
[Evidence](../baselines/weather-foundation.md).

## Existing implementation coverage

| Area | Implemented | Gap |
| --- | --- | --- |
| Mission inputs | `layer` name and choice index, `clouds` altitude, `wind` degrees and feet per second, and time all recovered and consumed | Campaign mission sources; generated and explicit cloud altitudes now draw; campaign sources remain open |
| Weather records | Reviewed fields of the 352-byte record in `tore-formats::weather`, all 24 supplied modules parsing | Vapor fill-pattern tables and native comparison |
| Environment state | `tore-sim::environment` advances a 120 Hz to 256-unit clock, selects and blends records by time and altitude, and answers pure queries | Pause, compression and long-session audit against the original |
| Terrain and sky | Live indexed palette, original horizon bands/transitions, above-sky branches and world-anchored planes | Alternate display/terrain-detail consumers; matched visible coverage and aliasing |
| Aircraft/cockpit/HUD | Source light-before-fog remaps, shape fog flags, private cockpit palette and HUD index/brightness | Other display effects and authored HUD/instrument geometry |
| Celestial | Original sun, moon and 94 stars, light/visibility selection, glare and world-fixed moon basis | Matched placement, size and horizon visibility |
| Clouds | Original CLOUD1 sheets, generated/explicit altitude, detail counts, sectors, range gates and point-sampled cutouts | Unestablished CLOUDS.SH producer; matched coverage and clipping |
| Visibility | Recovered ramps, altitude haze and ordered cross-layer indexed remaps | Retail crossing comparison; sensor consumers |
| Creator | Six source weather choices launch; duplicate overcast removed | Label mapping remains inferred; serialized weather replay |
| Wind | Resolved to world feet per second and applied by both adapters | Native default draw order implemented; wind audio and shared native RNG scheduling open |
| Air data | `telemetry::EnvironmentReading` accepts wind and atmosphere | Live terrain/standard-atmosphere producer; native atmosphere/instrument calibration open |
| Turbulence | Reviewed event generator, with corrected duration/priority, driving authored coupling | Nearby-aircraft wake strength; the daytime ground-query flag |
| Wing vapor | Shape-supplied attachment, position history, trigger, roll shortening and night suppression | Patterned fills, roll gate, scale and integer sample rounding remain open |

Source foundations: [theater format](../formats/theater.md),
[creator contract](../formats/quick-mission.md),
[native flight research](../formats/native-flight.md). SUN.SH has no identified
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
calling the port exact. See [review findings](../baselines/weather-review.md).

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

### W5 integration checkpoint — 2026-09-15

Typed generated/explicit wind now reaches both adapters and live AirData.
Synthetic checks cover calm/cardinal drift, preserved starting TAS and complete
attitude loops. The daytime flat-terrain class gate is traced; wake strength
has an explicit diagnostic input, with no fabricated flying neighbors.
The physical consumer uses body-axis rotations and supports the session
No turbulence cheat. F18/Rafale CE attachment axes are corrected and drawn
trail heads follow interpolated aircraft poses.

The gate remains partial: native whole-tick angular coupling, exact wake
geometry, object/carrier surface producers, audio dispatch, serialized replay
and retail response comparisons are open.
[Validation and scope](../baselines/wind-turbulence-vapor.md).

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
  [existing bounded diagnostics](../baselines/flight-performance.md), with matched
  clear and dense-weather runs. No live blocking readback or post-render sleeps.
- Formatting, warnings-denied Clippy, locked tests/build, Python checks and asset
  guards per [development instructions](../DEVELOPMENT.md); real Linux, macOS and
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

Before whole-system acceptance, close per-camera palette/visibility behavior
and matched retail comparisons; callback/tint and scattered-cloud defaults now work.
Wind defaults, turbulence coupling and serialized replay have their own later
delivery checkpoints; all remain required for whole-system acceptance.
Corrected helper tests and source-art screenshots alone do not close these gates.

### Step 2 implementation checkpoint

Original sun circles/glow remap, moon texture and 94 stars now render with source
time/flag/angle selection. Weather SH decoding is bounded and independent of
aircraft animation. The continuation implements sun-view whitening, nine original
lens-flare circles/remaps and the moon bank fix. Gouraud, solid and textured
deck horizon visibility are represented. All 24 reviewed LAYs use the same
celestial angles/rise/set values; the traced placement consumes LAY/time rather
than theater latitude or calendar date. Matched placement, scale, visibility
and transition acceptance remain open. Native integer projection is an explicit
adaptation, not a requirement for pixel identity.

### Step 3 implementation checkpoint

Original CLOUD1 top/bottom geometry, cutout texture, runtime-imported nine-entry
layout, high-detail 4×4 repeats, generated altitude defaults and explicit mission
altitude now render. Crossings and a second theater were captured. The sixteen
CLOUDS billboards are decoded but have no established active producer; native
low-detail, forward-sector relocation and SH coordinate-range rejection now
work. GPU triangle clipping replaces native sphere/frustum work rejection;
edge coverage still needs matched retail comparison. The static
resource/producer audit still establishes no active CLOUDS.SH placement. Cloud
bands are source fog records, not evidence for an authored volumetric deck.


### Remaining dependencies after this batch

| Item | What remains and how to unblock it |
| --- | --- |
| Retail behavior/visual acceptance for 1–3 | Run matched retail scenes once the Windows box is ready: Ukraine dawn/dusk boundaries; 4,500/5,000 and 9,000/9,500-foot fog overlaps; below/on/above a cloud patch; sun toward/away and moon bank; above-sky/horizon/zenith; repeat a distinct LAY family. Record build, mission, time, pose and detail/glare settings. Compare art, coverage, scale and transitions, not identical pixels. |
| CLOUDS.SH's 16 billboards | No active placement established after the cloud queue/frustum trace, nine-descriptor audit, all LAY shape fields and 1,654 FA_2 resource audit. A retail mission/view showing these billboards, or a resource-load trace identifying the caller, is the missing discriminator. Do not spawn them speculatively or call them absent. |
| Alternate display consumers | INFO2 sets a temporary solid horizon override; CP view paths select alternate color maps. Integrate with their actual display/camera paths in step 4 or the separately scheduled INFO2 screen, along with low-detail terrain/sky preference and ground-surface inputs. The cloud-only detail diagnostic does not select those terrain modes. |
| Other platforms | Windows and macOS builds/runtime/captures remain unexercised here. Linux checks cannot accept those rows. |

The implementation supported by the recovered ordinary-view contracts in this
batch has landed. The open rows above prevent whole-batch retail acceptance;
steps 4–9 retain their existing scope. The later step-4 checkpoint below
supersedes this batch's camera implementation status.

## Requested smoothing and size follow-up — 2026-09-15

- [x] Smooth source palette colors through fractional mission time and altitude;
  retain pure queries, fixed ticks and original callback scheduling.
- [x] Smooth spatial horizon/deck shades and ordered fog remap colors; retain
  original point-sampled artwork and a stepped diagnostic mode.
- [x] Share continuous tint/whitening with cockpit/HUD palette effects.
- [x] Correct sun/moon scale against user-confirmed default-zoom retail captures;
  preserve proportional viewport sizing, zoom and bank-independent lunar axes.
- [ ] Confirm fitted celestial projection against an exactly matched retail pose
  and complete the existing cloud/fog/theater/platform acceptance scenarios.

[Follow-up evidence and fitted/native boundaries](../baselines/weather-smoothing.md).
The user now has retail running through dgVoodoo: the earlier Windows-box-ready
prerequisite is superseded. Windows/macOS rebuild checks remain separate.

## Step 4 camera implementation checkpoint — 2026-09-15

- [x] Connect the original **No sun whiteout?** cheat to immediate suppression
  of whitening and lens flare across all views, retaining original sun geometry.
- [x] Resolve each drawn camera's palette, fog ramp, shade rows and sky/ocean
  decks at its own altitude from one authoritative environment instant.
- [x] Give main, rear mirror, forward panel and other panel independent fixed-tick
  tint/glare smoothing and seeded presentation RNG; update hidden slots too.
  Shared panel pose construction keeps simulation and rendering consistent.
- [x] Resolve the existing fitted vapor color through each camera's palette;
  camera queries do not change trail histories, weather callbacks or RNG.
- [x] Synthetic altitude/query/glare/pause checks, Linux creator/viewer and both
  aircraft GPU captures, wide/tall composition and bounded frame-time evidence.
- [ ] Matched retail camera comparisons, alternate CP display-map branch
  identification/integration, and Windows/macOS runtime acceptance.

The independent camera presentation streams are an authored host integration,
not recovered native multi-camera palette-thread scheduling. Instrument imagery
retains the existing asynchronous 10 Hz update and may lag the current main
image; every submitted scene uses one coherent weather instant. Unimplemented
target/missile cameras and special sensor/INFO2 displays are not newly enabled.
[Implementation and acceptance evidence](../baselines/weather-cameras.md).
