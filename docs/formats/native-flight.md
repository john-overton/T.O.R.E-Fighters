# Fighters Anthology native flight research

This is a static reverse-engineering pass against the local retail **FA** executable,
not a port of the reference application's custom flight engine. The Rust helper
translations are available in `tore-formats::flight_model`; the playable simulation
still uses the authored 120 Hz adapter. A full native flight tick is **not decoded**.
No imported executable, native module, or emulated native routine is run.

## Reproduce

From the repository root, with Python 3.10+, the pinned Rust toolchain and LLVM
`objdump` (included in macOS Command Line Tools):

```sh
python3 tools/extract_assets.py --native-flight --source gameassets/fighters-anthology --out .local/native-flight/repro --dry-run
python3 tools/extract_assets.py --native-flight --source gameassets/fighters-anthology --out .local/native-flight/repro
cargo run --locked -p tore-app -- --native-flight-report
```

The first two commands read `FA.EXE` and `FA.SMS` from the specified directory.
This research mode is separate from archive extraction; do not combine it with
asset selection switches. Ordinary menu/theater/aircraft extraction is unchanged.
The Rust report uses the already imported Hornet PT (normal first-run import rules
apply), needs no display, and prints deterministic helper results rather than a
simulated flight. `--help` lists it.

The script writes hashes, section bounds, 3,829 symbols, 107 selected symbol spans,
direct call addresses, and direct PT field references. Selection is a keyword
inventory, not proof that every flight dependency has been found. Spans end at the
next exported address and can contain unnamed helpers. Indirect calls, pointer
aliases and indexed field accesses need manual analysis. Absolute-address matches
can include non-memory constants; inspect the associated instruction.

Outputs are local research artifacts, not distributable assets. Repeating the same
command is accepted when contents match. Differing files require a new output
folder or `--overwrite`; all files are preflighted before writes. `objdump` version
or input-path changes can change textual output. Dry-run checks metadata and
prints provenance without invoking a disassembler or writing files. The standalone
`tools/extract_native_flight.py` exposes the same pass. Other PE32/i386 builds can
be inventoried, but build-specific PT annotations require both reviewed hashes:

- FA.EXE: `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`
- FA.SMS: `e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0`

## State and units

`_cp` at `0x50ce80` and `_cpt` at `0x50d268` are global instance/type buffers,
not pointers. PT field offsets are derived from the shared packed Rust schema.
`_serviceTicks` at `0x546ba0` is read as a signed word; helper updates multiply
by it and arithmetic-shift eight places. Probe time uses the inferred fixed8
seconds contract; the FA timer scheduling path still needs a separate audit.
Do not equate a render frame or a 120 Hz tick with one native service tick.

The movement code keeps movement roll/pitch/heading separate from displayed body
angles. It adds load-related AoA, low-speed pitch, rudder slip, and turbulence in
different places. Native scalar speed/force and angle arithmetic do not constitute
a conventional lift/drag-coefficient aerodynamic polar. Applying a recovered AoA
number directly to the current velocity-alignment spring would combine different
models and is not a faithful integration.

## Translated helpers

All translations below are static instruction readings with synthetic regression
checks, **not differential execution against the original**. Arithmetic retains
signed truncation, fixed8 scales and explicit wrapping operations where reviewed.
Division faults and malformed envelope intersections become Rust errors.

| FA address | Rust helper / recovered behavior | Boundaries still outside helper |
| --- | --- | --- |
| `0x4119a0` | `match_f24`: rate-limited state approach | Native tick scheduling |
| `0x476950` | `stick_input`: asymmetric ranges, neutral return, reversal acceleration | Device input and damage-adjusted limits |
| `0x476880` | `low_speed_limit`: reduce excursion below twice clean stall speed | Selection of clean stall envelope |
| `0x476aa0` | `g_to_turn`: signed G/speed conversion, 125 fps floor, ±40°/s cap | Composition with gravity and attitude |
| `0x47c18c–0x47c1e5` | `pull_aoa`: load offset and slew | Stall/spin branches and movement/display composition |
| `0x476daf–0x476e58` | `low_speed_pitch`: fade low-speed movement offset | Ground state and final velocity rotation |
| `0x49d2d0`, `0x49d440` | `envelope_limits`, `interpolate`: authored ascending/descending chains, structural interpolation | G-envelope selection, header indices and damaged/load-adjusted limits |
| `0x49d230` | `envelope_class`: minimum/performance/structural speed classification | Native caller flags |
| `0x412780`, `0x47ab60` | `sound_speed`, `drag_percent`: altitude and transonic correction | Full force accumulation |
| `0x47a8c0` | `scalar_thrust`: throttle/scale/speed, zero thrust-vector angle | Engine/fuel gates and selected loaded thrust |
| `0x47a970` | `clean_drag`: clean force term with caller-supplied idle floor | Stores, G, rudder, devices, turbulence, ground drag |
| `0x451e50` | `fuel_rate`: military/afterburner threshold | BurnFuel tank selection and scheduling |

Pull AoA uses `gpullAOA × load / 9`, with fixed8 G. For positive G, load is
`max(G−1G, 0)`; for negative G it is G itself. The offset approaches a nonzero
target at 10°/s; return rate is at least 4°/s. The imported Hornet coefficient is
20. The separate low-speed offset uses `lowAOASpeed=70` fps and
`lowAOAPitch=15` degrees, fading above the clean stall boundary and toward vertical
movement. These are now linked to consumers, superseding the earlier note that
their units/consumers were entirely unresolved. The previous banked-pull adapter's
fitted AoA curve remains explicitly authored.

Envelope lookup uses the last highest-altitude point, then the authored chains;
it is not a generic closed-polygon intersection. Flaps lower the minimum speed
by one quarter for |G| ≤ 1. Structural speed interpolates between PT endpoints
through 36,000 ft. Above the envelope ceiling both performance bounds use the top
point's speed. Malformed/no-intersection cases are rejected, not approximated.

Fuel rate switches to the afterburner value above command 100; this alone does not
establish the actual burn timing or prove the UI's normalized throttle is the
native command. The drag consumer also exposes a missing factor in any simplified
weight-only device-drag approximation: the added coefficient sum is multiplied
by a weight term scaled by the speed-dependent drag percentage.

## Full model coverage and remaining steps

| Area | Static evidence | Remaining implementation/acceptance |
| --- | --- | --- |
| Main update | `_FMFlight` `0x47b020` located and normal-control/AoA sections traced | Translate complete branch/state graph and all callback contracts |
| Movement | `_MovePlane` `0x476ae0`; movement angles, gravity, display offsets, velocity and position stages identified | Joined rotation/wind/contact diagnostic tested, including both vertical attitudes; native query producers and unrestricted live connection remain |
| Setup/load | `_FMAircraftSetup` `0x47a690`, `_FMUpdatePlaneFields` `0x452140`, `_FMGetWeight` `0x4516b0` located | Typed aggregate loading/damage consumers joined; refresh cadence and lifecycle producers remain |
| Power | `_FMGetAcc` `0x47a770` traced as a save/modify/query/restore routine | Loaded force builder, lift/gravity and ordered velocity joined diagnostically; engine/fuel lifecycle remains |
| Rudder | Normal branch `0x47c419–0x47c682` relates turn rate, authority, slip and bank | Airborne rudder/slip/bank and ground steering joined diagnostically; unrestricted live connection and retail acceptance remain |
| Stall/spin | PT fields and `_FMFlight` state branches identified | Dispatch/tumble/normal-control/force/movement joined diagnostically; event execution and unrestricted live connection remain |
| Ground | `_GetGround` `0x47af20`, collision helper `0x477240` located | Landing classifier, wheel drag and pitch settling translated; complete contact/crash/carrier behavior remains |
| Turbulence | `_FMTurbulence` `0x477590`: low-altitude/nearby-aircraft branches, daylight scaling and timed state traced; [follow-up](weather.md) | Complete geometry/surface semantics, RNG/scheduler acceptance, maneuver buffet and replay determinism |
| Devices | Gear/flap/brake/vector/fuel update symbols inventoried | Sampled device drag/lift coupling joined; native actuator schedules remain; visual fitted hinges stay separate |
| Integration | Reviewed pure helpers exposed through headless report | Joined typed diagnostic state and rotations/contact; scheduler, query producers and gameplay integration remain |
| Acceptance | Synthetic arithmetic and imported-data probes | Captured original-game trajectories; level, banked pull, negative G, stall/recovery, device transients, full loops |

The ignored reference checkout's `Docs/formats/native-flight-code.md`,
`native-power.md`, `native-performance.md`, and `flight-dynamics.md` helped locate
questions and candidate routines. Its USNF97 addresses and oracle claims are not
FA verification. Reproduce research from FA media and record mismatches explicitly.

## Second pass: departure, ground and integration components

The follow-up static pass adds `flight_model::{profile,departure,forces,ground,integration}`.
These are small, renderer-independent Rust components with **no dependencies or
successful-tick heap allocation**. They run in the diagnostic report/tests only;
the playable adapter has not been replaced. The broader static spans include
untranslated instructions. A recovered routine boundary does not mean all of its
behavior is implemented.

The extraction command now also writes 18 explicitly bounded `reviewed/*.txt`
slices and `reviewed-components.json` for the hash-reviewed FA build. The manifest
records slice hashes, direct call/branch edges (including outgoing edges), 21
reviewed instance fields and open contracts. This exposes unnamed spin and
integrator routines that were previously buried inside exported symbol spans.
It is a manual research map, not an automatic decompiler or executable flight
model. Neither unknown builds nor dry-run receive these build-specific artifacts.
Use a new output folder when moving between extractor versions:

```sh
python3 tools/extract_assets.py --native-flight --source gameassets/fighters-anthology --out .local/native-flight/components
cargo run --locked -p tore-app -- --native-flight-report
```

### Departure state and recovered PT semantics

`cp+0x20c` (`0x50d08c`) is the byte mode; `cp+0x20d` is a signed-word timer.
Native modes are normal=0, warning=1, stalled=2, spinning=3 and an additional
warning stage=4. The descriptive name **ExtendedWarning** is ours. PT flag
`0x400` enables that additional stage. Entry into warning resets the timer; while
below the stall predicate it advances toward `stallWarningDelay`. The additional
stage uses that delay again. Stalled recovery requires both `stallDelay` elapsed
and a cleared stall predicate. Counters test `<0x1400` before adding, so can
overshoot the apparent cap. Ground dispatch first clears the mode.

`0x47cc70` tests the envelope at `clamp(floor(G),0,2)` and excludes a separate
vertical-thrust-support predicate (`0x47add0`). Initial warning also depends on
the current-G envelope classification and difficulty flags. Consequently the
Rust transition component takes those conditions from its caller; it does not
replace them with a guessed critical-AoA threshold. Hornet's warning/stall delays
are both 512 native units (two seconds under the inferred fixed8 clock).

The stalled severity in `0x47b287..0x47b2e2` starts at
`min(stallSeverity * elapsed / 1024, 256)`. A speed deficit of at least half the
selected stall speed scales it further by `512*(stall-speed)/stall`, divided by
256 and capped at 256. The Hornet coefficient is 256. Control response then uses
`max(256-severity,150)`: pitch divides by 256, roll/rudder by 1024. Lift retains
`(256-severity)/256`. These components are translated. Pitch-down toward −90°,
roll fall toward ±90°, tumble scheduling and all warning/audio effects remain
outside the component; their consumers are recorded in the departure slice.

Spin entry (`0x47ccb0`) precedes mode dispatch. It requires warning/stalled mode,
no global `0x40000` inhibition, thrust vector above −45°, and PT `spinEntry != 2`.
Entry mode 1 requires directional rudder ≥240 and pitch stick >128; other enabled
modes require rudder ≥120 and pitch >0. Controls use the native −256..256 domain.
`0x47cd70` chooses direction from roll-rate sign, then body-roll sign; only an
exactly level, zero-roll-rate tie requests a random choice. The Rust kernel takes
that choice explicitly rather than adding a hidden random generator.

The spin branch (`0x47b780..0x47b998`) is substantially translated:

- Same-direction rudder ≥200 slews intensity toward 100%; opposite ≥200 toward
  zero, at 25 percentage points per inferred second. Neutral rudder holds it.
- Body rates approach zero at 90°/s; movement pitch approaches −88° at 40°/s.
- Interpolated PT spin yaw rotates **movement roll**, not the normal yaw-rate
  state. Later movement/display composition supplies its apparent spin geometry.
- Speed approaches clean stall +110 fps at 50 fps/s; PT bank/AoA offsets blend
  from low to high with intensity and slew at 40°/s. Slip approaches zero.
- Recovery tests speed after this slew, requires speed > clean stall +10 fps and
  opposite rudder ≥200. `spinExit=-2` needs pitch <0 and 256 continuous native
  time units; other settings need pitch <−100, throttle ≥50% and 768 units.
- A `spinExit` in 0..100 latches cp flag `0x02000000` once intensity reaches that
  integer percentage, preventing recovery through this branch. The caller must
  preserve the flag on entry; its global lifecycle is not decoded here. Breaking
  recovery conditions clears the recovery timer.

The imported F/A-18D has `spinEntry=0`, `spinExit=-2`, yaw endpoints 120/180,
AoA 30/70 and bank offsets 15/5 degrees. These are **game model parameters**, not
real-world aircraft limits. The hybrid uses yaw endpoints with fitted coupling;
movement/display offsets remain diagnostic (see the response ledger below).

### Scalar forces and movement

`0x47c860` orders updates as transverse decay → drag → forward force → side force
→ down force → ground down-velocity clamp. It divides each accumulated force by
`weight >> 5`. Drag approaches zero without reversing speed; subsequent forces
can reverse it. A net-force Euler step is not equivalent at zero speed. Each
stage uses its velocity and acceleration limits. On ground with gear, forward
acceleration is temporarily quartered **after** the drag stage.

`0x47cbe0` clamps acceleration between `−dacc*256` and `acc*256`, applies elapsed
time, then clamps velocity to `min/max*256`. Transverse decay (`0x47cb80`) uses
`clamp(abs(v)/2,256,16384)` and stops at zero. `service_delta` (`0x4c65ec`) uses a
64-bit product shifted eight places, unlike `MatchF24`'s wrapping 32-bit product.
The distinction is tested with a product exceeding 32 bits.

The drag assembly (`0x47a970`) now includes caller-supplied loaded clean/pull drag,
G, rudder, turbulence and gear/flap/brake/bay terms. Added aerodynamic coefficients
multiply `trunc(weight*dragPercent/100)`. Ground wheel drag is instead
`trunc(wheelCoefficient*(brake ? 50 : 20)/100)*weight`; gear-up contact adds
`384*weight`. Native device **flags**, not visual actuator fractions, select these
terms. The idle floor, loaded thrust/pull coefficients and device flag lifecycle
remain upstream contracts. Lift and gravity were initially only sliced/traced; their translations and
force composition are covered in the third pass below.

A critical profile detail: Hornet `_bv.x.max` is **zero** in PT. `COBv` at
`0x477ea0` replaces it with loaded instance `cp+0x245` (`0x50d0c5`) before use.
Following the producer through `0x452482..0x4524d6` confirms that setup writes the
current-altitude 1G envelope maximum to that word. It also stores the minimum at
`cp+0x23f` and min(performance maximum, structural speed) at `cp+0x243`.
`FlightProfile::loaded_velocity` requires an explicit positive override; do not
feed raw PT limits to a live flight. The diagnostic uses the reviewed envelope
maximum at a supplied 5,000-ft altitude. Full weight/damage/authority setup remains
separate; this does not decode every loaded instance field.

The angle stage (`0x476b0d..0x476bb0`) accepts already transformed rates, crosses
±90° by changing the angle representation and flipping roll/heading 180°, and
retains ±180° endpoints. There is no pitch stop. The body-rate transform and world position/wind stages are translated in the
third pass below. Gravity turn, display offsets, world velocity matrix and
collision remain outside this angle stage. Existing playable complete-loop probes remain a
separate acceptance check.

### Ground handling recovered so far

`landing_severity` (`0x477140`) checks movement roll/pitch, forward speed, side
speed and a signed-word vertical speed. It returns native codes 0, 5 or 6. Code 5
begins strictly above the PT limit; code 6 begins beyond an additional 20° roll,
20° pitch, 50 fps forward, 10 fps side or 20 fps descent. Hornet limits are
10° roll, 25° pitch, 330/51/95 fps forward/side/descent. The vertical-speed producer
and terrain-relative interpretation still need a complete audit. The classifier
is not a final crash decision: water, gear, surface compatibility, damage and
difficulty can change the final result.

Contact code (`0x477240`) re-queries terrain, treats >25-ft surface changes
specially, holds new touchdown for 128 time units, floors aircraft height to the
surface and damps pitch/roll toward ground attitude. The translated pitch-settling
component uses 90°/s when below slope; otherwise 45°/s below stall−73 fps, fades
linearly to zero at stall−44 fps. Terrain attachment, bounce/pitch-down response,
water codes and carrier callbacks remain untranslated. `_GetGround` calls
`0x4abab0`; final contact invokes `0x49fd40`. We need those contracts before
claiming runway/carrier landing support.

### Lightweight mapping and integration plan

| Component | Implemented input/state boundary | Remaining integration |
| --- | --- | --- |
| Profile | One-time checked PT map → Copy departure/drag/landing/velocity structs | Additional aircraft parser/schema review; retain original resource/hash in import report |
| Controls/envelopes | Existing pure control/AoA/envelope helpers | Full loaded/damage authority and initial stall predicate |
| Departure | Small mode/timer + spin state; caller-owned random choice | Stall tumble/pitch-fall, audio/events and difficulty dispatch |
| Forces | Drag builder; ordered velocity step | Loaded weight/thrust/limits, native lift/gravity and VTOL |
| Movement | Independent movement angles with vertical crossings | Native rotations, display/body composition, wind/position |
| Contact | Landing classifier and pitch settling | Terrain/carrier query, touchdown state and damage/events |
| Clock/replay | Explicit signed native elapsed ticks; no renderer access | Audit scheduler; adapt 120 Hz without drift or dropping sub-tick remainder |

Keep immutable profiles shared per aircraft type and mutable state per aircraft.
Successful component ticks use scalars/fixed arrays, no trait-object graph,
resource lookup, file IO or allocation. The current modules live alongside the
existing experimental helpers in `tore-formats`; move reusable simulation code to
a dedicated dependency-free sim crate when the whole-tick boundary stabilizes.
No framework or new crate is needed for this research step.

If a missing behavior needs a fitted implementation, keep it at an explicit
component boundary with provenance, parameter units and aircraft-specific probe
results. Do not mix fitted AoA springs into native movement/display offsets.
Continue with loaded setup → force builder → movement/contact → clock/replay
before enabling an experimental native mode. The original-game trajectory
comparison gate remains open; this pass executed no original or emulated code.

## Third pass: extracted trigonometry, forces and loading

This pass extends the same static workflow to **28 reviewed regions** and an inert
642-byte sine table. It does not execute FA, emulate its instructions, or enable
the experimental components as the playable flight model.

```sh
python3 tools/extract_assets.py --native-flight --source gameassets/fighters-anthology --out .local/native-flight/rotations-final
cargo run --locked -p tore-app -- --native-flight-trig .local/native-flight/rotations-final/tables/sine-q15.bin
```

`--native-flight-trig` enables the ordinary native report plus imported-table
rotation/force probes, without a display. It reads at most 643 bytes and requires
exactly 642. The table reader validates the data layout, not executable identity;
use `tables/inventory.json` to establish provenance. The extractor gates the table
on both reviewed source hashes and records its VA, encoding and SHA-256. Outputs
remain ignored local derivatives. Existing dry-run/conflict protections also
apply to binary table output; unknown builds receive no table.

### Extracted table and arithmetic

FA `0x4cd588` reads signed words at `0x515a48`. A 16-bit angle's high byte selects
one of 256 segments; its low byte interpolates with an arithmetic right shift.
Cosine uses the same table displaced by 64 words. Including both endpoint reads
requires **321 words**, not 256. `rotation::TrigTable` owns those imported samples
once and does no allocation during lookup. No generated floating-point sine or
embedded retail table substitutes for this data.

Angle conversion (`0x4c6620/0x4c6638`) is also translated: the native constants
are 1000 and 1406, with positive rounding offsets even for negative inputs.
The degrees-to-PA multiply wraps at 32 bits before sign extension. As a result,
+90° produces PA 16387 and −90° produces −16386, not exactly ±16384. The table
therefore gives small nonzero cosines at the converted vertical angles. Preserve
that distinction in parity probes rather than silently normalizing it away.

`0x477010` transforms body rates into movement-angle rates using the table, wide
fixed16 multiplication/division and staged shifts. Its **local pitch argument**
is clamped to PA ±14560 (roughly ±80°) before evaluating the rate transform.
This is not a clamp on aircraft attitude: the subsequent angle stage still
changes coordinate representation when crossing vertical. Synthetic tests cover
identity/quarter-turn arithmetic and division faults; imported probes show the
same rate-transform result at 80° and 90° due to this local limit.

`0x4c6654` rotates X/Z using individual wide products shifted 15 bits before
addition/subtraction. This differs from gravity's signed division by 32767.
Even a nominal identity rotation may shorten a positive component by one unit;
negative shifts round downward. Tests retain these arithmetic effects.

### Force assembly now translated

| Component / FA address | Recovered behavior | Caller contracts still needed |
| --- | --- | --- |
| `thrust_force`, `0x47a860/0x47a8c0` | Fuel/throttle gates, speed penalty, vector sine/cosine and ordered scaling | Live throttle/vector/AB state and difficulty gates |
| `lift_force`, `0x47c980` | First-envelope-speed cutoff, flap/gear branch, lift scale and low-speed floor | Updated envelope and damage/stall lift scale |
| `gravity_force`, `0x47ca70` | Pitch and negative-roll projection with native signed division order | Display/body attitude produced earlier in the tick |
| `assemble` + `velocity_step` | Thrust, drag, lift and gravity composition feeding the already translated ordered velocity stages | Complete top-level state/update ordering |
| `loaded_weight`, `0x4516f2..0x451814` | Fuel/store buckets, overload cap, loading percentages | Bounded equipment-to-mass resolution and difficulty override |
| `loaded_drag`, `0x47a690/0x4784a0` | Shared clean/pull drag loading and damage-addition arithmetic | Damage gates and PT load coefficients |
| `selected_thrust`, `0x478190` | AB fallback to military when zero, optional halving | Native difficulty/player predicate |

Lift first checks the minimum-lift-speed helper (`0x49d1b0`), which reads the **first 1G envelope
point's speed**, not the altitude-adjusted stall-speed intersection. Once above
that cutoff, native lift scale defaults to 256. With flaps and gear down, the
flap bonus is `dragPercent*flapsLift/100`, narrowed to a word and halved on ground.
With flaps and gear up, the reviewed branch leaves ECX at **raw `flapsLift`**,
even though EAX computed the scaled value. The Rust translation preserves this
unexpected difference; it is not presented as real-world flap aerodynamics.

Below `stall + min(stall,146)` fps, lift scale multiplies by speed over that bound
and is floored at 160. That floor can raise a previously reduced lift scale.
The final lift magnitude is `weight*scale`, subtracted from native down force.
Gravity uses `weight<<8`, pitch sine/cosine and negative-roll sine/cosine. Level
clean lift scale 256 exactly balances gravity at exact table angle zero. Banked
and inverted body-axis forces need not balance; body/world rotation comes later.

Weight loading adds integer internal fuel (`fuelF8>>8`) and resolved stores to
empty weight. Native hardpoint flag 1 chooses a second mass bucket. Above maximum
takeoff weight, mass is capped and both percentages become 50; otherwise each
bucket is divided by `(maximum-empty)` after multiplication by 100. These are
source buckets, not guessed internal/external aerodynamic classifications.
The resolver's gun-ammunition divisor and tank fuel handling are traced in the
weight slice but are still caller work, so callers must not substitute station
class/visual visibility for flag 1.

### Position and wind

`position_step` translates `0x476ed2..0x476f98` after the body-to-world velocity
builder. It integrates each world component separately with native elapsed time.
Airborne wind is an additional displacement: `_windSpeed` (`0x580e30`) is shifted
eight bits, time-scaled, then rotated by `_windH` (`0x583db0`) with Rotate2. Ground
motion omits wind. The kernel accepts these values explicitly and does not couple
to the renderer or a custom terrain system.

The landing vertical-speed producer is now resolved: `cp+0x22a` (`0x50d0aa`) gets
`(worldVelocity.y >> 8)` narrowed to a signed word, **before** contact correction.
It is world vertical feet/second, negative while descending, not the body-down
velocity component or terrain-relative sink rate. Wind only changes X/Z here.
This supersedes the earlier producer uncertainty in the ground notes.

### Remaining whole-tick gate

The principal movement gap is now the matrix builder/application path
`0x476fb0 → 0x4d5e58 → 0x4d64d8`, together with gravity-turn and display/body-angle
composition. The matrix application uses wide sums followed by a multi-bit SHLD
and overflow-flag branch; overflow flags for multi-bit shifts need explicit
handling/research before claiming all-domain integer parity. No fitted matrix
has been silently introduced into the translated path.

Still open: full loaded/damage control authority and equipment resolver, idle
thrust/drag setup, stall tumble, complete terrain/carrier contacts and events,
native timer/RNG ownership, and original-game trajectory comparison. Successful
new component updates remain allocation-free; no new dependencies were added.
For future aircraft, share the extracted trig table per source build and immutable
PT profiles per aircraft type. Keep world/contact/clock services outside the
per-aircraft data instead of cloning an engine for each aircraft.

## Fourth pass: composition, contact, loaded state and time

Static FA evidence only; the playable 120 Hz adapter is unchanged. New pure
components live in `tore-formats::flight_model`, with caller-owned state and no
runtime symbol lookup or executable loading.

- `rotation::Matrix` translates the roll → pitch → heading matrix construction
  at `0x4d5e58`, skipping zero rotations to preserve integer rounding. Word
  vector transforms use column dot products, wrapped 32-bit sums, Q15 shifts
  and saturation excluding −32768 (`0x4cf328`). World velocity uses
  `[side, -down, forward]` with movement heading, effective pitch and negative
  movement roll (`0x476fb0`). Wide-vector overflow returns an error: the native
  multi-bit SHLD overflow-flag branch is not established outside the normal
  representable domain. This is not an all-input CPU emulation claim.
- `AtanTable` reads 514 unsigned words at `0x515644`; the adjacent sine table
  remains 321 signed words at `0x515a48`. `0x4ccb88` uses octant interpolation
  and constants `0x3ffc`, `0x7ff8`, `0xffef`, not idealized quarter turns.
  `cockpit_offset` rotates forward/right basis vectors and recovers heading,
  pitch and roll. `cockpit_angles` follows `0x476cba`: movement conversion,
  display-only pitch clamp, slip/AoA composition, bank addition, then turbulence
  composition. A large wrapped heading change toggles native flag 4. Movement
  pitch itself is not constrained by this display path.
- `ground` now separates retention, queries, classification and settling.
  `0x411910` considers height ≤ queried ground + one fixed8 foot touching.
  The 25-foot abrupt-height threshold is strict; hold timers can cross below
  zero. The slow-ground retention comparison unusually mixes a PA ground-pitch
  word + 364 with effective F8 pitch; this translation preserves the observed
  comparison rather than silently correcting it.
- Contact classification preserves difficulty bypass → water → landing limits
  → gear/type/surface-query precedence. Surface query failure or a signed result
  above `0x465000` produces code 5. Successful new contact resets roll/yaw rates,
  applies pitch-down response, starts a 128-unit hold, floors height, settles
  pitch/roll and zeros side/positive-down velocity. Water remaps codes 5/6 to 7.
  `0x49fd40` is now identified as a **landing latch**, not carrier dynamics:
  flag `0x04000000` sets on airborne→ground, persists on ground and clears in air.
  `settle_contact` returns the touchdown event boundary `(4, 0x40)` separately.
- `loading::equipment_mass` handles resolved type 7 weapon mass and optional
  signed divisor, type 8 tank shell mass plus instance fuel (added once), other
  unsigned-word mass, quantity high-bit masking and the `0x7fff` sentinel.
  Resolver `0x452770` indexes 17-byte instance stations and 24-byte definitions;
  it is not a recursive equipment tree. Runtime pointers must be resolved into
  bounded typed equipment by the importer before calling the arithmetic helper.
- `loaded_controls` copies three four-word axes, replaces pitch min/max with
  loaded values, applies damage percentages/roll-lock deadline or the alternate
  50–75% reduction, then load reduction to roll min/max only (`0x477ed0`).
  Acceleration/deceleration and yaw survive unchanged. The alternate branch's
  cp+0x0e/cpt+0x49 fields retain neutral names pending semantic confirmation.
- `clock_rng` translates the shuffled 32-entry Park–Miller generator
  (`0x4561d0`, 40 initialization advances), explicit seed/state and bounded
  draws. Nonpositive bounds consume no state. Counter conversion uses 256
  units/second. Frame timing preserves signed-word shifts, pause, optional 4/3
  scaling and 5–128 clamps; service age wraps as a word with minimum 2.
  `FixedClock` is an **authored** remainder-preserving bridge from 120 Hz to
  256-unit time. It is not the original scheduler. RNG consumption order across
  all aircraft/events, initial seed source, scheduling and OS timer fallback
  remain unverified; reproducible helper draws do not establish native replay.

Repeatable static extraction now writes both tables and 43 reviewed regions.
Run the bounded, headless composition example in [DEVELOPMENT](../DEVELOPMENT.md)
to inspect imported-table matrices and explicit clock/RNG state.

Still open before a native whole-tick adapter: terrain/carrier height and surface
query producers (`0x4abab0`, `0x4ba8e0`), touchdown event dispatch, complete loaded
field/damage/equipment producers, scheduler and RNG call ordering, matrix overflow
edge semantics, and full-trajectory acceptance. These helpers close arithmetic
contracts; they do not establish complete contact-system or full-flight parity.

## Fifth pass: query producers, reseeding and object dispatch

This pass corrects the interpretation of the landing surface query. At
`0x4ba8e0`, the engine scans a reverse-order inventory of 0x134-byte records,
resolves each object's id at +0xe6, and requires type flag 0x8000. With the
landing caller's null object and disabled optional filters, it first considers
active objects passing `0x4747c0`; only if none exist does it retry without that
preference. It temporarily sets query Y to the candidate's Y before measuring
against candidate position +0xc8, then restores query Y. Thus `0x465000` in the
landing classifier is an **approximate horizontal distance of 18,000 fixed8
feet**, not a material id, friction coefficient or squared distance.

`queries::landing_surface` translates selection with explicit resolved candidates.
Strictly smaller distance wins; equal distance preserves the first candidate in
reverse inventory order. `approximate_distance` follows `0x4c66cc`: unsigned
absolute wrapped differences, largest + (other >> 2) + (other >> 2). It does not
substitute Euclidean distance. Candidate production and `0x4747c0` are still
external; these records must not be invented from theater map colors.

Ground height `0x4abab0` has two paths:

- In mode word 0x520a50 == 16, an object with deadline +0x27 strictly greater
  than current ticks supplies cached height +0x2f, two angle words +0x2b/+0x2d
  and a water result from byte +0x33 == 1. Unless request bit 2 is set, subtract
  the signed type-derived word at +8 (resolved via `0x42e0c0`) shifted by 8.
  The cache's producer/lifetime remains to be traced; mode 16 is not assumed
  to mean carrier mode.
- Otherwise it constructs a vertical segment at query X/Z from 30,000 to −100
  fixed8 feet and calls collision dispatcher `0x42b800`. Base mask is 3, request
  bit 2 adds 0x200, request bit 4 clears mask bit 2. An object with type flag
  0x8000 or without instance flag 0x4000 also clears mask bit 2.
  `queries::ground_query_mask` translates these gates. Terrain triangle tests,
  object geometry and cache writes below the dispatcher remain unported.

The touchdown `(4, 0x40)` call has a separate gate at `0x412a60`: global flag
0x08000000, byte 0x4eb64a mask 0x01, a nonzero id at 0x520a1c, and equality with
0x4eb64c. The downstream `0x499240` compares the requested code with current
code 0x501538; a positive current state rejects a larger code, the same code
ORs state with 0x10, otherwise it stores the new code and signed argument / 4.
Do not describe this small event-state helper as carrier dynamics. Its consumer
and the meaning of these global gates still require decoding.

RNG and scheduling refinements:

- `0x4561c0` reseeds with **negative absolute signed 16-bit input**, including
  zero and −32768. Zero becomes seed one on the next draw. It does not eagerly
  clear the shuffle array; the next draw rebuilds it. `NativeRng::reseed_word`
  preserves this contract.
- `0x4561a0` always takes a bound-100 draw and compares it against the signed
  percentage. Even impossible/certain outcomes consume state. `chance` tests
  enforce this distinction from the nonpositive-bound early return.
- Four direct reseed calls exist in this build: 0x42084b, 0x42f2fd, 0x44d96b
  and 0x48078b. The first three XOR a prior bound-65536 draw, the low word of
  `0x4869a0`, and word 0x4ece3c. The fourth uses word 0x54e490. The latter's
  upstream initialization and global RNG consumption order remain open.
- Object service age is calculated during object load, then aircraft type 4
  refreshes loaded fields via `0x452140`. Dispatcher `0x462a88` compares a due
  **unsigned word** at cp+0x68 against the low word of current ticks >> 6.
  This is not a wrapping-age comparison. It removes the queue head, clears
  instance bit 2, calls `0x462e70` only when active bit 1 is set, and writes
  last-service timestamp cp+0x66 after that call. `object_due` translates the
  due predicate; the queue and its dispatcher are not replaced by a new runtime.

Extraction now includes 52 reviewed regions and schema-2 per-region
`entry_references` (direct calls/jumps to the entry only). Indirect calls and
jumps into a region's interior are not a complete call graph. New components
remain diagnostic. Next gates are collision dispatcher geometry/cache producers,
object rescheduling and RNG seed producers, event consumers, loaded field
semantics, then a complete state/update harness with native trajectory evidence.

## Working hybrid adapter and Rafale C cross-check

The next pass builds a working free-flight adapter in `tore-sim` and verifies the
same extraction/model workflow on RAFALE.PT (retail long name “Rafale C”). The
660-byte reviewed layout is shared with F18.PT; RAFALEE/RAFALEF are distinct and
remain rejected. Extracted Rafale facts: 17,100 lb empty, 9,900 lb internal fuel,
24,000 lbf military thrust, 32,000 lbf AB thrust, spinEntry 1 and spinExit −2.
Hornet spinEntry is 0, so the second aircraft exercises different native thresholds.

This changes the earlier diagnostic-only boundary **for selected helper rules**:
`--researched-flight` uses native departure/recovery rules and source spin yaw
ranges in an explicitly fitted continuous adapter. The exact integer force/
matrix/scheduler model is still incomplete. The legacy app default remains intact.
See [FLIGHT-MODEL](../FLIGHT-MODEL.md) for component provenance and remaining gates.

Lessons from integrated tests: constant drag must not reverse a stopped wheel
roll; ground pitch needs support instead of integrating through the runway;
controlled takeoff must transition from rotation to a climb command rather than
hold a loop-producing high-G pull. Wind must be subtracted for air-relative
forces and added back for position, not change TAS merely through advection.
These are fitted integration decisions, not newly decoded native instructions.

## Response producer/consumer ledger — 2026-09-15

The repeatable flight extraction now includes bounded setup (`0x47b020..0x47b250`),
control (`0x47ba8c..0x47c682`), finish (`0x47c682..0x47c860`) and maneuver-sound
input/caller slices. Build/resource hashes and measurements are in the
[response baseline](../baselines/flight-response.md). The fixed-address pass is
still restricted to the reviewed FA EXE/SMS pair; SMS symbols do not prove that
another source build has these contracts.

| Channel | Native producer and units | Consumers and boundary |
| --- | --- | --- |
| G, cp+0x19b (`0x50d01b`) | Signed fixed8 G; `0x47c0ea..0x47c10b` sends pitch command, loaded limits, neutral 256 and source rates to `StickInput` | AoA/G-to-turn, drag, stall selection, sound `0x434555`. This is a controlled G state, not a measured accelerometer. Runtime achieved normal-force G is a separately defined adapter channel. |
| Roll rate, cp+0x17f (`0x50cfff`) | Signed fixed8 degrees/time, source `_brv.x` limits and `StickInput` around `0x47c246..0x47c2aa` | Body/movement transform, spin direction, sound absolute value divided by 37 plus 384. Native scheduling remains separate from host seconds. |
| Rudder (`0x545200`) | Signed pilot/control domain −256..256, read at setup `0x47b048`; stalled branch modifies it at `0x47b3b0..0x47b3c4` | Spin entry/recovery, normal yaw `StickInput` at `0x47c529..0x47c578`, sound absolute command ×4 unless flag 0x1000. It is not measured sideslip or a physical pedal angle. |
| Slip offset, cp+0x1ab (`0x50d02b`) | Normal control computes offset after yaw response; spin slews it toward zero | Display-angle composition and native drag. It is distinct from world-velocity-derived sideslip used by AirData. Loaded/damage consumers are joined diagnostically; subsystem producers and live connection remain open. |
| Departure, cp+0x20c / timer +0x20d | Byte mode / signed word time; entry before dispatch at `0x47b250`; warning/stall transitions in that slice | Control/lift attenuation, movement fall, spin, sound. Modes/transition helper are translated; native current-G classification, difficulty, device and tumble producers are not all runtime-connected. |
| AoA offset, cp+0x1a7 (`0x50d027`) | Controlled fixed8 degree display offset, from normal G/speed and spin endpoints | Display composition; not interchangeable with geometric AoA from body/velocity. |

`0x47ccb0` tests global inhibition, departure mode, PT spinEntry and vector-thrust
inhibition **before** requesting direction. Direction uses the signed native
quantized rate and bank. On entry, `0x47cd49..0x47cd62` writes mode/direction and
zeros recovery time/intensity. The adapter now follows this ordering and resets,
with caller-owned clock/RNG. Native global RNG stream/lifecycle remains unverified.
The reviewed Rafale has spinEntry 1; Hornet has 0; both have spinExit −2.

Stalled severity samples the timer before the increment at `0x47b3ed`.
The source selects the current-G envelope's stall speed for severity; initial
warning classification additionally branches through `0x49d230` at `0x47b9ee`.
The hybrid's clean-envelope reference/classification remains explicitly fitted.
Its connected attenuation uses the already translated scalar helper; this does
not establish native control/force update parity.

Pitch fall depends on movement pitch, trig factors and the deadline at cp+0x1ca;
roll fall uses movement roll and an additional exact-zero random branch
(`0x47b2e2..0x47b36f`). These are not body Euler-angle targets. Tumble/deadline
producers and complete later composition remain unconnected. They must not be
filled in by adding noise or applying sound intensity to aerodynamic forces.

Hybrid uses native yaw endpoints with fitted continuous coupling;
movement pitch/roll, speed slew and display AoA/bank offsets are now connected
in the restricted airborne native path; unrestricted lifecycle/contact remains open.
See [current runtime contracts](../FLIGHT-MODEL.md#flight-response-contracts--2026-09-15).

## Native tumble continuation — 2026-09-15

**Origin: native, established by static code in the reviewed FA EXE/SMS pair.**
**Historical checkpoint: initial diagnostic translation and synthetic branch tests; no live
adapter connection or matched retail comparison.** “Tumble” is our descriptive
name for this timed movement rotation, not proof of a particular real-world
flight phenomenon. See [provenance policy](../behavior-provenance.md).

The repeatable extraction adds explicit slices for warning/extended-warning
expiry, stalled movement fall, the timed movement rotation, the trig wrapper and
byte RNG wrapper. Native state at cp+0x1c6/1ca holds start/deadline dwords;
cp+0x1ce/1d0/1d2 holds progress/previous progress/direction words.

### Source-derived expected behavior

| Source | Trigger and observed contract |
| --- | --- |
| `0x47b554..0x47b5fb`, repeated at `0x47b681..0x47b72f` | On warning expiry, a still-active deadline cancels. Otherwise start only if signed PA pitch ≥0 and signed forward speed <0, OR pitch ≥0x31c4 and speed ≤110×256. Positive bank selects −1 direction; zero/negative bank selects +1. |
| Same scheduling branches | Start is current native time. Deadline is start+512 plus signed truncation of pitch×128/−0x3ffc for nonnegative pitch. Previous progress resets to zero. This is not a generic “low speed means tumble” trigger. |
| `0x47ba8c..0x47bb85` | Ground or spinning mode clears deadline. While current time is strictly below deadline, phase is `(elapsed×32760/duration)` narrowed to a PA word, minus 0x3ffc. Imported sine-table output becomes progress `(sin+32767)/256`; the progress difference produces a signed PA increment. At the deadline the source skips the final sample. |
| `0x451820 → 0x417f00 → 0x451820` | Convert movement angles to PA, compose the signed increment as a heading offset through native basis-vector math, then convert back. It is not an additive camera shake or a body-Euler heading change. The event call `(4,64)` is separate. |
| `0x47b2e2..0x47b36f` | Stalled movement pitch approaches −90° unless a tumble deadline is active. Pitch and roll rates use severity and the source cosine factor; roll approaches ±90°. Exact zero movement roll draws a byte and tests bit zero. |
| `0x4562e0..0x4562ea` | The fall-direction draw is a bound-256 native RNG call, not the spin-direction chance(50) helper. |

`tore-formats::flight_model::tumble` represents these as caller-owned state,
explicit native time, imported trig/atan tables, and explicit zero-roll RNG input.
No new force coefficient, random shake or fitted motion is added. The initial
synthetic tests cover scheduling boundaries, active cancellation, direction,
progress differences, exclusive deadline, ground/spin cancellation and pitch
suppression during an active tumble. They do not establish the complete native
tick or observed retail trajectories.

### What full coupling still needs

The diagnostic component consumes the original movement-state contract. The
playable adapter integrates independent floating-point body attitude and world
velocity using several fitted laws. Connecting tumble directly to that body's
Euler angles would change the meaning of the recovered code. Remaining work is
native envelope/difficulty/device producers, normal controls and offsets, ordered
force/velocity stages, movement conversion and event/state lifecycle in one
explicit update path. Table/composition helpers already exist, but their
existence alone does not complete that connection.

“Retail comparison missing” means we still lack matched original-game runs that
show timing, altitude/speed loss, rotation and recovery for the same aircraft,
loadout, inputs and conditions. That is separate from the missing implementation
above. Static branch expectations can be established now; whole-trajectory
agreement cannot yet be claimed. [Checks and limits](../baselines/native-tumble.md).

## Joined native departure stage — 2026-09-15

**Origin: native static contracts. Status: joined diagnostic stage tested with
synthetic inputs and both reviewed PTs/imported trig tables. Live integration
and retail trajectory comparison remain open.** No fitted flight law was added.

`departure_stage` joins the preceding timer, attenuation, spin and tumble
translations in original movement coordinates. Inputs retain the source G state,
explicit envelope queries, commands, flags, native time and RNG. The diagnostic
example resolves PT fields before stepping; kernels do not access raw PT maps.

### Producer refinements

- `0x47b4ec..0x47b50c` / `0x47b9e8`: initial classification uses floor(G).
  Global flag `0x20` moves that row one step toward −1..1. The flag's numeric
  branch is established; a complete user-facing difficulty mapping is not claimed.
- `0x47cc70`: the continued-stall query uses floor(G) clamped to 0..2, requires
  class 1 and excludes vertical-thrust support. This differs from initial
  classification, current-G severity speed and clean 1G spin-recovery speed.
- `0x49d200/0x49d230`: a missing classification row returns class 1. The severity
  query requires a present current-G row; malformed/missing required data becomes
  an explicit error rather than executing a native null pointer path.
- `0x49d1d0`: stall speed from the selected envelope is floored to one fps.
  Existing envelope interpolation preserves flap reduction for −1..1 G; the
  first clean 1G polygon point remains separate for ground-control inhibition.
- `0x47add0` reads `vtLimitDown` at cpt-derived `0x50d3c5`. Both reviewed aircraft
  store zero, so the native helper returns false before thrust/vector arithmetic.
  The diagnostic probe requires that source value; nonzero VTOL profiles are
  not silently assigned the same result.
- `0x47b207..0x47b250`: ground dispatch clears the mode and suppresses pitch below
  `min(first1Gspeed,73)` whole fps, then roll/rudder below 5 fixed8 fps. Exact
  thresholds retain source signedness and comparisons.

### Ordering now exercised together

1. Refresh source pitch PA from movement, preserve the caller-supplied cp body-bank
   word, and apply ground control inhibition. FMFlight does not rebuild body bank
   from movement roll at entry; existing display offsets can make them differ.
2. Spin entry gates, including the distinct chance(50) tie draw and entry reset.
3. For spin: slew rates, movement pitch/roll, speed and display offsets; evaluate
   recovery after speed slew. Return the updated recovery-lock state to the caller.
   The jump at `0x47b9e3` skips tumble **and normal controls even on recovery**.
4. Otherwise, stalled mode samples severity before timer increment, performs
   movement fall with explicit bound-256 RNG only at zero roll, and attenuates
   controls/lift. Warning/extended-warning expiry schedules or cancels tumble.
5. Apply the timed native movement composition, then expose that normal controls
   should run. Failed diagnostic updates leave state and RNG unchanged.

The stage exposes control/lift outputs and the normal-control dispatch decision.
It does not execute normal controls, force integration, collisions or sound/event
consumers. In particular, `0x47c682..0x47c6b5` temporarily substitutes 1G during
the stalled force call and restores the source G state afterward. A complete
native flight path must preserve that ordering rather than feed our adapter's
measured G into this source-controlled channel.

Remaining coupling work: normal loaded/damage control producers and yaw/slip/AoA
outputs, complete ordered force/velocity/movement update, and event/contact
lifecycle. These remain separate from the now joined departure stage. The
existing hybrid is unchanged; no body-Euler shortcut connects this stage to it.
[Native-data checks and reproduction](../baselines/native-departure-stage.md).


### Departure force/velocity connection — 2026-09-15

**Native, translated/tested diagnostically:** `force_stage::advance` joins the
previously reviewed force helpers with `velocity_step`. Source/build identity
is unchanged. `0x47c682..0x47c6b5` substitutes 256 for the native stored G only
when departure mode is **stalled (2)**, calls `0x47c860`, then restores G. The
Rust API takes stored G by value and exposes the temporary force G separately.
Warning and spinning retain their input G. This affects excess-G drag; it does
not replace the independently attenuated lift scale at `0x54b6e0`.

The stage shares forward speed, weight and device state across thrust, drag,
lift, gravity and ordered scalar velocity integration. Lift uses the first 1G
vertex cutoff and altitude-adjusted **1G** stall reference, not the current-G
severity reference. Preserve the existing low-speed lift floor of 160, including
when departure attenuation supplies zero. Gravity uses cached body pitch and
negative body bank, not reconstructed movement angles. Transverse decay precedes
force application and does not feed these force producers.

Loaded coefficients/limits, selected thrust, engine scale, idle floor, damage,
devices and cached body angles remain explicit inputs. The example checks force
snapshots from departure outputs independently; it does not feed their velocity
back into a trajectory with the intervening normal controls and later movement
still absent. This is not a new live adapter. [Evidence](../baselines/native-departure-stage.md#force-connection-follow-up).

### Normal-control and movement/contact composition — 2026-09-15

**Native, diagnostic translation:** `normal_control` joins the reviewed primary
G/pitch/AoA and roll consumers (`0x47c0a2..0x47c12b`, `0x47c18c..0x47c1e5`,
`0x47c235..0x47c2a5`, `0x47c2f0`). Loaded axis limits and commands after departure
attenuation are explicit inputs. G bounds are fixed8 and reduce about 1G below
twice stall speed; the lower bound is capped at zero. Roll bounds reduce in
**whole degrees before the fixed8 shift**. The updated G minus 1G drives pitch
rate through `0x476aa0`, while AoA uses the separate `pull_aoa` response. Ground
clears primary roll rate; bank display offset slews to zero at 90 degrees/sec.
Loaded/damage limit producers, auxiliary rates, high-G events and the remaining
rudder/control branches are not implemented by this component.

`movement_stage` joins `0x476ae0..0x476f98`, before the contact query call:

1. Transform body rates using movement roll/pitch, integrate movement angles and
   preserve both vertical chart crossings.
2. Apply `0x476bb0..0x476cba` gravity-turn correction except when spinning,
   grounded or already at/below −90° movement pitch. It uses cached speed at
   `0x5451f0`, a 1G turn rate, pitch cosine and then roll cosine/sine. Preserve
   the separate signed service-time products and final downward pitch bound.
3. Compose body/display angles. Departure offsets are `[bank,AoA,slip]`;
   `cockpit_angles` takes `[slip,AoA,bank]`. The boundary explicitly reorders them.
4. Resolve low-speed travel pitch using a **flaps-cleared** 1G stall query.
   Add the offset for nonnegative forward speed, subtract for negative speed;
   clamp only that effective travel pitch to ±90°. The movement attitude itself
   remains independent. Transform velocity, integrate position and separate wind,
   then record vertical speed before contact.
5. `Output::settle` supplies the new position/attitude/velocity to translated
   contact settling. Query results must refer to that position. Cached ground,
   second touching query, retained height and classified code remain distinct.
   Display angles and vertical speed retain their source pre-contact values.
   Touchdown rate changes and contact events are returned to caller-owned state.

The repeatable extraction now includes gravity-turn and low-speed-travel slices.
No terrain/carrier producer, equipment callback or imported native code executes.
The normal branch's auxiliary rates and damage effects remain upstream; neither
live adapter is switched. [Acceptance scope](../baselines/native-movement-control.md).


### Joined flight diagnostic — 2026-09-15

**Native, diagnostic connection:** `flight_model::diagnostic` now joins the
previously separate components into recurrent state updates. The component-only
boundaries described above still apply to `native_departure`; the new
`native_flight` example feeds the complete diagnostic result into the next service.
The legacy and hybrid adapters are unchanged. This diagnostic checkpoint now
also feeds a restricted airborne live option; see
[current runtime acceptance](../baselines/native-live-flight.md).

New translated contracts:

- `loading::loaded_g_limits`, `0x452167..0x452482`: accepted envelope rows and
  adjacent-row interpolation, loaded scaling and difficulty/player adjustments.
  The enclosing refresh cadence at `0x452140` is not reproduced.
- `normal_control::passive_fall`, `0x47bb85..0x47bcb2`: normal-mode low-speed
  pitch/bank fall, signed-speed target and cached pitch cosine.
- `control_disturbance`, `0x47bcb2..0x47c0a2` and selector `0x47af70`: numeric
  pitch/heading selection, throttle/speed/touching gates, signed offset response,
  zero-crossing and quiet timers. Numeric request producers remain external;
  this is distinct from environmental `_FMTurbulence`.
- `control_tail`: auxiliary scaling at `0x47b0e7..0x47b182` and rate consumers
  at `0x47c130`, `0x47c2aa`, `0x47c63a` use original commands before departure
  attenuation. Preserve the source vector **minimum with −90**, not a maximum.
  Ground steering `0x47c301..0x47c419` selects the larger absolute command
  (rudder wins ties) and quarters bounds over 73..146 fps. Airborne rudder
  `0x47c419..0x47c682` scales authority against PT `envMax`, applies damage,
  normalizes rudder, updates display slip and couples yaw percentage to bank.
- `forces::idle_drag_floor`, `0x47ac20`, called at `0x47a99e..0x47a9cd`: airborne
  idle throttle adds a pitch-dependent floor up to 64, regardless of fuel.

Joined order: initial ground query; typed weight/loaded G/control inputs;
departure dispatch and exact tumble PA composition; optional passive fall,
control disturbance, primary and auxiliary/rudder response; loaded forces and
ordered velocity; temporary auxiliary-rate addition; movement/display/world
position and wind; new-position ground query, retained height and touching
query; contact classification/settling/flags; auxiliary-rate subtraction.
Normal controls remain skipped on a spin recovery tick. Pre-contact display
and vertical speed remain cached independently of contact-corrected movement.

The service requires native environmental turbulence disabled for airborne
updates (`0x477590` tests `0x01000000`); ground also bypasses that routine. Its
bypass resets next/start/end times at `0x477ce4`, with no movement change. These
unused environmental timer words are excluded, and the enabled airborne branch
returns an error. This explicitly bounds the flight diagnostic without inventing
an environmental force or silently omitting an enabled native call.

Configuration resolves names only at construction. Mutable fuel/device/damage
samples, clock and read-only contact queries belong to the caller. State and RNG
commit only after successful queries; external callback side effects cannot be
rolled back. Departure, high-G and contact events are returned, not executed.
Terrain/carrier producers, actuator/fuel/damage lifecycles, setup refresh cadence,
global scheduler/RNG order and live activation remain separate open contracts.


### Restricted runtime connection follow-up

The source contracts above now also feed `tore-sim::native` through an explicit
live airborne option. This supersedes diagnostic-only completion claims for those
consumers; it does not complete their external producers or callbacks.
`movement_stage`/`diagnostic::Events` expose pre-contact air-world velocity and
native fixed8 lift force for host projection without reconstructing travel from
the cockpit attitude. Source branch order is unchanged. Runtime adaptation,
provenance and open gates are in the [flight guide](../FLIGHT-MODEL.md) and
[live baseline](../baselines/native-live-flight.md).

### Land-query foundation — 2026-09-15

[The land-contact contract](native-land-contact.md) now establishes GetGround
dual-query ordering, collision cache mutations/deadlines and landing preference.
Two predicate helpers are diagnostic translations; no terrain producer or live
contact activation is implied. The repeatable extraction adds five reviewed
regions. Query-cache mutation and conditional RNG require transactional ownership
before the current read-only diagnostic interface can host native queries.
