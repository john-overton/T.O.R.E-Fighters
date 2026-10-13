# Aircraft weapons: FA research

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Research notes, research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Requirements, gates and remaining
> work described here are research-mode scope; they are not acceptance gates
> for gameplay. Parity is measured by expression of feature, see
> [AGENTS.md](../../AGENTS.md). Player-visible behaviour is specified in
> [docs/spec/](../spec/).


Research date: 2026-09-14. This schedules aircraft armament research and subsequent
importer/simulation work. It does not mark combat or vanilla parity complete.
[Measured extraction and code evidence](../baselines/weapons-research.md).
Every JT record's identity and headline fields are inventoried in
[the catalog](fa-catalog.md).

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
The explosion table, the crater and fire routines and the shapes' own
selection of these pictures and recordings are now traced
([explosion resources](explosions.md)). &CHAFF.5K and &FLARE.5K
are traced to device creation
([sound notes](sound.md#countermeasure-release-sound)).

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

## Implementation plan and status

The ordered W0–W5 implementation plan and its dated status log moved to
[the frozen weapons plan](../research/weapons-plan.md) on 2026-09-16. This file
keeps the recovered facts. Current sequencing is in
[the parity plan](../parity-plan.md); measured results are in
[weapons systems](../baselines/weapons-systems.md) and
[combat components](../baselines/combat-components.md).

## FA entry points for the next pass

SMS locations below are research starting points, not fully reviewed routines
or a complete call graph.

| Contract | Entry points |
| --- | --- |
| Loading/mount | HARDCanLoad 0x452980; HARDLoad 0x452c20; HARDPos 0x4532a0; HARDPodHack 0x453710; HARDStoreWeight 0x452940 |
| Fire/service | PROJAdd 0x4c0a90; PROJFire 0x4c2170; PROJServiceWeapon 0x4c4700; PROJProc 0x4c1f50 |
| Movement | PROJSpeed 0x4c1120; PROJEngineState 0x4c1170; PROJMoveProc 0x4c11b0; PROJMakeBombEq 0x4c3eb0; PROJBombPos 0x4c4050 ([read](#bomb-motion)) |
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
probability, subsystem damage, immunity, debris and water effects remain open;
collateral is now read (see [Collateral damage](#collateral-damage)). Ordinary free flight stays externally clean; explicit live range
loads the PT weapon counts and auxiliary external equipment mass. Tank fuel is
carried mass only, with no transfer/jettison. Released stores reduce payload
through the existing aircraft-owned model; rack pairing, weapon-specific drag
and carried-store rendering are not recovered.

See [live-fire validation](../baselines/live-fire.md) for controls, screenshots,
end-to-end results and presentation approximations. This is a working test range,
not a completed W3–W5 vanilla acceptance gate.

## Collateral damage

Static reading on 2026-10-10 (slice X1) of `PROJSendCollateralDamages`
(`0x4c5670`) in FA.EXE 1.02F
(`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`), from the
saved research disassembly; no original code ran. It is the source of the
[splash damage](../spec/missiles.md#splash-damage) behaviour.

Arguments (stdcall, six words): the projectile's type record, the burst
position, the projectile's own object index, the shooter's object index, one
more excluded object index (the struck target, or none), and a share in
percent.

- It does nothing when the record's `collateralDamagePercent` (record offset
  `0x139`) is 0. `collateralDamageRadius` is at `0x137`.
- It walks every object slot. It skips the projectile itself, the excluded
  index, inactive objects, objects of type 6 (projectiles) and objects whose
  type record lacks flag bit 0x1.
- Distance: the object's shape bounds give a half extent along the axis on
  which the burst is farthest from the object's centre; the routine takes the
  centre distance (fixed point, shifted right 8) minus that half extent,
  floored at 0. Objects at or beyond the radius are skipped.
- Sides: unless the shooter object carries bit 0x80 in byte `0x10`, the
  shooter itself is skipped, and an object whose side bit (byte 9, bit 0x80)
  equals the shooter's is skipped unless its object type byte is 4.
- Amount: `percent - percent * distance / radius` (integer division), times the
  share argument, divided by 100, written as a byte percent into a damage
  message with the projectile's index. A zero result sends nothing. The
  receiver applies it to the projectile's damage for the target's class, as a
  direct hit's percent is (the direct-hit routine at `0x4c58a0` sends 100, or
  33 to 98 on a roll, in the same byte).
- The message is posted with a delay argument of `distance / 500 + 2`.

Callers and their share argument:

| Call | Context | Share | Excluded |
| --- | --- | ---: | --- |
| `0x4c161c` | After an object-type explosion (record byte `0x55`) at the burst point | 100 | none |
| `0x4c1dbc` | Ground impact: crater (record byte `0x56`) and land or water explosion (`0x12d`, `0x12e`) | 70 if the record's percent is 100, else 50 | none |
| `0x4c1e3a` | A hit on an object | 100 | the struck object |
| `0x4c1f01` | A hit on an object whose type lacks flag 0x800 (objects with it end the call earlier, without splash) | 70 or 50 as above | the struck object |
| `0x4c5cf1` | Another routine (not traced) | 100 | none |

Unknown: what object type 4 and shooter flag 0x80 are; which targets take the
`0x4c1e3a` path and which the `0x4c1f01` path; the delay's time unit.

## Bomb motion

Static reading on 2026-10-10 (slice R1, research mode) of FA.EXE 1.02F
(`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`) from the
saved disassembly, with routine names from the matching `FA.SMS`
(`e550a67e...`). No original code ran. The 1.0 build (`c7d2c1cc...`) has the
same arithmetic at shifted addresses (`PROJBombPos` at `0x4c03f0`, compiled
with different registers) and the same scatter constants. Addresses below are
1.02F.

Units: positions are 1/256 ft; `_currentTicks` (`0x552928`) counts 256 per
second ([time notes](weather.md#continuous-time-is-confirmed)); `_currentT`
(`0x5528c8`) counts quarter seconds; angles are 65,536 per turn (the sine table
at `0x515a48` reaches 32,767 at index 64, a quarter turn; pitch clamps at
plus or minus `0x3ffc`). JT values are read from the record copy at `0x50d268`
(projectile flags at `0x50d30e`, record `+0xa6`).

### Bombs do not use the gravity flag

A record with the bomb flag 0x10 never needs flag 0x4. The bomb flag itself
selects a separate closed-form ballistic path:

1. The projectile's creation message (proc table `0x4c1f50`, message 1 returns
   `0x4c1f90`) tests the record's flag 0x10 and, for a bomb, builds a "bomb
   equation" with `PROJMakeBombEq` (`0x4c3eb0`, called at `0x4c2055`), copies it
   into a 0x24-byte block held by the instance at `+0x108` (`0x50cf88`;
   `0x4c205a` to `0x4c207b`) and sets the object's motion type to 10
   (`0x4c2081` to `0x4c20b3`).
2. `MoveObj` (`0x436b30`) dispatches on the motion type (jump table `0x438228`;
   entry 10 is `0x436fc3`). Type 10 sets the position from
   `PROJBombPos(equation, _currentTicks - releaseTick)` (`0x436fc3` to
   `0x436fd8`) every service and skips the speed and turn steps.
3. The flag 0x4 sink at `0x4c14cf` (the missile sag in
   [missile notes](missiles.md#release-sag-and-cruise-profile)) only changes
   the altitude that type 10 overwrites, so it would not matter for a bomb.

The equation block (`0x4c3eb0`):

| Offset | Meaning |
| --- | --- |
| `+0x00` | Release position x, y (up), z |
| `+0x0c` | Impact point x, y, z |
| `+0x18` | Release tick (`_currentTicks`) |
| `+0x1c` | Impact tick |
| `+0x20` | Horizontal speed, ft/s (word) |
| `+0x22` | Vertical speed, ft/s, up positive (word) |

`PROJMakeBombEq(equation, record, position, angle, speed, altitude, passes)`,
stdcall, record unused:

- Speeds: horizontal = speed x cos(pitch), vertical = speed x sin(pitch), both
  divided by 32,767 (`_GRSinCos` `0x4d5c98`, sine table `0x515a48`, cosine
  `0x515ac8`; `0x4c3ec7` to `0x4c3f18`).
- Ground: an altitude argument of `0x7fffffff` asks `T_Info` (`0x4abab0`) for
  the ground under the release position (`0x4c3f20` to `0x4c3f43`).
- Fall time: `(vz + sqrt(vz^2 + 64 h)) / 32` seconds, where h is the release
  altitude above that ground in feet (`0x4c3f48` to `0x4c3f87`), stored in
  ticks. This is a fall under **32 ft/s² with no drag**.
- Impact: the horizontal speed times the fall time, rotated to the release
  heading (`_Rotate2` `0x4c6654`) and added to the release position
  (`0x4c3f8a` to `0x4c3fc0`).
- Refinement: with passes left, it samples the ground under the predicted
  impact and, when that differs by 5 ft (`0x500`) or more, jumps back to
  `0x4c3f20` (`0x4c3fd0` to `0x4c4009`). That label reads the ground under the
  release position again, so the stored ground is always the release point's.
  The bomb's creation, the CCIP sight and the HAB window ask for 4 passes;
  the AI release check asks for none.

`PROJBombPos(equation, t, out)` (`0x4c4050`), t in ticks since release:

- x and z move linearly from the release position to the impact point:
  `p0 + (p1 - p0) * t / (T + 1)`, T the fall time in ticks (`0x4c4064` to
  `0x4c40c2`). Past T they keep going at the same rate.
- y is `y0 + vz * t - t^2 / 16` in 1/256 ft, that is `y0 + vz * s - 16 s^2`
  feet after s seconds (`0x4c40c5` to `0x4c40ef`).

So the path is a vacuum parabola, g = 32 ft/s², with no drag, no terminal
speed and no retard after release. The real bomb hits whatever terrain or
object the normal collision step finds; the predicted ground only sets the
horizontal rate.

Orientation: after each position, `MoveObj` sets pitch to
`ArcTan(vz - 32 s, vh)` (`_ArcTan` `0x4ccb88`; `0x436fdd` to `0x43703b`),
clamped to plus or minus `0x3ffc`, in both the motion and attitude words. The
drawn bomb noses over along its velocity; heading stays at the release heading.

### Release state

The player trigger and the AI both release through `PROJFire` (`0x4c2170`):

- **Position**: `HARDPos` (`0x4532a0`) puts the store at the station's offset
  rotated with the aircraft (`0x411d40`) when `PROJAccurateHardPos`
  (`0x4c0820`) says so: a human launcher, one of the two objects at `0x4eb64c`
  and `0x4eb64e`, or one within 5,000 ft of the point `0x4eb650`. Any other
  launcher releases from its own centre (`0x4532f0` to `0x453321`). The
  lateral offset is mirrored when the hardpoint type has flag 0x4 and the
  station's remaining count divided by `actualRoundsPerGame` is odd (`HARDUnrotatedHardPos` `0x453220` to
  `0x453291`), so successive releases alternate sides of a paired station.
  `PROJAdd` raises a launch altitude under 10 ft to 10 ft (`0x4c0b30` to
  `0x4c0b47`).
  Whether the count is read before or after the release's own debit is
  unknown (the next step is a read of `PROJFire`'s debit against its
  `HARDPos` call). The positions per aircraft are in
  [hardpoint positions](aircraft-ordnance.md#hardpoint-positions); how T.O.R.E
  uses them is in [stores on pylons](../spec/stores-on-pylons.md).
- **Direction**: the aircraft's attitude (`0x50ce9d`, heading and pitch, not
  its direction of motion at `0x50cea3`) plus the station's slew angles
  (`0x45335e` to `0x45337d`, or a full composition at `0x453383` for an
  accurate launcher as above or one banked or pitched past `0x11c6`, 25
  degrees), pitch clamped to plus or minus `0x3ffc` (`0x453398`). Bombs
  leave **along the nose, not the flight path**. Every bomb station on the 27
  imported A2G carriers has zero slew and zero slew limits (PT `:hards`
  blocks), so the turret aim at `0x4c24b0` (stations with a nonzero
  `slewLimitH` or `slewLimitP`, `0x4c24e0` to `0x4c24ec`) never applies to them.
- **Speed**: `PROJSpeed` (`0x4c1120`, called from `PROJAdd` at `0x4c0d76`), a
  scalar: `max(launcherSpeed * launchRetard / 100, initialSpeed)`, clamped to
  the record's `_minSpeed` (`+0x67`) and `_maxSpeed` (`+0x6b`). Bomb records:
  `initialSpeed` 0, clamp 0 to 1,466 ft/s (slick, retarded, packs; Boulder and
  Moose 1,026), 0 to 2,933 ft/s (cluster).
- **Scatter**: the creation message adds `Rand(2A) - A` to the heading and to
  the pitch, with A the words of `adjustBombAngle` (`0x50ce08` = 0x222, 0x222):
  546 units, **plus or minus 3.0 degrees**, uniform, independently per bomb
  (`0x4c1fc0` to `0x4c200a`; `Rand` `0x4562f0` returns 0 to n-1).
  `adjustBombRandom` (`0x50ce10` = 1) has no reader in the listing.
- No ejection push: bomb records lack flag 0x8 (the 32 ft/s kick at
  `0x4c0e08` to `0x4c0e27`).

Retard: `launchRetard` (66 on MK82HD, MK82P and MK82AIR; 90 on the five
cluster records; 100 on slick bombs) only scales the release speed. A Snakeye
leaves at 66 percent of the aircraft's speed and then falls on the same
parabola. Flag 0x40 (on the cluster records, with `finalSpeed` 278) makes the
projectile step write a speed command (`0x4c1494` to `0x4c14ca`), which motion
type 10 never reads: it does not change a bomb's path.

### Packs and pods

- **Per call**: `PROJFire` makes `gameRoundsInBurst` projectiles when the
  record is not a gun (flag 0x80), has `projsInPod` 1 and a burst above 1;
  otherwise one (`0x4c22a7` to `0x4c22c9`). Ammunition is debited once per
  call, `actualRoundsPerGame` (`+0xf0`) through `HARDUnload` (`0x4527f0`,
  called at `0x4c224e`). A MK-82 3-, 6- or 7-pack therefore releases all its
  bombs in the same tick for one round.
- **Pack layout**: bomb i of a pack (i from 0) is offset from the station
  cumulatively by `(even i ? +i : -i) * 2 ft` in world x and `-2 i ft` in
  world y (`0x4c22db` to `0x4c2308`, `0x4c2475`): x 0, -2, +2, -4, +4, -6,
  +6 ft and y 0, -2, -6, -12, -20, -30, -42 ft. Each bomb gets its own
  scatter.
- **Pods**: at mission start `HARDPodHack(1)` (`0x453710`, called at
  `0x4043c4`) multiplies each loaded station's count by `projsInPod`; mission
  end divides it back (`0x404752`). Each `PROJFire` then fires one rocket and
  debits one. Two LAU-61 pods hold 38 rockets.
- **Player cadence** (`ServicePlayer`, `0x416f17` to `0x417027`): a record with
  flag 0x800 fires while the trigger is held, one call every `gameBurstT`
  quarter seconds (`0x416f3b` to `0x416f74`); any other record fires once per
  new press. LAU-61, B-8M1 and B-13L (0x800, `gameBurstT` 1) fire 4 rockets a
  second held; LAU-10 (0x20046, no 0x800) one per press. No bomb record has
  0x800, so `gameBurstT` never spaces a player's bombs.
- **AI cadence** (`PROJServiceWeapon`, `0x4c4d77` to `0x4c4fcd`): a burst of
  `gameRoundsInBurst` calls (the larger of it and `gameRoundsInCarpetBurst`
  when the launcher's type dword `+0xa6` has bit 0x2), `gameBurstT` quarter
  seconds apart, then `reloadT` quarter seconds, both randomised by record
  flags 0x80000 and 0x100000.

`gameBurstT` and `reloadT` are quarter seconds in both paths.

### Rocket fall

Rockets carry flag 0x4: the sink grows by `(dt << 13) >> 8` fixed-8 ft/s per
service with dt from `0x546ba0` (`0x4c14e4` to `0x4c14fc`), which is ticks
since the object's last service
([service notes](native-strip.md#load-service-requeue-and-merge)). That
confirms 32 ft/s² for the sink and its 80 ft/s cap, while unlit or burnt out.

### AI bombs

For an AI launcher (object byte `+0x10` without the human bit 0x80) with a
target, `PROJFire` replaces each bomb's impact point with the target's position
plus `Rand(200) - 100` ft in x and in z (`0x4c23c0` to `0x4c2470`, through
`PROJChangeBombEq` `0x4c4030`). The bomb's horizontal path then runs straight
to that point while it falls on its own parabola. When the launcher's type
dword `+0xa6` has bit 0x2 this happens 25 percent of the time (`Percent(75)`
skips it); among PT records only `~QUE` sets that bit. AI release itself waits
for `PROJLock`'s bomb check (`0x4c3163` to `0x4c31d9`): an equation from the
aircraft's own position, attitude and raw speed (no `launchRetard`) against
the target's altitude, accepted when the predicted impact is within the larger
of 1,000 ft and an eighth of the target distance.

### AI release against ground targets

Static read on 2026-10-11 (AI strike round, slice ST-R), same build; weapon
numbers from the FA_2.LIB JT records (zone 1 at JT `+0xcf`: horizontal and
vertical half-angles, minimum and maximum range, altitude limits). Every AI
shot passes these gates in `_PROJServiceWeapon` (`0x4c4700`), in order:

| Gate | Source | Rule |
| --- | --- | --- |
| Store class | `0x4c52f2..0x4c535c` | A target without class bits 0xc000 (not an aircraft) takes only stores flagged 0x20000: bombs, rockets, air-to-ground missiles, HARMs. Guns carry both class flags |
| Mount aim | `0x4c24b0..0x4c26e1` (called `0x4c48a4`, `0x4c4ae8`, `0x4c543f`, `0x4c21ea`), `_LimitAngle` `0x412da0` | A moving mount slews toward the led target within its neutral heading and pitch plus or minus arcH and arcP (half-widths); arcs 0 mean a fixed mount, as on every missile and rocket station. A human launcher's mount slews only with hardpoint flag bit 0x8 |
| Zone 1 | `_PROJLock` `0x4c3121..0x4c3155`, `_PROJInFOV` `0x4c2860..0x4c2b4d` | Angles about the mount's current direction, altitude difference, distance between minimum and maximum, and signature-scaled distance (distance x 100 / `COSig` percent) under the maximum. Bombs add the impact check of [AI bombs](#ai-bombs) |
| Radar AGM | `0x4c2fbc..0x4c3011`, `0x4c31f0` | A radar-seeker surface store needs a launcher with instance `+0xde` bit 0x400 (read as "radar fitted", inferred); launching keeps that launcher emitting for 10 s |
| Terrain line | `0x4c4b1a..0x4c4b3d` | Blocked: retry after 1 to 10 s (4 + 4 x Rand(10) quarter seconds) |
| One guided round at a time | `0x4c4b50`, `0x4c5270..0x4c52cd` | No new guided launch while this launcher has a live guided round on the same target; retry after 1 s |
| Half range for guided | `0x4c4b88..0x4c4bad` | Guided rounds fire only inside 10,000 ft or half the zone-1 maximum |
| Unpowered guided | `0x4c4bbb..0x4c4c04` | Where JT word `+0x101` is 0 (GBU-10, GBU-29; 40 on AGM-65G and AT-2, 100 on AGM-88; reading it as motor time is inferred): the target must be more than 25 degrees below the horizon, or within 5,280 ft while the launcher flies at 586 ft/s (347 kt) or more |
| Aim tolerance | `0x4c4c09..0x4c4c8e` | Heading and pitch error from the mount direction to the target at most JT byte `+0xfa` x 256 angle units, doubled inside 500 ft: guns and rockets 29.5 degrees, AGM-65 and GBU-10 43.6, Mk 82 88.6 |
| Gun and rocket lead | `_PROJGetTargetPos` `0x4c0710..0x4c0816` | Flag 0x4000 inside 20,000 ft: lead along the target's velocity by distance over the round's maximum speed, aim 20 ft above a non-aircraft target, no drop term |

Numbers a player notices: AGM-65G zone 1 is plus or minus 45 degrees, 500 to
60,000 ft, so the AI fires inside 30,000 ft (AGM-65A 0 to 50,000 ft); LAU-10 and
LAU-61 rockets plus or minus 45 degrees, 50 to 12,000 ft, released by zone and
aim only, with no impact prediction; M61 and GSh-30 0 to 7,500 ft; the AC-130's
C_25, C_40 and C_105 0 to 13,000 ft with an IR signature scale.

**HARM.** AGM-88 and AGM-45 are class 0x20000, so an AI may choose them against
any surface target, but the lock passes only while the target emits: `COSig`
(`0x478213..0x478242`) gives signature 4 a value of 100 percent only for object
kinds 2 and 4 (kind 2 read as a surface unit, inferred) whose `+0x11a` emitter
deadline is in the future or whose `+0xde` bit 0x10 is set (producer open), and
0 otherwise. A radar object's target search keeps it emitting for 2 s
(`0x4c41c4..0x4c41f8`); a radar-weapon lock for 10 s (`PROJRadarIsOn`
`0x4c2eb0`). In flight a target slower than 36 ft/s counts as emitting
(`0x4c1316..0x4c136a`), so a HARM keeps homing on a site that went quiet. AI
fire range: AGM-88 500 to 200,000 ft, fired inside 100,000 ft; AGM-45 0 to
60,000 ft, inside 30,000 ft; plus or minus 45 degrees. Store scoring
(`0x4c5444..0x4c5530`) does not test emission, so a HARM can be chosen against
a silent site and then wait at the lock.

**Guided bombs without guidance.** With no target, `0x4c1720` holds the current
heading, pitch and bank and the 0x40 command slows the bomb toward `finalSpeed`
278 ft/s (`0x4c149b..0x4c14ca`; deceleration field 14, unit open). GBU OBJ flags
0x3006 enable two `MoveObj` terms: bit 0x2000 changes speed by about
-32 x sin(path pitch) ft/s² (`0x437cf9..0x437d9d`), and bit 0x1000 floors the
requested path pitch at -20 degrees once speed is under 293 ft/s or the object
is on the ground, latched for a projectile by instance flag 0x40
(`0x4377f6..0x437846`). So a blind GBU keeps its release attitude while it slows,
then glides at 20 degrees to impact or the end of its life; a guided GBU that
slows below 293 ft/s is also held at that glide and steers in heading only.

### Cluster flag 0x800000

The only projectile-record test of 0x800000 is in `GRAPHICAddExp` (`0x443321`
to `0x443336`). For an explosion caused by such a projectile while the session
count `0x4eb604` is 1, the effect object is created as type 43
(`GRAPHICAddInvisible`'s type, `0x4440f0`) with a 1 second life and no debris
unless the burst's y position in 1/256 ft is a multiple of 11 (`0x4434e6` to
`0x44350d`, `0x4435d4` to `0x4435e9`). The explosion sound request is made
either way (`0x44354d` onward). Damage and splash are unaffected.

### Facts for B1 (plan section 7.1)

| Fact | Retail | Status |
| --- | --- | --- |
| Release velocity | `PROJSpeed` scalar along the aircraft's attitude plus station slew (zero on every imported bomb station), plus a uniform plus or minus 3.0 degree heading and pitch scatter per bomb; no ejection push | Closed |
| Gravity | 32 ft/s², closed form, no cap | Closed |
| Drag | None; `launchRetard` only scales the release speed; flag 0x40 does not affect a bomb | Closed |
| Orientation | Pitch follows the velocity; heading fixed | Closed |
| Packs | One press releases the whole pack in one tick, staggered 2 ft steps as above, one round debited | Closed |
| Pods | `projsInPod` rockets per pod; one rocket per call; held trigger repeats every `gameBurstT` quarter seconds for 0x800 records | Closed |
| Predictor | The same equation and release state without the scatter, against the ground under the release point (see [Bomb sight](#bomb-sight); TORE's pipper follows the path to the terrain instead, `predict_impact_flat` keeps this rule) | Closed; the `T_Info` cache detail is unknown below |

### Unknown

| Question | Next step | Fallback |
| --- | --- | --- |
| Whether `T_Info`'s per-object cached height (object `+0x27`, `+0x2f`) makes the release-point ground differ from the ground exactly below the station | Trace `0x4abab0`'s cache writers | Ground directly below the release point (fitted) |
| What object type 43 draws, so what a cluster burst looks like in single player | Trace type 43 in the shape and effect draw code | Decision 13's presentation-only flashes (opinionated, agent) |
| Meaning of launcher type bit 0x2 (`+0xa6`) | Read the NPC flag consumers | None needed: no imported aircraft sets it |
| World axis of the pack stagger relative to north | Read the theater axis convention | Use the store's own lateral axis (fitted) |
| How a guided bomb falls | Closed 2026-10-11: [AI release against ground targets](#ai-release-against-ground-targets), guided bombs without guidance | The 20 degree glide below 293 ft/s (retail) |

## Bomb sight

Same build and method as [Bomb motion](#bomb-motion).

### When the sight shows

The HUD mode byte (`0x521694`) is set at `0x406e2a` to `0x406f6c`: a selected
bomb-flag store gives mode 3 (CCIP) when HUD module byte `+0x239` is set, mode
6 (HAB) when only `+0x23a` is set, otherwise 5 (ARM). Dispatch at `0x4071a8`
(table `0x4075b0`) calls the CCIP routine `0x409760` for mode 3; mode 6 draws
no HUD sight. Rockets lack flag 0x10 and show ARM. The MIG21, MIG17, YAK141,
F14, F104, SEAHAR and SU33 modules set neither byte (their `CC` or `_C` twins
set `+0x239`); B2 and BMBROH set only `+0x23a`.

### The prediction

`0x409760`, every HUD frame:

1. Station `0x50cf9c`, its record and `HARDPos` (`0x4097b2`): the same release
   position and direction as a real release.
2. `PROJSpeed(record, own speed)` (`0x4097c6`) and `PROJMakeBombEq(..., ground,
   4)` (`0x4097dc`): the same equation, without the random scatter. The pipper
   is the mean impact point, over the ground below the release point (retail
   takes that one height for the whole fall; TORE's pipper instead follows the
   path to where it meets the terrain, opinionated John 2026-10-11, see
   [the CCIP pipper](../spec/air-to-ground.md#ccip-the-dive-pipper)).
3. The impact point is projected (`_GRTo2d` `0x4d5cc0`, `0x4097f3`). Behind or
   off the screen nothing more is drawn except trail dots that land on screen.
4. The symbol font is selected (`HUDSetSymFont` `0x40ab30`: handle at `_hud +
   0x5e`, `0x5213be`, the module's `hudsym` font named at `_hud + 0x51`), and
   restored after (`0x40ab10`).

### Fall line or trail

- Without HUD byte `+0x23a`: a dotted line, `G_PatLine` (`0x498380`) with
  pattern `0x55555555`, from the HUD reference point (`0x521060`) to the
  pipper (`0x409858` to `0x409881`).
- With `+0x23a` (on imported aircraft: F-16C, F-22, F-22N and X-31): instead a
  dot (`G_Point` `0x4976d0`) at `PROJBombPos` every 64 ticks, a quarter second
  of fall, from release to impact (`0x409807` to `0x409856`).

The reference point `0x521060` (`0x407026` to `0x40719e`) is the flight path
marker when HUD byte `+0x23b` is set and own speed is at least the HUD word
`+0x23e` in ft/s: a point 10,000 ft (`0x271000`) along the velocity, drawn with
symbol glyph 6. Otherwise it is the boresight point `0x521d94` (10,000 ft along
the nose, `0x406bb5` to `0x406c58`; symbol glyph 5). Among the retail HUD
modules only the X-31 family (`F31*.HUD`) lacks `+0x23b`; the minimum speed is
146 ft/s in the AV8, F4, MIG17, MIG21 and YAK141 modules (and the ASTOVL, F8,
SEAHAR and novelty ones) and 0 in the others.

### The pipper and its arc

One glyph of the symbol font, printed with `G_Print` flags 5 (centred
horizontally and vertically, `0x4986cd` to `0x4986f0`) at the impact point
(`0x4098e2` to `0x4098f3`). The glyph is chosen at `0x409886` to `0x4098dd`:

- d is `_Dist` (`0x4c66cc`: the largest axis difference plus a quarter of each
  of the other two) from the aircraft's position (`0x50ce91`) to the impact
  point. R is the record's `zone1.maxRange` (`+0xd7`, feet scaled by 256, so
  the comparison is in feet).
- d at most 0.4 R (computed as `(R - R/5) / 2`): glyph `0x2f`.
- 0.4 R to R: glyph `0x2e + trunc(14 * (0.4 R - d) / (0.6 R))`, so `0x2e` down
  to `0x21`.
- d at least R: glyph `0x20`.

The glyphs (HUDSYM11, height 31, advance 29, rendered locally to
`.local/a2g/renders/ccip-glyphs.png`, local only): `0x20` is a 23-pixel ring with twelve
clock marks, the four cardinal ones reaching the 29-pixel cell edge, and a
centre dot at (14, 15), so the centre dot sits on the impact point. `0x21` to
`0x2f` add an inner 17-pixel arc that starts at 12 o'clock and grows clockwise;
`0x2f` closes it. The arc winds closed as the range shrinks and the bare
pipper remains at and beyond R; nothing is blank while the impact is on screen.

R by record: 20,000 ft for MK-82 family and MK-84 (full arc inside 8,000 ft),
10,000 ft for FAB (4,000 ft), 30,000 ft for cluster (12,000 ft), 20,000 ft for
Boulder and Moose.

The manual (p. 85) says the arc is a complete circle at 10,000 ft or more and a
half circle at 5,000 ft, the opposite sense; the code is the evidence for the
original's behaviour. The routine prints no text.

`ComputeBombPosition` (`0x40ad40`) serves the HAB bombing window: the same
equation from the aircraft's own position and attitude, returning the camera
pitch toward the impact; `bombRange` (`0x539d6c`) is its zoom step, 0 to 5
(`CPBombRange` `0x43e7e0`). No imported aircraft has a HAB-only HUD.

### Unknown

| Question | Next step | Fallback |
| --- | --- | --- |
| Pixel rhythm of pattern `0x55555555` | Read the line core `0x4c619c` | One pixel on, one off (fitted) |
| Which symbol font file serves each display resolution | Trace the HUD font loader's name suffix | `HUDSYM11.FNT`, matching the `HUD11.FNT` text font TORE uses |

## Manual weapons integration follow-up

[Manual acceptance](../baselines/manual-weapons.md) supersedes the live adapter's
previous all-radar illumination rule, class-0-only fixture path, absent carried
geometry and combat-recording gap. The source category switch at 0x411470 is
translated; both PT categories are 0x8000 -> damage index 0. Five-class fixtures,
bounded nominal/applied hit history, failed-station high-bit semantics, shared
visual/radar acquisition, sampled terrain masking and manual jettison are wired.
R530 retains launcher radar during tracking; AIM120/MICA do not. Native activation,
lead/PN and whole-tick native parity remain unaccepted. The systems continuation
below supersedes this checkpoint’s open ECM/automatic-selection work with partial
translations; native RNG and complete subsystem effects remain open.
At this checkpoint static extraction emitted 22 reviewed regions, including category, amount and
station-failure spans; a reviewed span is not a complete native translation.
All 135 catalog JT files remain preserved; live acceptance covers the ten default
PT stations of the two ported identities, not alternative compatible loadouts.

## Player systems continuation

The static pass now includes 28 reviewed regions, including player capacity,
weighted subsystem selection, eligibility, ECM lookup/probability and equipment
damage. Runtime resolves both PT systemDamage tables and ECM once at startup.
[Exact translated contracts, integration and open native gates](../baselines/weapons-systems.md).
The 119 located symbol spans are not 119 fully translated routines.

The reviewed JT `si_names` block contains three strings: short label, long
description and resource identity. The HUD preserves the first independently
of menu descriptions. Local FA_2.LIB records give `AIM-120` / `AIM-120 AMRAAM`,
`AIM-9M` / `AIM-9M Sidewinder`, and `AGM-65` / `AGM-65G Maverick`.
This establishes resource labels, not proof of the original HUD's string lookup.
[Source identity and checks](../baselines/hud-cleanup.md).
