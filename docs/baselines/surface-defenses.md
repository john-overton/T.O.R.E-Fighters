# Surface defenses acceptance

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, slice A1 of the surface-AI round, 2026-10-10. Contract:
[surface objectives and air defenses](../spec/surface-defenses.md). This is the
one baseline for the round: every Quick Mission ground target the creator
offers, in all 16 theaters, flown headless with its defenses on.

## What was run

- **Build.** Branch `sf-a1` on `surface-ai` 51b2d613 (every slice of the round
  merged), debug build, Linux x86_64 (Ryzen 9 7900X). The only code change
  over 51b2d613 that the runs use is the acceptance options of
  `--surface-objective` (below); they change nothing in a mission.
- **Data.** The imported profile `.local/surface-data` (retail 1.02F media,
  pack marker `TORE_SURFACE_V1`).
- **Missions.** All 108 offered targets (124 template entries less the 16
  "nothing" entries), heavy AAA and heavy SAM, the theater's default enemy
  nationality, surface seed 1. Each template twice: at its retail spot
  (`--no-relocate`: jitter on, relocation off) and as a mission places it
  (jitter and relocation on). Anchored templates do not relocate, so their two
  runs are the same mission.
- **The run.** An invulnerable F/A-18D flies north and south along a 20 nm line
  through the centre of the targets at 400 kt for 900 s, holding the pass
  altitude above the ground under it, without chaff, flares or jammer. Once it
  is over the centre (59 s), a Mk 82 is placed on every living target every 2
  seconds, so the objective's timing measures the objective plumbing, not the
  defenses. Two passes: **low**, 3,000 ft above the ground (guns and
  short-range SAMs; below the flak floor and the higher SAM floors) and
  **high**, 12,000 ft (long-range SAMs and flak; above most guns and
  short-range SAMs). 432 runs, 12 at a time, 14.6 to 65.8 s each (median
  32.5 s), 4 hours of run time in about 72 minutes.
- **Counted** from each run's `surface-objective: layout` and `summary` lines:
  the template's own units by class word (a M113 counts as a tank whatever slot
  it fills), the template's SAM batteries with their system, its supply trucks
  (standing and added) and parked aircraft; missiles, gun rounds and flak
  shells fired by the template's units and, apart, by the theater layout's
  units; magazine swaps, truck rearms (every empty rail of a launcher at once)
  and refills (one spare magazine); the objective; the first shot, the last
  target's fall and any crash of the player.

```sh
TORE_DATA_DIR=.local/surface-data target/debug/tore-app --surface-objective TVIET QTAAA \
    --surface-seed 1 --defenses 3 3 --shuttle --run-on --follow-terrain --altitude 12000 --seconds 900
```

Add `--no-relocate` for the retail spot. The flags are described in
[development](../DEVELOPMENT.md#surface-unit-inspection).

## Result per theater

Sums over the theater's templates. Unit, battery, truck and parked counts are
per template (the same in both modes and both passes); fire and rearms are over
its four runs (retail and relocated, low and high).

| Theater | Templates | Units (SAM / AAA / ships) | Batteries | Trucks | Parked | Low pass: missiles / rounds / flak | High pass: missiles / rounds / flak | Rearms / refills (low + high) | Targets destroyed | Outcome |
| --- | ---: | --- | ---: | ---: | ---: | --- | --- | --- | --- | --- |
| Baltics (BAL) | 8 | 334 (70 / 70 / 16) | 9 | 167 | 13 | 1,525 / 216,891 / 0 | 1,081 / 15 / 0 | 300 / 57 | 172 of 172 | 32 of 32 SUCCESS |
| Cuba (CUB) | 6 | 245 (56 / 50 / 10) | 5 | 120 | 8 | 656 / 72,870 / 0 | 401 / 0 / 0 | 164 / 21 | 140 of 140 | 24 of 24 SUCCESS |
| Egypt (EGY) | 7 | 197 (54 / 42 / 7) | 8 | 128 | 12 | 634 / 39,838 / 0 | 490 / 49 / 0 | 234 / 6 | 140 of 140 | 28 of 28 SUCCESS |
| Falklands (LFA) | 6 | 306 (54 / 44 / 11) | 7 | 151 | 9 | 327 / 25,232 / 0 | 251 / 0 / 0 | 112 / 6 | 144 of 144 | 24 of 24 SUCCESS |
| France (FRA) | 7 | 199 (60 / 22 / 6) | 0 | 126 | 30 | 220 / 69,007 / 0 | 828 / 3,584 / 0 | 233 / 14 | 156 of 156 | 28 of 28 SUCCESS |
| Greece (GRE) | 5 | 324 (28 / 22 / 17) | 0 | 92 | 4 | 412 / 37,164 / 0 | 251 / 0 / 0 | 31 / 6 | 112 of 112 | 20 of 20 SUCCESS |
| Iraq (IRA) | 8 | 647 (90 / 80 / 0) | 14 | 237 | 4 | 974 / 142,526 / 0 | 696 / 12 / 0 | 285 / 38 | 192 of 192 | 32 of 32 SUCCESS |
| Kuril Islands (KURILE) | 7 | 142 (46 / 43 / 31) | 8 | 97 | 4 | 1,133 / 140,420 / 0 | 816 / 20 / 0 | 160 / 30 | 124 of 124 | 28 of 28 SUCCESS |
| North Vietnam (TVIET) | 9 | 430 (49 / 108 / 51) | 9 | 137 | 0 | 461 / 103,505 / 0 | 404 / 570 / 9,383 | 123 / 37 | 192 of 192 | 36 of 36 SUCCESS |
| Pakistan (SPA) | 6 | 440 (73 / 60 / 0) | 12 | 181 | 7 | 638 / 118,026 / 0 | 412 / 10 / 0 | 176 / 29 | 212 of 212 | 24 of 24 SUCCESS |
| Panama (APA) | 6 | 368 (67 / 60 / 12) | 13 | 150 | 15 | 664 / 161,424 / 0 | 383 / 0 / 0 | 139 / 46 | 156 of 156 | 24 of 24 SUCCESS |
| Persian Gulf (PGU) | 6 | 414 (69 / 60 / 20) | 7 | 159 | 20 | 433 / 89,956 / 0 | 278 / 0 / 0 | 122 / 20 | 132 of 132 | 24 of 24 SUCCESS |
| South Korea (NSK) | 6 | 471 (63 / 60 / 0) | 9 | 170 | 9 | 733 / 144,915 / 0 | 402 / 0 / 0 | 211 / 46 | 220 of 220 | 24 of 24 SUCCESS |
| Taiwan (WTA) | 6 | 169 (20 / 20 / 44) | 2 | 53 | 9 | 802 / 123,556 / 0 | 649 / 0 / 0 | 74 / 18 | 152 of 152 | 24 of 24 SUCCESS |
| Ukraine (UKR) | 8 | 209 (54 / 52 / 21) | 6 | 116 | 6 | 1,358 / 160,783 / 0 | 1,023 / 40 / 0 | 153 / 31 | 256 of 256 | 32 of 32 SUCCESS |
| Vladivostok (VLA) | 7 | 208 (60 / 60 / 7) | 11 | 137 | 11 | 881 / 114,741 / 0 | 591 / 0 / 0 | 247 / 33 | 168 of 168 | 28 of 28 SUCCESS |
| All | 108 | 5103 (913 / 853 / 253) | 120 | 2221 | 161 | 11,851 / 1,760,854 / 0 | 8,956 / 4,300 / 9,383 | 2764 / 438 | 2668 of 2668 | 432 of 432 SUCCESS |

Every run built its template, flew to the end and met its objective: 432 of
432 runs SUCCESS, every target destroyed, no refused round (the 1,000-slot
projectile reserve was never reached), no error. The invulnerable jet took
19,877 hits from 20,861 missiles (95 percent; it never dispenses or turns) and
439,712 hits from 1,775,133 rounds.

The first shot came 9 to 110 s into a run (median 38 s), as the line brought
the jet into a unit's zone; the last target fell 90 to 122 s in.

Base layout units fired in 14 runs: the Cuban submarine pens (`CSUB`), the
Kuril airstrip (`KPLNGR`), the South Korean observation area (`NSFOA`) and
three North Vietnam templates at the high pass (`TBARG`, `TBUNK`, `TSTRG`); 54
missiles and 596 rounds in all. Elsewhere the line through the targets never
comes within reach of a layout's defenses.

## Layout per template

The template's units (template ids only: added trucks and radars are counted
apart), its batteries, trucks and parked aircraft at heavy defenses, seed 1;
the anchor that keeps it in place (route, strip, runway, bridge or road, town)
or none, and how far seed 1 moved it as a mission places it.

| Theater | Template | Units | SAM | AAA | Ships | Batteries | Trucks | Parked | Anchor | Moved (nm, seed 1) |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | --- | ---: |
| BAL | `BFLT` | 16 | 0 | 0 | 16 | 0 | 0 | 0 | none | 23.5 |
| BAL | `BAIR` | 57 | 10 | 10 | 0 | 0 | 24 | 9 | runway | 0.0 |
| BAL | `BBRD` | 34 | 10 | 10 | 0 | 2 (SA-6,SA-6) | 26 | 0 | bridge-or-road | 0.0 |
| BAL | `BXING` | 48 | 10 | 10 | 0 | 1 (SA-6) | 27 | 0 | none | 3.8 |
| BAL | `BACOL` | 36 | 10 | 10 | 0 | 0 | 20 | 0 | none | 9.2 |
| BAL | `BFAIR` | 51 | 10 | 10 | 0 | 1 (SA-6) | 23 | 4 | strip | 0.0 |
| BAL | `BSPPY` | 45 | 10 | 10 | 0 | 3 (SA-6,SA-6,SA-6) | 23 | 0 | none | 14.7 |
| BAL | `BSHAR` | 47 | 10 | 10 | 0 | 2 (SA-6,SA-6) | 24 | 0 | none | 16.2 |
| CUB | `CFAIR` | 58 | 10 | 10 | 0 | 1 (SA-6) | 26 | 4 | runway | 0.0 |
| CUB | `CSCUD` | 66 | 16 | 10 | 0 | 3 (SA-6,SA-6,SA-6) | 29 | 0 | none | 17.7 |
| CUB | `CSUB` | 36 | 10 | 10 | 4 | 0 | 22 | 0 | none | 0.0 |
| CUB | `CLST` | 32 | 10 | 10 | 0 | 1 (SA-6) | 21 | 0 | none | 27.7 |
| CUB | `CCARG` | 6 | 0 | 0 | 6 | 0 | 0 | 0 | none | 14.5 |
| CUB | `CCMHQ` | 47 | 10 | 10 | 0 | 0 | 22 | 4 | runway | 0.0 |
| EGY | `ESFLT` | 7 | 0 | 0 | 7 | 0 | 0 | 0 | none | 16.2 |
| EGY | `ESAIR` | 28 | 8 | 7 | 0 | 1 (SA-6) | 21 | 4 | runway | 0.0 |
| EGY | `ELAIR` | 37 | 10 | 5 | 0 | 1 (SA-6) | 21 | 8 | runway | 0.0 |
| EGY | `ECMHQ` | 31 | 9 | 8 | 0 | 2 (SA-6,SA-6) | 22 | 0 | none | 19.3 |
| EGY | `ERDRI` | 32 | 10 | 7 | 0 | 0 | 20 | 0 | none | 7.8 |
| EGY | `EARMOR` | 28 | 8 | 9 | 0 | 2 (SA-6,SA-6) | 22 | 0 | none | 14.3 |
| EGY | `ECDEF` | 34 | 9 | 6 | 0 | 2 (SA-6,SA-6) | 22 | 0 | none | 23.2 |
| LFA | `LFCARG` | 27 | 9 | 7 | 7 | 2 (SA-6,SA-6) | 22 | 0 | none | 0.0 |
| LFA | `LFPATR` | 24 | 8 | 7 | 4 | 2 (SA-6,SA-6) | 22 | 0 | none | 0.0 |
| LFA | `LFSAM` | 75 | 12 | 8 | 0 | 0 | 27 | 0 | none | 8.6 |
| LFA | `LFFAIR` | 51 | 7 | 7 | 0 | 0 | 24 | 9 | runway | 0.0 |
| LFA | `LFSTOR` | 64 | 9 | 8 | 0 | 1 (SA-6) | 31 | 0 | none | 22.3 |
| LFA | `LFCMHQ` | 65 | 9 | 7 | 0 | 2 (SA-6,SA-6) | 25 | 0 | none | 9.2 |
| FRA | `FFLT` | 6 | 0 | 0 | 6 | 0 | 0 | 8 | none | 0.0 |
| FRA | `FSAIR` | 31 | 10 | 5 | 0 | 0 | 20 | 6 | runway | 0.0 |
| FRA | `FLAIR` | 35 | 10 | 2 | 0 | 0 | 20 | 9 | runway | 0.0 |
| FRA | `FSUP` | 29 | 10 | 4 | 0 | 0 | 25 | 0 | none | 11.6 |
| FRA | `FRDRI` | 36 | 10 | 5 | 0 | 0 | 20 | 0 | none | 23.7 |
| FRA | `FCMHQ` | 31 | 10 | 3 | 0 | 0 | 21 | 0 | none | 21.4 |
| FRA | `FFACT` | 31 | 10 | 3 | 0 | 0 | 20 | 7 | strip | 0.0 |
| GRE | `GRSAIR` | 81 | 7 | 6 | 0 | 0 | 25 | 4 | strip | 0.0 |
| GRE | `GRPATR` | 4 | 0 | 0 | 4 | 0 | 0 | 0 | none | 14.9 |
| GRE | `GRRDR` | 113 | 9 | 7 | 0 | 0 | 30 | 0 | none | 15.4 |
| GRE | `GRCARG` | 10 | 0 | 0 | 10 | 0 | 0 | 0 | none | 13.2 |
| GRE | `GRSTOR` | 116 | 12 | 9 | 3 | 0 | 37 | 0 | none | 0.0 |
| IRA | `IRRDR` | 87 | 10 | 10 | 0 | 1 (SA-6) | 30 | 0 | none | 21.1 |
| IRA | `IRFAIR` | 81 | 10 | 10 | 0 | 2 (SA-6,SA-6) | 28 | 4 | runway | 0.0 |
| IRA | `IRPOW` | 47 | 10 | 10 | 0 | 3 (SA-6,SA-6,SA-6) | 23 | 0 | none | 22.7 |
| IRA | `IRCCC` | 105 | 10 | 10 | 0 | 2 (SA-6,SA-6) | 29 | 0 | none | 20.6 |
| IRA | `IRARM` | 74 | 10 | 10 | 0 | 1 (SA-6) | 30 | 0 | none | 17.7 |
| IRA | `IRSCUD` | 94 | 14 | 10 | 0 | 1 (SA-6) | 29 | 0 | none | 26.9 |
| IRA | `IRCWP` | 65 | 10 | 10 | 0 | 3 (SA-6,SA-6,SA-6) | 35 | 0 | none | 21.1 |
| IRA | `IRRETR` | 94 | 16 | 10 | 0 | 1 (SA-6) | 33 | 0 | none | 24.0 |
| KURILE | `KSFLT` | 5 | 0 | 0 | 5 | 0 | 0 | 0 | none | 8.5 |
| KURILE | `KLFLT` | 16 | 0 | 0 | 16 | 0 | 0 | 0 | none | 27.6 |
| KURILE | `KSCFT` | 26 | 10 | 10 | 6 | 1 (SA-6) | 21 | 0 | none | 0.0 |
| KURILE | `KSUB` | 15 | 6 | 5 | 4 | 1 (SA-6) | 12 | 0 | none | 0.0 |
| KURILE | `KPLNGR` | 18 | 10 | 8 | 0 | 1 (SA-6) | 19 | 4 | runway | 0.0 |
| KURILE | `KSILO` | 34 | 10 | 10 | 0 | 2 (SA-6,SA-6) | 22 | 0 | none | 0.0 |
| KURILE | `KARMOR` | 28 | 10 | 10 | 0 | 3 (SA-6,SA-6,SA-6) | 23 | 0 | none | 0.0 |
| TVIET | `TBARG` | 49 | 4 | 12 | 29 | 2 (SA-6,SA-6) | 11 | 0 | town | 0.0 |
| TVIET | `TCARGO` | 17 | 5 | 9 | 3 | 1 (SA-6) | 6 | 0 | route | 0.0 |
| TVIET | `TBRDG` | 46 | 6 | 10 | 0 | 0 | 24 | 0 | bridge-or-road | 0.0 |
| TVIET | `TBUNK` | 63 | 7 | 12 | 3 | 0 | 23 | 0 | strip | 0.0 |
| TVIET | `TCOMM` | 36 | 6 | 20 | 0 | 0 | 17 | 0 | none | 18.9 |
| TVIET | `TSTRG` | 36 | 6 | 10 | 0 | 0 | 12 | 0 | town | 0.0 |
| TVIET | `TTRUCK` | 48 | 6 | 10 | 0 | 1 (SA-6) | 30 | 0 | bridge-or-road | 0.0 |
| TVIET | `TAAA` | 66 | 5 | 8 | 8 | 1 (SA-6) | 6 | 0 | none | 0.0 |
| TVIET | `TSAM` | 69 | 4 | 17 | 8 | 4 (SA-2,SA-2,SA-2,SA-2) | 8 | 0 | none | 0.0 |
| SPA | `SPFAIR` | 75 | 10 | 10 | 0 | 2 (SA-6,SA-6) | 27 | 7 | runway | 0.0 |
| SPA | `SPSAM` | 98 | 19 | 10 | 0 | 3 (SA-3,SA-3,SA-3) | 37 | 0 | none | 14.9 |
| SPA | `SPASA` | 70 | 10 | 10 | 0 | 0 | 25 | 0 | none | 27.2 |
| SPA | `SPFRU` | 86 | 14 | 10 | 0 | 1 (SA-6) | 30 | 0 | none | 4.8 |
| SPA | `SPSUP` | 50 | 10 | 10 | 0 | 3 (SA-6,SA-6,SA-6) | 33 | 0 | none | 20.4 |
| SPA | `SPCMHQ` | 61 | 10 | 10 | 0 | 3 (SA-6,SA-6,SA-6) | 29 | 0 | none | 6.4 |
| APA | `APFAIR` | 52 | 10 | 10 | 0 | 1 (SA-6) | 26 | 7 | runway | 0.0 |
| APA | `APBLK` | 27 | 10 | 10 | 7 | 2 (SA-6,SA-6) | 22 | 0 | none | 0.0 |
| APA | `APPATR` | 55 | 10 | 10 | 5 | 0 | 23 | 0 | none | 0.0 |
| APA | `APHELO` | 59 | 10 | 10 | 0 | 2 (SA-6,SA-6) | 26 | 8 | runway | 0.0 |
| APA | `APSAM` | 76 | 17 | 10 | 0 | 6 (SA-2,SA-2,SA-2,SA-2,SA-6,SA-6) | 26 | 0 | none | 0.0 |
| APA | `APCMHQ` | 99 | 10 | 10 | 0 | 2 (SA-6,SA-6) | 27 | 0 | none | 27.5 |
| PGU | `PGPATR` | 24 | 10 | 10 | 4 | 0 | 20 | 0 | none | 0.0 |
| PGU | `PGFAIR` | 65 | 10 | 10 | 0 | 1 (SA-6) | 26 | 9 | runway | 0.0 |
| PGU | `PGSAM` | 123 | 19 | 10 | 0 | 5 (SA-3,SA-3,SA-3,SA-6,SA-6) | 35 | 4 | none | 21.4 |
| PGU | `PGSRUN` | 96 | 10 | 10 | 0 | 1 (SA-6) | 33 | 7 | town | 0.0 |
| PGU | `PGRDR` | 70 | 10 | 10 | 0 | 0 | 25 | 0 | none | 13.4 |
| PGU | `PGWSHP` | 36 | 10 | 10 | 16 | 0 | 20 | 0 | none | 0.0 |
| NSK | `NSFAIR` | 82 | 10 | 10 | 0 | 3 (SA-6,SA-6,SA-6) | 30 | 9 | runway | 0.0 |
| NSK | `NSARM` | 86 | 13 | 10 | 0 | 1 (SA-6) | 29 | 0 | none | 14.8 |
| NSK | `NSFOA` | 97 | 10 | 10 | 0 | 1 (SA-6) | 29 | 0 | none | 23.3 |
| NSK | `NSBORD` | 70 | 10 | 10 | 0 | 2 (SA-6,SA-6) | 30 | 0 | none | 16.4 |
| NSK | `NSCOL` | 46 | 10 | 10 | 0 | 1 (SA-6) | 25 | 0 | none | 21.4 |
| NSK | `NSSUP` | 90 | 10 | 10 | 0 | 1 (SA-6) | 27 | 0 | none | 12.0 |
| WTA | `WTFAIR` | 57 | 10 | 10 | 0 | 1 (SA-6) | 24 | 9 | runway | 0.0 |
| WTA | `WTPATR` | 6 | 0 | 0 | 6 | 0 | 0 | 0 | none | 28.9 |
| WTA | `WTHYDO` | 11 | 0 | 0 | 11 | 0 | 0 | 0 | none | 17.4 |
| WTA | `WTWARS` | 11 | 0 | 0 | 11 | 0 | 0 | 0 | none | 26.6 |
| WTA | `WTCARG` | 9 | 0 | 0 | 9 | 0 | 0 | 0 | none | 12.2 |
| WTA | `WTLAND` | 75 | 10 | 10 | 7 | 1 (SA-6) | 29 | 0 | none | 0.0 |
| UKR | `USFLT` | 5 | 0 | 0 | 5 | 0 | 0 | 0 | none | 11.1 |
| UKR | `ULFLT` | 16 | 0 | 0 | 16 | 0 | 0 | 0 | none | 22.4 |
| UKR | `UCITY` | 39 | 12 | 11 | 0 | 3 (SA-6,SA-6,SA-6) | 26 | 0 | town | 0.0 |
| UKR | `UFACT` | 19 | 4 | 3 | 0 | 0 | 8 | 0 | route | 0.0 |
| UKR | `USTRIP` | 52 | 10 | 10 | 0 | 1 (SA-6) | 24 | 6 | strip | 0.0 |
| UKR | `UCOL` | 29 | 10 | 10 | 0 | 1 (SA-6) | 21 | 0 | route | 0.0 |
| UKR | `UNUKE` | 34 | 15 | 11 | 0 | 1 (SA-6) | 27 | 0 | none | 6.5 |
| UKR | `UBRI` | 15 | 3 | 7 | 0 | 0 | 10 | 0 | bridge-or-road | 0.0 |
| VLA | `VSFLT` | 7 | 0 | 0 | 7 | 0 | 0 | 0 | none | 8.8 |
| VLA | `VSAIR` | 38 | 10 | 10 | 0 | 1 (SA-6) | 22 | 5 | runway | 0.0 |
| VLA | `VLAIR` | 49 | 10 | 10 | 0 | 1 (SA-6) | 21 | 6 | runway | 0.0 |
| VLA | `VCMHQ` | 27 | 10 | 10 | 0 | 3 (SA-6,SA-6,SA-6) | 23 | 0 | none | 5.6 |
| VLA | `VARMOR` | 28 | 10 | 10 | 0 | 2 (SA-6,SA-6) | 22 | 0 | none | 20.2 |
| VLA | `VRDRI` | 30 | 10 | 10 | 0 | 4 (SA-6,SA-6,SA-6,SA-6) | 24 | 0 | none | 23.2 |
| VLA | `VSUP` | 29 | 10 | 10 | 0 | 0 | 25 | 0 | none | 12.7 |

## Low pass, 3,000 ft above the ground

Missiles / gun rounds / flak shells fired by the template's units, truck
rearms / refills, the first shot and the last target's fall in seconds, the
targets destroyed, and the base layout's fire (missiles/rounds/flak) when there
was any.

| Theater | Template | Retail: missiles / rounds / flak | Retail: rearms / refills | Retail: first shot / all down (s) | Relocated: missiles / rounds / flak | Relocated: rearms / refills | Relocated: first shot / all down (s) | Targets | Base layout fire (retail, relocated) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| BAL | `BFLT` | 250 / 12,861 / 0 | 0 / 0 | 22 / 114 | 167 / 3,616 / 0 | 0 / 0 | 26 / 114 | 1/1 | 0, 0 |
| BAL | `BAIR` | 94 / 17,315 / 0 | 17 / 4 | 50 / 90 | 94 / 17,315 / 0 | 17 / 4 | 50 / 90 | 7/7 | 0, 0 |
| BAL | `BBRD` | 89 / 11,185 / 0 | 15 / 3 | 32 / 94 | 89 / 11,185 / 0 | 15 / 3 | 32 / 94 | 1/1 | 0, 0 |
| BAL | `BXING` | 74 / 22,318 / 0 | 13 / 7 | 41 / 90 | 74 / 23,857 / 0 | 13 / 8 | 41 / 90 | 6/6 | 0, 0 |
| BAL | `BACOL` | 90 / 4,838 / 0 | 15 / 1 | 36 / 92 | 82 / 10,050 / 0 | 12 / 3 | 56 / 92 | 8/8 | 0, 0 |
| BAL | `BFAIR` | 46 / 17,542 / 0 | 7 / 5 | 40 / 90 | 46 / 17,542 / 0 | 7 / 5 | 40 / 90 | 9/9 | 0, 0 |
| BAL | `BSPPY` | 78 / 16,137 / 0 | 15 / 6 | 41 / 90 | 78 / 20,187 / 0 | 16 / 6 | 40 / 90 | 10/10 | 0, 0 |
| BAL | `BSHAR` | 84 / 5,359 / 0 | 12 / 1 | 30 / 112 | 90 / 5,584 / 0 | 15 / 1 | 30 / 112 | 1/1 | 0, 0 |
| CUB | `CFAIR` | 75 / 6,722 / 0 | 12 / 2 | 30 / 90 | 75 / 6,722 / 0 | 12 / 2 | 30 / 90 | 5/5 | 0, 0 |
| CUB | `CSCUD` | 56 / 7,278 / 0 | 9 / 2 | 10 / 90 | 40 / 2,259 / 0 | 7 / 0 | 30 / 90 | 6/6 | 0, 0 |
| CUB | `CSUB` | 59 / 10,076 / 0 | 11 / 3 | 10 / 92 | 59 / 10,076 / 0 | 11 / 3 | 10 / 92 | 4/4 | 3/0/0, 3/0/0 |
| CUB | `CLST` | 52 / 244 / 0 | 8 / 0 | 41 / 92 | 58 / 3,791 / 0 | 10 / 1 | 41 / 92 | 12/12 | 0, 0 |
| CUB | `CCARG` | 0 / 0 / 0 | 0 / 0 | - / 96 | 30 / 0 / 0 | 0 / 0 | 30 / 96 | 4/4 | 0, 0 |
| CUB | `CCMHQ` | 76 / 12,851 / 0 | 13 / 4 | 50 / 96 | 76 / 12,851 / 0 | 13 / 4 | 50 / 96 | 4/4 | 0, 0 |
| EGY | `ESFLT` | 30 / 0 / 0 | 0 / 0 | 30 / 96 | 41 / 4,840 / 0 | 0 / 0 | 30 / 96 | 3/3 | 0, 0 |
| EGY | `ESAIR` | 54 / 5,367 / 0 | 8 / 1 | 41 / 90 | 54 / 5,367 / 0 | 8 / 1 | 41 / 90 | 4/4 | 0, 0 |
| EGY | `ELAIR` | 56 / 5,163 / 0 | 12 / 1 | 41 / 90 | 56 / 5,163 / 0 | 12 / 1 | 41 / 90 | 8/8 | 0, 0 |
| EGY | `ECMHQ` | 41 / 513 / 0 | 9 / 0 | 41 / 96 | 42 / 1,953 / 0 | 9 / 0 | 41 / 96 | 5/5 | 0, 0 |
| EGY | `ERDRI` | 50 / 3,115 / 0 | 10 / 1 | 66 / 92 | 50 / 4,026 / 0 | 10 / 1 | 61 / 92 | 7/7 | 0, 0 |
| EGY | `EARMOR` | 48 / 1,409 / 0 | 11 / 0 | 41 / 92 | 48 / 1,407 / 0 | 11 / 0 | 41 / 92 | 5/5 | 0, 0 |
| EGY | `ECDEF` | 36 / 774 / 0 | 6 / 0 | 20 / 92 | 28 / 741 / 0 | 6 / 0 | 30 / 92 | 3/3 | 0, 0 |
| LFA | `LFCARG` | 15 / 0 / 0 | 1 / 0 | 70 / 96 | 15 / 0 / 0 | 1 / 0 | 70 / 96 | 3/3 | 0, 0 |
| LFA | `LFPATR` | 19 / 252 / 0 | 2 / 0 | 30 / 92 | 19 / 252 / 0 | 2 / 0 | 30 / 92 | 4/4 | 0, 0 |
| LFA | `LFSAM` | 30 / 5,492 / 0 | 3 / 2 | 20 / 90 | 16 / 2,153 / 0 | 2 / 0 | 66 / 90 | 6/6 | 0, 0 |
| LFA | `LFFAIR` | 28 / 7,048 / 0 | 6 / 2 | 64 / 90 | 28 / 7,048 / 0 | 6 / 2 | 64 / 90 | 5/5 | 0, 0 |
| LFA | `LFSTOR` | 32 / 419 / 0 | 8 / 0 | 36 / 90 | 26 / 950 / 0 | 7 / 0 | 41 / 90 | 17/17 | 0, 0 |
| LFA | `LFCMHQ` | 52 / 797 / 0 | 9 / 0 | 41 / 112 | 47 / 821 / 0 | 9 / 0 | 41 / 112 | 1/1 | 0, 0 |
| FRA | `FFLT` | 110 / 6,522 / 0 | 0 / 0 | 28 / 122 | 110 / 6,522 / 0 | 0 / 0 | 28 / 122 | 1/1 | 0, 0 |
| FRA | `FSAIR` | 0 / 7,086 / 0 | 0 / 2 | 73 / 90 | 0 / 7,086 / 0 | 0 / 2 | 73 / 90 | 4/4 | 0, 0 |
| FRA | `FLAIR` | 0 / 4,632 / 0 | 0 / 1 | 72 / 90 | 0 / 4,632 / 0 | 0 / 1 | 72 / 90 | 6/6 | 0, 0 |
| FRA | `FSUP` | 0 / 6,751 / 0 | 0 / 2 | 71 / 90 | 0 / 4,409 / 0 | 0 / 1 | 71 / 90 | 5/5 | 0, 0 |
| FRA | `FRDRI` | 0 / 5,866 / 0 | 0 / 2 | 46 / 90 | 0 / 2,000 / 0 | 0 / 0 | 97 / 90 | 8/8 | 0, 0 |
| FRA | `FCMHQ` | 0 / 1,172 / 0 | 0 / 0 | 73 / 96 | 0 / 3,547 / 0 | 0 / 1 | 53 / 96 | 6/6 | 0, 0 |
| FRA | `FFACT` | 0 / 4,391 / 0 | 0 / 1 | 71 / 95 | 0 / 4,391 / 0 | 0 / 1 | 71 / 95 | 9/9 | 0, 0 |
| GRE | `GRSAIR` | 14 / 5,684 / 0 | 3 / 2 | 47 / 90 | 14 / 5,684 / 0 | 3 / 2 | 47 / 90 | 10/10 | 0, 0 |
| GRE | `GRPATR` | 9 / 588 / 0 | 0 / 0 | 52 / 92 | 12 / 786 / 0 | 0 / 0 | 22 / 92 | 4/4 | 0, 0 |
| GRE | `GRRDR` | 14 / 0 / 0 | 3 / 0 | 37 / 90 | 16 / 2,260 / 0 | 2 / 0 | 29 / 90 | 4/4 | 0, 0 |
| GRE | `GRCARG` | 112 / 7,160 / 0 | 0 / 0 | 26 / 96 | 129 / 6,080 / 0 | 0 / 0 | 22 / 96 | 3/3 | 0, 0 |
| GRE | `GRSTOR` | 46 / 4,461 / 0 | 7 / 1 | 50 / 92 | 46 / 4,461 / 0 | 7 / 1 | 50 / 92 | 7/7 | 0, 0 |
| IRA | `IRRDR` | 28 / 0 / 0 | 4 / 0 | 10 / 90 | 22 / 0 / 0 | 3 / 0 | 10 / 90 | 6/6 | 0, 0 |
| IRA | `IRFAIR` | 36 / 8,807 / 0 | 12 / 1 | 28 / 90 | 36 / 8,807 / 0 | 12 / 1 | 28 / 90 | 6/6 | 0, 0 |
| IRA | `IRPOW` | 62 / 7,391 / 0 | 14 / 2 | 41 / 98 | 70 / 15,166 / 0 | 15 / 5 | 30 / 98 | 5/5 | 0, 0 |
| IRA | `IRCCC` | 77 / 6,022 / 0 | 12 / 1 | 10 / 92 | 60 / 0 / 0 | 7 / 0 | 17 / 92 | 6/6 | 0, 0 |
| IRA | `IRARM` | 68 / 4,339 / 0 | 9 / 1 | 41 / 92 | 68 / 4,329 / 0 | 9 / 1 | 41 / 92 | 7/7 | 0, 0 |
| IRA | `IRSCUD` | 60 / 7,950 / 0 | 9 / 3 | 41 / 90 | 63 / 11,269 / 0 | 10 / 4 | 12 / 90 | 4/4 | 0, 0 |
| IRA | `IRCWP` | 80 / 15,038 / 0 | 15 / 3 | 39 / 90 | 80 / 11,775 / 0 | 14 / 2 | 41 / 90 | 6/6 | 0, 0 |
| IRA | `IRRETR` | 82 / 29,394 / 0 | 13 / 11 | 41 / 92 | 82 / 12,239 / 0 | 12 / 3 | 41 / 92 | 8/8 | 0, 0 |
| KURILE | `KSFLT` | 89 / 5,176 / 0 | 0 / 0 | 58 / 114 | 95 / 2,018 / 0 | 0 / 0 | 26 / 114 | 1/1 | 0, 0 |
| KURILE | `KLFLT` | 209 / 22,938 / 0 | 0 / 0 | 30 / 114 | 208 / 18,828 / 0 | 0 / 0 | 30 / 114 | 1/1 | 0, 0 |
| KURILE | `KSCFT` | 52 / 12,084 / 0 | 10 / 4 | 24 / 92 | 52 / 12,084 / 0 | 10 / 4 | 24 / 92 | 6/6 | 0, 0 |
| KURILE | `KSUB` | 38 / 9,793 / 0 | 7 / 3 | 35 / 92 | 38 / 9,793 / 0 | 7 / 3 | 35 / 92 | 4/4 | 0, 0 |
| KURILE | `KPLNGR` | 72 / 17,003 / 0 | 14 / 6 | 41 / 90 | 72 / 17,003 / 0 | 14 / 6 | 41 / 90 | 4/4 | 0/298/0, 0/298/0 |
| KURILE | `KSILO` | 56 / 959 / 0 | 10 / 0 | 41 / 92 | 56 / 959 / 0 | 10 / 0 | 41 / 92 | 7/7 | 0, 0 |
| KURILE | `KARMOR` | 48 / 5,891 / 0 | 10 / 2 | 40 / 92 | 48 / 5,891 / 0 | 10 / 2 | 40 / 92 | 8/8 | 0, 0 |
| TVIET | `TBARG` | 20 / 203 / 0 | 4 / 0 | 31 / 90 | 20 / 203 / 0 | 4 / 0 | 31 / 90 | 5/5 | 0, 0 |
| TVIET | `TCARGO` | 22 / 0 / 0 | 4 / 0 | 36 / 96 | 22 / 0 / 0 | 4 / 0 | 36 / 96 | 3/3 | 0, 0 |
| TVIET | `TBRDG` | 32 / 6,810 / 0 | 3 / 1 | 9 / 92 | 32 / 6,810 / 0 | 3 / 1 | 9 / 92 | 1/1 | 0, 0 |
| TVIET | `TBUNK` | 56 / 13,457 / 0 | 5 / 3 | 14 / 90 | 56 / 13,457 / 0 | 5 / 3 | 14 / 90 | 6/6 | 0, 0 |
| TVIET | `TCOMM` | 6 / 16,375 / 0 | 2 / 5 | 75 / 90 | 11 / 17,660 / 0 | 3 / 6 | 20 / 90 | 2/2 | 0, 0 |
| TVIET | `TSTRG` | 44 / 7,671 / 0 | 8 / 2 | 11 / 90 | 44 / 7,671 / 0 | 8 / 2 | 11 / 90 | 6/6 | 0, 0 |
| TVIET | `TTRUCK` | 22 / 6,142 / 0 | 4 / 2 | 41 / 90 | 22 / 6,142 / 0 | 4 / 2 | 41 / 90 | 13/13 | 0, 0 |
| TVIET | `TAAA` | 26 / 18 / 0 | 3 / 0 | 10 / 90 | 26 / 18 / 0 | 3 / 0 | 10 / 90 | 8/8 | 0, 0 |
| TVIET | `TSAM` | 0 / 431 / 0 | 0 / 0 | 18 / 98 | 0 / 437 / 0 | 0 / 0 | 18 / 98 | 4/4 | 0, 0 |
| SPA | `SPFAIR` | 84 / 10,318 / 0 | 11 / 3 | 10 / 90 | 84 / 10,318 / 0 | 11 / 3 | 10 / 90 | 8/8 | 0, 0 |
| SPA | `SPSAM` | 16 / 2,255 / 0 | 1 / 0 | 110 / 90 | 34 / 0 / 0 | 7 / 0 | 20 / 90 | 9/9 | 0, 0 |
| SPA | `SPASA` | 70 / 16,952 / 0 | 8 / 3 | 43 / 92 | 62 / 15,103 / 0 | 8 / 4 | 15 / 92 | 12/12 | 0, 0 |
| SPA | `SPFRU` | 54 / 6,585 / 0 | 11 / 2 | 26 / 90 | 38 / 321 / 0 | 6 / 0 | 41 / 90 | 5/5 | 0, 0 |
| SPA | `SPSUP` | 30 / 6,062 / 0 | 10 / 0 | 41 / 90 | 30 / 6,849 / 0 | 10 / 1 | 41 / 90 | 15/15 | 0, 0 |
| SPA | `SPCMHQ` | 64 / 23,349 / 0 | 13 / 8 | 40 / 112 | 72 / 19,914 / 0 | 14 / 5 | 41 / 112 | 4/4 | 0, 0 |
| APA | `APFAIR` | 66 / 34,134 / 0 | 12 / 12 | 40 / 90 | 66 / 34,134 / 0 | 12 / 12 | 40 / 90 | 5/5 | 0, 0 |
| APA | `APBLK` | 55 / 9,088 / 0 | 1 / 0 | 27 / 100 | 55 / 9,088 / 0 | 1 / 0 | 27 / 100 | 2/2 | 0, 0 |
| APA | `APPATR` | 38 / 6,656 / 0 | 4 / 2 | 12 / 92 | 38 / 6,656 / 0 | 4 / 2 | 12 / 92 | 5/5 | 0, 0 |
| APA | `APHELO` | 56 / 16,456 / 0 | 12 / 4 | 35 / 90 | 56 / 16,456 / 0 | 12 / 4 | 35 / 90 | 8/8 | 0, 0 |
| APA | `APSAM` | 59 / 160 / 0 | 5 / 0 | 20 / 98 | 59 / 160 / 0 | 5 / 0 | 20 / 98 | 4/4 | 0, 0 |
| APA | `APCMHQ` | 62 / 8,936 / 0 | 12 / 3 | 46 / 96 | 54 / 19,500 / 0 | 13 / 7 | 40 / 96 | 15/15 | 0, 0 |
| PGU | `PGPATR` | 9 / 1,782 / 0 | 1 / 0 | 90 / 92 | 9 / 1,782 / 0 | 1 / 0 | 90 / 92 | 4/4 | 0, 0 |
| PGU | `PGFAIR` | 78 / 19,232 / 0 | 16 / 6 | 36 / 90 | 78 / 19,232 / 0 | 16 / 6 | 36 / 90 | 4/4 | 0, 0 |
| PGU | `PGSAM` | 23 / 8,486 / 0 | 6 / 1 | 21 / 90 | 12 / 6,750 / 0 | 3 / 1 | 21 / 90 | 9/9 | 0, 0 |
| PGU | `PGSRUN` | 64 / 10,167 / 0 | 11 / 2 | 10 / 92 | 64 / 10,167 / 0 | 11 / 2 | 10 / 92 | 9/9 | 0, 0 |
| PGU | `PGRDR` | 36 / 2,010 / 0 | 6 / 0 | 56 / 90 | 54 / 9,904 / 0 | 8 / 2 | 20 / 90 | 5/5 | 0, 0 |
| PGU | `PGWSHP` | 3 / 222 / 0 | 0 / 0 | 24 / 100 | 3 / 222 / 0 | 0 / 0 | 24 / 100 | 2/2 | 0, 0 |
| NSK | `NSFAIR` | 78 / 8,149 / 0 | 15 / 2 | 41 / 90 | 78 / 8,149 / 0 | 15 / 2 | 41 / 90 | 7/7 | 0, 0 |
| NSK | `NSARM` | 48 / 2,721 / 0 | 11 / 1 | 39 / 92 | 50 / 5,127 / 0 | 10 / 1 | 26 / 92 | 8/8 | 0, 0 |
| NSK | `NSFOA` | 30 / 5,722 / 0 | 4 / 2 | 20 / 92 | 42 / 115 / 0 | 8 / 0 | 18 / 92 | 8/8 | 0, 3/0/0 |
| NSK | `NSBORD` | 48 / 15,477 / 0 | 11 / 6 | 41 / 90 | 59 / 21,664 / 0 | 15 / 5 | 41 / 90 | 10/10 | 0, 0 |
| NSK | `NSCOL` | 74 / 42,028 / 0 | 13 / 15 | 21 / 92 | 57 / 15,307 / 0 | 10 / 5 | 41 / 92 | 8/8 | 0, 0 |
| NSK | `NSSUP` | 83 / 9,214 / 0 | 12 / 3 | 18 / 90 | 86 / 11,242 / 0 | 12 / 4 | 20 / 90 | 14/14 | 0, 0 |
| WTA | `WTFAIR` | 84 / 5,494 / 0 | 12 / 2 | 41 / 90 | 84 / 5,494 / 0 | 12 / 2 | 41 / 90 | 9/9 | 0, 0 |
| WTA | `WTPATR` | 11 / 226 / 0 | 0 / 0 | 28 / 92 | 5 / 1,513 / 0 | 0 / 0 | 48 / 92 | 6/6 | 0, 0 |
| WTA | `WTHYDO` | 95 / 12,073 / 0 | 0 / 0 | 26 / 92 | 101 / 26,744 / 0 | 0 / 0 | 48 / 92 | 5/5 | 0, 0 |
| WTA | `WTWARS` | 53 / 5,538 / 0 | 0 / 0 | 22 / 96 | 58 / 12,332 / 0 | 0 / 0 | 18 / 96 | 2/2 | 0, 0 |
| WTA | `WTCARG` | 55 / 0 / 0 | 0 / 0 | 26 / 96 | 106 / 8,648 / 0 | 0 / 0 | 26 / 96 | 3/3 | 0, 0 |
| WTA | `WTLAND` | 75 / 22,747 / 0 | 13 / 7 | 41 / 92 | 75 / 22,747 / 0 | 13 / 7 | 41 / 92 | 13/13 | 0, 0 |
| UKR | `USFLT` | 124 / 5,208 / 0 | 0 / 0 | 58 / 114 | 114 / 5,218 / 0 | 0 / 0 | 58 / 114 | 1/1 | 0, 0 |
| UKR | `ULFLT` | 258 / 25,310 / 0 | 0 / 0 | 30 / 114 | 218 / 13,188 / 0 | 0 / 0 | 30 / 114 | 1/1 | 0, 0 |
| UKR | `UCITY` | 32 / 5,270 / 0 | 0 / 0 | 40 / 92 | 32 / 5,270 / 0 | 0 / 0 | 40 / 92 | 39/39 | 0, 0 |
| UKR | `UFACT` | 32 / 20,114 / 0 | 4 / 7 | 47 / 90 | 32 / 20,114 / 0 | 4 / 7 | 47 / 90 | 3/3 | 0, 0 |
| UKR | `USTRIP` | 84 / 14,025 / 0 | 13 / 4 | 38 / 90 | 84 / 14,025 / 0 | 13 / 4 | 38 / 90 | 10/10 | 0, 0 |
| UKR | `UCOL` | 50 / 9,852 / 0 | 8 / 4 | 40 / 92 | 50 / 9,852 / 0 | 8 / 4 | 40 / 92 | 3/3 | 0, 0 |
| UKR | `UNUKE` | 120 / 4,544 / 0 | 20 / 1 | 36 / 91 | 126 / 3,959 / 0 | 22 / 0 | 41 / 91 | 6/6 | 0, 0 |
| UKR | `UBRI` | 1 / 2,417 / 0 | 1 / 0 | 53 / 90 | 1 / 2,417 / 0 | 1 / 0 | 53 / 90 | 1/1 | 0, 0 |
| VLA | `VSFLT` | 53 / 0 / 0 | 0 / 0 | 67 / 96 | 91 / 8,000 / 0 | 0 / 0 | 26 / 96 | 3/3 | 0, 0 |
| VLA | `VSAIR` | 76 / 18,766 / 0 | 16 / 7 | 41 / 90 | 76 / 18,766 / 0 | 16 / 7 | 41 / 90 | 4/4 | 0, 0 |
| VLA | `VLAIR` | 71 / 6,731 / 0 | 19 / 1 | 41 / 90 | 71 / 6,731 / 0 | 19 / 1 | 41 / 90 | 11/11 | 0, 0 |
| VLA | `VCMHQ` | 70 / 3,024 / 0 | 15 / 1 | 36 / 96 | 58 / 5,093 / 0 | 11 / 1 | 38 / 96 | 6/6 | 0, 0 |
| VLA | `VARMOR` | 38 / 5,040 / 0 | 9 / 1 | 64 / 90 | 46 / 4,283 / 0 | 12 / 0 | 41 / 90 | 8/8 | 0, 0 |
| VLA | `VRDRI` | 64 / 17,539 / 0 | 10 / 6 | 35 / 90 | 72 / 15,428 / 0 | 10 / 6 | 28 / 90 | 5/5 | 0, 0 |
| VLA | `VSUP` | 51 / 2,748 / 0 | 9 / 1 | 30 / 90 | 44 / 2,592 / 0 | 7 / 1 | 55 / 90 | 5/5 | 0, 0 |

## High pass, 12,000 ft above the ground

The same columns.

| Theater | Template | Retail: missiles / rounds / flak | Retail: rearms / refills | Retail: first shot / all down (s) | Relocated: missiles / rounds / flak | Relocated: rearms / refills | Relocated: first shot / all down (s) | Targets | Base layout fire (retail, relocated) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| BAL | `BFLT` | 237 / 0 / 0 | 0 / 0 | 22 / 114 | 161 / 0 / 0 | 0 / 0 | 26 / 114 | 1/1 | 0, 0 |
| BAL | `BAIR` | 75 / 0 / 0 | 10 / 0 | 50 / 90 | 75 / 0 / 0 | 10 / 0 | 50 / 90 | 7/7 | 0, 0 |
| BAL | `BBRD` | 60 / 0 / 0 | 10 / 0 | 36 / 94 | 60 / 0 / 0 | 10 / 0 | 36 / 94 | 1/1 | 0, 0 |
| BAL | `BXING` | 16 / 0 / 0 | 5 / 0 | 41 / 90 | 20 / 10 / 0 | 5 / 0 | 41 / 90 | 6/6 | 0, 0 |
| BAL | `BACOL` | 52 / 0 / 0 | 8 / 0 | 50 / 92 | 53 / 0 / 0 | 5 / 0 | 59 / 92 | 8/8 | 0, 0 |
| BAL | `BFAIR` | 22 / 0 / 0 | 4 / 0 | 41 / 90 | 22 / 0 / 0 | 4 / 0 | 41 / 90 | 9/9 | 0, 0 |
| BAL | `BSPPY` | 34 / 0 / 0 | 7 / 0 | 41 / 90 | 48 / 5 / 0 | 10 / 0 | 41 / 90 | 10/10 | 0, 0 |
| BAL | `BSHAR` | 73 / 0 / 0 | 11 / 0 | 30 / 112 | 73 / 0 / 0 | 12 / 0 | 30 / 112 | 1/1 | 0, 0 |
| CUB | `CFAIR` | 35 / 0 / 0 | 6 / 0 | 34 / 90 | 35 / 0 / 0 | 6 / 0 | 34 / 90 | 5/5 | 0, 0 |
| CUB | `CSCUD` | 34 / 0 / 0 | 5 / 0 | 10 / 90 | 26 / 0 / 0 | 4 / 0 | 45 / 90 | 6/6 | 0, 0 |
| CUB | `CSUB` | 22 / 0 / 0 | 3 / 0 | 10 / 92 | 22 / 0 / 0 | 3 / 0 | 10 / 92 | 4/4 | 5/0/0, 5/0/0 |
| CUB | `CLST` | 43 / 0 / 0 | 6 / 0 | 41 / 92 | 44 / 0 / 0 | 7 / 0 | 41 / 92 | 12/12 | 0, 0 |
| CUB | `CCARG` | 0 / 0 / 0 | 0 / 0 | - / 96 | 24 / 0 / 0 | 0 / 0 | 30 / 96 | 4/4 | 0, 0 |
| CUB | `CCMHQ` | 58 / 0 / 0 | 9 / 0 | 51 / 96 | 58 / 0 / 0 | 9 / 0 | 51 / 96 | 4/4 | 0, 0 |
| EGY | `ESFLT` | 17 / 0 / 0 | 0 / 0 | 30 / 96 | 39 / 0 / 0 | 0 / 0 | 30 / 96 | 3/3 | 0, 0 |
| EGY | `ESAIR` | 34 / 0 / 0 | 7 / 0 | 20 / 90 | 34 / 0 / 0 | 7 / 0 | 20 / 90 | 4/4 | 0, 0 |
| EGY | `ELAIR` | 40 / 12 / 0 | 13 / 0 | 20 / 90 | 40 / 12 / 0 | 13 / 0 | 20 / 90 | 8/8 | 0, 0 |
| EGY | `ECMHQ` | 32 / 0 / 0 | 8 / 0 | 41 / 96 | 32 / 0 / 0 | 8 / 0 | 20 / 96 | 5/5 | 0, 0 |
| EGY | `ERDRI` | 48 / 0 / 0 | 13 / 0 | 20 / 92 | 42 / 10 / 0 | 11 / 0 | 20 / 92 | 7/7 | 0, 0 |
| EGY | `EARMOR` | 30 / 0 / 0 | 9 / 0 | 20 / 92 | 30 / 5 / 0 | 9 / 0 | 41 / 92 | 5/5 | 0, 0 |
| EGY | `ECDEF` | 36 / 0 / 0 | 12 / 0 | 20 / 92 | 36 / 10 / 0 | 12 / 0 | 20 / 92 | 3/3 | 0, 0 |
| LFA | `LFCARG` | 5 / 0 / 0 | 0 / 0 | 70 / 96 | 5 / 0 / 0 | 0 / 0 | 70 / 96 | 3/3 | 0, 0 |
| LFA | `LFPATR` | 19 / 0 / 0 | 2 / 0 | 30 / 92 | 19 / 0 / 0 | 2 / 0 | 30 / 92 | 4/4 | 0, 0 |
| LFA | `LFSAM` | 24 / 0 / 0 | 3 / 0 | 20 / 90 | 22 / 0 / 0 | 2 / 0 | 18 / 90 | 6/6 | 0, 0 |
| LFA | `LFFAIR` | 26 / 0 / 0 | 9 / 0 | 20 / 90 | 26 / 0 / 0 | 9 / 0 | 20 / 90 | 5/5 | 0, 0 |
| LFA | `LFSTOR` | 14 / 0 / 0 | 6 / 0 | 20 / 90 | 14 / 0 / 0 | 6 / 0 | 41 / 90 | 17/17 | 0, 0 |
| LFA | `LFCMHQ` | 48 / 0 / 0 | 10 / 0 | 20 / 112 | 29 / 0 / 0 | 7 / 0 | 20 / 112 | 1/1 | 0, 0 |
| FRA | `FFLT` | 102 / 1,792 / 0 | 0 / 0 | 28 / 122 | 102 / 1,792 / 0 | 0 / 0 | 28 / 122 | 1/1 | 0, 0 |
| FRA | `FSAIR` | 50 / 0 / 0 | 20 / 0 | 20 / 90 | 50 / 0 / 0 | 20 / 0 | 20 / 90 | 4/4 | 0, 0 |
| FRA | `FLAIR` | 52 / 0 / 0 | 20 / 0 | 20 / 90 | 52 / 0 / 0 | 20 / 0 | 20 / 90 | 6/6 | 0, 0 |
| FRA | `FSUP` | 54 / 0 / 0 | 20 / 0 | 20 / 90 | 54 / 0 / 0 | 20 / 0 | 20 / 90 | 5/5 | 0, 0 |
| FRA | `FRDRI` | 44 / 0 / 0 | 18 / 0 | 20 / 90 | 44 / 0 / 0 | 16 / 0 | 20 / 90 | 8/8 | 0, 0 |
| FRA | `FCMHQ` | 58 / 0 / 0 | 19 / 0 | 20 / 96 | 58 / 0 / 0 | 20 / 0 | 20 / 96 | 6/6 | 0, 0 |
| FRA | `FFACT` | 54 / 0 / 0 | 20 / 0 | 20 / 95 | 54 / 0 / 0 | 20 / 0 | 20 / 95 | 9/9 | 0, 0 |
| GRE | `GRSAIR` | 0 / 0 / 0 | 0 / 0 | - / 90 | 0 / 0 / 0 | 0 / 0 | - / 90 | 10/10 | 0, 0 |
| GRE | `GRPATR` | 7 / 0 / 0 | 0 / 0 | 52 / 92 | 12 / 0 / 0 | 0 / 0 | 25 / 92 | 4/4 | 0, 0 |
| GRE | `GRRDR` | 4 / 0 / 0 | 1 / 0 | 95 / 90 | 4 / 0 / 0 | 1 / 0 | 20 / 90 | 4/4 | 0, 0 |
| GRE | `GRCARG` | 97 / 0 / 0 | 0 / 0 | 26 / 96 | 119 / 0 / 0 | 0 / 0 | 22 / 96 | 3/3 | 0, 0 |
| GRE | `GRSTOR` | 4 / 0 / 0 | 2 / 0 | 20 / 92 | 4 / 0 / 0 | 2 / 0 | 20 / 92 | 7/7 | 0, 0 |
| IRA | `IRRDR` | 22 / 0 / 0 | 2 / 0 | 10 / 90 | 22 / 0 / 0 | 3 / 0 | 10 / 90 | 6/6 | 0, 0 |
| IRA | `IRFAIR` | 18 / 0 / 0 | 6 / 0 | 41 / 90 | 18 / 0 / 0 | 6 / 0 | 41 / 90 | 6/6 | 0, 0 |
| IRA | `IRPOW` | 48 / 0 / 0 | 11 / 0 | 41 / 98 | 45 / 0 / 0 | 11 / 0 | 31 / 98 | 5/5 | 0, 0 |
| IRA | `IRCCC` | 66 / 0 / 0 | 9 / 0 | 10 / 92 | 59 / 0 / 0 | 7 / 0 | 30 / 92 | 6/6 | 0, 0 |
| IRA | `IRARM` | 44 / 0 / 0 | 6 / 0 | 41 / 92 | 44 / 0 / 0 | 6 / 0 | 41 / 92 | 7/7 | 0, 0 |
| IRA | `IRSCUD` | 38 / 0 / 0 | 5 / 0 | 41 / 90 | 46 / 0 / 0 | 7 / 0 | 15 / 90 | 4/4 | 0, 0 |
| IRA | `IRCWP` | 56 / 12 / 0 | 11 / 0 | 41 / 90 | 56 / 0 / 0 | 11 / 0 | 41 / 90 | 6/6 | 0, 0 |
| IRA | `IRRETR` | 60 / 0 / 0 | 8 / 0 | 41 / 92 | 54 / 0 / 0 | 6 / 0 | 41 / 92 | 8/8 | 0, 0 |
| KURILE | `KSFLT` | 90 / 0 / 0 | 0 / 0 | 58 / 114 | 88 / 0 / 0 | 0 / 0 | 26 / 114 | 1/1 | 0, 0 |
| KURILE | `KLFLT` | 193 / 0 / 0 | 0 / 0 | 30 / 114 | 187 / 0 / 0 | 0 / 0 | 30 / 114 | 1/1 | 0, 0 |
| KURILE | `KSCFT` | 10 / 10 / 0 | 2 / 0 | 41 / 92 | 10 / 10 / 0 | 2 / 0 | 41 / 92 | 6/6 | 0, 0 |
| KURILE | `KSUB` | 22 / 0 / 0 | 4 / 0 | 36 / 92 | 22 / 0 / 0 | 4 / 0 | 36 / 92 | 4/4 | 0, 0 |
| KURILE | `KPLNGR` | 36 / 0 / 0 | 7 / 0 | 41 / 90 | 36 / 0 / 0 | 7 / 0 | 41 / 90 | 4/4 | 0, 0 |
| KURILE | `KSILO` | 38 / 0 / 0 | 8 / 0 | 41 / 92 | 38 / 0 / 0 | 8 / 0 | 41 / 92 | 7/7 | 0, 0 |
| KURILE | `KARMOR` | 23 / 0 / 0 | 8 / 0 | 41 / 92 | 23 / 0 / 0 | 8 / 0 | 41 / 92 | 8/8 | 0, 0 |
| TVIET | `TBARG` | 20 / 0 / 479 | 4 / 0 | 22 / 90 | 20 / 0 / 479 | 4 / 0 | 22 / 90 | 5/5 | 6/0/0, 6/0/0 |
| TVIET | `TCARGO` | 14 / 0 / 560 | 3 / 0 | 36 / 96 | 14 / 0 / 560 | 3 / 0 | 36 / 96 | 3/3 | 0, 0 |
| TVIET | `TBRDG` | 32 / 0 / 608 | 3 / 1 | 9 / 92 | 32 / 0 / 608 | 3 / 1 | 9 / 92 | 1/1 | 0, 0 |
| TVIET | `TBUNK` | 49 / 0 / 434 | 5 / 0 | 18 / 90 | 49 / 0 / 434 | 5 / 0 | 18 / 90 | 6/6 | 4/0/0, 4/0/0 |
| TVIET | `TCOMM` | 6 / 0 / 1188 | 2 / 2 | 24 / 90 | 0 / 0 / 1213 | 0 / 2 | 25 / 90 | 2/2 | 0, 0 |
| TVIET | `TSTRG` | 43 / 0 / 433 | 8 / 0 | 14 / 90 | 43 / 0 / 433 | 8 / 0 | 14 / 90 | 6/6 | 6/0/0, 6/0/0 |
| TVIET | `TTRUCK` | 6 / 0 / 591 | 2 / 2 | 22 / 90 | 6 / 0 / 591 | 2 / 2 | 22 / 90 | 13/13 | 0, 0 |
| TVIET | `TAAA` | 22 / 0 / 37 | 2 / 0 | 10 / 90 | 22 / 0 / 37 | 2 / 0 | 10 / 90 | 8/8 | 0, 0 |
| TVIET | `TSAM` | 13 / 258 / 349 | 0 / 0 | 18 / 98 | 13 / 312 / 349 | 0 / 0 | 18 / 98 | 4/4 | 0, 0 |
| SPA | `SPFAIR` | 68 / 0 / 0 | 9 / 0 | 10 / 90 | 68 / 0 / 0 | 9 / 0 | 10 / 90 | 8/8 | 0, 0 |
| SPA | `SPSAM` | 17 / 0 / 0 | 1 / 0 | 81 / 90 | 16 / 0 / 0 | 1 / 0 | 41 / 90 | 9/9 | 0, 0 |
| SPA | `SPASA` | 40 / 0 / 0 | 3 / 0 | 58 / 92 | 45 / 0 / 0 | 5 / 0 | 18 / 92 | 12/12 | 0, 0 |
| SPA | `SPFRU` | 35 / 0 / 0 | 6 / 0 | 41 / 90 | 30 / 10 / 0 | 5 / 0 | 41 / 90 | 5/5 | 0, 0 |
| SPA | `SPSUP` | 18 / 0 / 0 | 6 / 0 | 41 / 90 | 18 / 0 / 0 | 6 / 0 | 41 / 90 | 15/15 | 0, 0 |
| SPA | `SPCMHQ` | 23 / 0 / 0 | 7 / 0 | 41 / 112 | 34 / 0 / 0 | 8 / 0 | 41 / 112 | 4/4 | 0, 0 |
| APA | `APFAIR` | 25 / 0 / 0 | 6 / 0 | 41 / 90 | 25 / 0 / 0 | 6 / 0 | 41 / 90 | 5/5 | 0, 0 |
| APA | `APBLK` | 50 / 0 / 0 | 1 / 0 | 52 / 100 | 50 / 0 / 0 | 1 / 0 | 52 / 100 | 2/2 | 0, 0 |
| APA | `APPATR` | 20 / 0 / 0 | 2 / 0 | 15 / 92 | 20 / 0 / 0 | 2 / 0 | 15 / 92 | 5/5 | 0, 0 |
| APA | `APHELO` | 28 / 0 / 0 | 6 / 0 | 38 / 90 | 28 / 0 / 0 | 6 / 0 | 38 / 90 | 8/8 | 0, 0 |
| APA | `APSAM` | 50 / 0 / 0 | 3 / 0 | 10 / 98 | 50 / 0 / 0 | 3 / 0 | 10 / 98 | 4/4 | 0, 0 |
| APA | `APCMHQ` | 17 / 0 / 0 | 5 / 0 | 41 / 96 | 20 / 0 / 0 | 5 / 0 | 41 / 96 | 15/15 | 0, 0 |
| PGU | `PGPATR` | 1 / 0 / 0 | 0 / 0 | 90 / 92 | 1 / 0 / 0 | 0 / 0 | 90 / 92 | 4/4 | 0, 0 |
| PGU | `PGFAIR` | 36 / 0 / 0 | 7 / 0 | 40 / 90 | 36 / 0 / 0 | 7 / 0 | 40 / 90 | 4/4 | 0, 0 |
| PGU | `PGSAM` | 18 / 0 / 0 | 2 / 0 | 41 / 90 | 18 / 0 / 0 | 3 / 0 | 41 / 90 | 9/9 | 0, 0 |
| PGU | `PGSRUN` | 58 / 0 / 0 | 9 / 0 | 10 / 92 | 58 / 0 / 0 | 9 / 0 | 10 / 92 | 9/9 | 0, 0 |
| PGU | `PGRDR` | 22 / 0 / 0 | 3 / 0 | 59 / 90 | 24 / 0 / 0 | 3 / 0 | 20 / 90 | 5/5 | 0, 0 |
| PGU | `PGWSHP` | 3 / 0 / 0 | 0 / 0 | 26 / 100 | 3 / 0 / 0 | 0 / 0 | 26 / 100 | 2/2 | 0, 0 |
| NSK | `NSFAIR` | 50 / 0 / 0 | 9 / 0 | 41 / 90 | 50 / 0 / 0 | 9 / 0 | 41 / 90 | 7/7 | 0, 0 |
| NSK | `NSARM` | 22 / 0 / 0 | 4 / 0 | 41 / 92 | 35 / 0 / 0 | 6 / 0 | 38 / 92 | 8/8 | 0, 0 |
| NSK | `NSFOA` | 22 / 0 / 0 | 3 / 0 | 41 / 92 | 22 / 0 / 0 | 3 / 0 | 30 / 92 | 8/8 | 0, 3/0/0 |
| NSK | `NSBORD` | 23 / 0 / 0 | 7 / 0 | 41 / 90 | 32 / 0 / 0 | 9 / 0 | 41 / 90 | 10/10 | 0, 0 |
| NSK | `NSCOL` | 21 / 0 / 0 | 4 / 0 | 41 / 92 | 12 / 0 / 0 | 4 / 0 | 41 / 92 | 8/8 | 0, 0 |
| NSK | `NSSUP` | 54 / 0 / 0 | 8 / 0 | 30 / 90 | 59 / 0 / 0 | 9 / 0 | 30 / 90 | 14/14 | 0, 0 |
| WTA | `WTFAIR` | 51 / 0 / 0 | 7 / 0 | 41 / 90 | 51 / 0 / 0 | 7 / 0 | 41 / 90 | 9/9 | 0, 0 |
| WTA | `WTPATR` | 11 / 0 / 0 | 0 / 0 | 31 / 92 | 5 / 0 / 0 | 0 / 0 | 53 / 92 | 6/6 | 0, 0 |
| WTA | `WTHYDO` | 94 / 0 / 0 | 0 / 0 | 29 / 92 | 94 / 0 / 0 | 0 / 0 | 57 / 92 | 5/5 | 0, 0 |
| WTA | `WTWARS` | 42 / 0 / 0 | 0 / 0 | 22 / 96 | 62 / 0 / 0 | 0 / 0 | 22 / 96 | 2/2 | 0, 0 |
| WTA | `WTCARG` | 46 / 0 / 0 | 0 / 0 | 26 / 96 | 101 / 0 / 0 | 0 / 0 | 26 / 96 | 3/3 | 0, 0 |
| WTA | `WTLAND` | 46 / 0 / 0 | 5 / 0 | 41 / 92 | 46 / 0 / 0 | 5 / 0 | 41 / 92 | 13/13 | 0, 0 |
| UKR | `USFLT` | 112 / 0 / 0 | 0 / 0 | 58 / 114 | 106 / 0 / 0 | 0 / 0 | 58 / 114 | 1/1 | 0, 0 |
| UKR | `ULFLT` | 238 / 0 / 0 | 0 / 0 | 30 / 114 | 214 / 0 / 0 | 0 / 0 | 30 / 114 | 1/1 | 0, 0 |
| UKR | `UCITY` | 21 / 0 / 0 | 0 / 0 | 41 / 92 | 21 / 0 / 0 | 0 / 0 | 41 / 92 | 39/39 | 0, 0 |
| UKR | `UFACT` | 0 / 0 / 0 | 0 / 0 | - / 90 | 0 / 0 / 0 | 0 / 0 | - / 90 | 3/3 | 0, 0 |
| UKR | `USTRIP` | 48 / 20 / 0 | 9 / 0 | 41 / 90 | 48 / 20 / 0 | 9 / 0 | 41 / 90 | 10/10 | 0, 0 |
| UKR | `UCOL` | 28 / 0 / 0 | 6 / 0 | 41 / 92 | 28 / 0 / 0 | 6 / 0 | 41 / 92 | 3/3 | 0, 0 |
| UKR | `UNUKE` | 79 / 0 / 0 | 14 / 0 | 39 / 91 | 80 / 0 / 0 | 15 / 0 | 41 / 91 | 6/6 | 0, 0 |
| UKR | `UBRI` | 0 / 0 / 0 | 0 / 0 | - / 90 | 0 / 0 / 0 | 0 / 0 | - / 90 | 1/1 | 0, 0 |
| VLA | `VSFLT` | 53 / 0 / 0 | 0 / 0 | 70 / 96 | 79 / 0 / 0 | 0 / 0 | 30 / 96 | 3/3 | 0, 0 |
| VLA | `VSAIR` | 48 / 0 / 0 | 10 / 0 | 41 / 90 | 48 / 0 / 0 | 10 / 0 | 41 / 90 | 4/4 | 0, 0 |
| VLA | `VLAIR` | 47 / 0 / 0 | 11 / 0 | 41 / 90 | 47 / 0 / 0 | 11 / 0 | 41 / 90 | 11/11 | 0, 0 |
| VLA | `VCMHQ` | 56 / 0 / 0 | 12 / 0 | 41 / 96 | 43 / 0 / 0 | 10 / 0 | 41 / 96 | 6/6 | 0, 0 |
| VLA | `VARMOR` | 20 / 0 / 0 | 4 / 0 | 41 / 90 | 30 / 0 / 0 | 10 / 0 | 41 / 90 | 8/8 | 0, 0 |
| VLA | `VRDRI` | 40 / 0 / 0 | 7 / 0 | 41 / 90 | 40 / 0 / 0 | 6 / 0 | 41 / 90 | 5/5 | 0, 0 |
| VLA | `VSUP` | 16 / 0 / 0 | 1 / 0 | 30 / 90 | 24 / 0 / 0 | 2 / 0 | 61 / 90 | 5/5 | 0, 0 |

## What these runs show

- **Every theater's templates build, place, fight and resupply.** All 108
  offered targets resolve at heavy with seed 1 at both spots; batteries form
  (120 in all: SA-6 in the group 2 and 3 theaters, SA-2 in North Vietnam and
  Panama, SA-3 in Pakistan and the Persian Gulf), supply trucks stand beside
  their slots (2,221), parked aircraft stand on their fields and on the
  Clemenceau's deck (161), and trucks rearmed launchers in every theater.
- **Floors and ceilings behave as the records say.** No flak shell bursts at
  the low pass (flak fires only above 4,000 ft). At the low pass France's
  Mistral and Crotale (launch zones from 5,000 ft above the unit) and the North
  Vietnam SA-2 sites (3,000 ft floor) do not launch, while their guns fire; at
  the high pass they launch. At the high pass the short-range systems are out
  of reach: `GRSAIR` (Turkish group 4: SA-7, SA-13, ZSU-23, ZSU-57), `UFACT`
  and `UBRI` (2S6, SA-19, SA-13, ZSU-23, ZSU-57; no SA-6 in their seed 1 rolls)
  stay silent at 12,000 ft and fire at 3,000 ft. This matches the manual's
  advice to fly above 15,000 ft against AAA.
- **One template never fires on this line.** `CCARG` at its retail spot (two
  armed ships, the nearest 10,500 ft off the line, the other 33,000 ft off):
  nothing comes into reach at either altitude. Relocated, the group is turned
  and its ships launch 30 missiles at the low pass.
- **Relocation changes the fight, not the template.** Relocated runs have the
  same units, batteries and trucks; their fire differs only with the geometry.

## Findings and limits

1. **Fixed during the pass: the acceptance line flew into hills.** The first
   sweep held one altitude above the centre, and in mountainous templates the
   player crashed into terrain (Panama's command HQ at 30.6 s) and was ignored
   by the defenses from then on, which read as a silent template.
   `--follow-terrain` was added and the whole sweep rerun; no run crashed.
2. **Invulnerable, scripted, one seed.** The jet cannot be shot down, does not
   react and has no countermeasures, so these runs show that the defenses
   detect, lock, fire, run out and rearm; they say nothing about how hard a
   template is to survive. Kills here are the placed bombs' and their splash.
   Other seeds, defense levels and nationalities were not flown here
   (`surface-resolve-all` and `surface-relocate-sweep` resolve and place every
   level and 20 seeds).
3. **Not covered by this sweep:** night and the stealth AAA rule, HARMs, chaff
   and flares (the `surface-*` battery scenarios cover them), multiplayer (the
   `net-surface-*` scenarios), recordings (`replay-surface`), and anything in
   a window.
4. **Looks to check in game** (renders below): the Falklands forward SAM
   site's Crotale stands on flat dark blue ground beside the sea in the
   relocated seed 1 render; relocation keeps land units on land cells, but the
   picture is ambiguous. The Kiev fleets' four Yak-141s stay out (the Kiev's
   shape has no deck the rule finds).

## Renders

One template per theater, heavy, seed 1, relocated, drawn by the game itself
offscreen (`--surface-scene`, 1080p, 30 s into the mission, the camera framed
on a target unit or a parked aircraft), kept locally in
`.local/tmp-sf-a1/renders/` (not committed): `sheet-16-theaters.png` (4 by 4)
and one `THEATER-STEM-look.png` per theater. Templates: BAL `BAIR`, CUB
`CSCUD`, EGY `ECDEF`, LFA `LFSAM`, FRA `FFLT`, GRE `GRRDR`, IRA `IRSCUD`,
KURILE `KPLNGR`, TVIET `TSAM`, SPA `SPSAM`, APA `APSAM`, PGU `PGSAM`, NSK
`NSFOA`, WTA `WTFAIR`, UKR `UCOL`, VLA `VRDRI`.

## Known battery failures at the end of the round

The four failures the lead listed before this pass, and what the battery runs
of this pass found.

- **`replay-rec-fight-2v2`: resolved, the check was wrong.** The RIO's "I'm
  getting scorched" twice at 14.825 s and 14.95 s are two calls for two hits:
  enemy missile 3 kills Friendly 1-2 and its splash (slice X1) grazes the player
  for 5 points, then enemy missile 2 hits the player for 128. The radio spec
  makes an "I'm hit" call for every guided hit with no cooldown (only gun hits
  wait 8 s), and the 1 in 5 variant roll picked the same line. The stutter check
  in `tools/battery_scenarios/_replay_record.py` now flags a line repeated
  within 0.2 s only when the same trigger made it. Whether a second call should
  cut off one still playing is the spec's open "playback overlap" item.
- **`net-server-smoke-pvp`: a timing assumption in the test, not a bug.** 1
  failure in 28 runs here (4 alone, 24 in batches of 8). In the failing run the
  Blue bot's first merge ended in a collision with an AI fighter (both lost, no
  kill credited), Red's first kill came at 1:36 and the second, which ends the
  mission at the kill limit of 2, at 3:13. The mission did end by the kill
  limit, but after the observer bot's 35 s life, so the check "the observer was
  not given the results" failed; when the second kill comes after 200 s the
  check "the kill limit did not end the mission" fails instead (N1's case). In
  passing runs both kills come in the first pass, about 20 to 30 s in. The
  mission is UKR, which has no surface units, so the round does not touch it.
  A sturdier test would keep the observer until the mission ends and wait out
  the four-minute time limit; not changed here.
- **`ai-theater-vla-takeoff-a6`: confirmed as AL1 marked it.** It passes as a
  known failure: Friendly 2-1 leaves the map at 155.8 s (x 1,027,552, z
  1,630,208) after a ground start at Spassk Dalniy, near the north edge.
- **The five ILS-terrain land orders AL1 dropped: cannot move to Blue fields.**
  `--validate-ils` lists only seven runway ends whose 3 degree path meets
  terrain in the last 5 nm: Amiens (FRA 9), Burevestnik (KURILE 3), Hyon Ni
  (NSK 6), Donets'k and Kharkiv (UKR 5, 6), all Redfor, and L'viv and Ivano
  Frankivs'k (UKR 8, 12), the two Blue ones still covered. Restoring the five
  needs a probe option that seats the wing on Redfor ([lane notes](../testing/lane-ai.md)).
- **`replay-keys-*`: a polluted profile, fixed.** All 19 windowless keyed
  flights failed with "recordings: 1 finished, 1 partial", with this build and
  with the base 51b2d613 build, because a run made straight against the shared
  battery profile had left a `.partial` recording in its `replays/` folder,
  which the battery copies into every scenario. With that file moved out, 35 of
  35 pass.
- **`net-migrate-kill`: flaky under load.** It failed once in the changed-set
  run ("a corrected plane at the resume") while the 24-minute AI probe matrix
  ran beside it, then passed 3 of 3 alone.

Battery runs of this pass (headless unless noted, the build above):

- `tools/battery.py --changed 139e34f8 --with-windows no --budget 3600`
  (the whole round's change set; the selection keeps at most 12 per family,
  so it is trimmed whatever the budget): 317 of 319. The two failures are
  `replay-keys-bookmarks` (the polluted profile) and `net-migrate-kill`
  (flaky, above).
- `--scenario 'surface-*' --scenario 'replay-*'` on a copy of the profile
  without the stale recording: 492 of 492, `replay-rec-fight-2v2` and
  `replay-surface` included.
- `--scenario 'replay-keys-*'` on the shared profile after the cleanup: 35 of
  35.
- `--lane net --windows 1` (windowed scenarios included, through
  `tools/agent-run.sh`): 57 of 58. `net-window-migrate-smoke` failed once
  ("snapshots never came again after the host was killed", 919 s) and passed
  alone in 166 s; `net-server-smoke-pvp` passed in this run.
- `net-server-smoke-pvp` 28 times (above) and `ai-theater-vla-takeoff-a6`
  once (known failure, passes as marked).
- Not run: the full battery (it waits for John), and any scenario on Windows
  or macOS.
