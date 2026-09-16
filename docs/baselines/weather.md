# Weather implementation evidence, 2026-09-14

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Measured evidence, research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature; see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


What this covers: the recovered retail environment now runs in the engine. Time
of day, the day/night palette cycle, visibility and haze, mission wind, physical
turbulence and wing vapor trails are all driven by data read out of the supplied
media, with authored host-clock, rendering and flight-coupling approximations. Source identity and the static
research method are unchanged from the
[clock and turbulence pass](weather-research.md); the field-level contracts are
in the [source specification](../formats/weather.md).

The [2026-09-15 full review](weather-review.md) corrects implementation defects
and supersedes stronger fidelity/absence claims from this initial pass.

## Reproduce

```sh
cargo run --locked -p tore-app -- --validate-weather
cargo run --locked -p tore-app -- --weather-condition 1 --capture-terrain .local/weather/cloudy.ppm
TORE_WEATHER_TIME=19:06 cargo run --locked -p tore-app -- --capture-terrain .local/weather/dusk.ppm
TORE_VAPOR_PROBE=1 cargo run --locked -p tore-app -- --free-flight --maneuver pull --flight-probe-ticks 400 --smoke-test
```

`--validate-weather` parses every imported `.LAY`, expands every record's
palette, runs one full simulated day against the mission's own module, probes
all 1,440 minutes for coverage and transitions, and checks that each of the six
source weather choices resolves a module and bands correctly with altitude.
`TORE_WEATHER_TIME=HH:MM` overrides the launch time for matched captures.

## What passed

All 24 supplied `.LAY` modules parse under the recovered 352-byte layout. The
day modules band by time into night, a ten-minute dawn, day, an eight-minute
dusk and night again; the cloud and fog modules band by altitude with deliberate
overlaps. Exactly the two night records carry the hazing flag.

Resolved through the translated blend kernel, `DAY2.LAY` produces a real
sunrise and sunset. The near haze distance rises from 0 to 165 record units and maximum see distance
rises from 1,031 to 6,187
across 07:00 to 07:10, the horizon color ramps with them, dusk mirrors it, and
the night flag clears partway through. Twelve of the day's 1,440 minutes are
interpolated rather than stored. A full simulated day leaves no uncovered tick
and returns the clock to its launch time.

The source cloud/fog modules select and blend altitude bands. Cloud transitions
occupy 4,500–5,000 and 9,000–9,500 feet; fog transitions occupy 7,500–8,000 feet.
Haze is distance-dependent, including in DAY2 clear weather. These probes do not
establish complete sky whiteout or rendered cloud geometry.

Rendering retains terrain and SKY0 source palette indices and resolves the live
palette on the GPU. That recovers changing source colors; it does not establish
native sky projection, rasterization, shading or the absence of authored effects.

Wing vapor appears past 4 G or below -2 G, reaches full length at 7 G, shortens
with roll rate and disappears at night. Both reviewed aircraft supply their own
attachment points from their shape data, about 18 feet outboard under the
app’s provisional one-third-foot scale. Turbulence appears below 1,000 feet above ground, is quartered outside
07:00 to 19:00, and drives the haptic event type that previously had no producer.

Loops still complete for both aircraft, and the stall and level probes are
unchanged.

## Measurements

Frame interval over 330 frames at 1280x720 with active simulation and camera
views: mean 1.46 ms, median 1.23, p95 1.51, max 11.38. Simulation and cameras
0.08 ms mean; submit and present 0.75 ms mean. This historical short sample does not establish unchanged cost; a matched
before/after run is required for a performance comparison.

## Limits

No retail side-by-side comparison was made. Everything here is checked against
the source's own data and arithmetic, not against captured original frames, so
none of it closes a 1:1 parity gate on its own.

Still missing or approximate: celestial shapes, the cloud and ocean decks as
geometry, the ten-step quantization the engine applies to the visibility ramp,
the patterned fills that give wing vapor its real color, nearby-aircraft wake
turbulence, wind audio, and the air-data integration. Overcast has been removed from the editor as a duplicate of cloudy. The creator-label to source-choice mapping is an inference.

Formatting, warnings-denied Clippy, locked workspace tests and build, 21 Python
tests and the asset guards passed. Viewer, creator and flight GPU smoke tests
passed on Linux with Vulkan on an RTX 4070. Windows and macOS were not
exercised.
