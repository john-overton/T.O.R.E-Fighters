# Fighters Anthology AI source notes

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-17. This is a static trace map, not a proposal to execute
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
The current host importer in `tore-app/src/assets.rs` reads FA_1 and FA_2 and
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
| B14: surface attack | `F.AI` labels `groundAttack`, `boring1`, `interesting`, `popup`, `fastHigh`, `diveBomb`, `exitRun` | Source branch probabilities, range thresholds and actual versus commented weapon checks; offset frame and command consumers remain open |
| B20: family differences | Corresponding labels in `F117.AI`, `H.AI`, `B.AI`, `AC130.AI`, `LARGE.AI`, `LINER.AI`, `MOTH.AI` | Active source handlers inspected; no claim of full compiled equivalence or family flight integration |
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

### Quick Mission skill boundary

The six skill fields use dword globals `0x537374`, `0x537380`, `0x53738c`,
`0x5373b8`, `0x5373c4`, `0x5373d0`. Their labels/indices are already recorded
in [Quick Mission tables](quick-mission.md). Reads at `0x4316c2`, `0x431749`,
`0x4317c8`, `0x431808`, `0x43188e`, `0x431910` feed the wing text writer
at `0x432240`. Its last argument supplies the emitted per-record skill at
`0x432538..0x432548`, using the skill format string at `0x4f39d8`. The writer
loops over members without changing that argument. No per-member skill draw
was found in that writer.

This closes the UI-to-writer boundary, not the randomization contract. The
manual describes individual variation, and the separate editor applier exists.
Trace the generated text through the mission loader's field writer and any
post-load transformation before deciding that runtime levels are uniform or
jittered. The previously recorded `0x45428c` copy is a generic object transfer,
not proof that Quick Mission takes that path. Likewise `CPSetSkill` at
`0x43de90` is a separate bulk setter filtered by category/side, not the missing
random distribution. Do not silently select one of these candidates.

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
| `_FMUpdatePlaneFields` skill branch | `0x4523a2..0x4523f1` | Levels 0/1 adjust positive/negative G limits by 0x100 with bounds +/-0x200; exemption tests bit 0x80 at `0x50ce90` |
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
| Quick Mission selection to individual skill; persisted mission skill | Creator option readers, writes to source +0xe2 and object skill, mission parser around `0x482200` | Per-wing distributions, ground assignment, saved-value precedence and exact label ordering |
| Actor G exemption | `0x4523a2` and producers of `0x50ce90` | Human/remote/AI applicability, with no change to player adapters |
| Pursuit offset units and negative speed | `0x4662c0`, command consumer `0x436b30`, speed selection around `0x437ecb` | Remaining signs, speed-unit conversion, weapon lead, minimum exemption and observable pursuit trajectories |
| Maneuver timing and cancellation | `_CTDo_move` `0x465cc0`, `_CTDo_jink` `0x4663f0`, `_CTDo_circle` `0x4660c0`, command deadline `0x463b90` and executor callers | Remaining completion exceptions, jink/circle timing, threat interruption and route resumption |
| Threat classification and target choice | `_PLANEEventProc` `0x49df40`, `Reaction` `0x464040`, `NPCSetReact` `0x474650` | Seeker visibility, warning-delivery eligibility and priority-policy producers beyond reviewed retention/ranking |
| Fire control and device responses | `_NPCWeaponsProc` `0x4736f0`, weapon service `0x4c4700`, device launch `0x4c39a0` | Weapon selection, range/angle gates, support, salvo timing, ammunition, chaff/flare mapping |
| Wing commands and formation | `_WNGSendWM` `0x45ed90`, `_WNGFormationMove` `0x45e970`, `_CTDo_wm_*` | Command acknowledgments, formation spacing, breaks, bracket and rejoin |
| Aircraft routes and survival | `PLANEDoCurrentWaypoint` `0x49d580`, `PLANECheckFuel` `0x49fb70`, existing airport evidence | Patrol/escort/strike waypoints, fuel/damage disengagement, takeoff and recovery |
| Surface threats and movement | `_GVEventProc` `0x473f50`, `GVDoCurrentWaypoint` `0x473de0`, `_GVProc`, `_CARRIERProc` and shared weapons | Separate SAM, AAA, vehicle, ship, carrier and static-object contracts; experience effects demonstrated per class |

Resolve branch semantics against the corresponding BI only where the text and
engine leave a player-visible ambiguity. Keep disassembly and extracted modules
local. No executable, bytecode or source-script interpreter ships with this plan.
