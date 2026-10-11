# Air-to-ground acceptance

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation validation, 2026-10-11, slice A1 of the air-to-ground round, on
branch `ag-a1` from `a2g` at `ef8000e8` (slices S1, K1, R1, R2, B1, G1, T1, H1,
W2, P1, X1, W1 and RA merged; the multiplayer slice NA, protocol 23, is not in
this base). The behaviour is in the [air-to-ground spec](../spec/air-to-ground.md).
Retail comparison is unavailable; every check here is against the spec's own
numbers and the imported records.

## Scope

Fly every weapon path of the round headless on at least two carriers each,
re-check the inventory against the code, render every HUD air-to-ground mode and
the targeting page, and list what was not run or not built.

## Machine and method

- Linux x86_64, debug build of `ef8000e8` plus the docs in this change (no
  code change), a copy of the `surface-data` import (never the shared profile).
- `tore-app --a2g-run THEATER STEM --aircraft ID --load REC --attack KIND ...`
  flies a scripted pass through the real player command path: the key actions
  for designation and weapon selection, the delivery mode, the trigger. It never
  places a projectile by hand and runs the whole mission tick, with the player's
  aircraft invulnerable. 112 runs are in the tables below, three to sixteen
  seconds each in a debug build (the longest are the high-altitude laser and
  JDAM passes). Stores an aircraft does not carry by default were fitted
  to a station for the run only, as the Cheat catalog would load them (the carrier
  counts below follow the real loading rules).
- Over the Ukraine column `~QUCOL` (tanks that start moving) for bombs,
  rockets, laser, Maverick and SARH runs; over the stationary trucks of
  `~QTTRUCK` in Vietnam for the sight and most JDAM runs; the Ukraine fleet `~QUSFLT`
  for the anti-ship runs; and the Vladivostok SAM site `~QVRDRI` (seed 4, three
  AAA and three SAM) for the HARM runs.
- `--validate-ordnance` for the inventory: 36 aircraft, 72 supported weapon
  records, 3,146 weapon placements, 146 tank placements, 2 designator pods in
  130 placements, 2 Cheat-only novelty records in 160 placements; passed. The
  carrier counts come from the same audit's station rules (a record counts for
  an aircraft when any source station takes it).
- Renders: the existing preview flags, to `.local/a2g/renders/a1/` (not
  committed; described below).

"Inside the scatter bound" means the bomb landed within the plus or minus 3
degree scatter envelope of the sight's mean impact point (the spec's rule: the
pipper marks the mean, so a single bomb can land beside it). A weapon that hits
a tank without killing it is a legitimate result: the tables show the hit points
left, and the retail warheads of the AT-2, AT-12 and AGM-65A left a T-80 alive in
these runs.

## Inventory and final status

Carriers are counted in the 36 imported aircraft (the F-22N counts apart from
the F-22; the F/A-XX is not in the audit). Every record was flown on at least two
carriers, except the two Cheat-only novelty bombs, flown on one each. The
classes are the spec's [weapon classes](../spec/air-to-ground.md#weapon-classes).

| Record | Name | Class | Carriers | Flown on | Status |
| --- | --- | --- | ---: | --- | --- |
| MK82 | MK-82 Slick | 1 | 26 | A-4E, F-4B, F/A-18D, A-7, F-16C | Built, flown |
| MK84 | MK-84 Slick | 1 | 25 | F-15, A-10 | Built, flown |
| FAB250 | FAB-250 M62 | 1 | 26 | MiG-23, Su-25, MiG-21 | Built, flown |
| FAB500 | FAB-500 M62 | 1 | 25 | Su-25, A-10 | Built, flown |
| FAB1000 | FAB-1000 M62 | 1 | 20 | Su-25, F-4E | Built, flown |
| MK82P3 | MK-82 3-Pack | 1 | 25 | A-7, F-16C | Built, flown |
| MK82HD | MK-82 Snakeye | 2 | 26 | A-4E, F-4B | Built, flown |
| MK82P | MK-82HD 6-Pack | 2 | 18 | F-4E, A-7 | Built, flown |
| MK82AIR | MK-82 AIR 7-Pack | 2 | 17 | A-4E, F/A-18D | Built, flown |
| CBU87 | CBU-87/B Cluster | 3 | 26 | A-4E, F-16C | Built, flown |
| CBU89 | CBU-97/B Cluster | 3 | 26 | F-15, A-10 | Built, flown |
| MK20 | Mk 20 Rockeye II | 3 | 26 | A-4E, A-7 | Built, flown |
| RBK250 | RBK-250 Cluster | 3 | 26 | Su-25, MiG-23 | Built, flown |
| RBK500 | RBK-500 Cluster | 3 | 26 | F-4B, Su-25 | Built, flown |
| GBU10 | GBU-10E Paveway II | 4 | 24 | F-16C, A-10, F/A-18D, F-4E | Built, flown |
| GBU10A | GBU-10 Paveway I | 4 | 24 | Su-25, A-10 | Built, flown |
| PAVEWAY | GBU-10E Paveway | 4 | 24 | F-16C, A-10 | Built, flown |
| PAVEWA3 | GBU-27 Paveway III | 4 | 24 | F-16C, Su-25 | Built, flown |
| GBU28 | GBU-28/B | 4 | 5 | F-15, Rafale C | Built, flown |
| GBU29 | GBU-29 JDAM | 5 | 25 | F-15, F-16C | Built, flown; Flies to the stored point; a moving tank is missed |
| GBU29P | GBU-29/P JDAM | 5 | 25 | F/A-18D, Rafale C | Built, flown; As GBU-29 |
| GBU30 | GBU-30 JDAM | 5 | 26 | F-16C, A-10, F/A-18D | Built, flown; As GBU-29; hits a standing target, misses a moving one |
| LAU61 | LAU-61 Rocket Pod | 6 | 26 | A-4E, AH-64 | Built, flown |
| B8 | B-8M1 Rocket Pod | 6 | 25 | Su-25, MiG-23 | Built, flown |
| B13 | B-13L Rocket Pod | 6 | 24 | Su-25, A-10, MiG-23 | Built, flown |
| LAU10 | LAU-10 Rocket Pod | 6 | 25 | A-4E, F-16C | Built, flown |
| AT2 | AT-2 Swatter | 7 | 27 | Mi-24, AH-64, A-10 | Built, flown; Hits, does not kill a T-80 (80 of 200 hit points left) |
| AGM65G | AGM-65G Maverick | 8 | 25 | A-10, F/A-18D, F-16C, AV-8B | Built, flown |
| AGM65A | AGM-65A Maverick | 8 | 25 | A-10, F/A-18D | Built, flown; Hits to about 4 nmi; from 5 nmi and beyond it falls short (see Findings) |
| AGM84E | AGM-84E SLAM | 8 | 24 | F/A-18D, F-15 | Built, flown |
| AGM88 | AGM-88 HARM | 9 | 26 | F-4G, F/A-18D, F-16C | Built, flown |
| AGM45 | AGM-45B Shrike | 9 | 26 | A-4E, F-4G | Built, flown |
| AGM84A | AGM-84A Harpoon | 10 | 20 | F/A-18D, F-15 | Built, flown; Hits the Krivak (150 of 500 left) |
| AM39 | AM-39 Exocet | 10 | 20 | Rafale C, F-16C | Built, flown; Hits the Krivak (200 of 500 left) |
| AS16 | AS-16 Kickback | 10 | 13 | Su-25, Su-35 | Built, flown; Hits the Krivak (150 of 500 left) |
| AS7 | AS-7 Kerry | 11 | 21 | MiG-23, Su-25 | Built, flown |
| AS15 | AS 15 | 11 | 21 | MiG-29, Su-27 | Built, flown |
| AS14 | AS-14 Kedge | 12 | 24 | Su-25, F-16C | Built, flown |
| AS30 | AS 30L | 12 | 24 | A-10, Su-25 | Built, flown |
| AT12 | AT-12 Swinger | 12 | 25 | Su-25, A-10 | Built, flown; Hits, does not kill a T-80 (120 of 200 left) |
| ~BLDR | Boulder | novelty | Cheat only | A-4E | Built, flown (Cheat only); Cheat catalog only; falls as a bomb |
| ~MOOSE | Moose | novelty | Cheat only | F-16C | Built, flown (Cheat only); Cheat catalog only; falls as a bomb |

Two things the spec's first inventory got wrong are corrected in the spec:
class 1 carries 20 to 26 aircraft (the FAB-1000 takes 20), not 18 to 26, and the
imported A-10 has no infrared sensor, so every FLIR designation run is the
AV-8B's (the A-10 designates on the A-G page, with Enter or with its Pave Spike).

The Pave Spike and Pave Knife pods (decision 7) are loadable on every pod
station: 130 placements pass the audit, which checks the one-pod rule, the pod's
removal by a weapon and its restoration through a saved mission. A pod was not
flown with `--a2g-run`, whose store fitting cannot load a pod (see Not run).

## Flights by weapon class

Each table row is one `--a2g-run`. The target is the driver's first ground
target (a tank of `~QUCOL`) unless the pass says otherwise; "target 28/200" is its
hit points left.

### Class 1, free-fall bombs

Release by the sight's own solution on `~QUCOL` (`release` kind); the CCIP and
LVL kinds are under Sights.

| Carrier | Store | Pass | Result |
| --- | --- | --- | --- |
| A-4E | MK82 | level, 5,000 ft, 450 kt | 1 bomb, 1/1 inside the scatter bound; target 165/200 |
| F-4B | MK82 | 30 degree dive | 1 bomb, 1/1 inside the scatter bound; target 110/200 |
| F-15 | MK84 | level | 1 bomb, 1/1 inside the scatter bound; target 28/200 |
| A-10 | MK84 | 20 degree dive | 1 bomb, 1/1 inside the scatter bound; target 4/200, 1 kill |
| Su-25 | FAB500 | 25 degree dive | 1 bomb, 1/1 inside the scatter bound; target 41/200 |
| MiG-23 | FAB250 | level | 1 bomb, 1/1 inside the scatter bound; target 119/200 |
| Su-25 | FAB1000 | level | 1 bomb, 1/1 inside the scatter bound; target 0/200, 2 kills |
| A-10 | FAB500 | 20 degree dive | 1 bomb, 1/1 inside the scatter bound; target 51/200 |
| F-4E | FAB1000 | 30 degree dive | 1 bomb, 1/1 inside the scatter bound; target 0/200, 2 kills |
| A-7 | MK82P3 | level, 3-pack | 3 bombs, 3/3 inside the scatter bound; target 24/200 |
| F-16C | MK82P3 | level, 3-pack | 3 bombs, 3/3 inside the scatter bound; target 24/200 |

### Class 2, retarded bombs and packs

The 6-pack and 7-pack leave in one tick for one round, each bomb with its own
scatter and the retail stagger (see the spec's [release modes](../spec/air-to-ground.md#release-modes-and-packs)).

| Carrier | Store | Pass | Result |
| --- | --- | --- | --- |
| A-4E | MK82HD | level | 1 bomb, 1/1 inside the scatter bound; target 200/200 |
| F-4B | MK82HD | 30 degree dive | 1 bomb, 1/1 inside the scatter bound; target 161/200 |
| F-4E | MK82P | level, 6-pack | 6 bombs, 6/6 inside the scatter bound; target 0/200, 4 kills |
| A-7 | MK82P | level, 6-pack | 6 bombs, 6/6 inside the scatter bound; target 0/200, 4 kills |
| A-4E | MK82AIR | level, 7-pack | 7 bombs, 7/7 inside the scatter bound; target 0/200, 4 kills |
| F/A-18D | MK82AIR | level, 7-pack | 7 bombs, 7/7 inside the scatter bound; target 0/200, 4 kills |

### Class 3, cluster bombs

The retail wide splash and the 28 presentation-only flashes inside a 1,200 ft
disc, the same every run. The tank's hit points are the splash falling off with
distance, not a direct hit.

| Carrier | Store | Pass | Result |
| --- | --- | --- | --- |
| A-4E | CBU87 | level | 28 flashes (deterministic yes), splash 2200 ft; tank 88/200 |
| F-16C | CBU87 | level | 28 flashes (deterministic yes), splash 2200 ft; tank 88/200 |
| F-15 | CBU89 | level | 28 flashes (deterministic yes), splash 2500 ft; tank 61/200 |
| A-10 | CBU89 | 25 degree dive | 28 flashes (deterministic yes), splash 2500 ft; tank 54/200 |
| A-4E | MK20 | level | 28 flashes (deterministic yes), splash 1600 ft; tank 98/200 |
| A-7 | MK20 | level | 28 flashes (deterministic yes), splash 1600 ft; tank 98/200 |
| Su-25 | RBK250 | level | 28 flashes (deterministic yes), splash 1500 ft; tank 100/200 |
| MiG-23 | RBK250 | level | 28 flashes (deterministic yes), splash 1500 ft; tank 100/200 |
| F-4B | RBK500 | 30 degree dive | 28 flashes (deterministic yes), splash 2500 ft; tank 49/200 |
| Su-25 | RBK500 | level | 28 flashes (deterministic yes), splash 2500 ft; tank 61/200 |

### Class 4, laser bombs

Own laser: the run designates the first tank on the A-G page, the laser comes on
by itself with a laser weapon selected, and the bomb is released on the HUD's
release range cue. Buddy: a second seat lases, the first has no designator. The
second row turns the laser off 20 s after the start, so the bomb falls blind and
misses. The first F-16C run needs 480 kt at 15,000 ft; at 400 kt a GBU-10 from
15,000 ft has no reach (the sight reads OUT OF REACH throughout and the run
releases nothing), so the A-10 passes use 12,000 ft.

| Carrier | Store | Pass | Result |
| --- | --- | --- | --- |
| F-16C | GBU10 | own Pave Spike, 15,000 ft, 480 kt | released at 22.058 s; impact 9.3 ft from the target, guiding to the end; target 0/200 |
| F-16C | GBU10 | own laser, off 20 s after the start | released at 22.058 s; impact 2604.4 ft from the target, blind at the end; target 200/200 |
| A-10 | GBU10 | own Pave Spike, 12,000 ft | released at 29.000 s; impact 9.9 ft from the target, guiding to the end; target 0/200 |
| Su-25 | GBU10A | own built-in laser | released at 29.000 s; impact 9.5 ft from the target, guiding to the end; target 0/200 |
| A-10 | GBU10A | own Pave Spike | released at 29.000 s; impact 9.9 ft from the target, guiding to the end; target 0/200 |
| F-16C | PAVEWAY | own Pave Spike | released at 22.058 s; impact 9.3 ft from the target, guiding to the end; target 0/200 |
| F-16C | PAVEWA3 | own Pave Spike | released at 22.058 s; impact 9.3 ft from the target, guiding to the end; target 0/200 |
| A-10 | PAVEWAY | own Pave Spike | released at 29.000 s; impact 9.9 ft from the target, guiding to the end; target 0/200 |
| Su-25 | PAVEWA3 | own built-in laser | released at 29.000 s; impact 9.5 ft from the target, guiding to the end; target 0/200 |
| F/A-18D | GBU10 | buddy: an Su-25 lases, the F/A-18D has no designator | released at 23.767 s; impact 9.3 ft from the target, guiding to the end; target 0/200 |
| F-15 | GBU28 | buddy: an Su-25 lases | released at 23.767 s; impact 9.6 ft from the target, guiding to the end; target 0/200 |
| Rafale C | GBU28 | buddy: an F-16C lases | released at 23.767 s; impact 9.7 ft from the target, guiding to the end; target 0/200 |
| F-4E | GBU10 | buddy: an A-10 lases | released at 23.767 s; impact 9.6 ft from the target, guiding to the end; target 0/200 |

### Class 5, JDAMs

Released in overcast (`cloudy`; the night row uses `night`) on a standing gun of
the Vietnam truck park, from 12,000 ft. They fly to the point stored at
release, in any weather. The second row is the tank that drives on: the bomb hit
the stored point (12.6 ft from it) and the tank was 2,582 ft away.

| Carrier | Store | Pass | Result |
| --- | --- | --- | --- |
| F-16C | GBU30 | overcast, level, standing gun | released at 2.733 s; impact 13.2 ft from the target, guiding to the end; target 0/100 |
| F-16C | GBU30 | overcast, level, tank that drives on | released at 23.767 s; impact 2582.3 ft from the target, guiding to the end; target 200/200 |
| F-15 | GBU29 | overcast, level | released at 2.733 s; impact 13.1 ft from the target, guiding to the end; target 0/100 |
| F-16C | GBU29 | overcast, level | released at 2.733 s; impact 13.2 ft from the target, guiding to the end; target 0/100 |
| F/A-18D | GBU29P | overcast, level | released at 2.733 s; impact 12.4 ft from the target, guiding to the end; target 0/100 |
| Rafale C | GBU29P | overcast, level | released at 2.733 s; impact 13.1 ft from the target, guiding to the end; target 0/100 |
| A-10 | GBU30 | overcast, level | released at 6.033 s; impact 11.3 ft from the target, guiding to the end; target 0/100 |
| F/A-18D | GBU30 | night, level | released at 2.733 s; impact 12.4 ft from the target, guiding to the end; target 0/100 |

### Class 6, rocket pods

An unguided salvo through the trigger (`release` kind): each press fires one
rocket, a held trigger repeats four a second, the LAU-10 fires one per press. The
rocket pipper runs are under Sights.

| Carrier | Store | Pass | Result |
| --- | --- | --- | --- |
| A-4E | LAU61 | held 1 s | 5 rockets in one 1.00 s trigger, 38 to 33 left |
| AH-64 | LAU61 | held 1 s, 100 kt | 5 rockets in one 1.00 s trigger, 38 to 33 left |
| Su-25 | B8 | held 1 s | 5 rockets in one 1.00 s trigger, 38 to 33 left |
| MiG-23 | B8 | held 1 s | 5 rockets in one 1.00 s trigger, 38 to 33 left |
| Su-25 | B13 | held 1 s | 5 rockets in one 1.00 s trigger, 10 to 5 left |
| A-10 | B13 | held 1 s | 5 rockets in one 1.00 s trigger, 10 to 5 left |
| A-4E | LAU10 | held 2 s (no repeat) | 1 rockets in one 2.00 s trigger, 8 to 7 left |
| F-16C | LAU10 | held 2 s (no repeat) | 1 rockets in one 2.00 s trigger, 8 to 7 left |

### Class 7, the AT-2

Command guidance: the missile follows while its launcher keeps the target in its
sight and goes blind for good when the launcher turns away.

| Carrier | Store | Pass | Result |
| --- | --- | --- | --- |
| Mi-24 | AT2 | hover-speed pass, 300 ft | released at 8.525 s; impact 22.1 ft from the target, guiding to the end; target 80/200 |
| Mi-24 | AT2 | turning away 2 s after the launch | released at 8.525 s; impact 34866.2 ft from the target, blind at the end; target 200/200 |
| AH-64 | AT2 | pass at 60 kt | released at 0.767 s; impact 33.2 ft from the target, guiding to the end; target 80/200 |
| A-10 | AT2 | pass at 250 kt, 1,000 ft | released at 0.767 s; impact 16.9 ft from the target, guiding to the end; target 80/200 |

### Class 8, infrared air-to-ground missiles

CUED is a designation (A-G page, FLIR or Enter) with the seeker locked on it;
BORE fires at the hottest ground object near the bore with nothing designated
(here a 100 hit point object north of the column, not a tank). The AGM-65A
falls short beyond about 4.5 nmi; see Findings.

| Carrier | Store | Pass | Result |
| --- | --- | --- | --- |
| A-10 | AGM65G | designated on the A-G page, CUED | CUED launch, seeker Locked; end 18 ft from the target (last Locked); target 0/200 |
| F/A-18D | AGM65G | A-G page, CUED | CUED launch, seeker Locked; end 19 ft from the target (last Locked); target 0/200 |
| A-10 | AGM65G | BORE, nothing designated | BORE launch, seeker Locked; end 1211 ft from the target (last Locked); target 0/100 |
| F-16C | AGM65G | BORE | BORE launch, seeker Locked; end 1211 ft from the target (last Locked); target 0/100 |
| AV-8B | AGM65G | designated on the FLIR (M, T), CUED | CUED launch, seeker Locked; end 20 ft from the target (last Locked); target 0/200 |
| A-10 | AGM65A | Enter picks the target, CUED | CUED launch, seeker Locked; end 27 ft from the target (last Locked); target 20/200 |
| F/A-18D | AGM65A | A-G page, CUED, 4 nmi | CUED launch, seeker Locked; end 27 ft from the target (last Locked); target 20/200 |
| F/A-18D | AGM65A | A-G page, 5 nmi | CUED launch, seeker Locked; end 207 ft from the target (last Locked); target 181/200 |
| F/A-18D | AGM65A | A-G page, 6 nmi | CUED launch, seeker Locked; end 953 ft from the target (last Locked); target 200/200 |
| A-10 | AGM65A | A-G page, 6 nmi | CUED launch, seeker Locked; end 961 ft from the target (last Locked); target 200/200 |
| F/A-18D | AGM84E | A-G page, CUED, 12 nmi | CUED launch, seeker Locked; end 82 ft from the target (last Locked); target 0/100 |
| F-15 | AGM84E | BORE | BORE launch, seeker Locked; end 719 ft from the target (last Locked); target 0/100 |

### Class 9, anti-radiation missiles

BORE fires at the HUD cue's candidate with nothing designated; CUED uses the
HARM page (M, then T). The ace SA-6 radar shuts down about three seconds after
the launch and the HARM keeps homing and kills it; a novice radar does not shut
down.

| Carrier | Store | Pass | Result |
| --- | --- | --- | --- |
| F-4G | AGM88 | BORE, ace SA-6 radar | BORE launch, seeker Locked; end 32 ft from the target (last Locked); target 0/50 |
| F-4G | AGM88 | BORE, novice radar | BORE launch, seeker Locked; end 32 ft from the target (last Locked); target 0/50 |
| F/A-18D | AGM88 | BORE, novice radar | BORE launch, seeker Locked; end 31 ft from the target (last Locked); target 0/50 |
| F-4G | AGM88 | HARM page (M), T, CUED | CUED launch, seeker Locked; end 36 ft from the target (last Locked); target 0/50 |
| F-16C | AGM88 | HARM page, CUED | CUED launch, seeker Locked; end 36 ft from the target (last Locked); target 0/50 |
| A-4E | AGM45 | BORE | BORE launch, seeker Locked; end 23 ft from the target (last Locked); target 0/50 |
| F-4G | AGM45 | HARM page, CUED | CUED launch, seeker Locked; end 27 ft from the target (last Locked); target 0/50 |

### Class 10, active-radar air-to-ground missiles

At the Krivak frigate (500 hit points) on the A-G page: the seeker rides the
cruise profile ("Midcourse") and goes active ("Pitbull") on the ship. The warhead
leaves the Krivak damaged, not sunk.

| Carrier | Store | Pass | Result |
| --- | --- | --- | --- |
| F/A-18D | AGM84A | A-G page, CUED, 15 nmi | CUED launch, seeker Midcourse; end 61 ft from the target (last Pitbull); target 150/500 |
| F-15 | AGM84A | A-G page, CUED, 15 nmi | CUED launch, seeker Midcourse; end 59 ft from the target (last Pitbull); target 150/500 |
| Rafale C | AM39 | A-G page, CUED, 15 nmi | CUED launch, seeker Midcourse; end 173 ft from the target (last Pitbull); target 200/500 |
| F-16C | AM39 | A-G page, CUED, 15 nmi | CUED launch, seeker Midcourse; end 177 ft from the target (last Pitbull); target 200/500 |
| Su-25 | AS16 | A-G page, CUED, 6 nmi | CUED launch, seeker Midcourse; end 178 ft from the target (last Pitbull); target 150/500 |
| Su-35 | AS16 | A-G page, CUED, 6 nmi | CUED launch, seeker Midcourse; end 175 ft from the target (last Pitbull); target 150/500 |

### Class 11, semi-active air-to-ground missiles

The A-G track must stay on for the whole flight. The rows marked "A-G page off"
turn it off 2 s after the launch: the missile goes unguided and misses.

| Carrier | Store | Pass | Result |
| --- | --- | --- | --- |
| MiG-23 | AS7 | A-G track held to impact | CUED launch, seeker Locked; end 25 ft from the target (last Locked); target 0/200 |
| MiG-23 | AS7 | A-G page off 2 s after the launch | CUED launch, seeker Locked; end 9282 ft from the target (last Lost); target 200/200 |
| Su-25 | AS7 | A-G track held | CUED launch, seeker Locked; end 28 ft from the target (last Locked); target 0/200 |
| MiG-29 | AS15 | A-G track held | CUED launch, seeker Locked; end 39 ft from the target (last Locked); target 0/200 |
| Su-27 | AS15 | A-G page off 2 s after the launch | CUED launch, seeker Locked; end 9223 ft from the target (last Lost); target 200/200 |

### Class 12, laser missiles

Own laser, released inside the weapon's zone.

| Carrier | Store | Pass | Result |
| --- | --- | --- | --- |
| Su-25 | AS14 | own laser | released at 0.767 s; impact 17.0 ft from the target, guiding to the end; target 0/200 |
| F-16C | AS14 | own Pave Spike | released at 0.767 s; impact 21.2 ft from the target, guiding to the end; target 0/200 |
| A-10 | AS30 | own Pave Spike | released at 0.767 s; impact 37.7 ft from the target, guiding to the end; target 0/200 |
| Su-25 | AS30 | own laser | released at 0.767 s; impact 34.9 ft from the target, guiding to the end; target 0/200 |
| Su-25 | AT12 | own laser | released at 0.767 s; impact 13.6 ft from the target, guiding to the end; target 120/200 |
| A-10 | AT12 | own Pave Spike | released at 0.767 s; impact 25.4 ft from the target, guiding to the end; target 120/200 |

### Sights: CCIP, LVL, the rocket pipper and the warnings

CCIP: a 30 degree dive at the stationary trucks of `~QTTRUCK`, the bomb released
when the HUD's own pipper covers the target (the MiG-21 has no sight in retail
and has one here, decision 1). LVL: the A-G page designates, the bomb is selected
(LVL on selection), the trigger is held and combat releases at the cue, once, with
OVERFLY after. Rocket: a shallow dive, fired when the rocket pipper covers the
target. Warnings: a low dive, a low level pass and a high level pass.

| Carrier | Store | Pass | Result |
| --- | --- | --- | --- |
| F/A-18D | MK82 | CCIP dive 30 degrees, 2,000 ft | released with the pipper 14.2 ft from the truck (no warning); impact ground 250.9 ft from it; truck 0/50 |
| A-7 | MK82 | CCIP dive | released with the pipper 6.9 ft from the truck (no warning); impact ground 248.1 ft from it; truck 0/50 |
| F-16C | MK82 | CCIP dive | released with the pipper 11.2 ft from the truck (no warning); impact ground 250.1 ft from it; truck 0/50 |
| Su-25 | FAB250 | CCIP dive | released with the pipper 5.5 ft from the truck (no warning); impact ground 247.2 ft from it; truck 0/50 |
| MiG-21 | FAB250 | CCIP dive (retail gives it no sight) | released with the pipper 7.2 ft from the truck (no warning); impact ground 248.2 ft from it; truck 0/50 |
| F-4B | MK82 | LVL, 5,000 ft | one release at the cue (along -1.8 ft, cross 4.6 ft, radius 500); impact 359.3 ft from the target; target 154/200; OVERFLY after |
| A-4E | MK82 | LVL | one release at the cue (along -1.1 ft, cross 6.2 ft, radius 500); impact 357.7 ft from the target; target 154/200; OVERFLY after |
| F-15 | MK84 | LVL | one release at the cue (along -5.8 ft, cross 0.6 ft, radius 1000); impact 363.6 ft from the target; target 20/200; OVERFLY after |
| Su-25 | FAB250 | LVL | one release at the cue (along -2.3 ft, cross 3.9 ft, radius 600); impact 360.0 ft from the target; target 139/200; OVERFLY after |
| A-4E | LAU61 | rocket pipper, 10 degree dive | 5 rockets, 5 hit the truck, 0 landed on the pipper's point; truck 0/50 |
| Su-25 | B8 | rocket pipper | 5 rockets, 5 hit the truck, 0 landed on the pipper's point; truck 0/50 |
| MiG-23 | B13 | rocket pipper | 5 rockets, 3 hit the truck, 2 landed on the pipper's point; truck 0/50 |
| AH-64 | LAU61 | rocket pipper, 120 kt | 5 rockets, 0 hit the truck, 5 landed on the pipper's point; truck 45/50 |
| A-4E | MK82 | 300 ft dive: LOW and FRAG | released with the pipper 2.7 ft from the truck (warnings: LOW and FRAG); impact dud 30.1 ft from it; truck 50/50 |
| A-4E | MK82 | 400 ft level: FRAG | released with the pipper 3.9 ft from the truck (warnings: FRAG); impact ground 736.1 ft from it; truck 50/50 |
| A-4E | MK82 | 1,500 ft level: no warning | released with the pipper 5.2 ft from the truck (no warning); impact ground 801.9 ft from it; truck 50/50 |

## HUD and targeting page renders

Rendered offscreen from the shipped code, to `.local/a2g/renders/a1/` (local,
not committed), and viewed:

- `hud-a2g/sheet-1.png` to `sheet-3.png` (`--hud-a2g-preview`): the CCIP pipper
  bare beyond range, near maximum (short arc), mid range, inside 0.4 of the
  maximum (full arc), the caret for a level pass whose impact is below the HUD,
  LOW and FRAG together, FRAG alone, IMP three seconds after a release, the F-16C
  dot trail at three ranges, the level T far out (dashed stem), 10 s, 2 s and at
  the release, off track (arrowhead) and OVERFLY, and the rocket pipper.
- `hud-guided/guided-sheet-1.png` and `guided-sheet-2.png`
  (`--hud-guided-preview`): the release range scale for a JDAM far beyond its
  reach (OUT OF REACH), at the far edge, mid reach (IN RNG), inside the near edge,
  with nothing designated; a laser bomb lasing in reach and with the laser off
  (NO LASER); an Su-25's AS-14.
- `missile-hud-sheet.png` (`--a2g-run ... --render`): the Harpoon's range scale
  on the A-G track, the HARM boresight cue (BORE, the emitter squares with the
  candidate blinking) and CUED on the HARM page, the Maverick BORE circle and
  CUED, and an AS-7 on the A-G track.
- `targeting-sheet.png` (`--panel-snapshot --target-cam-preview`): the targeting
  page in all sixteen states: lase, lase off (the dark half of the blink),
  masked (out of range, terrain), pinned, ready, slewing, gimbal limit, TTI,
  IMPACT, buddy, both (own LASE with a BUDDY row), the no-sight buddy form, and
  the FLIR page untracked and tracked.
- `hud-laser-sheet.png` (`--hud-snapshot-state laser-*`): the HUD laser box and
  its LSR, BUD and MASK text, with the edge arrow.
- `pages-sheet.png` (`--a2g-radar ... --render`): the A-G page over the defended
  column (dots for tanks, X for SAM and AAA), the IR page of an AV-8B over the
  column and the HARM page over the SAM site.

All of these are synthetic scenes (a plain sky and ground with the target
marked, a grey scene for the page). Only the H1 and P1 slices' windowed captures
saw the real cockpit; this pass did not open a window.

## Findings

- **AGM-65A falls short at long range.** From a 3,000 ft pass on the A-G page it
  hits at 4 nmi (27 ft), misses by 207 ft at 5 nmi and by about 950 ft at 6 nmi,
  well inside its 8.23 nmi record range. It glides down and reaches the ground
  short of a moving tank. The AGM-65G, with a faster and longer-burning record,
  hits from 6 nmi. Not investigated (this may be the older record's energy, not a
  fault); a reach probe would settle it.
- **BORE can pick a ground object that is not a tank.** The Maverick's BORE shot
  picks the hottest return in its bore cone; on `~QUCOL` that is one of two 100
  hit point static objects (ids 0x40000090 and 0x40000091) north of the column.
  By design (decision 10, the hottest ground object near the bore), noted so
  nobody expects a tank.
- **The LOW dud works.** A bomb released at 300 ft lands a dud and kills nothing
  (the first plan's "unarmed bomb still bursts with splash" is closed by the H1
  dud follow-up; the `a2g-warnings` run above shows `dud`, 50 of 50 hit points).
- **Tab does not fire the gun.** The key is bound (K1) and still answers "Fire
  the gun: not implemented yet"; decision 21 promised it. Not built.
- **A GBU-10 from 15,000 ft at 400 kt has no reach**; at 480 kt it does. The
  reach window is narrow at that altitude (W1's note); the HUD says so.

## Not run and not built

Not built:

- The JDAM "release at the bar" mode on the level T (H1's solution covers
  unguided bombs; the reach scale covers the JDAM).
- The HUD's `TTI m:ss` row for a laser or GPS weapon in flight. The targeting
  page's TTI row is drawn; the HUD row is not.
- A viewer overlay for laser spots and designations: replays and recordings carry
  the events (`player.laser`, `player.designate`, `weapon.guided`), the viewer
  draws no laser spot.
- Tab (fire the gun whatever is selected), above.
- `Ctrl+M` on [the keyboard map](../tore-keyboard-map.html): the map's rules call
  for a browser check of all five sheets at two sizes and a README image
  recapture, which this headless pass cannot do. The key is in the controls
  list and the spec.
- AI wings attacking ground targets, remote ground targeting, the bombers' bomb
  camera, stores drawn on pylons, a ripple setting (all decided out of scope).
- Pave pods in the multiplayer lobby's loadout and in a replay's loadout.

Not run:

- Pave Spike and Pave Knife flown as loaded pods: `--a2g-run` cannot fit a pod, so
  the pods are covered by the audit (130 placements, restoration through the
  saved mission) and the T1 unit tests, not a flight with an F/A-18D carrying a
  pod. The F-16C and A-10 fly their own built-in Pave Spike in the laser rows.
- Anything that needs protocol 23 (slice NA, not in this base): the
  `net-a2g-loopback` scenario, a client's view of the ground pages, sights and
  guided scale, buddy lasing between two real players online, and the pod and
  novelty-record loadout over the lobby. The buddy rows above use a second seat
  of one world.
- A windowed capture of any scene: no window opened.
- The full battery (John asks for it separately). Windowed scenarios
  (`--with-windows no`).

Known and left alone:

- `combat.ground_impact` (RA's finding, still true): the replay event matches the
  old ground effect kind and is rarely written now that a ground burst is a
  blast. Fixing it would change the guard recordings.

## Checks

The change is documentation only, so no simulation or recorded value moved. Run
on the committed tree:

- `python3 tools/check_docs.py` passed; the one Mermaid diagram added to the
  architecture guide parses and renders.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked
  -- -D warnings` and `cargo build --locked --workspace --bins` passed.
- `tools/battery.py --changed ef8000e8 --with-windows no --profile
  .../surface-data` selected nothing (documentation only). Run by name instead,
  on the same profile and base: all 20 `a2g-*` scenarios, `replay-a2g`,
  `surface-harm` and `menus-ordnance-availability` passed (23 of 23). They are
  the scenarios whose passes the tables above repeat on a second carrier; their
  own checks are in [lane-flight](../testing/lane-flight.md),
  [lane-ai](../testing/lane-ai.md) and [lane-replay](../testing/lane-replay.md).
- The full battery was not run.
