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
The physical reference table is `tore-formats::missile_data::REFERENCES`;
the per-store motor and conditional anchors are in `missile_motors::MOTORS`.
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
| OPEN-R440 | 84 kg, 150 mm, 10 km. Reported motor 20,550 N for 1.75 s then 17,550 N for 2.3 s. Charges 16.1/19 kg are used by the per-store motor; constant flow within each phase is fitted. | [Source](https://en.missilery.info/missile/crotal) |
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

## Per-store motors and paired anchors

Implementation mode, review corrections of 2026-10-26. John requested explicit
guidance life, plausible drag and useful tail chase. The per-store table records
59 profiles. These are game-model fits, not engineering performance claims.
Thrust is fuel mass times effective exhaust divided by phase duration unless
an explicit thrust override is supplied. Burn phase patterns remain fitted
where no phase-resolved measurements exist; their fuel and derived thrust are
refitted. Every profile has controlled-flight seconds from release, independent
of fuel burn and the later removal lifetime.

Sourced fields:

- AIM9B guidance is 20 seconds, from [NAVWEPS OP 2309, third revision,
  13 August 1966, section 2-1.1](https://www.scribd.com/document/1041639431/OP-2309-3rd-AIM-9B).
  Section 2-1.6 gives 8,440 lbf seconds over 2.2 seconds. The model uses their
  mean thrust, 17,064.995 N, replacing the older 17,800 N estimate. Constant
  thrust and 2,200 m/s effective exhaust are fitted; fuel follows that impulse.
  The manual's self-destruction rule is not adopted: John's rule leaves an
  unguided body after control expires.
- AIM9M boost is 12,810.878 N for five seconds from [Navy P-825,
  printed p.14-5](https://falcon.blu3wolf.com/Docs/P-825.pdf#page=284).
  X shares M's motor according to the [USAF fact sheet](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104557/aim-9-sidewinder/).
  Effective exhaust and derived fuel remain fitted.
- R440's two thrusts/durations and charges use OPEN-R440 above. AGM84A/E
  sustain thrust uses OPEN-HARPOON; SSN9 uses OPEN-P120. Their remaining
  phase fields are fitted. AS7 fuel fraction uses the cited 289/225 kg
  loaded/burnout masses; phase allocation is fitted.
- AIM7 phase durations use Navy P-825; its other refitted fields are described
  below. All other motor fields are labelled FITTED-PER-STORE. Initial classes are
  short IR, radar boost/sustain, surface and powered strike. Fictional records
  use the same method without changing their game identities.

No exact-variant primary guidance-duration figure was found for the other 58
profiles. Their times are fitted per missile to its head-on reference, using the
selected motor. For surface targets, this is the documented stationary or
receding reference instead of a head-on aircraft. These are labelled
FITTED-PER-MISSILE-REFERENCE-TIME, not attributed to John or a manufacturer.
For comparison, [NAVWEPS OP 3352, section 1-2](https://www.bulletpicker.com/pdf/OP-3352.pdf)
gives 60 seconds for AIM9D. That is a different variant, so it is not silently
assigned to M or X. Finding exact-variant control-power duration remains the
next evidence step for those fitted fields.

The per-store deterministic fit searched unknown fuel fractions 0.12..0.50 in 0.02
steps, with sourced thrusts and AS7 fuel held fixed. Drag candidates are 0.5,
0.75, 1, 1.5, 2, 2.5 and 3. Controlled lifetime is the primary intercept time
rounded up to a simulation tick, plus one tick; AIM9B retains its sourced
20 seconds. The score is four times secondary relative range error, plus
0.15*ln(drag)^2 and 0.03*((fuel_fraction-0.35)/0.15)^2. Tail reach below
clamp(0.08*primary_km,2,6) km at 5,000 ft adds ten times its fractional
shortfall. This makes tail reach and aerodynamic plausibility material fit
constraints. The drag band is a hard constraint except the two identified
cases below, which also pay 1+ln(0.5/drag)^2 outside it. Runtime never fits.

M/X high-head targets are restored to 18 km and R73/class to 30 km, instead
of the previous pass's 35 km. Their secondary tail targets are fitted 6/7 km
at 5,000 ft. Other head-on targets have not been raised. B/R3S retain fitted
20 km high-head and 1.65 km low-tail comparison targets. Rear-only seekers
still reject head-on shots. Other A2A low-head targets retain 45% of their
high-head value. Aircraft speeds are launcher 300/target 250 m/s. SAM primary
and secondary targets are stationary; the second height is halved, at least
300 ft, and the target range is 102% of primary. Strike secondary altitude is
halved and range 98%; AT2 uses 200 ft. SSN9 uses a 15 m/s receding secondary.
All of these conditions are agent choices, not manufacturer test conditions.

The earlier claim that every paired condition is within 15% is withdrawn.
Guidance expiry and the constrained drag fit preserve the primary head-on
anchors closely, but several old low-head/low-tail comparison targets cannot
be matched simultaneously in this simple model. The baseline exposes every
residual. Those comparison targets have not been raised to conceal the
mismatch. The verified requirements are useful tail reach, altitude dependence,
generation orderings where physically justified, finite long-shot guidance
and local fit smoothness. John removed the supplemental geometry multiplier
after round 3. The round-2 drag fits are restored, with the newer-Sparrow
refit retained. Current measurements are in the baseline.

| Store | Fuel fraction | Boost N / s | Sustain N / s | Controlled s | Motor evidence | Guidance evidence |
| --- | ---: | --- | --- | ---: | --- | --- |
| AA10.JT | 0.38000 | 34370.0 / 4 | 12338.0 / 6 | 107.733 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AA11.JT | 0.28000 | 12936.0 / 5 | 0.0 / 0 | 31.058 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AA11B.JT | 0.28000 | 12936.0 / 5 | 0.0 / 0 | 31.058 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AA12.JT | 0.50000 | 31281.3 / 4 | 11229.2 / 6 | 43.083 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AA2.JT | 0.12000 | 9360.0 / 2.2 | 0.0 / 0 | 29.092 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AA6.JT | 0.50000 | 149285.7 / 3.5 | 0.0 / 0 | 27.017 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AA8.JT | 0.24000 | 4646.4 / 5 | 0.0 / 0 | 11.375 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AA9.JT | 0.22000 | 21739.7 / 6 | 5336.1 / 20 | 204.483 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AAML.JT | 0.42000 | 42350.0 / 6 | 10395.0 / 20 | 200.917 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AEMP1.JT | 0.50000 | 31281.3 / 4 | 11229.2 / 6 | 43.108 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AGM45.JT | 0.38000 | 40692.3 / 2 | 11097.9 / 6 | 77.483 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AGM65A.JT | 0.30000 | 37933.5 / 2 | 10345.5 / 6 | 86.950 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AGM65G.JT | 0.32000 | 58836.4 / 2 | 16046.3 / 6 | 80.558 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AGM84A.JT | 0.21378 | 0.0 / 0 | 3000.0 / 962 | 584.883 | EXISTING-MOTOR-FIELD-EVIDENCE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AGM84E.JT | 0.09256 | 0.0 / 0 | 3000.0 / 483.6 | 296.408 | EXISTING-MOTOR-FIELD-EVIDENCE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AGM88.JT | 0.38000 | 83424.7 / 2 | 22752.2 / 6 | 86.275 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AIM120.JT | 0.50000 | 28215.7 / 4 | 10128.7 / 6 | 52.308 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AIM54C.JT | 0.24000 | 22409.2 / 6 | 5500.4 / 20 | 244.150 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AIM7.JT | 0.40000 | 37752.0 / 3.5 | 5691.8 / 12.5 | 51.550 | CNATRA-P825-SPARROW-PHASES;FITTED-FUEL | FITTED-PER-MISSILE-REFERENCE-TIME |
| AIM7E.JT | 0.50000 | 35213.8 / 4 | 12640.8 / 6 | 32.408 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AIM9B.JT | 0.24379 | 17065.0 / 2.2 | 0.0 / 0 | 20.000 | NAVWEPS-OP2309-MK15;FITTED-EXHAUST | NAVWEPS-OP2309-20S |
| AIM9M.JT | 0.33783 | 12810.9 / 5 | 0.0 / 0 | 16.492 | CNATRA-P825-MK36;FITTED-EXHAUST | FITTED-PER-MISSILE-REFERENCE-TIME |
| AIM9X.JT | 0.34143 | 12810.9 / 5 | 0.0 / 0 | 16.417 | CNATRA-P825-MK36;FITTED-EXHAUST | FITTED-PER-MISSILE-REFERENCE-TIME |
| AM39.JT | 0.34000 | 137819.0 / 2 | 37587.0 / 6 | 179.183 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AS14.JT | 0.30000 | 119790.0 / 2 | 32670.0 / 6 | 14.183 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AS15.JT | 0.34000 | 20526.9 / 2 | 5598.2 / 6 | 23.850 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AS16.JT | 0.36000 | 261360.0 / 2 | 71280.0 / 6 | 337.792 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AS30.JT | 0.34000 | 106964.0 / 2 | 29172.0 / 6 | 14.383 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AS7.JT | 0.22145 | 38720.0 / 2 | 10560.0 / 6 | 20.650 | OPEN-KH23-BURNOUT;FITTED-PHASES | FITTED-PER-MISSILE-REFERENCE-TIME |
| AT12.JT | 0.36000 | 13040.6 / 2 | 3556.5 / 6 | 11.667 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| AT2.JT | 0.30000 | 5717.2 / 2 | 1559.2 / 6 | 10.592 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| FIM92.JT | 0.34000 | 1555.8 / 3 | 389.0 / 8 | 9.583 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| MICA.JT | 0.50000 | 20020.0 / 4 | 7186.7 / 6 | 75.725 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| MIS.JT | 0.30000 | 2468.4 / 3 | 617.1 / 8 | 16.267 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| PL10.JT | 0.50000 | 31281.3 / 4 | 11229.2 / 6 | 49.667 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| PL7.JT | 0.50000 | 19580.0 / 5 | 0.0 / 0 | 10.583 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| R27ER.JT | 0.24000 | 24024.0 / 5 | 6468.0 / 10 | 191.217 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| R27ET.JT | 0.36000 | 35315.3 / 5 | 9508.0 / 10 | 145.550 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| R27T.JT | 0.50000 | 43793.7 / 4 | 15720.8 / 6 | 82.158 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| R440.JT | 0.41786 | 20550.0 / 1.75 | 17550.0 / 2.3 | 14.050 | EXISTING-MOTOR-FIELD-EVIDENCE | FITTED-PER-MISSILE-REFERENCE-TIME |
| R530.JT | 0.50000 | 48262.5 / 4 | 17325.0 / 6 | 36.975 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| R550.JT | 0.50000 | 19580.0 / 5 | 0.0 / 0 | 11.517 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SA13.JT | 0.38000 | 6688.0 / 3 | 1672.0 / 8 | 9.992 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SA14.JT | 0.34000 | 1540.9 / 3 | 385.2 / 8 | 10.625 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SA15.JT | 0.32000 | 23232.0 / 3 | 5808.0 / 8 | 29.475 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SA16.JT | 0.32000 | 1520.6 / 3 | 380.2 / 8 | 12.508 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SA19.JT | 0.30000 | 5544.0 / 3 | 1386.0 / 8 | 23.892 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SA2A.JT | 0.34000 | 323584.8 / 3 | 80896.2 / 8 | 83.150 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SA3.JT | 0.28000 | 117409.6 / 3 | 29352.4 / 8 | 69.533 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SA6.JT | 0.30000 | 79068.0 / 3 | 19767.0 / 8 | 68.783 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SA7.JT | 0.34000 | 1368.8 / 3 | 342.2 / 8 | 10.275 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SA9.JT | 0.34000 | 4488.0 / 3 | 1122.0 / 8 | 9.983 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SAN11.JT | 0.30000 | 5544.0 / 3 | 1386.0 / 8 | 23.892 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SAN3.JT | 0.38000 | 308316.8 / 3 | 77079.2 / 8 | 88.517 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SAN4.JT | 0.30000 | 16896.0 / 3 | 4224.0 / 8 | 25.175 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SAN7.JT | 0.34000 | 103224.0 / 3 | 25806.0 / 8 | 72.933 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SAN9.JT | 0.32000 | 23232.0 / 3 | 5808.0 / 8 | 29.475 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SEA_SPAR.JT | 0.28000 | 28459.2 / 3 | 7114.8 / 8 | 69.242 | FITTED-PER-STORE | FITTED-PER-MISSILE-REFERENCE-TIME |
| SSN9.JT | 0.52108 | 303160.0 / 3 | 7600.0 / 360 | 528.533 | EXISTING-MOTOR-FIELD-EVIDENCE | FITTED-PER-MISSILE-REFERENCE-TIME |

### Newer Sparrow refit

AIM7 uses the existing explicit F/M proxy. Navy P-825's printed 14-5 motor
table gives 3.5 s boost and 12.5 s sustain. The agent refit uses fuel fraction
0.40, drag scale 0.5, 51.55 s controlled life fitted to its 70 km high-head
reference, and a fitted 0.5 s post-release ignition delay instead of the imported
2 s. That delay is not inferred from the manual's trigger-to-launch time.
The shorter drop and lower base drag were selected during round 3. John
explicitly retained this refit when removing the geometry multiplier. The offline search checks
fuel fractions 0.35/0.40/0.45/0.50, base drag 0.5/0.75/1/1.5/2/2.5/3 and
ignition delays 0/0.25/0.5/1 s. Every cell must have a positive newer reach
and at least 5% advantage wherever the older missile can intercept. The
original round-3 search checked both drag variants; the retained offline tool
now uses only the current force law. Zero/zero is never an upgrade win.
Among feasible candidates the agent score is `(fuel-0.35)^2 + 0.02*ln(drag)^2
+ 0.01*(1-ignition_seconds)^2`. The retained candidate was selected by that round-3 search;
these penalties are agent choices, not manufacturer's constraints. This missile's
round-3 motor/drag/control parameters are unchanged by the geometry rollback. Other guidance times retain their previous
per-missile fit, except the sourced AIM9B value. Ground/ship controlled times
are fitted to their stationary or receding primary reference, not a head-on
A2A reference. The [baseline](../baselines/missile-energy.md) records all family
ordering checks and the limits of cross-family comparisons.

### Drag exceptions

57 of 59 profiles use 0.5..3. SSN9 uses 0.4: the single equivalent body with
public 360-second sustain cannot reach its unchanged 150 km anchor with 0.5.
This combines staged ship launch and cruise into one body, so its multiplier
is an effective approximation, not a measured coefficient. AS16 uses the value
below: the game's low-altitude powered-strike path cannot reproduce the
150 km conventional Kh15 proxy with 0.5. The model does not reproduce its real
high-altitude ballistic path. That trajectory difference is the individual
reason for this exception, not evidence of the real missile's drag. Neither
exception rescales the base Cd curve or increases a head-on anchor.

| Store | Drag multiplier |
| --- | ---: |
| AS16.JT | 0.25 |
| SSN9.JT | 0.4 |

## Span reference data

**Reference only.** John removed the geometry-based drag factor after round 3.
Largest deployed spans and span/diameter ratios below do not enter the force
kernel or model fingerprint. Body diameter retains its existing use in frontal
reference area; the physical diameter and mass sources elsewhere are unchanged.
No additional altitude multiplier or fade is applied.

Span means the largest fully deployed wing or fin tip-to-tip dimension. Missing
measurements in the reviewed sources remain unknown. The table retains explicit
agent estimates: ratio 3.0 for strike/cruise, 3.5 for remaining IR or surface
profiles, 3.0 otherwise, plus labelled fictional-family proxies. These are
reference estimates, not recovered or published dimensions and not simulation
inputs. The next evidence step is a variant-specific manufacturer drawing.

| Span evidence ID | Field and source |
| --- | --- |
| USAF-AMRAAM-SPAN | 20.7 in, 0.52578 m, [USAF AIM-120 fact sheet](https://www.af.mil/News/Fact-Sheets/Display/Article/104576/aim-120-amraam/). |
| NAVAIR-SIDEWINDER-SPAN | AIM-9M 24.75 in, 0.62865 m, [NAVAIR](https://www.navair.navy.mil/product/AIM-9M-Sidewinder). |
| USNAVY-SIDEWINDER-X-SPAN | AIM-9X largest fin span 17.6 in, 0.44704 m, [Navy fact file](https://www.navy.mil/Resources/Fact-Files/Display-FactFiles/Article/2168989/aim-9x-sidewinder-missile/aim-9x-sidewinder-missile/). Smaller forward wing span is not substituted. |
| NAVWEPS-OP2309-SPAN | AIM-9B 22 in including rollerons, 0.5588 m, OP 2309 section 1-2, [manual reproduction](https://www.scribd.com/document/1041639431/OP-2309-3rd-AIM-9B). |
| NAVAIR-PHOENIX-C | AIM-54C 0.92 m, [NAVAIR](https://www.navair.navy.mil/node/12701). |
| CNATRA-P825-SPARROW-SPAN | AIM-7 family 40 in, 1.016 m, [Navy P-825](https://falcon.blu3wolf.com/Docs/P-825.pdf#page=284). |
| USNAVY-AIM-RIM7-SPAN | AIM/RIM-7 40 in, 1.016 m, [Navy environmental statement](https://www.nepa.navy.mil/Portals/20/Documents/aftteis1/virginia-capes/nepa-eo/vacapes-feis-vol2-appendices-full.pdf). |
| ROE2005-R27 / ROE2005-R73 / ROE2005-RVVAE / ROE2005-R33 / ROE2005-KH29L | Manufacturer catalogue printed pp. 118-120, 122: R27 variants largest stabilizer 0.97 m (wing 0.77 m is smaller), R73 0.51 m, RVV-AE 0.70 m, R33 1.10 m, Kh29L empennage 1.10 m, [Rosoboronexport](https://server.3rd-wing.net/public/tiengo/Doc/r%C3%A9aliste%20avionics%20russes.pdf). |
| US-WEG-R60-SPAN | R60 0.39 m, [Army ODIN equipment guide](https://odin.t2com.army.mil/WEG/Asset/7d3e19d07bd4ac462c9404e8eef55457). |
| USAF-MAVERICK-SPAN | AGM-65 28 in, 0.7112 m, [USAF](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104577/agm-65-maverick/). |
| USAF-HARM-SPAN | AGM-88 44 in, 1.1176 m, [USAF](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104574/agm-88-harm/). Its parenthetical 101.60 cm is inconsistent; this field uses the explicit imperial measurement. |
| OPEN-SUPER530D | Super 530D 0.62 m, [Conservatoire de l'air](https://c-aea.fr/fr/50-collection/armement). Do not substitute older R530 or Super530F dimensions. |
| NAVAIR-HARPOON-SPAN | Harpoon 3 ft, 0.9144 m, [NAVAIR](https://www.navair.navy.mil/product/Harpoon). |
| CLASS-HARPOON-SPAN / CLASS-R73-SPAN / CLASS-PHOENIX-SPAN / CLASS-RVVAE-SPAN | Explicit family proxies for AGM84E / AA11B / AAML / PL10 and AEMP1 respectively. |
| CLASS-SPAN-RATIO | Span unknown in reviewed sources; agent class estimate above. MICA's reviewed manufacturer sheet supplies diameter but not span. |

| Store | Span m | Diameter m | Ratio | Span evidence | Diameter evidence |
| --- | ---: | ---: | ---: | --- | --- |
| AA10.JT | 0.9700 | 0.2300 | 4.217 | ROE2005-R27 | ROE2005-R27 |
| AA11.JT | 0.5100 | 0.1700 | 3.000 | ROE2005-R73 | ROE2005-R73 |
| AA11B.JT | 0.5100 | 0.1700 | 3.000 | CLASS-R73-SPAN | FITTED-AGILE-IR-CLASS |
| AA12.JT | 0.7000 | 0.2000 | 3.500 | ROE2005-RVVAE | ROE2005-RVVAE |
| AA2.JT | 0.4445 | 0.1270 | 3.500 | CLASS-SPAN-RATIO | RWD-R3S |
| AA6.JT | 1.0650 | 0.3550 | 3.000 | CLASS-SPAN-RATIO | OPEN-R40-FAMILY |
| AA8.JT | 0.3900 | 0.1200 | 3.250 | US-WEG-R60-SPAN | OPEN-R60M |
| AA9.JT | 1.1000 | 0.3800 | 2.895 | ROE2005-R33 | ROE2005-R33 |
| AAML.JT | 0.9200 | 0.3800 | 2.421 | CLASS-PHOENIX-SPAN | FITTED-HEAVY-RADAR-CLASS |
| AEMP1.JT | 0.7000 | 0.2000 | 3.500 | CLASS-RVVAE-SPAN | FITTED-RADAR-CLASS |
| AGM45.JT | 0.6096 | 0.2032 | 3.000 | CLASS-SPAN-RATIO | OPEN-SHRIKE-B-PROXY |
| AGM65A.JT | 0.7112 | 0.3048 | 2.333 | USAF-MAVERICK-SPAN | OPEN-MAVERICK-A |
| AGM65G.JT | 0.7112 | 0.3048 | 2.333 | USAF-MAVERICK-SPAN | USAF-MAVERICK-G |
| AGM84A.JT | 0.9144 | 0.3430 | 2.666 | NAVAIR-HARPOON-SPAN | OPEN-HARPOON |
| AGM84E.JT | 0.9144 | 0.3430 | 2.666 | CLASS-HARPOON-SPAN | OPEN-SLAM |
| AGM88.JT | 1.1176 | 0.2540 | 4.400 | USAF-HARM-SPAN | USAF-HARM |
| AIM120.JT | 0.5258 | 0.1778 | 2.957 | USAF-AMRAAM-SPAN | NAVAIR-AMRAAM |
| AIM54C.JT | 0.9200 | 0.3800 | 2.421 | NAVAIR-PHOENIX-C | NAVAIR-PHOENIX-C |
| AIM7.JT | 1.0160 | 0.2032 | 5.000 | CNATRA-P825-SPARROW-SPAN | OPEN-SPARROW-FM-PROXY |
| AIM7E.JT | 1.0160 | 0.2032 | 5.000 | CNATRA-P825-SPARROW-SPAN | OPEN-SPARROW-E |
| AIM9B.JT | 0.5588 | 0.1270 | 4.400 | NAVWEPS-OP2309-SPAN | OPEN-SIDEWINDER-B |
| AIM9M.JT | 0.6287 | 0.1270 | 4.950 | NAVAIR-SIDEWINDER-SPAN | USAF-SIDEWINDER |
| AIM9X.JT | 0.4470 | 0.1270 | 3.520 | USNAVY-SIDEWINDER-X-SPAN | NAVAIR-SIDEWINDER |
| AM39.JT | 1.0500 | 0.3500 | 3.000 | CLASS-SPAN-RATIO | MBDA-AM39 |
| AS14.JT | 1.1000 | 0.3800 | 2.895 | ROE2005-KH29L | ROE2005-KH29L |
| AS15.JT | 0.4004 | 0.1335 | 3.000 | CLASS-SPAN-RATIO | FITTED-CLASS |
| AS16.JT | 1.3650 | 0.4550 | 3.000 | CLASS-SPAN-RATIO | OPEN-KH15S-PROXY |
| AS30.JT | 1.0260 | 0.3420 | 3.000 | CLASS-SPAN-RATIO | OPEN-AS30L |
| AS7.JT | 0.8250 | 0.2750 | 3.000 | CLASS-SPAN-RATIO | OPEN-KH23 |
| AT12.JT | 0.3377 | 0.1126 | 3.000 | CLASS-SPAN-RATIO | FITTED-CLASS |
| AT2.JT | 0.4260 | 0.1420 | 3.000 | CLASS-SPAN-RATIO | OPEN-FALANGA-M-PROXY |
| FIM92.JT | 0.2450 | 0.0700 | 3.500 | CLASS-SPAN-RATIO | US-ORDNANCE-STINGER |
| MICA.JT | 0.4800 | 0.1600 | 3.000 | CLASS-SPAN-RATIO | MBDA-MICA |
| MIS.JT | 0.3150 | 0.0900 | 3.500 | CLASS-SPAN-RATIO | MBDA-MISTRAL-PHYSICAL-PROXY |
| PL10.JT | 0.7000 | 0.2000 | 3.500 | CLASS-RVVAE-SPAN | FITTED-RADAR-CLASS |
| PL7.JT | 0.5495 | 0.1570 | 3.500 | CLASS-SPAN-RATIO | OPEN-PL7 |
| R27ER.JT | 0.9700 | 0.2600 | 3.731 | ROE2005-R27 | ROE2005-R27 |
| R27ET.JT | 0.9700 | 0.2600 | 3.731 | ROE2005-R27 | ROE2005-R27 |
| R27T.JT | 0.9700 | 0.2300 | 4.217 | ROE2005-R27 | ROE2005-R27 |
| R440.JT | 0.5250 | 0.1500 | 3.500 | CLASS-SPAN-RATIO | OPEN-R440 |
| R530.JT | 0.6200 | 0.2630 | 2.357 | OPEN-SUPER530D | OPEN-SUPER530D |
| R550.JT | 0.5600 | 0.1600 | 3.500 | CLASS-SPAN-RATIO | OPEN-MAGIC2 |
| SA13.JT | 0.4200 | 0.1200 | 3.500 | CLASS-SPAN-RATIO | OPEN-9M37 |
| SA14.JT | 0.2520 | 0.0720 | 3.500 | CLASS-SPAN-RATIO | GICHD-SA14 |
| SA15.JT | 0.8050 | 0.2300 | 3.500 | CLASS-SPAN-RATIO | GICHD-SA15 |
| SA16.JT | 0.2520 | 0.0720 | 3.500 | CLASS-SPAN-RATIO | GICHD-SA16 |
| SA19.JT | 0.5320 | 0.1520 | 3.500 | CLASS-SPAN-RATIO | OPEN-9M311 |
| SA2A.JT | 2.2890 | 0.6540 | 3.500 | CLASS-SPAN-RATIO | OPEN-V750V-PROXY |
| SA3.JT | 1.3125 | 0.3750 | 3.500 | CLASS-SPAN-RATIO | GICHD-V601-PROXY |
| SA6.JT | 1.1725 | 0.3350 | 3.500 | CLASS-SPAN-RATIO | GULFLINK-SA6 |
| SA7.JT | 0.2520 | 0.0720 | 3.500 | CLASS-SPAN-RATIO | GICHD-SA7B-PROXY |
| SA9.JT | 0.4200 | 0.1200 | 3.500 | CLASS-SPAN-RATIO | GULFLINK-SA9 |
| SAN11.JT | 0.5320 | 0.1520 | 3.500 | CLASS-SPAN-RATIO | OPEN-9M311 |
| SAN3.JT | 2.2750 | 0.6500 | 3.500 | CLASS-SPAN-RATIO | OPEN-V611 |
| SAN4.JT | 0.7210 | 0.2060 | 3.500 | CLASS-SPAN-RATIO | OPEN-OSA-M1-PROXY |
| SAN7.JT | 1.4000 | 0.4000 | 3.500 | CLASS-SPAN-RATIO | OPEN-9M38-NAVAL |
| SAN9.JT | 0.8050 | 0.2300 | 3.500 | CLASS-SPAN-RATIO | GICHD-SA15 |
| SEA_SPAR.JT | 1.0160 | 0.2032 | 5.000 | USNAVY-AIM-RIM7-SPAN | OPEN-SPARROW-FM-PROXY |
| SSN9.JT | 2.4000 | 0.8000 | 3.000 | CLASS-SPAN-RATIO | OPEN-P120 |



## Other fitted fields

The [behaviour specification](../spec/missile-energy.md) owns atmosphere,
force curves, control-speed, loft, seeker, decoy and fuze rules. Diameter
fallback is 0.127*(mass/86)^(1/3) m, limited to 0.07..0.6 m. Public motor data do
not establish seeker identity, gimbal, activation, arming, fuze or damage.
Those retain their reviewed game contracts. No susceptibility is clipped
before evaluating the combined Energy decoy probability.

## Loaded aircraft mass changes

`Weapon::carried_weight_lb` applies public/class mass to carried stores and
loadout totals, for AI and players alike. Decoded `weight` remains intact for old combat rules, effects, the fitted radar-signature weight proxy
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
