# Aircraft variety import matrix

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation contract and progress matrix, 2026-10-05. This batch targets 23 additional retail
aircraft, all human-playable, using the hybrid model. John requested varied
transports, gunships, airborne radar aircraft, VTOL aircraft, helicopters,
fighters and airliners, and explicitly included all four retail F-4 variants.
Twenty is a starting size, not a ceiling. The other exact variant selections
below were agent proposals; John requested starting this batch on 2026-10-05.

The initial imports now have exact source identities, aircraft-owned hybrid
configurations, source cockpits and initial exterior rigs. All 23 have passed
source-backed flight checks and application headless loading/flight. The six
powered-lift types fly the VTOL overhaul's rigid-body physics (rotors, nozzles,
puffer jets, nacelles and an angle-of-attack wing), with separate vertical
landing, conversion, hover hold and start checks
([spec](powered-lift-flight.md), [baseline](../baselines/vtol-overhaul.md)). The intended result remains validated aircraft
plus a repeatable procedure for subsequent agents. Complete moving-surface,
special-system and multiplayer acceptance is tracked separately below. Milestones live in the
[roadmap](../ROADMAP.md#aircraft-variety-import); the reusable procedure lives in
the [aircraft import guide](../aircraft-import.md).

## Batch roster

Names and PT identities come from the [retail catalog](../formats/fa-catalog.md).
The review column identifies work for the requested behavior below, not claims
about retail capabilities. Source HUD pointers and other catalog numbers stay
in the catalog.

| Aircraft | Exact retail record | Family | Main behavior to review |
| --- | --- | --- | --- |
| C-130 Hercules | `C130.PT` | Turboprop transport | Propeller presentation, heavy-aircraft handling, takeoff and landing |
| AC-130U Spectre | `AC130.PT` | Gunship | Selected-target gun tracking, animated mounts and linked gun combinations |
| E-3 Sentry | `E3.PT` | BLUFOR airborne radar | Radar coverage, player sensor controls and sharing contacts |
| IL-76 Mainstay, retail name | `IL76.PT` | REDFOR airborne radar | Its own radar profile, controls and sharing contacts |
| E-2C Hawkeye | `E2.PT` | Carrier airborne radar | Turboprop handling, radar, hook and carrier limits |
| AV-8B Harrier II | `AV8.PT` | VTOL jet | Source-informed forces, vectored thrust with puffer jets and the manual's nozzle keys, wing matched to the conventional model |
| Yak-141 Freestyle-A | `YAK141.PT` | VTOL jet | Its own lift arrangement (nozzle and lift engines), fitted limits and the same nozzle keys |
| V-22 Osprey | `V22.PT` | Tiltrotor | Two proprotors, always-on conversion corridor protection, nacelle animation and mapped conversion controls |
| AH-64 Apache | `AH64.PT` | Attack helicopter | Rotor physics with torque, autorotation and hover hold, collective on the throttle controls, gun and stores |
| Mi-24 Hind-D | `MI24.PT` | Attack helicopter | Its own rotor physics (published power, stub wings), mapped controls, gun and stores |
| CH-47 Chinook | `CH47.PT` | Transport helicopter | Tandem-rotor physics (no tail rotor), animation and mapped controls |
| MiG-17F Fresco | `MIG17F.PT` | Early jet fighter | Source handling, gun aiming and equipment limits |
| F-4B Phantom II | `F4B.PT` | Carrier fighter | Earlier sensor and weapons configuration |
| F-4J Phantom II | `F4J.PT` | Carrier fighter | Radar differences and source gun-pod loadout |
| F-4E Desert Phantom | `F4E.PT` | Fighter | E-specific handling, gun and source desert exterior |
| F-4G Wild Weasel Phantom | `F4.PT` | Strike fighter | G-specific fuel and stores; anti-radar weapon support |
| A-7E Corsair II | `A7.PT` | Carrier attack jet | Attack stores, approach handling and hook |
| F-15C Eagle | `F15.PT` | Fighter | Its own performance, sensors and stores |
| F-16C Falcon | `F16C.PT` | Fighter | Its own performance, sensors and stores |
| F-104N Starfighter | `F104.PT` | Fighter | Source handling, approach and equipment limits |
| A-10 Thunderbolt | `A10.PT` | Attack jet | Gun, stores and low-speed handling |
| Boeing 747 | `B747.PT` | Airliner | Large-aircraft handling, cockpit and unarmed operation |
| Airbus A310 | `A310.PT` | Airliner | A second transport configuration and unarmed operation |

MiG-15, F-86 Sabre and F4U Corsair are absent from the reviewed 145-record
catalog. Keep them as additional-source or custom-aircraft candidates, not
aliases of available planes. The proposed Corsair here is the A-7E Corsair II.

## Per-aircraft progress

Source review and dependency closure are in the [inventory](../formats/aircraft-variety.md).
Flight results and fits are in the [flight baseline](../baselines/variety-flight.md).
"Rig reviewed"
means bounded CPU surface/device acceptance in the
[per-aircraft animation audit](../baselines/aircraft-animations.md), with explicit
fitted mechanics and unresolved devices. It does not add cockpit-mirror or GPU
appearance acceptance. "Source profiles"
means installed or absent equipment is explicit and default stores connect to
the shared systems. Sound references are loaded; audible acceptance remains separate.

"Integrated" means the expanded identities, powered controls, actual device
positions and gun metadata have serialization paths. It does not claim a
completed human LAN flight in every aircraft. Headless rows cover 1,200 app
ticks and the source-backed flight checks, not retail trajectory parity.
Keep complete validation evidence in feature baselines rather than per-check files.

| PT | Source | Hybrid flight and inputs | Exterior and cockpit | Systems and audio | Multiplayer | Validation |
| --- | --- | --- | --- | --- | --- | --- |
| C130 | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| AC130 | Reviewed | Passed | Rig reviewed; hook open | Source profiles | Integrated | Flight passed; poses reviewed |
| E3 | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| IL76 | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| E2 | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| AV8 | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| YAK141 | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| V22 | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| AH64 | Reviewed | Passed | Rotor/gear reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| MI24 | Reviewed | Passed | Rotor/gear reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| CH47 | Reviewed | Passed | Rotor/gear reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| MIG17F | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| F4B | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| F4J | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| F4E | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| F4 | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| A7 | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| F15 | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| F16C | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| F104 | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| A10 | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| B747 | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |
| A310 | Reviewed | Passed | Rig reviewed | Source profiles | Integrated | Flight passed; poses reviewed |

AC-130 linked fire and the tank side task have additional acceptance cases in
their linked contracts. New autonomous flight behavior is outside this batch;
the original roster's AI regression cases remain scoped to that roster.

## Fuel tank side task

John added [selectable external fuel tanks](external-fuel-tanks.md) on
2026-10-05. Fix the reported F-14 case, then apply the same loadout handling
to existing aircraft and this batch. Tank identity, installed quantity,
internal fuel, external fuel, shell mass and remaining fuel mass must stay
distinct. The linked draft holds the code review, open decisions and acceptance
matrix. Status: selection, separate shell/fuel accounting and accepted-load restoration
are implemented; the F-14 source probe checks zero, one and two tanks.
The follow-up [ordnance matrix](../formats/aircraft-ordnance.md) covers all
reviewed aircraft, omitted weapon stations and shared tank/weapon rows. John
limited that pass to the editor and weapon availability, with aircraft rendering
unchanged. Separate tank geometry and in-flight tank jettison remain open.
This is shared import/loadout work, not another aircraft in the 23-aircraft count.

## F-4 family contract

John proposed shared handling with fewer features on older variants. The
[source comparison](../formats/aircraft.md#f-4-variant-comparison) supports
substantial reuse: all four G-envelope tables match, and B/J have identical
decoded PLANE fields. E/G have different flight values from B/J; G also has
less internal fuel than E. Default sensors, weapons and exterior references
are variant-specific. A simple newest-to-oldest feature subtraction is not
established by the source.

Implementation choice: share reviewed family algorithms and shape
work where applicable, while each exact identity owns its validated
configuration. Preserve every source difference. A shared fitted coefficient
must be documented as fitted, not inferred from a shared envelope. Do not
substitute real-world F-4 equipment for the game's records. No decision here
requires four copies of the same numerical integration code.

The F4J's source SUU16 gun pod keeps one installed unit separate from ammunition.
The fitted host interpretation supplies 600 logical units, representing the
record's 1200 projectiles at two actual rounds per logical unit. Its 1702 lb
hardware mass remains when ammunition is exhausted, until explicit removal or
jettison. The source fixed-slot flag stays part of compatibility. The source
values, fitted interpretation and limitation to the reviewed source-installed
station live in the [source review](../formats/aircraft-variety.md). The
[loadout validation](../baselines/external-fuel-tanks.md) includes sustained
source-pod fire and accepted-load restart. This does not claim recovered empty
shell/ammunition mass or a separately identified pod mesh.

## VTOL and tiltrotor flight

John's direction, 2026-10-05: extract approximate forces and control behavior
where the retail data and recovered consumers support them, then fit the gaps.
Full recovery of the original force solver is not a prerequisite. Preserve
each aircraft's own mass, fuel, thrust and relevant control limits. Record the
evidence, units, rule and constants for every derived or fitted component.

Specify the player-visible behavior of vertical lift, fore/aft thrust,
attitude and yaw control, hover, climb/descent and forward-flight transition.
Review nozzle or nacelle travel, power response, weight/speed limits and
available low-speed control authority. Use source values where established;
choose and document fitted values where necessary. Keep transition continuous
and make the displayed nozzle/nacelle position agree with the simulated demand.

Each aircraft must be usable with the default controls. Define its accepted
hover, vertical takeoff/landing and conversion scenarios with numeric limits
in the numerical contract. Record known approximations without blocking delivery
on unavailable retail comparisons.

## Helicopter flight

John requested the same source-informed and fitted approach for helicopters,
with a relatively simple but believable model. All three helicopters must be
human-playable. Reuse supported stock controls, and add collective, power/thrust
or other axes where the player needs independent control that retail did not
provide. New axes and their defaults are authorized. The mappings are recorded
in the [input guide](../INPUT.md) and the physics in
[powered-lift flight](powered-lift-flight.md).

The implemented model (the VTOL overhaul, October 2026) flies a rigid body on
momentum-theory rotors with a rotor-speed state, so the rotor lift responds to
collective and available power, the disk tilt (cyclic) tilts the lift
direction, and translational lift, ground effect, power-limited climb and top
speed, autorotation, the vortex ring state, torque and retreating blade stall
all appear. It is a lumped-parameter model, fitted where no public figure
exists, not a claim about the original game or a blade-element engineering
model. Mass, fuel and reviewed aircraft limits are specific to each helicopter;
they are not jets with a low stall speed, and the Chinook's tandem rotors have
no tail-rotor torque.

On helicopters and the V-22 the throttle controls drive the collective and the
engines are governed (John, 2026-10-08), so the ordinary throttle no longer
supplies engine power separately. Give every supported
action a usable default mapping or button alternative and expose any additional
axis in the input editor. Do not require a particular joystick to take off,
hover, turn, translate, land or shut down.

## Default controls and input documentation

John requires the new aircraft controls to be mapped by default and listed in
the in-game input page. The [retail keyboard evidence](keyboard.md) confirms
Ctrl+arrows for thrust vectoring and 0 for returning vectoring to neutral.
Use those stock controls where applicable, after reviewing their axis meanings
and current binding conflicts. Their existence does not establish a retail
collective mapping or the force produced by a given nozzle angle.

| Deliverable | Requirement |
| --- | --- |
| Default maps | Provide normal keyboard and gamepad operation for supported actions, plus bindable joystick/HOTAS axes; preserve stock assignments where supported and give new actions explicit defaults |
| In-game input page | Include nozzle/nacelle, collective, power and gun-group actions as needed, with clear labels, aircraft applicability, remapping and axis calibration |
| Input behavior | Define rates, limits, neutral/reset behavior and button alternatives for axes; verify held-key release, modifier isolation, focus loss and aircraft-switch resets |
| Shared input state | Carry the new controls through fixed-tick simulation, host/client input, seat handoff and replay; local animation must not be the only consumer |
| Controls master list | Register actions in `crates/tore-app/src/input_catalog.rs` and applicable gamepad defaults in `input.rs`; regenerate [CONTROLS.md](../CONTROLS.md) with `TORE_UPDATE_CONTROLS_DOC=1 cargo test --locked -p tore-app controls_doc` when bindings land |
| Guides | Update [INPUT.md](../INPUT.md), [flight controls](../FLIGHT-CONTROLS.md) and the HTML keyboard map in the same implementation change |
| Keyboard-map tab | The new VTOL / Helicopters sheet in [tore-keyboard-map.html](../tore-keyboard-map.html) implements John's suggested extra tab; its mappings follow the implemented defaults |

The fifth keyboard-map tab retains the four existing sheets. The
[map conventions](../tore-keyboard-map-rules.md) and preview were updated in
place. The sheet shows implemented defaults and capability limits; navigation
and PNG, ZIP and PDF exports were checked.

## AC-130 guns and linked fire

John requested animated gun mounts that turn and track according to the selected
target, plus selectable combinations of weapons firing together at that target.
This is a requested opinionated capability. John reports that all guns are
modeled; verify the imported gun geometry, station identity, muzzle origins and
available articulation before claiming visual support.

Allow the player to select individual guns and link different combinations
into a firing group. The selected group uses one player-selected target and
one fire command. Proposed timing: ready guns with a valid firing solution
begin on the same simulation tick, then retain their own cadence, ammunition
and readiness rules. The [gunship contract](ac130-linked-guns.md) now specifies grouping controls,
source gun records and fitted tracking/slew limits. A dedicated gunner camera
remains a presentation question; existing target selection drives the mounts.

Animate articulated mounts toward their actual aim direction and emit shots
from the corresponding transformed muzzles. Review fixed versus movable mounts
and their arcs; fit articulation where necessary and record that choice. Define
feedback when a selected gun cannot bear, has no valid target or is out of
ammunition. Do not fire through the aircraft merely to keep a group synchronized.
Test individual guns and mixed groups, changing targets/groups, target loss,
trigger release, ammunition depletion, restart and multiplayer/replay agreement.

This is player-directed weapon tracking and linked fire. It does not authorize
autonomous target selection, gunship orbit control or AI pilot changes.

## Radar and cockpit selection

Retail data and recovered behavior guide every aircraft's sensors and cockpit.
Each aircraft must have an explicitly reviewed radar profile or an explicit
no-radar state. If no radar is installed, its radar instrument cannot scan,
detect or invent contacts. Airliners are a likely no-radar case to verify from
their own equipment, not a rule inferred from the aircraft's role. Keep visual,
infrared and any received data-link information separate from an onboard radar.
An unavailable instrument should clearly communicate its equipment status.

Every human-playable aircraft must resolve to a cockpit. Follow its explicit
HUD/art references and any evidenced retail fallback. John expects large
aircraft to commonly use the AC-130 cockpit; the catalog references guide which
ones do. Review null HUD pointers and the original fallback selection before
choosing a substitute. Where no retail choice can be established, record an
explicit opinionated choice of imported cockpit art. Never inherit the previous
aircraft's cockpit accidentally or borrow its sensors because the artwork matches.

Validate instrument availability and source telemetry, glass/HUD clipping,
mirrors where supported, and missing-equipment indications. A shared cockpit
does not grant radar, weapons or engine readings that the aircraft lacks.

## Hooks and aircraft animation

John requires an aircraft's hook to be included when it has one, along with
as much usable animation as the imported geometry allows. Review source
capability flags, shape branches and controls together; model a declared hook
and record any missing geometry or fitted visual solution explicitly.

Cover gear, doors, flaps, control surfaces, airbrakes, hook and applicable bays,
nozzles, nacelles, propellers, rotors and AC-130 mounts. Reuse original geometry
and textures. Source-derived motion is preferred where established; fitted
hinges, travel, rates and mixing are allowed and must be documented. Review
attachment transforms as parts move and keep presentation consistent with live
control/device state, including on remote aircraft and during replay.

Acceptance records should distinguish animated, static and missing parts with
reasons. Hook operation and animation are required where applicable; complete
carrier launch/arrestment remains a separate capability review.

## Remaining review and acceptance

| Area | Missing contract | Next review |
| --- | --- | --- |
| VTOL and tiltrotor | Human handling review and retail force evidence (none exists: retail never flew rotorcraft; the jets' hover was never measured) | Keep the tested [fits](powered-lift-flight.md#data-sources-and-fitted-values); John flies them and tunes the Easy flight physics numbers, then extend payload, weather and human control checks before changing constants |
| Helicopters | Human handling review across controller hardware and weather | Rotor effects (ground effect, autorotation, vortex ring, torque, retreating blade stall) are modelled; blade element and flapping dynamics stay outside the fit ([left out on purpose](powered-lift-flight.md#left-out-on-purpose)) |
| Input | Physical controller acceptance | Defaults, remapping and the fifth map sheet are implemented; verify actual HOTAS/gamepad operation in addition to synthetic input tests |
| AC-130 | Live aiming usability and wider engagement cases | Validate linked fire, barrel/muzzle agreement and blocked-gun feedback against the [contract](ac130-linked-guns.md); review a gunner view separately |
| Radar and shared contacts | AWACS sharing/operator behavior beyond existing sensors | Installed/absent profiles are reviewed; additional shared-contact behavior needs its own contract and acceptance |
| Cockpits | Wider display review and original fallback evidence | Current roots and null-pointer fits are explicit; verify every new cockpit on wide/tall windows and keep unsupported gauges unavailable |
| Hooks and animation | Textured GPU acceptance, unlocated AC-130 hook, individual rotor feathering and carrier-operation boundaries | All 37 playable profiles have bounded CPU pose acceptance; follow the [audit](../baselines/aircraft-animations.md) for retained source gaps and separate rendering/handling acceptance |
| Multiplayer | Selection, content compatibility, new control/state transport, seat handoff and replay | Trace client/server use of the shared aircraft type and every newly required control/state value |

Player-flyable helicopters extend the recovered retail multiplayer
[single-mission selection rule](multiplayer.md), which excludes helicopters
and blimps. This extension follows John's requested helicopter scope on
2026-10-05 and is opinionated. The requested simple flight model and additional
axes above define the scope; their initial numerical tuning and bindings are now documented fits. No AI pilot or autonomous target-selection changes are authorized.

## Completion and documentation

Apply the [per-aircraft gates](../aircraft-import.md#per-aircraft-implementation-gates)
to each row. Preserve the hybrid default, legacy compatibility and restricted
native research path, with deterministic 120 Hz simulation. Flight acceptance
must use the aircraft's own source numbers and documented fits. Hover and
conversion families need their own scenarios in addition to applicable
fixed-wing checks.

For each family, demonstrate player operation with defaults and remapped
inputs. Include hover/transition stability, control release and reset, absent
radar behavior, cockpit selection, device animation, applicable hook operation,
and the AC-130 gun-group scenarios. Record units, initial conditions, inputs
and expected bounds for fitted forces so later agents can reproduce the checks.

The [flight fit contract](variety-flight.md), [gunship contract](ac130-linked-guns.md)
and [tank contract](external-fuel-tanks.md) now hold the initial numerical and
control choices. Missing values are fitted under John's authorization, with
constants and known differences recorded.

The final import guide should contain a worked fixed-wing example, an F-4
variant comparison, and one worked example for each new flight/system family.
Each example should name the code entry points, required source evidence,
documented tuning, extraction commands, validation commands and known failure
messages. Link to specifications and baselines instead of copying their facts.
This is the planned handoff for Sol, Claude Sonnet and other contributors;
the guide becomes a proven procedure only after those examples pass.

Retail comparison is unavailable. Host regression tests, visual review and
source-derived checks must be reported separately from retail gameplay parity.
