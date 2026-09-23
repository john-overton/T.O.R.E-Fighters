# Cockpit voice: crew comments

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research, 2026-09-23. Static reading of FA.EXE 1.02F only; nothing was run.
Every component below is **native** provenance unless it says otherwise.
The AI and wingman radio system, the music and the airport tower are owned by
other specifications; this file only records where they meet the player's crew.

## What the player hears

In flight the game speaks short situational remarks from inside the player's
cockpit. A two-seat aircraft has a second crew member who talks: coaching in a
dogfight, fuel states, missile warnings, feet wet and feet dry, and the grunts,
heavy breathing and vomiting of hard manoeuvring. A single-seat aircraft has no
such voice. Its only coaching comes from the player's first wingman over the
radio, and it hears no crew fuel calls, missile warnings or G sounds.

Each remark plays one recorded clip and, at the same time, prints its text on
the HUD message lines as `SPEAKER: 'text'`. The speaker label is:

| Who is talking | Label |
| --- | --- |
| The crew of a two-seat fighter-class aircraft | `RIO` |
| The crew of any other two-seat aircraft (bombers, attack, transports, helicopters) | `CO-PILOT` |
| The player's own call in a single-seat aircraft (for example a radar link report) | `YOU` |
| A wingman | that aircraft's radio name, for example its wing name and number |

There is no separate backseater voice. The crew member speaks the same `^`
recordings the rest of the game uses, and only the label tells the player who is
talking. The `#` recordings are not the backseater: see
[the second voice set](#the-second-voice-set).

### Which aircraft have a crew voice

An aircraft has a crew voice when its PLANE record carries the multi-crew flag.
In the retail aircraft set that is 55 records. Eleven are fighter class and are
labelled `RIO`: Alpha Jet, AIDC Ching-Kuo, F-14D, F-15E, F/A-18D (`F18.PT`),
F-4B, F-4E, F-4G, F-4J, Hawk and MiG-31. The other 44 are labelled `CO-PILOT`,
among them the A-6E and A-6, A-37, B-1B, B-2, B-52, Buccaneer, EA-6B, F-111F,
SF.260, Strikemaster, Su-24, Su-34, Tornado IDS, the large transports and
tankers, and several helicopters. The F/A-18C, the AH-64 and AH-1 and every other
record without the flag are single seat for this purpose. Always read the flag
from the aircraft data; do not decide it from the real aircraft.

### Radio silence

The in-flight *Radio silence?* option (Alt+S; the HUD confirms `Radio silence`
or `Radio traffic OK`) switches off the crew's and wingman's combat coaching,
feet wet and feet dry, and the G sounds. The manual describes it as limiting
wingman and RIO chatter to the most important situations. Under radio silence
the player still hears the crew's fuel calls, missile warnings, radar link
reports, the mission result and "We're almost home". The feet wet state keeps
tracking silently, so turning radio traffic back on does not produce a stale
call.

### When the crew may speak

Coaching, feet wet and feet dry, the G sounds, the mission result and "We're
almost home" are checked on every update of the player's aircraft and need all
of the following. Fuel calls, missile warnings and radar link reports are event
lines that play as soon as their event happens.

- The aircraft is airborne and past its takeoff and landing sequences.
- No other radio or crew line has been shown in the last 3 seconds. Every spoken
  line, from anyone, holds the whole channel for 3 seconds.
- The player is not within 10 seconds of a scheduled death (the crash countdown
  that also drives the eject warning in [ejection](ejection.md)).
- Radio traffic is on, except for the lines listed under radio silence.

Combat coaching also needs a designated target, and the target must be on the
enemy side (the player's aircraft only enters its engaged state for an enemy
target). With no target the crew only calls feet wet and feet dry.

## Combat coaching

The coaching looks at the player's currently designated target and classifies
the geometry into one of four situations:

| Situation | Target within 90° of our nose | We are within 90° of the target's nose (azimuth) |
| --- | --- | --- |
| Head-on | yes | yes |
| Offensive (we are on his tail) | yes | no |
| Defensive (he is on our tail side) | no | yes |
| Neutral | no | no |

An aircraft target counts only within 105,600 ft (20 statute miles) and only
while it is itself flying freely: a target that is taking off, landing or
crashing produces no coaching. A ground or ship target has no range limit and
uses the approach calls below instead of the dogfight lines. When the player's
aircraft is not fighter class (the 0x4000 class: bombers, attack aircraft,
transports and helicopters, including single-seat ones such as the F-117,
F-105, Jaguar and A-1), there are no dogfight lines against aircraft, from the
crew or from a wingman. Such a crew keeps the ground-target calls, feet wet and
dry, fuel, missile warnings and G sounds.

### Pacing

A coaching remark comes at most every 4 to 7 seconds (4 plus a random 0 to 3),
with 2 seconds more when the target is beyond 8,000 ft. When the situation
changes from the one of the last remark, that wait is cancelled and the crew
speaks at once, which is how the transition lines below land on the moment.
Some lines add further delay, shown in the tables. All these delays are real
seconds: under time compression they stretch in game time to stay the same.

### Dogfight lines

Variants are chosen uniformly at random. "Self" is the crew of a two-seater;
"wingman" is the single-seat case described below.

| Situation and condition | Self (RIO) | Wingman |
| --- | --- | --- |
| Head-on, beyond 10,000 ft, 50% | position call: "He's at *n* o'clock...", turning left or right when he is turning between 20° and 160°; 2 s extra wait | same, "He's at your..." |
| Head-on, under 5,000 ft and his nose within 10° of us in azimuth and pitch | "He's coming right at us" or "He's on your nose" | "He's coming right at you" or "Stay off his beam" |
| Head-on otherwise, under 20,000 ft | "Let's get this guy" or "He's closing" | same |
| Offensive, just after a head-on pass | 6 lines: "Yahoo!", "We've got him now" (2 takes), "We're reeling him in", "The worm has turned", "Knock him out" | same |
| Offensive, gun selected, 5,000 ft or more, a missile would lock, 25% | "Switch to missiles" | not used |
| Offensive, within 1,200 ft and closing faster than 146 ft/s | "Don't overshoot!" | same |
| Offensive, 25% | not used | "I can't get a tone" or "Lock him up" |
| Offensive, 10,000 ft or more, 50% | position call, 2 s extra wait | same |
| Offensive otherwise | 6 lines: "Yahoo!", "We've got him now" (2 takes), "Turn'n'burn, baby", "Let's finish this guy", "Take him out!"; 3 s extra wait | same |
| Defensive, just after a neutral moment | 4 lines: "He's coming around", "He's back on our tail", "We can't shake him", "He's moving in position" | same |
| Defensive, he is behind us (more than 160° off our nose), his nose within 30° of us, and we are pulling no more than 3 G | 7 lines: "Break!" (2 takes), "He's on our six", "C'mon, do some of that pilot shit" (2 takes), "Get us outta here", "This is NOT good" | 10 lines: "Break!" (2 takes), "Bandit at six, break!", the two "pilot shit" takes, "Get outta there", "Bandit on your tail", "Maneuver, Maneuver!", "Better shake him", "Evasive action" |
| Defensive, faster than corner speed plus 110 ft/s (about 65 knots), 25% | "Slow down, we'll corner better" | not used |
| Defensive, target is an ace, 25% | 5 lines: "I don't like this", "This guy's good", "He's no rookie", "He's got some moves", "He looks skilled" | same |
| Defensive otherwise | position call | same |
| Neutral, just after a head-on pass | 4 lines: "Turn'n'burn, baby", "Bring it around", "I lost him! Wait...", "Do a 180"; 3 s extra wait | same |
| Neutral, target climbing or diving steeply (pitch 60° to 120°), 25% | "He's going vertical" | same |
| Neutral otherwise | position call ending "..., heading away" | same |

Offensive lines other than the position call add 4 seconds to the wait. The
"G" in the break rule is the aircraft's own G load; the rule stops the crew
shouting "Break!" at a pilot who is already pulling hard.

### Ground and ship targets

With a surface target ahead (within 90° of the nose), the crew announces range
as it closes. When the whole-mile distance first drops from 11 nautical miles or
more to under 11, they say "Approaching target". Otherwise, each time the
whole-mile distance changes and is at least 1, they give a range call: the
rounded distance in miles, the clock position unless it is 12 o'clock, and
", in range" at 2 miles or less. The phrase assembly of the range call belongs
to the radio system.

### Feet wet and feet dry

With no target, crossing from land to sea gives "Feet wet" and crossing back
gives "Feet dry", each one of three takes at random. After either call the next
remark waits 10 seconds more; when nothing is said it waits 5 to 9 seconds
more.

### The single-seat wingman

For a single-seat player the same coaching comes from the player's first
wingman (the second aircraft of the player's flight) when all of these hold:
the player leads the flight and is alive, the wingman and the player have the
same target, and the wingman is within 15,000 ft of the player. The geometry is
measured from the player's aircraft. The wingman's lines use the "you" wording
above and carry his radio name as the label. He never makes G sounds, but he
does call feet wet and feet dry for the player's aircraft when there is no
target. The wingman's own radio procedure is specified with the AI radio
system.

## G sounds

Only a two-seat player's aircraft makes G sounds, only with radio traffic on,
and only when the crew may speak. G here is the aircraft's G load as the flight
model holds it, taken to the whole G below it.

| Event | Trigger | Recording | Text | Extra wait |
| --- | --- | --- | --- | --- |
| Strain | G rises to 5 or more, or falls below -2, from between those limits; 30% chance | one of 7 at random: 4 grunts, 3 heavy breaths | `<grunt>` or `<heavy breathing>` | 3 s |
| Warning | the 18th crossing of -1 G (either direction) in the same clock minute | "Ease up on the stick..." | as spoken | 5 s |
| Sick | the 20th crossing in the same clock minute; the count then restarts | one of 7 vomiting sounds | the sound written out | 15 s |

The crossing count restarts at every new minute of the mission clock, so it takes
nine full push-pull cycles through -1 G within one clock minute to hear the
warning and ten to be sick. Counting pauses while another line holds the channel.
The text lines are printed with the crew label like any other remark. Whether
the strain and sickness recordings are the pilot's voice or the backseater's is
not established; the game labels them with the crew label.

## Fuel

A two-seat player's crew makes each fuel call once per flight, whatever the
radio setting:

| Fuel state | Crew line | Single-seat or AI line |
| --- | --- | --- |
| Joker (endurance at most time to home plus 10 minutes) | "Joker fuel" | "Joker fuel" |
| Bingo (endurance under time to home plus 5 minutes) | "Bingo fuel" | "Bingo fuel" |
| Critical (under 4 minutes of endurance) | "We're running on fumes" | "I'm running on fumes" |
| Empty | "We're out of gas" | "I'm out of fuel. I'm punching out" |

The fuel states come from the flight model's regular fuel check, specified in
[the AI specification](ai.md) (B48). A worse state also marks the milder ones
as said, so a sudden jump to bingo never plays joker afterwards. A single-seat
player hears none of these about their own aircraft; the right-hand column is
what AI aircraft say, heard from wingmen.

## Missile warnings

When a missile is fired at a two-seat player's aircraft, the crew warns half a
second later:

| Missile seeker | Line | Repeat limit |
| --- | --- | --- |
| Infrared class | "Atoll inbound, drop flare!" | not again for 6 game seconds |
| Radar class | "Apex inbound, drop chaff!" | not again for 6 game seconds |
| Any other | "Missile inbound! Break!" | none |

The seeker class is the missile's signature class (2 infrared, 3 radar), the
same classes that drive the missile warning tones. A single-seat player gets
the warning tones only. "SAM launch" and "AAM launch" calls belong to the AI
radio system.

## Other crew and player lines

- **Radar link.** Pressing the supplemental radar key with no link available
  gives "Supplemental radar not responding." with a radio beep, labelled `RIO`
  or `CO-PILOT` in a two-seater and `YOU` otherwise. Link on and link broken are
  squelch clicks with text, not voice.
- **Mission result.** When the mission is first won, "Mission accomplished!";
  when first lost, one of five failure recordings at random, all captioned
  "MISSION FAILURE!". Each plays once per mission, about 2 seconds after the
  result is decided, not in fortress missions. This happens for any human
  player, single or two seat.
- **Almost home.** Once per mission, after the mission is won: when the player
  is within 42,240 ft (8 statute miles) of the home airport and below 20,000 ft,
  and airborne, "We're almost home!". Checked every 4 game seconds.
- **Hit and kill calls.** When the player's own weapon hits, "Bullseye", "Good
  hit!", "Splash one bandit!" and the rest are spoken as the player's own call.
  When the player's aircraft is hit, "I'm hit" and similar. Both are addressed
  to the flight leader, which is the player when leading or alone, so they are
  labelled like a crew line: `RIO` or `CO-PILOT` in a two-seater, `YOU` in a
  single seat. These are radio calls of the AI radio system; variants and
  cooldowns are specified there.
- **Friendly fire.** Hitting a friendly aircraft within 10 statute miles makes
  the victim complain ("What the hell are you doing?" and seven others). This is
  the victim's radio call, not the crew.
- **Death and ejection.** When the player's aircraft is destroyed, a scream is
  played directly, not over the radio: "Aaargh..." 50%, "Oh, sh..." 25%,
  "Yaaaah" 25%. Ejection calls are in [ejection](ejection.md).

## The second voice set

The `#` recordings are a second, smaller voice set, used for every speaker whose
nationality is North Vietnamese or South Vietnamese. In the retail Vietnam
missions those are the enemy MiG pilots; the player flies as an American and
never uses it. When such an aircraft speaks, each `^` clip is swapped for its
`#` counterpart, or for the nearest `#` clip that exists (all seven vomiting
sounds become one, all four grunts become one, "He's on our six" becomes "Bandit
at six, break!", and so on). A crew remark with no counterpart becomes a radio
beep.
Composite calls (position and range) collapse to one generic `#` clip. Their
screams and ejection call use `#` recordings the same way. This is a speaker
nationality rule, not a crew-seat rule.

## Airport and carrier calls

The tower and landing signal officer speak to the human aircraft using the
airport. They are not the crew and are specified with [airports](airports.md).
For reference, the lines are: clear for takeoff (2 takes), "Ready on the cat",
"Stand by", "Cat one!" or "Cat failure, EJECT!", "Airborne", "Rotate, rotate!",
"Good luck" or "Good hunting", clear to land or "Clear the deck! Emergency crews
ready!" for a badly damaged aircraft, wind reports with occasional gusts,
"Call the ball", distance countdown, "Lower your gear", "Lower your hook", the
LSO corrections (go around, left, right, higher, lower, faster, slower, too much
bank, on the ball, steady), a landing grade and "welcome back" or "welcome home".
Each fires once per approach; their numbers are not part of this spec.

## Event to recording table

Stems are the recording names without the `.5K` extension.

| Event | Speaker | Stems |
| --- | --- | --- |
| Head-on, close | crew | `^ATUS`, `^YRNOSE` |
| Head-on, close | wingman | `^ATYOU`, `^OFFBEAM` |
| Head-on, closing | both | `^GETGUY`, `^CLOSING` |
| Offensive after head-on | both | `^YAHOO2`, `^GOTNOW1`, `^GOTNOW2`, `^REELING`, `^WORM`, `^KNOCK` |
| Offensive | both | `^YAHOO3`, `^GOTNOW1`, `^GOTNOW2`, `^BURN1`, `^FINISH`, `^TKOUT` |
| Switch to missiles | crew | `^SWCMISS` |
| Overshoot | both | `^DONTOVR` |
| No tone | wingman | `^CNTTONE`, `^LOCKHIM` |
| Defensive, coming around | both | `^COMARND`, `^ONTAIL`, `^WESHAKE`, `^INPOSI` |
| Break | crew | `^BREAK1`, `^BREAK2`, `^ONOUR6`, `^PLTSHT1`, `^PLTSHT2`, `^USOUT`, `^NOTGOOD` |
| Break | wingman | `^BREAK1`, `^BREAK2`, `^BANDIT6`, `^PLTSHT1`, `^PLTSHT2`, `^GETOUT`, `^BANTAIL`, `^MNVR`, `^SHAKHM`, `^EVASV` |
| Slow down | crew | `^SLOWDWN` |
| Skilled opponent | both | `^DNTLIKE`, `^GUYGOOD`, `^NOROOK`, `^SMMOVE`, `^LKSKILL` |
| Neutral after head-on | both | `^BURN2`, `^BRGARND`, `^LOSTHIM`, `^DO180` |
| Vertical | both | `^VERTICL` |
| Position call | both | `^HESAT` or `^HESYOUR`, clock clips, `^TURNLFT`/`^TURNRGT`, `^HEADAWY` (radio system) |
| Approaching target | both | `^APPTRGT` |
| Range call | both | number and `^MILE`/`^MILES` clips, clock clips, `^INRANGE` (radio system) |
| Feet wet / dry | both | `^FT_WETA`..`C` / `^FT_DRYA`..`C` |
| Strain | crew | `^GRUNT1`..`4`, `^BREATH2`..`4` |
| Ease up | crew | `^EASEUP` |
| Sick | crew | `^BARF1`..`7` |
| Fuel | crew | `^JOKER`, `^BINGO`, `^WEFUMES`, `^OUTGAS` |
| Fuel | AI | `^JOKER`, `^BINGO`, `^IMFUMES`, `^OUTFUEL` |
| Missile warning | crew | `^ATOLFLR`, `^APEXCHF`, `^MISSBRK` |
| Radar link | crew or player | `^BEEP2` |
| Mission won | player side | `^MISSACC` |
| Mission lost | player side | `^NOTPLSD`, `^NOMEDAL`, `^BLEWIT`, `^MESSUP`, `^SERIOUS` |
| Almost home | player side | `^ALMSTHM` |
| Death scream | player | `^AARRRGH`, `^OHSH`, `^YAAAAAH` |

## Implementation in TORE

Implementation, 2026-09-23: `crates/tore-app/src/crew_voice.rs`, evaluated on
every fixed 120 Hz tick just before due radio lines are delivered. Lines go
through the shared channel (`crates/tore-app/src/comms.rs`), which applies the
3 second hold, the radio silence filter and delivery. Components are
spec-derived unless listed below.

Implemented:

- Crew label and fighter class from the aircraft data, read once per flight.
- The four dogfight situations, the 105,600 ft aircraft limit, pacing (4 plus
  0 to 3 seconds, 2 more beyond 8,000 ft, immediate on a change) and every row
  of the dogfight table, including position calls composed from the imported
  phrase text and clock recordings. Offensive lines other than the position
  call add 4 seconds; the plain offensive line adds its 3 seconds on top.
- Surface targets: "Approaching target" and whole-mile range calls.
- The single-seat wingman: the player's wing member 1, alive, on the same
  target (or both without one) and within 15,000 ft, speaking the "you" lines
  under his radio label. TORE's player always leads Quick Mission wing 1.
- G strain, "Ease up on the stick" and being sick, counted on the fixed tick.
- Fuel calls, once each, never silenced.
- Missile warnings one second after launch (the human-flown warning delay of
  B47) plus half a second, with the shared 6 second limits.
- The death scream, played directly, only when the aircraft is destroyed
  without an ejection, so it never doubles the ejection calls.
- Feet wet and feet dry.

Fitted components:

- **Water test.** Over water means the terrain grid cell under the aircraft
  has class 1, the class the collision query reports as water; outside the
  grid counts as water, as the original's fallback cell does
  ([land contact](../formats/native-land-contact.md)). Which query
  `_PLANESetFeetWet` uses is not read. The first sample of a flight is taken
  silently, so an air start over water does not open with "Feet wet".
- **Free flight.** "Past takeoff and landing" is read as airborne with the gear
  up; TORE has no player takeoff or landing sequence state.
- **Scheduled death.** The eject-warning danger test stands in for the
  10 second crash countdown.
- **Mission clock.** The G crossing minute is the minute of the flight's time
  of day.
- **Turning left or right.** The position call's turn angle is unknown; TORE
  uses his heading relative to ours, and says "turning left" or "turning
  right" when that is 20 to 160 degrees. The clock position is always level.
- **A missile would lock.** A loaded infrared or radar air-to-air missile
  whose own seeker would see the target now, using TORE's seeker model without
  terrain masking, in place of the original's seeker evaluation.
- **Corner speed.** The slowest speed of the highest-G envelope at the current
  altitude, the same host rule the AI uses.
- **Fuel state.** The AI's B48 thresholds, with endurance from all remaining
  fuel at the military flow scaled by the current throttle (floored at 10%)
  and time home from the straight distance to the flight's first position at
  cruise speed. Known difference: at high throttle joker and bingo come earlier
  than with the original's cruise-throttle estimate.
- **Break and head-on geometry.** "His nose within 30 degrees of us" and the
  head-on 10 degree test use our azimuth and elevation off his nose, ignoring
  his bank.
- **Extra wait after a G sound** is added to the later of the pending coaching
  time and now.

Not implemented here: the radar link report (no supplemental radar key), the
mission result and "almost home" lines (music and debrief work), airport and
carrier calls, the `#` second voice set (no Vietnamese speakers fly with the
player) and the takeoff and landing states of AI targets (TORE's AI targets
are always airborne; only "going down" silences the coaching).

## Unknown

- **Voice identity.** Whether the `^` crew lines, strain sounds and the `#` set
  are distinct voice actors, and the accent of the `#` voice, needs listening to
  the imported recordings. The code gives no speaker identity beyond the label.
- **Mission result label.** These lines are addressed to the player's flight,
  not to the player alone, so their HUD label (`YOU` or the crew label) is not
  resolved. Next step: trace the flight-address expansion in the message sender.
- **Engaged-state producer.** The player enters the engaged state for an enemy
  target in a routine whose call frequency and other conditions are only partly
  read. Next step: finish the player target-state routine that sets it.
- **Update rate.** The comment check runs on every update of the aircraft; the
  original update rate varies with frame rate. The G crossing count depends on
  sampling. A host should evaluate it at the fixed 120 Hz tick (fitted).
- **Mission result gating** skips the result lines for a player who is a
  wingman in a flight led by a particular kind of leader; that leader test is not
  read.
- **Supplemental radar key** binding and availability are not traced here; see
  [radar](radar.md).
- **Clear the deck threshold** and the rest of the airport numbers belong to the
  airports research.

## Source notes

FA.EXE 1.02F, SHA-256
e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c; LLVM
disassembly and `symbols.json` under `.local/weapons-research/native/`. Byte-level
facts are for `docs/formats/radio.md`. Main routines: `_PLANECommentProc`
0x48ec40 (called for every object update by `Service` with util kind 7),
`_APCommentProc` 0x48f6a0, `@SAYTranslate@4` 0x490f30, the `#` rewrite
0x490480, SAY output 0x48d470 (label and nationality test),
`@SAYLowFuelMessage@8` 0x48eb20 (crew call from `_ServicePlayer` 0x416696),
`@SAYSuppRadarMessage@12` 0x48ea10, the missile warning in `_PLANEEventProc`
0x49e0f4, `_Kill` 0x473c10, `_AlmostHome` 0x481b80, `MSGSend` 0x4180a0 (radio
silence filter), the Alt+S toggle at 0x4159dc. Multi-crew flag: PLANE flags
bit 0x4 read from the locally extracted retail `FA_2.LIB` PT files; class from
`docs/formats/fa-aircraft.csv`. Nationality 20 and 21 names from the table at
0x4fb2b8; Vietnam missions (`map tviet.T2`) remap raw 14 and 15 to them. Manual:
"RADIO SILENCE?" entry (PDF text, printed page 333) and the two-seater radar
note. Recording presence: `#` stems listed in the FA_2.LIB directory.
