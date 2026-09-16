> **Frozen as of 2026-09-15. Superseded by the parity strategy (D30) — see [AGENTS.md](../../AGENTS.md) and [the parity plan](../parity-plan.md).** Kept for its recovered facts, dated checkpoints and evidence links. Its sequencing, gates and status columns are no longer authoritative.

# Flight response and maneuver buffet plan

Requested 2026-09-15. **Remaining native dependencies now follow the
[living environment/systems plan](../research/native-environment-systems-plan.md).**
This is an implementation and acceptance plan; unchecked items claim no new
native behavior. Scope is the reviewed F/A-18D (`F18.PT`) and Rafale C
(`RAFALE.PT`), preserving the legacy default and explicit hybrid selection.

## Current priority and provenance

Scheduling clarification: after the restricted airborne connection, the next
contact, ground/sea asset, lifecycle/event, decoy/guidance and environmental
continuation is organized in the linked living plan. Its creation turn is
planning only and includes no AI scope. Maneuver audio/rumble and final response
acceptance follow those dependencies; detailed feedback work remains here.


User clarification 2026-09-15: focus first on **native** departure/tumble and
control/force/movement coupling, with source-derived expected behavior. A useful
retail comparison is unavailable; continue implementation using source contracts
and tests, without claiming retail trajectory parity. Resume audio/rumble
afterward. Follow
[behavior provenance](../behavior-provenance.md): separate native implementation
steps, existing fitted choices, and explicitly user-directed additions.

The earlier “steps 1–3 complete” summary overstated native coverage. The
[adapter baseline](../baselines/flight-response.md) records completed component and
regression work; native steps 2–3 remain open where fitted laws or missing
coupling remain. Legacy is still the default. Native-derived warning/stall/spin
components retain fitted hybrid boundaries; a separate restricted airborne native
option now runs the joined service. [Live scope](../baselines/native-live-flight.md).

### Native continuation before step 4

- [x] Establish native warning-transition tumble scheduling and movement-fall
  source branches; record [contracts and translation status](../formats/native-flight.md#native-tumble-continuation--2026-09-15).
- [x] Join native tumble/fall/spin dispatch and movement composition; validate
  synthetic contracts and both PTs with imported tables. [Evidence](../baselines/native-departure-stage.md).
  This is diagnostic component acceptance; no live activation or retail comparison is implied.
- [x] Resolve initial/bounded/current-G envelope roles and the non-VTOL gate
  for both supported profiles; translate/test the departure force-G override
  and ordered force/velocity stage. These remain diagnostic components.
- [x] Join primary G/pitch/AoA/roll consumers and movement through explicit
  post-query contact settling; validate component tests and both PT snapshot
  probes. [Evidence](../baselines/native-movement-control.md).
- [x] Finish the joined diagnostic: loaded G/control/drag consumers, passive fall,
  auxiliary rates, full rudder/steering, ordered forces/movement/contact and
  returned events. Both PTs pass recurrent state/replay probes.
  [Evidence and explicit native turbulence bypass](../baselines/native-flight-diagnostic.md).
- [x] Connect the joined service to an explicit airborne live research path, with
  caller-owned clock/RNG, output projection, restart and error rollback. Validate
  both aircraft headlessly and on the GPU. [Evidence](../baselines/native-live-flight.md).
- [ ] Connect terrain/carrier queries, equipment/fuel/damage lifecycle producers,
  event execution and complete native runtime ownership before unrestricted
  activation. The airborne path consumes adapted samples and rejects contact;
  it does not close these lifecycle/landing gates.
- [ ] Compare source-derived expected outputs and, when available, matched
  retail maneuvers. Record missing implementation separately from missing evidence.

## Current coverage and remaining gates

[FLIGHT-MODEL](../FLIGHT-MODEL.md) describes the working adapters and their fitted
parts. [Native flight research](../formats/native-flight.md) records translated
components and unresolved whole-tick contracts. Those are the existing backlog;
this plan orders the next flight-response slice.

| Area | Already present | Work to finish in this slice |
| --- | --- | --- |
| G-load / AoA | Source ledger, separate demand/lift/achieved G and geometric AoA telemetry; acceleration checks | Loaded/damage force ordering connected in airborne research; native lifecycle producers and feedback remain |
| Roll rate | Applied body-rate snapshot, verified release and full-loop probes; source hybrid/fitted legacy caps | Control/load/damage consumers connected in airborne research; native lifecycle producers remain |
| Rudder / slip | Distinct command/deflection/effective controls, fitted symmetric slip drag and yaw release; tested native spin predicates | Native display-slip/force coupling connected in airborne research; native lifecycle and unavailable retail acceptance remain |
| Departure / spin | Translated warning/stall predicates and timers, connected severity/control/lift attenuation; corrected hybrid spin entry/recovery with fitted continuous coupling | Native initial classification and movement fall/tumble translated diagnostically; restricted airborne live coupling is connected; lifecycle/contact producers remain open |
| Maneuver feedback | Native sound-side `Turbulence` routine traced at `0x434550`, caller at `0x434d76` | Recover full input-to-intensity and sound dispatch contracts; integrate a distinct maneuver-feedback signal |
| Controller rumble | Environmental turbulence supplies severity only when its shake flag is set; overlapping pulses support sustained strong events | Add sustained maneuver feedback with intensity tracking, release and lifecycle checks; audit the mild environmental threshold separately |

The sound routine reads G, roll rate, rudder, departure, speed and device/state
flags. That establishes maneuver-responsive sound logic, **not an aerodynamic
buffet-force equation or a verified rumble mapping**. Do not translate its sound
intensity into forces. Do not trigger feedback merely from stick deflection.
[Source boundaries](../formats/weather.md#maneuver-effects-and-sound-are-separate).

## 1. Trace producers and establish a baseline

- [x] Extend the repeatable static extraction pass for the relevant flight and
  sound callers. Record executable/resource hashes, source-build distinctions,
  offsets, units, signedness, clamps, update order and unresolved branches.
- [x] Map G, actual roll rate, rudder command/deflection, AoA/slip and departure
  state from source producers through force/control, display and sound consumers.
  Establish whether a channel is a demand, measured response or display offset.
- [x] Record the current behavior of both adapters and aircraft before edits:
  level flight, hard pull and push, sustained turn, roll/release, rudder/release,
  low-speed warning/stall, spin entry and recovery.
- [x] Capture per-tick inputs, configuration identity, wind/atmosphere/contact,
  G/AoA/slip, body rates, velocity, attitude, departure state/timers and RNG state.
  Put local source-derived traces in `.local/`, committed methodology and results
  in `docs/baselines/flight-response.md` when that evidence exists.

**Gate:** a producer/consumer ledger distinguishing translated, fitted and unknown
behavior; repeatable baseline for both identities and adapters. Native modules
remain inert data. A missing native branch stays explicitly unresolved.

## 2. Finish G, roll and rudder response contracts

- [x] Correct verified input/output units and state ownership before tuning.
  Resolve reviewed PT fields once into each model's typed configuration; validate
  configuration replacement. Keep F18 and Rafale laws in their own modules.
- [x] Complete native G-envelope/loading and pitch-response consumers in the
  diagnostic and restricted airborne path, checking
  positive/negative load, low/high speed and relevant device/loading changes.
  Do not equate requested G or raw stick position with achieved load factor.
- [x] Complete native roll authority, acceleration/limiting and release behavior
  in the diagnostic and restricted airborne path;
  preserve actual body rates through vertical/inverted flight.
- [x] Complete native rudder authority, slip/drag and roll/yaw coupling in the
  diagnostic and restricted airborne path.
  Document any remaining fitted law per aircraft instead of borrowing calibration.
- [x] Expose typed maneuver telemetry only where existing `AirData`/state channels
  are insufficient; consumers must use one authoritative fixed-tick snapshot.

**Gate:** symmetric left/right probes where supported by the model; finite
full loops through both vertical attitudes; stable release; wind advection once;
consistent G/AoA/rate telemetry. Preserve independent aircraft attitude and
velocity. Do not silently switch the default adapter or claim whole-tick parity.

Diagnostic status: native G/pitch/roll/rudder consumers above are now joined and
tested for both PTs. The complete native **producer/lifecycle** gate remains open;
restricted airborne runtime connection is now tested. No fitted host device or
fuel producer has been promoted to native acceptance.

## 3. Complete supported departure and recovery behavior

- [x] Trace the supported non-VTOL envelope/difficulty/device predicates and
  warning timers. Diagnostic source predicates are separate from the existing
  fitted live stall-entry gate; runtime replacement remains open.
- [x] Verify control/lift attenuation coupling and implement recovered departure
  consumers, including pitch/roll fall or tumble only when their contracts are
  established, connected in the restricted airborne path. Keep random choices and
  mutable timers outside configuration.
- [x] Verify spin direction, entry/recovery thresholds, interrupted recovery,
  neutral/opposite rudder, throttle/pitch requirements and source state flags for
  each aircraft. Preserve movement/display-angle distinctions.
- [x] Make warning/departure state available to feedback without feeding a
  presentation effect back into aerodynamic state.

The requested remaining diagnostic is complete: native departure attenuation,
tumble, recovery dispatch and downstream control/force/movement/contact order
are joined. Restricted airborne live activation is now tested; external lifecycle/query
producers and event execution remain open. See the [joined baseline](../baselines/native-flight-diagnostic.md).

**Gate:** deterministic warning → stall/spin → recovery traces, threshold-boundary
and timer-interruption tests, no hidden RNG draws from audio, rumble or cameras.
Unrecovered native coupling remains listed rather than filled with assumed physics.

## 4. Connect maneuver buffet feedback and original audio

- [ ] Recover the complete sound-intensity helper, activation gates, sample
  selection, gain/retrigger/stop behavior and call cadence. Verify source G/rate
  scales before using host floating-point channels.
- [ ] Produce maneuver-feedback state from the verified flight channels. Keep
  environmental turbulence, maneuver feedback and any verified aerodynamic
  disturbance independently identifiable in diagnostics.
- [ ] Add an explicitly authored controller mapping: sustained nonzero maneuver
  intensity sustains rumble; changing intensity changes strength; release/recovery
  clears it promptly. Define bounded attack/release, clamping and mixer priority
  so repeated pulses do not accumulate or mask weapon/damage cues unexpectedly.
- [ ] Test strong stick input with little achieved response, sustained hard turns,
  rolls, rudder maneuvers and departure/recovery. Do not assume every positive G
  or roll rate must activate a cue; use recovered sound gates and label authored
  haptic choices separately.
- [ ] Connect original audio only after its dispatch is verified. If no exact
  sample/caller mapping is established, record the gap instead of choosing a
  similarly named resource. Sound-side thresholds are not force laws.
- [ ] Verify settings semantics: effects mute, rumble disable, pause/focus loss,
  device disconnect, crash, restart and aircraft switch clear/restore the correct
  state without stale pulses. Trace whether **No turbulence?** also affects native
  maneuver sound; do not assume the environmental cheat suppresses all buffet.
- [ ] Audit the existing environmental shake threshold. Document whether mild
  turbulence feedback should remain gated or gain an authored continuous mapping;
  any change must be independently tested and must not alter environmental RNG.

**Gate:** fixed-input intensity/envelope tests plus sustained physical-controller
and audio checks on available hardware. Enabled/disabled feedback must yield
identical flight-state traces. No rumble or audible acceptance claim from tests
that only exercise the mixer. Record hardware/platform availability honestly.

## 5. Acceptance and aircraft-import handoff

- [ ] Run the same `--validate-flight` suite for both aircraft, plus the new
  maneuver probes in legacy and hybrid modes. Include explicit calm/crosswind,
  pause/resume without catch-up, restart, and deterministic same-input replay.
- [ ] Cover hard pull/push and release at multiple speeds/altitudes; sustained
  turns; left/right roll and rudder; warning/stall/spin entry and recovery;
  gear/flap/airbrake and supported mass cases where relevant to traced consumers.
- [ ] Run formatting, warnings-denied Clippy, tests and build with `--locked`,
  Python checks and asset guards as listed in [DEVELOPMENT](../DEVELOPMENT.md).
  Use synthetic fixtures for committed tests.
- [ ] Run flight/camera GPU checks and creator/viewer smoke tests for rendering
  changes. Capture wide/tall composition if feedback changes the view; preserve
  full-canvas HUD/cockpit projection and screen-anchored instruments. Record
  bounded frame-time evidence for flight-performance changes.
- [ ] Compare matched retail maneuvers when recordings/runtime are available,
  recording input/aircraft/loadout/conditions and measurable differences. Separate
  synthetic invariants, host regression results and retail parity evidence.
- [ ] Update `FLIGHT-MODEL.md`, native research, progress and the acceptance record
  with completed substeps and unresolved gates. Next add F-14, A-4E and X-31
  using the [aircraft import guide](../aircraft-import.md), then return to remaining
  weather work. Carry unavailable retail/platform evidence explicitly.

## Broader flight-model backlog retained

This slice does not close native full-tick trajectory parity, integer scheduling
and global RNG order, terrain/object collision/cache and carrier producers,
remaining equipment/damage/fuel-transfer coupling, pitot/static calibration,
indicated instruments or new-aircraft acceptance. These stay in
[flight-model status](../FLIGHT-MODEL.md) and [progress](progress.md). Broader weather,
AI and remaining menu screens are outside this scheduled slice. New aircraft
follow it as a separate scheduled pass through the import guide.
