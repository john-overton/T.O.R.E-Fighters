# Standard aircraft radar component proposal

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research and design proposal, 2026-09-16. John requested an aircraft capability
survey and a standard component plan. John then proposed using PT radar/IR
signatures with our own look-down, era, notch and jamming behaviour on 2026-09-16.
John also requested era-dependent jammer effectiveness, directional scope
noise based on jammer location/distance, and the orientation-sensitive RCS panel.
John's subsequent scope clarification keeps TWS/RWS and IR air-to-air, includes
history and persistent clicked selection, retains detectable destroyed aircraft,
and leaves gamified IFF in the target view. John also specified single-target
tracking only, not multi-track. Air-to-ground awaits ground weapons
and object systems. These directions are requested;
the formulas, constants and preset
assignments below are agent proposals, not values requested by John. No radar
gameplay changes are implemented by this document. The [retail behaviour spec](spec/radar.md) owns
all recovered numbers; [source notes](formats/radar.md) own their evidence.

## Review summary and scope

Status: **proposed, awaiting review before implementation**. Deliver one common
radar implementation serving all twelve imported aircraft, with data-driven
profiles and an authored detection model. First release covers air targets,
ownship radar and installed IR air-to-air sensors, contact history, self-protection
RF jamming, persistent mouse selection and radar-guided missile support in the existing manual combat environment. It creates no AI,
new missions or autonomous target selection.

| Review item | Proposed first release |
| --- | --- |
| Retail inputs | Exact PT equipment and target signatures; SEE range/angles; ECM capability/statistics |
| Our detection rules | Square-root signature scaling, geometric look-down, radar/jammer generation matchups, notch and directional interference |
| Controls | Automatic RWS/TWS, installed IR air-to-air, Y history, green selector, persistent clicked selection and bearing-only jammer noise |
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
curve and immediate launcher-support loss are deliberate agent proposals and may be
changed independently before implementation. In particular, the proposed curve
makes signature-10 targets detectable at about one third of nominal range.

## Player contract

Every aircraft has the same understandable radar controls. Its installed
hardware changes how far and where it finds contacts, how well it sees downward,
and which contacts it can track for a missile. Players should be able to explain
why a contact is visible but cannot be engaged. Different aircraft must not need
separate radar implementations or special rules in the scope renderer.

Use the recovered range ladder and automatic RWS/TWS rule. Show the current
mode and range prominently. A long-range RWS contact is useful search information,
not a promise of a missile lock. Shrinking range changes scope scale and targeting
mode; it does not improve the physical radar. Retain the retail art and fonts.

## Profile, live state and presentation

Proposed boundary: one typed RadarProfile and one RadarState per installed set
in tore-sim. Reuse the existing dependency-free SEE reader in tore-formats.
Normalize once when importing/building the aircraft equipment suite, not during
rendering. Avoid aircraft-name branches. Resolve every sensor hardpoint by its
parsed signature/channel; support no radar as an explicit valid equipment state
for later aircraft. Do not default an unknown plane to F18R.

| Part | Owns | Does not decide |
| --- | --- | --- |
| Imported profile | Source identity, search and track volumes, look-down coefficient, authored notch/ECCM preset, component provenance | Selected target or screen pixels |
| Radar live state | Power/failure, search observations, the single fire-control track and loss reasons | Weapon ammunition or missile flight |
| Player sensor controls | Selected channel, display range, designation request, history preference | Hidden targets or detection probability |
| Scope presentation | Projection, original glyphs, selector and contact picking | Whether a radar lock exists |
| Weapon system | Seeker compatibility, launch envelope, required radar support, missile lifecycle | A duplicate radar detection test |

Use existing simulation feet and radians, with explicit field units. A volume
contains independent azimuth/elevation half-angles, minimum/maximum distance and
relative altitude bounds. Keep search and track independent, including cases
where a non-radar sensor's track distance exceeds its search distance. Preserve
unknown source flags in source configuration without guessing gameplay meanings.

Proposed environmental input: ownship pose and equipment state, target ID/pose/
velocity/category/radar signature, plus the existing terrain query. The service
returns contact observations with identity, bearing, elevation, distance and
search/track eligibility. Distinguish a current observation from a historical
plot. The renderer receives no authority to make a hidden target selectable.

Do not collapse visually detected, IR-detected and radar-detected contacts into
one boolean. A visual target can remain designated while radar support is absent.
The weapon system must ask for the appropriate channel and target ID.

## Proposed authored detection model

Use retail equipment and aircraft data as inputs, with an **opinionated** shared
detection model. This deliberately replaces the original detection calculation;
it does not wait on the remaining native caller branches. Keep the retail
findings in the spec so differences remain visible. No physics fidelity claim
is attached to this gameplay tuning.

For either radar volume, proposed effective range is:

`min(nominal_range, nominal_range * sqrt(effective_target_radar_signature / 100) * L * N * J)`

Here L, N and J are range factors for look-down, notch and jammer effects.
Use the appropriate nominal search or track range. Enforce its angle/altitude
bounds, power/failure state and terrain visibility independently. Zero signature
gives no ordinary radar return; reject invalid negative/non-finite inputs.
The nominal value remains a hard maximum. A large signature can compensate for
some interference but cannot extend detection beyond the installed volume.

For a clean nose-on target in clear, level conditions this gives signature 100 the full range, signature
50 about 71 percent, signature 10 about 32 percent and signature 80 about
89 percent. For a 90-mile search radar the proposed examples are 90 miles for
F/A-18D, 63.6 for Rafale C and 28.5 for F-22A. Its 50-mile tracking volume gives
50, 35.4 and 15.8 miles respectively. These are proposed gameplay results, not
retail measurements or real-aircraft detection claims. A-4E's 120 signature hits
the nominal cap in clear conditions but provides more margin under interference.

IR uses the target's separate IR signature and installed signature-2 SEE device.
The initial IR policy is specified below. Never apply radar RCS/aspect, Doppler
notch or RF jammer factors to the IR channel.

### Look-down and equipment generations

Proposed clutter exposure C is zero for targets at or above ownship. Otherwise:

`C = clamp(max(downward_angle_deg / 45, 1 - target_height_agl_ft / 5000), 0, 1)`

Proposed look-down factor is `L = 1 - (look_down_coefficient / 100) * C`.
Use terrain height at the target, not height above sea level. This is an authored
range penalty inspired by recovered geometry; retail reduces signature and has
additional branches. The source coefficients give older sets distinct behaviour
without applying a second arbitrary year-based range penalty.

The following agent-proposed presets cover additional notch/ECCM differences.
They are gameplay groupings, not recovered historical classifications. Assign by
installed radar, with per-field overrides available when porting another device.
Never select a preset automatically from aircraft year alone.

| Proposed preset | Initial radar records | Notch half-width, ft/s | Range factor at notch centre | Reference burn-through B, nmi | Angular coupling half-width / sidelobe floor |
| --- | --- | ---: | ---: | ---: | --- |
| Basic | F4BR, MIG21R, MIG27R | Disabled | 1.0 | 5 | 3 degrees / 0.05 |
| Transitional | F14R | 100 | 0.20 | 8 | 2 degrees / 0.02 |
| Advanced | F18R, MIG29R, SU24R, SU27R, F22R | 60 | 0.45 | 10 | 1 degree / 0.005 |

Basic sets are primarily vulnerable to ground clutter in this proposal; do not
also give them a Doppler notch merely as another old-aircraft punishment. The
advanced set resists jamming and has a narrower notch but is not immune. SU24R's
initial assignment follows the zero look-down source grouping, not a historical
claim; it is a deliberate tuning candidate. Expose named device parameters so
future adjustments never require changing the common component.

### Notching

Proposed notch speed is the absolute projection of the target's **ground-relative
velocity** onto the radar-to-target line of sight. Do not use ownship-relative
closing speed, which would incorrectly notch a matching-speed tail chase.
For an enabled notch with half-width W and centre factor F:

`N = 1 - C * (1 - F) * clamp(1 - notch_speed / W, 0, 1)`

The penalty is strongest when the target crosses the beam, blends out at W,
and needs clutter exposure. A target above the radar has no notch penalty in
this first model. Radar immunity and exact historical notch widths are not
claimed. These zero-Doppler retail records do not supply the proposed widths;
notching here is an explicit requested departure from retail data behaviour.

### Era-dependent jamming and burn-through

Give the **jammer equipment** its own profile: generation, strength, compatible
radar band group and operating state. Give the radar a separate resistance
profile. Use technology generation, not the airframe's service year. Early,
transitional and late-Cold-War are design labels; no particular real system's
classified capabilities or exact historical transition year is asserted.

Agent-proposed effectiveness multipliers:

| Jammer generation against radar | Basic radar | Transitional radar | Advanced radar |
| --- | ---: | ---: | ---: |
| Early | 1.00 | 0.60 | 0.30 |
| Transitional | 1.30 | 1.00 | 0.65 |
| Late-Cold-War | 1.60 | 1.25 | 1.00 |

A newer radar resists old jammers, and newer jammers remain effective against old
radars. None of these matchups guarantees immunity or a lost lock. These are
agent-proposed gameplay coefficients, not retail or real-world measurements.
Before implementing, inventory the installed ECM records and add an explicit
reviewable generation assignment for each unique device. Unknown assignments
remain unknown until a labelled agent choice is made; do not silently derive
them from the associated radar preset or PT year. Version these tuning profiles.

Only installed, powered, undamaged RF jammers contribute. Initial jammer strength
S is `clamp(ECM.radar_deception_chance / 100, 0, 1)`, repurposed as an authored
strength input, not its original probability semantics. Normalize P as S/0.30.
Band compatibility K is a device-pair parameter, 0 or 1 initially. Because retail
records do not establish frequency coverage, proposing K=1 for the current
fighter radar/self-protection jammer pairs is an explicit approximation, not a
claim that every jammer defeats every radar. IR jammer fields remain separate.

For each emitter, use its true position internally to compute distance Dj and
bearing/elevation at the radar. Terrain masking and receiver coverage gate the
received signal. Initially assume an omnidirectional self-protection emitter;
keep antenna pattern as a profile extension. No through-mountain or whole-world
jamming. Distances below 0.1 nmi use 0.1 for numerical stability.

Define normalized received interference `I = P * matchup * K * (20 / Dj)^2`,
with distances in nautical miles. This is noise received from the emitter,
not whether that emitter has been detected as a target. A jammer beyond the
selected display range can still interfere if its signal reaches the radar.
Do not clip emissions at the display-range setting or require a target lock.

For each candidate contact, calculate angular separation theta between its
3D line of sight and the jammer's. Proposed coupling is
`A = floor + (1 - floor) * clamp(1 - abs(theta) / width, 0, 1)`, using the
receiving radar's width and sidelobe floor above. Sum all received emitters
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
These are proposed balance examples, not measured performance.

The broad motivation is that direct jammer energy and reflected target energy
have different distance dependence, so a closer return can overcome interference.
The distinction between received noise, target return and burn-through is
supported by the Naval Air Warfare Center's
[Electronic Warfare and Radar Systems Engineering Handbook, section 4-8](https://ausairpower.net/PDF-A/NAWCWPNS-TP8347-Rev.2-April-1999.pdf).
It supports the qualitative design, not our normalized equations or tuning.

The current weapons code has separate source-derived ECM deception checks.
Do not apply this new radar range penalty again in the missile launch code.
Retain a clear distinction between receiver interference and a seeker-deception
event. Dedicated stand-off jammers, false tracks, detailed RF waveforms and
home-on-jam remain outside the first component.

### Directional scope interference

Proposed presentation: a green noisy bearing strobe in the jammer's direction.
On the standard top-down scope it is a wedge from the origin through the full
range axis. An eventual bearing/range projection uses a vertical band instead.
It must not start or stop at the jammer's actual distance: passive noise alone
provides no target range in this model. Do not draw a selectable contact, target
identity or altitude from the noise indication alone.

Drive brightness/density from received I, reduced by the receiver's angular
filtering, and clamp it for readability. Agent-proposed density is
`0.35 * I / (1 + I)` within the bearing strobe, with a lower-density sidelobe
haze using the radar's floor. Use a deterministic 10 Hz texture update based on
simulation tick and an independent display seed; never consume combat RNG.
Fade presentation over 0.25 seconds. Immediate authoritative support changes
must not wait for this cosmetic fade. Keep text, selector and valid track symbols
legible; a burned-through target may appear over a still-noisy background.

Quantize indicated bearing to 2 degrees and use a minimum 2-degree visual
half-width so noise does not disclose perfect angular precision. The physical
coupling width remains independent of raster readability. Multiple emitters
combine with bounded brightness; a strong emitter must not turn every pixel
white. When own radar is off or failed, hide its live noise indications.

Use one interference result for detection and display. The renderer receives
bearing and intensity, not true emitter range/identity. Range selection changes
projection only; it must not change interference strength or reveal jammer range.
The key player experience is a noisy direction, unreliable contacts there,
cleaner sectors elsewhere, and targets returning as ownship burns through.

### Selection, weapon tracking and contact loss

**Single-track only**, requested by John. Maintain one optional selected ID, one
acquisition timer and one optional acquired fire-control track across radar and
IR together. A list of search contacts and stored observations is not a list of
weapon tracks. No background acquisition, track queue, simultaneous radar/IR
locks, automatic promotion or multi-target firing solution. TWS continues showing
other search returns while only the selected target can acquire tracking.

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

Preserve each weapon's existing guidance requirements. Fire-and-forget radar and
IR weapons continue using their own target and seeker rules after cockpit lock
or radar power is lost. A weapon requiring continuous launcher illumination loses
support when the aircraft switches targets or otherwise breaks the required lock.
Its existing loss/reacquisition rules decide the outcome; loss of support does
not itself authorize a fabricated instant destruction rule. IR seekers do not
add another cockpit track. Do not introduce a mandatory mid-course support phase
or an activation distance for all active weapons: exact activation behaviour
remains unresolved, and this pass preserves the existing weapon model. No
multi-track aircraft support is implied by the TWS name.

John's requested click behaviour takes precedence over the retail target-cycle
restriction: clicking a current contact selects its stable target ID immediately,
including a search-only RWS contact. Selection persists without holding the
mouse, re-clicking or periodically re-designating. Empty clicks leave it alone;
explicit clear or clicking another current contact changes it. Selection supplies
the existing target view and its gamified identification. It is not a weapon lock.

Clear selection when the active sensor no longer has a current observation:
notch, look-down, jamming, terrain masking, range/angle exit, power/failure, crash
ending airborne existence, or removal. Loss of weapon tracking alone does not
clear a still-observed search contact. A missile-envelope failure also does not
clear selection. Reappearance creates an unselected contact; there is no hidden
sticky ID or automatic reacquisition of a lost selection.

A display zoom that moves a still-observed contact outside the plotted range does
not itself lose the sensor observation or selection. RWS/TWS changes preserve
selection while the underlying return remains current; RWS removes radar weapon
support. Channel switching preserves the ID only when the new channel has a
current observation of it. These distinctions are agent interpretations of the
requested persistence, documented explicitly for review.

Proposed fitted host cadence stays at fixed 120 Hz. Selection appears immediately;
opinionated weapon-track acquisition takes 0.5 seconds (60 consecutive valid
steps) in TWS or the selected IR tracking volume. Radar support and IR tracking
remain separate. RWS never provides radar weapon lock, even for a selected target.
Master-arm-safe and zero-ammunition states do not erase a sensor track.

Loss of current observation clears selection and weapon support immediately.
Keep its final observation as an unselected coasting symbol for 1 second (120
steps), with no predicted motion or updated position. It is visibly stale and
non-selectable. After it expires, only enabled history samples may remain until
their independent age limit; they do not preserve an active track or target view.
A new current observation after a gap requires a fresh click and track acquisition.
No random per-render-frame rolls or autonomous target selection are introduced.

### History trails, based on the USNF manual

The USNF manual's historical mode draws a sequence of past contact positions,
enabled with Y or the scope Y button. It does not provide timing or capacity;
[source evidence](formats/radar.md#usnf-history-and-ir-functional-reference)
keeps that limit explicit. Reproduce that function, using these agent-proposed
values in TWS, RWS and IR air-to-air:

- Default history off. Y and the Y button toggle the same preference.
- Record one actual sensor observation every 0.5 seconds (60 simulation steps),
  at most 8 past observations per target per channel, with maximum age 4 seconds.
  Store simulation timestamp and observed world position, not old screen pixels.
- Draw small progressively dimmer dots, separate from the current contact symbol
  and heading tail. Reproject samples through the current scope transform when
  ownship moves or the range/layout changes. Do not connect across missing samples.
- No extrapolation, no points after loss, no target identity inferred from trails,
  no selecting or firing at historical dots. The current return is always distinct.
- Collect history while its display is off, so enabling it shows the recent
  observations immediately. Inactive channels collect no new samples. Switching
  channels does not fuse their observations; RWS/TWS share the radar channel.
- Age samples using simulation time, freeze while paused and clear on mission
  restart or aircraft change. Retain bounded history for lost or destroyed contacts
  until its normal expiration; never fabricate a trail for an undetected object.

Presentation timing is independent of rendering and independent of the 1-second
stale contact marker. Contact samples from different channels remain separate even
when they refer to the same physical ID.

### Infrared air-to-air mode

First release supports only aircraft with installed IR sensors listed in the
[retail equipment table](spec/radar.md#other-installed-sensor-channels). Carrying
an IR missile does not grant the aircraft a full IRST scope. An absent/failed
IR sensor is unavailable rather than borrowed from another aircraft.

Proposed initial IR effective range is
`min(nominal_IR_range, nominal_IR_range * sqrt(PT_IR_signature / 100))`.
Apply its own SEE search/track angles and ranges plus terrain visibility.
No RF jammer/noise, radar notch, look-down coefficient or RCS aspect factors
apply to this channel. Use base PT IR signature initially; engine/throttle heat,
rear-aspect bonuses, weather attenuation and post-destruction cooling remain
explicit fitted limitations, not invented retail capability. IR operation is
passive and does not emit radar energy or confer radar missile illumination.

For sensors with search 9/track 10 nmi, new contacts require the search envelope.
An already selected acquired IR track can stay current within its own tracking
envelope, even just outside search coverage. It cannot acquire an unseen target
at 10 miles. Current retained tracks may be selected/displayed as observations;
the wider track range does not grant hidden world-object access.

I requests IR and turns off radar emission. R returns to active radar using its
existing power action; selecting IR again after powering radar down still works.
The on-screen M button cycles only available radar/IR channels in this release;
radar channel RWS/TWS remains automatic by display range. Keep the recovered
range ladder for scope zoom in both channels; selecting a larger scale does not
extend the IR device's physical coverage. Label IR clearly and clear RF noise
from that page. If no IR is available, M or I reports unavailable and preserves radar
state. Exact default-key migration is covered in delivery below.

IR selection drives the same target view and existing IR seeker launch checks.
It never bypasses a missile's own seeker envelope, ammunition, arming or radar
requirement. Existing radar-emission and missile lifecycle rules remain independent
of merely having an IR-selected target. Preserve the target view's gamified IFF;
no interrogation simulation or new radar-scope friend/foe classification is added.

### Destroyed aircraft remain sensor objects

John requested keeping dead opponents visible, an intentional departure from his
reported retail behaviour. Treat combat viability, physical existence and sensor
observation as separate states. HP reaching zero must not delete a radar/IR
return, selection, or its history while the aircraft is still airborne and the
active sensor can observe it. It may still be clicked as a current contact.
Identification and known damage status stay in the target view, not an omniscient
DEAD label automatically applied to every scope return.

Disable destroyed-aircraft combat actions and keep damage/kill accounting final.
Preserve the existing weapon rejection of destroyed targets where applicable;
that rejection must not erase the observation. Existing missile retirement rules
are not implicitly changed into wreck engagement. Emissions cease when the
relevant equipment is destroyed or powered off, independently of passive radar
reflection or the fitted IR signature. A radar return is not proof of radiation.

Audit current HP-based movement, visibility, designation and target-view filters.
Retain a physical airborne remnant until impact/removal, using existing remnant
motion where available. If absent, add only the minimal fitted ballistic fall to
impact, not new AI or a full breakup simulation. A grounded wreck ends the active
A2A observation/selection; its existing trail can age out normally. This does not
remove the wreck from the world or implement A2G acquisition. Detailed wreck,
debris, fire and cooling behaviour belongs with future object-system work.

## RCS instrument and shared aspect model

Include page 0 (RCS) in this implementation pass. Its [retail contract](spec/rcs.md)
explains the player's exposure contour and surrounding emitter symbols. Preserve
the retail chrome/font and heading-relative 0/90/180/270 presentation. Keep RCS
and active radar separate: turning own radar off does not disable a passive
exposure display. The outer ring is a display scale, not a radar beam boundary.

Agent-proposed signature model: transform the unit direction from target aircraft
to observing radar into the target's body axes. Let f, r and u be its forward,
right and up components. Start with:

`effective_signature = PT_radar_signature * (f*f + 2*r*r + 4*u*u) * configuration`

Proposed configuration multiplier is
`(1 + 0.25*gear) * (1 + 0.15*flaps) * (1 + 0.50*bay_open)`, using actual deployed
fractions 0..1, not command switches. Components absent from an aircraft contribute
zero deployment. These weights are agent-proposed gameplay tuning, not recovered
RCS measurements. Keep them as profile fields with common defaults, not extra
hardcoded rules for named aircraft. External-store modifiers remain deferred.

At clean configuration, nose/tail is 1x, side is 2x, top/bottom is 4x the PT base.
A radar abeam sees the factor rise from 2x to 4x as the aircraft banks from level
to 90 degrees. A radar directly on the longitudinal axis does not receive that
same bank bonus. This ties exposure to what that observer can see rather than
making every turn globally increase signature. At 180-degree roll the clean
level-side factor returns to 2x. Apply this to targets as well as ownship.
The existing square-root detection law and jammer return comparison both use
this same effective signature. IR uses its own base-signature policy above.

Proposed contour: sample this function around ownship at 5-degree bearing steps
against a documented, level reference radar. Use 25 nmi nominal search range,
reference signature 400, no terrain/clutter/jamming and the same range law as
active detection. Plot those reference detection distances on the passive scope's
range scale. Reference signature is a profile parameter: actual radar profiles
use 100 in the proposed law above; this deliberately weaker reference set uses
400 to keep the common PT-100 contour responsive rather than saturated at every
bearing. A clean PT-100 aircraft yields 12.5/17.68/25-nmi front/side/top reference
radii. Any model reaches its nominal range cap eventually; further signature
increases improve interference margin rather than exceeding that cap.
It is an estimate of directional vulnerability to a reference set,
not an assertion of any particular emitter's knowledge, range, power or lock.
Use 5/10/20/30/50 nmi view scales, default 50, as an agent-proposed reuse of the
current RWR scale. The actual retail RCS +/- semantics remain unknown. Values
beyond the selected scale clip with an over-range indication rather than changing
the underlying signature. The reference-radar calibration is a reviewable knob.

Feed emitter symbols from a shared passive-observation service. This requires a
small explicit emitter registry: pose, RF equipment profile, powered/failed state
and any externally supplied tracking/launch state. Existing manual fixtures can
supply those states; no AI or autonomous scanning/target decisions are added.
Basic first-pass reception is bounded by the selected passive instrument's
50-nmi maximum, terrain visibility and powered compatible emissions. This is a
fitted gameplay receiver, not a claim of measured receiver sensitivity.

Known ranged contacts may be plotted by position. A received emitter with bearing
but no independently available range stays at the outer bearing ring with an
unknown-range marker. Do not convert the jammer-noise field's internal true
position into a ranged or identified RCS contact. Do not show inactive hidden
aircraft just because their world positions exist. Draw ground squares, aircraft
symbols and warning states only to the extent classification/state is available;
otherwise use an unknown-emitter symbol. No enemy/friendly inference without data.

Use the manual's inside/outside contour interpretation as relative exposure
feedback. An emitter outside it can still detect us with stronger equipment;
one inside may be masked, pointed away or unable to track. Do not show a lock
warning merely because a symbol crosses the contour. Own radar and jammer
transmissions affect emission detectability through the emitter model, not by
silently inflating physical RCS. This distinction is our deliberate presentation
choice; document it in the player guide.

Additional acceptance: same PT viewed nose/side/top gives 1/2/4 factors; abeam
bank changes exposure but nose-on roll does not; lowering gear adds the proposed
25 percent smoothly; clearing gear/flaps/bays restores baseline; the RCS contour
and radar/jammer calculations sample the identical signature function. Test
passive operation with own radar off, no ranged-dot leaks from jammer-only
bearings, and no inferred lock from an inside-contour contact.

## Mouse designation and missiles

Proposed interaction: hover shows a green selector; click/release on the same
visible contact designates its stable ID. Use one projection for drawing and
picking at every window size. Agent-proposed tolerance is 7 instrument raster
pixels; empty clicks leave the designation unchanged. Equal-distance ties use
stable target ID. Cancel a press on focus loss, layout/channel changes or window
exit. The simulation revalidates the requested target when applying the command.

Show a selected marker, separate tracking/lock status and an actionable inhibit
reason: radar off, damaged, RWS search only, beyond tracking coverage, masked,
or outside the missile envelope. Those are proposed presentation messages, not
recovered retail strings. Keep the existing clear-designation action. Reject an
unavailable target without silently selecting another one. Record the target ID
in combat replay, rather than replaying screen coordinates.

Proposed shared operations are observe, designate(target_id), clear_designation,
and support_status(target_id). The last returns whether current radar data or
illumination supports that specific target, with a reason when unavailable.
Weapons consult that result for launch and, only when their guidance requires
it, during flight. Retargeting the cockpit must not silently retarget every
missile already in flight. No target selection is automated.

A clicked RWS contact stays selected for information but provides no radar weapon
lock. The player-requested persistent selection deliberately differs from retail
RWS acquisition. Changing to RWS ends radar illumination while preserving a
current observation/selection. Switching designation, losing tracking coverage,
power loss, failure or masking ends illumination. Selection is cleared only under
the observation-loss rules above. A selected but untracked or stale target cannot
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
3. Produce a human-readable capability summary like the spec table. The expected
   cost of a port is reviewing its data, not writing another radar controller.
4. Run the common synthetic coverage, look-down, state-transition and missile
   handoff tests, then a local media audit for that aircraft's actual bindings.

## Delivery and acceptance

Milestones live in [the roadmap](ROADMAP.md#1f-sensors-and-weapons). Each stage
ends with a reviewable result. Begin only after review of this plan.

| Stage | Work | Reviewable exit |
| --- | --- | --- |
| 1. Profiles | Expose PT radar/IR signatures, installed radar/IR channels and ECM inputs; review radar and jammer tuning | All twelve aircraft produce the expected capability summary; no missing sensor silently becomes F18R |
| 2. Shared simulation | Add radar/IR detection, aspect factors, passive emitters, bounded history and persistent selection; separate death from physical contact lifetime | Numeric boundary tests and deterministic headless contact traces pass; no renderer or frame rate affects results |
| 3. Player scope | Use shared contacts/interference; correct range modes; add IR air-to-air, Y history, directional noise, persistent selection, RCS contour and clear track status | Mouse picks the intended target in both layouts and after resize; RWS selection cannot supply weapon lock; history cannot be clicked; target-view IFF stays unchanged |
| 4. Weapon integration | Replace duplicated radar checks with shared support status; preserve weapon-specific launch envelopes and active/semi-active distinctions | Correct target receives launch; each support-loss case passes; recording/replay reproduces selected target and engagement |
| 5. Aircraft tuning pass | Exercise synthetic same-target scenarios for each radar preset and local imported profile; run workspace and rendered checks | Side-by-side capability results and captures reviewed, known approximations recorded, guides updated |

Stage 3 includes migrating saved scope ranges by their old nautical-mile value
to the nearest new range, with equal-distance ties choosing the lower setting.
Do not reinterpret an old index as a different range. New profiles default to the
recovered 10-mile display. Wire M to available radar/IR channels and Y to history;
no HARM or A-G page is enabled by this change. Keyboard cycling and mouse clicks
use the same current-observation eligibility, including selectable RWS contacts.

The current defaults use Y for the target-jammer fixture and I for an incoming
weapon fixture. Agent-proposed migration: Y becomes history, I becomes IR, and
those two development commands move to Shift-Y and Shift-I. Keep their named
actions, CLI commands and replay commands; preserve user-customized bindings.
Add named history/channel actions to the controls editor and report conflicts
rather than overwriting a custom profile. O's existing radar-mode action becomes
the available sensor-channel cycle rather than cosmetic RWS/TWS toggling. Record
all default changes in INPUT.md and FLIGHT-CONTROLS.md when implemented.

Stage 4 must explicitly define each currently supported radar weapon's
requirements from existing reviewed weapon configuration. Do not silently give
all active missiles a new terminal activation range; preserve the current
supported active/semi-active lifecycle and report its approximation. Capture
radar/IR channel, range/power/selection changes, physical contact lifetime and
target jammer inputs in deterministic
replay, versioning its format when needed and preserving existing supported tapes.

### Acceptance cases

Use source-free synthetic fixtures for the automated cases:

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
  off/failure, masking, out-of-track coverage, RWS and changed designation remove
  illumination. Launch an independent missile at A, then acquire B and launch
  another: their stored target IDs and pursuit remain A and B respectively, with
  only one cockpit track. Repeat with IR guidance. For a continuous-lock weapon,
  switching to B removes support for A and invokes its existing guidance-loss
  rules; unrelated designation must never count as illumination of A. Radar-off
  cases preserve independent guidance while removing required launcher support.
  Lost/stale cockpit tracks cannot create a new guided launch.
- Replay: the same inputs produce the same contacts, designation, lock transitions
  and shots under different rendering rates, with no retail fixtures committed.

Run required formatting, Clippy, Rust tests/build, Python tests, asset and document
checks. Run rendered smoke and inspect representative captures after UI changes.
Do not alter flight adapter defaults. The baseline records what was actually
validated and which gameplay pieces remain opinionated or fitted. No retail
comparison is required to validate these local contracts, and none establishes
retail parity.
