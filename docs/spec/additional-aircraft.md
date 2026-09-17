# F-14D, A-4E and X-31 aircraft

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation contract, 2026-09-16. John requested Fighters Anthology sources
for every component. USNF-ATF is a research guide only. No AI is included.
Source identity and validation belong in the aircraft port baseline.

## Aircraft and flight

Select the F-14D from F14.PT, A-4E from A4E.PT and X-31 EFM from F31.PT.
Do not substitute F-14B or the desert, forest or snow X-31 variants.
Use each aircraft's own mass, thrust, fuel, loading, G envelopes, departure
parameters, equipment and stores. These source configuration values are:

| Aircraft | Empty lb | Internal fuel lb | Maximum takeoff lb | Military thrust lbf | Afterburner thrust lbf | G rows |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| F-14D | 40104 | 15741 | 74349 | 28435 | 41800 | -4 through 7 |
| A-4E | 10800 | 4434 | 25000 | 11200 | 0 | -4 through 7 |
| X-31 EFM | 16225 | 9975 | 40200 | 21000 | 32000 | -4 through 9 |

G rows are speed/altitude boundaries, not constant allowed load factors.
A-4E has no afterburner: commands must not activate its thrust, sound, flame or
rumble. F-14D and A-4E have hook controls; X-31 has none. Hook presentation does
not establish carrier arrestment. Preserve the existing three flight adapters,
fixed 120 Hz simulation and externally clean ordinary free flight.

Until measured aircraft-specific response laws are available, the agent chooses
the existing shared response fit independently for these three models: legacy
pitch response 0.1 s; alignment 0.7; rudder 0.12;
sideslip drag 0.5; trim 2 degrees plus 1.25 degrees per requested G above 1,
scaled by (450 knots / max(TAS,150 ft/s)) squared and limited to -12..20 degrees;
thrust lapse exp(-altitude/70000 ft). Devices take 3 s, exhaust 0.2 s, controls
0.1 s; throttle changes at 0.35/s, afterburner requires throttle above 0.95.
Ground response uses tire scrub 8/s, rolling deceleration 0.8 ft/s² and braking
18 ft/s². These are **fitted** agent choices, not measured original response.
Each model owns its configuration; tuning one must not alter another.
Primary roll now follows each PT's signed limits and acceleration/deceleration,
with speed authority rising linearly to full authority at twice the clean stall
speed. F-14D: 225 degrees/s, acceleration 286 degrees/s², release 571 degrees/s².
A-4E: 180, 214, 427 respectively. X-31: 345, 498, 996 respectively.
These **spec-derived** values apply in legacy flight and to F-14/X-31 hybrid
flight. A-4 hybrid uses the [requested roll override](#a-4-roll-tuning) below.
Existing Hornet/Rafale handling and adapter selection are unchanged.
The host uses a continuous rate limiter: full-stick acceleration uses the PT
rate, partial stick scales it with a quarter-rate floor, reversal adds half the
release rate. This continuous approximation omits source integer quantization.

FA's low-speed auxiliary control is available on all three aircraft, not only
X-31. All have ±90 degrees/s auxiliary roll, pitch and yaw limits, with each
plane's primary roll acceleration/release values. Authority is zero on the
ground, rises with throttle to full at 50%, and fades linearly from full at
zero airspeed to zero at 220 ft/s (about 130 knots). The recovered FA consumers
are documented in [native flight](../formats/native-flight.md). All three PTs
have zero VTOL nozzle limits: do not add a Harrier-style nozzle control.
Use ordinary roll, pitch and rudder inputs automatically. These are
**spec-derived** rate and authority rules. Engine-off or empty fuel immediately
removes auxiliary authority in the host, a **fitted** physical guard chosen by
the agent. Auxiliary rotation does not redirect the host's translational thrust.
The restricted table adapter keeps its existing recovered auxiliary consumer.

X-31 vector presentation is **fitted**: deflect the existing exhaust plume up to
15 degrees in pitch and yaw in proportion to achieved auxiliary pitch/yaw rate
relative to 90 degrees/s. Positive pitch sends exhaust downward, positive yaw
sends it left. The three existing source paddles follow the same demand, even
without afterburner. Each paddle rotates about its two forward root vertices.
For a normalized hinge axis (x,0,z), its fitted angle is x*pitch - z*yaw,
clamped to ±15 degrees. Both skins receive the same transform; neutral demand
preserves the source mesh. No new paddle geometry is invented. This animation
was requested by John; the hinge interpretation and mixing are agent choices.
Original animation schedules remain unknown. Only the plume requires the
source afterburner flame to be visible; paddle motion does not require burner.
Fitted ground clearance is 8 ft for F-14D, 26/3 ft for A-4E and 16/3 ft for
X-31, using the projected deployed gear bounds, not recovered contact physics.
X-31 spinEntry=2 disables spin entry in the existing hybrid departure model.

## Presentation and systems

Use each aircraft's base FA shape, textures, original cockpit and referenced
engine/start/stop samples. Do not overlay the toolkit SWPATCH F-14 model.
Use actual device state and live instrument telemetry. Never fill unavailable
instruments or sensors with invented healthy values or contacts.

Manual weapons and sensors use the selected PT's stations and referenced
JT/SEE/ECM/GAS definitions through the existing supported systems. Changing
planes resets weapon, sensor, damage and device state to that plane's settings.
Full weapon catalog parity is not implied by import success.

## Fitted exterior behavior

Agent choices for the initial port, 2026-09-16: preserve FA neutral geometry,
texture mapping and source device endpoints. Keep the existing host scale of
one-third foot per unit, multiplied by four for F14's exponent-10 shape.
A4 and F31 have exponent 8. Absolute scale remains a host fit.

F-14 wings sweep an additional 0..48 degrees linearly over 400..700 knots TAS.
Flap extension multiplies that angle by (1 - flap fraction), holding fully
extended wings with full flaps. Sweep affects wing meshes, flaps and fitted
wingtip vapor origins. No extra aerodynamic effect is asserted. FA's F14 CE
points do not coincide with its quantized base mesh; use the reviewed visible
wing tips as fitted attachments. Do not describe these as decoded CE placement.

Flaps droop up to 0.4 rad. F-14 tailplanes mix -0.3 rad pitch and ±0.2 rad roll;
A-4 elevators use -0.3 rad pitch and outboard ailerons ±0.2 rad roll.
The A-4 elevator is the complete strip aft of fitted source y=-54, z=7 across
both tail halves. Split all eight horizontal-tail faces at this straight hinge,
interpolating texture coordinates; keep the forward stabilizer fixed. Source
polygon diagonals are not hinges. X-31
canards use +0.35 rad pitch; aft panels mix 0.4 rad flap, -0.3 rad pitch and
±0.2 rad roll. Rudders deflect up to 0.35 rad. F-14 fin clipping preserves the
fixed forward area. These hinges and mixing are fitted, not original schedules.

Gear rotates through a fitted quarter turn and disappears at full retraction.
A-4/X-31 side brakes interpolate through 1.05 rad from their open meshes;
F-14 brakes hinge continuously through a fitted 45 degrees to the source raised
pose, around their forward edge; see the [animation contract](aircraft-animation.md). A-4's stowed hook rotates
0.9 rad; F-14's projected hook uses a 0.6 rad travel, but its source triangles
collapse through quantization. Give the coincident root vertices a fitted
±0.125 source-unit width, preserving the center and tip, to make the hook visible.
F-14/X-31 flames follow the existing exhaust fraction. A-4 has no flame branch.

Cockpit mirrors use three reviewed flat-fill regions for F-14D and A-4E;
X-31 has no mirror regions. Their optics share the existing fitted rear camera.
X-31 side-mirror art is absent and is not borrowed from another aircraft.

## Unknown behavior

Exact F-14 wing sweep schedules and X-31 physical nozzle/paddle laws are unknown.
Next research: review the FA PT fields, shape branches and original control
consumers, then specify player-visible schedules. Old ATF rig addresses and
real-aircraft expectations do not establish these behaviors. Shape-specific
fitted presentation choices must be recorded before use. Full damage shapes,
LOD, shadows, original instrument composition and retail comparisons remain
outside the current established evidence.

## A-4 roll tuning

John requested 90% of a researched real-world roll rate on 2026-09-16. The
[Marine Flight School Primer](https://skyhawk.org/article-readyroom/marine-primer),
written by Marine flight-school graduates and hosted by the Skyhawk Association,
reports 720 degrees/s for a clean A-4 in its TA-4J training section. It supplies
no test speed, weight, load factor or A-4E-specific test results. It supports a
reported Skyhawk-family figure, not a measured A-4E envelope or FA behavior.

Use 648 degrees/s (90% of 720) as an opinionated hybrid A-4E peak target. Agent
fitted acceleration is 1296 degrees/s², release 2592 degrees/s², reaching peak
in 0.5 seconds and releasing from peak in 0.25 seconds at full authority.
Retain continuous stick scaling and the existing fitted speed authority
clamp(speed/(2*clean-stall-speed),0,1), plus stall/spin control attenuation.
At clean stall speed this permits half the peak before departure attenuation;
it does not assert full real-world roll capability at arbitrarily low speed.
Retain FA's 180/214/427 rate/acceleration/release values in legacy mode and leave
the restricted research adapter unchanged. No other aircraft receives this
roll override. Real A-4E speed/load-dependent roll curves remain unknown.
