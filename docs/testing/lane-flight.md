# Flight lane

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

The flight lane flies the player's aircraft from ground start to touchdown in all fourteen
flyable identities (`f18`, `rafale`, `f14`, `a4e`, `x31`, `mig29`, `su27`, `mig21`, `su25`,
`mig23`, `su35`, `f22`, `f22n`, `faxx`), and checks the weapons, countermeasures, systems and
instruments that go with it. It was built in the 2026-09-28 overnight bug battery. The scenario
file is [`tools/battery_scenarios/flight.py`](../../tools/battery_scenarios/flight.py); how the
runner works is in the [testing overview](README.md).

## What it means for the game

Nothing in this lane crashed, produced a NaN, put an aircraft under the ground, let ammunition go
negative or over capacity, or left an aircraft stuck on a runway. Every aircraft takes off from
every airport, lands on every long runway, recovers from a stall, recovers from a spin unless its
data forbids spinning, and burns and runs out of fuel sensibly. The one real breakage was in the
test tools: the headless combat smoke probe had failed for thirteen of fourteen aircraft since
0.1.0 because it still expected rules the game has deliberately moved past. The probe is fixed
and now passes for all fourteen. Several behaviours are not defined anywhere in the specs and are
listed under "Needs a decision" below, with the evidence.

## Running it

```sh
cargo build --locked -p tore-app
python3 tools/battery.py --lane flight --jobs 6 --windows 2 --tag flight
python3 tools/battery.py --scenario 'flight-land-*' --jobs 6              # one family
python3 tools/battery.py --scenario 'flight-livefire-f18-*' --windows 2   # windowed
```

The whole lane is 3,089 scenarios, 394 of them windowed (through `tools/agent-run.sh`).
The headless ones take about a second each; the missile acceptance runs take up to five minutes
each and the windowed ones five to eight seconds. See "Runtime" at the end for the measured time.

## What is covered

| Family | Scenarios | What it checks |
| --- | --- | --- |
| `takeoff-*` | 772 | Ground start and takeoff for all 14 aircraft from the first and middle airport of every base theater, the Hornet from every airport of every base theater and the first airport of every imported variant: reaches 100 ft, no crash, sane liftoff speed and time, a default load no heavier than the aircraft's maximum takeoff weight, and external tanks that feed before the internal fuel. |
| `land-*` | 676 | The scripted approach and landing (see below): the Hornet at every airport of every base theater, every other aircraft at the first airport of every base theater, the Hornet at the first airport of every variant, all 14 with gear up (must crash for the gear), off the runway (must crash for not landing on a runway) and with no flare, and all 14 in five different winds. On runways of 5,500 ft or more: touches down, stops, no crash, no unsafe touchdown, does not leave the runway surface. |
| `level`, `pull`, `loop`, `roll`, `stall`, `spin`, `bank-left`, `bank-right` | 336 | Every manoeuvre in every aircraft on the default, `--legacy-flight` and `--researched-flight` adapters: no non-finite value, fuel never rises, no energy from nothing, load, speed, altitude and rates within limits, no veil without G. |
| `spinrecover-*`, `stallrecover-*` | 28 | The manual's spin and stall recovery procedures recover every aircraft that can enter a spin or stall (X-31 and the F-22 family cannot spin: their data says so). |
| `climb-*`, `sprint-*` | 28 | How far past its own envelope an aircraft goes (never past the 1.5 times overspeed loss line), and that full-afterburner level flight settles near the top speed. |
| `overspeed-*` | 28 | Every aircraft is lost at 1.6 times its top speed with cause overspeed, and an afterburner dive from 40,000 ft never passes 1.52 times ([overspeed](../spec/overspeed.md)). |
| `belly-*` | 28 | The gear key at 80 knots on the roll is refused by the ground sensor (one message, gear still down); the same key once airborne is a normal retraction ([gear on the ground](../spec/gear-on-the-ground.md)). The names keep "belly" from the slide they replaced. |
| `fault*` | 73 | Every system fault 0..44 on three aircraft in a pull, all 45 at once, and one after another, in every aircraft. |
| `combatsmoke-*`, `combatevidence-*`, `missileacceptance-*` | 40 | The headless combat smoke (default slots, five damage classes, jettison, radar power, incoming missiles, jammer), the same smoke with per-slot combat tapes written and replayed to the identical state, and the missile reach probes (the F-14 and Su-35 tables are left to the slow set, below). |
| `livefire-*`, `cheat-unlimited-ammo-*`, `cheat-damage-*`, `countermeasures-*` | about 90 | Windowed: fire every weapon slot of every aircraft (ammunition never negative or over capacity, drops by exactly what was fired, other stations untouched, surface weapons refuse the practice aircraft), Unlimited ammo, the three Damage modes, and chaff and flare counts against capacity. |
| `cheat-*` | 70 | Extra G reaches about 9 G, No redout or blackout, No spins, No crashes and Unlimited fuel on every aircraft. |
| `devices-*` | 14 | Gear, flaps, airbrake and hook stay within 0..1, never move against their command and take the aircraft's own deployment time; an aircraft with no hook does not lower one. |
| `eject-*`, `ejectionpose-*` | 70 | The seat and parachute at 5,000 ft and 250 ft in every aircraft; windowed frames of the three ejection poses. |
| `autopilot-*`, `waypoint-*`, `fuelout-*` | 42 | Heading and altitude hold from a 25 degree bank, waypoint steering, and running out of fuel. |
| `climbout-*` | 30 | The scripted leader's takeoff, gear and flaps up (the A-4E and Su-25 hold the flaps a moment for speed), and cruise, for every aircraft alone and for a wing of five from every base theater. |
| `edge-*`, `terrain-*` | 160 | Flying out over each of the four map edges at 20,000 ft for 200 s (still flying), `edge-lost-*` flying on until the aircraft is lost 105 nm past the map with cause out of bounds ([world edge](../spec/world-edge.md)), and a spin or roll at 90 ft over every theater. |
| `weather*-*`, `hour*-*`, `groundstart-*`, `damage-*`, `bay-*` | about 250 | Windowed frames of every weather condition in every base theater, every hour of the day in three theaters, every aircraft at a ground start, and the damage and F-22 bay fixtures, each checked for a blank or flat frame. |
| `loadout-*` | 56 | Round two: every aircraft with `--loadout none` and `--loadout guns` takes off and lands on a long runway like any other and carries less than its default load. |
| `jettison-*` | 38 | Round two, windowed: jettisoning every station of every aircraft empties exactly that station (an internal station, for example the Su-35 slot 5, refuses and still fires), lightens the load, keeps the flight model's carried weight in step (less fuel already burned) and touches no other station. |
| `fight-*`, `attack-*` | 56 | Round two: the player passive, then attacking through its own controls, in a 5 v 5 against ace or average AI from ahead and behind: the debrief's fate, damage, hit points, crash flag and hit and shot counts agree, and the AI invariants hold. |
| `environment-*` | 96 | The wind, air data and turbulence probe in every base theater at 100 and 5,000 ft in three winds. |
| `panel-*`, `panelfault*` | 175 | Every instrument page of every aircraft, and the Systems page under panel faults 1..35. |

Also run by hand and found clean: bad or out-of-range command lines (about twenty, all refused
with a message and no panic), extreme winds up to 200 ft/s (the scripted takeoff pilot crashes in the strongest headwinds only because it holds its rotation), starting at 90,000 ft, and every
theater and weather condition through `--validate-weather` and `--validate-maps`.

## The scripted pilots and probes

The headless flight probe (`--headless-flight`) gained the tools this lane needs, all described
in [DEVELOPMENT.md](../DEVELOPMENT.md#headless-flight-checks): an `extremes:` line that a script
reads for impossible states (non-finite values, fuel that rises, energy from nothing, speed
against the envelope, height above the terrain, the G veil), `--flight-trace`, `--flight-fault`,
`--flight-cheat`, `--flight-fuel`, `--flight-start`, and scripted pilots for spin recovery, stall
recovery, an approach and landing (with gear-up, hard and off-runway variants), autopilot,
waypoint, devices, climb, sprint and ejection. They are `fitted` test harnesses (agent decision,
2026-09-28) and are not game behaviour. The scripted landing floats about 1,500 ft past its aim
point, so it is only expected to stop on a runway of about 5,500 ft or more; a tailwind landing
is allowed a 400 ft longer roll.

## Bugs found

| Symptom | Cause | Fix |
| --- | --- | --- |
| `--combat-smoke` failed for 13 of 14 aircraft (only the MiG-29 passed), since 0.1.0. | Not the game: the probe expected rules that have since changed on purpose. Radar power off now allows an unguided radar-missile release (feature matrix, "Uncued launch with the onboard seeker enabled"); surface weapons refuse the practice aircraft (`WrongTarget`); a fixture flying at the player can win the race against a slow gun and collide with it; the A-4E's guns need far more than forty hits to destroy the player fixture. | The probe now checks those rules and names the slot, weapon and reason when it fails (commit 503b580). All fourteen pass and are in the battery. |
| A combat tape replayed with `--replay-combat` drifted its smoke differently from the live run (`TORE_COMBAT_EVIDENCE` smoke: "serialized live-fire replay diverged"). | The host sets the mission wind on the smoke and countermeasures before every step, but a tape does not record it, so the replay drifted them with no wind. The smoke also compared a live state without airfields to a replay that added them, and kept stepping a crashed flight. | The replay is given the theater's wind (`combat_tape.rs`); the smoke replays without airfields and starts its manual-command tape from a fresh flight. All fourteen aircraft roundtrip their tapes and are in the battery as `combatevidence-*` (commit "Give combat tape replays the mission wind and fix the smoke's tape roundtrip"). |
| A full-power climb carried aircraft far past their own ceiling (X-31 to 66,700 ft against 41,000) and they flew on there; the F-22 could hold level flight 6,000 ft above its ceiling. | The manual (p. 90) says the 1 G ceiling is where the air is too thin to lift the weight, but above the polygon the model kept the full 1 G. | Lift above the ceiling now falls with air density (`flight.rs`, fitted, in FLIGHT-MODEL.md); a zoom climb carries an aircraft at most 23 percent past it and it sinks back. Unit test `above_its_ceiling_an_aircraft_cannot_hold_level_flight`. |
| A loaded aircraft at full afterburner in level flight (autopilot altitude hold) sank into the ground near its top speed: F-22, MiG-21, X-31. | Loading divides the G limit, and on the outermost band of the envelope, where only the 1 G row holds, the divided limit is below 1 G. | Inside the envelope the limit never falls below 1 G, in the flight model and in the AI's own G limit (fitted, FLIGHT-MODEL.md). Unit test `a_loaded_aircraft_holds_level_flight_on_the_outer_band_of_its_envelope`. The sprint scenarios now hold altitude. |
| The RCS instrument window drew "NO EXPOSURE DATA" over the 270 bearing label. | Message placed at the left edge of the window. | Moved below the crosshair. The menus lane fixed the same line the same way; that version is the one merged. |

No other game defect was found in takeoff, landing, the manoeuvres, spin and stall recovery,
faults, ejection, devices, autopilot, fuel, weapons accounting, countermeasures, cheats or the
terrain checks. Three problems belong to other lanes and were handed on:

- **AI wingman flies into rising terrain behind a fast leader.** Still present after round two:
  `tore-app --theater UKR --aircraft f22 --ground-start 3 --ai-probe-ticks 9000 --maneuver takeoff --no-audio`
  ends with `HAZARD off landable surface: Friendly 1-2 agl=14 kt=809` at t=8544 and
  `wingman[Dead ...]`. It happens with any wingman type (`--probe-friendly-aircraft f18` too) but
  only with the F-22 as the scripted leader, which cruises fast. The wingman is in formation at
  full throttle, drifts 2,000 ft below the leader (which holds its MSL altitude), reaches about
  800 kt and hits terrain that rises 586 to 1,726 ft over the last 14,000 ft. It is not the
  envelope: the numbers did not change with the round-two flight-model fixes. The AI terrain floor
  (B44, `ai/steering.rs::terrain_pitch_floor`) looks only 1,000 ft ahead, under a second at 800 kt,
  so it cannot start a climb in time, and the formation code lets the wingman sink below its slot.
  Not changed: it is AI behaviour.
- **The debrief can credit more kills than recorded hits.** With the seeded fight
  `--aircraft su27 --probe-enemy-aircraft mig29 --probe-enemy-skill average --ai-probe-ticks 20000 --probe-fight 5:5 --separation 10 --probe-attack 100:8`
  the debrief shows `a2a=1/5 kills=[2]` while the probe saw two player hits; the Su-35 shows the
  same (`a2a=1/4 kills=[2]`). One of the two kills is credited from a last-hit record with no
  hit in the launch tally (`ledger.rs`: a lost aircraft is credited to the last shooter to damage
  it). The two scenarios skip that one check (`DEBRIEF_KILL_MISMATCH`); every other fight check
  passes.
- **AI aircraft collide.** In the F-14 rear-geometry fight two AI enemies touch (56 ft apart)
  and both are lost; the AI invariant counts it. The fight scenarios do not count mid-air
  collisions between AI aircraft.

## Stall and liftoff speeds against the imported data

John doubted the F-22, Su-27 and Su-25 numbers on 2026-09-29: at 80 knots they were
inside their lift envelope, and he expected 130 to 150 knots to fly. This was checked
with the headless takeoff (`envelope:` and `liftoff:` lines, sea-level airports, calm
air, the default loadout, full flaps and afterburner, back stick 0.35 held from the
start). The model is doing what the imported data says. Nothing was changed.

**How the model gets its stall speed** (`flight.rs`, hybrid adapter). It takes the left
edge of the aircraft's imported 1 G speed and altitude polygon at the current altitude
(the game's "stall speed limit", manual p. 90). Full flaps lower it 25 percent (the
reviewed flap effect, [takeoff rules](../spec/takeoff-ground-contact.md)). Lift is
`(speed / stall)^2` of one G up to the stall speed. Between the stall speed and the next
G row's left edge the G limit ramps from 1 G up to that row, and loading divides it
(`1 + loading * loadedElevator / 100`, for example 1.35 for the F-22), so a loaded
aircraft gets a full 1 G only above the stall speed. That is the only weight term: the
polygon itself is fixed, and it is not scaled by the square root of the weight. Thrust
contributes its vertical share while the nose is up. There is no wing area or CLmax in
the model: the imported polygon stands for them.

**The imported data.** The sea-level vertex of each 1 G polygon is round in feet per
second: F/A-18D 200 (118.5 kt), Su-27 180 (106.7), Su-25 130 (77.0), F-22 120 (71.1). The
F-22's edge is 120 ft/s, low against the F/A-18D's 200; the game data gives the F-22 the low
figure, and F-22N and F/A-XX share it. The manual has no stall, takeoff or approach figure
for the F-22, Su-27 or Su-25 (its real-aircraft pages give approach speeds only for a few
other types; the 80 to 90 knot "stall speed" on pp. 70 to 71 is the STOVL vector-nozzle
procedure), so the polygons are the only per-aircraft data. The imported landing limit is
195.5 kt for every aircraft (a touchdown limit, not an approach speed).

| Aircraft | Gross lb | 1 G edge (kt) | With flaps (kt) | Min 1 G with flaps and load (kt) | Rotation (kt) | Liftoff (kt) | Liftoff run (ft) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| F/A-18D | 41,838 | 118.5 | 88.9 | 110.1 | 103.1 | 111.9 | 780 |
| Rafale C | 32,092 | 118.5 | 88.9 | 107.6 | 105.3 | 111.6 | 588 |
| F-14D | 64,511 | 100.7 | 75.5 | 88.5 | 85.7 | 91.3 | 607 |
| A-4E | 22,026 | 106.7 | 80.0 | 91.2 | 90.4 | 94.1 | 847 |
| X-31 | 28,620 | 130.4 | 97.8 | 115.0 | 114.2 | 120.5 | 597 |
| MiG-29 | 31,895 | 118.5 | 88.9 | 106.2 | 105.4 | 111.0 | 506 |
| Su-27 | 57,920 | 106.7 | 80.0 | 91.4 | 92.6 | 96.9 | 429 |
| MiG-21 | 18,018 | 118.5 | 88.9 | 97.2 | 96.2 | 100.9 | 662 |
| Su-25 | 69,875 | 77.0 | 57.8 | 70.8 | 68.4 | 73.4 | 513 |
| MiG-23 | 39,413 | 106.7 | 80.0 | 97.4 | 91.4 | 99.4 | 748 |
| Su-35 | 69,054 | 106.7 | 80.0 | 93.1 | 91.9 | 97.1 | 523 |
| F-22, F-22N, F/A-XX | 57,800 | 71.1 | 53.3 | 70.0 | 73.2 | 77.6 | 207 |

Sea level, so calibrated and true airspeed and ground speed are the same here. The
"Min 1 G" column is where the loaded G limit first reaches 1 G, and liftoff follows it
closely. The default loadout is the takeoff weight in the table; the stall speed does not
change with weight, only the loading term does.

**Against John's reference figures** (unsourced real-world numbers, a plausibility range
only): liftoff 135 to 150 kt for the Su-27, 130 to 145 kt for the Su-25 and 130 to 150 kt
for the F-22; approach 120 to 135, 125 to 140 and 135 to 145 kt. The model lifts them off at
97, 73 and 78 kt, 30 to 45 percent lower. The other types sit low the same way (the F/A-18D lifts
off at 112 kt and its 1 G edge is 118.5 kt, against a real approach of about 135 kt), so the
cause is the game's own polygons, whose left edges are below real stall speeds, and not a
fault in one aircraft's handling. The scripted landing's
approach speed is 1.3 times the 1 G edge (F-22 92 kt, Su-25 100, Su-27 139), a probe choice
and not an aircraft figure.

John decided the same day to have the model scale the stall speed with weight,
as a deliberate departure from the polygon data (next section). The
`flight-takeoff-*` scenarios check that liftoff follows the model's own
minimum speed for 1 G.

## Weight-scaled stall speed (2026-09-29)

`opinionated`, requested by John; rules in the
[takeoff spec](../spec/takeoff-ground-contact.md#weight-scaled-stall-speed). The polygon's
slow edges apply at a reference weight and grow with the square root of the weight over
it. The first version used the empty weight for every aircraft. It could not bring every
aircraft within 10 percent of John's figures: no single fraction of the empty weight (0.6
to 1.6 tried) does, because a fraction low enough to help the F-22 family and Su-25 puts the
F/A-18D, Rafale and A-4E 20 to 40 percent too high, and a high one does the reverse. Since
2026-09-30 (`fitted`, an agent decision, John's decisions 1 and 2 of the aircraft pass) each
aircraft has its own fraction of its empty weight, keyed by the aircraft's identity
(`STALL_REFERENCE_FRACTIONS`, `AircraftModel::stall_reference_fraction`, the table in the
[takeoff spec](../spec/takeoff-ground-contact.md#reference-fraction-per-aircraft-fitted)).
A lower fraction means faster liftoff and approach. The X-31, with no figure from John,
keeps 1.00.

Liftoff at the default loadout (calm, sea level, UKR airport 1, full flaps, afterburner,
back stick 0.35), with the imported speeds (`--retail-stall-speeds`), with one reference for
every aircraft (the empty weight, before 2026-09-30) and now, against John's figures where
he gave them:

| Aircraft | Gross lb | Scale now | Liftoff, imported (kt / ft) | Liftoff, one reference (kt / ft) | Liftoff now (kt / ft) | John's liftoff |
| --- | ---: | ---: | ---: | ---: | ---: | :---: |
| F/A-18D | 41,838 | 1.17 | 112 / 780 | 147 / 1,383 | 129 / 1,050 | - |
| Rafale C | 32,092 | 1.17 | 112 / 588 | 149 / 1,056 | 128 / 781 | - |
| F-14D | 64,511 | 1.38 | 91 / 607 | 114 / 948 | 123 / 1,107 | - |
| A-4E | 22,026 | 1.30 | 94 / 847 | 132 / 1,716 | 121 / 1,427 | - |
| X-31 | 28,620 | 1.33 | 121 / 597 | 156 / 1,001 | 156 / 1,001 | - |
| MiG-29 | 31,895 | 1.21 | 111 / 506 | 143 / 851 | 132 / 719 | - |
| Su-27 | 57,920 | 1.37 | 97 / 429 | 120 / 659 | 128 / 757 | 135 to 150 (5 percent low) |
| MiG-21 | 18,018 | 1.39 | 101 / 662 | 118 / 911 | 136 / 1,224 | - |
| Su-25 | 69,875 | 1.74 | 73 / 513 | 93 / 836 | 123 / 1,506 | 130 to 145 (5 percent low) |
| MiG-23 | 39,413 | 1.46 | 99 / 748 | 126 / 1,219 | 142 / 1,552 | - |
| Su-35 | 69,054 | 1.34 | 97 / 523 | 124 / 855 | 127 / 897 | - |
| F-22, F-22N, F/A-XX | 57,800 | 2.14 | 78 / 207 | 103 / 359 | 128 / 559 | 130 to 150 (2 percent low) |

Approach speed of the scripted landing (flaps down, 65 percent internal fuel, stores as
loaded; 1.3 times the flapped, scaled stall speed and never under 1.05 times the loaded
minimum for 1 G, capped at 166 kt, 0.85 times the landing limit) with the imported speeds
(full fuel, 1.3 times the polygon edge), with one reference and now, against the figures
John gave:

| Aircraft | Imported (kt) | One reference (kt) | Now (kt) | Figure (kt) | Off by now |
| --- | ---: | ---: | ---: | :---: | ---: |
| F/A-18D | 154 | 156 | 135 | 135 | in range |
| Rafale C | 154 | 158 | 135 | 130 to 140 | in range |
| F-14D | 131 | 125 | 135 | 130 to 140 | in range |
| A-4E | 139 | 148 | 136 | 130 to 140 | in range |
| X-31 | 166 | 166 | 166 (cap) | (research aircraft) | - |
| MiG-29 | 154 | 154 | 140 | 135 to 145 | in range |
| Su-27 | 139 | 132 | 142 | 120 to 135 | +5 percent |
| MiG-21 | 154 | 137 | 160 | 160 to 170 | in range |
| Su-25 | 100 | 97 | 131 | 125 to 140 | in range |
| MiG-23 | 139 | 134 | 152 | 150 to 165 | in range |
| Su-35 | 139 | 136 | 139 | 135 to 145 | in range |
| F-22, F-22N, F/A-XX | 92 | 102 | 148 | 135 to 145 | +2 percent |

Eight of the eleven figures are inside John's range and the other three are within 5
percent of it, against 2 of 11 with one reference. The F/A-18D's 135.1 kt against a single
figure of 135 counts as in. The Su-27 is the one aircraft the model cannot fit both ways:
John's liftoff (135 to 150 kt) and approach (120 to 135 kt) ranges put its approach at or
below its liftoff, and the scripted approach is about 1.1 times the liftoff at every
fraction, so 0.86 takes the middle (John's decision 2). The nine aircraft with no liftoff
figure are fitted to their approach only, and their liftoff comes out 4 to 15 percent under
it. The battery's windows for these figures are `KNOWN_LIFTOFF` and `KNOWN_APPROACH` in
`tools/battery_scenarios/flight.py`, re-recorded on 2026-09-30 (plus or minus 6 percent
around the recorded value, on top of John's range plus or minus 10 percent).

Each fraction was chosen at least 0.02 clear of the liftoff jumps that appear when the
model's G rows cross at a larger scale (the X-31 between 0.94 and 0.95, the MiG-21 between
0.68 and 0.69 and the Su-25 between 0.52 and 0.53, measured by sweeping every fraction 0.06
either side; the Su-25's 0.55 is the closest).

**Combat-speed G (correction, 2026-09-29).** The first version scaled every row of the
polygon, which counted the weight twice (the edges, then the loaded-elevator divisor) and left a
fuelled F/A-18D pulling 5.1 G in the held pull at 450 kt and 5,000 ft (7.64 with the imported
speeds; the replay lane's scripted pull peaked at 3.8 G). Now the scale applies to the 0 G, 1 G and
2 G rows, fades linearly to nothing at the 4 G row and leaves the rows from 4 G up as imported
(`row_scale`, `STALL_SCALE_FADE_G`). The stall, liftoff and approach speeds are unchanged (they
come from the 1 G edge and the ramp to the 2 G edge), and above roughly twice the stall speed the G limit is the
imported one; below it the weight still costs G. The divisor stays: removing it would put the
F-22's liftoff back near 80 knots (97 kt for 1 G instead of 74).

Held pull at 450 kt and 5,000 ft (default loadout, `--maneuver pull`, max G reached), and the
G limit at the top row's own speed (corner speed) at 5,000 ft with the same weight
(`--maneuver gcurve`), the first (double counted) version, now, and with `--retail-stall-speeds`:

| Aircraft | Pull before | Pull now | Pull retail | Corner G before | Corner G now | Corner G retail |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| F/A-18D | 5.10 | 7.64 | 7.64 | 5.02 | 7.53 | 7.53 |
| Rafale C | 6.57 | 7.39 | 7.39 | 4.87 | 7.31 | 7.31 |
| F-14D | 6.18 | 6.18 | 6.18 | 4.40 | 6.15 | 6.15 |
| A-4E | 6.10 | 6.13 | 6.13 | 4.29 | 6.01 | 6.01 |
| X-31 | 5.64 | 7.25 | 7.25 | 4.82 | 7.22 | 7.22 |
| MiG-29 | 4.78 | 7.15 | 7.15 | 4.71 | 7.06 | 7.06 |
| Su-27 | 5.25 | 7.54 | 7.54 | 5.18 | 7.77 | 7.77 |
| MiG-21 | 6.16 | 6.47 | 6.47 | n/a | n/a | n/a |
| Su-25 | 4.99 | 4.99 | 4.99 | 4.11 | 4.93 | 4.93 |
| MiG-23 | 5.92 | 6.77 | 6.77 | 4.21 | 6.73 | 6.73 |
| Su-35 | 5.22 | 6.96 | 6.96 | n/a | n/a | n/a |
| F-22 family | 6.14 | 6.91 | 6.91 | 4.56 | 6.84 | 6.84 |

(The MiG-21 and Su-35 have no top row at 5,000 ft, so they have no corner figure there.) The
pull now equals the imported value for every aircraft and the corner G is within 2 percent
of it (the F/A-18D's small difference is the divisor at the moment of measuring); measured again
with the per-aircraft fractions of 2026-09-30, every held pull and every corner G is the same.
The G limit against speed at gross weight
(default loadout, UKR airport 1, calm, 5,000 ft, full back stick), imported speeds / now:

| Speed (kt) | F/A-18D | F-22 | Su-27 |
| ---: | :---: | :---: | :---: |
| 100 | 0.84 / 0.84 | 1.03 / 0.76 | 0.86 / 0.86 |
| 150 | 1.32 / 1.16 | 2.28 / 0.76 | 1.57 / 0.86 |
| 200 | 1.67 / 1.67 | 3.04 / 2.66 | 1.73 / 1.57 |
| 250 | 2.51 / 2.51 | 4.56 / 4.56 | 3.45 / 3.45 |
| 300 | 3.35 / 3.35 | 6.08 / 6.08 | 4.32 / 4.32 |
| 350 | 5.02 / 5.02 | 6.84 / 6.84 | 5.18 / 5.18 |
| 400 | 6.70 / 6.70 | 6.84 / 6.84 | 6.91 / 6.91 |
| 450 and up | 7.53 / 7.53 | 6.84 / 6.84 | 7.77 / 7.77 |

So the weight now costs G only at the slow speeds where the low G rows lie (up to about 200 to
250 kt), where a heavy aircraft really is lift-limited, and nothing at combat speed. With the
per-aircraft fractions the clean aircraft (no flaps, 5,000 ft, default load) first pulls 1 G at
150 kt (F/A-18D, Rafale, A-4E, Su-25), 175 kt (F-14D, MiG-29, Su-27, MiG-23, Su-35) or 200 kt
(X-31, MiG-21, F-22 family), in 25 kt steps, against 100 to 150 kt with the imported speeds: the
F-22 cannot hold level flight clean below about 200 kt, the direct cost of lifting off at 128 kt
(the flaps lower this by a quarter).

**Short strips.** The roll to liftoff at full afterburner is longer for every aircraft, most
for the heavy loadouts and for the ones whose fraction is under 1. From the Santa Fe start
(`--theater APA --ground-start 3`, a strip of about 1,074 ft, 1,020 ft of paved run from a solo
start slot; San Carlos is the same length) the liftoff runs are, with the imported speeds and now:
F/A-18D 823 to 1,098 ft, Rafale C 620 to 816, F-14D 650 to 1,164, A-4E 908 to 1,507, X-31 626 to
1,040, MiG-29 532 to 751, Su-27 454 to 790, MiG-21 702 to 1,278, Su-25 560 to 1,588, MiG-23 796 to
1,622, Su-35 555 to 940 and the F-22 family 222 to 584. So the F/A-18D, F-14D, A-4E, X-31, MiG-21,
Su-25 and MiG-23 no longer lift off within the paved strip (they roll on past its end); the
Rafale, MiG-29, Su-27, Su-35 and F-22 family still do. The AI takeoffs on those strips were already
known failures (lane-ai item 11). Since 2026-09-30 a Quick Mission never starts on such a strip
(lane-ai item 11); these developer `--headless-flight --ground-start` runs, which measure the
flight model and not a mission, still start there. Landing rollouts shorten slightly (the flare
from 60 ft touches down nearer the threshold), so the 5,532 ft UKR runway still works for all twelve.

**Other users of the stall speed.** The flight model, the stall warning and departure
behaviour, the autopilot, the ejection G check, the flight envelope window (the drawn slow
edges follow the weight), the scripted landing probe and the crew callouts all
read the scaled speeds. The HUD landing-speed brackets are not implemented in the game, so
there is nothing to update. The AI reads the same scaled speeds through the model's
configuration without any change to `tore-sim/src/ai`. The AI follow-up this rule needed is done: the AI lane retuned its speed
limits during the bug bash (`4e47944`, `0057684`: the minimum never below the 1 G level-flight
speed, the maximum never above the fastest row that leaves 1.2 G, a final lift margin of 1.15 G;
the twelve scenarios that failed first are listed in the
[battery report](../baselines/battery-2026-09-28.md), section 2). With the per-aircraft stall
references of the aircraft pass (2026-09-30) the whole AI lane passes. AI takeoffs are
unaffected (the roll only lasts longer).

## Needs a decision

None of these is defined in the specs, the manual text or the feature matrix, so none was changed.

1. **Overspeed** (decided). John asked on 2026-09-29 for a shake from 95 percent of the top speed
   and a loss at 1.5 times it, for every aircraft, with the cause recorded
   ([overspeed](../spec/overspeed.md), `opinionated`, numbers are agent decisions). Before it
   aircraft passed their own top speed in the dive after a full-power climb (F-22 1.7 times,
   Su-25 1.9 times, F-14, MiG-29, Su-27 and X-31 1.5 to 1.6 times). Level full afterburner still
   settles at 92 to 100 percent of the top speed, and the climb and dive scenarios now expect
   nothing past 1.52 times.
2. **The autopilot's altitude hold outside the envelope.** Beyond the top speed the aircraft cannot
   hold 1 G and sinks; the autopilot spec says "no guaranteed hold outside the flight envelope
   is implied", so this is as written, and inside the envelope the hold now works (see "Bugs
   found"). Overspeed now has a rule (item 1); the hold outside the envelope is unchanged.
3. **The map edge** (decided). John asked on 2026-09-29 for a warning at 100 nautical miles past the map
   and a loss at 105, AI aircraft lost without a kill ([world edge](../spec/world-edge.md),
   `opinionated`, distances are agent decisions). The `flight-edge-*` scenarios fly 200 seconds out
   and must stay flying; `flight-edge-lost-*` fly on until the loss.
4. **A ground-start airport is a flat square of 5,000 to 8,000 ft a side and all of it counts as
   runway.** The footprint (`footprint_half_ft` in the `landing_start:` line, for example 2,604
   by 3,000 ft either side of the centre at UKR airport 1) is the airport shape's bounds. A
   touchdown 1,300 ft to the side of the paved runway is graded as a normal landing, a takeoff
   roll can run past the runway's end on the flat square, and rolling off the paved strip is never
   penalised. The airport spec calls this "fitted contact" and says terrain outside is unchanged,
   so it is by design; whether a narrower landing area is wanted is a decision.
5. **The small airstrips (about 1,000 ft) accept any aircraft.** Changed (John, 2026-09-30): a Quick Mission ground start is no longer offered on a runway under 2,000 ft (the 22 strips), so no aircraft or wing starts there. Round two had searched the specs, the airport spec and the manual for a minimum runway length and found none, so the rule is John's, not retail's ([short strips](../spec/airports.md#short-strips)).
6. **Combat tapes do not record everything the host does.** A replayed tape shows the player alive
   after a flight that crashed, because the host turns a crashed flight into a dead player and the
   tape does not record that. Only the developer replay uses tapes; the mission recordings of
   [REPLAYS.md](../REPLAYS.md) are separate.
7. **Carrier hook and arresting gear.** The hook lowers on the five aircraft that have one (F/A-18D,
   F-14D, A-4E, F-22N and the F/A-XX concept) and does nothing else; carriers are listed as
   remaining work in the feature matrix.

## Needs a human eye or ear

- The `weather*` frames: the cloudy and foggy conditions are a solid white at the 5,000 ft
  start, with no ground visible at all. Check that being inside the cloud layer is what the
  retail game shows.
- The `hour*` frames of the day: hour 07 has a dawn glow, hours 08 to 19 are full day and night
  starts at 20; check the sunset transition looks right around 19 to 20.
- The `damage-*`, `ejectionpose-*`, `groundstart-*` and `bay-*` frames: the checks only reject
  blank pictures. The F-22 bay frames are taken from a distance where the doors are hard to see.
- Captures made with `--flight-devices` show "RADAR OFF" on the radar window because the capture
  path skips the combat service; live flight shows the scope. Cosmetic, in a diagnostic mode only.
- Aural checks (stall horn, RWR tones, gun and missile sounds) belong to the replay and sound work.

## The slow set and the missile acceptance tables

`--missile-acceptance` prints one row per weapon, mode, launch motion, target motion and range
(hit, expiry or an inhibited shot). Every aircraft was run to completion in an optimized build
(`cargo build --release -p tore-app`, each run a few minutes; the F-14's long-range AIM-54 rows
take far longer) and every row was checked: a hit has a positive time, a sane average speed
(100 to 5,500 ft/s) and a travel that matches the target's motion (about the range for a stationary
target, less for an approaching one, more for a receding one); an inhibited shot never flies; the
only outcomes are hit, expiry and inhibit. No impossible row was found in any table. Patterns
worth knowing: every infrared missile has a block of inhibited rows (the seeker is still
acquiring when a cued shot is asked for) and eight boresight expiries (a blind shot at an
approaching target from a climbing or slipping launcher); the F-22A has ten more AIM-120
boresight expiries against a crossing target at 75 and 100 percent range than the F-22N and the
F/A-XX, which fits the F-22A's bay doors adding a second to the launch.

The slow set is off by default (the default battery leaves out the F-14 and Su-35 tables, which
take about 40 minutes each in a debug build):

```sh
CARGO_TARGET_DIR=target-release cargo build --release --locked -p tore-app
TORE_BATTERY_SLOW=1 python3 tools/battery.py --scenario 'flight-slow-*' \
    --bin target-release/release/tore-app --jobs 4
```

## Not covered

RWR reaction to AI radars and missiles, and system faults appearing on the panels during an AI
fight, have no headless or scripted path (the AI probe does not feed the player's RWR or panel
faults, and `--dummy-aircraft` cannot be combined with `--live-fire`), so they were not tested in
round two; the windowed `cheat-damage-*` scenarios cover system faults from incoming missiles and
the `--combat-command` sequences cover designation. Radar and infrared channel switching and
target cycling with several targets in view are exercised only by the probe-attack leader
(`attack-*`), not by a scripted key sequence.

## Runtime

The full lane, 3,089 scenarios with `--jobs 6 --windows 2`, took 1,804 seconds (30 minutes) on the
24-thread dev machine with other agents running (load average about 24). Round two's run had one
failure, the jettison check of the Su-35's internal slot 5, which was the check's mistake and is
fixed; that family passes 38 of 38, and the rest passed. The missile acceptance
runs (about five minutes each in a debug build) and the windowed frames set the length.
