# Complete aircraft import and acceptance guide

Updated 2026-09-15. Start here when adding an aircraft. This guide joins the
existing extraction, format, simulation, presentation and systems contracts;
linked research remains authoritative for byte layouts and native behavior.
“Imported” is not synonymous with “fully implemented” or “retail validated.”

## Scheduled aircraft and execution order

1. Continue the F/A-18D/Rafale C native foundation through the
   [living environment/systems plan](native-environment-systems-plan.md), then
   resume the [maneuver audio/rumble and acceptance slice](flight-response-plan.md).
   The new plan is documentation only in its creation turn; no AI scope.
   Keep [native, fitted and user-directed provenance](behavior-provenance.md) separate.
2. Add **F-14**, then **A-4E**, then **X-31**, using the per-aircraft gates below.
3. Resume remaining [weather work](weather-plan.md) outside the environment/systems dependencies.

The new aircraft are scheduled, **not supported yet**. Review each exact FA PT,
variant, shape/HUD names and resource hashes before registering an identity.
In particular, “F-14” does not select a specific variant by itself; inventory the
available FA variants and record the chosen identity before implementation.
Do not infer resource filenames or copy another aircraft's native metadata.
Broader AI, new menu screens and the rest of the aircraft roster remain deferred.

## Current coverage and authoritative references

| Area | Implemented for F18 / Rafale C | Remaining work / reference |
| --- | --- | --- |
| Identity and extraction | Reviewed F18.PT = F/A-18D and RAFALE.PT = Rafale C; shared app/CLI dependency closure, source hashes, bounded readers | Other variants/layouts need review. [Extraction](EXTRACTION.md), [aircraft formats](formats/aircraft.md), [format coverage](formats/coverage.md) |
| Flight configuration | Separate models own validated typed configuration; source mass, engines, envelopes, controls/equipment and departure fields resolve at construction | Unresolved source fields stay explicit; no per-tick raw PT access. [Model contract](FLIGHT-MODEL.md) |
| Flight dynamics | Fixed 120 Hz shared integration, independent attitude/velocity, fuel/devices, legacy and selectable hybrid paths; hybrid departure/spin/contact helpers | Fitted response/coupling remains; exact whole-tick native parity is open. [Native research](formats/native-flight.md) |
| G / roll / rudder / departure | Supported response/telemetry and hybrid stall attenuation/spin recovery validated for both identities/adapters | Restricted airborne native coupling is tested; contact/lifecycle producers are next under the environment/systems plan. [Flight-response plan](flight-response-plan.md) |
| Exterior and animation | Both original shapes/cockpits; separate aircraft animation mappings and fitted device travel | General native SH execution, exact hinges/schedules, LOD/shadow/damage and store-placement acceptance remain incomplete. [Shapes](formats/objects-and-shapes.md) |
| Cockpit / HUD / instruments | Original art/fonts, full flight canvas, responsive overlays, rear mirror and asynchronous camera windows; supported live instrument channels | Full native HUD/window composition and unmodeled system readings remain open. [Aircraft formats](formats/aircraft.md), [controls](FLIGHT-CONTROLS.md) |
| Environment / air data | Shared resolved wind, terrain/standard-atmosphere AirData, camera-local weather, physical turbulence and corrected own-shape vapor attachments | Native atmosphere, wake/contact/coupling, broader vapor and weather acceptance remain partial. [Wind/turbulence evidence](baselines/wind-turbulence-vapor.md) |
| Audio / feedback | Original engine/device audio, weapon/damage feedback, afterburner rumble, strong environmental turbulence rumble | Maneuver buffet and verified original dispatch are next; audibility/hardware acceptance is distinct from event tests. [Flight-response plan](flight-response-plan.md) |
| Weapons / sensors / damage | PT-default manual range, supported weapon/sensor/ECM/damage behavior; source stations and partial custom-load creator flow | Not full JT catalog or aircraft-system parity. [Systems](baselines/weapons-systems.md), [creator/loadout](baselines/creator-ordnance.md) |
| Validation | Same aircraft flight suite, deterministic same-host probes, rendering and documented Linux checks | Retail trajectory, complete systems and unavailable platform checks remain separate gates. [Flight baseline](baselines/shared-flight-model.md), [progress](progress.md) |

Older first-pass sections in individual docs describe historical limitations.
Use their dated follow-ups and the current evidence above when assessing runtime
support. Missing telemetry must remain unavailable, not fabricated to fill a gauge.

## Per-aircraft implementation gates

### A. Identify the source and dependency closure

- [ ] Inventory the user's FA media; record exact displayed identity, PT name,
  schema/type size, archive/provider, source hashes and source-build distinctions.
- [ ] Review PT/object fields, G polygons, mass/fuel/thrust, equipment, default
  stores, hardpoints and referenced SH/HUD/SEE/ECM/JT/GAS/audio resources.
- [ ] Add the reviewed identity/profile to the shared resolver used by app and CLI.
  Preserve archive boundaries, safe paths, limits, conflicts and missing-reference
  reason chains; required unknowns fail rather than silently using another plane.
- [ ] Extend bounded readers only for reviewed layouts, using synthetic malformed
  and boundary fixtures. Update `formats/coverage.md` for actual reader coverage.
- [ ] Verify runtime cache refresh and CLI extraction independently. The app
  imports from user media into application data; CLI outputs are research files,
  not an alternate runtime asset bundle. Never execute imported native modules.

**Gate:** reproducible complete selected dependency report, including explicit
optional/unresolved references. Extraction success alone does not enable flight.

### B. Build the aircraft's own model

- [ ] Create a separate `tore-sim::models` implementation and complete typed
  configuration from that aircraft's reviewed source fields. Validate before
  replacement; no raw profile/string cache in update ticks and no guessed zeros.
- [ ] Review speed/G envelopes, loading, positive/negative G, roll/rudder authority,
  engine count/thrust/fuel, devices, departure/spin and contact limits.
- [ ] Apply the completed flight-response contracts to this identity and verify
  its own behavior. Shared helper code is reusable; aircraft calibration and
  configuration remain independently owned and explicitly source-based or fitted.
- [ ] Review special control/propulsion modes from source. For F-14, investigate
  variable wing geometry and its flight/attachment consequences. For X-31,
  investigate thrust-vector/control modes and departure behavior. These are
  research targets, not claims that specific native modes are decoded or present.
- [ ] Preserve 120 Hz deterministic state, explicit wind/atmosphere/contact and
  clock/RNG inputs, independent movement/body attitude and both adapter boundaries.

**Gate:** the same headless suite runs for the new identity; differences are
explained by its reviewed configuration or documented fitted laws. No aircraft
is accepted by merely reusing F18's numbers until it happens to fly.

### C. Recover exterior, devices and attachments

- [ ] Project the aircraft's own SH with the bounded reader; review branches,
  materials, normals, UV/palette rules, scale and coordinate conventions.
- [ ] Build its own device/control-surface rig, using verified source structure.
  Never apply F18 or Rafale code offsets to a new shape. Document fitted hinges,
  schedules and unsupported topology instead of claiming native animation parity.
- [ ] Check neutral and deployed gear/flaps/brakes/hook and applicable aircraft-
  specific moving parts, plus engine-off/on and afterburner where supported.
- [ ] Validate CE vapor, gun/muzzle, hardpoint/store and other effect origins
  against that aircraft's own records, mesh and component transforms. Mesh uses
  right/forward/up; reviewed CE uses right/up/forward. Do not assume every record
  shares either convention. Check identity, yaw, pitch, bank and moving geometry.
- [ ] Record unimplemented LOD, shadow, damage and material behavior. Preserve
  original art and raw data; no retail derivatives in committed assets.

**Gate:** neutral/device and attachment captures plus numerical transform checks.
A correctly attached vapor trail does not establish emission/material parity or
validate store placement; each consumer needs its own evidence.

### D. Integrate cockpit, controls and telemetry

- [ ] Resolve the aircraft's own HUD/cockpit variants, fonts, mirror artwork,
  instrument dependencies and device capabilities from source.
- [ ] Preserve cover-fit forward art, full-canvas world/HUD, independent instrument
  overlays, body-coordinate head-look and renderer-consistent projection.
- [ ] Consume actual typed telemetry and supported sensor/system state. Keep
  IAS/CAS, pressure/indicated altitude and unmodeled engine/system readings absent
  until their producers exist. No invented contacts or healthy-system values.
- [ ] Preserve source versus provisional input labels, modifier/release isolation,
  unsupported-device handling, pause/resume and aircraft-switch resets.

**Gate:** cockpit/exterior/mirror/panel and wide/tall captures, control release and
restart checks. Original cockpit art does not imply complete instrument parity.

### E. Connect effects, audio, stores and systems

- [ ] Connect the completed maneuver-feedback contract to this aircraft's actual
  G/rates/rudder/departure state and its own source gates; verify sustained rumble,
  release and device/mute/pause lifecycle without changing physics or RNG.
- [ ] Verify engine/start/stop/actuator and maneuver audio through original resource
  references and dispatch. Record any authored haptic mapping separately.
- [ ] Resolve weapons, ammo, stations, compatibility, radar/visual/ECM and supported
  damage behavior. Test actual launch/muzzle/attachment transforms and supported
  loadout mass/drag effects; extracted definitions do not enable unsupported weapons.
- [ ] Keep ordinary free flight externally clean. Validate deliberate range and
  creator loadouts separately, including fuel/weight, restart and replay boundaries.
- [ ] Add creator selection only with honest capability validation and reviewed
  aircraft metadata/art; avoid exposing a selectable but incorrectly configured plane.

**Gate:** supported equipment and default-load scenarios, original audio checks,
controller checks where hardware exists, and explicit unavailable systems.

### F. Acceptance record and support status

For each aircraft, create `docs/baselines/aircraft-<reviewed-id>.md` with:

- Exact identity/resources/hashes, import commands and dependency report location.
- Implemented versus translated/fitted/unavailable subsystems and open decisions.
- Same flight-suite results plus G/roll/rudder/departure/buffet probes; level,
  pull/push/turn/loop, roll/rudder release, stall/spin/recovery, wind, fuel and
  explicit-runway/contact cases. Never infer safe runways from arbitrary T2 height.
- Aircraft-specific controls/devices/equipment tests and matched retail evidence
  when available, kept distinct from host regression tests.
- Visual captures, sound/controller observations, platform/driver/hardware and
  bounded performance results for flight-performance changes.
- Formatting, Clippy with warnings denied, tests/build `--locked`, Python and asset
  guard results; creator/viewer and flight/camera GPU checks for rendering changes.
- Deterministic headless replay and feedback/camera independence, pause/restart,
  aircraft-switch state isolation and failures without stale mutable state.

Use the terms **source reviewed**, **extraction supported**, **headless flight
supported**, **rendered flight supported**, **systems partially supported**, and
**retail validated for named cases** independently. Record unavailable checks;
do not collapse these into a single “complete” checkbox. Update this guide,
`formats/coverage.md`, `FLIGHT-MODEL.md` and `progress.md` with actual results.

## Commands and implementation entry points

Current supported extraction/flight validation commands (these do not enable the
three scheduled aircraft):

```sh
python3 tools/extract_assets.py --aircraft f18 --exclude-archive 'disc1/LHX/*' --out .local/aircraft/f18 --validate-flight
python3 tools/extract_assets.py --aircraft rafale --exclude-archive 'disc1/LHX/*' --out .local/aircraft/rafale --validate-flight
cargo run --locked -p tore-app -- --free-flight --aircraft rafale --researched-flight
```

The LHX exclusion only skips unrelated bundled media; adapt source selection as
specified in [EXTRACTION](EXTRACTION.md). Register and document new CLI identities
only after their source review; do not use speculative F14/A4E/X31 flags today.

Start with `tools/extract_assets.py`, `crates/tore-extract`, the shared
`tore-formats` aircraft/schema/resource-selection code, `tore-sim::models` and
its flight-suite example, then the app aircraft/animation/cockpit/system consumers.
Keep formats and simulation independent of rendering and preserve the shared
app/CLI resolver. Detailed commands and host checks live in
[DEVELOPMENT](DEVELOPMENT.md).
