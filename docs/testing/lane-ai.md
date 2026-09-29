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

About 584 scenarios, each one `--ai-probe-ticks` run (plus the roster probe and
the 1,008-case probe matrix). With the other lanes running on the same Ryzen 9
7900X, the whole lane took 26 minutes at `--jobs 5` (final run 2026-09-28:
444 of 445 passed, the one failure is the known failure below). The probe
matrix alone is about 16 minutes and the six 30-minute runs 1 to 5 minutes
each; without them the lane takes about 12 minutes. The 138 theater and
takeoff scenarios added in the second round take about 10 minutes more. A 15 v 15 probe holds
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
| Other theaters | `ai-theater-*` | all 16 base theaters: a 4 v 4 fight, an 8 v 8 without the leader attacking, wing-of-3 takeoffs at airports 1 and 3, and a pair ordered to land at airport 1 |
| Every aircraft's takeoff | `ai-takeoff-*` | all 14 aircraft as a ground-started pair at Zaporizhzhya (UKR 1), Ras Al Khaimah (PGU 2), Chateaudun (FRA 3) and Nuchon Ni (NSK 5) |
| Long runs elsewhere | `ai-long-15v15-pgu`, `ai-long-15v15-vla` | 30 minutes of 15 v 15 in the Persian Gulf and Vladivostok |
| Random configurations | `ai-fuzz-NNNN` | seeded whole configurations from `tools/battery_scenarios/_ai_fuzz.py` (theater and layout variant, sizes, three aircraft types, skill, mission, separation, geometry, adapter, ground start, orders, threats and faults at random ticks, attack script, 1 to 5 minutes); the first 60 of 400 fixed seeds by default, `TORE_AI_FUZZ=all` for all; `python3 tools/_ai_fuzz_cmd.py SEED` prints a seed's command |
| Creator objectives | `ai-objective-*` | ten group setups (`--probe-group GROUP:CHOICE[:survive]`: free, CAP, targets, protection, self-defense, hold, required survival on either side) at four fight sizes; the debrief's targets, protected aircraft and SUCCESS or FAILURE must match the aircraft left alive ([debrief rules](../spec/debrief.md)) |
| Order drills | `ai-orders-cycle-*` | the four player wing orders cycled twelve times through a fight, in the air and after a ground start; every order must be answered |
| Damaged aircraft | `ai-damaged-*` | twelve recovery faults, each with hits or gunfire and a second fault, for 20 minutes under weapons hold |

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

| Two wingmen breaking out of formation at the same time turned into each other and collided (`ai-mission-hold-10v10`, `ai-mission-self-defense-10v10`) | Each scored its escape assuming the other would fly straight on; and for two aircraft already 50 ft apart every heading away scored the same, so straight ahead won | `b43c9b3`: a lower-ID aircraft that is itself breaking out is predicted along its chosen escape (repositioning aircraft already yield to lower IDs this way), and an aircraft already within 220 ft is scored on clearance from 2 s ahead (both fitted, in the AI spec). The formation diving-reversal test still passes |
| The in-flight activity line kept saying "Friendly 1-2: Defending" for an aircraft destroyed a moment later | Changes inside the line's 2-second interval were dropped, including "Destroyed" | `04b0657`: a later change of the aircraft the line already names is posted as soon as the interval allows |

| A wingman flipped from missile defense to its previous task and straight back when a radar warning lapsed and it could see the missile | The first visual sample cannot judge a missile's motion, so the threat vanished for one tick; and round one's sticky "was aimed at us" memory made a passing missile a stale threat again the moment it left sight | `1c5f013`: a missile warned of as aimed at the aircraft last tick counts as incoming at the switch to eyesight, and seen missiles are judged afresh (fitted) |
| A fighter at the edge of its missile's zone swapped between the missile tactic and gun tracking every few ticks for seconds (30 activity changes in 3 s) | Its own stick input moved the body-relative zone test each time; the motion ignored B42's two-second "no suitable station" retry that the weapon service already honours | `6e81213`: once no store resolves, gun tracking holds until the retry ends (spec-derived from B42, recorded in the AI spec; `ai-regress-envelope-edge-flap`) |

| Shooting down an enemy group whose own side required it to survive failed the player's mission (Destroy 5 of 5, Protect 0 of 5, FAILURE) | The debrief counted every "must survive" group as the player's friendly objective, enemy groups included | `e146788`: only the player's side becomes friendly objectives, as the debrief spec says |

Supporting commit: `17144d1` adds the per-tick checks and the lane's scenarios.

### Golden fingerprints

The terrain and incoming-missile fixes change behaviour, so three golden
fingerprints (compared on Apple silicon only) will fail there until their
values are updated from the macOS CI log: `ai/mission-engagement`,
`combat/guided-missiles` and `combat/player-countermeasures`. On Linux, after
the merge with the menus lane (whose stores fix also moves
`combat/guided-missiles`) and the third round's threat and weapon-retry fixes,
the totals are `0xfc8ffd6c7bdf65e9`, `0x614fbe4f273d1e9b` and
`0x7bdb41a533c2ec1b` (macOS values may differ in the last bits). The
formation and activity-line fixes move none. They were not
edited here.

### Recording summary churn flags (third round, 2026-09-29)

The replay lane's recording summaries flagged `ai_flipping` 89 times,
`track_lost_early` 69 times (48 on its first count) and `control_oscillation`
6 times on its recordings. Classified on the lane's own fight recordings:

- **`track_lost_early`: every one was a missile decoyed by chaff or flares**
  within 2 s of launch, an intended outcome (the decoy rule in
  [countermeasures](../spec/countermeasures.md)) already shown in the shot
  table. The summary rule was wrong; `0ba94aa` stops counting decoys.
- **`ai_flipping`: about two thirds were a dogfight's ordinary progression**
  (pursue, fire with a one-tick Attacking, defend, resume): six changes in
  5 to 10 s. The rest were real: decisions swapping every few ticks (the two
  AI fixes above). `0ba94aa` narrows the window from 10 s to 2 s, which still
  catches every real case (six changes in 0.1 to 1.6 s).
- **`control_oscillation`** came with the defense flips and mostly went with
  them.

Re-recording the replay lane's 84 recording scenarios with all fixes and the
new rule: `ai_flipping` 1 (a furball with three missiles in two seconds,
reasonable transitions), `track_lost_early` 0, `control_oscillation` 2 (one
aircraft pulling at its G limit in pursuit, both recordings of the same run).
The rule alone, on the old recordings, gives 20, 0 and 6.

Still open: a fighter whose chosen store alternates between its gun and a
missile (not "no store") can still swap maneuvers every few ticks
(`ai-pair-a4e-vs-mig29`, Enemy 1-2 at 48 s). B42 gives a retry only for "no
suitable station", so a hold here would be a new rule.

## Found, not fixed

**Aircraft spawned inside mountains (fourth round).** In a ground start at
Jixian (Vladivostok, airport 2) or Bahawalpur (South Asia, airport 2) with
the enemy 50 nm away, the enemy group starts at the chosen 5,000 ft over
ground up to 6,856 ft high and is destroyed at 0.0 s (fuzz seed 100). The
probe now reports `AI probe UNDERGROUND start`. The spec's check ("the
altitude must clear the airport ground by at least 100 feet", [Quick
Mission](../spec/quick-mission-menu.md#player-ground-start)) only covers the
airport; extending it to every airborne aircraft's own ground, or raising
those aircraft, would be a new rule, so it is item 12 under decisions. A scan
of all 16 base theaters, airports 1 to 3 and separations 5 to 100 nm found
only these two cases.

**AI aircraft outside formation collide in the air.** After the formation
fix, 18 of 86 fight, mission and big-fight scenarios still showed a mid-air
collision (6 scenarios), all between aircraft that fly no formation:

- Two AI wing leaders (Enemy 2-1 and Enemy 3-1 in `ai-fight-1v15-noattack`),
  both searching with no target, flew straight on headings 11 degrees apart
  and closed at 145 ft/s until they hit at 62 s.
- Leaderless wingmen (`ai-fight-1v5-default`): after their leader died, four
  wingmen all searched toward the same last-seen point, and two pairs collided
  there.

Nothing steers an aircraft that is not flying formation away from other
traffic, and no spec says it should (see "Needs a decision", item 9).

The lane reports these as `mid-air collision` anomalies but does not fail on
them (see `KNOWN_ANOMALIES` in the scenario file).

## Known failures

- `ai-big-a4e-vs-f22n-researched`: an F-22N searching at 975 kt at 3,000 ft
  flies into rising ground. At that speed the B44 look-ahead (terrain 1,000 ft
  ahead) is 0.6 s of warning, and the loaded G limit near the top of its
  envelope is 2.2 G. See "Needs a decision". Other runs can show the same
  class; the failing scenario can change with any behaviour change because the
  fights are chaotic.
- After the third round's fixes changed the fights' paths, the same terrain
  class also shows in `ai-big-x31-vs-faxx-researched` (an FA-XX at 958 kt) and
  `ai-long-15v15` (an F/A-18D searching level at 3,000 ft, 446 kt, into a
  hillside rising about 14 degrees; the floor gave 1.3 s of warning). Which
  runs hit this class moves with any behaviour change.
- `ai-takeoff-nsk-a5-f22`, `-f22n`, `-faxx`: at Nuchon Ni the F-22-family
  wingman chases its leader (which the test harness cruises at about 890 kt,
  3,000 ft above the ground) at 1,065 kt, 1,300 ft above rising ground, and
  flies into a hillside 145 s after takeoff. Same cause as the F-22N above.
- `ai-theater-apa-takeoff-a3` (Santa Fe) and `ai-theater-lfa-takeoff-a3` (San
  Carlos): these ground starts use 1,074 ft strips; each wingman's takeoff roll
  runs off the end onto the grass at 70 to 90 kt before it lifts off, leaving
  the probe's hazard open (item 12 below).
- `ai-theater-cub-takeoff-a1` (Key West, near the north edge): the airborne
  friendly wing starts on the runway heading, north, has no route and leaves
  the map after 163 s (item 5 below).
- `ai-known-f22-leader-wingman-ukr3` (reported by the flight lane): after a
  ground start at Krasnodar (UKR 3) behind the test harness's F-22, the
  wingman (the player's wing flies the player's type; `--probe-friendly-aircraft`
  sets wings 2 and 3 only, so an F-18 there gives the same run) flies into
  rising ground at about 800 kt after 71 s. The formation trace shows why: the harness cruises
  the F-22 at about 890 kt, 3,000 ft above the ground, so the wingman is in
  Intercept 25,000 to 35,000 ft behind and 2,500 to 3,100 ft below its gate.
  The fitted intercept rule ([physical departure and
  rejoin](../spec/ai.md#physical-departure-and-rejoin)) asks for leader
  velocity plus a closing vector toward the gate, so the altitude closes in
  proportion to the distance: about 20 ft/s of climb, while the ground ahead
  rises at over 100 ft/s at that speed. The wingman at full afterburner,
  lower and in denser air, still loses ground (closure -100 to -700 ft/s), and
  the 1,000 ft terrain look-ahead (item 7) warns 0.7 s before the hill. This
  follows the formation spec as written, so it was not changed; closing the
  altitude error first when far behind, or a speed-scaled look-ahead, would be
  new rules (item 14). The same happens with an F-22 in human hands only if
  the player cruises that fast that low.
- Default fuzz seeds `ai-fuzz-0014`, `-0028`, `-0032` and `-0053`, and
  `ai-damaged-fault04-hit` and `-gun` (an undamaged X-31 wingman at 900 to
  950 kt): the supersonic low-level class above (see "Fourth round").
- Activity flapping and pitch-stick oscillation at a weapon's envelope edge,
  and mid-air collisions, are reported but allowed (see above and below);
  regression scenarios check strictly.

## Needs a decision

Behaviour the specs do not define, with the evidence. None of these were changed.

1. **Dithering between gun and missile.** The "no store" case is fixed from
   B42's retry (above). A fighter whose chosen store alternates between its
   gun and a missile at the zone edge still swaps maneuvers every few ticks
   (`ai-pair-a4e-vs-mig29`); B42 has no retry for a change of store.
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
   (`ai-long-hold-4v4`). In a ground start near a map edge, the airborne
   friendly wings start on the runway heading and leave within minutes (Key
   West, 163 s); the "keep the enemy on the map" rule turns only the enemy
   there. Nothing defines a map edge for AI.
6. **Landing over hills.** A wing ordered to land at Simferopol flies the
   approach gates south of the field over rising ground. The fitted terrain
   correction holds the current heading, so an aircraft that just passed a gate
   flies on away from the field for about a minute; each approach takes about
   400 s, taxiing to parking another 330 s, and a wing of four needs more than
   30 minutes. Meanwhile a ground-started wingman still waiting cannot take off,
   because the runway gate stays closed while anyone flies the gates.
7. **Fast low flight and the terrain look-ahead.** B44 looks 1,000 ft ahead;
   the [AI source notes](../formats/ai.md) confirm the fixed distance, so it
   was not changed. At supersonic speed near the ground that is under a second
   of warning, and it now costs the F-22-family wingmen at Nuchon Ni and an
   F-22N in a 6 v 6 (see Known failures). A time-based look-ahead, or a speed
   limit near the ground, would be a new rule.
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
    spec; the land order's spec ([AI airfield](../spec/ai-airfield.md),
    [airports](../spec/airports.md), opinionated, John 2026-09-23) and the
    [ground-start baseline](../baselines/ground-start.md) do not say what the
    reply or the aircraft should do.
11. **Short strips.** Quick Mission ground starts accept 1,074 ft strips
    (Santa Fe, San Carlos), and fighters then roll off the end onto grass
    before lifting off. No spec sets a minimum runway for an aircraft.
12. **Airborne starts inside the terrain.** See "Found, not fixed": should
    the creator refuse, or raise, an airborne aircraft that would start below
    its own ground?
13. **Slow damaged approaches.** A damaged Rafale recovering at 158 kt spent
    12 minutes flying the approach over the hills south of Simferopol, went
    around once and was still approaching after 20 minutes (`ai-damaged-*`,
    fuel leak and control faults); same cause as item 6.
14. **A wingman far behind and below its leader.** Should Intercept close a
    large altitude error before the distance (see Known failures, F-22
    leader at Krasnodar)?

## Needs a human eye or ear

- The in-flight AI activity line and target-window activity during the
  envelope-edge dithering above, and after deaths (now fixed to catch up).
- Radio chatter volume in 10 v 10 and larger fights (the probe prints only the
  first 40 lines heard; none repeated, none spoken by a dead pilot).
- How the formation and missile-defense changes look in the cockpit view.
- Every aircraft that lands at Simferopol and taxis clear leaves the pavement
  for about 25 seconds at the same runway-exit corner (x 1,106,135, z 593,076),
  creeping at 3 kt while it turns (the fitted creep turn that replaces the
  retail stopped pivot), then rejoins the taxiway. The probe reports and then
  clears an "off landable surface" hazard there; worth a look in the viewer.

## What was not run

Windowed captures (this lane is headless only), theater layout variants
(the `~` maps; only the 16 base theaters), Windows and macOS, and a retail
comparison.

### Fourth round (2026-09-29, all four lanes merged)

The merged branch passed the lane with only the known failures. All 400 fuzz
seeds ran (about 45 minutes at `--jobs 6`): 385 passed. The 15 failures are
known classes: 12 are F-22-family aircraft (and one X-31) flying at 890 to
1,140 kt low over rising ground (item 7; several are the player's wingmen
chasing the test harness leader, which cruises an F-22 at about 890 kt),
seven are ground starts on 1,074 ft strips (item 11; a crash also leaves the
probe's ground hazard open), and seed 100 is the spawn inside a mountain. In
the default 60, seeds 14, 28, 32 and 53 fail this way. Creator objectives:
all 40 scenarios agree with the debrief after the fix. Order drills: every
order answered, refusals with the documented messages (no airport selected,
no hostile designated, wingmen bugged out). Damaged aircraft: no impossible
states; every damaged aircraft either died, ejected, landed or was still
flying its approach; the only other failures were the known supersonic class
and aircraft with no route leaving the map (now allowed there). The replay lane's list of 29 flagged scenarios
(`.local/battery/ai-churn-flags.txt`, recorded before the third-round fixes)
was re-run: of its 26 probe commands, 24 now carry no churn flag, and the two
left are the furball and the G-limited pursuit described in the third round.
After the
player dies the player's wingmen keep "In formation" with no leader, which
only matters if a flight continued without the player. No friendly-fire kill
by a wingman was seen in any run, so that counter was not exercised.

Fourth-round final run of the whole lane: 697 of 712 passed in 45 minutes at
`--jobs 6`; the 15 failures are the Known failures above. After merging the
flight lane's round two (the envelope 1 G floor and ceiling lift), the gates
passed and a 33-scenario post-merge subset (regressions, objectives, fuzz
seeds 1 to 9, determinism, order drills) passed.

### Third round (2026-09-29, after the merge with the replay lane)

Second read of the specs for the two round-two items: the airborne friendly
wings in a ground start "keep the airborne launch" ([Quick
Mission](../spec/quick-mission-menu.md#player-ground-start)), which places
them relative to the player's heading, here the runway heading; with no route
they hold it (B48), so leaving the map from Key West is what the specs say and
stays a decision (item 5). The 1,074 ft strips are the documented runway
fallback fields, whose placement is specified but whose usable length for a
given aircraft is not (item 11). Neither was changed.

### Second round (2026-09-28, after the merge with the menus lane)

The merged branch passed 445 of 446 (the known F-22N failure). Investigated
against the specs without a change: the defence dive-and-turn push (item 3:
the fitted pitch law does exactly what the spec formula says), the approach
over the hills (item 6: the fitted terrain correction is specified to hold
heading), the terrain look-ahead (item 7: executable-confirmed 1,000 ft) and
the land order on the ground (item 10: undefined). All 16 theaters' landing
pairs parked (in 618 to 816 s), both 30-minute 15 v 15 runs abroad were clean,
and the 138 new scenarios passed except the known failures above. Final run
of the whole lane on the merged branch: 577 of 584 passed in 29 minutes at
`--jobs 6`; the 7 failures are exactly the Known failures listed above.
Third-round final run (with the replay and menus merges and the round-three
fixes): 576 of 585 passed in 28 minutes; the 9 failures are the Known failures above (the
terrain look-ahead class, the two short strips and Key West).
