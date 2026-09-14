# Aircraft weapons: FA research and implementation plan

Research date: 2026-09-14. This schedules aircraft armament research and subsequent
importer/simulation work. It does not mark combat or vanilla parity complete.
[Measured extraction and code evidence](../baselines/weapons-research.md).

## Scope and fidelity contract

Recover the supplied Fighters Anthology game's internal guns, gun/rocket pods,
guided missiles, ballistic/retarded/guided bombs, cluster/special ordnance, tanks,
targeting equipment and countermeasures. Inventory the whole JT library so
aircraft compatibility research cannot silently lose alternatives. Ground/ship
weapons remain identified separately; importing their definitions does not
schedule ground/ship AI. Preserve unusual vanilla weapons and statistics even
when they differ from real-world specifications.

Missiles, bombs, rockets and bullets need movement models, not just graphics.
Acquisition, launch permission, guidance, motor/coast phases, fuzes, impacts,
damage and countermeasures must form one reviewed lifecycle. A missile following
an authored pursuit curve does not establish vanilla performance. Nor does a
real-world gun rate substituted for FA's representative-projectile accounting.

The target is unchanged vanilla gameplay performance: launch conditions, cadence,
ammunition, trajectories, range, guidance, sensors, damage and stores effects.
Preserve native arithmetic/order when it affects those outcomes. Keep rendering
independent of authoritative 120 Hz state. Verify native service-clock conversion;
never equate a render frame or Rust tick with one native update. Do not add
realism corrections, rebalance weapons or adopt the reference engine's fallbacks.

Current aircraft flight remains legacy/hybrid. Faithful projectile components
coupled to those adapters cannot establish whole-engagement parity until aircraft,
contact and native scheduling gates also pass. See [native flight](native-flight.md).

## Source hierarchy

1. User-owned FA archives and this exact FA.EXE/FA.SMS build, with hashes and
   bounded offsets. Source records establish values, not consumer semantics.
2. Static FA consumer/caller analysis, with unresolved branches and external
   state recorded. SMS names locate routines; names alone prove no behavior.
3. Matched original-game observations for end-to-end acceptance. The importer,
   app and tests never execute imported native code or drawing modules.
4. Ignored USNF-ATF format notes/readers as research leads. Their other-title
   addresses, hypotheses and custom engine are not FA parity evidence.

No original C/C++ source tree was identified in the supplied media/reference
checkout during this pass. Original code here means statically inspected retail
machine code and compiled asset modules. The reference gun exporter contains
authored ballistics/mounts alongside USNF-derived native fields; neither can be
copied wholesale as the FA specification.

## Initial exporter audit (before implementation)

| Area | Current result/code | Completion needed |
| --- | --- | --- |
| Whole weapons catalog | Shared resolver roots every JT with `--weapons`; 135 named JT analyses succeed | Classify all native branches; library membership is not aircraft compatibility |
| Aircraft bindings | 145 PTs contain 70 unique literal JT references; all 70 exported | PTS alternatives, compatibility masks, station pairing, racks/pods, availability and mass rules |
| App/CLI selection | App imports F18/Rafale with `weapons=false`; wrapper accepts one aircraft, Rust CLI repeated flags | Shared armament profile/cache validation and deliberate wrapper union support |
| Discovery | Token scan follows PT/JT/SEE/ECM/GAS/SH/HUD; exact names or `.PIC` suffix | Typed edges, aliases, generated names, executable tables and reason chains |
| Missing files | BRF JT/SH/SEE/ECM/GAS/11K/5K references fail when absent | Equivalent required PIC/nonliteral checks and reported optional/unknown edges |
| Weapon graphics | 75 SH, 61 PIC preserved | Native scales, mounts, LOD branches, animation and actual weapon rendering |
| Shared effects | Six graphics-initializer SH roots demonstrably absent | Add reviewed shared roots and follow their textures/audio/effect tables |
| Native code | JT preserves inert `_PROJProc` symbol, not its implementation | Separate hash-gated static weapon research and bounded Rust translations |
| Equipment | Named JT/SEE/ECM; GAS raw/BRF preservation | Checked typed configuration, resolved pointers and verified units |
| Provenance | Archive boundaries, hashes, bytes and named raw/scaled fields | Dependency edges, overrides, unresolved mappings and separate extraction/decode/runtime/parity statuses |

`--include` filters after closure and can remove required files. Mark these
exports filtered/incomplete. CLI dependency lookup takes the last matching archive
in input order, while extraction preserves matches from every archive. App import
reads FA_1/FA_2. Record duplicate/build selection before widening profiles; do not
silently prefer a patch/disc variant or imply every variant was reviewed.

### Confirmed gaps and FA checks

`_GRAPHICInit` (0x442c00) requests CRATER.SH, SMOKE.SH, FIRE.SH, DEBRIS.SH,
CHAFF.SH and FLARE.SH. All exist and none is in the measured weapons export.
Their module strings name CRATERS.PIC, SMOKE.PIC, FIREA.PIC and FLARE.PIC;
these also exist and are missing. The already-selected FIRE.PIC is different art.
Executable sound-string candidates missing from selection include &EXPL12.5K,
&SPLASH3.11K, &FIRE.5K, &CHAFF.5K and &FLARE.5K. Finish table/caller tracing
before mapping sounds to effect indices. Similar names alone are insufficient.

FA `_PROJSpeed` (0x4c1120 through return at 0x4c1163) takes launcher speed
shifted right eight, multiplies by unsigned JT launchRetard (+0x115), divides
by 100, takes the maximum with signed initialSpeed (+0xfb), then clamps to
signed _minSpeed/+0x67 and _maxSpeed/+0x6b. These offsets agree with the local
315-byte packed JT schema. It is scalar selection, not vector addition of
aircraft velocity. This is static arithmetic review, not differential execution.

M61 and DEFA both contain initial/final speed 2933/1466, actualRoundsPerGame=2,
gameBurstT=1 and removeT=40. Hornet capacity is 570; Rafale C is 250. FA
`_PROJFire` reads ammunition debit at +0xf0 before calling `_HARDUnload`.
The older USNF note uses +0xec. Trace FA player dispatch, burst/reload scheduling
and unlimited-ammo gates before assigning rounds/second or seconds to those
raw timing fields. Do not transplant USNF offsets or cadence claims.

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

## FA entry points for the next pass

SMS locations below are research starting points, not fully reviewed routines
or a complete call graph.

| Contract | Entry points |
| --- | --- |
| Loading/mount | HARDCanLoad 0x452980; HARDLoad 0x452c20; HARDPos 0x4532a0; HARDPodHack 0x453710; HARDStoreWeight 0x452940 |
| Fire/service | PROJAdd 0x4c0a90; PROJFire 0x4c2170; PROJServiceWeapon 0x4c4700; PROJProc 0x4c1f50 |
| Movement | PROJSpeed 0x4c1120; PROJEngineState 0x4c1170; PROJMoveProc 0x4c11b0; PROJBombPos 0x4c4050 |
| Seeker | PROJInFOV 0x4c2860; PROJLock 0x4c2f20; PROJLockUpdate 0x4c0960; PROJRadarIsOn 0x4c2eb0; PROJSelectTarget 0x4c4100 |
| Equipment/CM | HARDBestSeeker 0x452e60; HARDFindJammer 0x452ea0; HARDFindECMForObj 0x452f10; PROJLaunchDevice 0x4c39a0 |
| Damage | PROJHitChance 0x4c3380; PROJHit 0x4c20c0; PROJDamageProc 0x4c1870; PROJSendCollateralDamages 0x4c5670; DAMAGEDoHit 0x40f970 |
| Graphics/audio | GRAPHICInit 0x442c00; GRAPHICAddExp 0x4432d0; GRAPHICAddSmoke 0x443e80; GRAPHICAddDebris 0x4441d0; PROJFireSound 0x4c26f0 |

Do not classify the inventory symbol `_explode` as combat explosion code merely
from its name.

## Development live-fire adapter (subsequent implementation)

The user's subsequent live-fire request explicitly permits documented
approximations. `tore-sim::combat::live` is that connected development adapter;
it does not promote the diagnostic components to native-parity status.

Identity resolution follows `AircraftId::ALL`, the F18 and Rafale C model modules,
reviewed cockpit/exterior assets and each PT's actual hardpoint records. Only
F18.PT (F/A-18D) and RAFALE.PT (Rafale C) are supported. Live configuration resolves
PT weapon counts/mounts and JT movement/damage/sensor/effect fields once. The
Hornet has M61/570, AIM120/2, two AGM65G/4 groups and AIM9M/2; Rafale C has
DEFA/250, AGM65G/4, MICA/2, R530/2 and R550/2. These are **PT defaults**, not a
recovered mission-specific loadout preset or a new compatibility claim.

Source launch speed, motor states, axial acceleration/deceleration, altitude
performance, expiry, fall and ammo-debit arithmetic are reused. The connected
adapter advances at 120 Hz with an authored remainder conversion to 256-unit
service time and four-unit-per-second deadlines. Representative burst count
and per-projectile ammunition debit come from JT; burst grouping and input
service ordering remain approximate. No real-world RPM replaces FA data.

The explicit range's targets are scripted instances of the selected ported
aircraft, with source HP (116 Hornet; 100 Rafale). They do not return fire.
Designation refers to real target IDs; dead targets cannot lock or receive
another destruction. SEE range/FOV plus radar emission gate radar contacts and
radar-guided launch; JT launch/track zones gate guided weapons. The current
cone test, direct pursuit capped by source turn-rate fields, all-radar-weapons
illumination requirement and irreversible loss of track are approximations.
Native lead/PN, active/semi-active distinctions, signature strength, ground/air
eligibility, aspect/Doppler, terrain masking, sun, ECM and difficulty/RNG remain
open. AGM65G can engage the same range aircraft surrogate; this is not native
AGM65 target-class acceptance.

Swept relative-motion sphere intersection prevents round/target tunneling;
eight terrain samples plus bisection find the earliest sampled ground crossing.
This is not native polygon collision. Fuzes use source arm time/radius; damage
subtracts the source aircraft-class entry from source target HP. Native hit
probability, subsystem damage, immunity, collateral, debris and water effects
remain open. Ordinary free flight stays externally clean; explicit live range
loads the PT weapon counts and auxiliary external equipment mass. Tank fuel is
carried mass only, with no transfer/jettison. Released stores reduce payload
through the existing aircraft-owned model; rack pairing, weapon-specific drag
and carried-store rendering are not recovered.

See [live-fire validation](../baselines/live-fire.md) for controls, screenshots,
end-to-end results and presentation approximations. This is a working test range,
not a completed W3–W5 vanilla acceptance gate.
