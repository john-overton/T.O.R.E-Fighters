# Missile energy data evidence

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-27. The energy model uses public
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

## Per-store motors and class control budgets

Implementation mode, Phase 2.5 of 2026-10-27. John requested class control
budgets, post-burn inert defeat, one-anchor fits and class tail floors. The per-store table records
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

AIM9B retains its cited 20-second control time. Every other store's time is
an opinionated game-class budget approved by John on 2026-10-27, labelled
OPINIONATED-CLASS-JOHN-2026-10-27. Classes are named explicitly in each
data record. No runtime range or real-world designation selects a class.
The agent assigned R27T/ET to medium/long air, matching their radar-family
budgets. The surface long class is SA2A, SA3, SEA_SPAR, SAN3 and SAN7;
SA6 uses surface radar 70 seconds. AT12 uses the existing game's cruise
path/drag class despite its unresolved real-world identity. AS16 uses the
existing fast ground path, not an invented ballistic or cruise class.
Only AT2 uses the command control class. Guidance/support types remain
independent of these control budgets.

The single post-burn air-target defeat knob is Mach 1.0 with a 30-tick
(0.25-second) dwell. Tuning alternatives are 0.9 and 1.1. It does not apply
to ground-target classes. Exact contact ordering and inert behaviour are in
[the specification](../spec/missile-energy.md#motion).

A2A calibration uses only each existing 35,000-foot head-on anchor,
launcher 300 and target 250 m/s. Fuel is bounded to 0.12..0.50 and drag
to 0.5..3. Sourced thrust/impulse pairs retain their existing in-bounds fuel;
other fuel/derived thrust values are fitted. Control time, ignition, motor
duration, loft and dimensions are not fitted. Candidate drag increments are
0.25, with 1.05/1.10/1.15/1.20 refinement for the R73 family to
retain its existing 12 km deck-tail escape regression. Fourteen bisections
choose fuel to meet the anchor and tail floor where
feasible, otherwise the bounded endpoint is measured. The score is
`4*abs(H35/anchor-1) + 0.02*ln(drag)^2 + 0.02*((fuel-0.35)/0.38)^2`.
These weights and grid resolution are agent choices.

`missile_energy_ranges --fit-class` exports all candidates and twelve
altitude/aspect cells. `python3 tools/missile_energy_fit.py CANDIDATES OUTPUT`
selects the lowest total family score subject to strict Sparrow and R-27
ordering in all twelve cells, and modern IR tail ordering at 5k/15k/35k.
Zero physical-reach cells cannot satisfy a family constraint. The fictional AA11B
shares AA11's fit. R73 uses drag 1.10 and fuel 0.50 to retain the
12 km deck escape, leaving a 0.84% high-head shortfall rather than altering
the fixed 20-second budget. Runtime never fits. `--fit-pair` and `--fit-sparrow`
reject use; historical timer fitting is available only with the explicit
`--legacy-fit-pair` and `--legacy-fit-sparrow` names and warning.

Class tail floors at 5,000 feet are 1.5/3/5 km for short/medium/long air.
Every A2A tail also exceeds 1.2 times the largest decoded minimum launch
range. The fitter adds a 10 m margin to that decoded minimum. Warning bands
are H5/H35 0.20..0.90 and T5/H35 0.05..0.60; altitude reversals exceeding
10% require investigation. Warnings are not additional fitted anchors.
AIM9M/X and AA8 overshoots remain visible. The AA8 tail floor moves its
best bounded result from the study's roughly 13 km to roughly 14 km.
The [baseline](../baselines/missile-energy.md) reports all residuals and margins.

The old secondary references remain data for historical comparison only.
They were agent choices, including many low-head values at 45% of high head,
not independent measurements. No A2A secondary reference fits a motor or timer.
Ground/SAM fuel, drag and motor data remain unchanged; their new class times
and air-target defeat policy can change reach. Historical ground comparisons
use stationary SAM targets, 1,500 m airborne strike release, 100 m for AT2,
and 10 m target/ship heights.

| Store | Control class | Fuel fraction | Boost N / s | Sustain N / s | Controlled s | Motor evidence | Guidance evidence |
| --- | --- | ---: | --- | --- | ---: | --- | --- |
| AA10.JT | MediumAir | 0.401382 | 36304.0 / 4 | 13032.2 / 6 | 70 | FITTED-PER-STORE | opinionated class |
| AA11.JT | ShortIr | 0.500000 | 23100.0 / 5 | 0.0 / 0 | 20 | FITTED-PER-STORE | opinionated class |
| AA11B.JT | ShortIr | 0.500000 | 23100.0 / 5 | 0.0 / 0 | 20 | FITTED-PER-STORE | opinionated class |
| AA12.JT | MediumAir | 0.151102 | 9453.3 / 4 | 3393.5 / 6 | 70 | FITTED-PER-STORE | opinionated class |
| AA2.JT | ShortIr | 0.263265 | 20534.7 / 2.2 | 0.0 / 0 | 20 | FITTED-PER-STORE | opinionated class |
| AA6.JT | MediumAir | 0.352050 | 105111.9 / 3.5 | 0.0 / 0 | 70 | FITTED-PER-STORE | opinionated class |
| AA8.JT | ShortIr | 0.120000 | 2323.2 / 5 | 0.0 / 0 | 20 | FITTED-PER-STORE | opinionated class |
| AA9.JT | LongAir | 0.379835 | 37534.0 / 6 | 9212.9 / 20 | 180 | FITTED-PER-STORE | opinionated class |
| AAML.JT | LongAir | 0.358567 | 36155.5 / 6 | 8874.5 / 20 | 180 | FITTED-PER-STORE | opinionated class |
| AEMP1.JT | MediumAir | 0.151195 | 9459.1 / 4 | 3395.6 / 6 | 70 | FITTED-PER-STORE | opinionated class |
| AGM45.JT | FastGround | 0.380000 | 40692.3 / 2 | 11097.9 / 6 | 90 | FITTED-PER-STORE | opinionated class |
| AGM65A.JT | FastGround | 0.300000 | 37933.5 / 2 | 10345.5 / 6 | 90 | FITTED-PER-STORE | opinionated class |
| AGM65G.JT | FastGround | 0.320000 | 58836.4 / 2 | 16046.3 / 6 | 90 | FITTED-PER-STORE | opinionated class |
| AGM84A.JT | Cruise | 0.213778 | 0.0 / 0 | 3000.0 / 962 | 600 | EXISTING-MOTOR-FIELD-EVIDENCE | opinionated class |
| AGM84E.JT | Cruise | 0.092555 | 0.0 / 0 | 3000.0 / 483.6 | 600 | EXISTING-MOTOR-FIELD-EVIDENCE | opinionated class |
| AGM88.JT | FastGround | 0.380000 | 83424.7 / 2 | 22752.2 / 6 | 90 | FITTED-PER-STORE | opinionated class |
| AIM120.JT | MediumAir | 0.220219 | 12427.2 / 4 | 4461.1 / 6 | 70 | FITTED-PER-STORE | opinionated class |
| AIM54C.JT | LongAir | 0.404907 | 37806.9 / 6 | 9279.9 / 20 | 180 | FITTED-PER-STORE | opinionated class |
| AIM7.JT | MediumAir | 0.292327 | 27589.8 / 3.5 | 4159.7 / 12.5 | 70 | CNATRA-P825-SPARROW-PHASES;FITTED-FUEL | opinionated class |
| AIM7E.JT | MediumAir | 0.156298 | 11007.6 / 4 | 3951.5 / 6 | 70 | FITTED-PER-STORE | opinionated class |
| AIM9B.JT | ShortIr | 0.243786 | 17065.0 / 2.2 | 0.0 / 0 | 20 | NAVWEPS-OP2309-MK15;FITTED-EXHAUST | NAVWEPS-OP2309-20S |
| AIM9M.JT | ShortIr | 0.337835 | 12810.9 / 5 | 0.0 / 0 | 20 | CNATRA-P825-MK36;FITTED-EXHAUST | opinionated class |
| AIM9X.JT | ShortIr | 0.341432 | 12810.9 / 5 | 0.0 / 0 | 20 | CNATRA-P825-MK36;FITTED-EXHAUST | opinionated class |
| AM39.JT | Cruise | 0.340000 | 137819.0 / 2 | 37587.0 / 6 | 600 | FITTED-PER-STORE | opinionated class |
| AS14.JT | FastGround | 0.300000 | 119790.0 / 2 | 32670.0 / 6 | 90 | FITTED-PER-STORE | opinionated class |
| AS15.JT | FastGround | 0.340000 | 20526.9 / 2 | 5598.2 / 6 | 90 | FITTED-PER-STORE | opinionated class |
| AS16.JT | FastGround | 0.360000 | 261360.0 / 2 | 71280.0 / 6 | 90 | FITTED-PER-STORE | opinionated class |
| AS30.JT | FastGround | 0.340000 | 106964.0 / 2 | 29172.0 / 6 | 90 | FITTED-PER-STORE | opinionated class |
| AS7.JT | FastGround | 0.221453 | 38720.0 / 2 | 10560.0 / 6 | 90 | OPEN-KH23-BURNOUT;FITTED-PHASES | opinionated class |
| AT12.JT | Cruise | 0.360000 | 13040.6 / 2 | 3556.5 / 6 | 600 | FITTED-PER-STORE | opinionated class |
| AT2.JT | Command | 0.300000 | 5717.2 / 2 | 1559.2 / 6 | 30 | FITTED-PER-STORE | opinionated class |
| FIM92.JT | SurfaceIr | 0.340000 | 1555.8 / 3 | 389.0 / 8 | 25 | FITTED-PER-STORE | opinionated class |
| MICA.JT | MediumAir | 0.229913 | 9205.7 / 4 | 3304.6 / 6 | 70 | FITTED-PER-STORE | opinionated class |
| MIS.JT | SurfaceRadar | 0.300000 | 2468.4 / 3 | 617.1 / 8 | 70 | FITTED-PER-STORE | opinionated class |
| PL10.JT | MediumAir | 0.157434 | 9849.5 / 4 | 3535.7 / 6 | 70 | FITTED-PER-STORE | opinionated class |
| PL7.JT | ShortIr | 0.122969 | 4815.5 / 5 | 0.0 / 0 | 20 | FITTED-PER-STORE | opinionated class |
| R27ER.JT | LongAir | 0.394911 | 39530.6 / 5 | 10642.8 / 10 | 180 | FITTED-PER-STORE | opinionated class |
| R27ET.JT | LongAir | 0.293510 | 28792.7 / 5 | 7751.9 / 10 | 180 | FITTED-PER-STORE | opinionated class |
| R27T.JT | MediumAir | 0.255217 | 22353.8 / 4 | 8024.5 / 6 | 70 | FITTED-PER-STORE | opinionated class |
| R440.JT | SurfaceRadar | 0.417857 | 20550.0 / 1.75 | 17550.0 / 2.3 | 70 | EXISTING-MOTOR-FIELD-EVIDENCE | opinionated class |
| R530.JT | MediumAir | 0.179955 | 17370.1 / 4 | 6235.4 / 6 | 70 | FITTED-PER-STORE | opinionated class |
| R550.JT | ShortIr | 0.122482 | 4796.4 / 5 | 0.0 / 0 | 20 | FITTED-PER-STORE | opinionated class |
| SA13.JT | SurfaceIr | 0.380000 | 6688.0 / 3 | 1672.0 / 8 | 25 | FITTED-PER-STORE | opinionated class |
| SA14.JT | SurfaceIr | 0.340000 | 1540.9 / 3 | 385.2 / 8 | 25 | FITTED-PER-STORE | opinionated class |
| SA15.JT | SurfaceRadar | 0.320000 | 23232.0 / 3 | 5808.0 / 8 | 70 | FITTED-PER-STORE | opinionated class |
| SA16.JT | SurfaceIr | 0.320000 | 1520.6 / 3 | 380.2 / 8 | 25 | FITTED-PER-STORE | opinionated class |
| SA19.JT | SurfaceRadar | 0.300000 | 5544.0 / 3 | 1386.0 / 8 | 70 | FITTED-PER-STORE | opinionated class |
| SA2A.JT | SurfaceRadarLong | 0.340000 | 323584.8 / 3 | 80896.2 / 8 | 120 | FITTED-PER-STORE | opinionated class |
| SA3.JT | SurfaceRadarLong | 0.280000 | 117409.6 / 3 | 29352.4 / 8 | 120 | FITTED-PER-STORE | opinionated class |
| SA6.JT | SurfaceRadar | 0.300000 | 79068.0 / 3 | 19767.0 / 8 | 70 | FITTED-PER-STORE | opinionated class |
| SA7.JT | SurfaceIr | 0.340000 | 1368.8 / 3 | 342.2 / 8 | 25 | FITTED-PER-STORE | opinionated class |
| SA9.JT | SurfaceIr | 0.340000 | 4488.0 / 3 | 1122.0 / 8 | 25 | FITTED-PER-STORE | opinionated class |
| SAN11.JT | SurfaceRadar | 0.300000 | 5544.0 / 3 | 1386.0 / 8 | 70 | FITTED-PER-STORE | opinionated class |
| SAN3.JT | SurfaceRadarLong | 0.380000 | 308316.8 / 3 | 77079.2 / 8 | 120 | FITTED-PER-STORE | opinionated class |
| SAN4.JT | SurfaceRadar | 0.300000 | 16896.0 / 3 | 4224.0 / 8 | 70 | FITTED-PER-STORE | opinionated class |
| SAN7.JT | SurfaceRadarLong | 0.340000 | 103224.0 / 3 | 25806.0 / 8 | 120 | FITTED-PER-STORE | opinionated class |
| SAN9.JT | SurfaceRadar | 0.320000 | 23232.0 / 3 | 5808.0 / 8 | 70 | FITTED-PER-STORE | opinionated class |
| SEA_SPAR.JT | SurfaceRadarLong | 0.280000 | 28459.2 / 3 | 7114.8 / 8 | 120 | FITTED-PER-STORE | opinionated class |
| SSN9.JT | Cruise | 0.521081 | 303160.0 / 3 | 7600.0 / 360 | 600 | EXISTING-MOTOR-FIELD-EVIDENCE | opinionated class |

### Newer Sparrow refit

AIM7 retains the explicit F/M proxy, 3.5-second boost, 12.5-second sustain
and fitted 0.5-second post-release ignition delay from round 3. The Phase 2.5
joint family fit replaces only fuel/drag and the old per-anchor control time.
Its new 70-second medium-air budget is shared with AIM7E. The data table above
and calibration baseline give the current coefficients and all twelve margins.
The shorter ignition delay is an agent fit, not a manufacturer trigger-to-launch
measurement. No geometry multiplier is applied.

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
