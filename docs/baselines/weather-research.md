# Weather clock and turbulence static evidence — 2026-09-14

Read-only native-code investigation on Linux. Source identity:

- FA.EXE SHA-256: `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`
- FA.SMS SHA-256: `e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0`

Both hashes matched before fixed-address interpretation. No retail code ran.
A fresh pass wrote 62 symbol spans, 3,829 symbols and 11 reviewed regions with
hashes/direct edges. Full disassembly/manifests stay ignored under
`.local/weather-clock-turbulence/`.

## Reproduce

```sh
python3 tools/extract_native_flight.py --domain weather --source gameassets/fighters-anthology --out .local/weather-clock-turbulence
```

This standalone research entry point shares bounds, hash gating, output preflight
and conflict checks with flight/menu/weapons research. Unknown builds receive
symbol inventory without fixed-address reviewed regions. Runtime extraction
profiles and app behavior are unchanged.

## Conclusions and limits

Continuous simulation time and downstream weather selection are confirmed.
Physical low-altitude and nearby-aircraft turbulence exists, with daylight
scaling and per-aircraft randomized event state. A separate sound intensity
function responds to maneuver state. See the [source specification](../formats/weather.md).

Follow-up passes recovered the reviewed LAY record fields, the blend kernel, the
visibility ramp, the wind line, the turbulence event generator and the wing
vapor streamer subsystem, and found no confirmed dedicated engine contrail producer in the inspected
paths. The earlier claim of exhaustive absence is withdrawn; the subsequent
[aircraft-shape and commit review](weather-review.md) inspects the embedded code
and records the exact scope and implementation corrections. Those contracts and their limits are
in the [source specification](../formats/weather.md), and the implementation
evidence is in [weather implementation evidence](weather.md). No complete
physical buffet model, retail trajectory or visual comparison is claimed.

## Validation

Fresh reviewed-build extraction and identical-output rerun passed. A synthetic
regression checks that unknown builds cannot receive fixed-address weather
artifacts. Formatting, warnings-denied Clippy, locked workspace tests/build,
21 Python tests and source/app/extractor asset guards passed. No rendering
changed; GPU smoke tests, retail comparison and Windows/macOS execution were
not performed.
