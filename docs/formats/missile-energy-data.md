# Missile energy data evidence

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-25. The energy model uses public
physical anchors below, with explicitly fitted aerodynamic and motor values.
The authoritative numeric table is `tore-formats::missile_data::REFERENCES`.
Mass and diameter share `dimensions_evidence`; range has `range_evidence`.
These are public facts/estimates and authored game coefficients, not retail
media. [Behaviour](../spec/missile-energy.md) owns the equations; calibration
results belong in [the feature baseline](../baselines/missile-energy.md).

## Reviewed primary references

| Evidence ID | Fields and interpretation | Source |
| --- | --- | --- |
| NAVAIR-AMRAAM | A/B/early-C mass 348 lb, diameter 7 in. Table converts lb to kg. Exact game subvariant is unspecified; this is the early-family physical reference, not AIM120D. | [NAVAIR](https://www.navair.navy.mil/product/AMRAAM) |
| USAF-AMRAAM-LOWER-BOUND | 20+ statute miles, converted to 32.18688 km. A lower bound only, not a maximum-range calibration target. NAVAIR lists range classified. | [USAF](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104576/aim-120-amraam/) |
| USAF-SIDEWINDER | 190 lb and 5 in. Use exact pound conversion rather than the page's inconsistent rounded 85.5 kg. The same page identifies the X as imaging and sharing the M motor. | [USAF](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104557/aim-9-sidewinder/) |
| NAVAIR-SIDEWINDER | Early family 188 lb, 5 in from the archived Navy fact sheet. Current X page instead gives 186 lb and describes Block II; do not silently substitute its generation. | [Archived Navy fact sheet](https://www.agriculturedefensecoalition.org/sites/default/files/file/us_navy/417W_1_2012_U.S._Navy_Fact_File_AIM_9_sidewinder_Missile_Website_May_24_2012.pdf), [current NAVAIR](https://www.navair.navy.mil/product/AIM-9X-Sidewinder) |
| NAVAIR-PHOENIX-C | Variant C: 463 kg, 380 mm, 150 km. Same source's family summary says over 100 nmi; retain the distinction rather than treating the two as one measured envelope. | [NAVAIR historical fact sheet](https://www.navair.navy.mil/node/12701) |
| ROE2005-R27 | R/R1 253 kg/230 mm; ER/ER1 350/260; T/T1 245/230; ET/ET1 343/260. Manufacturer printed ranges 60/62.5/65/80 km, respectively. Export variants are explicit proxies for game-family identities, not an assertion of identical domestic performance. Use the separately identified ODIN domestic-family estimate for calibration, rather than silently changing these printed export numbers. | [Rosoboronexport 2005 catalogue, printed p.119](https://server.3rd-wing.net/public/tiengo/Doc/r%C3%A9aliste%20avionics%20russes.pdf#page=119) |
| ROE2005-R73 | R73E 105 kg, 170 mm, 30 km, an export proxy for R73. Target maneuver limits in the catalogue are not missile structural G limits. | [Manufacturer, p.118](https://server.3rd-wing.net/public/tiengo/Doc/r%C3%A9aliste%20avionics%20russes.pdf#page=118) |
| ROE2005-RVVAE | 175 kg, 200 mm. 50 km against fighter targets and 80 km against bomber targets, not a single guaranteed range. Initial reference uses fighter value as an export proxy. | [Manufacturer, p.120](https://server.3rd-wing.net/public/tiengo/Doc/r%C3%A9aliste%20avionics%20russes.pdf#page=120) |
| ROE2005-R33 | R33E 490 kg, 380 mm, 120 km; export proxy. Game AA9 remains held. | [Manufacturer, p.118](https://server.3rd-wing.net/public/tiengo/Doc/r%C3%A9aliste%20avionics%20russes.pdf#page=118) |
| USAF-MAVERICK-G | 670 lb and 12 in. Convert the pounds exactly; do not propagate the page's rounded 301.5 kg. Range classified. | [USAF](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104577/agm-65-maverick/) |
| USAF-HARM | 800 lb and 10 in. Range 30+ statute miles is a lower bound (48.28032 km), not exact maximum. The exact early-game motor variant remains unknown. | [USAF](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104574/agm-88-harm/) |
| MBDA-AM39 | 670 kg and 350 mm, confirmed in the manufacturer sheet. Its range is described qualitatively; the older manufacturer sheet supplies the separate 70 km anchor. | [MBDA sheet](https://www.mbda-systems.com/media/19213/download) |

The range IDs ending EXPORT-PROXY or FIGHTER-PROXY identify the corresponding
manufacturer row above. Proxy selection is an agent decision, not John's
choice of a specific variant. A fit to a maximum does not validate the motor,
seeker, acceleration or complete envelope independently.

## Additional field evidence and estimates

| Evidence ID | Fields and interpretation | Source |
| --- | --- | --- |
| OPEN-AMRAAM-A-50-70 | 60 km midpoint of the published 50 to 70 km early-family estimate, not AIM-120D. | [Source](https://www.designation-systems.net/dusrm/m-120.html) |
| SIDEWINDER-OPEN-ESTIMATE / OPEN-SIDEWINDER-B / OPEN-SIDEWINDER-B-TAIL | M 18 km estimate. B 70 kg, 127 mm, 4.8 km tail engagement, 17,800 N for 2.2 s, 12 G, 11 deg/s and 4 deg full field (2 deg half-angle). | [Source](https://www.designation-systems.net/dusrm/m-9.html) |
| SHARED-MOTOR-FITTED | X uses M 18 km calibration target because USAF identifies the shared motor. This is an agent fit, not a claimed measured X maximum. | [Source](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104557/aim-9-sidewinder/) |
| ODIN-R27-MAXIMUM-ESTIMATE | R/ER/T/ET maximum figures 73/117/63/104 km. Distinguish effective head/tail envelopes and the export catalogue. | [Source](https://odin.t2com.army.mil/WEG/Asset/b4eb3c7bffab161b7b67ce580544f84e) |
| RWD-R3S / RWD-R3S-TAIL | R3S 78 kg, 127 mm, 7.6 km. Tail calibration condition is agent-fitted for this rear-aspect seeker, not a measured launch condition. | [Source](https://www.rwd-mb3.de/ftechnik/pages/r3s.htm) |
| OPEN-R60M | M variant 44 kg, 120 mm, 10 km upper range, 47 G missile load estimate. Not the original R60 43.5 kg/7 km. | [Source](https://weaponsystems.net/system/218-Molniya%20R-60) |
| OPEN-MAGIC2 | 89 kg, 160 mm, 15 km upper estimate. The same page gives shorter conditional high/low ranges; no guarantee at every altitude. | [Source](https://www.redstar.gr/en/greek-aviation/hellenic-aerial-means-af/weapons/air-to-air-missile/r-550-magic-2-short-range-air-to-air-missile.html) |
| OPEN-SPARROW-E / OPEN-SPARROW-E-HEAD / OPEN-SPARROW-FM-PROXY / OPEN-SEASPARROW-M-PROXY | E 197 kg/203.2 mm/35 km favorable head-on; unqualified AIM7 uses an explicit F/M physical proxy 231 kg/203.2 mm/70 km; Sea Sparrow M proxy 26 km. | [Source](https://www.designation-systems.net/dusrm/m-7.html) |
| OPEN-SUPER530D | 270 kg, 263 mm, 40 km estimate. Game R530 is treated as Super 530D; no silent switch to the smaller original R530. | [Source](https://c-aea.fr/fr/50-collection/armement) |
| MBDA-MICA / DGA-MICA | MICA 112 kg/160 mm manufacturer dimensions; 60 km DGA public upper range. | [Source](https://www.mbda-systems.com/media/20541/download) |
| DGA-MICA | 60 km, Vision 13, February 2011. | [Source](https://archives.defense.gouv.fr/content/download/108415/1057854/vision_13_fevrier_2011.pdf) |
| OPEN-PL7 | 89 kg, 157 mm, 14 km open estimate, references SinoDefence and AusAirPower. | [Source](https://en.wikipedia.org/wiki/PL-7) |
| OPEN-R40-FAMILY | 475 kg, 355 mm, 30 km base-family estimate. Conflicting diameter figures and unspecified radar/IR suffix remain unresolved; held availability is unchanged. | [Source](https://www.globalsecurity.org/military/world/russia/aa-6-specs.htm) |
| MAVERICK-OPEN-ESTIMATE | G 20 statute miles (32.18688 km), an open estimate where USAF classifies range. | [Source](https://www.airandspaceforces.com/weapons/agm-65/) |
| OPEN-MAVERICK-A / OPEN-MAVERICK-LOWER-BOUND | A 209 kg and 12 in; 27 km is a published lower bound, used as a conservative calibration anchor. | [Source](https://www.designation-systems.net/dusrm/m-65.html) |
| OPEN-AM39-RANGE | Older manufacturer sheet: up to 70 km depending on launch altitude/speed. Physical dimensions use the newer sheet above. | [Source](https://www.mbda-systems.com/wp-content/uploads/2015/07/EXOCET-AM39.pdf) |
| GICHD-SA7B-PROXY / GICHD-SA14 / GICHD-SA16 / GICHD-SA15 | SA7B 9.15 kg/72 mm; SA14 10.3/72; SA16 10.8/72; 9M330 Tor 165/230. Pages 198, 199, 200, 211. SA7B and naval Tor are explicit physical proxies. | [Source](https://www.gichd.org/fileadmin/user_upload/Explosive_Ordnance_Guide_for_Ukraine__GICHD___Third_Edition.pdf) |
| GULFLINK-SA7B-PROXY / GULFLINK-SA14 / GULFLINK-SA16 / GULFLINK-SA6 / GULFLINK-SA9 | Public ranges 4.2/4.5/5.2 km. SA6 599 kg, 335 mm, 24 km; SA9 30 kg, 120 mm, 4.2 km. These period estimates have variant uncertainty. | [Source](https://www.gulflink.osd.mil/irfna/irfna_refs/n28en143/airdef.html) |
| US-WEG2011-TOR | 12 km Tor family range. SAN9 uses the same 9M330 physical family, with its game guidance retained. | [Source](https://cgsc.contentdm.oclc.org/digital/api/collection/p4013coll11/id/2090/download) |
| MBDA-MISTRAL-PHYSICAL-PROXY | 18.7 kg, 90 mm, 6.5 km. Physical proxy only: MIS keeps its reviewed game radar guidance. | [Source](https://www.mbda-deutschland.de/wp-content/uploads/2018/04/MISTRAL-ATAM-1.pdf) |
| US-ORDNANCE-STINGER | FIM92A missile 10.4 kg and 70 mm, not complete shoulder-launch system weight. | [Source](https://www.marines.mil/Portals/1/Publications/MCRP%2010-10D.1%20%28SECURED%29.pdf) |
| US-ARMY-STINGER-PLANNING | 4 km unclassified planning range. This is explicitly not the maximum kinematic reach claim. | [Source](https://api.army.mil/e2/c/downloads/2023/01/19/18849503/18-16-maneuver-leader-s-guide-to-stinger-handbook-apr-18-public.pdf) |
| OPEN-R440 | 84 kg, 150 mm, 10 km. Reported motor 20,550 N for 1.75 s then 17,550 N for 2.3 s. Charges 16.1/19 kg; model mass flow remains fitted. | [Source](https://en.missilery.info/missile/crotal) |
| OPEN-V750V-PROXY | 2163 kg, 654 mm booster diameter, 29 km. Explicit V750V proxy for SA2A, not later extended-range S75. A single equivalent body does not simulate stage separation. | [Source](https://en.wikipedia.org/wiki/S-75_Dvina) |
| GICHD-V601-PROXY / OPEN-V601-PROXY | V601/5V27 physical proxy 953 kg and 375 mm main-body diameter; 25 km family range estimate. Booster is represented by equivalent mass/thrust, not a separate body. | [Dimensions](https://www.bulletpicker.com/pdf/Ammunition-Identification-Guide-Lybia.pdf), [range](https://weaponsystems.net/system/1470-S-125%20Neva) |
| OPEN-9M37 | SA13 40 kg, 120 mm, 5 km family estimate. | [Source](https://en.wikipedia.org/wiki/9K35_Strela-10) |
| OPEN-9M311 | 42 kg missile (57 kg is in container), 152 mm booster/76 mm dart, 8 km. SAN11 shares this physical family; no new guidance identity. | [Source](https://en.missilery.info/missile/tunguska/zur) |
| OPEN-V611 | 1844 kg, 650 mm, 30 km base V611; do not apply V611M 55 km. | [Source](https://weaponsystems.net/system/595-V-611) |
| OPEN-OSA-M1-PROXY | 128 kg, 206 mm, 10 km improved 9M33M1 naval family proxy (base M is 9 km). | [Source](https://weaponsystems.net/system/1065-9M33%2BOsa) |
| OPEN-9M38-NAVAL | 690 kg and 400 mm 9M38 family; 25 km naval M22 envelope estimate. | [Physical source](https://www.deagel.com/weapons/9m38/a000997), [naval range](https://en.wikipedia.org/wiki/Buk_missile_system) |
| OPEN-P120 | 3180 kg, 800 mm, upper 150 km (source also reports 110 to 120 km). Sustain thrust range 600 to 950 kgf and 360 to 385 s. Model uses 7600 N/360 s; launch boost 303160 N/3 s is fitted. | [Source](https://en.missilery.info/missile/malahit) |
| OPEN-SHRIKE-B-PROXY | 177 kg, 203.2 mm, B-family 40 km. Generic game AGM45 has no suffix; B is an explicit physical proxy, not a rename. | [Source](https://www.designation-systems.net/dusrm/m-45.html) |
| OPEN-HARPOON / OPEN-HARPOON-A / OPEN-SLAM | Harpoon 540 kg/343 mm/185 km A estimate; SLAM E 627 kg/343 mm/93 km. Shared J402 sustainer 3000 N; burn durations 962/483.6 s and effective exhaust 25000 m/s are fitted. | [Source](https://www.designation-systems.net/dusrm/m-84.html) |
| OPEN-KH23 | 289 kg, 275 mm, 10 km. Burnout mass 225 kg is reported, but phase-resolved mass flow remains unreviewed. | [Source](https://missilery.info/missile/x23) |
| OPEN-KH15S-PROXY | 1200 kg, 455 mm conventional antiship proxy, not the 300 km strategic nuclear trajectory. | [Source](https://weaponsystems.net/system/99-Raduga%2BKh-15) |
| CSIS-KH15S-PROXY | 150 km conventional Kh15A/S estimate; game AS16 active anti-surface guidance is retained. | [Source](https://csis-website-prod.s3.amazonaws.com/s3fs-public/legacy_files/files/publication/150713_Cover_and__Report%20_Gulf_Military_Balance_2015.pdf) |
| ROE2005-KH29L | 660 kg, 380 mm, 10 km laser variant, explicit physical reference for AS14. | [Source](https://server.3rd-wing.net/public/tiengo/Doc/r%C3%A9aliste%20avionics%20russes.pdf#page=122) |
| OPEN-AS30L | 520 kg, 342 mm, 11 km laser variant estimate. | [Source](https://en.missilery.info/missile/as30l) |
| OPEN-FALANGA-M-PROXY | 31.5 kg, 142 mm, 4 km M-family proxy; game AT2 has no variant suffix. | [Source](https://www.samolotypolskie.pl/samoloty/2457/126/3M11-9M11-3M17-9M17-Falanga) |

## Fictional and ambiguous stores

The same data-class derivation applies to fictional/game-only weapons:
AA11B uses the agile IR reference (105 kg, 170 mm, 30 km); AAML the heavy radar
reference (500 kg, 380 mm, 150 km); PL10 concept and AEMP1 the radar reference
(175 kg, 200 mm, 50 km). IDs FITTED-AGILE-IR-CLASS,
FITTED-HEAVY-RADAR-CLASS and FITTED-RADAR-CLASS label these agent decisions.
PL10 is not silently changed into the present Chinese infrared missile.

AS15 and airborne AT12 remain **unknown identity**. Their reviewed game
configuration conflicts with current real-world designations (AS15's record
is a small radar AGM; AT12 is an airborne laser weapon). Neither naming nor
shape establishes which real variant was intended. They retain runtime retail weight
and range, with diameter derived by the common class rule. The FALLBACKS data
rows contain only their fitted drag multipliers, calibrated to that retail
range. They do not invent public physical measurements. Next research step:
find a contemporary game manual/developer identification or period variant
sheet tying the exact store to an unambiguous real weapon. The existing
[decoded inventory](fa-weapons.csv) is the single home for those fallback
record values. No public number is invented to fill that gap.

## Fields without a reviewed public value

All coefficients below are agent-fitted, not claimed manufacturer data:

- Diameter fallback is 0.127*(mass/86)^(1/3) m, limited to 0.07..0.6 m.
  Valid decoded mass converts pounds with 0.45359237 kg/lb. A placeholder
  weight of 0 or 1 uses 15 kg for IR or 250 kg otherwise; reviewed SAM rows
  override these placeholders.
- Initial motor fit uses 35% propellant for rockets and 20% for long cruise
  records. Boost uses 65% of that fuel, effective exhaust 2200 m/s, burn
  clamp(0.2*sqrt(range_km),2,6) s (cruise 3 s). Sustain burns the remainder
  over clamp(0.15*range_km,0,25) s, or cruise
  clamp(1.3*range_m/250,30,1800) s with effective exhaust 10000 m/s.
  Public motor rows override thrust/duration; their mass flow is impulse
  divided by the table's effective exhaust. The impulse of a jet engine
  is not claimed to be a rocket specific impulse.
- Structural limits are 12 G for strike, 20 G for bodies above 400 kg,
  30 G otherwise; imaging A2A is 40 G. B-Sidewinder and R60M use cited limits.
  PN gain is 3.5, imaging 4. Lift and induced-drag coefficients are specified
  in the behaviour document. Effective drag scale is fitted offline to the
  stated anchor, independently for each data row. Large scale factors are
  effective game coefficients, not measured zero-lift body Cd.
- Lifetime is at least twice range_m/300 seconds and at least the decoded
  removal time; long cruise also covers its full motor plus 10 seconds.
  Surface release has at least 12 degrees initial elevation, fitted to leave
  clearance while accelerating. Air release keeps the reviewed ejection.
- Seeker type, mechanical gimbal, active activation distance, fuze radius,
  arming delay and existing damage rules retain reviewed game data. These
  are not established by the public mass/range sources. Seeker slew/FOV
  defaults and decoy generation are fitted in the behaviour document, with
  the cited AIM9B override. No raw susceptibility is clipped before the
  combined probability.

Public numbers missing from this table are not known measurements. The next
research step is a period motor/seeker technical sheet, not another game
simulation's performance table. The class defaults remain acceptable fitted
behaviour under the project provenance policy.

## Loaded aircraft mass changes

`Weapon::carried_weight_lb` applies public/class mass to carried stores and
loadout totals. Decoded `weight` remains intact for old combat rules, effects, the fitted radar-signature weight proxy
and inspection. External-fuel and gun-pod accounting are unchanged. The table
below is pounds per store; aircraft change is the delta times carried quantity.
Surface SAM placeholder weights are included for audit, although aircraft do
not normally carry those stores.

| Store | Decoded lb | Carried lb | Delta lb |
| --- | ---: | ---: | ---: |
| AA10.JT | 560 | 557.769 | -2.231 |
| AA11.JT | 230 | 231.485 | +1.485 |
| AA11B.JT | 230 | 231.485 | +1.485 |
| AA12.JT | 385 | 385.809 | +0.809 |
| AA2.JT | 165 | 171.961 | +6.961 |
| AA6.JT | 1010 | 1047.196 | +37.196 |
| AA8.JT | 145 | 97.003 | -47.997 |
| AA9.JT | 1080 | 1080.265 | +0.265 |
| AAML.JT | 1650 | 1102.311 | -547.689 |
| AEMP1.JT | 191 | 385.809 | +194.809 |
| AGM45.JT | 795 | 390.218 | -404.782 |
| AGM65A.JT | 675 | 460.766 | -214.234 |
| AGM65G.JT | 675 | 670.000 | -5.000 |
| AGM84A.JT | 1165 | 1190.496 | +25.496 |
| AGM84E.JT | 1380 | 1382.298 | +2.298 |
| AGM88.JT | 795 | 800.000 | +5.000 |
| AIM120.JT | 345 | 348.000 | +3.000 |
| AIM54C.JT | 975 | 1020.740 | +45.740 |
| AIM7.JT | 505 | 509.268 | +4.268 |
| AIM7E.JT | 440 | 434.311 | -5.689 |
| AIM9B.JT | 170 | 154.324 | -15.676 |
| AIM9M.JT | 190 | 190.001 | +0.001 |
| AIM9X.JT | 190 | 187.999 | -2.001 |
| AM39.JT | 1476 | 1477.097 | +1.097 |
| AS14.JT | 1495 | 1455.051 | -39.949 |
| AS15.JT | 220 | 220.000 | +0.000 |
| AS16.JT | 2640 | 2645.547 | +5.547 |
| AS30.JT | 1146 | 1146.404 | +0.404 |
| AS7.JT | 631 | 637.136 | +6.136 |
| AT12.JT | 132 | 132.000 | +0.000 |
| AT2.JT | 65 | 69.446 | +4.446 |
| FIM92.JT | 35 | 22.928 | -12.072 |
| MICA.JT | 230 | 246.918 | +16.918 |
| MIS.JT | 1 | 41.226 | +40.226 |
| PL10.JT | 661 | 385.809 | -275.191 |
| PL7.JT | 198 | 196.211 | -1.789 |
| R27ER.JT | 772 | 771.618 | -0.382 |
| R27ET.JT | 756 | 756.186 | +0.186 |
| R27T.JT | 541 | 540.133 | -0.867 |
| R440.JT | 1 | 185.188 | +184.188 |
| R530.JT | 595 | 595.248 | +0.248 |
| R550.JT | 196 | 196.211 | +0.211 |
| SA13.JT | 1 | 88.185 | +87.185 |
| SA14.JT | 1 | 22.708 | +21.708 |
| SA15.JT | 1 | 363.763 | +362.763 |
| SA16.JT | 1 | 23.810 | +22.810 |
| SA19.JT | 1 | 92.594 | +91.594 |
| SA2A.JT | 1 | 4768.599 | +4767.599 |
| SA3.JT | 1 | 2101.005 | +2100.005 |
| SA6.JT | 1 | 1320.569 | +1319.569 |
| SA7.JT | 1 | 20.172 | +19.172 |
| SA9.JT | 1 | 66.139 | +65.139 |
| SAN11.JT | 1 | 92.594 | +91.594 |
| SAN3.JT | 1 | 4065.324 | +4064.324 |
| SAN4.JT | 1 | 282.192 | +281.192 |
| SAN7.JT | 1 | 1521.190 | +1520.190 |
| SAN9.JT | 1 | 363.763 | +362.763 |
| SEA_SPAR.JT | 1 | 509.268 | +508.268 |
| SSN9.JT | 500 | 7010.700 | +6510.700 |
