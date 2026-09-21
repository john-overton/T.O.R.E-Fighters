# Standard aircraft radar component

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Component guide. Implementation mode, 2026-09-16. John requested an aircraft
capability survey and a standard component plan. John then proposed using PT
radar/IR signatures with our own look-down, era, notch and jamming behaviour on
2026-09-16. John also requested era-dependent jammer effectiveness, directional
scope noise based on jammer location/distance, and the orientation-sensitive RCS
panel. John's subsequent scope clarification keeps TWS/RWS and IR air-to-air,
includes history and persistent clicked selection, retains detectable destroyed
aircraft, and leaves gamified IFF in the target view. John also specified
single-target tracking only, not multi-track. Air-to-ground awaits ground weapons
and object systems. These directions are requested; the formulas, constants and
preset assignments below are agent decisions, not values requested by John, and
they are gameplay tuning rather than retail measurements. The
[retail behaviour spec](spec/radar.md) owns all recovered numbers,
[source notes](formats/radar.md) own their evidence, and
[the baseline](baselines/radar.md) records what was validated.

## Review summary and scope

Status: **implemented**, 2026-09-16. Stages 1 to 4 of the delivery table below
shipped; stage 5's tuning pass is partly done, with no side-by-side review yet.
One common radar implementation serves all twelve imported aircraft, with
data-driven profiles and an authored detection model. It covers air targets,
ownship radar and installed IR air-to-air sensors, contact history,
self-protection RF jamming, persistent mouse selection and radar-guided missile
support in the existing manual combat environment. It creates no AI, new missions
or autonomous target selection.

| Review item | Shipped behaviour |
| --- | --- |
| Retail inputs | Exact PT equipment and target signatures; SEE range/angles; ECM capability/statistics |
| Our detection rules | Square-root signature scaling, geometric look-down, radar/jammer generation matchups, notch and directional interference |
| Controls | Automatic RWS/TWS, installed IR air-to-air, M/O channel cycle, Y history, hover selector, selection and acquired-track markers, persistent clicked selection and bearing-only jammer noise |
| Track timing | Immediate selection; 0.5-second weapon-track acquisition; selection clears on observation loss; 1-second unselected stale plot |
| Tracking limit | One selected target and at most one acquired fire-control track across radar and IR; other returns are search observations |
| Missile support | One target for launcher illumination; fire-and-forget radar/IR missiles retain separate launch targets |
| Aircraft differences | Profile parameters, never separate aircraft-specific radar code |
| RCS panel | Shared observer-relative aircraft signature, exposure contour and passive emitter symbols |
| Contact lifetime | Destroyed aircraft remain detectable while an airborne physical object remains; HP zero does not erase a return |
| Identification | Existing gamified target-view IFF stays; no separate radar IFF system |
| Deferred | A-G, HARM, ground-weapon/object systems, scan animation, false contacts, escort jamming and detailed IR/engine modelling |

The complete [aircraft capability/signature tables](spec/radar.md) are the data
review. The numerical rules below are the tuning review. The presets, square-root
curve and immediate launcher-support loss are deliberate agent choices and can be
changed independently of the rest of the component. The shipped curve makes
signature-10 targets detectable at about one third of nominal range.

## Player contract

Every aircraft has the same understandable radar controls. Its installed
hardware changes how far and where it finds contacts, how well it sees downward,
and which contacts it can track for a missile. A player can explain why a contact
is visible but cannot be engaged. No aircraft needs a separate radar
implementation or a special rule in the scope renderer.

The scope uses the recovered range ladder and the automatic RWS/TWS rule, and
shows the current mode and range prominently. A long-range RWS contact is useful
search information, not a promise of a missile lock. Shrinking range changes
scope scale and targeting mode; it does not improve the physical radar. The
retail art and fonts are retained.

## Profile, live state and presentation

The boundary is one typed profile set and one live sensor state per aircraft, in
`crates/tore-sim/src/sensors/`. It reuses the existing dependency-free SEE reader
in tore-formats and normalizes once when the aircraft equipment suite is
imported, never during rendering. There are no aircraft-name branches: every
sensor hardpoint resolves by its parsed signature channel, a missing optional
channel is an explicit valid equipment state, and an unreviewed radar or ECM
record fails the import rather than defaulting to F18R.

| Part | Owns | Does not decide |
| --- | --- | --- |
| Imported profile | Source identity, search and track volumes, look-down coefficient, authored notch/ECCM preset, component provenance | Selected target or screen pixels |
| Radar live state | Power/failure, search observations, the single fire-control track and loss reasons | Weapon ammunition or missile flight |
| Player sensor controls | Selected channel, display range, designation request, history preference | Hidden targets or detection probability |
| Scope presentation | Projection, original glyphs, selector and contact picking | Whether a radar lock exists |
| Weapon system | Seeker compatibility, launch envelope, required radar support, missile lifecycle | A duplicate radar detection test |

Units are the existing simulation feet and radians, with explicit field units. A
volume contains independent azimuth/elevation half-angles, minimum/maximum
distance and relative altitude bounds. Search and track stay independent,
including the cases where a non-radar sensor's track distance exceeds its search
distance. Unknown source flags are preserved in source configuration without
guessing gameplay meanings.

Environmental input: ownship pose and equipment state, target identity, pose,
velocity, category and radar signature, plus the existing terrain query. The
component returns contact observations with identity, bearing, elevation,
distance and search/track eligibility. A current observation stays distinct from
a historical plot. The renderer has no authority to make a hidden target
selectable.

Visually detected, IR-detected and radar-detected contacts are never collapsed
into one boolean. A visual target can remain designated while radar support is
absent, and the weapon system asks for the appropriate channel and target ID.

## The authored detection model

Retail equipment and aircraft data are the inputs; the shared detection model is
**opinionated**. It deliberately replaces the original detection calculation and
does not wait on the remaining native caller branches. The retail findings stay
in the spec so differences remain visible. No physics fidelity claim is attached
to this gameplay tuning.

For either radar volume, effective range is:

`min(nominal_range, nominal_range * sqrt(effective_target_radar_signature / 100) * L * N * J)`

Here L, N and J are range factors for look-down, notch and jammer effects. The
appropriate nominal search or track range is used, and its angle/altitude bounds,
power/failure state and terrain visibility are enforced independently. Zero
signature gives no ordinary radar return, and invalid negative or non-finite
inputs are rejected. The nominal value is a hard maximum: a large signature can
compensate for some interference but cannot extend detection beyond the installed
volume.

For a clean nose-on target in clear, level conditions this gives signature 100 the full range, signature
50 about 71 percent, signature 10 about 32 percent and signature 80 about
89 percent. For a 90-mile search radar the shipped examples are 90 miles for
F/A-18D, 63.6 for Rafale C and 28.5 for F-22A. Its 50-mile tracking volume gives
50, 35.4 and 15.8 miles respectively. These are gameplay results, not retail
measurements or real-aircraft detection claims. A-4E's 120 signature hits the
nominal cap in clear conditions but provides more margin under interference.

IR uses the target's separate IR signature and installed signature-2 SEE device.
The IR policy is specified below. Radar RCS/aspect, Doppler notch and RF jammer
factors never apply to the IR channel.

### Look-down and equipment generations

Clutter exposure C is zero for targets at or above ownship. Otherwise:

`C = clamp(max(downward_angle_deg / 45, 1 - target_height_agl_ft / 5000), 0, 1)`

The look-down factor is `L = 1 - (look_down_coefficient / 100) * C`. It uses
terrain height at the target, not height above sea level. This is an authored
range penalty inspired by recovered geometry; retail reduces signature and has
additional branches. The source coefficients give older sets distinct behaviour
without applying a second arbitrary year-based range penalty.

The following agent-authored presets cover additional notch/ECCM differences.
They are gameplay groupings, not recovered historical classifications. They are
assigned by installed radar record, with per-field overrides available when
porting another device. A preset is never selected from aircraft year alone, and
an unreviewed radar record fails the import instead of borrowing a preset.

| Preset | Radar records | Notch half-width, ft/s | Range factor at notch centre | Reference burn-through B, nmi | Angular coupling half-width / sidelobe floor |
| --- | --- | ---: | ---: | ---: | --- |
| Basic | F4BR, MIG21R, MIG27R | Disabled | 1.0 | 5 | 3 degrees / 0.05 |
| Transitional | F14R | 100 | 0.20 | 8 | 2 degrees / 0.02 |
| Advanced | F18R, MIG29R, SU24R, SU27R, F22R | 60 | 0.45 | 10 | 1 degree / 0.005 |

Basic sets are primarily vulnerable to ground clutter and carry no Doppler notch,
rather than taking a second old-aircraft punishment. The advanced set resists
jamming and has a narrower notch but is not immune. SU24R's assignment follows
the zero look-down source grouping, not a historical claim, and remains a
deliberate tuning candidate. Device parameters are named profile fields, so
future adjustments never require changing the common component.

### Notching

Notch speed is the absolute projection of the target's **ground-relative
velocity** onto the radar-to-target line of sight. Ownship-relative closing speed
is not used; it would incorrectly notch a matching-speed tail chase.
For an enabled notch with half-width W and centre factor F:

`N = 1 - C * (1 - F) * clamp(1 - notch_speed / W, 0, 1)`

The penalty is strongest when the target crosses the beam, blends out at W,
and needs clutter exposure. A target above the radar has no notch penalty in
this first model. Radar immunity and exact historical notch widths are not
claimed. These zero-Doppler retail records do not supply these widths;
notching here is an explicit requested departure from retail data behaviour.

### Era-dependent jamming and burn-through

The **jammer equipment** carries its own profile: generation, strength,
compatible radar band group and operating state. The radar carries a separate
resistance profile. The grouping is by technology generation, not the airframe's
service year. Early, transitional and late-Cold-War are design labels; no
particular real system's classified capabilities or exact historical transition
year is asserted.

Agent-authored effectiveness multipliers:

| Jammer generation against radar | Basic radar | Transitional radar | Advanced radar |
| --- | ---: | ---: | ---: |
| Early | 1.00 | 0.60 | 0.30 |
| Transitional | 1.30 | 1.00 | 0.65 |
| Late-Cold-War | 1.60 | 1.25 | 1.00 |

A newer radar resists old jammers, and newer jammers remain effective against old
radars. None of these matchups guarantees immunity or a lost lock. These are
agent-authored gameplay coefficients, not retail or real-world measurements.

### Installed ECM inventory and generation assignment

Eight distinct ECM records are installed across the twelve aircraft. Every one
carries an explicit reviewable generation assignment, recorded in code and never
derived from the associated radar preset or the PT year. An unreviewed ECM record
fails the import instead of borrowing a generation.

| Generation | ECM records | Aircraft carrying them |
| --- | --- | --- |
| Early | F4.ECM, MIG21.ECM | A-4E; MiG-21, MiG-23 |
| Transitional | F14.ECM, MIG29.ECM, SU24.ECM | F-14D; MiG-29; Su-25 |
| Late-Cold-War | F18.ECM, SU27.ECM, F22.ECM | F/A-18D, Rafale C, X-31; Su-27, Su-35; F-22A |

F4.ECM and MIG21.ECM carry no radar-deception mode flag and a zero deception
chance, so the A-4E, MiG-21 and MiG-23 emit no RF noise at all and never reach
the matchup table above. That falls out of their own records, not from a tuning
decision. F22.ECM has deception chance 50; every other RF record has 30, the
reference strength below. The [retail record values](spec/radar.md#installed-ecm-records)
are in the spec.

Only installed, powered, undamaged RF jammers contribute. Jammer strength S is
`clamp(ECM.radar_deception_chance / 100, 0, 1)`, repurposed as an authored
strength input, not its original probability semantics. P is S/0.30. Band
compatibility K is a device-pair parameter, 0 or 1. Because retail records do not
establish frequency coverage, K=1 for the current fighter radar/self-protection
jammer pairs is an explicit approximation, not a claim that every jammer defeats
every radar. IR jammer fields remain separate.

For each emitter, its true position is used internally to compute distance Dj and
bearing/elevation at the radar. Terrain masking and receiver coverage gate the
received signal. The self-protection emitter is omnidirectional; an antenna
pattern remains a profile extension. There is no through-mountain or whole-world
jamming. Distances below 0.1 nmi use 0.1 for numerical stability.

Normalized received interference is `I = P * matchup * K * (20 / Dj)^2`,
with distances in nautical miles. This is noise received from the emitter,
not whether that emitter has been detected as a target. A jammer beyond the
selected display range can still interfere if its signal reaches the radar.
Emissions are not clipped at the display-range setting and require no target
lock.

For each candidate contact, the angular separation theta between its
3D line of sight and the jammer's is calculated. Coupling is
`A = floor + (1 - floor) * clamp(1 - abs(theta) / width, 0, 1)`, using the
receiving radar's width and sidelobe floor above. Received emitters are summed
in stable ID order. Nearby angular contacts share the noisy sector; other sectors
receive only the weaker sidelobe contribution. This permits incidental masking
of another aircraft without adding escort-jammer tactics or AI.

With target distance Dt, positive relative radar signature T=effective_signature/100 and
receiver reference B, compute:

`Q = sum(I * A) * Dt^4 / (20^2 * B^2 * T)`

`J = 1 - 0.60 * clamp(sqrt(Q) - 1, 0, 1)`

Q is a dimensionless gameplay interference-to-return proxy. J remains the
range factor in the main detection rule. This hybrid model is deliberately
simpler than calibrated RF simulation. At Q<=1 it adds no range penalty;
at Q>=4 it reaches the 60-percent maximum. Zero-signature targets have no ordinary
return and skip this division. Off/failed/incompatible or masked emitters give I=0.

For a target jamming from its own position, no masking, and angular coupling 1,
this reduces to an effective burn-through distance
`B_effective = B * sqrt(T / (P * matchup * K))` for positive jammer input.
At or inside that distance J=1; at twice it J=0.40. Stronger jamming or lower
signature brings burn-through closer; better receiver resistance pushes it out.
For reference strength 0.30 and effective signature 100, equal-generation matchups burn
through at B. An early jammer against an advanced radar has B_effective about
18.26 nmi; a late-Cold-War jammer against a basic radar about 3.95 nmi.
These are balance examples, not measured performance.

The broad motivation is that direct jammer energy and reflected target energy
have different distance dependence, so a closer return can overcome interference.
The distinction between received noise, target return and burn-through is
supported by the Naval Air Warfare Center's
[Electronic Warfare and Radar Systems Engineering Handbook, section 4-8](https://ausairpower.net/PDF-A/NAWCWPNS-TP8347-Rev.2-April-1999.pdf).
It supports the qualitative design, not our normalized equations or tuning.

The weapons code keeps its separate source-derived ECM deception checks, and this
radar range penalty is not applied again in the missile launch code. Receiver
interference and a seeker-deception event stay clearly distinct. Dedicated
stand-off jammers, false tracks, detailed RF waveforms and home-on-jam remain
outside this component.

### Directional scope interference

Presentation: a green noisy bearing strobe in the jammer's direction. A top-down
scope would draw a wedge from the origin through the full range axis; the shipped
scope is a bearing/range projection, so it draws a vertical band instead, as this
guide anticipated. The band does not start or stop at the jammer's actual
distance: passive noise alone provides no target range in this model. No
selectable contact, target identity or altitude is drawn from the noise
indication alone.

Brightness and density follow received I, reduced by the receiver's angular
filtering and clamped for readability. The agent-authored density is
`0.35 * I / (1 + I)` within the bearing strobe, with a lower-density sidelobe
haze using the radar's floor. The texture updates deterministically at 10 Hz from
the simulation tick and an independent display seed, and never consumes combat
RNG. Presentation fades over 0.25 seconds (30 steps) after an emission stops.
Authoritative support changes never wait on that cosmetic fade, and detection
never reads the faded copy. Text, the selection marker and valid track symbols
stay legible, and a burned-through target can appear over a still-noisy
background.

Indicated bearing is quantized to 2 degrees, with a minimum 2-degree visual
half-width, so noise does not disclose perfect angular precision. The physical
coupling width remains independent of raster readability. Multiple emitters
combine to a bounded density, so a strong emitter cannot turn every pixel
white. When own radar is off or failed, its noise indications disappear at once
rather than fading out.

One interference result serves detection and display. The renderer receives
bearing and intensity, not true emitter range or identity. Range selection
changes projection only; it does not change interference strength or reveal
jammer range. The player experience is a noisy direction, unreliable contacts
there, cleaner sectors elsewhere, and targets returning as ownship burns
through.

### Selection, weapon tracking and contact loss

**Single-track only**, requested by John. There is one optional selected ID, one
acquisition timer and one optional acquired fire-control track across radar and
IR together. A list of search contacts and stored observations is not a list of
weapon tracks. There is no background acquisition, track queue, simultaneous
radar/IR lock, automatic promotion or multi-target firing solution. TWS keeps
showing other search returns while only the selected target can acquire
tracking.

Selecting B while A is selected immediately replaces the selection, releases
A's acquired track/illumination and resets the timer for B. Re-clicking the same
current target does not reset acquisition. Switching radar/IR preserves a still
observed selected ID but releases the old channel's fire-control track and starts
new acquisition; the inactive channel cannot retain a second track. Entering RWS
releases the track and pauses acquisition, while a current selected return stays
selected. Returning to TWS starts a new acquisition timer.

Single-track limits the aircraft's fire-control support, not the number of
missiles in flight or their assigned targets. John explicitly requests preserving
fire-and-forget versus continuous-lock guidance. With a valid firing solution,
select A, fire an independently guided missile, select B, acquire a valid firing
solution and fire another. The first missile retains A and the second retains B.
Each launch snapshots its own target ID; cockpit selection changes never redirect
an earlier shot. Multiple missiles may also share the same target.

Each weapon keeps its existing guidance requirements. Fire-and-forget radar and
IR weapons continue using their own target and seeker rules after cockpit lock
or radar power is lost. A weapon requiring continuous launcher illumination loses
support when the aircraft switches targets or otherwise breaks the required lock,
and required illumination is specific to that missile's own target rather than to
whatever the cockpit has selected now. Its existing loss/reacquisition rules
decide the outcome; loss of support does not itself authorize a fabricated
instant destruction rule. IR seekers do not add another cockpit track. No
mandatory mid-course support phase or activation distance was introduced for
active weapons: exact activation behaviour remains unresolved, and this pass
preserves the existing weapon model. No multi-track aircraft support is implied
by the TWS name. The subsequent [missile plan](spec/missiles.md) proposes
an initially silent flight phase and a separate onboard activation/acquisition
transition. That proposal is not implemented and does not change the behavior
reported here.

John's requested click behaviour takes precedence over the retail target-cycle
restriction: clicking a current contact selects its stable target ID immediately,
including a search-only RWS contact. Selection persists without holding the
mouse, re-clicking or periodically re-designating. Empty clicks leave it alone;
explicit clear or clicking another current contact changes it. Selection supplies
the existing target view and its gamified identification. It is not a weapon lock.

Selection clears when the active sensor no longer has a current observation:
notch, look-down, jamming, terrain masking, range/angle exit, power/failure, crash
ending airborne existence, or removal. Loss of weapon tracking alone does not
clear a still-observed search contact. A missile-envelope failure also does not
clear selection. Reappearance creates an unselected contact; there is no hidden
sticky ID or automatic reacquisition of a lost selection.

A close target stays selectable through the visual channel with the radar off.
The visual sensor is collected in parallel with the selected scope channel, keeps
the existing geometric contract with no signature scaling or interference, is not
part of the channel cycle, and never supplies radar weapon support. It does
honour its own damage state: a destroyed visual sensor removes visual contacts.

This resolves an apparent contradiction in the two rules above. "Selection clears
when the active sensor no longer has a current observation" and "a visual target
can remain designated while radar support is absent" pull in opposite directions
once the radar loses a contact that is still inside the roughly 10-nmi visual
envelope. The implementation follows the second: selection survives on the visual
observation, and the radar answer separately reports its own inhibit reason. That
choice is an agent decision, not a requested one.

A display zoom that moves a still-observed contact outside the plotted range does
not itself lose the sensor observation or selection. RWS/TWS changes preserve
selection while the underlying return remains current; RWS removes radar weapon
support. Channel switching preserves the ID only when the new channel has a
current observation of it. These distinctions are agent interpretations of the
requested persistence, documented explicitly for review.

The fitted host cadence is the existing fixed 120 Hz. Selection appears
immediately; opinionated weapon-track acquisition takes 0.5 seconds (60
consecutive valid steps) in TWS or the selected IR tracking volume. Radar support
and IR tracking remain separate. RWS never provides radar weapon lock, even for a
selected target. Master-arm-safe and zero-ammunition states do not erase a sensor
track.

Loss of current observation clears selection and weapon support immediately. The
final observation stays as an unselected coasting symbol for 1 second (120
steps), with no predicted motion or updated position. It is visibly stale and
non-selectable, and it belongs to the channel that produced it: changing channel
retires the old channel's plots rather than redrawing them on the new page. After it expires, only enabled history samples remain until
their independent age limit; they do not preserve an active track or target view.
A new current observation after a gap requires a fresh click and track acquisition.
No random per-render-frame rolls or autonomous target selection are introduced.

### History trails, based on the USNF manual

The USNF manual's historical mode draws a sequence of past contact positions,
enabled with Y or the scope Y button. It does not provide timing or capacity;
[source evidence](formats/radar.md#usnf-history-and-ir-functional-reference)
keeps that limit explicit. That function is reproduced with these agent-authored
values in TWS, RWS and IR air-to-air:

- History is off by default. Y and the Y button toggle the same preference.
- One actual sensor observation is recorded every 0.5 seconds (60 simulation
  steps), at most 8 past observations per target per channel, with maximum age
  4 seconds. Each sample stores a simulation timestamp and the observed world
  position, not old screen pixels.
- They draw as small progressively dimmer dots, separate from the current contact
  symbol and heading tail. Samples reproject through the current scope transform
  when ownship moves or the range/layout changes, and are never connected across
  a missing sample.
- There is no extrapolation, no point after loss, no target identity inferred
  from trails, and no selecting or firing at historical dots. The current return
  is always distinct.
- History is collected while its display is off, so enabling it shows the recent
  observations immediately. Inactive channels collect no new samples. Switching
  channels does not fuse their observations; RWS/TWS share the radar channel.
- Samples age with simulation time, freeze while paused and clear on mission
  restart or aircraft change. Bounded history for lost or destroyed contacts is
  retained until its normal expiration; no trail is fabricated for an undetected
  object.

Presentation timing is independent of rendering and independent of the 1-second
stale contact marker. Contact samples from different channels remain separate even
when they refer to the same physical ID.

### Infrared air-to-air mode

Only aircraft with the installed IR sensors listed in the
[retail equipment table](spec/radar.md#other-installed-sensor-channels) have this
channel. Carrying an IR missile does not grant the aircraft a full IRST scope. An
absent or failed IR sensor is unavailable rather than borrowed from another
aircraft.

IR effective range is
`min(nominal_IR_range, nominal_IR_range * sqrt(PT_IR_signature / 100))`.
Its own SEE search/track angles and ranges apply, plus terrain visibility.
No RF jammer/noise, radar notch, look-down coefficient or RCS aspect factors
apply to this channel. It uses the base PT IR signature; engine/throttle heat,
rear-aspect bonuses, weather attenuation and post-destruction cooling remain
explicit fitted limitations, not invented retail capability. IR operation is
passive and does not emit radar energy or confer radar missile illumination.

For sensors with search 9/track 10 nmi, new contacts require the search envelope.
An already selected acquired IR track stays current within its own tracking
envelope, even just outside search coverage. It cannot acquire an unseen target
at 10 miles. Retained tracks can be selected and displayed as observations;
the wider track range does not grant hidden world-object access.

I requests IR, which stops radar transmission without moving the radar power
switch. R returns to the radar channel when IR is selected, and otherwise toggles
radar power as before, so the power switch is not spent leaving the passive page.
M and O cycle the available radar and IR channels; radar channel RWS/TWS stays
automatic by display range. The recovered range ladder serves scope zoom in both
channels, and selecting a larger scale does not extend the IR device's physical
coverage. IR is labelled on the page and RF noise is cleared from it. With no IR
installed, M, O and I report that it is unavailable and preserve radar state. The
default-key migration is in delivery below.

IR selection drives the same target view and existing IR seeker launch checks.
It never bypasses a missile's own seeker envelope, ammunition, arming or radar
requirement. Existing radar-emission and missile lifecycle rules remain independent
of merely having an IR-selected target. The target view's gamified IFF is
unchanged; no interrogation simulation or new radar-scope friend/foe
classification is added.

### Destroyed aircraft remain sensor objects

John requested keeping dead opponents visible, an intentional departure from his
reported retail behaviour. Combat viability, physical existence and sensor
observation are separate states. HP reaching zero does not delete a radar/IR
return, selection, or its history while the aircraft is still airborne and the
active sensor can observe it. It can still be clicked as a current contact.
Identification and known damage status stay in the target view, not an omniscient
DEAD label automatically applied to every scope return.

Destroyed-aircraft combat actions stay disabled and damage/kill accounting stays
final. The existing weapon rejection of destroyed targets is preserved where
applicable, and that rejection does not erase the observation. Existing missile
retirement rules are not implicitly changed into wreck engagement. Emissions
cease when the relevant equipment is destroyed or powered off, independently of
passive radar reflection or the fitted IR signature. A radar return is not proof
of radiation.

The HP-based movement, visibility, designation and target-view filters were
audited and split. A physical airborne remnant is retained until impact: a wreck
falls ballistically at 32.174 ft/s^2 until it reaches terrain, which is the
minimal fitted fall this guide allowed, not new AI or a breakup simulation. A
grounded wreck ends the active A2A observation and selection; its existing trail
ages out normally. This does not remove the wreck from the world or implement A2G
acquisition. Detailed wreck, debris, fire and cooling behaviour belongs with
future object-system work.

## RCS instrument and shared aspect model

Page 0 (RCS) is part of this component. Its [retail contract](spec/rcs.md)
explains the player's exposure contour and surrounding emitter symbols. It keeps
the retail chrome/font and heading-relative 0/90/180/270 presentation. RCS and
active radar stay separate: turning own radar off does not disable the passive
exposure display. The outer ring is a display scale, not a radar beam boundary.

Agent-authored signature model: the unit direction from target aircraft to
observing radar is transformed into the target's body axes. With f, r and u its
forward, right and up components:

`effective_signature = PT_radar_signature * (f*f + 2*r*r + 4*u*u) * configuration`

The configuration multiplier is
`(1 + 0.25*gear) * (1 + 0.15*flaps) * (1 + 0.50*bay_open)`, using actual deployed
fractions 0..1, not command switches. Components absent from an aircraft contribute
zero deployment. These weights are agent-authored gameplay tuning, not recovered
RCS measurements. They are profile fields with common defaults, not extra
hardcoded rules for named aircraft. External-store modifiers remain deferred.

At clean configuration, nose/tail is 1x, side is 2x, top/bottom is 4x the PT base.
A radar abeam sees the factor rise from 2x to 4x as the aircraft banks from level
to 90 degrees. A radar directly on the longitudinal axis does not receive that
same bank bonus. This ties exposure to what that observer can see rather than
making every turn globally increase signature. At 180-degree roll the clean
level-side factor returns to 2x. This applies to targets as well as ownship.
The square-root detection law and the jammer return comparison both use
this same effective signature. IR uses its own base-signature policy above.

The contour samples this function around ownship at 5-degree bearing steps
against a documented, level reference radar: 25 nmi nominal search range,
reference signature 400, no terrain/clutter/jamming and the same range law as
active detection. Those reference detection distances are plotted on the passive
scope's range scale. Reference signature is a profile parameter: actual radar
profiles use 100 in the law above; this deliberately weaker reference set uses
400 to keep the common PT-100 contour responsive rather than saturated at every
bearing. A clean PT-100 aircraft yields 12.5/17.68/25-nmi front/side/top reference
radii. Any model reaches its nominal range cap eventually; further signature
increases improve interference margin rather than exceeding that cap.
It is an estimate of directional vulnerability to a reference set,
not an assertion of any particular emitter's knowledge, range, power or lock.
The view scales are 5/10/20/30/50 nmi, default 50, an agent-authored reuse of the
current RWR scale. The actual retail RCS +/- semantics remain unknown. Values
beyond the selected scale clip with an over-range indication rather than changing
the underlying signature. The reference-radar calibration is a reviewable knob.

Emitter symbols come from a shared passive-observation service reading the same
explicit emitter state as the rest of the component: pose, RF equipment profile,
powered/failed state and any externally supplied tracking/launch state. The
existing manual fixtures supply those states; no AI or autonomous scanning or
target decision is added. Reception is bounded by the passive instrument's 50-nmi
maximum, terrain visibility and a powered compatible emission, so an aircraft
whose ECM record carries no radar-deception mode raises no symbol at all. This is
a fitted gameplay receiver, not a claim of measured receiver sensitivity.

Known ranged contacts are plotted by position. A received emitter with bearing
but no independently available range stays at the outer bearing ring with an
unknown-range marker. The jammer-noise field's internal true position is never
converted into a ranged or identified RCS contact, and inactive hidden aircraft
are not shown merely because their world positions exist. Ground squares,
aircraft symbols and warning states are drawn only to the extent
classification/state is available; anything else uses the unknown-emitter symbol.
There is no enemy/friendly inference without data.

The manual's inside/outside contour interpretation is relative exposure feedback.
An emitter outside it can still detect us with stronger equipment; one inside may
be masked, pointed away or unable to track. No lock warning appears merely
because a symbol crosses the contour. Own radar and jammer transmissions affect
emission detectability through the emitter model, not by silently inflating
physical RCS. That distinction is a deliberate presentation choice and is
recorded here as the player guide.

Covered by tests: the same PT viewed nose/side/top gives 1/2/4 factors; abeam
bank changes exposure but nose-on roll does not; lowering gear adds 25 percent
smoothly; clearing gear/flaps/bays restores baseline; the RCS contour and the
radar/jammer calculations sample the identical signature function; passive
reception works with own radar off; a jammer-only bearing leaks no ranged dot;
and an inside-contour contact implies no lock.

## Mouse designation and missiles

Interaction: hovering a contact draws the selector corners around it, and a click
and release on the same visible contact designates its stable ID. Drawing,
hovering and picking share one projection at every window size and in both
layouts, so the selector always marks the contact a click would take. The
agent-authored tolerance is 7 instrument raster pixels, and an empty click leaves
the designation unchanged. Equal-distance ties resolve by stable target ID.
Hovering never designates by itself, and focus loss clears the hover along with
any pending press, as do layout and channel changes and window exit. The
simulation revalidates the requested target when applying the command, so a click
can never select a target it does not observe.

The page shows a selection marker, a separate acquired-track marker and an
actionable inhibit reason: radar off, failed, RWS search only, acquiring, beyond
tracking coverage, or outside the missile envelope. Terrain masking is not one of
them any more, because masking clears the observation and the selection, so the
readout reports no target instead. The scope's status text follows the channel in
use, so an infrared track reads TRACK rather than a radar power complaint; the
separate radar weapon permission is what a launch consults. These are authored
presentation messages, not recovered retail strings. The existing clear-designation action is unchanged, and
an unavailable target is rejected without silently selecting another one. Combat
replay records the target ID rather than screen coordinates.

The shared operations are one observe step, designate(target_id), clear
selection, and support(target_id). The last returns whether current radar data or
illumination supports that specific target, with a reason when unavailable.
Weapons consult that result for launch and, only when their guidance requires
it, during flight. Retargeting the cockpit does not retarget any missile already
in flight. No target selection is automated.

A clicked RWS contact stays selected for information but provides no radar weapon
lock. The player-requested persistent selection deliberately differs from retail
RWS acquisition. Changing to RWS ends radar illumination while preserving a
current observation and selection. Switching designation, losing tracking
coverage, power loss and failure end illumination; masking ends it by clearing
the observation. Selection is cleared only under the observation-loss rules
above. A selected but untracked or stale target cannot
preserve radar support. These transitions are opinionated where retail evidence
is unresolved. An already active missile retains its own target and seeker state;
active-seeker activation and flight tuning remain weapon-owned work.

## Porting an aircraft

1. Read its actual PT hardpoints and resolve all SEE records with the existing
   importer. Preserve the exact aircraft identity and equipment source hashes.
2. Convert each device to its channel profile. Validate finite units, usable
   ranges and angles; fail unsupported data explicitly instead of substituting
   another aircraft's radar. A missing optional channel is different from an
   invalid or missing referenced file.
3. Review the human-readable capability summary that
   `cargo run -q --locked -p tore-app -- --sensor-summary` prints for every
   registered aircraft. The cost of a port is reviewing its data, not writing
   another radar controller.
4. Run the common synthetic coverage, look-down, state-transition and missile
   handoff tests, then a local media audit for that aircraft's actual bindings.

## Delivery and acceptance

Milestones live in [the roadmap](ROADMAP.md#1f-sensors-and-weapons). Each stage
ended with a reviewable result.

| Stage | Work | Reviewable exit | Status |
| --- | --- | --- | --- |
| 1. Profiles | Expose PT radar/IR signatures, installed radar/IR channels and ECM inputs; review radar and jammer tuning | All twelve aircraft produce the expected capability summary; no missing sensor silently becomes F18R | **Done.** `--sensor-summary` reports all twelve; an unreviewed radar or ECM record is an import error |
| 2. Shared simulation | Add radar/IR detection, aspect factors, passive emitters, bounded history and persistent selection; separate death from physical contact lifetime | Numeric boundary tests and deterministic headless contact traces pass; no renderer or frame rate affects results | **Done.** The component and its acceptance tests live in `crates/tore-sim/src/sensors/` |
| 3. Player scope | Use shared contacts/interference; correct range modes; add IR air-to-air, Y history, directional noise, persistent selection, RCS contour and clear track status | Mouse picks the intended target in both layouts and after resize; RWS selection cannot supply weapon lock; history cannot be clicked; target-view IFF stays unchanged | **Done** |
| 4. Weapon integration | Replace duplicated radar checks with shared support status; preserve weapon-specific launch envelopes and active/semi-active distinctions | Correct target receives launch; each support-loss case passes; recording/replay reproduces selected target and engagement | **Done.** Radar weapons consult the shared support result; version-3 tapes carry the controls and the designated identity |
| 5. Aircraft tuning pass | Exercise synthetic same-target scenarios for each radar preset and local imported profile; run workspace and rendered checks | Side-by-side capability results and captures reviewed, known approximations recorded, guides updated | **Partly done.** All twelve produce the capability summary and pass their combat smokes, and the approximations and guides are recorded here. No side-by-side tuning review has happened |

Stage 3 migrated saved scope ranges by their old nautical-mile value to the
nearest new range, with equal-distance ties choosing the lower setting; an old
index is never reinterpreted as a different range. New profiles default to the
recovered 10-mile display. M and O select an available radar or IR channel and Y
toggles history; no HARM or A-G page was enabled. Keyboard cycling and mouse
clicks use the same current-observation eligibility, including selectable RWS
contacts.

The previous defaults used Y for the target-jammer fixture and I for an incoming
weapon fixture. The agent-authored migration shipped: Y is history, I selects
infrared, and those two development commands moved to Shift-Y and Shift-I. Their
named actions, CLI commands and replay commands are unchanged, and
user-customized bindings are preserved. The existing `radar-mode` controller
action now cycles available sensor channels instead of toggling cosmetic
RWS/TWS, and three named actions joined the controls editor and custom profiles:
`sensor-channel` (an alias of `radar-mode`), `sensor-infrared` and
`sensor-history`. Default changes are recorded in [INPUT.md](INPUT.md) and
[FLIGHT-CONTROLS.md](FLIGHT-CONTROLS.md).

Stage 4 took each supported radar weapon's requirements from existing reviewed
weapon configuration. The subsequent missile update adds explicit per-weapon active activation and
seeker acquisition, documented in [the missile specification](spec/missiles.md). Deterministic replay captures radar/IR channel,
display range, power and selection changes, physical contact lifetime and target
jammer inputs: version-4 combat tapes retain sensor controls and designation identity, and
add full launch velocity and bay permission. Version 2/3 tapes explicitly
replay with compatibility weapons.

### Acceptance cases

The automated cases use source-free synthetic fixtures and live in
`crates/tore-sim/src/sensors/`, with the fifteen scenario tests in its
`acceptance` module:

- Signature law: a 90-mile radar detects signature 100 at 90, signature 50 at
  about 63.64 and signature 10 at about 28.46 miles in clear level conditions.
  Check just inside/outside each boundary and enforce the nominal maximum.
- Geometry: search and tracking half-angles are separate; a search contact
  outside tracking coverage is visible but cannot provide a radar missile lock.
- Look-down: with C=0.5, coefficients 50/30/0 produce L=0.75/0.85/1.0. Changing
  terrain elevation changes target AGL correctly; looking upward removes C.
- Notch: at C=1 and zero target radial ground speed, transitional/advanced N is
  0.20/0.45. At each preset's width, N=1. A matching-speed tail chase does not
  notch solely because relative closing speed is zero. Basic preset has no notch.
- Jamming: absent/off/failed/incompatible/masked jammer gives J=1. Reference
  strength, signature 100 and equal generation gives J=1 at B and J=0.40 at 2B.
  Check each generation pair, lower-signature burn-through and angular coupling.
  At fixed target geometry, doubling jammer distance quarters received I.
- Noise: movement changes bearing; distance and power change intensity; terrain
  hides the signal. A jammer outside the display range can still add noise.
  No range, identity or selectable target leaks from a noise-only indication.
  A burned-through target remains usable on a noisy background. Multiple emitters
  remain readable, and cosmetic randomness cannot alter detection or replay.
- Modes: Hornet 50 TWS/100 RWS, A-4E 25 TWS/50 RWS, MiG-29 25 TWS/50 RWS;
  switching display range does not modify physical coverage values.
- Selection/track lifecycle: a current RWS/TWS/IR click selects immediately and
  persists without repeat input. TWS/IR weapon tracking acquires on step 60, not
  59. Search-only selection never implies radar lock. Loss clears selection and
  firing permission immediately; reappearance stays unselected. A stale plot is
  unselected/non-selectable for at most 120 steps. Empty clicks and display zoom
  alone do not clear a still-observed target.
- Single-track limit: select A then B, re-click B, switch TWS/RWS/IR and remove
  the selected object. There are always zero or one acquired aircraft tracks;
  non-selected contacts never acquire, old illumination is released immediately,
  and no destroyed/lost target causes automatic selection of another contact.
- History: sample every 60 ticks, retain at most 8 per target/channel, expire at
  480 ticks; no samples after loss or through gaps. Pause, resize, turn, channel
  change, toggle and restart preserve the specified sample/age behaviour. Trail
  dots cannot produce selection, identity or launch permission.
- IR: unavailable without installed signature-2 SEE; passive mode disables radar
  emissions, uses PT IR values and source zones, ignores RF jammer/notch effects,
  and supports retained 9/10-nmi search/track cases without acquiring unseen targets.
- Destroyed contacts: HP zero alone does not remove an airborne observable
  contact or selection. Radar failure can silence emissions without hiding its
  reflection. Crash/removal ends A2A selection; no double kill/damage credit or
  new A2G acquisition is introduced. Existing target-view IFF remains unchanged.
- UI: two adjacent contacts, equal-distance tie, stale target between press and
  release, resize, both layouts, focus loss, empty click and no-radar aircraft.
- Weapons: failed launch consumes no ammo; launch uses the designated ID; radar
  off, radar failure, out-of-track coverage, RWS and changed designation remove
  illumination, and masking removes it by clearing the observation. Launch an independent missile at A, then acquire B and launch
  another: their stored target IDs and pursuit remain A and B respectively, with
  only one cockpit track. Repeat with IR guidance. For a continuous-lock weapon,
  switching to B removes support for A and invokes its existing guidance-loss
  rules; unrelated designation must never count as illumination of A. Radar-off
  cases preserve independent guidance while removing required launcher support.
  Lost/stale cockpit tracks cannot create a new guided launch.
- Replay: the same inputs produce the same contacts, designation, lock transitions
  and shots under different rendering rates, with no retail fixtures committed.

## Deliberate departures and known approximations

These are choices, not oversights. Each one is a place where the shipped
behaviour differs from retail data, from an earlier plan in this guide, or from
what a first reading of the code might suggest.

- All detection, notch, jamming, selection, history and RCS constants are
  agent-authored gameplay tuning, not retail measurements. They carry no physics
  fidelity claim.
- Notching is an explicit requested departure from the retail data: the nine
  inspected radar records are zero-Doppler and supply no widths.
- The visual channel keeps its existing geometric contract: no signature scaling,
  no interference and no weapon support. It is collected in parallel with the
  selected scope channel, so a close target stays selectable with the radar off,
  and it is not part of the M cycle.
- The directional noise draws as a vertical band rather than a wedge, because the
  scope is a bearing/range projection.
- Selection survives on a visual observation after the radar loses the contact,
  which is the agent resolution of two plan rules that conflict in that case.
- Terrain masking no longer produces a launch inhibit reason. It clears the
  contact and the selection instead, so the readout reports no target.
- Destroyed aircraft stay observable while airborne, which John requested as a
  departure from his reported retail behaviour. The wreck's ballistic fall is a
  minimal fitted addition.
- Persistent click selection, including of a search-only RWS contact, departs
  from the recovered retail RWS target-cycle restriction. That restriction stays
  a research fact in the [spec](spec/radar.md).
- Band compatibility K is 1 for every current fighter radar and self-protection
  jammer pair, because retail records establish no frequency coverage.
- Air to ground, HARM, escort jamming, false contacts, scan animation, detailed
  infrared modelling and any AI remain out of scope and unimplemented.

Required formatting, Clippy, Rust tests/build, Python tests, asset and document
checks were run, along with the rendered smoke test and capture inspection. No
flight adapter default changed. [The baseline](baselines/radar.md) records what
was actually validated and which gameplay pieces remain opinionated or fitted. No
retail comparison was made, none is required to validate these local contracts,
and none establishes retail parity.

Missile observations now belong to the weapon seeker and use the shared
RCS/aspect function. Cockpit designation remains independent. IR heat and passive
emission eligibility never substitute for one another. Missiles retain their
last measured intercept during loss and can reacquire their original target
until guidance expiry. [Missile rules](spec/missiles.md) define the fitted tuning.

The entire black radar screen uses a green crosshair with a central gap instead of the OS
pointer, following John's 2026-09-17 reference image. Its position follows the
mouse and uses the same scaling as contact selection. Takeover starts at the
screen edge, including margins outside the contact plot, and ends at the bezel.
It remains visible with the radar off. [Screen bounds](spec/missiles.md#radar-cursor-screen-boundary). L or the upper-right
RELEASE LOCK button clears designation. [HUD rules](spec/missiles.md#weapon-hud-delivery).

The HUD additionally remembers the explicitly selected target for its square or
edge chevron, independent of sensor selection and weapon support. Loss of
observation still expires sensor selection normally. The display-only cue
never supplies radar gun lead or missile guidance. L/RELEASE LOCK clears both.
[Target-cue behavior](spec/gunsight-targeting.md#target-square-and-edge-chevron).

The map also observes surface entities with the existing active-channel search
and visual rules. These presentation-only observations never enter the airborne
scope, designation or weapon support. Visual observation supplies map identity;
radar/IR alone supplies an unknown marker. [Map rules](spec/flight-map.md).
