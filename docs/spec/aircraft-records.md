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
(`aircraft_record_fit.rs`) to the decoded record, so the flight model, the
loadout, the fuel gauge, the AI and the native research path all read the same
numbers. A correction applies only while every listed field of that record holds
its retail value: an edited or different record is left alone.
`Aircraft::parse_decoded` reads the record as retail has it, and the imported
files are not changed. Every peer applies the same table, so the wire format is
unchanged, but a mixed-version session would disagree on these three aircraft.

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
