# Fighters Anthology AI source notes

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research and implementation, 2026-09-17. This is a static trace map, not a proposal to execute
retail scripts or reproduce the original VM. Player-visible rules belong in
[the main AI specification](../spec/ai.md) and
[the experience specification](../spec/ai-experience.md). Exact build identities,
corpus hashes and commands belong in [the baseline](../baselines/ai-research.md).

## Existing work and limits

The ignored USNF-ATF checkout's `Docs/formats/ai.md` describes nine script
families, the language, skill literals and some executable routines from earlier
titles. Its `engine/src/sim/ai/program.ts` and `vm.ts` parse and interpret
scripts, but do not provide the aircraft movement/sensor host. Its conservative
action suspension is explicitly a hypothesis. Do not import that runtime.

Separately, `engine/src/sim/combat/world.ts` around lines 890-960 implements
authored pursuit and guns. It uses a turn acceleration proportional to
`3 + skill`, a 40 to 1000 metre firing window and a two-second burst cycle with
`0.65 + 0.1 * skill` seconds enabled. It directly constructs attitude and
velocity. These are observations of the reference implementation, not FA facts.
That controller does not call the recovered AI interpreter. Earlier notes about
operand/RNG ordering do not impose requirements on the Rust rebuild.

Current Rust reuse points are `tore-sim/src/sensors.rs`, `combat/live.rs`,
`combat/missiles.rs`, `models/mod.rs`, `flight.rs` and `autopilot.rs`.
Sensors already expose simulation-owned observations; flight supplies the
motion model, and combat supplies launch/damage services. The live combat
adapter is still organized around a player and explicit target fixtures, not
a fleet of independently armed, sensing actors. Multi-actor ownership needs
an explicit adapter step.

`tore-app/src/quick_mission.rs::dummy_wings` passes only `(AircraftId, count)`.
It drops experience and side/wing identity from the launch payload. The six
experience controls occupy draft indices 5, 8, 11, 22, 25 and 28. Ground
target/defense controls are currently rejected by `unsupported`. There is no
implemented AI module in `tore-sim/src/lib.rs`.

## FA catalog bindings

In the inspected FA_2.LIB, 145 PT records use `_PLANEProc` and name a BI module
through `ctName`. The family counts are in the spec. Of 84 NT records, 73 use
`_GVProc`, five `_CARRIERProc`, four `_OBJProc`, one `_CATGUYProc` and one
`_EJECTProc`. Only SARAN.NT names HYDRO.BI. There are also 170 OT records:
157 `_OBJProc` and 13 `_STRIPProc`. None names a `ctName` program in this census.
These are record counts, not proof that every record spawns an autonomous unit.

Nine AI source files and nine corresponding BI modules were extracted. Every
source `chance` literal has one matching push-immediate byte pattern in its
corresponding BI. This corroborates constants only. It does not establish full
source/compiled equivalence or prove a byte pattern is reachable code. Do not
promote old USNF/ATF program counts to FA coverage.

The installed-root census now also covers FA_1.LIB, FA_4B.LIB and FA_4D.LIB.
None contains matching AI/BI/PT/NT/OT entries. Thus there is no cross-archive
collision for those resource classes among these four installed archives.
This does not cover loose overrides, disc subdirectories or other builds.
The current host importer in `tore-import` (`crates/tore-import/src/import.rs`) reads FA_1 and FA_2 and
rejects differing duplicate selected resources; FA_4B/FA_4D are optional music
inputs. This is host behavior, not proof of original archive precedence.

## Behavior source points

The IDs below point to prose rules in [the main spec](../spec/ai.md). Script
labels refer to the hashed FA_2.LIB source, not the USNF-ATF engine. Executable
entries were inspected statically; unreviewed consumers remain explicit.

| Spec ID / subject | Source points | What was inspected; next boundary |
| --- | --- | --- |
| B01/B02: target geometry | `_CTEval_tgtoffbeam` `0x465120`, `_CTEval_tgtahead` `0x465150`, `_CTEval_tgtfacing` `0x465180`; helpers `0x411af0`, `0x411b60` | Heading/pitch error construction, absolute magnitudes and strict/inclusive comparisons; target-acquisition producer remains separate |
| B03: distances/height | `_CTEval_hrzdisttotgt` `0x4651d0`, `_CTEval_disttotgt` `0x465220`, `_CTEval_alt` `0x465510` | Horizontal query equalizes the vertical coordinate; all three convert fixed8 position results; ground query selection remains open |
| B04/B05: performance | `_CTEval_canclimb` `0x465380`, `_CTEval_betterspeed` `0x465460`, `_CTEval_bettertwr` `0x4654a0`, `_CTVarDiff` `0x464de0` | Own-minus-target query values and comparison thresholds; exact speed/TWR API conversion remains open |
| B06: target control | `_CTEval_tgthumancontrol` `0x465020` | Reads target instance +0x10 bit 0x80; same object-relative flag as the own G-exemption check, but ownership producers still need tracing |
| B10: invocation and missile reactions | `_CTEval_do_*` `0x464e30..0x464e8d`; `F.AI` labels `nothing`, `hit`, `ir_launch`, `radar_launch`; `_PLANEEventProc` `0x49df40` | Reason codes/accessors and active source requests; event eligibility, target identity and preemption remain open |
| B11: defensive selection | `F.AI` labels `bestEvade`, `jjj1..jjj3`, `last_ditch`, `jj0`, `jj1` | Conditional source guards and unsuitable-candidate redraws; full maneuver dynamics not recovered |
| B12: approach/pursuit | `F.AI` labels `plane`, `notWingApp`, `p100`, `bestAttack`, `homeOnTarget`; heading/pitch difference accessors `0x465750`, `0x465770` | Active decision conditions, inclusive random comparisons and distinction from commented ambitions; compiled branch correspondence remains open |
| B13: motion requests | `F.AI` labels `straight`, `straightClimb`, `straightDive`, `turnAround`, `breakLeft`, `breakRight`; `_CTDo_move` `0x465cc0` | Source requests and clamps in helpers `0x465c90`, `0x465d40`, `0x465da0`, `0x465de0`, `0x465e00`; command builder `0x4ac510` distinguishes timed from geometric completion |
| B13: timing boundary | `0x463af0`, `0x463b90`, completion evaluator `0x4382d0` | Constructor/deadline inspected with existing [command evidence](native-strip.md#condition-evaluation-and-saturated-command-time); physical clock units now traced below, completion exceptions remain |
| Engagement pitch | `_CTEval_engagep` `0x4657a0..0x465899` | Reads target altitude, bounds it by own limits, applies an aircraft-flag branch, selects pitch bands, clamps and suppresses a positive result if climbing is unavailable; full host rule remains open |
| B14: surface attack | `F.AI` labels `groundAttack`, `boring1`, `interesting`, `popup`, `fastHigh`, `diveBomb`, `exitRun` | Source branch probabilities, range thresholds and actual versus commented weapon checks; offset frame, dive rollout, jink and exit closed in [strike maneuvers](#strike-maneuvers-steering-frame-and-orbit) |
| B20: family differences | Corresponding labels in `F117.AI`, `H.AI`, `B.AI`, `AC130.AI`, `LARGE.AI`, `LINER.AI`, `MOTH.AI` | Active source handlers inspected; AC-130 circle, helicopter pop-up and bomber pass numbers in [strike maneuvers](#strike-maneuvers-steering-frame-and-orbit); no claim of full compiled equivalence |
| B30: surface | `HYDRO.AI` labels `attack`, `p1`, `p2`; `_GVProc` `0x473db0`, `_GVEventProc` `0x473f50`, `GVDoCurrentWaypoint` `0x473de0` | Script and callback separation established; the larger event handler was located, but per-class targeting/fire decisions remain open |
| B40: weapons/lifecycle | `_NPCWeaponsProc` `0x4736f0`, service `0x4c4700`, `_WNGSendWM` `0x45ed90`, `PLANEDoCurrentWaypoint` `0x49d580`, `PLANECheckFuel` `0x49fb70` | Weapon-service calls and device consumer established; wing, fuel and route entries are leads, not complete recovered behavior |

### Clock, pursuit and service follow-through

This continuation uses the same hashed FA build. It closes selected contracts,
not entire routines or a runnable original AI pipeline.

| Spec rule | Reviewed source points | Evidence boundary |
| --- | --- | --- |
| B13: duration units | `0x486e20..0x486e7d`, `0x486bf0..0x486c52`, existing `TIMEUpdate` `0x486aa0..0x486be9`, constructor `0x463af0..0x463b73` | Frequency import initializes `0x4fc928`; counter difference is multiplied by 256 and divided by that frequency through `__alldiv`. TIMEUpdate accumulates simulation delta and derives `0x5528c8` by shifting six, hence quarter-second counts. Mode 8 multiplies duration by four. Pause/compression affect simulation accumulation. |
| B13: geometric completion | `0x4ac572..0x4ac611`, `0x43830c..0x43839a`, `0x438427..0x438453` | Builder selects one angular axis and requests both comparison bits. Pitch completion includes minimum-speed and avoidance/state branches; no invented universal angular tolerance. |
| B15: target frame | `0x4662d0..0x466384`, `0x436c70..0x436deb`, `_Rotate2` `0x4c6654..0x4c66c8` | Homepos builds target mode 3; table at `0x438228` maps it to the target-position consumer. Offset expansion shifts eight, horizontal rotation uses target +0x1d, and Y is unchanged. Hostile-target prediction calls `0x4c0710` with selected weapon metadata; that calculation is continued in B44 below. |
| B15: speed regulation | `0x437ecb..0x43805d`, target scalar-speed read `0x436cdf..0x436cf5` | Inert ten-entry table at `0x4382a8` maps mode 8 to `0x437f2d`. It measures against the unoffset target snapshot, subtracts the operand magnitude in fixed8 feet, selects bands and clamps through COMaxSpeed/COMinSpeed. Speed units and aircraft minimum exemption remain open. |
| B41: retention | `_PROJSelectTarget` `0x4c4100..0x4c4181` | Own actor kind, target validity, distance and policy bit `0x20000000`; non-aircraft shortcut calls terrain blocking. The shortcut does not perform a fresh full seeker scan. |
| B41: eligibility | `Reaction` `0x464040..0x4642fb`, `NPCSetReact` `0x474650..0x47473d`, usable-store helper `0x474740..0x4747bf`, candidate prefix `0x4c4390..0x4c455f` | Side bit, instance validity/state, class masks and seeker checks established. `0x474740` requires positive store count and excludes hardpoint-type flag 2; no universal detection radius inferred. |
| B41: ranking | `0x4c45bd..0x4c46b6`, wing count `0x45ef20..0x45efac`, allowance `0x45eef0..0x45ef1c` | Ordinary distance penalties, separate priority bypasses and surface ranking overrides. Policy pointer `0x50cf68` bits 8/16 control allowance, with null/default returning 100; mission-facing names unresolved. |
| B42: service transitions | `_PROJServiceWeapon` `0x4c4700..0x4c4fef` | Deadline and hostility gates; search/preparation/lock/fire states; retries at `0x4c4b0c`, `0x4c4e18`; pre-fire delay variation at `0x4c4fa5`. Launch call is `_PROJFire` at `0x4c2170`, with the ordinary ammunition path continued in B45 below. |
| B42: imported timing fields | `0x4c49cd..0x4c4a9e`; `aircraft_schema.rs` NPC layout | `0x50d316/17/18` map to `searchFrequencyT`, `unreadyAttackT`, `attackT`. Twelve PT directive streams were matched to schema field order. Values live in the behavior spec, not a new aircraft identity table. |
| B42: weapon profile | `0x4c4a85`, `0x4c4d77..0x4c4f74`; OBJECT+PROJECTILE layout | +0xe5 is trackT; +0xf1/f2 burst counts, +0xf3 burst interval, +0xf4 reload, +0xf5 startup count. Equipment flags add branches. Field names alone do not close the launch envelope or burst policy. |
| B42: blocking | `0x4c48c8..0x4c48fb`, `0x4c4b1a..0x4c4b41`, `_COLTerrainBlocking` `0x42e4e0..0x42e52e` | Wrapper queries the line between own and target position and reports the collision result. Firing delays when blocked; full collision sampling/resolution is not newly recovered. |
| B43: command eligibility | `0x4665e0..0x4666fb`, spacing setters `0x466700..0x466796`, `_WNGPart` `0x45e710`, `_WNGWingmen` `0x45e6e0` | Sender membership/leader gate, member target comparison, event requests and spacing clamps. Receiver semantics are continued in B46 below. |
| B43: formation motion | `_WNGFormationMove` `0x45e970..0x45eaaf`, setters `0x45eb70..0x45ebee` | Table-scaled offsets, changing component offsets, previous-deadline advancement, nominal command duration and distinct speed mode 9. Formation table names and complete speed mode 9 remain open. |
| Experience: device timing | `0x49e287..0x49e2d6`, `0x47399d..0x4739ef` | Initial bound-1 random draw, count/selector, finite weapon deadline delay and successful-release reschedule. No device inventory or warning-delivery closure claimed. |

Offset packing at `0x463be0..0x463c49` repeatedly halves signed components until
all absolute values are below 128, storing a common shift. This explains small
quantization of requested offsets; do not reproduce the encoding as a host API.
The speed dispatch table was read as inert dwords, not disassembled as code.
The previously exploratory slice beginning `0x437dc0` was replaced for these
claims by an aligned decode starting `0x437ecb`.

The duration mapping does not erase the existing deadline saturation at 0x7fff
or establish behavior through every long-session clock wrap. The gameplay spec
states nominal simulation seconds and scheduling qualifications; whether those
original boundary artifacts are observable enough to reproduce is unresolved.

The wing assignment routine has an additional caution: `0x45ee73..0x45ee9b`
contains a distance comparison followed by a Boolean-valued comparison. Do not
claim a straightforward 20000-foot wing-command cutoff from the literal alone.
It is not needed for the established target-ranking penalties.

### Steering, seeker, release and receiver follow-through

This pass supports B44 through B46. All addresses use the same baseline build.
Local bounded decodes and their hashes are indexed in `closure/manifest.json`.

| Spec | Source points | Established boundary and remaining work |
| --- | --- | --- |
| B44: axis consumers | `0x4374ac..0x437daa`; rate helpers `0x478090..0x478142` | Heading, pitch and bank progression, bank-dependent authority and conditional reduced rates. Aircraft helpers call performance lookup `0x477ed0`; turn additionally calls `0x476aa0`. Table selection, terrain helper `0x42df80`, state names and mode dispatch coverage remain open. |
| B44: lead | `0x4c0710..0x4c0816` | Projectile flag `0x4000`, 20000-foot early exit, 1600-foot close-range adjustment, attitude-based 10/35-degree ramp and non-aircraft vertical offset. Speed estimator `0x477d50` and prediction-time units need follow-through. |
| B45: geometry | `_PROJInFOV` `0x4c2860..0x4c2b4d` | Zone selection at seeker +0x0f/+0x23; separate range, vertical and angular checks. Geometry is not evidence for unlimited target knowledge. |
| B45: effective range | `0x4c2b50..0x4c2e36` | `COSig` at `0x478200` supplies initial percentage. Aspect, look-down and speed branches modify it; final division is distance times 100/percentage. Zero gives the maximum signed-distance sentinel. Global `0x50ce28` and signature producers are not assigned speculative gameplay names. |
| B45: launch/support | `_PROJLock` `0x4c2f20..0x4c31e2`; emission helpers `0x4c2eb0..0x4c2f14`, `0x4c31f0..0x4c324d` | Launch-context G gate applies to human aircraft. Projectile flags `0x700` require launcher validity; `0x200` has emission/support checks, `0x400` requires compatible seeker selection. This routine passes zone 1 to InFOV. Trace projectile guidance callers before assigning flight-zone or support-loss semantics. |
| B45: release | `_PROJFire` `0x4c2170..0x4c24a0`; `_HARDUnload` `0x4527f0..0x452865` | Target clearing versus release refusal, station inhibit, finite debit, unlimited sentinel and player-only global bypass. Projectile creation at `0x4c23ad` follows debit; allocation failure returns false without observed rollback in this routine. Special store helper `0x4c58a0` remains open. |
| B46: receivers | `_PLANEEventProc` branch `0x49e9e8..0x49f184`; helpers `0x49f7e0`, `0x49f810`, `0x49f830` | Subcodes 5/6 break/approach; 7/8 spacing; 9 formation; 10 control; 11 target/policy. Receivers and sender conditions are distinct. Concrete-target deadline at `0x49ef05..0x49ef65` includes a global time-adjustment branch. |
| B46: approach builder | `0x4ac880..0x4ac8de` | Builds a target-relative request and adds a distance-related completion predicate. Full consumer/mode semantics still need tracing, so no guessed approach duration or completion distance is specified. |

`_PROJLockUpdate` at `0x4c0960..0x4c0a84` updates player-targeted incoming
projectile counts on a two-second schedule in the inspected branch. Its name
is not proof of a generic lock-memory timer. Warning delivery and classification
still require their callers. Similarly, wing receiver return values must not
be renamed acceptance acknowledgments: applied setters and no-motion branches
have different return conventions.

### Performance, terrain, warning, countermeasure and route follow-through

Closed on 2026-09-17 against the same build. Bounded decodes and a SHA-256
manifest are under local `session3/`. Object positions are 1/256 ft; the tick
clock `0x5528c8` counts quarter seconds; time of day `0x5528e0` is seconds.
Type kind bytes: aircraft type 5, aircraft instance 4, projectile type 7,
projectile instance 6. The PLANE extension starts at `cpt+0xba`.

| Spec | Source points | Established boundary and remaining work |
| --- | --- | --- |
| B04/B13/B15: speed units | `HUDDrawSpeed` `0x407f45..0x407f6e`; `_CTEval_speed` `0x4653a0`; `_COMinSpeed` `0x477d10`; `_COMaxSpeed` `0x477e50`, `_MaxSpeed` `0x477d50..0x477e42`; `_COCornerSpeed` `0x477d30`, producers `0x4524a9..0x452522`; pursuit read `0x436cdf`; gravity term `0x437d20..0x437d84` | `cp+0x34` is fixed8 feet per second (HUD multiplies by 3600/6076 for knots); envelope words `cp+0x23f/0x241/0x243` are integer fps. Corner floor 110 fps only for type flag bit 8 (not F-18 or Rafale). Projectile maximum scales by `performanceAt0/At20` through 19968 and 39936 ft. B05 scale open |
| B44: performance selection | `_COBrv` `0x477ed0..0x478032`; `_COBankRate` `0x478090..0x4780cb`; `_COTurnRate` `0x4780d0..0x478142`; `@GToTurn@8` `0x476aa0..0x476ade`; `0x4c6620`; `_COTurnRadius` `0x478150..0x47818a` | `_brv` block at `cpt+0xe2` with loaded G at `cp+0x259`, damage/hit-point/load reductions. GToTurn = G(fixed8)*2500/speed in degrees*256/s, so 2500*G/V deg/s, speed floor 125, cap 40 deg/s; bank rate x182 with 0x7fff above 180 deg/s; radius 10433*V/rate ft, cap 32767 |
| B44: terrain floor | `@COLPitchToAvoidTerrain@0` `0x42df80..0x42e0b5`; pitch consumer `0x437744..0x43776c`, `0x43789b..0x4378a4`; completion `0x438318..0x43833c`; event `0x46361e..0x46362e`, handler `0x49e4dd..0x49e553` | 1000 ft look-ahead, `minAlt` `cpt+0x75` clearance (^300 in inspected PTs), 1.375 R sin(p) test in 5 degree steps, 1 s / 0.25 s cadence, +20 deg/s authority, immediate pitch completion, 3 s `maxClimb` (`cpt+0x61`, 80 degrees) recovery masked while motion commands run (mask writers `0x466389`, `0x466019`, `0x466149`, `0x466240`, `0x45eaa0`, `0x45f7d0`, `0x49f511`, `0x49f582`). Event delivery frequency open |
| B44: overrides | ceiling `0x437722..0x43773b` (`maxAlt` `cpt+0x79`); airport cap `0x437772..0x4377af`; ground `0x4377af..0x4377f6`; flag 0x1000 dive `0x437808..0x43783f`; bank-scaled pitch `0x437846..0x437898`; ground turn floor `0x437562..0x437576`; bank rate `0x437c26..0x437c8c`; bank bound `0x437bd8..0x437c13`; gravity `0x437cf9..0x437d9d`; heading authority `0x4374db..0x437557` with `maxBank` `cpt+0x65` | State byte `cp+0xe3`: 1..0x12 and 0x13..0x1e airport-attached, 0x0a..0x10 airborne takeoff part, 0x1f free flight. Airport word `+0x10b` is the 4 degree airfield pitch cap (see [airfield sequences](#airfield-takeoff-and-landing-sequences)). Bank bound second term open |
| B13: completion and interruption | `0x4382d0..0x438453`; `_CreateMoveGoal` `0x463af0..0x463b71`; `_CancelCmdBuf` `0x463e50..0x463e94`; `_CTExecProgram` `0x466970..0x466a74`; `_CreateMove` `0x463a20..0x463a2e`; event raiser `0x4635f0..0x46371e`; `_MaybeCallEventProc` `0x463980..0x4639ae`; handler exits `0x49f48d`, `0x49f533`, `0x49f59c..0x49f5c5`; wing cancels `0x49ec77`, `0x49ed29`, `0x49ed8d`, `0x49eff3` | Goal kinds 0..8 (kind 8 seconds x4); free-flight pitch exception at min+25 fps; one motion per script run replaces the current command; event 0x80 on buffer exhaustion; waypoint fallback only after the script declines. Gate chain `0x49e048..0x49f42a` and events 0x2000/0x800 open |
| B47: warning producer and delay | `_PROJAdd` `0x4c0a90`, send `0x4c0f49..0x4c1021`; `_MSGSend` `0x4180a0` record `0x4182e8..0x418341`; delay `0x4c0f67..0x4c0ffd`, table `0x50ce18` = 24/12/4/0; delivery `_MSGReceive` `0x4185a0`, `CheckForEvents2` `0x4636a1..0x4636bf`, `MessagesToPlayer` `0x414555..0x414585` | Message type 0x400 to the target handle only, owner must be this computer. Base 24 (12 or 4 in states 0x20/0x21), 4 per 0x294000 (2 statute miles) capped 80, experience add, human cap 4, floor 2 quarters |
| B47: receiver gates and maneuver | `_PLANEEventProc` `0x49e0f4..0x49e3b6`; gate `0x49f7e0`; abort `0x49f810`; `_HARDFindStore` kind 9 at `0x49e1c5`; hold time `0x50cf81` (writers `0x4a76a1`, `0x47cfeb`, `0x4280c6`) | Human message ids 0x19/0x1a/0x1b behind `0x50d322` bit 2 (open); same side / current target return without maneuver; `Reaction(...,0x400,0xa)`; reasons 4 (IR) / 3 (radar); fallback reversal `0x7ff8`/`0x8008` at corner speed |
| B47: devices | `PROJLaunchDevice` `0x4c39a0..0x4c3aed`; `_PROJRetargetMissilesOnDevice` `0x4c3af0..0x4c3c31`; consumer `0x47398a..0x4739ef` | Kind-9 stores; selector chooses count byte +6/+7, device kind 3 (radar) / 2 (IR), message 0xc/0xd; empty store skipped, none left returns 0; decrement unless human with `[0x4eb6f8] & 8`. Decoy roll `Percent(susceptibility * effectiveness / 100)` for missiles with matching seeker class targeting the releaser; time shortening `0x4c3bdf..0x4c3c10` open |
| B47: reason ranking | `_CTExecProgram` `0x466991..0x4669a8`; restart `0x464cd0`, `0x464d93`, `0x464dc0`; producers `0x49f5ab` (0), `0x49f53c` (1), `0x49f49a`/`0x474353` (2), `0x49e355` (3/4), `0x49e979` (5) | Saved reason >= new resumes; else restart. Effect on an in-flight move command open |
| B48: waypoints | `PLANEDoCurrentWaypoint` `0x49d580..0x49d6dd`; `_WPMaybeAdvance` `0x499680..0x499832`; `_WPSetupCurrent` `0x4993c0..0x49963a`; `_WPPos` `0x4999b0`; usable-store `0x474740` | Record layout (+3 position kind, +4 goal, +6..+0x11 position, +0x12 speed, +0x14.. formation, +0x1a.. reaction); 0xea6000 (60000 ft) landing trigger, 0x138800 (5000 ft) / 0x6400 (100 ft) leader jitter, 0x7d000 (2000 ft) floor, speed clamp, 5 s goal; completion by octant, goal dead or no usable weapons, on ground. Modes 0xa/0x4a open |
| B48: tick states and landing join | `0x49e0d5..0x49e0ed` table; targets `0x49f47c`, `0x49f185`, `0x49f1a6`, `0x49f239`, `0x49f351`, `0x49f3ba`; join `0x49f289..0x49f347` | State 0 crash; 1..0x12 takeoff handler; 0x13..0x1e landing; 0x1f free; 0x20/0x21 producers open. Join within 0x271000 (10000 ft) of leader and 0x9c4000 (40000 ft) of its airport |
| B48: fuel | `PLANECheckFuel` `0x49fb70..0x49fccf`; caller `0x452589`; cruise `0x49f7b0`; wingman bingo `0x49e07f..0x49e0d0`, `0x49f6b0..0x49f7a6`; out of fuel `0x49e084`, `0x49f456..0x49f47b` | Flags 0x40000/0x20000/0x8000/0x10000 at 0, 240 s, home+300 s, home+600 s; cruise min+(max-min)/5 if >= 75 else min+(max-min)/2; private landing waypoint at 5000+Rand(5000) ft for an AI wingman with an AI leader; that route ends in a Land goal (34), so the aircraft lands and stays parked ([airfield sequences](#airfield-takeoff-and-landing-sequences)). Leader/singleton RTB and damage disengagement open |
| B30: surface events | `_GVEventProc` `0x473f50..0x474300` | Dispatch on the same event masks; 0x8000 subcodes 7/8/9/10 call setters `0x45f8a0`, `0x45f8e0`, `0x45f860`, `0x45f7f0`; 0x400 skipped for human control then a 2-of-100 gate; 0x200 applies `NPCSetReact` and reason 0x1f. Command operands (0x3c/0x10/0x8, 0x180/0x3ffc) and event meanings open |

### Airfield takeoff and landing sequences

Static trace on 2026-09-23 of the FA.EXE hashed in [the baseline](../baselines/ai-research.md)
(re-verified, 1319424 bytes; SMS names below). Decodes, the annotation helper and notes are in
local `ground-start-research/`. Nothing here was run against retail. Units: positions
and distances 1/256 ft, speeds `cp+0x34` fixed8 ft/s (command speed operands are
whole ft/s), angles 65536 per turn (182 per degree), command deadlines quarter
seconds. The airport record fields and anchor points are in
[native STRIP notes](native-strip.md#remaining-template-callback-boundaries).

Both handlers run from `_PLANEEventProc` state dispatch (`0x49f185`, `0x49f224`)
and queue a command list; the list's end (event 0x80) calls them again, so each
state below is "check, else queue the next leg". A state's move list often sets
the next state immediately, so the state number leads the aircraft's position.
Move records are `_CreateMove` `0x463a20` (four channels: steer, pitch or offset,
bank, speed; one or two completion goals, B13) with record words +0x14/+0x16 as
heading and pitch offsets of the nose from the flight path (steering
`0x437626..0x437a51`; the helicopter climb uses path +90 degrees with nose -90).

| Takeoff state (`APTakeoff` `0x4badb0`) | Source | Behaviour and numbers |
| --- | --- | --- |
| Every call | `0x4bae2e..0x4bae3c`; `_T_InitPlane` `0x4a7355..0x4a736f` | Gear commanded down. An aircraft created on the ground starts in state 1 with takeoff flaps down; an airborne one in 0x1f |
| 1 hold | `0x4bae98..0x4baf4d`, hold `0x4bb314` | Stopped (speed 0) and re-checked every 5 s. Leaves when it is this aircraft's turn (`0x4bc020`) and the runway is free (`_APStripFree` `0x4bac70`) and `@APTakeoffType@8` `0x4baa10` is not -1: to 2 (runway), 0xa (helicopter, type flag 8) or 0xd (STOVL on a vertical pad or already on the takeoff spot). Within 475 ft (`0x1db00`; 175 ft carrier) of the takeoff spot only the turn gate applies |
| 2 start taxi | `0x4baf52`, `0x4bb34a..0x4bb486` | Pivot toward anchor 0 at zero speed for at most 6 s (deadline 24), taxi to it at the airport taxi speed (+0xf9, 50 ft/s, 30 kt) until passed, stop dead (`0x4bc110` zeroes speed), set 3. Within 475 ft of the takeoff spot it skips straight to 3 |
| 3, 4, 5 taxiway | `0x4baf69`, `0x4bb027`, `0x4bb0df`; legs `0x4bb48b`, `0x4bb544`, `0x4bb5f7` | Steer at 50 ft/s toward anchor 1, 2, 3 in turn, re-planned every 1 s. A leg ends within 50 ft (`0x3200`) of the line through that anchor running 10000 ft back along the reversed runway heading (`_DistToLine` `0x412200`), or within 475 ft of the takeoff spot |
| 6 line up | `0x4bb197`, `0x4bb6a7..0x4bba39` | More than 26400 ft (5 statute miles) from the spot: stop and return to 1. Within 150 ft (125 carrier): pivot in place until heading equals runway heading. On the centerline (within 50 ft): taxi to the spot, stop within 10 ft. Otherwise aim at the centerline point 50 ft closer than the current distance: heading error of 45 degrees or more stops the aircraft and pivots it, 5 to 45 degrees taxis at a quarter speed (12.5 ft/s), under 5 degrees at 50 ft/s. On the spot (ground contact within 125 ft, callback +8) with exactly runway heading: enter the takeoff type and set throttle 0 |
| 0x11 runway roll | `0x4bbe08..0x4bbeed`; end `0x4bc1a0` | Runway heading, pitch 0, wings level: speed request minimum speed until reached, then maximum speed for 5 s, then maximum speed for 4 s with the nose 4 degrees above the path (+0x10b), then 0x12. AI flaps go down on entering 0x11 (`EnterState`) |
| 0x12 climb out | `0x4bbeef..0x4bbf53`; done `0x4bbfe0` | Runway heading, path pitch 4 degrees (+0x10b), maximum speed, 2 s legs, until 650 ft (`0x28a00`) above terrain. Then autopilot off, gear up, flaps up, hook up, throttle 100 without afterburner, thrust vector 0, state 0x1f |
| Airfield pitch cap | `0x437772..0x4377f6` | In states 1..9 and 0x11..0x12 the requested path pitch is capped at +0x10b (4 degrees); on the ground the path cannot pitch above the ground attitude until speed exceeds minimum speed |
| 0xa..0x10 vertical and STOVL | `0x4bbbe7..0x4bbdb7`, `0x4bb20b..0x4bb2c1` | 0xa: 5 s hold, throttle 101 (afterburner if fitted), nozzles -90; 0xb: vertical climb at 25 ft/s to 800 ft AGL (500 ft helicopters, which then finish); 0xc: accelerate to minimum+73 ft/s. 0xd..0x10 short takeoff: runway heading to minimum speed, 10 degree climb to 250 ft AGL, then to minimum+73 ft/s. Nozzle angles -60/-40/-20. Not required for conventional jets |
| 7..9 carrier catapult | `0x4bb1f4`, `0x4bba74..0x4bbbe2`, `0x4bc120`; speed `0x49f876..0x49f8f4` | Hold on the catapult 5 s, catapult sound, then speed grows by +0xff scaled by service time, clamped to +0xfb..+0x103. A human launch rolls `Percent(1)` for flag 0x4000, which plays a grind sound at the stroke end and holds deck speed at 73 ft/s; reading it as a failed shot is unconfirmed. Carrier template values are not decoded here |

Turn gate `0x4bc020`: in a wing (`_WNGPart` result 2), every member earlier in wing
order must be past state 3 (not in 1..3). If the wing leader is the player, members
after it also wait while the player is in 0xa..0x11. The player is in state 1 when
stopped on the ground and 0x11 when rolling at 7 ft/s or more (`_ServicePlayer`
`0x41731b..0x417403`), so AI wingmen of a grounded player wait until the player
is airborne. Runway-free gate `0x4bac70`: no other aircraft attached to the same
airport may be on the ground within 125 ft of the takeoff spot, and none may be in
states 2, 3, 6..0xa, 0xd..0x11, 0x14..0x19 or 0x1b. States 1, 4, 5, 0xb, 0xc, 0x12,
0x13, 0x1a and 0x1c..0x1e do not block. The state test is skipped while
`_curScreen` is 3 or 0xc (screen identities open). There is no timed interval
between aircraft: spacing comes only from these two gates, re-tested every 5 s.

| Landing state (`APLanding` `0x4bc270`) | Source | Behaviour and numbers |
| --- | --- | --- |
| Entry | `0x49d5df..0x49d5ff`; join `0x49f289..0x49f347` | Current waypoint with goal bit 2 within 60000 ft (B48), or wingman join (10000 ft / 40000 ft, B48): state 0x13 |
| Wing abort | `0x4bc2a4..0x4bc2d1`, `_WNGLeaderLanding` `0x45e8f0`; tick `0x49f1a6..0x49f202` | A wing member still in 0x13 or 0x14 whose leader is neither landing nor on the ground takes the takeoff-finish tail (gear and flaps up, free flight) |
| 0x13 marshal | `0x4bc394..0x4bc3d1`, hold `0x4bc4be..0x4bc554` | Cleared (to 0x14) when a parking slot is held (`_APAssignPark`/`_APGetPark`), every living earlier wing member is on the ground (`0x4bd1e0`), the runway is free (`0x4bac70`) and `@APLandingType@8` `0x4baac0` is not -1. Otherwise hold: fly to a corner of a square centred on the landing point with corners 52800 ft (+0xed, 10 statute miles, 8.7 nm) out on each world axis, chosen by the aircraft's octant (`_Octant` `0x411cb0`, tables `0x50ca78`/`0x50ca88`): north octants go to the +X+Z corner, east to +X-Z, south to -X-Z, west to -X+Z, and within 26400 ft of the point to +X+Z. This circulates +Z to +X to -Z to -X, clockwise from above if +X is east. Altitude 6000 ft (+0xf1) plus 1000 ft (+0xf5) per wing position, absolute; speed (maximum+corner)/2. Re-planned every 5 s. Helicopters: square 10000 ft, speed (3 maximum+corner)/4 |
| 0x14 approach | `0x4bc559..0x4bc6e4`, end `0x4bc210` | Gear down, hook down at wire-equipped airports. Three gates on a 6 degree path (+0x10d) back along the landing heading from the landing anchor: 35200, 17600 and 8800 ft out (D x 4/6, 2/6, 1/6), about 3680, 1840 and 920 ft above it. Each gate ends within 250 ft horizontally. Speed (maximum+corner)/2, then at most 366 ft/s (217 kt) from the second gate. Then 0x15 (0x18 vertical types, whose gates start 500 ft higher and add a fourth at 6000 ft, at most 146 ft/s) |
| 0x15 final | `0x4bc3d3..0x4bc468`, `0x4bc6e9..0x4bc77b` | Fly at the landing anchor at no more than 293 ft/s (174 kt), nose 17 degrees above the path (+0x10f), re-planned every 1 s. Touchdown when on the ground or the anchor is 90 degrees or more off the nose: 0x17 (runway) or 0x16 (arrested: human with wire airport and hook down; AI at a carrier with a hook-capable type). No separate flare state |
| 0x17 rollout | `0x4bc46d`, `0x4bc89d..0x4bc994` | Brakes on, flaps down; landing heading at 146 ft/s (86 kt) for 2 s with the nose 8 degrees up, 2 s more, then speed 0 until stopped; below 1 ft/s go to 0x1b |
| 0x1b clear runway | `0x4bcc6b..0x4bce02` | Hook up, brakes off, flaps up, object flag 0x100. Without a parking slot: stop and re-check every 10 s. Otherwise pivot (up to 14 s, only when anchor 4 has a smaller world Z than the aircraft, an axis-bound quirk) and taxi to anchor 4 at 50 ft/s (100 ft/s for type flag 0x20), stop, and set 0x1c at once, so the runway counts as free while it still rolls to anchor 4 |
| 0x1c..0x1e taxi back | `0x4bce07`, `0x4bcf1b`, `0x4bd035` | Pivot up to 4 s and taxi at 50 ft/s to anchors 5, 6 and 7, stopping at each, then 0x1a |
| 0x1a park | `0x4bca6f..0x4bcc66`; slot `0x4be200` | Slot k position is box 0x19+k; heading is the airport heading plus 90 degrees. Pivot up to 10 s, taxi to the slot, stop, turn to the slot heading. Within 200 ft: throttle 0; if the current waypoint goal has bits 0x22 (the Land goal 34) the aircraft stays parked permanently, otherwise it returns to takeoff state 1 |
| Airport lost | `0x4bd170..0x4bd194`; takeoff `0x4badf4..0x4bae23` | Airport object dead: an aircraft in landing states 0x13..0x15 takes the takeoff-finish tail and flies on; one in 0x16..0x1e or in takeoff states 1..8 is removed (`_Kill`) |
| Vertical and carrier landings | 0x16 `0x4bc780`, 0x18 `0x4bc999`, 0x19 `0x4bca19`, `_APEndArrestorCatch` `0x4bc240` | Arrest at +0x107 deceleration then state 0x1a; vertical types descend to the anchor at 50 ft/s with nozzles -80, then hold 5 s |

The shipped AI approach and final fly 3 degrees instead of the recovered
6 degree path (+0x10d), to the player's ILS aim point 1,000 ft past the
threshold instead of the landing anchor, matching the player's ILS (opinionated, requested by
John on 2026-09-29; [AI airfield sequences](../spec/ai-airfield.md)). The
recovered value in the table stays the retail evidence.

The player takes the landing states from `_ServicePlayer` `0x417420..0x4174ca`: gear down,
below 4000 ft AGL, no faster than 953 ft/s and within 25000 ft of the nearest friendly
airport puts the player in 0x14 (0x12 just after a takeoff, changing to 0x14 when
aligned with the approach, callback +0x10). Because 0x14 blocks the runway-free gate,
AI aircraft in marshal wait while the player is in that condition; this is the
manual's "hold marshal while you land" rule. AI landers are sequenced by wing order,
the runway-free gate and a parking slot per aircraft; between different wings it is
whichever marshal aircraft next finds the runway free on its 5 s re-check.
While not in an airport state an aircraft is attached to the nearest airport of its
side where its type can land (`@FMUpdatePlaneFields@0` `0x4525d5..0x45261b`,
`_APNearest` `0x4ba8e0`) and claims a parking slot there.

Open: `_curScreen` 3/0xc identities; carrier template values and catapult timing;
what mission-start placement does for grounded aircraft that are not on a slot or
the takeoff spot (the reviewed TRAIN01 grounded aircraft sit exactly on them);
the AI throttle behaviour between the throttle 0 at line-up and the speed requests.

### Formation and wing-order follow-through

| Spec | Source points | Established boundary and remaining work |
| --- | --- | --- |
| B43: formation table and names | data `0x4f6cb8..0x4f6d6b`; readers `0x45ea15`, `0x45ea1d`, `0x45ea3b`; `_GRPFormationMove` `0x45f745..0x45f76b`; name tables `0x4f0780` (`0x428cf0`), `0x4ff1e8` (`0x48d877`); control names `0x4f078c`, `0x4ff1f8` | 3 formations x 10 slots x (lateral, vertical, longitudinal) signed words; 0 Echelon, 1 Line abreast, 2 Line astern; control 1 loose, 2 medium, 3 tight |
| B43: axis convention | `_Rotate2` `0x4c6654..0x4c66c8`, sine table `0x515a48`, projection `0x4120c0..0x412147`, consumer `0x436da7` | +x right, +y up, +z ahead; horizontal pair rotated by leader heading |
| B43: player spacing and waypoint defaults | `0x415a72..0x415a8f` (512/2048), `0x415b66..0x415b96` (0/+512/-512); `_WPSetupCurrent` `0x4994dc..0x4995fa`; idle defaults `0x49f5d3..0x49f60b` | Waypoint fields +0x14 control, +0x15 formation, +0x16 H, +0x18 V |
| B43: script control quirk | `_CTDo_wm_control` `0x4667e0..0x46680f` | Jumps to the formation setter `0x45eb30`; no extracted script uses `wm_control` |
| B43: speed mode 9 | `0x437fa5..0x43801e`, error source `0x4371ac..0x4371cb`, flag `0x437400..0x4374a7` | Mode 8 bands on 3D distance to the slot; negative bands behind a 2000 ft lead-projection flag whose entry condition is open (`0x437250..0x437400`) |
| B46: approach completion | `@MVRApproachTarget` `0x4ac8c9..0x4ac8d9`, `_CreateMoveGoal` `0x463b15..0x463b6a`, evaluator `0x4383fb..0x43844b` | Goal kind 6, operand 2000 ft; steering point consumer `0x436df0..0x436e8d` open |
| B46: order map | receiver `0x49e9e8..0x49f184`; player senders `0x4157a7..0x415bd4`, `0x416200..0x41637f`; script senders `0x4665e0..0x4666f3`; broadcast `0x45eec4` | Subcodes 5 break (175/170 deg, 70 deg pitch), 6 approach (45/35 deg), 7/8 spacing, 9 formation, 0xa control, 0xb target (0 hold, 0x1fffffff free, bit 0x40000000 protect, else concrete with 20 s deadline `0x50cfa4`), 0xc class, 0xd second attack state (sender not found), 0xe/0xf waypoint, 0x10 bug out (`0x49ecc0..0x49ecf4`: ignored in airport states 1..0x1e or on the ground; otherwise the private RTB route of `0x49f6b0` then `@NPCSetStateTarget@8` `0x4736b0` with state 0x1f and no target, i.e. disengage), 0x11 leader rebroadcast, 0x12..0x14 goal events |
| B46: control side effects and sharing | setter `0x45eac0..0x45eae7`; `_WNGSendWM` `0x45ed90..0x45eee3` gate `0x45edbc`; cap `0x45eef0..0x45ef1c`, count `0x45ef20..0x45efac`; receiver gate `0x49f0db` | Quiet-flag control changes per order; broadcast refused at control >= 2; 20000 ft compare at `0x45ee73`/`0x45ee96` is dead |
| B46: radio path | poster `0x4180a0..0x41859c`, `@SAYMsg` `0x48d350`, say proc `0x48d780..0x48e0b9`, phrase tables `0x4ff170..0x4ff538`; replies `0x49ef91..0x49efb0` (0x15 "Engaging", radio bit 0x100000 of `0x4eb6f8`), `0x49ee68..0x49ee80` (0x23 "Showtime!") | Sender-side voicing "YOU: '<phrase>'"; player-side voicing of received replies open |
| B46: rejoin and idle | `0x49f239..0x49f657`; formation service `0x45e8a0..0x45e8e0`; exits `0x49f3f3`, `0x49f362`, leader `0x49f40e`; disengage `0x49ed9a..0x49edb8` (hold bit 2 of `0x50cf5e`) | Idle state re-issues the formation move; no rejoin distance or timer |

### Seeker, signature and store-selection follow-through

| Spec | Source points | Established boundary and remaining work |
| --- | --- | --- |
| B45: zone layout and roles | `_PROJInFOV` `0x4c2860..0x4c2b4d`; zone 0 callers `0x4094b1`, `0x43e1eb`, `0x4c4558`; zone 1 callers `0x4c30d2`, `0x4c314e`, `0x4c5473` | Zone 0 at seeker +0x0f (JT +0xbb), zone 1 at +0x23 (JT +0xcf), stride 0x14: horizontal +0, vertical +2, min range +4, max range +8, min altitude +0xc, max altitude +0x10; sentinels 0x80000000/0x7fffffff; both angles 0x7fff bypass angles. Matches the committed `Zone` reader. Unreviewed callers `0x4165a1`, `0x49032d` |
| B45: signature producers | `COSig` `0x478200..0x47848e`; ECM `0x452f10`; `CPComputeRCS` `0x43e8c0`; weather `_WRWeatherEffects` `0x4b4720` | `sigs[]` at type +0x3f indexed by seeker sig byte; emitter branch uses deadline +0x11a; IR doubles with a 200 floor; radar adds 33 and 25 with 100 floors; visual lift 200..1500 ft; night divisor between clock 0x6270 and 0x10c5c. Instance +0x16f bit 0x20, word +0x274 and option byte `0x50d276` bit 8 open |
| B45: aspect and look-down | `0x4c2b97..0x4c2bf6`, helper `0x4c2e40..0x4c2ea5`, `_AnglesOffNose` `0x411b60`; look-down `0x4c2bf8..0x4c2d25`, `0x43e1a0`; amplifier `0x50ce28` writer `0x43e1ca`/`0x43e1f0` | Rear cone 0x1c70 (40 deg) / 0x6388 (140 deg), vertical bypass 30 deg; type +0xba bit 1 gates the exemption (open); look-down 45 deg and 5000 ft; amplifier only for human viewer and contact behind `0x4eb6fc` bit 0x20 and `0x4eb604` > 1 (open) |
| B45: closing-speed gate | `0x4c2d27..0x4c2dbc` | SEE +0x09 bit 4 enables; unused by roster radars |
| B45: target class and store preference | `0x4c52d0..0x4c5562`; `_HARDFindStore` `0x452f80`; `_HARDPtrs` `0x452770`; `_PROJHitChance` `0x4c3380` | Class word +0x0d masked 0xC000 against store flags 0x10000/0x20000; up to ten candidates; score = angle term + hit chance + 50 beyond 0x5dc00 (1500 ft) for flag 1 + damage[category]/25; highest wins under byte `0x50d31d`. Station bit 2 and hit-chance internals open |
| B45: in-flight support | `PROJMoveProc` `0x4c12f9..0x4c1373`; `_PROJLock` `0x4c2f20..0x4c31e2`; `PROJSetTarget` `0x4c0870`; `_PROJRadarIsOn` `0x4c2eb0..0x4c2f14`, surface `0x4c31f0..0x4c324d` | Launch context 0, range check 0 in flight; failure clears the target without destroying; emitter exception `0x4c1303..0x4c133d` for target speed under 0x2400; AI launcher extends deadline to clock + 0x28. Terminal branch `0x4c1379..0x4c13d8` (+0x19b, `0x50d38f`) open |

### Strike maneuvers, steering frame and orbit

Static read on 2026-10-11 (AI strike round, slice ST-R) of the FA.EXE hashed in
[the baseline](../baselines/ai-research.md) and the `F.AI`, `H.AI`, `B.AI` and
`AC130.AI` sources in FA_2.LIB. Nothing was run against retail. Units as in
the sections above: positions 1/256 ft, angles 182 per degree (65536 a turn),
command durations whole seconds on the quarter-second clock (B13).

**Operand order.** Script operands are popped in source order (`0x465ad0` pops
the interpreter stack at `0x546bf2`; `0x466290` pushes a value back). The
readers: `0x465d40` heading wrapped to 0..359, `0x465c90` pitch clamped to
plus or minus 90, `0x465da0` bank or angle clamped to plus or minus 180 (or the
"any" sentinel), `0x465e00` speed clamped to `_COMinSpeed`..`_COMaxSpeed`,
`0x465de0` duration clamped to 0..15. `_CreateMove` `0x463a20` records four
channels (steer, pitch or offset, bank, speed: kind byte plus value) and the
completion goal; its word argument (`+0x1e`) carries flags: 0x10 weapon lead
for a hostile target (`0x436cf6`), 0x08 far-point pitch smoothing (`0x4371cf`),
0x800 no target-heading rotation (`0x436d85`), 0x200 no `maxDive` floor
(`0x4372dc`).

| Subject | Source points | Finding |
| --- | --- | --- |
| `homePos` frame (B15) | `_CTDo_homepos` `0x4662d0..0x466384`; consumer `0x436c70..0x436deb`; lead `_PROJGetTargetPos` `0x4c0710` | Operands x, y, z, speed, duration. Steering point = target position (lead-adjusted for a hostile target when a store is selected, flag 0x10) plus the offset in feet: x to the target's right, z ahead of the target, both rotated by the **target's** heading (`+0x1d`, `_Rotate2`); y straight up from the target's own height, never rotated. So `homePos 0 500 0` aims at a point 500 ft above the target object, and `homePos 0 0 0` at the target itself. The aircraft flies straight at that point (heading and pitch from `_Angles` `0x437193`), so a far approach is a straight descending or climbing line, not "fly low, then": a level hold appears only because pitch requests within 3 degrees snap to 0 while the point is 3,000 ft or more away (`0x437358..0x43738a`). Pitch is clamped to `maxClimb` and `maxDive` (PT `cpt+0x61/+0x63`, 80 and -80 degrees on 110 of 145 records) and, for type flag 0x80 (`cpt+9`), to (speed - minimum) x 0.31 degrees per ft/s, at least 5 (`0x4372ec..0x437351`). Offsets are packed with a common shift until every component is under 128 (`@ObjPlusDeltaParm` `0x463be0`), so 5,000 ft is flown as 4,992 ft and 10,000 as 9,984 |
| `homeAngle` (pop-up pull) | `_CTDo_homeangle` `0x4661a0..0x46623b`; consumer `0x436df0..0x436e57`, offsets added `0x43727e..0x43729b` | Operands heading offset, pitch offset, speed, separation, duration. Heading = bearing to the target plus the offset; pitch = line-of-sight pitch plus the offset. A non-negative speed operand is a plain speed and the separation operand is ignored. The pop-ups (`F.AI` `homeAngle 0 25 0 corner 4`, `H.AI` `homeAngle 0 30 0 corner 6`) therefore request **minimum speed** (operand 0) for the pull; the `corner` they meant sits in the ignored slot. Player-visible: the pop-up bleeds speed for 4 s (6 s helicopter) |
| Pop-up final leg | `F.AI` `pu2`, `H.AI` `pu2` | `homePos hToTgt 0 0 corner 1`: the first offset is the bearing in degrees used as feet, so the aim point sits 0 to 359 ft to the right of the target in the target's frame (a script quirk, under a wingspan at the end of the run) |
| Dive bomb, entry and roll-in | `F.AI` `diveBomb` `db1`, `db2` | Entry only when altitude AGL and horizontal distance are both at least max(10,000 ft, 2R) (R = `turnRadius`, `_COTurnRadius` `0x478150`: speed over turn rate, 2,500 G / V degrees a second, so R = V^2 / (43.6 G) ft; 600 ft/s at 7 G gives 1,180 ft). Approach: steer at a point H = max(2R, AGL at entry) above the target at corner speed in 1 s legs until horizontal distance < 1.5R. Roll-in: hold the bearing to the target at that moment, level, bank 180 (inverted, explicit bank is not limited by `maxBank`: kind 1 at `0x437a90`), corner speed, 1 s legs, until horizontal distance < R |
| Dive bomb, dive and rollout | `F.AI` `db3` and the three lines after it; `_CTDo_invert` `0x4664c0`, `_MVRInvert` `0x4ac760`; `_MVRMove` `0x4ac510` goal axis `0x4ac572..0x4ac60e` | Pull to pitch -90 inverted (geometric completion), flown at `maxDive`, -80 degrees on every fighter and attack jet; hold in 1 s legs while AGL > max(2R, 2,000 ft). Then `invert`: only when attitude pitch is beyond plus or minus 88 degrees it relabels heading and bank by 180 degrees (same physical attitude, no motion); at -80 it does nothing. Rollout: heading = roll-in heading + 180, pitch 0, wings level, corner speed, until the slowest axis arrives (geometric goal), which flies as a split-S pull-through to level on the reciprocal heading. Then 25 degrees nose up, wings level, corner speed, for 10 s; then the exit run |
| Exit run and jink | `F.AI` `exitRun`; `_CTDo_jink` `0x4663f0..0x466489`; `_MVRJink` `0x4ac9e0..0x4acae8` | Jink operands: heading, pitch, heading step, pitch step, re-randomise flag, legs (clamped to 4 at `0x466437`), speed, seconds per leg. `_MVRJink` queues the legs at once: heading alternates between H + s and H (cumulative toggle, side chosen by one `Rand8` bit), pitch alternates P + p and P - p (sign by a second bit), bank automatic. `F.AI`: inside 10,000 ft horizontal, H = current heading + `random 20 - 10` (-10..+9), P = 10 to 14 degrees, s = 10, p = 0, 4 legs of 3 s at maximum speed, so a 12 s climbing weave of 10 degrees to one side, repeated while still inside 10,000 ft. Beyond 10,000 ft: hold heading, 10 degrees up, maximum speed, 5 s legs. At max(30,000 ft, 2R) horizontal: turn back to the target level at corner speed (geometric) and restart the attack choice |
| Evade with a ground target | `F.AI` `bestEvade`; `_CTDo_movetoalt` `0x465e20`; `_MVRMoveToAlt` `0x4ac680..0x4ac72e`; far-point pitch `0x4371cf..0x437278` | `moveToAlt h max(AGL, 20000) maxSpeed 5`: the altitude is **absolute** (world height), clamped to `minAlt`..`maxAlt` (`cpt+0x75/+0x79`); a goal below 2,500 ft is instead taken above the higher of the ground below and the ground 2,000 ft ahead (flag 0x40, `0x436f16..0x436fae`). The aim point is 100,000 ft ahead on the current heading and 500 ft beyond the goal altitude; flag 0x08 steers through a point 5,000 ft ahead with pitch held within plus or minus 20 degrees. So: same heading, maximum speed, climb at up to 20 degrees toward 20,000 ft above sea level, re-issued every 5 s while the evade reason lasts |
| AC-130 circle | `_CTDo_circle` `0x4660c0..0x466149`; consumer `0x436e5c`, `0x437046..0x43718b`; bank blend `0x437b2d..0x437b5d`, bound `0x437bd8..0x437c13` | `circle 0 4000 2000 35 %a 5` operands: direction (0 left, else right), radius 4,000 ft, height 2,000 ft, bank 35 degrees, speed (corner + maximum) / 2, 5 s. Centre = the target's current position (so it follows a moving target) raised 2,000 ft. With d the horizontal distance from the aircraft to the target and R the radius, the requested heading is the bearing to the centre plus 90 - 45 (d - R) / R degrees, clamped to 45..135 (left orbit, counter-clockwise from above, headings increasing clockwise as in `_Rotate2`); pitch is toward the raised centre, so the orbit settles 2,000 ft above the target. Bank blends from the heading-error bank toward 35 degrees by an on-orbit score Q = 100 - 100 x heading error / 30 degrees - 100 x abs(d - R) / R (floor 0), then is bounded by min(`maxBank`, max(20 degrees, the open second term)): the AC-130's `maxBank` is 30 degrees (PT 5460), so it banks 20 to 30, not 35. Its `maxClimb`/`maxDive` are plus or minus 20 degrees. The command lasts 5 s and the script `restart`s it for as long as the attack reason holds. On the orbit the target is 4,472 ft slant, 26.6 degrees below the horizon, abeam left: a third of the guns' 13,000 ft zone. The radius and height are script constants, not derived from gun reach. Direction 1 would negate the heading offset but not the bank (unused) |
| Helicopter pop-up | `H.AI` `groundAttack` | Aim 1,500 ft above the target until 10,000 ft, then pull to line-of-sight + 30 degrees for 6 s at minimum speed (operand quirk above), then at the target until 1,500 ft or not ahead; exit 5 degrees up at maximum speed in 5 s legs to max(20,000 ft, 2R); turn back 5 degrees up at corner speed |
| Bomber high pass | `B.AI` `groundAttack` | Aim at max(AGL, 10,000 ft) above the target at corner speed in 2 s legs while the target is ahead (B01); exit level at maximum speed in 5 s legs to max(50,000 ft, 2R) horizontal; turn back level at corner speed |
| Ground target assignment (B41) | Masks `0x50cf74` attack, `0x50cf76` defend, `0x50cf78` evade, policy `0x50cf70`, `searchDist` `0x50cf7a`; `_T_AddObj` `0x4a7660..0x4a76ae`; `_WPSetupCurrent` `0x499480..0x4994b5`; `NPCSetReact` `0x474650..0x47473d`, `AmmoForClass` `0x474740`; `_PreferredTargetId` `0x4644f0`, group `0x45e630`; `_PROJSelectTarget` `0x4c4210`, `0x4c42e3..0x4c4378`, `0x4c44aa..0x4c4561`, ranking `0x4c45bd..0x4c46a7`; `InSearchArea` `0x464420` | Each aircraft carries three class masks (IGNORE is "in no mask"), a policy dword and a search distance, set from the mission object (`react`, `searchDist`, `preferredTargetId`) and replaced by each waypoint (`w_react`, `w_searchDist`, `w_preferredTargetId`, record `+0x1a..+0x21`). Class bits (type `+0x0d`): 0x8000 fighter, 0x4000 bomber, 0x2000 ship, 0x1000 SAM, 0x800 AAA, 0x400 tank, 0x200 vehicle, 0x100 structure, 0x40 other; an aircraft under 10 ft/s reads 0x40 (`_CTEval_tgtclass` `0x464f50`). The masks drop surface classes when no store with flag 0x20000 has rounds (air classes need 0x10000), re-run on event 0x200. The **preferred target** (the mission's target object; bit 0x80000000 a group, 0x40000000 protect, 0x20000000 no retention) is tested first, outside the class and box filters: hostile, alive, ammunition for its class, and seen by a seeker's zone 0, then chosen at once with no distance ranking. Ordinary search: surface candidates in a square of plus or minus 26,400 ft around the aircraft (about 25 and 50 miles for some own flags, open), reaction ATTACK, inside `searchDist` statute miles of the leg line (0 anywhere), zone 0 view (a visual seeker skips its angle test in states 0x20/0x21), lowest of distance + 10,000 ft if not an aircraft + 10,000 ft per wing penalty; terrain vetoes |
| Reactions to state | `Reaction` `0x46417c..0x4642e8`; switching `0x46419a..0x4641e2`; free flight `0x49f239..0x49f27f`; fallbacks `0x49f4a5..0x49f51d`, `0x49f547..0x49f582` | Search event: class in ATTACK gives attack (state 0x20, script reason 2), in EVADE gives evade (0x21, reason 1) unless `Ignore()` drops it. Warning or hit events: ATTACK or DEFEND makes the shooter the target (evade if no ammunition for it), EVADE evades. A current target of a higher class word is kept; an equal one is kept 10 percent of the time. If the script declines: attack homes on the target at 1,000 ft separation (speed mode 8); evade flies away from it for 5 s |
| Threat perception | `Ignore()` `0x463fa0..0x46403d`; warning handler `0x49e10d..0x49e11b`, `0x49e303..0x49e355`; hit `0x49e936..0x49e979`; evaluators `0x464e90`, `0x464ed0`, `0x465000`, `0x465010`, `0x4650b0`, `0x4650f0` | The only threat geometry: a ship threat is ignored beyond 40,000 ft; any other beyond 158,400 ft (30 statute miles); between 20,000 ft and 30 miles it is ignored unless this aircraft is within 100 degrees of the threat's nose; inside 20,000 ft it is always evaded. No weapon ranges or threat rings are read. A missile warning runs `Reaction` on the **launcher**: a launcher class in no mask gets no maneuver (countermeasures were already rolled by skill), ATTACK or DEFEND retargets it, EVADE evades. `tgtIsSam`, `tgtIsAAA`, `tgtAttackingMe`, `tgtAttackingAnyone`, `tgtRadar`, `tgtIR` exist but the strike scripts use none. Speed reaches the pass choice only through `turnRadius` |
| Quick Mission strike wings | wing writer `0x432240` (`react` line `0x43255c..0x432564`, masks `0x4322ba..0x4322ee`); calls `0x4316ed`, `0x43176f`, `0x4317ee` (friendly), `0x431833`, `0x4318b4`, `0x431936` (enemy) | Retail Quick Mission writes per-member `react` masks and `searchDist 0`, never waypoints or a preferred target. Fighter-class types (class 0x8000, which includes the A-10, A-7, AV-8B and Su-25) get air only. Any other type (bomber class 0x4000: A-4E, A-6, F-117, B-52, AC-130U, AH-64, Mi-24, Su-24, Tornado) in a **friendly** wing gets ATTACK and DEFEND 0x2700 (ship, tank, vehicle, structure) and EVADE fighters when a ground target is chosen (`0x5373d8`), so it self-tasks on ground units seen within about 26,400 ft; never SAM or AAA (no retail SEAD). Enemy wings never strike. How these wings reach the target area is open (`wng` fields and spawn positions `0x431744..0x43190b`) |
| Skill in the strike path | `chance` `0x466e00`; `0x4c4fa5..0x4c4fb8`; `0x4c49e2..0x4c49f6` | `groundAttack` uses `percent` and `random` only, so skill does not change the pass. The weapon service has no skill input (a flat `Percent(10)` adds 2 quarter seconds before firing; `unreadyAttackT` or `attackT` by a state flag). Skill reaches strikes only through the documented channels: countermeasure odds, the Novice/Average 1 G loss, and the B.AI/AC130.AI evade "go straight" at 40/25/10/5 percent |
| AC-130 guns on the orbit | mount aim `0x4c24b0..0x4c26e1`; zone 1; aim tolerance `0x4c4c09..0x4c4c8e`; lead `0x4c0710` | The AI's guns slew within the hardpoint arcs as half-widths (C_25 plus or minus 60/60, C_40 45/45, C_105 25 heading and 45 pitch about the left-facing neutral), which confirms the reading the gunsight spec labels fitted. On the circle the target is near azimuth -90 and a few degrees above the left wing line (26.6 degrees depression against 20 to 30 degrees of bank), inside every arc, at 34 percent of the 13,000 ft zone; while converging C_25 and C_40 still bear, C_105 only near abeam. Firing details are in [weapons](weapons.md#ai-release-against-ground-targets). Whether the rounds hit (aim 20 ft above the target with no drop term; about 17 ft low at 4,470 ft) is open: `_PROJHitChance` `0x4c3380` and collision sizes |
| Wing orders, keys | FlightKey `0x414690` compare chain `0x4147ec..0x4148a0`; senders `0x416200..0x4162bf` (Alt+E 0x1204, Alt+R 0x1304), `0x4162c0..0x41637f` (Alt+F 0x2104), `0x41586f..0x4158e4` (Alt+W 0x1104) | Alt+E and Alt+R send subcode 0xb with the player's current target `0x50cf6e` when it is of the other side, else the free order 0x1fffffff; no aircraft or class test, so a hostile ground target is sent. **Alt+F** is the same sender with the player contact `0x522c10` (`@SetPlayerContact` `0x4143f0`, set by `/` and `\` at `0x414a64`, `0x415150` only while `_gamePrefs` `0x4eb6f8` bit 0x2000000, IR/Laser advanced targeting, is on; also by a cockpit pick `0x439d74`). Laser and IR seeker stores fire at the same contact (`_ServicePlayer` `0x416fbd..0x417021`). With no hostile contact Alt+F sends the free order, so the manual's "attack on contact" is its fallback. Alt+F does not re-test the option. Alt+W sends subcode 0xc with the current target's class word (`+0x0d`, any side). Replies: "Attack" plus name and clock position, plain "Attack" for the free order, "Attack" plus the class plural for Alt+W (`0x4ff0a8`: bandits 0xc000, ships 0x2000, SAMs 0x1000, AAAs 0x800, tanks 0x400, vehicles 0x200, structures 0x100, missiles 0x80, else targets) |
| Wing orders, receiver | `_PLANEEventProc` `0x49ee8f..0x49efb0` (0xb concrete), `0x49edc7..0x49edf9` (free), `0x49efc4..0x49f041` (0xc class); gates `0x49f7e0`, `0x49f810`; `_PROJSelectTarget` retention `0x4c4100..0x4c4181`, class scan `0x4c42e3..0x4c4378`; `_PreferredTargetId` `0x4644f0`; `_CTEval_tgtisplane` `0x464fd0` | A ground target is accepted: the receiver stores the order, sets state 0x20 and the current target at once (`0x45ebf0`), with a 20 s order deadline `0x50cfa4`; for a non-aircraft target the class include masks are cleared, so the ordered object is the only candidate. It is kept while alive and within 20,000 ft. Re-picking it still needs the object alive, a reaction other than IGNORE (`Reaction(..,0x80)` not 0x1f), a seeker zone-0 view (`0x4c4558`) and no terrain block. Reply "Engaging" (0x15) from wing part 1 only. `tgtIsPlane` is true only for a class 0xc000 object moving at 10 ft/s or more, so the script runs `groundAttack` (a parked aircraft counts as ground). The class order scans a box of about plus or minus 26,400 ft around the wingman for the class. The deadline is in time-of-day seconds; a target change to something other than the ordered object after it has passed sends a wing message (`0x45ec71..0x45ed0d`, `0x45f9a1`), whose content is open |

### Quick Mission skill boundary

Closed on 2026-09-17. The six skill fields use dword globals `0x537374`,
`0x537380`, `0x53738c`, `0x5373b8`, `0x5373c4`, `0x5373d0`, read at
`0x4316c2`, `0x431749`, `0x4317c8`, `0x431808`, `0x43188e`, `0x431910` and
passed as argument 8 to the wing text writer `0x432240`. The writer emits
`"\tskill %d"` from `[esp+0x5c]` at `0x432538..0x432548` inside the member
loop (`0x4325bf..0x4325c1`) and never modifies that argument. Enemy wings OR
0x80 into the per-member `nationality2` value (`0x4322fb..0x432307`); a
`controller $%x` line (0x80 plus player slot) is emitted only for members with
a player slot (`0x4324a6..0x4324ee`).

The mission text parser `0x481c10` tokenizes on whitespace (`0x483d10..0x483d29`),
zeroes a 0x382-byte record on `obj` (`0x48247c..0x48248e`, default skill 0),
stores the `skill` number unchanged into record +0xe2 (`0x4828d8..0x48290b`),
stores `controller` into record +0x10 with a computer-count check
(`0x482846..0x482891`), and `_T_AddObj@12` copies +0xe2 to the object skill and
+0x10 to the control byte (`0x4a7654..0x4a765a`, `0x4a746f..0x4a7478`). Every
writer of the object skill byte was enumerated: only `_T_AddObj` and
`CPSetSkill` run on the play path. The 33/67 applier `0x42a656` is called only
from the editor's `?MAPOnSpecial@@`; `_ChangePlaneType@12` `0x454292` preserves
an existing value via editor, repair and multiplayer callers.

Header keywords `usgroundskill`, `usairskill`, `themgroundskill`,
`themairskill` (`0x4822a0..0x482399`) store to `0x54bd98`, `0x5527fc`,
`0x54e46c`, `0x5516e8`; their only readers are the editor and the multiplayer
header resend (`0x49618b..0x4961ca`). `CPSetSkill` `0x43de90..0x43dedc` sets
+0xe2 for every category 4 object with +9 bit 0x80 (enemy). Callers:
`_FlightMenu` `0x475abc` (0) and `0x475b18` (1) after the dialogs at `0x4f7e18`
and `0x4f7d78`, setting `_gameMultiPrefs` `0x4eb6fc` bits 0x1000/0x2000; and
`usnfmain` `0x40438f..0x4043a8`, which tests those bits after the mission's
objects exist. Persistence of those bits to the prefs file is not traced.

Quick Mission ground templates: `<sam>` and `<aaa>` placeholders are gated by
`@Percent@4` with tables `0x4f3148` and `0x4f31b8` (0, 25, 60, 100) indexed by
fields 32 and 31 (`0x431e67..0x431e88`, `0x431f1a..0x431f5a`); family jump
tables `0x4321e4`, `0x4321f8` and a bounded pick `0x432210` choose the record.
Night (field 15 = 6, `0x4302a6`) with `F117.PT` or `B2.PT` in a friendly wing
(`0x431a20..0x431a98`) selects the ZSU-23 list `0x4f31b0` and rewrites the next
template `skill` token to 0 (`0x432144..0x4321ae`). Template skill values are
theater data; read them next via the resource name resolved around `0x431ab0`.

Control byte producers, complete: mission text (`0x482879`, `0x4a7478`),
`_OBJSetControl@16` `0x491810..0x4918c9` (sets `computer | 0x80`; callers
FlightKey `0x414c47`, editor `0x422341`, MPReceive `0x46e63d`, MPAssignPlayers
`0x471f75`, MPChangePlaneType `0x47292f`), editor `0x424e9e`, `0x424ea5`,
`0x427aa2`, and non-aircraft creation copies (`0x47cf6c`, `0x469aff`,
`0x4c0bd8`). No AI path sets bit 0x80 on an aircraft, so the `0x4523a2` G
exemption means human control, local or remote. Call frequency of
`_FMUpdatePlaneFields` (`0x452140` callers) is not traced.

## Reviewed executable contracts

Addresses below refer only to the hashed FA.EXE in the baseline. SMS symbols
locate entry points; instruction inspection, not names alone, supports claims.

| Evidence | Address | Finding and limit |
| --- | --- | --- |
| `_CTEval_skill` | `0x4656c0` | Returns the current object's byte at `0x50cf62`; valid level meaning is corroborated by the four-way consumers |
| Chance handler | `0x466e00..0x466e48` | Divides the packed decimal operand by 100 `3 - skill` times, then takes remainder modulo 100 and calls the percentage helper |
| Percentage helper and random bound | `0x4561a0`, `0x4561d0`, `0x4562f0` | Compares a bounded 0..99 draw with the threshold; bounded generator uses remainder, not an inclusive upper bound |
| `_CTEval_corner`, `_CTEval_cornerspeed` | `0x465400`, `0x4653f0` | Both jump to `_COCornerSpeed` at `0x477d30`; the older sentinel-versus-number uncertainty is resolved for FA |
| Editor assignment | `0x42a656..0x42a717` | Filters object category and side, adjusts selected skill at draw thresholds 33 and 67, clamps 0..3 and writes object skill |
| Skill transfer | `0x45428c..0x454292` | Copies a source-record byte at +0xe2 into current object skill; this alone does not identify the Quick Mission generator |
| `_FMUpdatePlaneFields` skill branch | `0x4523a2..0x4523f1` | Levels 0/1 adjust positive/negative G limits by 0x100 with bounds +/-0x200; exemption tests bit 0x80 at `0x50ce90`, the human-control bit (see the Quick Mission skill boundary) |
| `_PLANEEventProc` launch branch | `0x49e249..0x49e2d6` | Event subcodes 2/3 gate a device schedule using 35/50/75/90; records a count 2 or 3 and a selector bit |
| `_NPCWeaponsProc` device consumer | `0x47398a..0x4739f3` | Consumes the deadline/count, calls `PROJLaunchDevice` at `0x4c39a0`, decrements count and reschedules on success |
| `_GVProc` | `0x473db0..0x473dd7` | Returns `_GVEventProc` for selector 3 and `_NPCWeaponsProc` for selector 5; other selectors delegate to `_OBJProc` |
| `_CTDo_homepos` | `0x4662c0..0x4663d4` | Negative speed takes mode 8 with positive magnitude; the consumer traced below establishes separation regulation, not negative airspeed |

The same working address can represent different fields for different object
types. For example, projectile routines also reference `0x50cf62`. A global
address search is a lead list, not a list of aircraft experience effects.

The executable loop at `0x466970` and its caller sites were inspected for
research orientation. Exact resume/preemption semantics remain open. The old
claim that every action yields is not sufficient: opcode 04 at `0x466af7`
calls the stack-pop helper, while `homepos` itself sets stop flags when it
queues a command. Trace observable interruption/duration behavior rather than
copying the old interpreter's suspension model.

## Next trace map

| Missing player-visible contract | Start here | Completion evidence |
| --- | --- | --- |
| Quick Mission template ground skill and prefs persistence | Template resource name near `0x431ab0`; writers of `0x4eb6fc` bits 0x1000/0x2000 | Ground object skill values in theater templates; whether the enemy-skill override survives restart |
| Pursuit lead | Speed estimator `0x477d50`, `_PROJGetTargetPos` `0x4c0710` | Weapon lead estimator and prediction time, minimum-speed exemption producer. Offset signs are closed in [strike maneuvers](#strike-maneuvers-steering-frame-and-orbit) |
| Maneuver restart effect | `_AllocCmdBuf` `0x463d00`, `FinishCmdBuf` `0x463ce0`; gate chain `0x49e048..0x49f42a`; circle bank bound second term `0x437bd8` | What a script restart does to an in-flight move, events 0x2000/0x800, reason 1/2 gates. Jink and circle shapes are closed in [strike maneuvers](#strike-maneuvers-steering-frame-and-orbit) |
| Attack states and target choice | `Reaction` `0x464040`, `NPCSetReact` `0x474650`, `EnterState` sites `0x442a53`, `0x469657`; `0x45e8a0` | Producers of states 0x20/0x21, priority-policy route, retarget-policy flag |
| Fire control remainder | `_PROJHitChance` `0x4c3380`; `_HARDPtrs` `0x452770`; terminal branch `0x4c1379..0x4c13d8`; decoy timing `0x4c3bdf..0x4c3c10`; type +0xba bit 1; instance +0x16f/+0x274 | Hit-chance rule, station bit 2, 4000 ft terminal branch, decoyed-missile time shortening, aspect-exemption gate, hot-engine producers, `0x50d322` bit 2 |
| Wing remaining branches | `0x437250..0x437400`, `0x436df0..0x436e8d`, human event proc near `0x49fb40`, `0x464040`, readers of `0x50cfa4` | Mode 9 negative-band entry, approach steering point, reply voicing, loose/medium self-engagement, 20 s deadline expiry |
| Recovery, RTB and damage | Readers of `0x50cfef` bits (0x800000 set by the RTB route `0x49f6b0`), `OBJDamageProc` `0x473b40`, `_PLANECheckEject` `0x49fa10`; `_curScreen` values 3/0xc in `0x4bad11`; carrier airport records (`@APAddToCarrier@4` `0x4beb00`) | Leader and singleton return to base, damage-triggered disengagement, route modes 0xa/0x4a. Takeoff and landing sequences are traced in [airfield sequences](#airfield-takeoff-and-landing-sequences); their remaining gaps are the screen exemption, carrier values and mission-start placement |
| Surface threats and movement | `_GVEventProc` `0x473f50` (dispatch inspected), command builder `0x463a20`, setters `0x45f7f0..0x45f8e0`, `GVDoCurrentWaypoint` `0x473de0`, `_CARRIERProc` | Event mask meanings and command operands for surface groups; separate SAM, AAA, vehicle, ship, carrier and static-object contracts; experience effects demonstrated per class |

Resolve branch semantics against the corresponding BI only where the text and
engine leave a player-visible ambiguity. Keep disassembly and extracted modules
local. No executable, bytecode or source-script interpreter ships with this plan.
