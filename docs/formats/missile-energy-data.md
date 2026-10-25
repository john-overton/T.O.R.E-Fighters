# Missile energy data evidence

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation research, 2026-10-25. In-progress public-reference anchors for
[energy flight](../spec/missile-energy.md), not a complete or calibrated weapon
table. These constants are not yet applied to imported loadout weights or live
flight. Each field identifies its evidence; unreviewed weapons remain absent.
No retail bytes are stored here. The code table is
`tore-formats::missile_data::REFERENCES`.

## Reviewed primary references

| Evidence ID | Fields and interpretation | Source |
| --- | --- | --- |
| NAVAIR-AMRAAM | A/B/early-C mass 348 lb, diameter 7 in. Table converts lb to kg. Exact game subvariant is unspecified; this is the early-family physical reference, not AIM120D. | [NAVAIR](https://www.navair.navy.mil/product/AMRAAM) |
| USAF-AMRAAM-LOWER-BOUND | 20+ statute miles, converted to 32.18688 km. A lower bound only, not a maximum-range calibration target. NAVAIR lists range classified. | [USAF](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104576/aim-120-amraam/) |
| USAF-SIDEWINDER | 190 lb and 5 in. Use exact pound conversion rather than the page's inconsistent rounded 85.5 kg. The same page identifies the X as imaging and sharing the M motor. | [USAF](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104557/aim-9-sidewinder/) |
| NAVAIR-SIDEWINDER | Early family 188 lb, 5 in from the archived Navy fact sheet. Current X page instead gives 186 lb and describes Block II; do not silently substitute its generation. | [Archived Navy fact sheet](https://www.agriculturedefensecoalition.org/sites/default/files/file/us_navy/417W_1_2012_U.S._Navy_Fact_File_AIM_9_sidewinder_Missile_Website_May_24_2012.pdf), [current NAVAIR](https://www.navair.navy.mil/product/AIM-9X-Sidewinder) |
| NAVAIR-PHOENIX-C | Variant C: 463 kg, 380 mm, 150 km. Same source's family summary says over 100 nmi; retain the distinction rather than treating the two as one measured envelope. | [NAVAIR historical fact sheet](https://www.navair.navy.mil/node/12701) |
| ROE2005-R27 | R/R1 253 kg/230 mm; ER/ER1 350/260; T/T1 245/230; ET/ET1 343/260. Manufacturer printed ranges 60/62.5/65/80 km, respectively. Export variants are explicit proxies for game-family identities, not an assertion of identical domestic performance. The unusual ER/T ordering needs corroboration before calibration. | [Rosoboronexport 2005 catalogue, printed p.119](https://server.3rd-wing.net/public/tiengo/Doc/r%C3%A9aliste%20avionics%20russes.pdf#page=119) |
| ROE2005-R73 | R73E 105 kg, 170 mm, 30 km, an export proxy for R73. Target maneuver limits in the catalogue are not missile structural G limits. | [Manufacturer, p.118](https://server.3rd-wing.net/public/tiengo/Doc/r%C3%A9aliste%20avionics%20russes.pdf#page=118) |
| ROE2005-RVVAE | 175 kg, 200 mm. 50 km against fighter targets and 80 km against bomber targets, not a single guaranteed range. Initial reference uses fighter value as an export proxy. | [Manufacturer, p.120](https://server.3rd-wing.net/public/tiengo/Doc/r%C3%A9aliste%20avionics%20russes.pdf#page=120) |
| ROE2005-R33 | R33E 490 kg, 380 mm, 120 km; export proxy. Game AA9 remains held. | [Manufacturer, p.118](https://server.3rd-wing.net/public/tiengo/Doc/r%C3%A9aliste%20avionics%20russes.pdf#page=118) |
| USAF-MAVERICK-G | 670 lb and 12 in. Convert the pounds exactly; do not propagate the page's rounded 301.5 kg. Range classified. | [USAF](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104577/agm-65-maverick/) |
| USAF-HARM | 800 lb and 10 in. Range 30+ statute miles is a lower bound (48.28032 km), not exact maximum. The exact early-game motor variant remains unknown. | [USAF](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104574/agm-88-harm/) |
| MBDA-AM39 | 670 kg; the 350 mm/70 km family values need corroboration against the downloaded manufacturer sheet before becoming calibration inputs. | [MBDA sheet](https://www.mbda-systems.com/media/19213/download) |

The range IDs ending EXPORT-PROXY or FIGHTER-PROXY refer to the associated
manufacturer row above and label an estimated family mapping. These are not
new weapon identities or catalog additions.

## Remaining range evidence

`SIDEWINDER-OPEN-ESTIMATE` is a provisional 18 km L/M open-reference figure;
`SHARED-MOTOR-FITTED` uses the same initial range target for the early X until
the shared motor and different drag are calibrated. `MAVERICK-OPEN-ESTIMATE`
is a provisional 20 statute mile open-reference figure. They are not verified
primary maximum-range measurements and must not be presented as such. Next
step: corroborate with period government/manufacturer data or retain an
explicit estimate and its open reference in the final per-field table.

No motor thrust, propellant fraction, Cd, structural G, fuze radius or seeker
rate is established by these mass/range references. Fitted values belong in
the behaviour specification and calibration report. Do not claim a successful
range fit proves those separate physical quantities.
