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

The whole lane is about 2,800 scenarios, 380 of them windowed (through `tools/agent-run.sh`).
The headless ones take about a second each; the missile acceptance runs take up to five minutes
each and the windowed ones five to eight seconds. See "Runtime" at the end for the measured time.

## What is covered

| Family | Scenarios | What it checks |
| --- | --- | --- |
| `takeoff-*` | 772 | Ground start and takeoff for all 14 aircraft from the first and middle airport of every base theater, the Hornet from every airport of every base theater and the first airport of every imported variant: reaches 100 ft, no crash, sane liftoff speed and time, and a default load no heavier than the aircraft's maximum takeoff weight. |
| `land-*` | 676 | The scripted approach and landing (see below): the Hornet at every airport of every base theater, every other aircraft at the first airport of every base theater, the Hornet at the first airport of every variant, all 14 with gear up (must crash for the gear), off the runway (must crash for not landing on a runway) and with no flare, and all 14 in five different winds. On runways of 5,500 ft or more: touches down, stops, no crash, no unsafe touchdown, does not leave the runway surface. |
| `level`, `pull`, `loop`, `roll`, `stall`, `spin`, `bank-left`, `bank-right` | 336 | Every manoeuvre in every aircraft on the default, `--legacy-flight` and `--researched-flight` adapters: no non-finite value, fuel never rises, no energy from nothing, load, speed, altitude and rates within limits, no veil without G. |
| `spinrecover-*`, `stallrecover-*` | 28 | The manual's spin and stall recovery procedures recover every aircraft that can enter a spin or stall (X-31 and the F-22 family cannot spin: their data says so). |
| `climb-*`, `sprint-*` | 28 | How far past its own envelope an aircraft goes (see "Needs a decision"), and that full-afterburner level flight settles near the top speed. |
| `fault*` | 73 | Every system fault 0..44 on three aircraft in a pull, all 45 at once, and one after another, in every aircraft. |
| `combatsmoke-*`, `combatevidence-*`, `missileacceptance-*` | 40 | The headless combat smoke (default slots, five damage classes, jettison, radar power, incoming missiles, jammer), the same smoke with per-slot combat tapes written and replayed to the identical state, and the missile reach probes (the F-14 and Su-35 tables take about 40 minutes each in a debug build and are run by hand). |
| `livefire-*`, `cheat-unlimited-ammo-*`, `cheat-damage-*`, `countermeasures-*` | about 90 | Windowed: fire every weapon slot of every aircraft (ammunition never negative or over capacity, drops by exactly what was fired, other stations untouched, surface weapons refuse the practice aircraft), Unlimited ammo, the three Damage modes, and chaff and flare counts against capacity. |
| `cheat-*` | 70 | Extra G reaches about 9 G, No redout or blackout, No spins, No crashes and Unlimited fuel on every aircraft. |
| `devices-*` | 14 | Gear, flaps, airbrake and hook stay within 0..1, never move against their command and take the aircraft's own deployment time; an aircraft with no hook does not lower one. |
| `eject-*`, `ejectionpose-*` | 70 | The seat and parachute at 5,000 ft and 250 ft in every aircraft; windowed frames of the three ejection poses. |
| `autopilot-*`, `waypoint-*`, `fuelout-*` | 42 | Heading and altitude hold from a 25 degree bank, waypoint steering, and running out of fuel. |
| `climbout-*` | 30 | The scripted leader's takeoff, gear and flaps up, and cruise, for every aircraft alone and for a wing of five from every base theater. |
| `edge-*`, `terrain-*` | 96 | Flying out over each of the four map edges at 20,000 ft, and a spin or roll at 90 ft over every theater. |
| `weather*-*`, `hour*-*`, `groundstart-*`, `damage-*`, `bay-*` | about 250 | Windowed frames of every weather condition in every base theater, every hour of the day in three theaters, every aircraft at a ground start, and the damage and F-22 bay fixtures, each checked for a blank or flat frame. |
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
| A combat tape replayed with `--replay-combat` drifted its smoke differently from the live run (`TORE_COMBAT_EVIDENCE` smoke: "serialized live-fire replay diverged"). | The host sets the mission wind on the smoke and countermeasures before every step, but a tape does not record it, so the replay drifted them with no wind. The smoke also compared a live state without airfields to a replay that added them, and kept stepping a crashed flight. | The replay is given the theater's wind (`combat_tape.rs`); the smoke replays without airfields and starts its manual-command tape from a fresh flight. All fourteen aircraft roundtrip their tapes and are in the battery as `combatevidence-*` (commit below). |
| The RCS instrument window drew "NO EXPOSURE DATA" over the 270 bearing label. | Message placed at the left edge of the window. | Moved below the crosshair. The menus lane fixed the same line the same way; that version is the one merged. |

No game defect was found in takeoff, landing, the manoeuvres, spin and stall recovery, faults,
ejection, devices, autopilot, fuel, weapons accounting, countermeasures, cheats or the terrain
checks. One aircraft problem outside this lane was seen and passed on: in a default two-aircraft
wing takeoff with the F-22 as the player, the AI wingman flew into the ground at 810 knots and
14 ft (`--ai-probe-ticks 9000 --ground-start 3 --maneuver takeoff --aircraft f22`, seen before
the AI lane's fixes were merged; the AI lane owns it).

## Needs a decision

None of these is defined in the specs, the manual text or the feature matrix, so none was changed.

1. **Nothing stops an aircraft leaving its own envelope.** In a full-power climb (`flight-climb-*`)
   the aircraft carry on past the ceiling of their own 1 G envelope: X-31 to 66,700 ft against
   41,000 ft, MiG-29 86,000 against 65,000, Su-27 72,100 against 61,000, F-14 66,100 against
   56,000, F-22 71,200 against 65,000. In the dive after it they pass their own top speed: F-22
   1,348 knots (1.7 times), Su-25 1.9 times, F-14, MiG-29, Su-27 and X-31 1.5 to 1.6 times. Level
   flight in full afterburner behaves: it settles at 92 to 96 percent of the top speed. The retail
   manual describes the envelope as the speeds and altitudes where G is available; whether the
   original also blocked flight outside it is not established. The scenarios record today's
   behaviour with a margin (1.7 times the ceiling, 2.0 times the top speed) so a change for the
   worse is caught.
2. **A loaded aircraft near the top of its envelope cannot hold 1 G and sinks.** At full
   afterburner in level flight with the autopilot holding 5,000 ft, the F-22, MiG-21 and X-31
   reach the fast edge of their envelope (only the 1 G row is left there), lose lift because the
   loaded elevator factor divides that row below 1 G, and sink into the ground with the autopilot
   still engaged (the X-31 from about 1,350 knots, the F-22 from about 1,080). This follows from
   the PT loaded-elevator divisor in the flight code, which the flight-model guide does not describe,
   so it may be right; it is worth a look because a pure speed gain turns into a crash.
3. **The map has no edge.** Aircraft fly out over all four sides of every theater, with the terrain
   height clamped to the edge value, no boundary message and no turn-back, for at least 100,000
   feet past the edge (`flight-edge-*`). Neither the specs nor the manual say what should happen.
4. **A ground-start airport is a flat square of 5,000 to 8,000 ft a side and all of it counts as
   runway.** The footprint (`footprint_half_ft` in the `landing_start:` line, for example 2,604
   by 3,000 ft either side of the centre at UKR airport 1) is the airport shape's bounds. A
   touchdown 1,300 ft to the side of the paved runway is graded as a normal landing, a takeoff
   roll can run past the runway's end on the flat square, and rolling off the paved strip is never
   penalised. The airport spec calls this "fitted contact" and says terrain outside is unchanged,
   so it is by design; whether a narrower landing area is wanted is a decision.
5. **The small airstrips (about 1,000 ft) accept any aircraft.** A ground start and takeoff work for a
   Su-25 or an F-22 there, and nothing says a fighter needs a longer runway.
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

## Runtime

RUNTIME_PLACEHOLDER
