# Weather in flight cameras — 2026-09-15

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


## Behavior

The imported FMENUD label **No sun whiteout?** now works under Escape → Cheat.
On suppresses lens-flare circles and palette whitening in the main world,
cockpit/HUD, mirrors and camera panels, including while paused. Off restores
them. The original sun geometry/glow remains. The cheat is session-only and
survives mission restarts; it does not change saved preferences.
`TORE_SUN_GLARE=0` remains a diagnostic override that also suppresses the effects.
The label is recovered; this wiring is the requested host behavior.

Each scene draw resolves its own altitude-dependent palette, shade rows, fog
ramp and sky/ocean decks. Lens-flare palette lookup uses that same view palette.
Main, rear mirror, Forward View and Other View have separate presentation state
for tint, overlap reduction and sun whitening. All four advance on fixed 120 Hz
simulation ticks, including hidden feeds; draw/query count cannot advance them.
They share the authoritative environment clock and callbacks. Seeds are fixed
independently per slot and restart resets all slots. The slot allocation and
scheduling are authored host integration, not native palette-thread parity.

Instrument camera construction is shared between ticking and rendering. Main
weather now uses the camera's actual altitude, including exterior offsets.
Existing vapor positions/history stay shared; their fitted palette-254 color
now resolves on the GPU per camera, retaining the existing fade/haze behavior.
Native patterned vapor materials remain step-7 work.

The rear mirror stays GPU-only. Instrument cameras retain asynchronous roughly
10 Hz readbacks; a completed image may show an earlier instant than the main
view. Each submitted scene is coherent, but simultaneous displayed images are
not claimed to have zero latency. Explicit smoke/capture operations may block
for readback; live rendering does not add a blocking readback or sleep.

## Validation

Host: Linux x86_64, NVIDIA RTX 4070, Vulkan, Immediate presentation.
Ignored evidence, logs and capture command manifest: `.local/weather-cameras/`.

- Formatting, warnings-denied Clippy, locked workspace tests/build passed:
  290 Rust tests and 24 Python tests. Repository and both debug executable asset
  guards passed. Fixtures are synthetic; no imported bytes are committed.
- Synthetic checks cover different-altitude simultaneous samples, independent
  tint, repeated/reordered query purity, toward/away sun whitening, immediate
  paused cheat suppression and unchanged sun geometry. Existing clock,
  pause/release, weather callback and replay tests remain green.
- Creator and foggy-viewer GPU smoke tests passed. Four flight captures passed:
  F18 fog/cockpit and fog/chase at 1280×720; Rafale cloudy/cockpit at 720×1000;
  Rafale night/look-back at 1280×720. Forward and Other View feeds were exercised.
  Wide/tall captures were inspected for full-canvas cockpit, retained mirrors,
  instrument anchoring and dense-weather consistency.
- Both shared `tools/extract_assets.py --aircraft f18|rafale --validate-flight`
  suites passed, including full loops and wind/contact probes. Extraction used
  an ignored directory containing copies of just FA_1/FA_2: the full media-tree
  scan encountered an unrelated unsupported archive compression flag.
- A 400-tick pull capture exercised visible vapor at 5.60 G in the exterior
  and Other View feed; an additional look-up GPU smoke passed.
- `--validate-weather` passed all 24 imported LAY modules, full-day clock and
  source selection/palette checks. This is source/helper acceptance, not matched
  retail rendering acceptance.

Repeat the GPU camera captures using the ignored `run.py`/`manifest.json`, or:

```sh
target/debug/tore-app --free-flight --weather-condition 2 --instrument-page 3 --window-size 1280x720 --flight-probe-ticks 240 --capture-flight .local/weather-cameras/check.ppm --smoke-test --no-audio
TORE_PERF_FRAMES=330 TORE_PERF_ACTIVE=1 target/debug/tore-app --free-flight --weather-condition 0 --instrument-page 3 --window-size 1280x720 --no-audio
```

Use condition 2 for the paired fog run. Final samples, first 30 frames excluded:

| Condition | Mean interval | p95 | Mean simulation/cameras | Completed panel readbacks |
| --- | --- | --- | --- | --- |
| Clear | 1.79 ms | 2.73 ms | 0.19 ms | 7 |
| Fog | 1.63 ms | 2.08 ms | 0.21 ms | 6 |

Both runs report zero paused frames and 330 rear mirror renders. These are short
CPU wall-clock intervals including presentation backpressure, not GPU timing,
verified displayed FPS, sustained-load acceptance or a causal before/after
performance comparison.

## Remaining acceptance

Matched retail views, layer crossings and glare/altitude transitions remain
open, as do Windows/macOS runtime checks. All six conditions/all theaters are
not accepted by these representative captures. CPDraw's alternate display map
selection still needs its actual branch/view contract; it is not inferred from
the host page name “Other View”. INFO2 and unavailable target/missile/sensor
views remain separate work. No new contacts or sensor visibility rules were
invented. See [weather plan](../research/weather-plan.md) for later wind, turbulence,
vapor, replay and whole-system gates.
