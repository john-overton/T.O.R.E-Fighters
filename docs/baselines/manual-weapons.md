# Two-aircraft manual weapons integration

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


Follow-up: [weapons, ECM, player damage and controller integration](weapons-systems.md)
supersedes the open player-damage/controller statements below. Complete native
ECM/subsystem/sensor parity remains open; this page retains the earlier evidence.

Continuation of [live fire](live-fire.md), 2026-09-14. Scope remains F18.PT
(F/A-18D) and RAFALE.PT (Rafale C): ten PT-default JT stations, eight distinct
weapons. Imported compatible alternatives are a separate acceptance backlog;
no additional flyable aircraft or combat AI is enabled.

## Implementation sequence

1. Resolve readiness/inhibit reasons, separate launch acquisition from tracking,
   and repair target replacement identity and stale-track handling.
2. Resolve damage class from source object category, retain bounded per-hit
   amounts/cumulative damage, expose class fixtures and station failure testing.
3. Connect manual controls/instruments and carried source weapon geometry,
   jettison/payload updates, and deterministic command replay checks.
4. Exercise every default slot through positive and negative lifecycle cases;
   run workspace/Python/asset/GPU checks and capture both aircraft.

## Additional native evidence

Same FA executable/SMS hashes as [weapon research](../formats/weapons.md).
`0x411470..0x4114eb` maps the object category word at +0x0d to damage index:
0x40/0x200/0x800/0x1000 -> 4; 0x100 -> 2; 0x400 -> 3;
0x2000 -> 1; other categories -> 0 (including 0x80/0x4000/0x8000).
This is an exact category switch, not a bitwise membership test.
`DAMAGEDoHit 0x40f9b0..0x40f9e8` selects damage[class], applies the caller's
percentage, then a random 80..119 percent factor with integer divisions.
The live adapter retains explicit full-strength, unrandomized damage until the
caller/RNG/difficulty contract is recovered. It must not claim native hit amounts.

`PROJLock 0x4c2fbc..0x4c3011` gates signature-3 player launch on radar for
flag 0x10000; `0x4c3027..0x4c30e7` checks the launcher for flags 0x700,
with the radar dependency specifically under 0x200 and 0x10000. This supports
separating launch radar acquisition from illumination-dependent tracking;
it does not establish native active-seeker activation range or PN guidance.

`DAMAGEDoHit 0x4103f1..0x4104b7` locates a hardpoint, marks its ammo word
with 0x8000, and invokes HARDSetFlags for projectile/equipment damage.
Station failure can therefore inhibit a live station without deleting its mass.
Choosing which subsystem fails from a hit remains unverified; do not invent
HP-percentage engine/radar failures or represent fault injection as native rolls.

## Acceptance

Completed integrations, evidence and remaining native contracts are recorded below.
AI is deferred until the complete manual weapon acceptance pass; no AI work is
part of this change. Broader native parity contracts remain tracked explicitly.

## Connected behavior

The live adapter now distinguishes READY, SAFE, launcher loss, failed station,
empty ammo, projectile capacity, absent/destroyed target, radar power/coverage,
terrain masking, minimum/maximum range, altitude and seeker field-of-view.
A launch inhibit consumes no ammunition. Sensor lock can remain valid while the
master arm is safe or a station is failed; lock is not launch permission.

Both aircraft resolve their own VIS340.SEE and F18R.SEE at construction. Manual
cycling selects living contacts inside visual or radar coverage. Designation,
launch solution and post-launch tracking remain separate. Bounded sampled terrain
visibility gates contacts and launch, and can break missile tracking. This uses
the existing height-query contract; it is an authored visibility approximation,
not recovered native terrain masking, Doppler, aspect or sensor cadence.

All reviewed radar stores require radar acquisition at launch. R530's source
0x200 flag keeps the launcher-radar requirement during tracking; AIM120 and MICA
continue after radar-off. IR stores use their own JT acquisition/tracking zones.
Rate-limited direct pursuit remains fitted. No native active-seeker activation
range, PN/lead, lock timers, ECM or automatic reacquisition is claimed. A dead,
removed or out-of-track target emits a track-lost event and clears the missile's
ID. Replacing the range target clears old projectiles/effects, retains ammo and
history, and allocates a fresh target ID.

Both PT object categories are 0x8000, which the recovered switch maps to damage
class 0. Each hit records station, target, tick, class, nominal source amount,
actual HP removed and HP remaining. The 128-record history is bounded; target HP
retains cumulative damage even after old records are evicted. Lethal damage is
clamped to remaining HP and emits destruction once. Other damage classes are
explicit test fixtures using the same aircraft surrogate, not newly ported ground
objects or native target eligibility.

| Source JT | PT stations / initial ammo | Nominal damage classes 0,1,2,3,4 |
| --- | --- | --- |
| M61 | F18 slot 1 / 570 | 25, 2, 7, 7, 25 |
| AIM120 | F18 slot 2 / 2 | 140, 14, 42, 28, 140 |
| AGM65G | F18 slots 3,4 / 4 each; Rafale slot 2 / 4 | 200, 200, 200, 200, 200 |
| AIM9M | F18 slot 5 / 2 | 100, 10, 30, 20, 100 |
| DEFA | Rafale slot 1 / 250 | 25, 2, 7, 7, 25 |
| MICA | Rafale slot 3 / 2 | 80, 8, 24, 16, 80 |
| R530 | Rafale slot 4 / 2 | 100, 10, 30, 20, 100 |
| R550 | Rafale slot 5 / 2 | 100, 10, 30, 20, 100 |

Manual station-failure injection sets the recovered 0x8000 bit. It inhibits the
weapon while retaining the lower 15-bit round count and carried mass. Jettison
uses the recovered unload helper and preserves the failure bit; only restart
repairs it. Automatic hit-to-subsystem selection, engine/hydraulic/radar failure
coupling and player combat damage remain open. No healthy subsystem percentages
or HP-threshold failures are fabricated.

External station groups display one original body mesh at the recovered mount
with the existing fitted one-third-foot transform. The group remains visible
until empty; no guessed rack/pair offsets multiply it by ammo count. Texture-only
SH faces, including exhaust sheets, are omitted from the palette-only mesh path
rather than rendered as opaque black surfaces. Weapon texture animation, native
scale and racks remain open. Jettison removes the selected external weapon group
and its payload mass; internal guns cannot be jettisoned. Auxiliary tank/pod mass
remains aboard. Jettison disappearance is authored; no falling inert-store model
or native jettison timing is claimed.

HUD status and weapon/target instruments reflect actual ammo, readiness, source
class, last applied hit and remaining HP. Original firing/impact/explosion PCM,
explosion sheet and launch rumble remain event driven through the existing systems.
Track loss adds text feedback without inventing an alert-sound mapping.

## Manual controls and replay

Existing Space, semicolon, T/Enter, R and backslash behavior remains. Additional
**development** controls (not claimed native FA bindings):

- U toggles arm/safe; L clears designation.
- K jettisons the selected external weapon group in the explicit range.
- `]` cycles damage-class fixtures; `[` fails the selected station.
- Range commands cancel held fire; interruption requires physical release before
  a new press. Modifier combinations and paused-menu input cannot invoke them.

`--combat-command arm|jettison|clear|class|fail|next|target|designate` applies an
explicit setup command before a capture probe (maximum 32 commands).

```sh
cargo run --locked -p tore-app -- --live-fire --aircraft f18 --record-combat .local/manual-session.tape
cargo run --locked -p tore-app -- --aircraft f18 --theater UKR --replay-combat .local/manual-session.tape
```

The create-new tape stores commands and explicit launcher position/basis/speed,
radar/alive state and held trigger at combat-service boundaries. Full reset and
release events are included; pause creates no ticks. Recording ends on aircraft,
theater or mission exit. The header guards version, aircraft, theater and a
stable FNV fingerprint of imported resources. The fingerprint detects accidental
mismatches; it is not a cryptographic integrity guarantee. Reader bounds are
4096 bytes/line, 432000 records and 256 MiB, with finite numeric and orthonormal
basis validation. This is a deterministic **combat-service** replay: it does not
re-simulate pilot flight inputs, render the replay, or establish native replay or
cross-architecture floating-point parity. The existing pilot tape remains separate.

## Remaining non-AI acceptance gaps

This pass exercises **all ten PT-default weapon slots**, not all 135 catalog JT
files. Alternate HARDCanLoad-compatible loadouts, mission PTS presets, bombs,
rockets, laser/anti-radiation/cruise/cluster branches remain unaccepted and are not
silently enabled. None of those is present in the two reviewed PT default weapon
lists. Full loadout-screen work remains deferred.

Other open native contracts: whole-tick clock/order/RNG, representative burst
spread/reload semantics, sensor signature/aspect/Doppler/ECM and sun terms,
AGM air/ground eligibility, native collision geometry and hit probability,
percentage/random damage scaling, automatic subsystem selection, collateral,
unarmed contact details, water/crater/debris effects, carried textures/racks,
tank transfer/jettison and store-specific drag. A range target follows an explicit
constant-velocity fixture, with no decision-making or return fire. No combat AI
was added; future basic fly-forward AI remains deferred until manual acceptance.

## Validation results

Linux, NVIDIA RTX 4070 / Vulkan, pinned Rust 1.91.1:

- **210 Rust tests and 14 Python tests pass**; formatting, warnings-denied
  workspace Clippy and locked workspace build pass. Source and both executable
  asset guards pass. New tests cover category mapping/restoration, all damage
  classes, readiness, failed-station mass, target replacement, radar-off tracking,
  terrain masking, command cadence/pause, input isolation, malformed tape fields
  and texture-only exhaust exclusion.
- **50 imported lifecycle cases pass:** five default slots × five source damage
  entries × two aircraft. Each class-0 case reaches destruction; other classes
  verify source nominal damage, applied damage and cumulative HP through contact.
  Negative checks exercise safe/failed station, missing designation, exact
  minimum-range rejection, radar-off launch, source-specific post-launch
  tracking behavior and exact jettison mass/ammo for every applicable slot.
- **Ten serialized slot tapes pass** complete-state comparisons after live
  fire/release, after manual commands, and after reset. Independent 30/60/144 Hz tests
  include pause without tick catch-up. The two desktop capture recordings also
  replay headlessly: Hornet 75 ticks / 12 representative projectiles / 5 hits /
  1 kill / 546 gun rounds; Rafale 120 ticks / 1 MICA / 0 hits yet / 1 MICA left.
  Wrong-aircraft replay is rejected by the header guard.
- **26 source flight scenarios pass** through the shared `flight_suite` example
  on the extracted F18.PT and RAFALE.PT (the same suite invoked by the extractor's
  `--validate-flight`). No aircraft flight law was merged or retuned.
- Static `--native-weapons` extraction passes with **22 reviewed regions** and
  119 symbol spans. Runtime checks reuse the existing expanded cache; VIS340 and
  the additional typed source fields require no new retail assets or fallback
  metadata. Required weapons/art/audio remain in the shared export closure.
- Creator and viewer GPU smokes, six final flight captures and the live camera
  benchmark pass. Captures were visually inspected. A found opaque-exhaust
  rendering defect was fixed before the final exterior captures.

Logs and artifacts are ignored under `.local/manual-weapons/`. Final serialized
smoke tapes are in `tapes-v5/`; earlier tape directories retain intermediate
investigation evidence. `f18-smoke.log`, `rafale-smoke.log`, `flight-suite.log`,
`tests.log`, `python-tests.log`, `clippy.log`, `build.log`, `asset-*.log`,
`native.log` and `*-gpu.log` contain the corresponding results.

Original-game differential acceptance, Windows/macOS runtime checks, physical
controller fire/rumble and audible listening checks were not performed. Passing
these tests establishes this adapter's connected behavior, not vanilla parity.

### Final representative screenshots

All are local retail-derived captures, never committed:

| Path under `.local/manual-weapons/` | Visible evidence |
| --- | --- |
| `f18-gun.png` | Original explosion, target HP 0, ammo 546, final applied hit 16 |
| `rafale-mica.png` | MICA ammo 1, actual target HP/lock and radar contact |
| `f18-safe.png` | SAFE, AIM120 ammo 2 unchanged, sensor lock still available |
| `rafale-failed-tall.png` | Tall cockpit/instrument layout, STATION FAILED, MICA ammo 2 retained |
| `f18-stores.png` | Exterior station bodies and 7568 lb payload |
| `rafale-stores.png` | Exterior station bodies and 5092 lb payload, no opaque exhaust sheets |

PPM originals and per-capture logs share each basename. The first two also have
`.tape` and `-replay.log` files from the actual desktop host.

### Short performance sample

Sequential 330-frame runs, first 30 excluded, 1280×720, no audio, recording off,
`TORE_PERF_ACTIVE=1`; no concurrent intentional GPU workload. Each run has zero
paused frames and 330 rear-mirror renders.

| Case | Mean CPU frame interval | p95 | Simulation/cameras mean | Camera readbacks |
| --- | ---: | ---: | ---: | ---: |
| Clean F18 | 1.71 ms | 1.96 ms | 0.06 ms | 0 |
| F18 live gun/effects after 75-tick probe | 2.15 ms | 2.42 ms | 0.11 ms | 0 |
| Rafale MICA after 120 ticks, camera page 3 | 2.06 ms | 2.39 ms | 0.17 ms | 7 |

These measure CPU intervals including presentation backpressure, not GPU time,
verified displayed FPS, native performance parity or maximum-pool stress.
No blocking live GPU readback or post-render sleep was introduced. Logs:
`{clean-f18,live-f18,live-rafale-camera}-performance.log`.
