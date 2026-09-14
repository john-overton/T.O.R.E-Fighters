# Two-aircraft development live-fire baseline

Follow-up: [manual weapons integration](manual-weapons.md) supersedes the earlier
class-0-only fixtures, all-radar tracking dependency, missing carried geometry
and combat-service recording gaps described below. This document preserves the
first-pass acceptance evidence.

2026-09-14, Linux / NVIDIA RTX 4070 / Vulkan, Rust 1.91.1. This implements the
subsequent user request for a working live-fire pass with clearly documented
approximations. It is not completion of vanilla combat acceptance.

## Aircraft and source loadouts

The registry `AircraftId::ALL`, `tore-sim::models::{f18,rafale_c}`, the app's
reviewed SH/cockpit rigs and each PT independently agree on these identities:

| Ported identity | Native data/art | Weapon slots in PT order | Target HP |
| --- | --- | --- | --- |
| F/A-18D | F18.PT, F18.SH, ~F18H.PIC | M61 570; AIM120 2; AGM65G 4; AGM65G 4; AIM9M 2 | 116 |
| Rafale C (source display name RAFALE) | RAFALE.PT, RAF.SH, ~RAFH.PIC | DEFA 250; AGM65G 4; MICA 2; R530 2; R550 2 | 100 |

Counts and mounts are parsed from those PT slots; movement, damage, burst and
sensor values come from the referenced JT files. No F18C, RAFALEE, RAFALEF or
other aircraft is substituted. Both source aircraft explicitly name F18R.SEE;
using it for Rafale does not silently borrow Hornet equipment. Auxiliary external
PT equipment is F150.GAS on the Hornet and AAS38.SEE on Rafale. Their source mass
is included in range payload; tank fuel transfer and jettison remain unimplemented.
All other catalog imports from the preceding work are preserved, without enabling
additional aircraft or arbitrary loadouts.

## Run and controls

```sh
cargo run --locked -p tore-app -- --live-fire --aircraft f18
cargo run --locked -p tore-app -- --live-fire --aircraft rafale
cargo run --locked -p tore-app -- --combat-smoke --aircraft f18
cargo run --locked -p tore-app -- --combat-smoke --aircraft rafale
```

- Space holds the selected trigger; release stops new firing. Modifier combinations
  cannot fire. Pause, menu, focus loss, resize and aircraft/restart transitions
  cancel held fire. A held key must physically release after interruption.
- Semicolon cycles the aircraft's five PT weapon slots. Empty slots remain empty.
- Backslash resets the explicit range target at a range suited to the selected
  weapon's source minimum range. It does not replenish ammunition.
- T or Enter designates the actual scripted target. Radar-guided stores require R
  to be on and source radar/weapon range/FOV checks to pass. Guns are visual/unguided.
- Restart resets ammo, projectiles, damage and range target. These bindings are
  development controls; the complete native FA dispatch remains unverified.

For a missile after starting with guns: semicolon selects the next slot,
backslash resets target range, T designates, then press/release Space. A minimum
range rejection consumes no ammunition. `--weapon-slot N` selects a 1-based slot
at startup. `--live-fire` deliberately creates one scripted target of the same
ported aircraft, flying straight at 300 ft/s; ordinary free flight creates none
and leaves external stores clean, while its internal gun can fire.

## Completed connected capabilities

The app advances the combat lifecycle inside the same authoritative 120 Hz loop
as flight. Typed source configuration is constructed once. Mutable ammo, target
IDs, trigger deadlines, projectiles, HP and effects remain combat state. The live
adapter explicitly reuses recovered launch speed, engine phases, axial speed,
altitude performance, gravity fall, lifetime and ammo-debit helpers.

Player fire spawns actual projectiles, debits actual station ammunition, advances
them independently of the launcher, guides applicable stores, finds swept target
or sampled terrain hits, applies source class-0 damage and destroys targets once.
Destroyed targets stop rendering and cannot be locked or damaged again. Store
release reduces payload through the aircraft's existing model API. Damage,
projectile and effect pools are bounded (256 projectiles, 64 effects); pool
saturation rejects a spawn before ammunition is consumed.

The target uses the selected aircraft's original mesh and atlas. Missile shapes
use bounded SH static geometry with palette colors; their textures and native
scale/mount transforms are not fully recovered. The current fitted transform uses
one-third-foot source coordinates. BULLET.SH uses unimplemented line/point
branches, so rounds have an authored thin tracer strip along actual swept motion.
Depth-tested impacts use the original AIRLRG.PIC sheet: visually fitted 3×4 cells,
20×20 sampling, 80×58 source cells, fitted size and 45/240-tick hit/destruction
lifetimes. This is not native explosion table/index/animation parity. No generated
replacement retail art is committed.

Firing events select each JT's original `fireSound`; hit/destruction events use
imported &EXPL3.5K / &EXPL12.5K through the bounded, pause-aware PCM mixer. Hit-sound
mapping, lack of spatial attenuation and burst grouping remain authored. Rumble
is emitted by successful gun/missile launch events, not raw key presses. The smoke
checks referenced PCM decoding and nonconstant firing signals; manual audible
playback and physical rumble acceptance were not performed.

Weapon and target instruments now show live ammo, actual target HP, visual/lock
state, and source-gated radar contacts. RWR stays empty because the range target
has no implemented threatening emissions. Systems displays actual stores mass
instead of a constant external-fuel placeholder. Full sensor modes and missile
camera pages remain open. Controller fire bindings and combat input recording are
not implemented; the existing flight-only recording format rejects live range.

## Fidelity limits

[Detailed arithmetic and approximation contract](../formats/weapons.md#development-live-fire-adapter-subsequent-implementation).
The user authorized approximations for this live pass; none is labeled native
parity. Major open contracts are:

- Native scheduler, representative burst/reload/spread ordering and RNG draws.
- Native guidance/lead/PN, active versus semi-active radar, signatures, target-class
  eligibility, Doppler/aspect, terrain masking, ECM, sun and reacquisition. Current
  guidance is rate-limited direct pursuit through source zones; all radar-guided
  weapons conservatively require player radar throughout. AGM65G engages the
  aircraft range surrogate without a verified native ground/air eligibility gate.
- Native collision geometry, hit probability, fuzes beyond arm-time/radius,
  subsystem damage, collateral, water response and debris. Contacts use a 28-ft
  target sphere plus source fuze radius, relative swept segments, and bounded
  terrain sampling/bisection. Narrow terrain features between samples can be missed.
- Native station pairing/racks, carried-store rendering, store-specific drag,
  tank fuel transfer, loadout UI, countermeasures, hostile AI and player combat
  damage. The target has no return-fire behavior.

## Acceptance evidence

Both imported app smoke suites pass every actual JT slot (10 station cases,
8 distinct definitions), running source flight plus the live combat host with
explicit range input. Every case observes ammunition consumption, spawning,
hits, exactly one destruction and a destruction effect, followed by release
without additional ammo debit. The suite stops at destruction; fixed-duration
captures can fire more rounds after the target is destroyed.

| Aircraft / slot | Representative shots | Hits | Destroyed | Ammo after |
| --- | ---: | ---: | ---: | ---: |
| F/A-18D M61 | 12 | 5 | 1 | 546 |
| F/A-18D AIM120 | 2 | 1 | 1 | 0 |
| F/A-18D AGM65G, each of two groups | 2 | 1 | 1 | 2 |
| F/A-18D AIM9M | 2 | 2 | 1 | 0 |
| Rafale C DEFA | 8 | 4 | 1 | 234 |
| Rafale C AGM65G | 2 | 1 | 1 | 2 |
| Rafale C MICA | 2 | 2 | 1 | 0 |
| Rafale C R530 | 2 | 1 | 1 | 0 |
| Rafale C R550 | 2 | 1 | 1 | 0 |

- 198 Rust tests and 14 Python tests pass. New focused tests cover release and
  modifier interruption, partial/empty ammo, expiry, moving swept contacts,
  terrain crossing, damage/destruction-once, lock rejection/loss, capacity and
  dead-launcher rejection, and deterministic replay at 30/60/144 presentation
  rates with a pause interval.
- Formatting, warnings-denied workspace Clippy and locked workspace build pass.
- Linux creator/viewer smokes, both aircraft gun captures, both guided-weapon
  captures and a tall Rafale capture pass and were visually inspected.
- The expanded cache exposed an inherited 128 MiB reload cap against a roughly
  132 MiB export. Reader and writer now share a bounded 256 MiB cap; repeated
  launches load the existing cache instead of silently importing again.
- Original-game differential acceptance, Windows/macOS runtime checks and manual
  physical input/audio acceptance remain unavailable. These results establish a
  working deterministic development range, not vanilla 1:1 performance.

## Representative captures

Retail-derived captures are local only under `.local/live-fire/`:

- `f18-gun.png`: Hornet target destroyed, ammo 546, 5 hits, original explosion art.
- `rafale-gun.png`: Rafale target destroyed, ammo 226 after 75 ticks, 4 hits.
- `f18-missile.png`: AIM-120 launch, ammo 1, target HP 116 and radar lock/contact.
- `rafale-missile.png`: MICA launch, ammo 1, target HP 100 and radar lock/contact.
- `rafale-tall.png`: 800×1200 window, actual target and anchored instruments.

Commands (capture files are PPM; PNGs are lossless conversions for inspection):

```sh
cargo run --locked -p tore-app -- --live-fire --aircraft f18 --combat-probe-ticks 75 --capture-flight .local/live-fire/f18-gun.ppm --window-size 1280x720 --no-audio
cargo run --locked -p tore-app -- --live-fire --aircraft rafale --combat-probe-ticks 75 --capture-flight .local/live-fire/rafale-gun.ppm --window-size 1280x720 --no-audio
cargo run --locked -p tore-app -- --live-fire --aircraft f18 --weapon-slot 2 --combat-probe-ticks 120 --capture-flight .local/live-fire/f18-missile.ppm --window-size 1280x720 --no-audio
cargo run --locked -p tore-app -- --live-fire --aircraft rafale --weapon-slot 3 --combat-probe-ticks 120 --capture-flight .local/live-fire/rafale-missile.ppm --window-size 1280x720 --no-audio
```

Probes run the configured tick count through the actual flight/combat path,
designate the explicit range target and hold the trigger, then pause for a
reproducible capture. They are distinct from normal player input and never run
implicitly during ordinary flight.

## Short active performance sample

Sequential 330-frame Linux/Vulkan runs at 1280×720, first 30 frames excluded,
`TORE_PERF_ACTIVE=1`, no audio. Zero paused frames in all runs. Live cases start
from the 75-tick gun probe with active projectiles and impact effects; this is
not a sustained maximum-pool stress test or a before/after native comparison.

| Case | Mean CPU frame interval | p95 | Simulation/cameras mean | Completed camera readbacks |
| --- | ---: | ---: | ---: | ---: |
| Clean F18 flight | 1.69 ms | 1.94 ms | 0.06 ms | 0 |
| F18 live projectile/effect pass | 2.13 ms | 2.37 ms | 0.10 ms | 0 |
| Rafale live pass with camera panel 3 | 2.03 ms | 2.35 ms | 0.13 ms | 7 |

All runs rendered 330 rear-mirror frames. These are CPU intervals including
presentation backpressure, not GPU time or verified displayed FPS. No blocking
live readback or post-render sleep was introduced. Logs are
`.local/live-fire/{clean-f18,live-f18,live-rafale}-performance.log`.
