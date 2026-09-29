# Battery lane: AI

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

This lane flies AI fights, wings and airfield traffic headlessly, hundreds at a
time, and looks for things that cannot happen: aircraft alive at zero hit
points, under the ground or outside the world, dead aircraft firing, ammunition
appearing, labels that disagree with the state, stuck or spinning aircraft,
orders that are never answered, and runs that are not repeatable. It does not
judge whether the AI flies well; the behaviour it checks against is the one in
[the AI spec](../spec/ai.md), [awareness](../spec/ai-awareness.md),
[airfield sequences](../spec/ai-airfield.md) and the
[AI baselines](../baselines/ai-research.md).

## How to run it

```sh
cargo build --locked -p tore-app
python3 tools/battery.py --lane ai --jobs 6 --tag ai
python3 tools/battery.py --scenario 'ai-regress-*'
```

About 446 scenarios, each one `--ai-probe-ticks` run (plus the roster probe and
the 1,008-case probe matrix). With the other lanes running on the same Ryzen 9
7900X, the whole lane took 26 minutes at `--jobs 5` (final run 2026-09-28:
444 of 445 passed, the one failure is the known failure below). The probe
matrix alone is about 16 minutes and the six 30-minute runs 1 to 5 minutes
each; without them the lane takes about 12 minutes. A 15 v 15 probe holds
about 400 MB and its tick rate rises as aircraft are lost, so 30 simulated
minutes cost about 4 minutes.

## What it covers

| Group | Scenarios | What varies |
| --- | --- | --- |
| Fight sizes | `ai-fight-*` | 1 v 1 up to 15 v 15, and lopsided (1 v 15, 15 v 1, 5 v 10, 3 v 12, 6 v 2...), with and without the scripted leader attacking |
| Every pair | `ai-pair-*` | all 14 aircraft as friendly against all 14 as enemy, two a side, skills rotated |
| Big fights | `ai-big-*` | each aircraft as the enemy in 6 v 6, researched adapter |
| Skill and geometry | `ai-skill-*` | 4 skills x head/side/rear x legacy/researched, 1 v 1 and 3 v 3 |
| Missions | `ai-mission-*` | free, CAP, intercept, escort, self-defense, hold at 1 v 1, 4 v 4, 10 v 10 |
| Separations | `ai-separation-*` | 1 to 100 nm |
| Guns | `ai-guns-*` | guns-only player and AI |
| Faults and threats | `ai-fault-00..44`, `ai-threat-*` | every system fault index, hit, gun and AAA threats, 64 hits in a row |
| Wing orders | `ai-order-*` | bug out, land at selected airport, attack on contact, engage my target, wings of 2 and 5 |
| Ground starts | `ai-ground-*` | takeoff with wings of 1 to 5, landing and bug-out orders, a fight after takeoff, an idle wing |
| Long runs | `ai-long-*` | 30 simulated minutes (216,000 ticks) of 1 v 1, 5 v 5, 15 v 15, hold, guns and a ground landing |
| Recordings | `ai-record-*` | `--record-mission --verify-render` must say PASS |
| Determinism | `ai-determinism-*` | the same arguments run twice on fresh profile copies give identical output; `ai-determinism-recordings` records a 5 v 5 twice and `--recording-diff` must say they match |
| Regressions | `ai-regress-*` | one per defect fixed below, checked strictly |
| Acceptance probes | `ai-roster-probe`, `ai-probe-matrix` | the fixed roster and 1,008-encounter probes |

## What is checked

Every tick of every probe, `crates/tore-app/src/probe_invariants.rs` reads the
mission (it never writes to it, so the probe's output and checksum are
unchanged) and prints an `AI probe anomaly:` line the first time an aircraft:

- is alive with zero hit points, or alive and more than 20 ft under the ground;
- is outside the terrain, faster than about 2,400 kt or above 120,000 ft;
- is labelled Destroyed while alive, or anything else two ticks after dying;
- is a settled (grounded or burst) wreck that still moves;
- fires, spends ammunition or releases decoys after it died;
- gains ammunition or decoys;
- hangs in the air below 30 kt for 10 s, is frozen for 2 s, or turns faster
  than 40 degrees a second for 30 s;
- flips its activity 30 times, or its pitch stick 60 times, in 3 seconds;
- dies within 60 ft of another aircraft (a mid-air collision).

It closes with `AI probe invariants:` and the peak heading rate, bank rate,
speed and altitude. The lane's `check=` functions (`tools/battery_scenarios/ai.py`,
tested by `tools/test_battery_ai.py`) then fail a run on any anomaly, and also
when: the final actor list disagrees with the destroyed and ejection events;
the debrief credits the player with fewer kills than the combat counted; a free
mission's objective count differs from the enemies lost; any launch is dropped;
an aircraft flew into the ground with no weapon hit, ejection or fault; a
ground start leaves a hazard open, a wingman never takes off or (for the pair)
never lands; or the same radio line repeats three times in five seconds.

## Bugs found and fixed

| Symptom | Cause | Fix |
| --- | --- | --- |
| A mission stopped with "invalid AI input: decoy percentages exceed 100" when an AI aircraft dropped a flare at certain missiles (Su-25 or F-22N against MiG-21s) | The imported chaff/flare chance of some missiles is above 100; the AI decoy roll rejected it, and the error ended the mission. The player's own dispensers already treated it as 100 | `d7143e3`: the AI path clamps to 100 too |
| An enemy wingman left a fight for good, following its damaged leader in to land, and kept landing after the leader ejected | The wing-abort rule counted a missing (dead) leader as "still landing" | `21a1d5d`: a destroyed leader is neither landing nor on the ground, so the wingman returns to free flight |
| Undamaged AI aircraft flew into the ground at full G while still turning toward a target (about one run in forty) | The input-only controller kept asking for up to 75 degrees of bank while the terrain floor asked for a climb, so almost no lift went into climbing | `c147085`: below the terrain floor the wings come level (fitted) |
| An undamaged AI fighter eased into a rising hillside at 2 G | The pitch loop closed the terrain floor's demand over 3 seconds | `2a4cc9b`: below the floor the pitch error closes over 1 second (fitted) |
| A wingman under missile attack switched between missile defense and formation flying on every tick for seconds (68 stick reversals in 2 s) and did neither | A seen missile's "incoming" judgment (closest approach within 1,000 ft) had no memory; the aircraft's own reaction moved the answer across the limit each tick | `8c46b58`: once incoming, a missile still in sight stays incoming for the existing 2-second grace (fitted). The RWR shares this and no longer flickers |

Supporting commit: `17144d1` adds the per-tick checks and the lane's scenarios.

### Golden fingerprints

The terrain and incoming-missile fixes change behaviour, so three golden
fingerprints (compared on Apple silicon only) will fail there until their
values are updated from the macOS CI log: `ai/mission-engagement`,
`combat/guided-missiles` and `combat/player-countermeasures`. On Linux the new
totals are `0xd84f7b64adbdab50`, `0xefb2604ac7d9952f` and `0x24800010e49363c6`
(macOS values may differ in the last bits). They were not edited here.

## Found, not fixed

**AI aircraft collide in the air.** With the collision check added, 8 of the
72 fight and mission scenarios had a mid-air collision between two AI
aircraft of the same side, for example:

- Two AI wing leaders (Enemy 2-1 and Enemy 3-1 in `ai-fight-1v15-noattack`),
  both searching with no target, flew straight on headings 11 degrees apart
  and closed at 145 ft/s until they hit at 62 s. Nothing steers an aircraft
  that is not flying formation away from other traffic; the spec does not
  say it should (see "Needs a decision").
- Two wingmen breaking out of formation (Friendly 2-2 and 2-3 in
  `ai-mission-hold-10v10`) each picked an escape heading on the same side:
  the fitted escape score prefers the wing's formation side by 0.05 ft per
  degree, and both share that side, so they turned together, flew about 50 ft
  apart for 2.5 s and collided at 19.8 s. The formation spec (opinionated,
  John 2026-09-18) wants departures to account for neighbours, but its
  clearances are documented as margins, not guarantees. A likely cause: the
  escape score is the closest approach over the next 8 seconds counted from
  now, so for two aircraft already 50 ft apart every heading that moves away
  scores the same 50 ft, and the straight-ahead candidate wins on its smaller
  offset penalty. Scoring from 1 second ahead was tried and made the
  synthetic diving-reversal test's minimum separation worse (240 ft against
  its 250 ft requirement), so the fitted rule was left for a decision; splitting
  a converging pair to opposite sides by position is another option.

The lane reports these as `mid-air collision` anomalies but does not fail on
them (see `KNOWN_ANOMALIES` in the scenario file).

## Known failures

- `ai-big-a4e-vs-f22n-researched`: an F-22N searching at 975 kt at 3,000 ft
  flies into rising ground. At that speed the B44 look-ahead (terrain 1,000 ft
  ahead) is 0.6 s of warning, and the loaded G limit near the top of its
  envelope is 2.2 G. See "Needs a decision". Other runs can show the same
  class; the failing scenario can change with any behaviour change because the
  fights are chaotic.
- Activity flapping and pitch-stick oscillation at a weapon's envelope edge,
  and mid-air collisions, are reported but allowed (see above and below);
  regression scenarios check strictly.

## Needs a decision

Behaviour the specs do not define, with the evidence. None of these were changed.

1. **Dithering at a missile's employment-zone edge.** A fighter whose missile
   is at the edge of its zone flips every one to five ticks between its chosen
   maneuver (for example a 45 degree dive) and gun lead tracking, for up to
   about 3 seconds (`ai-fight-8v8-default`, Friendly 2-3 at 20.4 to 21.5 s;
   `ai-pair-a4e-vs-mig29`). The zone test uses the body attitude, which the
   previous tick's opposite stick input just moved. Hysteresis or a minimum
   commitment time would stop it; the spec has neither.
2. **Notch side on a head-on shot.** The notch turns 90 degrees toward the
   smaller turn with a tie break; with the radar source dead ahead the side
   flipped after one second (heading 288, then 108), wasting the roll-in.
3. **Missile defense that dives and turns at once.** The fitted pitch law
   pushes negative G to reach a 20 degree dive while banked, which turns the
   aircraft the other way: a wingman asked to turn 90 degrees achieved 2 degrees
   in 3.5 s and was hit. Steep and inverted tracking is a documented
   approximation of the input-only controller.
4. **Leaderless wingmen after a fight.** When a wing's leader dies, the
   survivors have no leader succession or route. Once the fight ends they fly
   straight ("Searching", no target) for as long as the run lasts; in 30
   minutes they leave the map by up to 100 nm (`ai-long-5v5`). The retail
   game does pass leadership (the "You're the wingleader now" call in
   [radio chatter](../spec/radio-chatter.md)), but the situations that pass it
   are recorded there as unknown.
5. **Aircraft with no route leave the map.** In a hold mission the enemies hold
   their heading (B48) and fly off the terrain after about 16 minutes
   (`ai-long-hold-4v4`). Nothing defines a map edge for AI.
6. **Landing over hills.** A wing ordered to land at Simferopol flies the
   approach gates south of the field over rising ground. The fitted terrain
   correction holds the current heading, so an aircraft that just passed a gate
   flies on away from the field for about a minute; each approach takes about
   400 s, taxiing to parking another 330 s, and a wing of four needs more than
   30 minutes. Meanwhile a ground-started wingman still waiting cannot take off,
   because the runway gate stays closed while anyone flies the gates.
7. **Fast low flight and the terrain look-ahead.** B44 looks 1,000 ft ahead.
   At supersonic speed near the ground that is under a second of warning (see
   Known failures).
8. **A decoyed missile still kills.** A missile decoyed by chaff coasts on and
   can still hit an aircraft that flies straight into it. The shot table then
   says "spoofed" while the kill is credited (2 v 2, shot 1 at 10.7 s), and
   the debrief shows 0 air-to-air hits of 1 launch next to 1 kill. That
   follows the [debrief spec](../spec/debrief.md) ("a missile resolves once",
   retail), so it is not a bug by the specs, but a player may read it as one.
9. **Deconfliction outside formation.** Should AI aircraft that are not in a
   formation (wing leaders, singletons, leaderless wingmen) avoid other
   traffic? Today nothing does, and two of them on converging straight courses
   collide (see "Found, not fixed").
10. **"Land at selected airport" for an aircraft still on the ground.** A
    wingman still waiting to take off counts in the reply ("Land at
    Simferopol: 3 landing"), then takes off once the runway frees (645 s
    later, behind the others' long approaches) only to fly the marshal and
    approach and land again at 1,484 s. Bug out is ignored on the ground by
    spec; the land order's spec (opinionated, John 2026-09-23) does not say.
11. **The in-flight activity line after a death.** A "Destroyed" change inside
   the 2-second message interval is dropped rather than queued, so the bar can
   keep showing "Friendly 1-2: Defending" for an aircraft that has just died.

## Needs a human eye or ear

- The in-flight AI activity line and target-window activity during the
  envelope-edge dithering above, and after deaths.
- Radio chatter volume in 10 v 10 and larger fights (the probe prints only the
  first 40 lines heard; none repeated, none spoken by a dead pilot).
- How the formation and missile-defense changes look in the cockpit view.
- Every aircraft that lands at Simferopol and taxis clear leaves the pavement
  for about 25 seconds at the same runway-exit corner (x 1,106,135, z 593,076),
  creeping at 3 kt while it turns (the fitted creep turn that replaces the
  retail stopped pivot), then rejoins the taxiway. The probe reports and then
  clears an "off landable surface" hazard there; worth a look in the viewer.

## What was not run

Windowed captures (this lane is headless only), other theaters than Ukraine,
Windows and macOS, and a retail comparison.
