# Aircraft record corrections

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation contract, 2026-10-13. John asked (opinionated, 2026-10-13) to fix
the Su-25's record and to check every other aircraft record for numbers copied
from another aircraft. The audit, with every fixed-wing record beside published
figures, is research and lives in
[the fixed-wing record audit](../formats/aircraft.md#fixed-wing-record-audit).
This page is what the game now does with what the audit found. These are not
retail behaviour: retail flies the records as they are.

## Rule

An agent decision (2026-10-13), from John's request:

- **Corrected:** a value that is another aircraft's word for word and far from
  the real aircraft (the Su-25's Su-24 numbers, the MiG-17F's MiG-21 fuel), or a
  value off by a large factor against the record's own siblings (the C-130's
  thrust, 2.7 times the AC-130U's on the same airframe and engines). Fuel flows
  that go with a corrected tank are refit too.
- **Left as retail and listed for John:** values that are only roughly off (within
  about a third), copies that happen to be close to the real figure, notional
  designs (the X-31 EFM, the 1990s F-22) and decoded envelope ceilings and G
  limits. See the audit's table.

## Corrections

| Aircraft | Field | Retail | Game | Source or fit |
| --- | --- | ---: | ---: | --- |
| Su-25 | Empty weight, lb | 41,885 | 21,605 | 9,800 kg (Wikipedia, Sukhoi Su-25, specifications, Su-25K) |
| Su-25 | Internal fuel, lb | 22,664 | 6,614 | 3,000 kg (MILAVIA, Su-25 specifications) |
| Su-25 | Maximum takeoff, lb | 90,390 | 42,549 | 19,300 kg (Wikipedia) |
| Su-25 | Military thrust, lbf | 35,270 | 19,860 | 2 x R-195 at 44.18 kN (9,930 lbf) (Wikipedia) |
| Su-25 | Military fuel flow, lb/s | 6 | 2 | `fitted`: the fleet median, 1.12 lb/s per 10,000 lbf, rounded |
| MiG-17F | Internal fuel, lb | 4,534 | 2,520 | 1,143 kg (FAS, MiG-17) |
| MiG-17F | Military fuel flow, lb/s | 3 | 1 | `fitted`: fleet median, rounded (0.64) |
| MiG-17F | Afterburner fuel flow, lb/s | 20 | 4 | `fitted`: fleet median, 5.0 lb/s per 10,000 lbf, rounded (3.7) |
| C-130 | Military thrust, lbf | 108,192 | 40,000 | `fitted`: the AC-130U record's value for the same airframe and four T56 engines |

What a player notices:

- **Su-25.** Half its former weight with its own engines: thrust-to-weight at the
  default load goes from 0.50 to 0.59. It lifts off at 120 kt in 1,210 ft (was 123 kt
  in 1,506 ft), approaches at 131 kt (unchanged), climbs a little faster and holds
  5.4 G instead of 5.0 at 450 kt and 5,000 ft. Full military power burns 2 lb/s,
  so the 6,614 lb tank lasts 55 minutes at full power (was 63 minutes on 22,664 lb
  at 6 lb/s). The maximum takeoff weight leaves 14,330 lb for stores and external
  fuel. The top speed and ceiling are the record's (604 kt, 23,812 ft) and did not
  change.
- **MiG-17F.** 2,014 lb lighter with full internal fuel. Full military power lasts
  42 minutes (was 25) and the burner 10.5 minutes (was 3.8).
- **C-130.** A believable transport: the takeoff roll at Odesa is about 1,900 ft
  and 20 s (was 660 ft and 7 s), and it climbs like a loaded turboprop. Its top
  speed is the envelope's and did not change.

## Stall reference (Su-25)

The weight-scaled stall speed takes its reference from the empty weight times a
fitted fraction ([takeoff and ground contact](takeoff-ground-contact.md#reference-fraction-per-aircraft-fitted)).
The Su-25's fraction is refit from 0.55 to 0.51 (`fitted`, agent decision,
2026-10-13) so its default takeoff load keeps the stall scale it had on the retail
record (1.74). 0.51 sits 0.02 above the liftoff jump, now at about 0.49, the same
margin as before.

The MiG-17F keeps its retail empty weight (6,000 lb against a published 8,640).
Raising it moved the variety model's stall reference and loading with it and left
the clean MiG-17F unable to climb away after liftoff; with only the fuel and flows
corrected it lifts off clean at 119 kt and climbs, as it did on the retail record.

## How it applies

`tore_formats::aircraft::Aircraft::parse` applies the corrections
(`aircraft_corrections.rs`, the mechanism slice nr-hf3 added for the V-22's
copied AH-64 record) to the decoded record, so the flight model, the loadout,
the fuel gauge, the AI and the native research path all read the same numbers.
A correction applies only while every value it replaces still holds its copied
retail number: an edited or different record is left alone. The imported files
are not changed. Every peer applies the same table, so the wire format is
unchanged, but a mixed-version session would disagree on these aircraft.

## Open findings

- **MiG-17F with drop tanks cannot climb away.** With its default load (two
  drop tanks, 12,730 lb on the retail record) the scripted player and the AI lift
  off at about 138 to 154 kt and settle back onto the ground past the runway end,
  on the retail record as on the corrected one. Clean, it flies. Not changed
  here.
- Listed for John in the audit: Rafale and F-16C empty weights about 20 percent
  low (copies of the Mirage 2000 and the F-16A), the MiG-29's fuel 1.6 times real,
  the F-22's pre-production weights, the IL-76's E-3 numbers, the AV-8's thrust,
  and the envelope ceilings of the A-10 (22,812 ft), F-15 (100,000 ft) and
  F-4 family (42,000 ft).

## Mirage 2000: one engine, one M53-P2 (2026-11-09)

**Opinionated (John, 2026-11-09), implemented by slice M2K-1.** The retail
`M2000.PT` record says 2 engines with 17,687 lbf military and 32,000 lbf
afterburner thrust, which are the Rafale template's numbers. The real Mirage 2000
has one SNECMA M53-P2 at 64.3 kN dry and 95.1 kN with afterburner (MILAVIA
Mirage 2000 specifications and aerospaceweb.org Mirage 2000 data, both accessed
2026-11-09, as cited in the [Mirage 2000 source
review](../formats/aircraft-variety.md#mirage-2000-source-review-2026-11-09)).
TORE flies the Mirage 2000 with the real engine. This is a TORE override over the
record, not retail behaviour, and the retail numbers stay documented as retail.

| Field | Retail | Game |
| --- | ---: | ---: |
| Engines | 2 | 1 |
| Military thrust, lbf | 17,687 | 14,455 |
| Afterburner thrust, lbf | 32,000 | 21,380 |

What a player notices: a lighter pull. At the retail 17,100 lb empty weight and
6,346 lb internal fuel the afterburner thrust-to-weight is 0.91 instead of 1.36
at the empty-plus-fuel weight, so the climb and the acceleration are slower than
the record's. Empty weight and internal fuel stay retail. Slice M2K-8 measures
climb and acceleration with the lower thrust and reports the envelope against the
real aircraft. Anything that follows the engine count (damage, sound, the trail
and nozzle glow of slice M2K-4, the AI) sees one engine, because the override
lives in the same place as the other record corrections
(`aircraft_corrections.rs`, applied where `Aircraft::parse` reads the record), so
the flight model, the app, the AI and the saved mission all read one set of
numbers. It applies only to `M2000.PT` and only while all three values still hold
their retail numbers. The Rafale record, which carries the same 32,000 lbf and
2 engines, is not touched, and `M2000E.PT` is not part of this identity. The
catalog row in [`fa-aircraft.csv`](../formats/fa-aircraft.csv) and the decoder
keep the retail values.

## Mirage 2000: positive G limit +9 (2026-11-10)

**Opinionated (John, 2026-11-10: "yeah go ahead and make the correction"),
implemented by slice M2K-11.** The retail `M2000.PT` record carries G rows -4 to
+8, the Rafale template's positive limit one G short of the F-16A's and the
Rafale's own +9. The real Mirage 2000 is cleared to +9 and -3.2 G (GlobalSecurity
"Mirage 2000 Specifications", "Design and Technical Features", as cited in the
[Mirage validation](../baselines/variety-flight.md#mirage-2000-validation-2026-11-10)).
TORE flies the Mirage 2000 with +9 G. The negative limit stays the record's -4.
This is a TORE override over the record, not retail behaviour; the decoder and
[`fa-aircraft.csv`](../formats/fa-aircraft.csv) keep the retail -4 and +8.

| Field | Retail | Game |
| --- | ---: | ---: |
| Lowest G row (`envMin`) | -4 | -4 |
| Highest G row (`envMax`) | 8 | 9 |
| Rows | 13 | 14 |

**The +9 row is fitted** (agent decision, 2026-11-10), from the Mirage's own +8
row and not copied from another record. The slow side (the vertices up to the
first highest one) is lift-limited, so at each altitude its speed scales by
sqrt(9 / 8) = 1.0607, the corner speed rising with the square root of G, rounded
to the record's whole ft/s. The fast side and every altitude are the +8 row's,
so the +8 row's altitude cap (20,000 ft) is the +9 row's. Each fast-side speed is
then held to the 1 G row's top speed at its altitude (a no-op for this record:
the +8 fast edge is far inside the 1 G edge). In ft/s, altitude in ft:

| Row | Points (speed, altitude) |
| --- | --- |
| +8 (retail) | (570, 0) (675, 11000) (920, 20000) (1235, 20000) (1330, 18000) (1205, 9000) (1100, 0) |
| +9 (fitted) | (605, 0) (716, 11000) (976, 20000) (1235, 20000) (1330, 18000) (1205, 9000) (1100, 0) |
| +9 (Rafale record, for comparison only) | (540, 0) (665, 9000) (910, 14000) (1240, 13000) (1195, 6500) (1190, 0) |

At sea level the +9 row spans 358 to 652 kt (the +8 row 338 to 652 kt); the
Rafale's own +9 row is a different airframe and is not used.

**Where it lives.** `aircraft_corrections.rs` (`raise_g_limit`, `plus_nine_row`),
called by `Aircraft::parse` once the envelope rows are read, so the parsed record
says `envMax` 9 and carries the +9 row for every reader: the flight model and its
loaded-elevator divisor, the envelope window, the AI, the autopilot, the
strike-class rule (`g_limit_max`), the diagnostic and native profile builders and
the checkpoint. It applies only to `M2000` and only while the record still holds
the retail range (-4 to +8) and the retail +8 polygon; the same rows on the
Rafale or F-16A identity stay +8 here, and an edited Mirage record is read as it
is. The catalog tooling (`tools/catalog_fa.py`) reads raw files and keeps the
retail columns.

What a player notices: a full-back-stick pull at 461 kt and 1,000 ft peaks at 7.96
G with full fuel (was 7.08) and 9.09 G at 300 lb (was 8.09). The loaded-elevator
rule still scales the limit down to the gross weight, so the clean, full-fuel
aircraft does not reach 9. The push limit and the sustained turns do not change.
Measurements: [Mirage validation](../baselines/variety-flight.md#mirage-2000-positive-g-limit-9-2026-11-10).

## Batch 5 record corrections: Tornado IDS, Mirage F1, F-5E (B5-2, 2026-11-10)

**Opinionated (John, 2026-11-10, batch decision round), implemented by slice
B5-2.** The source reviews in
[the variety document](../formats/aircraft-variety.md) found values in three of
the five batch records that are another aircraft's, not the real one's. John
decided, for each, to follow the real aircraft. The Gripen (570 gun rounds kept)
and the F-15E (retail kept) have no correction. Like the Mirage 2000 above, each
correction is a TORE override over the record, not retail behaviour, and applies
only to its own identity and only while every value it replaces still holds the
retail number (`aircraft_corrections.rs`, applied where `Aircraft::parse` reads
the record). The decoder, the catalog and the retail rows stay documented as
retail.

| Aircraft | Field | Retail | Game | Source |
| --- | --- | ---: | ---: | --- |
| Tornado IDS | Military thrust, lbf (both engines) | 35,270 | 17,300 | 2 x RB199-34R Mk 103 at 38.48 kN (8,650 lbf) (MILAVIA, "Tornado specifications", accessed 2026-11-10) |
| Tornado IDS | Afterburner thrust, lbf (both engines) | 50,700 | 32,150 | 2 x 71.50 kN (16,075 lbf) (MILAVIA, same page) |
| Tornado IDS | Empty weight, lb | 41,885 | 31,065 | 14,091 kg (MILAVIA; aerospaceweb.org "Tornado IDS data" agrees) |
| Tornado IDS | Maximum take-off, lb | 90,390 | 61,620 | 27,961 kg (MILAVIA) |
| Tornado IDS | Highest G row (`envMax`) | 6 | 7 | real limit +7.5 (`TOR.INF`); see below |
| Mirage F1 | Military thrust, lbf | 15,000 | 11,020 | one Atar 9K-50, 49.0 kN dry (MIGFlug, "Dassault Mirage F1") |
| Mirage F1 | Afterburner thrust, lbf | 30,000 | 15,873 | 70.6 kN (Jane's text in `MF1.INF`; MILAVIA gives 70.21 kN) |
| F-5E | Internal fuel, lb | 6,240 | 4,400 | `F5E.INF` prints 4,400 lb; 677 US gal (Wikipedia, "Northrop F-5", F-5E specifications) is about 4,500 lb |
| F-5E | Ceiling (`maxAlt`), ft | 42,000 | 51,800 | 15,800 m service ceiling (Wikipedia, same page; `F5E.INF` 15,790 m) |
| F-5E | Highest G row (`envMax`) | 8 | 7 | real limit +7.33 / -3.0 G (T.O. 1F-5E-1, cited secondhand); see below |

The Mirage F1 already says one engine, and its weights and fuel stay retail
(John: keep). The Tornado's engine count (2) and its fuel (11,221 lb) are its own
and stay; its fuel flows are still the Su-24's (6 and 38 lb/s) and are not part of
this decision. The F-5E's thrust, weights and engine count already match the real
aircraft.

**How a whole-G envelope honours +7.5 and +7.33.** The envelope rows and `envMax`
are whole G, so neither +7.5 nor +7.33 has a row of its own. Both are held to
+7, the nearest whole row below the real limit, the conservative side.

- **Tornado.** The retail record has rows -3 to +6. A fitted +7 row is added from
  the Tornado's own +6 row, the way `plus_nine_row` fits the Mirage's +9 (now
  `fitted_row`): each slow-side speed is multiplied by sqrt(7 / 6) = 1.0801 and
  rounded; the fast side and every altitude are the +6 row's; each fast-side speed
  is held to the 1 G row's top speed at its altitude (a no-op here).

  | Row | Points (speed ft/s, altitude ft) |
  | --- | --- |
  | +6 (retail) | (575, 0) (667, 7000) (880, 14000) (1120, 14000) (1140, 0) |
  | +7 (fitted) | (621, 0) (720, 7000) (951, 14000) (1120, 14000) (1140, 0) |

  Residual: 0.5 G (6.7 percent) under the real +7.5. The +6 row is a repeat of the
  record's -3 row (the review noted it), so the fitted row inherits that polygon's
  14,000 ft altitude cap.
- **F-5E.** The retail record has rows -4 to +8, but the +8 row is word for word
  the -4 row (570 ft/s at sea level, 610 at 4,000 ft, 730 at 3,000 ft, 760 at sea
  level), a copy, not an authored row. It is retired and `envMax` becomes 7, so the
  record's own +7 row is the top. Residual: 0.33 G (4.5 percent) under the real
  +7.33. The retirement applies only while the +8 row still equals the -4 row.

**The F-5E ceiling.** Two things express a ceiling: the record's `maxAlt` field
(read by the diagnostic readers) and the top of the 1 G envelope polygon (the flight
model's ceiling: above it the air is too thin to hold level flight). Both move:
`maxAlt` goes to 51,800 ft, and the two vertices at the top edge of the 1 G
polygon (580 and 965 ft/s at 42,000 ft) and of the 0 G polygon (570 and 995 ft/s
at 44,000 ft) go up by 9,800 ft, to 51,800 and 53,800 ft. That is the minimal
consistent change: no other vertex of any row moves, so the low and medium altitude
envelope, the corner speeds and the stall edge below 38,750 ft are the record's own.
The 1 G slow edge now runs from 458 ft/s at 38,750 ft straight to 580 ft/s at 51,800
ft, and the fast edge from 1,607 ft/s at 39,000 ft to 965 ft/s at 51,800 ft (about
1,300 ft/s, Mach 1.35, at 45,000 ft). The ceiling rows move only when the record's
`maxAlt` has just been corrected and both polygons are the retail ones.

Where it lives: `aircraft_corrections.rs` (`CORRECTIONS` rows, `fit_envelopes`,
`fitted_row`), called from `Aircraft::parse`; every reader sees one set of numbers.

What a player notices (`performance_probe`, clean, afterburner, local records;
the retail column is the record with one value nudged so the guards skip):

- **Tornado.** Half the thrust and 10,820 lb less empty weight. The afterburner
  thrust-to-weight at empty weight plus full internal fuel falls from 0.95 to 0.76
  (military power from 0.66 to 0.41), so the aircraft holds less speed in a hard
  turn: at 461 kt and 1,000 ft the sustained pull is the same 4.8 G but it settles
  at 508 kt where the Fencer numbers held 567 kt. The instantaneous pull at 461 kt
  and 1,000 ft rises from 5.47 to 6.17 G, because the lighter airframe is scaled
  down less by the loaded-elevator rule; the +7 row itself is not reached with
  fuel aboard.
- **Mirage F1.** The sustained 461 kt turn at 1,000 ft falls from 5.76 G to 3.63 G
  (12.7 to 8.1 degrees a second); the instantaneous pull (6.14 G) is the G rows',
  unchanged.
- **F-5E.** A little more sustained turn from the smaller fuel load (4,400 lb
  against 6,240 lb): 4.13 G against 3.63 G at 461 kt and 1,000 ft. The 461 kt
  instantaneous pull at 1,000 ft falls from 6.27 to 5.98 G with the +8 row gone.

Logs are local: `.local/m3-aircraft/checks/B5-2/`.

## Batch 5 follow-up corrections: Tornado fuel flows (B5-3, 2026-11-10)

**Opinionated (John, 2026-11-10, batch decision round 2, item D3, "follow recs"),
implemented by slice B5-3.** B5-2 left the Tornado's fuel flows as the Su-24's
(`fuelConsumption` 6 and `aftFuelConsumption` 38 lb/s). They are now those of
two real Turbo-Union RB199 Mk 103 at the thrust B5-2 set. Like the B5-2 values,
the override fires only on the Tornado identity and only while every value it
replaces, the flows included, still holds the retail number (an edited record is
read as it is); it is two more rows of `CORRECTIONS` with a unit test in
`aircraft_corrections.rs` and one in `aircraft.rs`.

**The record's unit.** `fuelConsumption` is a whole number of pounds a second for
all the engines together at full military power, and `aftFuelConsumption` the same
at full afterburner; the flight model debits that times the throttle
([rotorcraft fuel records](../formats/aircraft.md#rotorcraft-fuel-records)). The
unit is the same for the Mirage 2000 and MiG-31 audits.

| Input | Value | Source |
| --- | --- | --- |
| Thrust, dry, both engines | 17,300 lbf | B5-2 (2 x 8,650) |
| Thrust, reheat, both engines | 32,150 lbf | B5-2 (2 x 16,075) |
| RB199 dry TSFC (Tornado, static, sea level) | 0.598 lb/(lbf h) | Wikipedia "Thrust-specific fuel consumption" (RB.199 row, citing the jet-engine.net engine database), accessed 2026-11-10 |
| RB199 reheat TSFC (Tornado, static, sea level) | 2.5 lb/(lbf h) | same table |

Flow = thrust x TSFC / 3,600 s: dry 17,300 x 0.598 / 3,600 = 2.87 lb/s, stored 3;
reheat 32,150 x 2.5 / 3,600 = 22.3 lb/s, stored 22. The same table lists the
Tornado ECR's RB199 (Mk 105) at 0.637 dry; the Mk 103 value is used. The record
holds whole pounds, so the dry flow carries a rounding of +4.5 percent against the
cited figure.

| | Retail (Su-24's) | B5-3 |
| --- | --- | --- |
| `fuelConsumption` (military, lb/s) | 6 | 3 |
| `aftFuelConsumption` (afterburner, lb/s) | 38 | 22 |

What a player notices (`performance_probe`, `PROBE_ONLY=fuel`, the local record,
tank 11,221 lb, `.local/m3-aircraft/checks/B5-3/fuel-tornado.txt`): full tank at
military power lasts 62.3 minutes (it was 31.2 at 6 lb/s) and in full afterburner
8.5 minutes (it was 4.9 at 38 lb/s). The probe reads an implied military
consumption of 0.624 and an afterburner one of 2.463 lb/(lbf h) at the record's
thrust. Trimmed cruise at 65 percent of top speed burns 2.54 lb/s, 73.7 minutes
and about 890 nm from a full tank. Thrust, weights and the G rows are the B5-2
values.

## Batch 5 flight maxima and fuel flows (B5-4, 2026-11-11)

**Opinionated (John, 2026-11-11: "Correct all flight model maximums and fuel
flows per realistic numbers that fit in TORE"), implemented by slice B5-4.** It
applies to the five batch aircraft and supersedes the earlier "leave as retail"
answers for these quantities (the Gripen's ceiling and -4 limit, the F-5E's G,
the Tornado's ceiling, the Mirage F1's afterburner flow and +7 row, the F-15E's
100,000 ft ceiling). Thrust and weights are John's earlier decisions and do not
move. Like the corrections above, each is a TORE override that applies only
while the record still holds the retail value it replaces
(`aircraft_corrections.rs`: `CORRECTIONS` rows for the fields, `fit_envelopes`
for the envelope rows). The earlier sections of this page are kept as written;
where this section differs (the Tornado's +7, the F-5E's +7, the Mirage F1's
unchanged flows) it is the current rule.

**The rule for G.** Envelope rows are whole G, and the model divides a row's G by
a loading divisor, `1 + (fuel + stores) / empty weight x loadedElevator / 100`
(the F-15E record's `loadedElevator` is 43, the other four 40), so the limit on
the record is only pulled nearly empty. John's Mirage 2000 precedent (M2K-11) is
to judge the limit at half internal fuel. So for each aircraft the top row is the
whole G at which the clean aircraft, at half its internal fuel, pulls closest to
the real limit without passing it by more than about 0.3 G. The new row is fitted
from the aircraft's own top row by `fitted_row` (slow side times
sqrt(new G / old G) rounded to whole ft/s; fast side and altitudes from the base
row; fast side held to the 1 G top speed). The negative limit follows the same
test at half fuel. The record mirrors each negative row on the positive row of
twice the G (the -3 row is the +6 row, and so on), so a new -4 row is a copy of
the +8 row.

**The rule for ceilings.** The record's `maxAlt` field and the polygons agree
(the model's ceiling is the top of the 1 G polygon). The F-5E's B5-2 fix moved
the two top-edge vertices, which cannot work here: the F-15E, Gripen and Tornado
have rows above the new ceiling (the F-15E's 2 G polygon reaches 85,000 ft). Each
polygon is instead cut off at the new ceiling (the 0 G polygon at the same step
above it that the record has: 5,000, 3,000 and 4,000 ft), so every speed band up
to the ceiling is the record's own to within 1 ft/s of rounding.

**The rule for fuel flows.** The record stores whole pounds a second for all
engines, at full military power and at full afterburner. The new value is the
record's thrust times the cited thrust-specific fuel consumption, over 3,600
seconds an hour, rounded to the whole pound.

| Aircraft | Field | Retail | Game | Real (cited) | Rounding error |
| --- | --- | ---: | ---: | --- | ---: |
| F-15E | Ceiling `maxAlt`, ft | 100,000 | 60,000 | 60,000 (Wikipedia "McDonnell Douglas F-15E Strike Eagle", specifications, read 2026-11-11; the game's `F15.INF` 60,000 to 65,000) | 0 |
| F-15E | Top G row (`envMax`) | 9 | 10 | +9 (Wikipedia, same page; `F15E.INF` design +9 / -3); half fuel pulls 8.28 G with +9, 9.19 G with +10 | n/a |
| F-15E | Lowest G row (`envMin`) | -4 (a sliver, 910 to 950 ft/s) | -3 | -3 (`F15.INF`); the pull is unchanged (2.74 G push at half fuel) | n/a |
| F-15E | `fuelConsumption`, lb/s | 4 | 7 | 32,558 lbf x 0.73 lb/(lbf h) / 3,600 = 6.60 (Wikipedia "Pratt & Whitney F100", F100-PW-220 military, read 2026-11-11) | +6% |
| F-15E | `aftFuelConsumption`, lb/s | 24 | 26 | 47,860 x 1.94 / 3,600 = 25.79 (same page, full afterburner; listed under the -229) | +1% |
| Gripen | Ceiling `maxAlt`, ft | 60,000 | 50,000 | 50,000 (15,240 m; Wikipedia "Saab JAS 39 Gripen", JAS 39C/D specifications, read 2026-11-11) | 0 |
| Gripen | Lowest G row (`envMin`) | -4 (a copy of the +8 row) | -3 | -3 (same page, "+9 / -3"); half fuel pushes 3.78 G with -4, 2.83 G with -3 | n/a |
| Gripen | `fuelConsumption`, lb/s | 2 | 3 | 12,140 lbf x 0.844 / 3,600 = 2.85 (Wikipedia "Volvo RM12", read 2026-11-11) | +5% |
| Gripen | `aftFuelConsumption`, lb/s | 14 | 9 | 18,100 x 1.79 / 3,600 = 9.00 (same page) | 0 |
| F-5E | Top G row (`envMax`) | 7 (after B5-2) | 8 | +7.33 (T.O. 1F-5E-1, cited secondhand as in B5-2); half fuel pulls 6.49 G with +7, 7.41 G with +8 | n/a |
| F-5E | Lowest G row (`envMin`) | -4 (a sliver, 570 to 760 ft/s) | -3 | -3.0 (same source); pull unchanged (2.77 G at half fuel) | n/a |
| F-5E | `fuelConsumption`, lb/s | 1 | 2 | 7,000 lbf x 1.24 / 3,600 = 2.41 (Wikipedia "Thrust-specific fuel consumption", J85-GE-21, read 2026-11-11) | -17% |
| F-5E | `aftFuelConsumption`, lb/s | 8 | 6 | 10,000 x 2.13 / 3,600 = 5.92 (same table) | +1% |
| Tornado IDS | Ceiling `maxAlt`, ft | 60,000 | 50,000 | 50,000 (Wikipedia "Panavia Tornado" and the GR4 page, aerospaceweb.org, as read for TOR-8 on 2026-11-11) | 0 |
| Tornado IDS | Top G row (`envMax`) | 7 (after B5-2) | 8 | +7.5 (`TOR.INF`); half fuel pulls 6.59 G with +7, 7.52 G with +8 | n/a |
| Tornado IDS | Fuel flows, lb/s | 3 and 22 (B5-3) | 3 and 22 | confirmed: 17,300 x 0.598 / 3,600 = 2.87 and 32,150 x 2.5 / 3,600 = 22.3 | +4.5%, -1.3% |
| Mirage F1 | Top G row (`envMax`) | 7 | 8 | +7.5 (MIGFlug "Dassault Mirage F1", "as commonly published", read 2026-11-11); half fuel pulls 6.59 G with +7, 7.53 G with +8 | n/a |
| Mirage F1 | Lowest G row (`envMin`) | -3 | -4 | -3.6 (same page); half fuel pushes 2.80 G with -3, 3.74 G with -4 | n/a |
| Mirage F1 | `aftFuelConsumption`, lb/s | 23 | 9 | 15,873 lbf x 1.991 / 3,600 = 8.78 (Wikipedia TSFC table, Atar 09K-50, read 2026-11-11); the dry 3 lb/s already matches (0.98 against 0.981) | +2.5% |

Left as retail (inside about 5 percent or half a G of the real figure): the
Gripen's +9 (half fuel pulls 8.52 G; a +10 row would pull 9.47 G, 0.47 over),
the F-5E's ceiling (B5-2, 51,800 ft), the Tornado's -3, the Mirage F1's ceiling
(66,125 ft against 65,600 to 66,000) and its top speeds, the Tornado's top speeds.
The F-15E, Gripen and F-5E top speeds were not changed: the record's envelope is
not another aircraft's, and what is left is the shared drag law, which John has
not decided (see [the variety flight baseline](../baselines/variety-flight.md#batch-5-flight-maxima-and-fuel-flows-b5-4-2026-11-11)).

What a player notices (`performance_probe`, clean, afterburner, 461 kt at 1,000 ft,
full aft stick; endurance from `PROBE_ONLY=fuel`): the F-15E pulls 9.19 G at half
fuel instead of 8.28 and its military endurance on a full tank falls from 56.9 to
32.5 minutes; the Gripen can no longer push -4 (2.83 G at half fuel) and burns its
afterburner 55 percent longer (9.3 minutes); the F-5E pulls 7.41 G at half fuel
and lasts 36.7 minutes at military power (73.3 before); the Tornado and the Mirage
F1 pull 7.5 G at half fuel; the Mirage F1's afterburner lasts 11.3 minutes (4.4
before). Every figure is in the baseline.
