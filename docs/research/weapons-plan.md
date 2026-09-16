> **Frozen as of 2026-09-15. Superseded by the parity strategy (D30) — see [AGENTS.md](../../AGENTS.md) and [the parity plan](../parity-plan.md).** Split out of [weapons research](../formats/weapons.md) on 2026-09-16, which keeps the recovered facts. Its sequencing, W0–W5 gates and status columns are no longer authoritative.

# Aircraft weapons: implementation plan and status log

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Implementation status — 2026-09-14

The plan was committed and pushed as `457c85b` before implementation. W0/W1
have substantial completed substeps; W2 has diagnostic arithmetic components.
W0–W5 acceptance gates remain open. See [implementation evidence](../baselines/combat-components.md).

- The shared resolver now selects all JT/SEE/ECM/GAS definitions with
  `--weapons`, plus 19 reviewed shared graphics/audio roots. Aircraft profiles
  also retain their PTS modules. CLI and app use the same closure; the wrapper
  accepts repeated aircraft flags. The app cache imports both aircraft and the
  armament catalog, with a version marker to reject older incomplete caches.
- Reports now expose dependency edges, unresolved native symbols/module/art
  candidates, archive providers and the archive chosen for dependency reads.
  `filtered` and edge `included` distinguish successful writes from a complete
  selected closure. `native_parity` remains false.
- Typed JT/SEE/ECM/GAS readers preserve source configuration without missing
  numeric fields defaulting to zero. All 220 equipment definitions parse on the
  supplied FA installation. The source-labeled rear-facing SU35R.SEE retains
  negative zone headings; its native coordinate/configuration handling is open.
- PTS must **not** be treated as a decoded loadout/preset format. The reviewed
  F18/RAFALE PTS resources are compiled PL modules containing icon strings and
  small data arrays. ICONF18.PIC and ICONRAF.PIC are absent from this catalog.
  Preserve the modules and unresolved edges rather than substituting artwork.
- `--native-weapons` reuses bounded PE/SMS readers and disassembly, with both
  reviewed hashes required for fixed-address artifacts. It emits 119 selected
  symbol spans, 19 reviewed regions and packed JT field-reference evidence.
  Extracted spans include untranslated branches; they are not runnable engines.
- `tore-sim::combat` implements launch-speed selection, motor phases, lifetime,
  quantized altitude performance, axial speed approach, positive-distance
  position arithmetic using imported native trigonometry, gravity fall, trigger
  deadline/rising-edge decisions and ammunition debit. `combat::loading` has
  store-weight and station capacity/mask decisions. Sensors have radar-emission
  deadline and partial range/FOV gates with explicit caller inputs.

These are isolated translations, not a combined native update. Native currentT
is assigned from currentTicks shifted right six at 0x486bd7–0x486be6. With the
reviewed 256-unit tick domain this gives quarter-second units; scheduler/service
ordering and host-clock equivalence remain open. fuelT is a launch-relative
cutoff in PROJEngineState, not an additional duration after ignition. HARDUnload
accepts a partial final debit and preserves the high count flag. Player repeat
uses flag 0x800 and an unsigned deadline; bay/target/fire rejection occurs later.
None of these facts alone establishes end-to-end gun cadence.

The `weapon_probe` example exercises scalar components for imported JT files;
all 135 definitions pass 540 launch-condition rows and lifetime/ignition checks.
No live firing, target detection, guidance, collision/damage, release/mass updates
or combat rendering is enabled. Source mounts, full dispatch/order, native target
producers, effect tables and original-game engagement observations are still
required before those hooks can meet the vanilla fidelity gate.

## Ordered implementation plan

### W0 — Complete the native contract and catalog

- Inventory every JT and PT/PTS reference, with SEE/ECM/GAS, shapes, textures,
  palettes, audio, effects and native callbacks. Separate supported aircraft
  from aircraft merely inventoried; keep F18C and other Rafales distinct.
- Trace compatibility, station coordinates/scale, pairing, racks/pod capacity,
  load/unload/jettison, availability and loaded mass/drag rules. Default stores
  alone do not establish all legal stores or counts.
- Add repeatable static weapon research alongside native-flight mode: both
  EXE/SMS hashes, bounded spans/tables, incoming/outgoing references, indirect
  dispatch obligations, field offsets, source hashes and open contracts. Share
  PE/SMS readers; ordinary archive extraction stays independent of disassembly.
- Trace every distinct projectile flag/guidance branch. Record units, signedness,
  saturation, RNG draws, difficulty/player/AI modifiers and event/clock ordering.
  Field names such as trackMaxG are not sufficient evidence of physical units.

Gate: every definition has a reviewed family or explicit unsupported reason;
every gameplay field has a consumer/status entry. Never replace unknown behavior
with modern specifications.

### W1 — Complete extraction and provenance

- Extend the shared Rust resolver with reviewed combat-effect roots and typed
  dependency edges; follow aliases/generated-name tables, stores/pod geometry,
  textures, palettes and audio. Preserve original modules as inert resources.
- Report root → field/table → missing file, optional/unknown edges, filtered
  closure and duplicate provenance. Do not report success as complete coverage
  merely because all selected files were written.
- Construct validated immutable weapon/sensor/tank configuration once; retain
  raw reports. Avoid `Equipment::number`'s current missing/invalid-to-zero
  convenience fallback for new simulation configuration.
- Share armament selection between CLI/app, update cache requirements, preserve
  safe paths, archive boundaries, size/conflict checks and media provenance.

Gate: whole JT library and both reviewed aircraft extract twice (second run
unchanged), including mandatory shared effects. Synthetic tests cover missing
textures/audio, cycles, bounds, filtering, duplicate builds and conflicts.
No raw/generated retail resources become committed fixtures.

### W2 — Headless ordnance and sensor foundation

Use dedicated renderer-independent combat modules in `tore-sim`, immutable typed
configuration and caller-owned ammo, target IDs, seeker state, timers and RNG.
Keep each aircraft's complete configuration in its own model; reviewed stores
loading feeds that model rather than a mutable universal aircraft tuning profile.

| Family | Movement/lifecycle to recover | Required coupling |
| --- | --- | --- |
| Internal/pod guns | Representative shots, burst/reload, launch speed, deceleration, fall, spread, lifetime | Mount, trigger, ammo, collision/damage |
| Unguided rockets | Salvo pattern, ignition, powered/coast acceleration, fall, dispersion, removal | Pod capacity/mass, launch attitude/speed, environment |
| Ballistic/retarded bombs | Release/retard, drag/fall, terminal limits, arming, impact | Station transform, body versus movement, land/water query |
| Guided missiles/bombs | Ignition/burnout/coast, altitude performance, turn laws, cruise/jink, lock loss/reacquisition | Radar/IR/laser/emitter tracking, illumination/designation, ECM |
| Cluster/special | Native release/fuze, submunition or area-effect representation, blast/special branches | Event ordering, RNG, classification/collateral |

These are research families, not a claim FA uses modern aerodynamic solvers.
Share native generic kernels where established and preserve native branch
differences. Do not invent drag coefficients or one universal homing equation.

Sensor flow: actual objects/signatures/emissions → SEE search/track → weapon
acquisition and launch zones → illumination/designation/target state → JT
guidance plus ECM → fuze/impact/damage. Keep detection, acquisition, launch
permission and post-launch tracking distinct. A radar range setting does not
establish detection logic.

Recover signature selection, FOV/aspect, look-down/Doppler, terrain masking,
cadence, lock timers, sun terms, deception/chaff/flare, jammer and anti-radiation
behavior. Trace active versus illumination-dependent radar branches. Instrument
snapshots must reflect real contact/weapon state; no fabricated lock or threats.
Use `telemetry::AirData` with explicit environment inputs when needed; do not
substitute TAS for IAS or geometric altitude for pressure altitude.

Gate: exact arithmetic/branch tests and complete lifecycle probes with explicit
targets, terrain, wind, clocks and RNG. Synthetic targets belong to test fixtures,
not populated free flight. Unknown coupling stays diagnostic until accepted.

### W3 — First complete playable gun slice

- Connect F18/M61 and Rafale/DEFA independently: source mounts, ammunition,
  trigger/release, movement, collision, damage, original bullet art and audio.
- Recover damage-class mapping, direct/proximity/collateral decisions and
  destruction. Do not multiply damage by representative ammo count without
  native evidence. Trace damage effects back into aircraft performance.
- Decode SH near/line/point branches, native scale and palette behavior. Do not
  apply the aircraft renderer's provisional one-third-foot scale to bullets.
- Wire actual firing events to weapon instruments, input and existing rumble
  cues; test modifier/release isolation, focus loss and pause/resume.

Gate: matched retail traces/captures for both guns: bursts, empty ammo, different
launch speeds, hits/misses and lifetime. Determinism alone is not vanilla parity.

### W4 — Stores, sensors and remaining ordnance

- Implement compatible loadout state, mass/drag, release and jettison. Preserve
  clean external free flight until loadout behavior exists. Full loadout-menu
  work remains deferred; headless fixtures can validate loaded states first.
- Progress through rockets/bombs and IR/radar/laser/anti-radiation/cruise/cluster
  branches according to their sensor dependencies. Cover every distinct flag
  combination or leave its status explicitly open.
- Render original carried/released models, trails, impacts, water effects,
  craters, debris and countermeasures from actual events. Recover effect
  scheduling, size/lifetime, palettes and sounds rather than substituting art.
- Connect real lock/target/RWR state to instruments and combat music only to
  implemented context/event contracts. Maintain responsive full-canvas layout.

Gate: each aircraft-compatible store has extraction, configuration, movement,
sensor, effect and acceptance status. Complete both reviewed identities before
widening flyable aircraft. Inventorying 145 PTs does not make them flyable.

### W5 — Vanilla acceptance and regression

- Establish original-game baselines per family/branch: speed/altitude/aspect,
  launch/motor/coast/range/lifetime, turn/lock loss, countermeasures, arming
  boundaries, land/water impact, damage/collateral and release mass/state.
- Record build/archive hashes, inputs, difficulty, seeds where observable,
  sampling uncertainty and tolerances before comparing. Integer kernels should
  match exactly; observational tolerances require a measurement reason.
- Repeat deterministic engagement tapes across render cadences, pause/resume
  and sustained load. Interpolation and visual/audio effects cannot alter state.
- Run formatting, warnings-denied Clippy, locked tests/build, Python tests and
  source/binary asset guards. Rendering changes require creator/viewer/flight
  GPU checks, wide/tall captures and live camera checks. Use bounded
  [frame-time diagnostics](../baselines/flight-performance.md); distinguish CPU
  intervals from GPU timing or displayed FPS.
- Validate Linux/Windows/macOS and report unavailable runtime/hardware checks.

Completion means every claimed weapon passes its full contract; raw preservation
or a translated helper is never labeled full native performance parity.
