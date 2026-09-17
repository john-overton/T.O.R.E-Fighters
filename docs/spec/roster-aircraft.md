# REDFOR and F-22A aircraft

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation contract, 2026-09-16. John requested these seven player aircraft.
No autonomous behavior is included. Source identities, hashes and validation
belong in [the acceptance record](../baselines/aircraft-roster-expansion.md).

## Source configuration

Use the exact PT identities below, their own source mass, fuel, thrust,
G envelopes, loading, controls, departure fields, stations and audio.
These are game configuration values, not real-aircraft performance claims.

| Aircraft / PT | Empty lb | Fuel lb | Max takeoff lb | Military lbf | Burner lbf | Roll limit / acceleration / release, degrees/s and degrees/s² |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| MiG-29 Fulcrum-C / MIG29.PT | 18025 | 13000 | 36375 | 22220 | 36600 | 270 / 362 / 724 |
| Su-27 Flanker-B / SU27.PT | 36000 | 19000 | 60000 | 40755 | 59510 | 225 / 286 / 571 |
| MiG-21 Fishbed / MIG21.PT | 12824 | 4534 | 20725 | 8598 | 13120 | 180 / 214 / 427 |
| Su-25 Frogfoot-A / SU25.PT | 41885 | 22664 | 90390 | 35270 | 0 | 105 / 106 / 212 |
| MiG-23 Flogger-B / MIG23.PT | 23589 | 11704 | 41556 | 17310 | 24728 | 225 / 286 / 571 |
| Su-35 / SU35.PT | 40564 | 22000 | 74956 | 40755 | 58642 | 225 / 286 / 571 |
| F-22A Raptor / F22.PT | 30000 | 25000 | 72000 | 64000 | 78000 | 360 / 526 / 1352 |

Su-25 has no afterburner: burner commands must not produce thrust, flame, audio
or rumble. F-22 spinEntry=2 disables spin entry; the others retain their own
source departure settings. All seven have zero VTOL nozzle limits. Do not add
player-adjustable vectoring or infer modern Su-35S capabilities from its name.

## Fitted handling

Agent choice: use the existing [additional-aircraft response fit](additional-aircraft.md#aircraft-and-flight)
independently in each model, including source roll and auxiliary controls.
Each model owns its configuration. No new adapter defaults or shared-aircraft
tuning changes. Preserve fixed 120 Hz execution and externally clean free flight.
Ground clearances, fitted from deployed gear bounds at the host shape scale:
MiG-29 20/3 ft, Su-27 23/3 ft, MiG-21 6 ft, Su-25 23/3 ft,
MiG-23 20/3 ft, Su-35 22/3 ft and F-22 23/3 ft.

## Presentation and manual systems

Use each plane's FA base exterior and texture. Source HUD references select
SU33CC / SU33 cockpit art for MiG-29, MiG-23 and Su-25, AV8 for Su-27,
MIG21 / M21 art for MiG-21, and F22 for F-22A. Su-35 has a null PT HUD
reference; the agent selects its own SU35.HUD and SU35 cockpit resources,
as an explicit opinionated player-presentation choice.
MiG-29, Su-27, Su-25 and MiG-23 have no PTS companion on this media.
Their new player integration uses PT data and the referenced shared cockpit. Do not substitute
another aircraft's flight model because it shares cockpit art.

Use each profile's gun, radar, visual sensor, ECM and default stores through
the existing manual systems. Imported extra sensors do not imply new sensor
modes. Preserve the Su-35 AA11B station internal flag, including its
non-jettisonable ammunition. Switching aircraft resets equipment, weapons, damage and device state.

Fitted moving surfaces, rigid gear travel, continuous brakes, MiG-23 visual
sweep and F-22 main bays follow the [animation contract](aircraft-animation.md).
Flames scale longitudinally from their own source root with exhaust fraction.
Reviewed round afterburning outlets use the existing
[engine material and pink-mask grading](engine-material.md).
Su-27 and Su-35 use the reviewed flat center-mirror fill with the existing
fitted rear camera; the other new cockpits have no enabled live mirror mask.
MiG-23 uses twice the host one-third-foot scale for its source exponent 9;
the other new aircraft have exponent 8 and use one-third foot per source unit.

## Unknowns and limits

Original continuous control-surface schedules, MiG-23 sweep aerodynamics,
original F-22 bay sequencing and side bays, complete cockpit instruments, side mirror masks,
LOD, shadows, damage models and complete store placement are not established.
Preserve neutral geometry for unreviewed controls; do not borrow rig offsets.
Next research: inspect each aircraft's shape groups and source control consumers,
then specify schedules or explicitly document independent fitted hinges.
Retail comparison is unavailable, and this port does not claim retail parity.
