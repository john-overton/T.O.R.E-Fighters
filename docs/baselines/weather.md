# Weather implementation evidence — 2026-09-14

What this covers: the recovered retail environment now runs in the engine. Time
of day, the day/night palette cycle, visibility and haze, mission wind, physical
turbulence and wing vapor trails are all driven by data read out of the supplied
media rather than by authored approximations. Source identity and the static
research method are unchanged from the
[clock and turbulence pass](weather-research.md); the field-level contracts are
in the [source specification](../formats/weather.md).

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
sunrise and sunset. Light rises from 0 to 165 and haze falls from 1,031 to 6,187
across 07:00 to 07:10, the horizon color ramps with them, dusk mirrors it, and
the night flag clears partway through. Twelve of the day's 1,440 minutes are
interpolated rather than stored. A full simulated day leaves no uncovered tick
and returns the clock to its launch time.

The six source weather choices band as expected: clear shows no haze at any
altitude, cloudy is clear below 5,000 feet, a complete whiteout inside the 4,500
to 9,500 foot deck and clear again above 9,000, fog is total below 8,000 and
clear above 7,500, dawn sits mid-transition, and night hazes almost completely.

Rendering takes the retail artwork as palette indices and resolves the live
palette on the GPU, which is what the source does — every pixel of `SKY0` and of
every terrain tile is a weather-palette index, and none carries a local
override. No colors are authored anywhere. At night the original city grid
lights up from the same indices.

Wing vapor appears past 4 G or below -2 G, reaches full length at 7 G, shortens
with roll rate and disappears at night. Both reviewed aircraft supply their own
attachment points from their shape data, 18 feet outboard, matching their half
spans. Turbulence appears below 1,000 feet above ground, is quartered outside
07:00 to 19:00, and drives the haptic event type that previously had no producer.

Loops still complete for both aircraft, and the stall and level probes are
unchanged.

## Measurements

Frame interval over 330 frames at 1280x720 with active simulation and camera
views: mean 1.46 ms, median 1.23, p95 1.51, max 11.38. Simulation and cameras
0.08 ms mean; submit and present 0.75 ms mean. The GPU palette lookup replaced a
hardware-filtered sample with four loads and a manual blend at no measurable
cost.

## Limits

No retail side-by-side comparison was made. Everything here is checked against
the source's own data and arithmetic, not against captured original frames, so
none of it closes a 1:1 parity gate on its own.

Still missing or approximate: celestial shapes, the cloud and ocean decks as
geometry, the ten-step quantization the engine applies to the visibility ramp,
the patterned fills that give wing vapor its real color, nearby-aircraft wake
turbulence, wind audio, and the air-data integration. Overcast has no recovered
source module. The creator-label to source-choice mapping is an inference.

Formatting, warnings-denied Clippy, locked workspace tests and build, 21 Python
tests and the asset guards passed. Viewer, creator and flight GPU smoke tests
passed on Linux with Vulkan on an RTX 4070. Windows and macOS were not
exercised.
