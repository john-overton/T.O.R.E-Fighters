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

| Area | Implemented | Gap |
| --- | --- | --- |
| Mission inputs | `tore-formats/src/theater.rs` reads layer, layer parameter, clouds, raw wind and time | Runtime loads the base MM; weather selection, defaults and units are unresolved |
| Weather records | Bounded PL/CODE palette reader, explicit keyframe | Complete record fields, selection, interpolation, altitude and update timing |
| Terrain and sky | `tore-app/src/terrain.rs` selects keyframe 2 and SKY0 | Palette is baked into RGBA textures and vertex colors at world construction |
| Fog | `sim_renderer.rs` supplies palette entry 235 and density 0.000004; `terrain.wgsl` applies exponential distance fog | Native visibility, horizon and fog evolution |
| Celestial/cloud assets | Shared importer selects SKY0–8, SUN/MOON/STARS, CLOUD1/CLOUDS and named textures | Dedicated shape interpretation, placement, animation and compositing |
| Creator | Recovered seven condition labels; `quick_mission.rs` rejects launch unless condition index is 1 | Native condition-to-environment mapping and accepted launch/restart state |
| Wind | Diagnostic native position helper and hybrid `tore-sim::flight::State::step_surface` support explicit wind | Live app passes height only; legacy adapter ignores supplied wind |
| Air data | `telemetry::EnvironmentReading` accepts wind and atmosphere | Shared live weather integration; standard atmosphere remains an authored approximation |
| Turbulence | Native force/display helpers accept turbulence; haptic event type exists | Generator, scheduling, weather coupling and live producer |

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
- Creator condition field 15: trace all seven values through mission generation
  to LAY, parameter, cloud, clock and wind values. Index 6's night flag alone
  does not recover the night environment.
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

Vapor scope is now settled. Wingtip vapor trails are a recovered streamer
subsystem with a complete trigger, attachment, sampling and fade contract.
Engine contrails and broader wing-induced vapor are verified-absent from the
reviewed executable; the afterburner plume is data-driven inside aircraft shapes
whose embedded code we never execute. See [the source specification](formats/weather.md).

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

**Gate:** clear/cloudy/overcast/foggy comparisons below, within and above cloud
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

- All seven retail conditions across all 16 base theater resource profiles;
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

## Recommended execution order

W1 → W2 → W3 → W4 → W5 → W6. Collect retail references early and validate each
slice as it lands. Wind/turbulence research belongs in W1 even though live coupling
comes later. The first implementation deliverable is the recovered condition/LAY
contract and a deterministic palette/clock probe, followed by accepted day/night
rendering. No reliable implementation estimate is available until W1 bounds the
remaining native contracts.
