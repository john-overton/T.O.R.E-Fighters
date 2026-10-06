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
| Wing orders | `ai-order-*` | bug out, land at selected airport, attack on contact, engage my target, wings of 2 and 5; `ai-order-sort-wing4` (stage G3c) has the lead of four sort its three wingmen onto the two enemy aircraft 20 nm out and requires an assignment with the order Sort for each wingman, the first two on different bandits and no bandit with more than two |
| Data link | `ai-datalink-*` | the flight data link's picture (stage G): `ai-datalink-picture` prints every plane's radar flag once, the player's designation becoming a lock, AI locks and unlocks in pairs, and a wing order to one wingman (`@1`); `ai-datalink-player-lock` and `ai-datalink-player-lock-mirror` (stage G2) have the player lock one of two bandits 20 nm out and require the wingman, ordered to attack later, to lock the other one in both directions; `ai-datalink-assign` prints the `assign`, `acknowledge` and `clear` lines of an Engage my target to the first wingman and the Attack on contact that clears it; `ai-datalink-order-link` (stage G3b) blinds the first wingman (`--probe-blind-wing`), has the player lock the bandit 50 nm out and orders Engage my target, and requires the order to be taken, the wingman to end acquiring, and no lock or shot from it |
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

The scripted player leads the wing, and a probe that orders the wing to land
(`ai-ground-land-selected-*`, `ai-ils-terrain-*`) requires a living leader: the
order must be accepted, the pair must land, and a scripted player that has
crashed is reported, never excused (a crashed leader would hand the wing to its
next member and get "you are not leading your wing"). The probes with no enemy
near (`ai-takeoff-*`, the bug-out and land-pair probes) also require the player
to end alive.

A probe that needs a lost player (`lost-lead-*`) loses it on purpose with
`--probe-lose-player TICK`, which crashes the scripted player's aircraft at that
tick (test harness only). Then lead succession applies: the order is refused with
"you are not leading your wing", and the AI that takes the lead flies a
[mission of opportunity](../spec/ai.md#mission-of-opportunity-after-a-lost-human-leader)
(John, 2026-09-30). With no enemy and no route the new lead must print
`mission of opportunity: ... returning to base (no enemy position known)` and
land at its home runway, and the landing checks apply as for a living player
(no aircraft may leave the map). In any ground start, a wingman still waiting
to take off when its wing goes home stays parked, and the takeoff check excuses
it.

**The scripted pilot** (`ProbePilot` in `crates/tore-app/src/main.rs`, a `fitted`
test harness, agent decision 2026-09-23 and 2026-09-30) flies the takeoff and
climb-out the way the AI's own departure does, so every aircraft from every
ground start reaches a safe cruise. Rolling, it holds the runway line (rudder
only if it is more than 2 degrees off the runway heading or 15 ft off the line,
so a roll that stays true is untouched). At 50 ft it raises the gear, and the
flaps once the speed is 1.05 times the clean wing's 1 G minimum (the A-4E and
Su-25 hold them a couple of seconds). In the climb it holds 10 degrees nose-up
but eases toward level flight as its speed falls from 1.12 to 1.02 times the
aircraft's own 1 G minimum for its current flaps (`State::minimum_level_speed`),
and raises the nose, up to 25 degrees, to clear the highest ground within 45 s
ahead by 500 ft. It levels off (autopilot on) at 3,000 ft above the ground once
it is 1,000 ft over that ground, and in the cruise it hands back to the climb,
at full power, if the ground ahead rises to within 600 ft of its altitude. What
went wrong before, by cause:

| Symptom | Cause |
| --- | --- |
| The player flew into a hill 40 to 60 s after takeoff (UKR 6 and 12, the home probe) or 18 minutes into a long cruise (the full landing probe) | The pilot held 10 degrees and then the autopilot's altitude and never looked at the terrain; the hills ahead rise more than it climbs |
| The player left the runway and hit high ground at the end of the runway 11 s after starting (KURILE 3) | The roll had no rudder and the mission's wind turned it 20 degrees off the runway, 350 ft off the line, before it was airborne (with `TORE_WIND=0,0` it holds the runway) |
| Every scripted takeoff at Simferopol flew about 8 degrees off the runway heading before P5 | The same wind on the same unsteered roll, where nothing was in the way; the leader now flies the runway heading |

The Simferopol crash of the older record (16.8 s after takeoff, 85 ft) does not
happen on this base: the fitted stall speeds (P1) removed it. Turbulence was not
the cause (`TORE_TURBULENCE=0` changed nothing).

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

### Sixth round: John's decisions (bb2, 2026-09-29)

John answered the decision list. Each change below is a new behaviour he
requested on 2026-09-29, recorded as opinionated in the spec; the numbers are
agent decisions (see "Agent decisions to review" below).

| Decision | Change | Commit |
| --- | --- | --- |
| 12, spawns inside terrain | Airborne Quick Mission groups start at the higher of 5,000 ft MSL and 1,000 ft above their own ground ([Quick Mission](../spec/quick-mission-menu.md)). A scan of all 16 base theaters, airborne and ground starts at airports 1 to 3, and separations 5, 20, 50 and 100 nm (256 starts, 15 v 15) found no aircraft starting under the ground | `219f981` |
| 10, land order on the ground | A wingman still parked or taking off answers unable and stays in its departure; "Land at Simferopol: N landing" counts only airborne aircraft ("M unable, still on the ground or taking off"). After takeoff it joins the formation as usual ([AI airfield](../spec/ai-airfield.md)) | `042662f` |
| 7 and 14, terrain look-ahead | B44 looks six seconds of travel ahead (never less than 1,000 ft) at up to twelve points and asks for the climb that clears that ground; six seconds is derived in the [AI spec](../spec/ai.md#b44-steering-execution-and-pursuit-lead) from the worst case met. The intercept rule (decision 14) is unchanged: with the longer look-ahead the far-behind wingman at Krasnodar climbs over the hills | `92c6a5a` |
| 4, leaderless wingmen | The next wingman in order leads and the others close up on it; a wingman following the lost leader in to land stops. No radio call: the retail call needs a living previous leader ([leader succession](../spec/ai.md#leader-succession)) | `7366700` |
| 9, deconfliction | Aircraft flying on their own (not in a formation procedure, not defending, not in an airfield sequence) predict closest approach against every airborne aircraft and hold a heading 30 degrees away, right when head-on, until 3 s after the conflict ([traffic avoidance](../spec/ai.md#traffic-avoidance)). Mid-air collisions over the 126 fight, mission, big-fight and objective scenarios: 15 before, 2 after; no activity-flapping anomaly; a 15 v 15 probe runs in the same time (26 s either way) | `39e7590` |
| 5 in part, after the fight | Once a side has seen hostile aircraft and none is alive, its AI wings land at their home runway, or the leader flies back to its launch point and holds there when there is none ([return to base](../spec/ai.md#return-to-base-when-the-mission-is-over)); scenario `ai-rtb-after-win` | `475cafc` |
| 8, decoyed missile still kills | Unchanged by decision: the kill credit stays as the debrief spec says | none |

| Symptom | Cause | Fix |
| --- | --- | --- |
| Returning to base, fuelled F/A-18Ds sank at 75 ft/s at idle with the speedbrake out (a 17 degree descent against the 7 degrees commanded) and touched down on grass 2,500 ft short of Simferopol (`ai-long-1v1`, `ai-long-5v5`) | Inside the 1 G envelope the AI's positive G limit was 1 G divided by the loading, 0.77 G, although the hybrid model keeps 1 G there and ramps higher, so no pitch request could hold the path; and the final speed, 1.1 times the minimum (94 kt), left no lift in hand. The go-around came at 199 ft, too late | `06b8017`: the AI G limit is never below the model's own; the final flies no slower than the speed with 1.3 G of loaded lift (137 kt here), the speedbrake stays closed below the path, and a final 150 ft below its path above 100 ft goes around (all fitted, in the airfield spec). The same F/A-18D now tracks the path within 30 ft and lands. A damaged MiG-21 (`ai-damaged-fault21-*`) that used to go around 16 times in 20 minutes now lands at 1,000 s and is taxiing in at the end, which the scenario check now accepts |

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

**Aircraft spawned inside mountains (fourth round).** Fixed in the sixth
round (`219f981`, see above). In a ground start at Jixian (Vladivostok,
airport 2) or Bahawalpur (South Asia, airport 2) with the enemy 50 nm away,
the enemy group started at the chosen 5,000 ft over ground up to 6,856 ft high
and was destroyed at 0.0 s (fuzz seed 100). The probe still reports `AI probe
UNDERGROUND start` should it happen again.

**AI aircraft collide in the air.** Before the sixth round, 15 of 126 fight,
mission, big-fight and objective scenarios showed a mid-air collision, all
between aircraft that fly no formation (two searching leaders on converging
courses in `ai-fight-1v15-noattack`, leaderless wingmen meeting at the same
last-seen point in `ai-fight-1v5-default`). With traffic avoidance two remain:

- `ai-fight-15v15-noattack`: Enemy 2-5 and Enemy 3-4 at 35.7 s (two wings in
  a furball).
- `ai-fight-3v3-default`: Enemy 1-1 and its wingman Enemy 1-3 at 22.9 s.
  Both had been released to engage (neither is in the formation trace) and
  both had been defending against missiles since 5.7 and 8.0 s (32 decoys
  used); avoidance never overrides missile defense, which is the likely
  reason. Not confirmed tick by tick.

The lane reports these as `mid-air collision` anomalies but does not fail on
them (see `KNOWN_ANOMALIES` in the scenario file).

## Known failures

The sixth round's six-second terrain look-ahead ended the supersonic
low-level class: `ai-big-a4e-vs-f22n-researched`, `ai-big-x31-vs-faxx-researched`,
`ai-long-15v15`, `ai-takeoff-nsk-a5-f22`, `-f22n` and `-faxx`,
`ai-known-f22-leader-wingman-ukr3`, fuzz seeds 14, 32 and 53 and
`ai-damaged-fault04-hit` and `-gun` now pass, and their markers are gone.

- `ai-theater-cub-takeoff-a1` (Key West, near the north edge): the airborne
  friendly wing starts on the runway heading, north, has no route and leaves
  the map after 163 s while the enemy is still alive (item 5 below).
- Activity flapping and pitch-stick oscillation at a weapon's envelope edge,
  and mid-air collisions, are reported but allowed (see above and below);
  regression scenarios check strictly.

## Needs a decision

Behaviour the specs do not define, with the evidence. John answered items 4,
5 (in part), 7, 8, 9, 10, 12 and 14 on 2026-09-29; they are listed here for the
record with what changed (see "Sixth round" above).

1. **Dithering between gun and missile.** Fixed in the fifth round from B13
   (timed motions run to their deadline); left here for the record.
2. **Notch side on a head-on shot.** The notch turns 90 degrees toward the
   smaller turn with a tie break; with the radar source dead ahead the side
   flipped after one second (heading 288, then 108), wasting the roll-in.
3. **Missile defense that dives and turns at once.** The fitted pitch law
   pushes negative G to reach a 20 degree dive while banked, which turns the
   aircraft the other way: a wingman asked to turn 90 degrees achieved 2 degrees
   in 3.5 s and was hit. Steep and inverted tracking is a documented
   approximation of the input-only controller.
4. **Leaderless wingmen after a fight.** Changed (John, 2026-09-29): leader
   succession (`7366700`), and after the fight the wing returns to base
   (`475cafc`). Later (John, 2026-09-29): in the multiplayer work this succession was replaced by
   multiplayer's lead succession ([lead succession](../ARCHITECTURE.md#lead-succession)).
5. **Aircraft with no route leave the map.** Changed in part (John,
   2026-09-29): once no hostile aircraft remains, wings go home and land, or
   fly back to their launch point and hold (`475cafc`). While hostile aircraft
   remain, an aircraft with no route still holds its heading (B48): the hold
   mission (`ai-long-hold-4v4`, about 16 minutes) and the friendly wing on a
   Key West ground start (163 s) still leave the map. A map-edge rule for AI
   is still open.
6. **Landing over hills.** A wing ordered to land at Simferopol flies the
   approach gates south of the field over rising ground. The fitted terrain
   correction holds the current heading, so an aircraft that just passed a gate
   flies on away from the field for about a minute; each approach takes about
   400 s, taxiing to parking another 330 s, and a wing of four needs more than
   30 minutes. Meanwhile a ground-started wingman still waiting cannot take off,
   because the runway gate stays closed while anyone flies the gates.
   Since 2026-09-29 AI approaches fly the player's 3 degree ILS path (John's
   decision): over the 16 theaters' landing pairs the time from approach to
   touchdown barely moved (median 300 s before, 296 s after), because the
   gates keep their distances and the track length dominates.
7. **Fast low flight and the terrain look-ahead.** Changed (John,
   2026-09-29): a six-second speed-scaled look-ahead (`92c6a5a`).
8. **A decoyed missile still kills.** Unchanged by decision (John,
   2026-09-29): the kill credit follows the [debrief spec](../spec/debrief.md).
9. **Deconfliction outside formation.** Changed (John, 2026-09-29): traffic
   avoidance (`39e7590`).
10. **"Land at selected airport" for an aircraft still on the ground.**
    Changed (John, 2026-09-29): it answers unable and stays in its departure
    (`042662f`).
11. **Short strips.** Changed (John, 2026-09-30): the 22 airstrips of about
    1,074 ft are no ground start and leave the in-flight airport list (see
    "Short strips" below). The two theater takeoffs that started on one
    (`ai-theater-apa-takeoff-a3`, `ai-theater-lfa-takeoff-a3`) now start on the
    next airport that is not (`-a7`, `-a4`), and the eight fuzz seeds that
    started on one (89, 149, 180, 296, 309, 316, 338, 399) start on that
    airport too, so their known-failure markers are gone.
12. **Airborne starts inside the terrain.** Changed (John, 2026-09-29): raised
    to at least 5,000 ft MSL and 1,000 ft above the ground (`219f981`).
13. **Slow damaged approaches.** A damaged Rafale recovering at 158 kt spent
    12 minutes flying the approach over the hills south of Simferopol, went
    around once and was still approaching after 20 minutes (`ai-damaged-*`,
    fuel leak and control faults); same cause as item 6.
14. **A wingman far behind and below its leader.** Answered by item 7 (John,
    2026-09-29): the intercept rule is unchanged; the longer look-ahead keeps
    the wingman over the hills.

### Agent decisions to review (sixth round)

The numbers and edges of John's new behaviours are agent decisions, recorded
as such in the specs:

- **Look-ahead:** six seconds of travel, twelve samples at most, 1,000 ft
  apart at least; the derivation is in the AI spec.
- **Traffic avoidance:** 6 s horizon, 300 ft plus half a second of closing
  speed, 150 ft against the aircraft it is attacking, a fixed 30 degree
  heading change, head-on within 20 degrees, held 3 s after the conflict.
  Formation wingmen are left to their slots, and missile defense always wins,
  so two aircraft defending at once can still collide (likely the 3 v 3 case
  above).
- **Return to base:** triggered by hostile aircraft only, because the AI's
  objectives (patrol, intercept, escort) are air objectives and the AI mission
  sees no ground targets. A wing led by the player never goes home on its own,
  since the player's ground objectives may still be open. A mission with no
  hostile aircraft at all never ends this way. Without a home runway the
  leader "holds" by flying the return-to-base heading back over its launch
  point every few miles (a figure-of-eight rather than a racetrack); no
  scenario in the lane has a wing without a home runway, so this is covered
  by a unit test only.
- **Leader succession:** a wingman following the lost leader in to land in
  the early approach stops, as the wing abort does.
- **Land order on the ground:** "taking off" counts as on the ground until the
  departure sequence ends.

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

### Short strips (aircraft pass, 2026-09-30)

John decided that the 22 airstrips of about 1,074 ft are no ground start and
leave the in-flight airport list ([rule](../spec/airports.md#short-strips), item
11 above). What the lane does about it:

- `tools/battery_scenarios/_strips.py` lists each theater's short strips (from
  `TORE_AIRPORT_PROBE=1 tore-app --quick-mission --snapshot X.ppm --theater CODE`,
  which prints `airport runway: ... short_strip=` for every runway) and
  `ground_airport(theater, n)` moves a ground start to the next airport that is
  not one. The AI probe's `--ground-start N` refuses a short strip, so a scenario
  or fuzz seed must not name one.
- `ai-theater-apa-takeoff-a3` and `ai-theater-lfa-takeoff-a3` are now
  `ai-theater-apa-takeoff-a7` (Chitre, 7,246 ft) and
  `ai-theater-lfa-takeoff-a4` (Walker Creek, 7,246 ft). Their known-failure markers are gone.
- The fuzz seeds that drew a ground start on a strip (89, 113, 149, 180, 296, 309,
  316, 338, 399; all in Panama or the Falklands) keep every other draw and start
  on the next long airport. The eight marked as known failures now pass and are
  unmarked; 113 passed before and still does.
- A unit test (`tools/test_battery_ai.py`, `ShortStripTests`) checks that no AI
  scenario and none of the 400 fuzz seeds starts on a short strip.

Run of the lane's theater, takeoff, ground, ILS-terrain, long-ground, record,
determinism and all 400 fuzz scenarios (`TORE_AI_FUZZ=all`, `--jobs 6
--windows 1`): 557 of 558 passed. The one failure is `ai-fuzz-0163`
(`~IRAF`, an airborne 15 against 10 on the legacy model): the radio repeats
"Get this guy off me" 3 times in 5 s. It is not from the strip change: the
multiplayer branch's tip before this change (a31d66a) prints the same lines, and
this build's output for that seed is identical to it apart from timestamps
(`~IRAF` has no short strip). It passed on the main-based builds of 2026-09-29, so
it began with the multiplayer branch's radio changes; it is not marked here
because the lead owns that branch's known failures.

### Aircraft pass: the new lead after a lost human (2026-09-30, branch `mp/air-lead`)

John's decision 6 of the aircraft pass
([mission of opportunity](../spec/ai.md#mission-of-opportunity-after-a-lost-human-leader)).
Whole lane at `--jobs 8`: 727 of 727 passed in 51 minutes, the known failures
caught as such. The player is lost in 142 scenarios: in 24 the new lead knew
of no enemy and went home (every ground start at Simferopol and the ILS
terrain pairs among them), in 10 its wing flew under weapons hold or
self-defense and went home, and in 112 it searched and fought. None of the
112 ran long enough, or kept a wing alive long enough, to end a search by
itself; the unit tests cover the end of the search. The first run failed four
ground starts at Simferopol (wings of four and five): the waiting wingmen,
held by the lead's landing traffic, never took off. They now stay parked when
the wing goes home, and the takeoff check excuses them.

Then (John, 2026-09-30) the new lead flies the wing's remaining waypoints
before going home. No mission has waypoints yet, so every scenario above goes
home as before; `ai-lost-lead-route-ukr-a6` gives the player's wing two
waypoints with `--probe-wing-route`, and after the player is lost 60 s after
takeoff at Kharkiv the new lead flies both (the first reached at 213 s, the
second at 364 s), returns to base and lands. The scripted pilot no longer
crashes there (the aircraft pass's pilot work), so the player is lost on purpose
with `--probe-lose-player 7200`; `ai-lost-lead-land-order-wing2` and `-wing4` keep
the land-order branch the same way, at Simferopol, 20 s after takeoff.

### Sixth round (2026-09-29, John's decisions)

Whole lane on the sixth-round build before the last fix: 713 of 716 passed in
58 minutes at `--jobs 6` (alongside two other lanes). Besides the known
failures listed above it failed `ai-ground-fight-wing4`: once the enemy was
gone the airborne second wing went home to the same runway, and its landing
traffic kept the player's fourth wingman from ever taking off. `51dd2c1` holds
a side's return to base while any of its aircraft still departs; the ground
scenarios and `ai-rtb-after-win` then passed (13 of 13). The remaining two new
failures, `ai-long-1v1` and `ai-long-5v5`, were winners landing short of
Simferopol; `06b8017` fixed the final approach (see the second table under
"Sixth round"), and both now pass. After that fix: the landing, ground-start,
return-to-base, damaged-aircraft and regression scenarios passed 62 of 66 on
the first run; the other four were those two runs, now passing (markers
removed), and the damaged MiG-21 that now lands and taxis in (check
corrected). John then asked for the AI to fly the player's 3 degree ILS
path (`2eb77ba`, see "3 degree approach" below). The mid-air collision comparison is in the sixth-round table
above; over the whole lane 27 collisions were reported (allowed, not
failing). The spawn scan used `--probe-fight 15:15` for 2 ticks per start.

### 3 degree approach (2026-09-29)

John asked for AI approaches to follow the player's 3 degree ILS angle.
`2eb77ba` flies the gates and final on the shared `airport::glide_path_height_ft`
path to the ILS aim point 1,000 ft past the threshold (the path crosses it
about 52 ft above the wheels; retail flew 6 degrees to the landing anchor).
The probe now prints each final's wheel height over the threshold and the
lane fails one below 10 ft (short) or above 300 ft (long). New scenarios
`ai-rtb-after-win-2v15-ukr`, `-pgu`, `-fra` and `-vla` land fifteen winners
each. A follow-up holds a final level short of the threshold when the wheels
come within 40 ft of the ground (KURILE 3 skimmed 15 ft over the ground
before the threshold, which stands higher than the path there).

Results with the 3 degree path: the landing, ground-start, return-to-base,
damaged, regression, takeoff and theater scenarios passed 190 of 190 (the
three known failures caught as expected). Threshold crossings over the
landing scenarios: 42, with the wheels 57 to 77 ft up (median 65; the
path is 52 ft), none short or long. After the terrain hold (`65afc67`) the 62 landing, ground-start,
return-to-base and damaged scenarios passed 62 of 62, and KURILE 3 crosses
its threshold 119 ft up and lands. In each 2 v 15 return to base, 20 minutes
after the start 2 to 5 of the 15 winners have landed and the rest hold at
marshal: one runway, one approach at a time, about five minutes each.
A pair landing at NSK 6 (Hyon Ni) then flew its approach gates for 700 s
before reaching final: the approach terrain rule saw hills beyond the lower
3 degree gates and held the heading, flying away from the gate. `5ecce00`
counts only the ground on the way to the gate being flown to and stops the
descent without holding the heading (fitted); Hyon Ni reaches final in
284 s. The same commit lands a runway without anchors from its clear side
when the ground stands above the 3 degree path to the arrival side.

**Runway ends with terrain above the 3 degree path.** The ILS survey
(`--validate-ils`, [ILS checks](ils.md)) lists seven: Amiens (FRA 9, far
end), KURILE 3 and NSK 6 (near ends, ground 12 to 19 ft above the path at
the threshold) and the far ends of Donets'k, Kharkiv, L'viv and
Ivano-Frankivs'k (UKR 5, 6, 8, 12; hills about 1,900 to 2,000 ft above the
path 13,000 to 28,000 ft out). `ai-ils-terrain-*` orders a pair to land at
each and fails a crash, a threshold crossing below 10 ft or above 300 ft,
or more than 540 s on the gates. The scripted player is a living leader for all seven (its pilot clears the hills
after UKR 6 and 12 and the high ground at KURILE 3), so each landing order is
accepted and the landing is checked. (Before the pilot work the player flew into
a hill 60 s after takeoff at Kharkiv and 40 s at Ivano-Frankivs'k, the wing's
next member led, and the order was refused; with the mission of opportunity that
lead returned to base and landed, 64 ft over the threshold, parked about 650 s
after the start. That branch is still tested, with a deliberately lost player,
by `land_order_checker(player_lost=True)`.) All seven pass: every one lands on the
near end, which the airports' landing anchors choose (spec-derived), so the
AI never flies the UKR and Amiens far ends; threshold crossings 64 to 119 ft
(KURILE 3 holds level over its high ground), 240 to 284 s from the first
gate to final.

### Whole lane and all fuzz seeds on the merged tree (2026-09-29, af5ffd4)

The whole lane with all 400 fuzz seeds (`TORE_AI_FUZZ=all`) on a frozen copy
of the merged build (bb2-ai, bb2-menus, bb3-ils and the flight agent's
overspeed, world-edge and belly rules): 1,051 of 1,060 passed in 61 minutes
at `--jobs 12`; the four known failures were caught as such. Outside the
fuzz seeds nothing failed. The nine fuzz failures fall in two classes, now
marked as known failures:

- Seven ground starts on the 1,074 ft strips at Goose Green, Santiago and
  Santa Fe (seeds 149, 180, 296, 309, 316, 338, 399; item 11): a wingman's
  takeoff roll runs off the strip and leaves the probe's hazard open.
- Two undamaged fighters flying straight at full power at 1,095 and
  1,133 kt (seeds 183, an FA-XX, and 266, an F-22 on the legacy model) sank
  steadily from about 2,800 ft into flat ground in 15 s, never pulling up.
  They are airborne AI starts, which fly the legacy flight model, and at
  that speed only the 1 G envelope row covers them; the legacy model divides
  that 1 G by the loading (1 + fuel and stores over empty weight times the
  loaded-elevator percentage), so a loaded aircraft has less than 1 G and
  cannot hold level flight, let alone climb over the terrain floor. The
  hybrid model keeps 1 G there (the flight agent's rule, 2026-09-29), the
  legacy model deliberately does not. This is the remaining supersonic
  low-level class of item 7, and it is not the look-ahead: the ground was
  flat. Fixing it means either the same 1 G rule for the legacy model
  (flight model, not this lane) or an AI top speed where the loaded
  aircraft still has 1 G (a broad change to every fight); neither was made.

Mid-air collisions outside the fuzz seeds: 11 (10 on the build before the
landing work). The 400 fuzz seeds show 79, with no earlier all-seed count to
compare.

### Weight-scaled stall speeds (2026-09-29, 1d135fb)

The flight agent's weight-scaled stall speeds raised every aircraft's slow
edges, and 12 AI scenarios failed (six landing pairs, the ground landing,
`ai-rtb-after-win`, `ai-long-guns-3v3`, `ai-long-15v15`, fuzz seeds 28 and
43). `4e47944`:

- The AI minimum speed was the slowest edge of any envelope row, the 0 G row
  included: 114 kt for a fuelled F/A-18D that needs about 144 kt to hold 1 G,
  so finals built on it sank. It is now never below
  `flight::State::minimum_level_speed` in the current flap setting (the flight
  agent's idea, applied with the current flaps rather than full flaps).
- The AI maximum is now never above the fastest envelope row that still
  leaves 1.2 G after the loading. Near the top speed only the 1 G row holds
  and the legacy flight model (airborne AI starts) divides it by the
  loading, so a loaded fighter at full power sank into flat ground at about
  1,100 kt (seed 43 after the stall change, and the all-seed seeds 183 and
  266 before it). All three now pass.
- The final's lift margin drops from 1.3 to 1.15 G (1.3 now asked for more
  than the 174 kt final cap) and the flare horizon from 6 to 4 s (the faster
  final floated a high F/A-18D past KURILE 3's go-around point). The F/A-18D
  now flies its final at 162 kt; threshold crossings over the whole lane
  were 56 to 114 ft (median 62) at 122 to 169 kt (median 163).

Whole lane with all 400 fuzz seeds on a frozen copy of that build: 1,063 of
1,067 passed in 41 minutes at `--jobs 12`. The four failures: fuzz seeds 28,
183 and 266 now pass (markers removed), and seed 89 is a new 1,074 ft strip
case (an FA-XX at San Carlos no longer lifts off within the strip; marked).
All 12 scenarios above pass. Mid-air collisions: 11 outside the fuzz seeds,
84 in them.

Short strips: with the heavier liftoff speeds more aircraft roll off the
1,074 ft strips (Santa Fe, San Carlos, Goose Green, Santiago) before lifting
off. AI wingmen are still placed there and roll off the end, leaving the
probe's hazard open; the markers cover the lane's cases (two theater
takeoffs, eight fuzz seeds). Whether Quick Mission should refuse a ground
start on a strip too short for the aircraft, or the AI should hold its
wingmen parked there, is still decision 11. World edge and overspeed: the
Key West friendly wing still leaves the map by 163 s (its known failure);
no AI aircraft in the lane was destroyed by the new overspeed rule, and the
new top-speed limit keeps AI fighters below the 1 G row's edge.

### Fifth round (2026-09-29, review follow-ups)

- A review found that round two's breakout fix scored every heading from 2 s
  ahead when within 220 ft, hiding conflicts inside the first 2 s (the winning
  escape in its own test passed 26 ft away at 0.12 s). `b4f1d85` scores a
  heading that still closes on its true closest approach and uses the 2 s
  distance only for a heading that already opens. The unit test now asserts
  the chosen escape's predicted separation; the diving-reversal minimum is
  265 ft. On the same 132 fight, mission, objective and regression scenarios
  the collision counter is unchanged (30 flags before and after), all between
  wing leaders, different wings or opponents, none between formation
  wingmen; `ai-pair-f18-vs-rafale` (a leader and wingman both released to
  fight) collides identically on the previous build.
- The gun and missile alternation (item 1 under decisions) is fixed by B13's
  timed-motion rule: gun lead tracking is a 1-second motion and now runs to
  its deadline (`3142392`, regression scenarios `ai-regress-gun-missile-flap-su35`
  and `-a4e`, including the flight lane's `flight-attack-su35` case).
- Final subset on the merged branch: 321 of 321 passed (fights, pairs,
  missions, objectives, regressions, determinism), gates green.

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
