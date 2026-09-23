# In-flight situation music

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research specification, 2026-09-23, research mode. Build: reviewed FA.EXE 1.02F,
SHA-256 `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`.
Evidence is static disassembly and the retail score scripts. Nothing was run and
no retail session was observed. The score script grammar, the recordings and
their availability live in [the music format notes](../formats/music.md).

## Summary

During flight the game plays one of nine recorded scores and changes score when
the situation changes. Each score has a fixed rank. A situation that needs a
higher-ranked score cuts in at once, mid-phrase. A situation that needs a
lower-ranked score waits until the current score reaches one of its marked
phrase boundaries, or until the current score ends by itself. Success music and
home music play at most once per flight. Being hit keeps combat music selected
for 30 seconds.

Provenance for every rule below is **native** (reconstructed from the reviewed
executable's control flow and data) unless a row says otherwise. None of it has
been checked against a running retail game.

## Rank and conditions

The game checks the conditions from the top of this table down and takes the
first one that holds. Rank decides whether a change is immediate (see
[transitions](#transitions)). Rank is not the order of `tore_formats::music::SCORES`;
an implementation must not use that array index as the rank.

| Rank | Score | Selected when | Plays |
| --- | --- | --- | --- |
| 8 | VALK | The Valkyries toggle is on (Ctrl+V in flight). | Loops one phrase forever. Silent in this install, see below. |
| 7 | SUCC | The mission has succeeded for this player, and SUCC has not yet started this flight. | One phrase, then ends. Once per flight. |
| 6 | EJECT | The object the player controls is no longer an aircraft, which is the case after ejecting. | Loops. |
| 5 | LAUNCH | The player's aircraft is in the takeoff part of the airport sequence: rolling on the ground at an airport or carrier, launched from a catapult, or in the low climb-out just after takeoff. | Up to six phrases, then ends. Restarts if still launching. |
| 4 | AIR | The player's designated target is a live enemy aircraft within 40,000 ft, **or** the player's aircraft was hit by a projectile in the last 30 seconds. | Loops. |
| 3 | DANGER | Any of: the player's designated target is a live enemy aircraft beyond 40,000 ft, or a live enemy of the moving-vehicle class; an AI aircraft is targeting the player with a missile selected (4 second or 1 second memory, below); a missile in flight is guided at the player. | Loops. |
| 2 | HOME | The mission has succeeded, the player then came within 42,240 ft of the home base below 20,000 ft ("We're almost home!"), and HOME has not yet started this flight. | Up to four phrases, then ends. Once per flight. |
| 1 | DECK | The player's aircraft is on the ground at an airport or carrier and not rolling for takeoff: parked, stopped, lined up, or hooked to a catapult. Also during the touchdown and rollout (or arrested) part of a landing. | Loops. |
| 0 | NORMAL | None of the above. | Loops. |

### Numbers behind the conditions

- **Enemy** means the other side: the target's nationality differs from the
  player's in its side bit. **Live** means the target object still exists and
  is marked active.
- **Designated target** is the player aircraft's own current target, the one
  the player has locked or selected. Having no target contributes nothing.
- **Air combat range** is 40,000 ft (6.6 nm), straight-line distance between
  the player and the target.
- **Hit hold**: any projectile hit that the game applies to the player's
  aircraft sets combat music for 30 seconds of game time after that hit. The
  latest hit wins. A projectile from a same-side shooter is filtered out
  before this point unless the shooter's controller flag is set (believed to
  mark a human controller, not verified); unowned projectiles count.
  Only the hit hold keeps AIR selected after the threat is gone. Nothing holds
  DANGER.
- **Being targeted**: while an AI aircraft has the player as its target and a
  missile selected, the game keeps a warning alive for 4 seconds after the
  latest refresh while the AI's attack stage is below 3, and for 1 second at
  stage 3 or above. Either warning selects DANGER. How often an AI refreshes it,
  and what the stages mean to the player, are unknown.
- **Missile inbound**: every 2 seconds the game counts live missiles whose
  target is the player. An AIM-120 more than 30,380 ft (5.0 nm) from the player
  is not counted. A count above zero selects DANGER. A missile also joins or
  leaves the count at the moment it acquires or drops the player as its target,
  with the same AIM-120 range exception.
- The being-targeted and missile-inbound warnings are kept per seeker class,
  classes 1 to 4. All four classes count toward DANGER. The meaning of each
  class is not established here.
- **Moving-vehicle class** is the non-aircraft object class that shares the
  aircraft's airport state machine (source object type 3). Which retail objects
  that covers, for example ground vehicles or ships, is unknown.
- **Mission success** is the game's mission result, re-evaluated every 4
  seconds of game time. In multiplayer the result is inverted for players on
  the opposite side to the first player slot.
- **Home**: once the mission has succeeded, the game checks every 4 seconds
  whether the player is within 42,240 ft (6.95 nm, 8 statute miles) of the home
  airport or carrier and below 20,000 ft altitude. When both hold, the "We're
  almost home!" radio call plays (only while airborne) and HOME becomes
  eligible. A mission with no home base never selects HOME.
- **SUCC and HOME are disabled for the whole flight** when the mission result
  is already decided at flight start (already succeeded, already failed, or no
  objectives to decide), or when there is no mission name.
- **Mission failure** has no in-flight score. The flight keeps its situation
  music.

### Airport sequence for DECK and LAUNCH

The game gives the player aircraft an airport state. The ranges below are the
ones the chooser tests; the transitions listed are the ones reviewed for this
spec, not the complete state machine.

| Player situation | Score |
| --- | --- |
| On the ground at an airport or carrier, below 7 ft/s (about 4 kt) | DECK |
| Stopped on a catapult-type position, heading within 30 degrees of it | DECK (hooked) |
| Throttle value above 100 while hooked to the catapult | LAUNCH (launch) |
| On the ground in the takeoff sequence at 7 ft/s or more | LAUNCH (includes taxiing) |
| Airborne after takeoff, within 25,000 ft of the airport, below 4,000 ft above the ground, gear down, and below 954 ft/s (about 565 kt) | LAUNCH (climb-out) |
| Airborne otherwise, including landing approach | not DECK or LAUNCH |
| Touchdown, rollout and wire arrest of a landing | DECK |
| Landing complete and stopped | DECK once returned to the parked state (partial: a separate stopped landing state outside the DECK range also exists; whether the player passes through it is not established) |

Leaving the climb-out window (climbing past 4,000 ft above the ground, raising
the gear, reaching 954 ft/s or moving beyond 25,000 ft of the airport) ends the
LAUNCH condition. Because LAUNCH has no marked boundaries, the LAUNCH score still
plays to its end.

## Transitions

- **The situation is checked every frame of flight**, except for about one
  second after a score change. If a score cannot be loaded, the next attempt is
  10 seconds later.
- **Higher rank than the current score: immediate.** The current phrase is cut
  and the new score starts from its beginning with no fade.
- **Lower rank than the current score: wait.** The current score continues
  until it passes a marked boundary (the script's reevaluation marker) or ends.
  At a marked boundary the game picks again from the table and may select any
  score, higher or lower. The player hears the change at that phrase boundary.
  If the same score is still chosen, it simply continues.
- **When a score ends by itself**, the next check picks from the table with no
  rank restriction. LAUNCH restarts from its beginning if the player is still
  launching; SUCC and HOME never restart.
- **Same score chosen again**: nothing changes. A score is never restarted just
  because its condition is re-met.

Marked boundaries and typical waits, from the retail scripts and recordings
(11,025 Hz PCM):

| Score | Marked boundaries | Phrase length, min / median / max | Ends by itself |
| --- | --- | --- | --- |
| NORMAL | After almost every phrase; one run of nine phrases has none | 23.8 / 43.6 / 81.5 s | No |
| AIR | After almost every phrase | 19.6 / 36.7 / 68.8 s | No |
| DANGER | After every phrase | 26.7 / 39.0 / 56.3 s | No |
| DECK | After every phrase | 38.3 / 48.8 / 49.8 s | No |
| EJECT | After every phrase | 37.0 / 41.8 / 81.5 s | No |
| LAUNCH | None | 23.8 / 45.6 / 68.8 s | Yes, after up to six phrases: expected about 127 s, at most 275 s |
| HOME | None | 34.6 / 45.2 / 49.1 s | Yes, after two to four phrases: 43.5 s to 174 s |
| SUCC | None | 16.9 / 24.5 / 43.5 s | Yes, after one phrase |
| VALK | None | missing | No |

So a downgrade, for example from AIR back to NORMAL after the enemy is gone,
normally happens at the end of the current phrase: up to about 70 seconds later.

### Consequences a player notices

- Locking an enemy fighter inside 40,000 ft, or taking a hit, switches to AIR at
  once. After the fight, AIR plays out its current phrase before calming down,
  and a hit keeps AIR for at least 30 seconds.
- Being targeted or fired upon brings DANGER at once unless a higher score is
  playing. DANGER to AIR is immediate; AIR to DANGER waits for a boundary.
- Combat cannot interrupt LAUNCH, because LAUNCH outranks AIR and DANGER.
  Ejection, success and the Valkyries toggle can.
- SUCC cuts in over combat and ejection as soon as the mission result turns to
  success, plays one phrase once, then the situation music returns.
- HOME can be cut short by combat. It is counted as played the moment it
  starts, so it never returns.
- Starting a flight on a runway or carrier deck opens with DECK; starting
  airborne opens with NORMAL.

## Other controls that stop the music

- **Ctrl+V** toggles the Valkyries score on or off and stops the current music
  immediately; the next check then picks VALK or the situation score. The key
  only works while the player is flying an aircraft. The toggle is not reset
  when a new flight starts. In this install the Valkyries recording
  (`VALK001`) is absent under both the digital and the MIDI naming, so the
  toggle produces silence; the existing `VALK01.XMI` does not match the name
  the script asks for.
- Setting the in-flight music volume to zero stops the score at once, and no
  score is chosen while it is zero. Restoring it starts a fresh choice.
- Ending the mission from the flight menu (Ctrl+Q) and leaving flight stop the
  score.
- While the game is paused (or time compression is at its pause value), no
  score is chosen and no new phrase starts. Whether the phrase already playing
  is paused or plays on is not established.

## Current TORE state

Implementation mode, 2026-09-23. TORE plays the situation scores by the rules
above. The rank order, the condition order, the 1 second lockout, immediate
upgrades, downgrades at marked boundaries, a re-chosen score continuing, SUCC
and HOME once per flight, LAUNCH restarting, the 30 second hit hold and the
distances are **spec-derived**. The selector is
`crates/tore-app/src/audio/situation.rs`, its inputs come from
`crates/tore-app/src/flight_music.rs`, and the mission result cadence from
`crates/tore-app/src/ai_wings/outcome.rs`. Audio only reads simulation state;
headless and `--no-audio` runs do not compute any of it.

| Condition | What feeds it in TORE | Provenance |
| --- | --- | --- |
| VALK | Ctrl+V while flying an aircraft (not ejected, not crashed, not paused). The toggle lasts for the session and is not saved. It stops the current score. The message "Valkyries music on" or "off" is an agent addition (2026-09-23). The recording is absent, so the result is silence. | spec-derived; message opinionated |
| SUCC | The in-flight mission result below reaching success. "Mission accomplished!" (`^MISSACC`) is sent on the radio channel about 2 seconds later, the first time. Retail does not establish its label; TORE uses the crew label in a multi-crew aircraft, otherwise `YOU`. | spec-derived; label fitted |
| EJECT | The player has ejected. | spec-derived |
| LAUNCH | Takeoff roll: on a runway surface at 7 ft/s or more, having not just landed. Climb-out: after lifting off, within 25,000 ft of the liftoff point, under 4,000 ft above the ground, gear down and at 954 ft/s or less. Leaving the window ends it for good. | fitted state tracking, spec-derived numbers |
| AIR | The player's designated target (T, Enter or a scope click) is an aircraft with hit points left, on the enemy side, and within 40,000 ft; or a projectile damaged the player in the last 30 game seconds. Enemy side is the AI wing side; without AI wings (range and fixture aircraft) any target not marked friendly counts. | spec-derived; fixture side rule fitted |
| DANGER, target | The same designated enemy aircraft at 40,000 ft or more. The moving-vehicle class stays false: TORE has no object class that matches it. | spec-derived; vehicle class unknown |
| DANGER, AI aim | A live AI aircraft whose current target is the player and which still carries a usable guided air-to-air store. TORE's AI keeps no selected station, so carrying one stands in for having it selected. The warning lasts 4 seconds after the last such step; the 1 second final-attack memory has no TORE equivalent. | fitted |
| DANGER, missile inbound | A live projectile fired at the player whose guidance target is the player. An AIM-120 farther than 30,380 ft is not counted. Checked every simulation step rather than at acquisition plus every 2 seconds. | spec-derived; cadence fitted |
| HOME | After success, checked every 4 game seconds: within 42,240 ft of the home base and below 20,000 ft above sea level. The home base is the Quick Mission ground-start airport, at the mean centre of its runways; an airborne start has none, so HOME never plays there. "We're almost home!" (`^ALMSTHM`) is queued once, only while airborne. | fitted home base and altitude datum |
| DECK | On a runway surface below 7 ft/s, or in the touchdown and rollout after a landing until stopped. A bounce during the rollout counts as airborne, not as a new takeoff. There is no carrier, catapult or taxiway state. | fitted |

**In-flight mission result.** Every 4 game seconds the Quick Mission is judged
by the [debrief](debrief.md) evaluator itself, so the success music and the
debrief always agree: its retail rules decide success (every target shot down,
crashed or ejected, no friendly objective lost, no friendly-fire kill by the
player). The 4 second cadence is spec-derived. If the mission has already
succeeded at the first check, for example with no enemy aircraft, SUCC and HOME
are off for the flight. The debrief reports only success or failure, not an
open result, so a mission already failed at flight start is not detected and
does not disable them (fitted). Free flight, the range and fixture wings have no
mission, so SUCC and HOME never play there. Failure plays no score and no radio
call.

**Transitions in TORE.**

- Choices run once per 120 Hz simulation step in game time, so nothing is chosen
  while paused. Holds and timers count game seconds, so time compression
  shortens them on the clock; the music itself plays at normal speed.
- The score player reports each marked boundary. While the situation wants a
  different score, it stops at the boundary instead of starting the old score's
  next phrase, and the selector switches on the next step. The retail game
  starts the next phrase and cuts it one frame later; TORE leaves a gap of a few
  milliseconds instead. Agent decision, fitted.
- A missing script or phrase stops the score, and the next choice comes 10 game
  seconds later. Retail applies the 10 second retry to a failed load; in TORE a
  missing phrase is only found when it is due. Each distinct fault is printed
  once. Fitted.
- Pause keeps freezing the current phrase, which resumes where it stopped. This
  is TORE's existing behaviour; the retail behaviour is unknown.
- With Music off nothing plays and nothing is chosen; turning it on starts a
  fresh choice. Music can currently only be switched from the main menu.
- A crash without ejecting keeps the situation music, as retail is unknown.

## Unknowns and next research steps

| Unknown | Next step |
| --- | --- |
| Seeker classes 1 to 4 for targeting and missile warnings | Trace the byte at weapon record +0xb4 in the importer data and in `ServiceSounds`, which uses classes 2 and 3 for warning tones. |
| AI attack stage that separates the 4 s and 1 s warnings, and how often it refreshes | Review `_NPCWeaponsProc` from `0x4736f0`, field +0x11d. |
| Which objects form the moving-vehicle class | Map type-3 objects in the imported object catalog. |
| Full airport state machine, including states 2 to 6 and 9 to 0x10, the catapult flag and throttle units | Continue `APTakeoff` `0x4badb0`, `APLanding` `0x4bc270`, `_ServicePlayer` `0x416470..0x417530`; record in the airport spec. |
| Home base choice for carrier versus land missions, and whether altitude is above sea level | Review `_APHomeAirport` `0x4bed70`. |
| Whether free flight and Quick Missions have a mission name and objectives, which decides whether SUCC and HOME can ever play there | Review `_MISSIONSuccess` `0x481a70` inputs per mission type. |
| Whether pause freezes the current phrase | Review the digital music voice and pause handling around `DMusicOff` `0x432bd0`. |
| Music when the player's aircraft is destroyed without ejecting | Review the end-of-flight path after state 0 (crash). |

## Source notes

Byte-level evidence (addresses, globals, decoded tables and scripts) is recorded
in [the music format notes](../formats/music.md). Primary routines:
`_ChooseScore` `0x441c90`, `_ChooseScoreInit` `0x441c60`, `ScoreUpdate`
`0x432ca0`, `ScoreOn` `0x432c30`, `ScoreOff` `0x432c70`, `_DAMAGEDoHit`
`0x40f970`, `_NPCWeaponsProc` `0x4736f0`, `@PROJSetTarget@4` `0x4c0870`,
`_PROJLockUpdate@0` `0x4c0960`, `_PLANECommentProc` `0x48ec40`,
`_MISSIONCheckSuccess@0` `0x486860`, `_ServicePlayer@0` and `@FlightKey@4`
`0x415b3e`. Phrase durations are file sizes of the imported PCM divided by
11,025. No Fighters Anthology manual was available locally; the USNF manual
transcript only describes the music volume panel. Ctrl+V is key code `0x2f02`
(V with modifier 0x02). Modifier 0x02 is read as Ctrl because the flight menu's
End Mission action uses `0x1002` (Q), matching the manual's Ctrl-Q, and the
key table's `0x1f04` (S) matches the manual's Alt-S Radio Silence. This naming
is inferred, not observed.
