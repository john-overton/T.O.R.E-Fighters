# Mission scripts

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-11-06. Static review of FA.EXE 1.02F and the 21 mission
code modules in `FA_2.LIB`; nothing was run. Evidence, addresses and module
identities are in [mission code modules](../formats/mission-code.md).

Some campaign and training missions carry a small script of their own, named
by the mission file's `code` line. This page says what those scripts do for the
player. Missions without a script follow the game's ordinary rules.

## What the player sees and does

Scripts add four kinds of experience to an otherwise ordinary mission:

1. **Scripted radio calls.** A supporting unit (AWACS, E-2C, FAC, J-STARS, a
   ship, a helicopter, an escort) speaks at a set mission time, when something
   happens to an object, or a short while after the mission objectives are met.
   The call is a radio message with the speaker's name at the start of its text,
   and plays either recorded voice phrases or the radio beep. Radio silence does
   not mute it.
2. **Story state.** A ship starts already damaged, a cruiser cannot be damaged,
   friendly submarines sink ships on a schedule, or a rescued pilot disappears
   when the helicopter picks him up.
3. **A different win rule.** A few training missions are won by landing, one by
   ejecting after a catapult failure, and one by destroying tanks only while a
   J-STARS controller designates them.
4. **Nothing visible.** Three shipped scripts are not used by any mission.

Scripts only run in single player. Times below are on the mission clock, in
seconds from the start of the mission. Every scripted event happens once.

Whether the mission has been won is checked every 4 seconds of mission time
while flying and again when the mission ends. "When the mission is first
reported won" below means the first of those checks that finds it won.

## Numbers

### Campaign and single missions

| Mission | Script | What happens |
| --- | --- | --- |
| Eviction Notice (Ukraine campaign 1) | `U01` | The first time the IL-96 carrying Yeltsin loses any hit points, its crew makes a voiced emergency call. 30 s after the mission is first reported won, the IL-96 thanks the player (beep). |
| Nightprowler (Ukraine 7) | `U07` | The Kirov cruiser cannot be damaged: any hit point loss is undone at once. At 180 s the E-2C calls bogeys high inbound from the northeast (voiced). |
| Fog of War (Ukraine 8) | `U08` | The Kirov starts the mission at half its hit points (rounded down). |
| Carrier Strike (ATF Gold single mission `EXTRA01`) | `EXTRA01` | The carrier Clemenceau starts at half its hit points. |
| Mop Up (Ukraine 11) | `U11` | Friendly submarines sink three non-target ships, each with a large water explosion and a message: a Krivak at 180 s, the Sovremennyy at 300 s, a second Krivak at 360 s. |
| Top Cover (Ukraine 12) | `U12` | At 120 s AWACS reports bogeys to the northwest and asks for visual identification (beep). |
| HAVCAP (Ukraine 15, single mission `UKR18`) | `U15` | While the E-3 AWACS is alive: at 120 s it calls bogeys high inbound out of the northeast (voiced); when the E-3 starts the last leg of its route, the relieving F-14s say "go on home" (voiced). |
| BARCAP (Ukraine 22, `UKR30`) | `U22` | At 120 s the E-2C calls contact, bogeys low inbound from the north (voiced). |
| Down and Dirty (Ukraine 23, `UKR16`) | `U23` | At 180 s the FAC says the FEBA is 10 miles west of waypoint Alpha (beep). At 240 s it orders an attack on the advancing tanks and APCs (beep), with a second line saying they are crossing the highway. |
| MiG Hunt (Ukraine 24, `UKR33`) | `U24` | At 420 s the strike flight announces it is commencing its bombing run (beep). |
| Convoy Escort (Ukraine 25, `UKR24`) | `U25` | At 180 s the USAF F-15 escort calls bingo fuel and hands over (beep). 30 s after the mission is first reported won, the convoy thanks the player for the top cover (beep). |
| Eyes in the Sky (Ukraine 29, `UKR19`) | `U29` | At 120 s AWACS calls contact, bogeys high inbound from the southeast (voiced). At 240 s it identifies Su-34s escorted by MiG-29s and orders the attack (beep). |
| Stealthy Ivan (Ukraine 34, `UKR47`) | `U34` | The first time either of the two Sacramento supply ships loses hit points, it calls for help (voiced). One call for both ships. |
| Rough Tread (Kurile campaign 16) | `K16` | Timed tank designation, see below. |
| RESCAP (Kurile 17) | `K17` | When the SH-60 starts its pickup approach it says it is picking up the pilot; on the next route point it reports the pilot on board and the downed pilot on the ground disappears (both with beep). |

None of these scripts changes how the mission is won, except `K16`.

### Rough Tread: designated tanks

Eight T-72s stand in two groups, Alpha and Bravo, about 29 nautical miles apart.
None is a target at the start. J-STARS then designates two tanks at a time:

| Mission time | J-STARS | Designated pair |
| --- | --- | --- |
| 10 s | Scramble to Alpha and await instructions | none |
| 150 s | Destroy designated tanks at Alpha; when finished, egress to Bravo | two at Alpha, until 500 s |
| 500 s | Destroy designated tanks at Bravo; then await instructions at Alpha | two at Bravo, until 850 s |
| 850 s | Destroy designated tanks at Alpha; then await instructions at Bravo | two more at Alpha, until 1,200 s |
| 1,200 s | Destroy designated tanks at Bravo; then head home | two more at Bravo, until 1,400 s |

Each call is a beep with its first line; the "when finished" line is text only.

The mission fails, permanently, if a designated tank is still alive when its
window closes, or if any of the eight is destroyed before its pair is
designated. If neither happens, the mission counts as won from 1,400 s.
Nothing else affects the result; destroying other objects does not fail it.

### Training missions

| Mission | Script | How it is won |
| --- | --- | --- |
| Basic Flight, Landing, Stalls, Spins, Cobra, Advanced Spins, High-Speed Dives (`UKR01` to `UKR07`) | `UKR02` | Be airborne at some point, then touch down while alive. |
| Running on Empty (`VIET03`) | `VIET03` | The same rule. |
| Unwelcome Surprise (`UKR14`) | `CATFAIL` | The catapult always fails; eject after it fails. |

**Landing rule.** Once the player has been airborne (Basic Flight starts on the
Eisenhower's deck, the others start in the air) the first moment the player's
aircraft is on the ground and alive wins the mission, for good. The script was
written to require touching down within 3,000 ft of the carrier, but as shipped
it measures the player against itself, so any live touchdown anywhere counts,
including a touch-and-go. Crashing first is not a touchdown. The ordinary
objectives no longer matter. TORE plays the intended rule instead (see
[TORE decisions](#tore-decisions)).

**Catapult failure.** On launch the catapult fails: after the stroke, while the
F-14 is still on the deck, its speed is held at 73 ft/s (about 43 knots), far
below flying speed. The mission is won only by ejecting (SHIFT-E) after that
failure; the debrief says so. Ejecting before the launch does not count, and
nothing else wins it. In any other carrier mission the same failure is a
1 percent chance on each player catapult launch
([AI notes](../formats/ai.md#airfield-takeoff-and-landing-sequences)).

## Edge cases

- **Mop Up chain.** The scheduled sinkings happen in order and stop at the first
  ship that is already dead. If the player sinks the first Krivak before 180 s,
  the other two ships are never sunk by the script; if the Sovremennyy is dead
  by 300 s, the last Krivak is not sunk by the script. TORE sinks each ship on
  its own schedule instead (see [TORE decisions](#tore-decisions)).
- **HAVCAP.** If the E-3 is shot down, neither call is made after that. In the
  single mission `UKR18` the E-3 has no route, so the "go on home" call is not
  expected to play.
- **Thank-you calls.** The 30 s delay counts from the first 4-second check that
  finds the mission won, so it lands 30 to 34 s after the objectives are met.
- **Protected Kirov.** Damage that does not destroy it disappears at once.
  Whether one hit that takes all its hit points at once still sinks it is
  unknown.
- **Landing rule.** A bolter or touch-and-go on the deck already wins the
  mission, despite the "land on the first pass" wording in Running on Empty.
- **Multiplayer.** Scripts are not installed in a networked game, so these
  events do not happen there.

## TORE decisions

Recorded 2026-11-06. Each is [opinionated](../behavior-provenance.md); the
requester is named.

- **Radio text from the player's own media** (requested by John). The
  scripted calls' text and voice names are read from the player's own module
  files at import time, so TORE ships none of the retail lines.
- **Landing rule fixed** (requested by John). The training landing rule requires
  the live touchdown to be within 3,000 ft of the carrier, as the script was
  written, rather than anywhere. A touch-and-go within that distance still wins.
- **Mop Up sinkings fixed** (agent decision, applying John's rule of the same
  day: shipped bugs are fixed, intended mission features are kept). The early
  stop comes from the script returning at the first dead ship, not from the
  mission design: the three ships are not targets and each has its own time and
  message. TORE sinks each ship still afloat at its scheduled time, whatever has
  happened to the others.

## Unknown

- How the scripted radio text is shown (screen position, colour, how long it
  stays). Next step: trace the game's handling of literal radio messages next
  to [radio chatter](radio-chatter.md).
- Debrief credit for Mop Up's scripted sinkings.
- What the player sees at the bow after the catapult failure, beyond the 73 ft/s
  hold.
- That a carrier deck counts as "on the ground" for the landing rule is
  inferred from the missions being winnable, not traced.
- The RESCAP and HAVCAP triggers read "the route point the aircraft is flying
  to"; this reading is from the waypoint-change code and is not yet confirmed
  against the waypoint-reached handler.

## Source notes

Behaviour recovered from each module's imports, inline strings, object
aliases resolved against its `.M` files, and the `.MT` briefing and debrief
text, with the host side read in FA.EXE through the game's own symbol map
`FA.SMS`. Addresses, identities and the classification for the mission loader
are in [mission code modules](../formats/mission-code.md).
