# Weather implementation and aircraft effects review — 2026-09-15

The working tree was clean at the start. Eleven local commits were ahead of the
local upstream tracking reference, from `0e81b38` through `21b5823`. This review
does not fetch, rewrite, commit or push them. It reviews their code and claims;
new fixes remain uncommitted. Retail assets and generated disassembly stay local.

## Commit audit

| Commit | Area | Finding |
| --- | --- | --- |
| `0e81b38` | Clock/turbulence research | Continuous time and physical turbulence confirmed; helper evidence is not whole-flight acceptance |
| `a582885` | LAY reader and environment | Useful bounded records and deterministic host clock; callback/tint, native defaults and complete record semantics remain open |
| `cc76771` | Time/altitude blending | Recovered blend and flag rules; extreme scalar arithmetic needed widening |
| `dcfe7d5` | Contrail/vapor scope | Absence claim overstated; this review adds static aircraft-embedded code inspection |
| `92c21aa` | Live GPU palette | Source indices retained; fixed 256×256 sampling incorrectly addressed taller aircraft atlases |
| `f4bd19e` | Haze | Distance scale and ramps have source evidence; smooth RGB fog, sky projection and per-camera handling remain approximations |
| `9703732` | Wingtip vapor | Source attachments and trigger confirmed; sample commit order, reset placement and night probe needed fixes; exact geometry/timing claim withdrawn |
| `2a55ed7` | Wind/turbulence coupling | Duration used remainder instead of quotient, active event priority differed, AGL rounding differed, coefficient bypassed typed configuration |
| `c5f07b3` | Creator weather | Removed duplicate overcast per user request; preserve six-row mapping and resolved launch identity |
| `b1c9543` | Evidence/plan | Corrected “none authored,” whole-game absence and cloud-band/clear-weather overstatements |
| `21b5823` | Help | Six native CLI choice indices remain unchanged; editor indices are a separate mapping |

## Confirmed defects corrected

- **Aircraft atlas sampling:** shared WGSL now uses actual texture dimensions.
  F18's 256×644 and Rafale's 256×457 atlases previously sampled only a 256×256
  region. Terrain/sky remain 256×256. This changes the incorrectly addressed
  aircraft texture, not source artwork.
- **Turbulence duration:** native `0x477a9e` uses AX after division by 100,
  so length is `Rand(7680) / 100 + 89`, or **89–165 clock units**. The previous
  modulo version produced 89–188 with a different distribution.
- **Event priority:** native `0x4775b5` applies an active event and returns before
  considering recurrence. New events also return without contributing on their
  creation call. Both behaviors now match those branches.
- **AGL rounding:** native divides the negative height term before adding 100.
  At 999 ft it retains strength 1; the old implementation incorrectly returned 0.
- **Aircraft coefficient:** required `turbulencePercent` now belongs to each
  model's typed, validated configuration. No silent zero for a missing field and
  no stale app-level value after aircraft changes. Ground/crash suppression uses
  flight/contact state instead of assuming a one-foot AGL threshold.
- **Sample history:** native `0x412585..0x4125aa` writes the new head before the
  overlapping move. Entries 0 and 1 coincide on each 25-unit commit; the prior
  Rust order retained the previous tick and shortened subsequent intervals.
- **Restart/probe integration:** restart resets weather clock and turbulence/RNG,
  then seeds vapor after mission altitude/fuel and combat reset. Removed duplicate
  reseeding; aircraft switching also clears old weather-effect state. Flight
  probes now run turbulence as live flight does, retain its resulting state and
  apply the actual night-hazing gate. Vapor samples follow turbulence movement.
  Serialized environment replay remains open.
- **Creator:** imported source labels include punctuation (`overcast.`). The
  editor removes that row, keeps original text for the others and maps dawn,
  clear, cloudy, foggy, sunset, night to native choices `[3,0,1,2,4,5]`. The raw
  source option inventory is unchanged. Cloudy selects `CLOUD1`.
- **Diagnostics:** selected condition/time/layer parameter are retained in the
  world's resolved mission metadata. Cloud/fog validation no longer tests DAY2
  accidentally or demands time transitions from altitude-only modules.
- **Bounds:** widened scalar blend/haze/visibility arithmetic, validated shade
  and tint components, and rejected the unrepresentable mirrored streamer
  coordinate instead of allowing debug overflow.

## Static aircraft-file investigation

The executable/symbol hashes still match [the reviewed build](weather-research.md).
Extracted shape hashes:

| Resource | SHA-256 |
| --- | --- |
| `F18.SH` | `d6c876d63d10a05072c8afd8a53cedffdd9cdfbcff4c4576a90c1b6c064b8bb9` |
| `RAF.SH` | `7da4f1d7e2a296022634567b37b0aebd697542a526ad0fb0e25fbe649b19e081` |

The new standard-library inspector reads PL/PE sections and imports, associates
local aliases with imported names and locates bounded native re-entry candidates.
Optional GNU objdump disassembly reads inert bytes; no imported code executes.
Reproduce after extracting the aircraft profiles with the shared extractor:

```sh
python3 tools/extract_assets.py --help
python3 tools/inspect_shape_effects.py PATH/FA_2.LIB/F18.SH --disassembly .local/F18-native-review.txt > .local/F18-native-review.json
python3 tools/inspect_shape_effects.py PATH/FA_2.LIB/RAF.SH --disassembly .local/RAF-native-review.txt > .local/RAF-native-review.json
```

Disassembly outputs require new filenames. JSON goes to stdout. Current local
outputs are under `.local/weather-shape-review/`. The tool marks byte-pattern
candidates explicitly; it is not a complete SH control-flow or absence prover.

F18 has 36 recognized re-entry candidates; Rafale has 44 and one unmatched raw
byte match. The inspected native blocks use imported aircraft device state and
return to `do_start_interp`. Afterburner guards and two-sided streamer draws are
identified in both shapes, including multiple detail paths. No additional
contrail or broad wing-vapor trigger was found in these inspected blocks.
[Offsets and imports](../formats/weather.md#aircraft-embedded-code-inspected-statically--2026-09-15).
Other aircraft and all indirect runtime paths remain outside this bounded result.

**Conclusion:** wingtip vapor is confirmed. Dedicated engine contrails and broader
wing vapor remain unconfirmed, not proven absent game-wide. The “never execute”
rule permits this static inspection and later bounded translation. Future optional
engine contrails are now explicitly scheduled in [W7](../weather-plan.md#w7--optional-engine-contrails-after-retail-weather).

## Remaining findings and acceptance limits

- The day/night **palette** cycle exists. Source indices and LAY colors do not
  make the entire renderer native: SKY0's hemisphere projection, bilinear color
  filtering, continuous RGB haze and vapor alpha are authored approximations.
  Aircraft use their own atlas palette; they do not receive the live weather
  palette merely because their textures now use index lookup.
- Clear DAY2 weather still hazes at distance. Cloud overlap bands are transitions:
  4,500–5,000 and 9,000–9,500 ft are blends, not simultaneously fully clear and
  fully opaque. “Complete whiteout inside the deck” is too broad: density depends
  on range, and the independently drawn sky/cloud geometry remains incomplete.
- Missing mission wind currently resolves to calm, despite the recovered native
  randomized default. Scattered-cloud probability/altitude selection is not
  implemented, and the `clouds` deck is not drawn. Existing MM files alone do
  not exercise a live nonzero-wind mission in the app.
- Per-record callbacks are ignored by the runtime; fog jitter and global
  `tint`/`tint_scalar` are not applied. Finding their imported native handlers is
  not implementing them. Header shade/fill tables remain incomplete.
- Turbulence still uses floating-point sine/rates and authored Euler/height
  coupling, not retail integer display/movement arithmetic or a complete force
  model. Nearby-aircraft strength, the ground-query flag and native preference
  mapping remain unresolved. Haptic intensity/presentation is authored.
- Vapor still uses float load factor, float position interpolation, rounded
  lookback samples, provisional shape scale and unconditional roll shortening
  where retail tests an unresolved object flag. Its night gate uses ownship
  altitude; retail's current view layer and per-camera behavior need acceptance.
- Rendering shares one resolved palette/haze between camera views; cameras at
  different altitudes need their own pure queries. Fog maximum-see-distance
  clipping, sensor visibility, wind audio, atmosphere/air-data integration,
  sun/moon/stars and cloud/ocean geometry remain open.
- Restart state cleanup does not implement serialized weather replay or reproduce
  retail shared RNG/cadence. The 120 Hz-to-256-unit adapter is still authored.
- No matched retail execution/captures, Windows or macOS validation in this review.
  Historical 1.46/1.48 ms summaries are not a new matched performance result.

## Validation

- Formatting, warnings-denied Clippy, locked workspace build and **255 Rust
  tests** passed; **24 Python tests** and source/debug-app/debug-extractor/release-
  extractor asset guards passed. Synthetic regressions cover source label
  punctuation/mapping, event duration/priority, AGL rounding, commit history,
  required configuration, extreme scalar arithmetic and static import bounds.
- `--validate-weather` passed for clear, cloudy and foggy Ukraine launches. Each
  run parsed all 24 imported modules, exercised one full selected-module day,
  and probed six source choices. This does not validate every theater visually.
- The documented shared flight suites passed for F18 and Rafale: **13 scenarios
  each**, including complete loops, wind, ground contact and adverse landings.
  Reproduction used:

  ```sh
  python3 tools/extract_assets.py --aircraft f18 --exclude-archive 'disc1/LHX/*' --out .local/weather-shape-review/f18-profile --validate-flight
  python3 tools/extract_assets.py --aircraft rafale --exclude-archive 'disc1/LHX/*' --out .local/weather-shape-review/rafale-profile --validate-flight
  ```

  Initial invocations incorrectly passed `--validate-flight` to the app and
  omitted the documented LHX archive exclusion; those attempts failed and do
  not count as passes. The corrected wrapper runs above completed successfully.
  These suites validate aircraft dynamics separately; they are not a retail
  oracle or a full low-altitude weather-coupling acceptance suite.
- Imported `--validate-creator` passed for both aircraft. The condition-selector
  capture was visually checked: six rows, no overcast, night retained. The first
  capture exposed the punctuated source label and prompted the filter correction.
- Linux/Vulkan RTX 4070 creator/viewer/flight GPU checks passed. Inspected F18 and
  Rafale exterior captures, a 1280×720 F18 cockpit and a 720×960 Rafale cockpit.
  The daytime pull probe produced visible wingtip trails at 5.60 G; the same
  nighttime probe returned no trail for either side. Original PPM captures and
  PNG viewing copies remain ignored under `.local/weather-shape-review/`.
- Two short 330-frame samples (30 warmup frames excluded): cycling views measured
  **1.47 ms mean / 1.63 ms p95** CPU frame interval, zero paused frames and 150
  mirror renders. The camera-panel case measured **1.54 ms mean / 1.65 ms p95**,
  zero paused frames, six completed camera readbacks and 330 mirror renders.
  Commands use the existing `TORE_PERF_FRAMES`, `TORE_PERF_ACTIVE` and
  `TORE_PERF_VIEWS` controls from [performance evidence](flight-performance.md).
  These include presentation backpressure, are not GPU timing/displayed FPS,
  and do not establish unchanged performance without a matched old-build run.

No retail side-by-side, Windows/macOS execution or full weather replay validation
was performed. All edits remain uncommitted; no local commits were rewritten or
pushed.
