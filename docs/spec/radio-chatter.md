# Radio chatter

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-23. Static review of FA.EXE 1.02F, SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`, its local
disassembly and symbols, the local user-owned `FA_2.LIB` entry listing, and the
retail manual text. The original executable was not run. Byte-level facts and
table addresses live in [radio metadata](../formats/radio.md).

This spec covers when aircraft and other objects speak on the radio during
flight, what they say, who is heard, and the limits a player notices. The
player's wing orders and their replies are specified in
[B46](ai.md#b46-wing-command-receiver-contract); this page adds the chatter that
B46 does not cover and corrects one B46 reply rule. Crew situation comments
(feet wet/dry, approaching target, defensive coaching, "Ease up on the stick",
"He's at your 3 o'clock") and G-strain sounds are in
[cockpit voice](cockpit-voice.md); this page only notes where the radio system
hands off to them. Music scoring is out of scope. Ejection announcements are in
[ejection](ejection.md).

## What the player hears, in one paragraph

Every radio call is a message from one speaker to a set of listeners. The
player hears a call only when the player is one of its listeners, or when the
player made the call. Almost all AI calls go to the speaker's own flight, so
**the player hears wingmen of their own flight and nobody else**: other friendly
flights, enemies, SAM sites and ships are never heard on the radio. The two
exceptions addressed straight to the player are friendly-fire complaints and
the AWACS report. Every call prints a line of text, `Speaker: 'text'`, and
plays its recordings in order. The in-flight **Radio silence** toggle (manual:
"RADIO SILENCE?") drops routine chatter at the moment it is sent but keeps
important calls such as missile launches, as the manual promises.

## Delivery rules

These rules apply to every event below. All are executable-confirmed unless
marked.

**Listeners.** A call is addressed to one of four audiences:

| Audience | Who receives it |
| --- | --- |
| Flight leader | The first living member of the speaker's flight. A speaker in no flight addresses itself, which nobody hears. |
| Wingmen | Every other member of the speaker's flight, only when the speaker is the flight leader. |
| Whole flight | The acting leader plus every other member except the speaker. |
| One object | A named aircraft, such as the player. |

The player hears an AI call only when the player is among the receivers. A
call reaching an AI aircraft is silent.

**The player's own calls.** When the player's own aircraft is the speaker, the
call is printed and voiced once at the moment it is sent, whether or not anyone
receives it. This includes the player's weapon launch, hit, kill and "I'm hit"
calls described below, not only wing orders. It is labelled `YOU`, except that
a call which ends up addressed to the player's own aircraft uses the crew label
below. That happens for whole-flight calls when the player has no other flight
member, so a two-seat player flying alone hears their own "Fox two" or "I'm
hit" as `RIO` or `CO-PILOT`.

**Delay.** Each call is delivered after a fixed delay: none, half a second, two
seconds or five seconds, listed per event. The player's own calls play at send
time.

**Speaker label.**

- A member of a flight is named by the flight's colour and the member's
  position: Red, Blue, Green, Black, White, Orange, Purple or Yellow for the
  first to eighth flight, then one, two, three, up to ten for the member's
  position. Example: `Red two`. A ninth or tenth flight has no colour entry;
  its label is **unknown**.
- Any other object uses its own assigned name if it has one, otherwise the
  second name string of its type, for example `E-3 AWACS Sentry (AIR)`.
- The player is `YOU`. A call the player's aircraft addresses to itself in a
  multi-crew aircraft is labelled `RIO` when the aircraft type is in the
  fighter class (F-14, F-4 and similar) and `CO-PILOT` otherwise (A-6, Su-34,
  B-52, helicopters and similar). In a single-seat aircraft such calls are
  labelled `YOU`.

**Radio silence.** The toggle prints "Radio silence" or "Radio traffic OK". The
original preference bit behind it is set for "traffic OK". Its default comes
from saved preferences and is **unknown**. While silence is on, these calls are
dropped when sent and never reach anyone, including the player's own calls of
those kinds:

- weapon launch, gun and bomb release calls
- hit and kill calls
- engage replies ("Engaging", "Tally Ho" and the rest)
- wingman contact reports
- "I'm hit" calls
- friendly-fire complaints
- the relative-position calls handed off to the comment system

These calls are **never** silenced: wing orders, "Showtime!", waypoint calls,
"You're the Wingleader now", SAM and AAM launch calls, missile-inbound warnings,
death screams, mission accomplished/failure and "almost home", fuel calls, the
AWACS report and datalink messages. A queued call keeps the setting that was in
force when it was sent.

**Variants.** When an event has several recordings, one is picked at random.
A single random roll from 0 to 99 is made when the call is sent. It chooses the
variant (roll modulo the number of variants) and, for the launch-call and
contact-report chances, also that chance; the kill-call chance uses its own
roll. So every listener hears the same variant, and variants
are close to uniform: with 8 variants the first four each come up 13% of the
time and the last four 12%.

**Recordings.** A call's recordings play back to back, in order. A recording
missing from the archive is skipped; the text still prints. The recording
volume comes from a sound setting whose menu control is **unknown**. Whether a
new call cuts off one still playing is **unknown**.

**Cooldowns.** Cooldowns listed as "global" are shared by every speaker in the
mission. They run on game time. Only the waypoint call and the contact report
scale their cooldown with time compression, so those last the same real time
at any compression.

**Vietnam voice set.** A speaker whose nationality is North Vietnamese or
South Vietnamese uses a separate, smaller set of recordings (85 entries in
`FA_2.LIB`). Most radio events map to one or two of those recordings instead of
the full variant set. The text is unchanged. The swap rule is described in
[cockpit voice](cockpit-voice.md#the-second-voice-set); the per-event radio
mapping is only partly read and is **unknown** in detail.

## Events

Each entry gives the trigger, the speaker and who hears it, the variants and
limits, and the provenance. Recordings are listed by resource stem; the files
are `<stem>.5K` in `FA_2.LIB`.

### Weapon launch calls

- **Trigger.** A weapon release. The player's releases are always announced.
  AI releases are announced on one of the two AI weapon-release paths only; which
  AI releases that covers is **unknown**. A release with no target is announced
  only when the released store carries the bomb flag (**unknown**: the check
  reads the current object type, which is probably the released store).
- **Speaker and listeners.** The shooter, to its whole flight, after half a
  second. Silenced by radio silence.
- **What is said.** The first matching rule wins:
  1. AIM-54 Phoenix: "Fox three". This is the only weapon that gets "Fox three".
  2. Free-fall and cluster bombs (bomb flag), and laser-guided stores under a
     condition that is **unknown**: "Bombs away". Global cooldown 4 seconds;
     during the cooldown nothing is said.
  3. Otherwise an unguided store (guns and rockets) first adds "I'm using my
     gun", global cooldown 4 seconds. The recording `^FIRGUN` is absent from
     `FA_2.LIB`, so this is text only. The call then continues with rule 4.
  4. On a 50% roll, if the target is an aircraft: "Fox one" for a
     radar-guided store (so the AIM-7 and AIM-120 get "Fox one") and "Fox two"
     for an infrared store. In every other case: one of "I'm taking a shot",
     "Missile away" or "Firing missile".

  A gun burst therefore prints "I'm using my gun" followed by one of the three
  generic lines. Provenance: executable-confirmed.

### Hit calls

- **Trigger.** A projectile damages an object that survives. Hits on the
  shooter's own side are never announced. When the shooter is an aircraft
  and the weapon is unguided, that shooter announces at most one hit every 8
  seconds.
- **Speaker and listeners.** The shooter, to its whole flight, after half a
  second. Silenced by radio silence.
- **Guided weapon hits.** One of five: "Bullseye", "Impact!", "Oh, yeah!",
  "Alright!", "Good shot!". No further cooldown.
- **Unguided weapon hits.** One of eight: "Bullseye", "He's taking damage",
  "Multiple hits", "Oh, yeah!", "He's getting fragged!", "Eat hot lead!",
  "Debris is flying!", "He's hurtin' now!". Global cooldown 4 seconds.
- Provenance: executable-confirmed.

### Kill calls

- **Trigger.** A projectile destroys an object. Kills of the shooter's own side
  are not announced.
- **Speaker and listeners.** The shooter, to its whole flight, after half a
  second. Silenced by radio silence.
- **Aircraft killed.** A fresh 40% roll picks the generic set: one of twelve,
  "Good hit!", "Good kill!", "Splash one bandit!", "Impact!", "Yee-haw!",
  "Beautiful!", "Oh, yeah!", "He's down for the count!", "Crash and burn!",
  "Wipeout!", "He's breaking up.", "He's goin' down in flames.". Otherwise (60%)
  the call names the type: text "Splash one " plus the victim's full type name,
  recordings `^SPLASH` then `^AC` plus the victim's resource name (for example
  `^ACMIG29`). Helicopters, the V-22 and the blimp, and the aircraft whose
  resource name is `F22`, always get the generic set. `FA_2.LIB` holds 78
  `^AC` recordings; a type without one says only "Splash one". No cooldown.
- **Anything else killed** (ground vehicles, sites, ships, buildings). One of
  ten: "Impact!", "Yee-haw!", "Beautiful!", "Got him!", "Bullseye!", "Hoo-hoo!",
  "Impact! Boom! Oh, yes!", "We have a fireball!", "History!", "Woooh!". Global
  cooldown 4 seconds, which only a bomb kill starts.
- Provenance: executable-confirmed. The four "Splash one MiG!" entries (two
  recordings) and "Mission objective destroyed!" are in the table, but no
  radio event selects them in this build.

### "I'm hit"

- **Trigger.** An aircraft takes damage from an enemy projectile, or one with no
  owner. For gun and cannon rounds (the bullet flag) each aircraft calls at most
  once every 8 seconds; other hits call every time.
- **Speaker and listeners.** The hit aircraft, to its whole flight, with no
  delay. Silenced by radio silence. When the player is hit, the player's
  aircraft makes this call as `YOU`.
- **Variants by attacker.** An aircraft attacker: one of five, "I'm hit",
  "I'm taking damage", "Get this guy off me", "I'm getting scorched", "I'm
  taking heat". An anti-aircraft gun: one of four, "I'm hit", "I'm taking
  damage", "I'm taking AAA", "I'm eating lead". Anything else: "I'm hit" or
  "I'm taking damage".
- Provenance: executable-confirmed.

### Death calls

- **Trigger.** An AI aircraft is destroyed. The player's aircraft never makes
  this call. A preference bit together with the aircraft kind can suppress it;
  that bit's menu meaning is **unknown**.
- **Speaker and listeners.** The dying aircraft, to its whole flight, with no
  delay. Not silenced.
- **Variants.** A fixed-wing aircraft with an ejection seat: one of six,
  "Aaargh...", "Oh, sh...", "Yaaaah", "Ejecting", "I'll see you in Hell!",
  "Punching out!". Any other aircraft: one of the first three.
- The choice is random. It is **unknown** whether it agrees with an actual
  ejection; see [ejection](ejection.md).
- Provenance: executable-confirmed.

### SAM and AAM launch calls

- **Trigger.** An AI aircraft receives a missile launch warning (the delayed,
  target-only warning of [B47](ai.md#b47-threat-warnings-countermeasures-and-reason-priority)),
  passes its warning gate, carries a countermeasure dispenser, and the launcher
  is on the other side.
- **Speaker and listeners.** The targeted aircraft, to its whole flight, after
  half a second. Not silenced.
- **What is said.** "AAM launch" when the launcher is an aircraft, otherwise
  "SAM launch".
- B47 says a same-side launch sends a radio message. In this call the side test
  selects **opposite**-side launchers; the B47 sentence needs rechecking.
- Provenance: executable-confirmed.

### Missile-inbound warnings to the player

- **Trigger.** The player is warned of a missile fired at them (one second
  after launch, per B47), and the player's aircraft type is multi-crew.
  Single-seat aircraft get no spoken warning.
- **Speaker.** The player's own `RIO` or `CO-PILOT`, after half a second. Not
  silenced.
- **What is said.** By the missile's seeker, not its actual name: radar,
  "Apex inbound, drop chaff!" (global cooldown 6 seconds); infrared, "Atoll
  inbound, drop flare!" (global cooldown 6 seconds); anything else,
  "Missile inbound! Break!" (no cooldown).
- This is the backseater speaking through the radio system; the other
  backseater comments belong to the comment-system pass.
- Provenance: executable-confirmed.

### Engage replies (correction to B46)

- **Trigger.** The first wingman accepts an order to attack a target, or to
  attack on contact.
- **Speaker and listeners.** That wingman, to its flight leader, after two
  seconds. Silenced by radio silence.
- **What is said.** For a ground or sea target: "Engaging". For an aircraft
  target, or for attack on contact: one of nine, "Engaging", "Where? Wait, I
  see 'em", "Showtime!", "Alright, let's get 'em", "Yahoo!", "Tally Ho",
  "I'm on him", "I'm going for it", "I'm going after him". B46's
  "Engaging"-only statement covers only ground and sea targets.
- Accepting the order also blocks that wingman's own contact reports for
  20 seconds and records the ordered target as already reported.
- Provenance: executable-confirmed.

### "Showtime!"

The first wingman accepting "protect me" says "Showtime!" to its leader after
two seconds. Not silenced. Executable-confirmed, consistent with B46.

### Wingman contact reports

- **Trigger.** An AI aircraft (never the player's) takes a new aircraft target
  in one particular attack state (original state 31; its name is **unknown**).
  Only the first two aircraft of a group, or an aircraft in no group, report.
  The target must not be in the
  original states 1 to 17 or 22 to 30 (**unknown** names; probably ground and
  landing states).
- **Limits.** Each aircraft reports at most once every 15 seconds, and never
  reports the same target twice in a row.
- **Speaker and listeners.** The reporting aircraft, to its whole flight, after
  half a second. Silenced by radio silence.
- **What is said.** "Contact, " then:
  - A size word when the target is within 15 nautical miles. The group counted
    is the target plus members of its flight within 10,000 feet of it and
    heading within 45 degrees. One aircraft: no word. Two: "pair of" (40%),
    no word but ", two-ship formation" after the noun (30%), or "multiple"
    (30%). Three to twelve: the number, such as "three" (50%), or "multiple"
    (50%). More than twelve: "multiple".
  - A noun, again only within 15 nautical miles. When the target is close
    enough to identify, the text names the type, such as "MiG-29s", but the
    voice still says "bandits", except for MiG-17, MiG-19 and MiG-21, which
    have their own recordings. Otherwise "bandit" or "bandits". The
    identification range is 8 statute miles scaled by a visibility percentage
    whose inputs are **unknown**.
  - The clock position relative to the listener: "your two o'clock", with
    " high" or " low" when the target is above or below. The o'clock and
    high/low thresholds are **unknown**.
  - The distance, rounded to whole nautical miles, when at least 1: ", 12
    miles".
  - ", please advise" when the reporting wingman is under medium or tight
    formation control, or a state flag is set.

  Example: "Contact, pair of bandits, your two o'clock high, 12 miles, please
  advise." Distances of 1 to 10, 20 and 30 miles have single recordings; other
  distances are voiced digit by digit, then "miles".
- Provenance: executable-confirmed, except the noted unknowns.

### Waypoint calls

- **Trigger.** A flight leader moves on to its next waypoint, other than at
  mission start (the first second).
- **Speaker and listeners.** The leader, to its wingmen, after half a second.
  Not silenced. A player leading a flight makes this call as `YOU`; a player
  flying as an AI leader's wingman hears it. Global cooldown 5 seconds.
- **What is said.** "Proceed to waypoint " or "Inbound to waypoint " (50/50),
  the waypoint letter (the second entry of the flight's waypoint list is
  Alpha, the next Bravo, up to Kilo),
  ", bearing " plus the bearing from the leader in whole degrees, then
  ", climb to", ", maintain" or ", descend to" by the height difference rounded
  to thousands of feet, then " angels " plus the waypoint altitude in thousands
  of feet. Numbers up to twelve are single words; larger numbers are digits, so
  a bearing of 270 is voiced "two seven zero" and a bearing of 5 "five". The
  last number word of the bearing and of the altitude uses a falling-intonation
  recording.
- Past Kilo the letter table runs into colour names ("Red"), and the first
  list entry would read "twelve". Whether a player can reach either is
  **unknown**.
- Provenance: executable-confirmed.

### "You're the Wingleader now"

Sent when flight leadership passes to another aircraft, five seconds later, to
the new leader. It is voiced only when the previous leader is still alive to
send it. The exact situations that pass leadership are **unknown**. A player
who becomes leader hears it. Not silenced. Executable-confirmed.

### Friendly-fire complaints

- **Trigger.** The player's projectile damages a same-side aircraft that
  survives, within 52,800 feet (10 statute miles) of the player.
- **Speaker and listeners.** The victim, to the player only, after two seconds.
  Silenced by radio silence. Global cooldown 6 seconds.
- **What is said.** One of eight: "What the hell are you doing?", "Watch out!",
  "Are you nuts?", "Are you crazy!?", "Who's side are you on?", "I'm a good
  guy, remember!", "Get off of me!", "I'm on your side!".
- Provenance: executable-confirmed. AI-on-AI friendly fire is never
  complained about.

### Fuel calls

Fuel level is judged against the flight home. With no home base the game
reports only running dry. Otherwise, with the endurance at the best cruise
throttle:

| Level | Condition |
| --- | --- |
| Joker | Endurance below the time to fly home plus 10 minutes |
| Bingo | Endurance below the time home plus 5 minutes |
| Fumes | Endurance below 4 minutes |
| Out | No fuel left |

Each aircraft announces each level once. The worst new level is announced, and
announcing it also marks every milder level as done.

- **AI aircraft** announce to their flight leader, with no delay: "Joker fuel",
  "Bingo fuel", "I'm running on fumes", "I'm out of fuel.  I'm punching out".
  An unidentified condition can skip the check (**unknown**).
- **The player**, only in a multi-crew aircraft, hears their `RIO` or
  `CO-PILOT` say "Joker fuel", "Bingo fuel", "We're running on fumes", "We're
  out of gas". A single-seat player gets no fuel call.
- Fuel calls are never silenced.
- Provenance: executable-confirmed. The fuel-flow and home-time inputs are
  approximate for implementation purposes; see the source notes.

### AWACS report

- **What is said.** From the nearest same-side aircraft or ground object that
  carries a particular type flag and whose radar range reaches the player (the
  game's own failure text calls this "AWACS/GCI"): "No bandits detected" or
  "Nothing to report" (50/50) when its
  radar sees no enemy aircraft. "Bandits, visual range" (text "Bandit, visual
  range", with no recording, for a single one) when the nearest seen enemy is
  within 15,000 feet of the player. Otherwise "Contact, your eleven o'clock low,
  35 miles", with clock and range from the player and no size or noun. With no
  AWACS in reach the text line "No AWACS/GCI information available" prints and
  nothing is voiced. The speaker is the AWACS, to the player only, never
  silenced.
- **Trigger.** **Unknown.** The routine has no call site or pointer in the
  executable image; it may be reached only through a named-symbol interface.
  Next step: search mission, menu and resource modules for the symbol name
  `_SAYAwacsReport@0`, and the key tables for a request key.

### Datalink and rearm messages

The player's supplemental radar link commands print "Supplemental air-air radar
link on.", "Supplemental air-ground radar link on.", "Supplemental radar link
broken." with `&SQACK2`/`&SQACK1` acknowledgement tones, or "Supplemental radar
not responding." with `^BEEP2`. Rearming prints " Plane Re-Armed and
Re-Fueled" with no recording. All are addressed to the player and never
silenced. The keys and link conditions are outside this pass.

### Calls produced by the comment system

These calls are voiced by the radio system but triggered by the crew comment
system; their triggers are in [cockpit voice](cockpit-voice.md):

- "Mission accomplished!", "MISSION FAILURE!" (five different recordings,
  one chosen at random), "We're almost home!". Each is said once per mission.
  None is voiced while a mission flag is set (cockpit voice identifies it with
  fortress missions).
- "He's at " or "He's at your " plus clock position, optionally ", turning
  left" or ", turning right" (when an angle carried by the call is between 20
  and 160 degrees; its meaning is **unknown**) or ", heading away"; and a range
  call such as "2 miles, three o'clock, in range", which adds ", in range" at
  2 miles or less. Only the "heading away" form has a located sender.
- Feet wet/dry, approaching target, defensive coaching and "Ease up on the
  stick" are in the comment system only.

After any radio call is voiced the game marks the radio busy for 3 seconds
(scaled by time compression). The comment system waits for that; radio calls
themselves do not.

### Player orders

Player orders are printed and voiced as `YOU` at send time. B46 has the
details. This pass confirms the attack order wording: "Attack" or "Evade", then
the target noun, then the clock position; "Clear my six" or "Watch my tail"
(50/50) for protect me; "Attack" alone for attack on contact; "Disengage" for
the cancel order. The producers of "Evade" and "Bug out" were not located in
this pass.

## Event to recording table

| Event | Recordings (stems) | Selection | Silenced? |
| --- | --- | --- | --- |
| Launch, Phoenix | `^FOXTHR` | fixed | yes |
| Launch, bombs | `^BOMBAWY` | 4 s global cooldown | yes |
| Launch, unguided prefix | `^FIRGUN` (absent, text only) | 4 s global cooldown | yes |
| Launch, radar / IR at aircraft | `^FOXONE` / `^FOXTWO` | 50% roll | yes |
| Launch, generic | `^IMSHOT`, `^MISSAWY`, `^FIRMISS` | 1 of 3 | yes |
| Hit, guided | `^BULLS1`, `^IMPACT`, `^OHYEAH`, `^ALRIGHT`, `^GDSHOT` | 1 of 5 | yes |
| Hit, unguided | `^BULLS1`, `^HEDAMGE`, `^MULTHIT`, `^OHYEAH`, `^FRAGGED`, `^HOTLEAD`, `^DEBRIS`, `^HURTIN` | 1 of 8, 4 s global cooldown | yes |
| Kill, aircraft, generic | `^GOODHIT`, `^GDKILL`, `^SPLBNDT`, `^IMPACT`, `^YEEHAW1`, `^BEAUT1`, `^OHYEAH`, `^DNCNT`, `^CRSHBRN`, `^WIPEOUT`, `^BRKUP`, `^GOFLAM` | 40%, then 1 of 12 | yes |
| Kill, aircraft, named | `^SPLASH` + `^AC<resource>` | 60% | yes |
| Kill, other | `^IMPACT`, `^YEEHAW2`, `^BEAUT2`, `^GOTHIM`, `^BULLS2`, `^HOOHOO`, `^OHYES`, `^FIRBALL`, `^HISTORY`, `^WOOH` | 1 of 10, 4 s global cooldown after bomb kills | yes |
| I'm hit, by aircraft | `^IMHIT1`, `^IMDMGE1`, `^OFFME`, `^SCORCH`, `^HEAT` | 1 of 5 | yes |
| I'm hit, by AAA | `^IMHIT2`, `^IMDMGE2`, `^IMAAA`, `^EATLD` | 1 of 4 | yes |
| I'm hit, other | `^IMHIT2`, `^IMDMGE2` | 1 of 2 | yes |
| Death, ejection seat | `^AARRGH`, `^OHSH`, `^YAAAAAH`, `^EJECT`, `^SEEHELL`, `^PUNCH` | 1 of 6 | no |
| Death, other | `^AARRGH`, `^OHSH`, `^YAAAAAH` | 1 of 3 | no |
| SAM / AAM launch | `^SAMLCH` / `^MISSLCH` | by launcher | no |
| Missile inbound, radar | `^APEXCHF` | 6 s global cooldown | no |
| Missile inbound, IR | `^ATOLFLR` | 6 s global cooldown | no |
| Missile inbound, other | `^MISSBRK` | fixed | no |
| Engage, ground or sea | `^ENGAGE` | fixed | yes |
| Engage, aircraft or free | `^ENGAGE`, `^ISEEEM`, `^SHWTIME`, `^GETEM`, `^YAHOO1`, `^TALLYHO`, `^ONHIM`, `^IGO`, `^IGOAF` | 1 of 9 | yes |
| Protect me reply | `^SHWTIME` | fixed | no |
| Contact report | `^CONTACT`, `^PAIROF` / `^NUMnn` / `^MULTPLE`, `^BANDIT(S)` or `^MIG17(S)`/`^MIG19(S)`/`^MIG21(S)`, `^2SHFORM`, `^YOUR`, `^CLOCKnn` or `^CLCKnnD`, `^HIGH`/`^LOW`, `^MILEnn` or digits with `^MILE(S)`, `^PLSADVS` | composed | yes |
| Waypoint | `^PROCTO` / `^INBDTO`, `^WAYPNT`, `^MLTRY-A`..`^MLTRY-K`, `^BEARING`, digits, `^DSCNDTO` / `^MAINTN` / `^CLIMBTO`, `^ANGELS`, digits | composed, 5 s global cooldown | no |
| New wingleader | `^WNGLDR` | fixed | no |
| Friendly fire | `^WHTHELL`, `^WTCHOUT`, `^YOUNUTS`, `^YOUCRZY`, `^WHOSIDE`, `^IMGOOD`, `^GETOFF`, `^IMYOUR` | 1 of 8, 6 s global cooldown | yes |
| Fuel, AI | `^JOKER`, `^BINGO`, `^IMFUMES`, `^OUTFUEL` | by level, once each | no |
| Fuel, player crew | `^JOKER`, `^BINGO`, `^WEFUMES`, `^OUTGAS` | by level, once each | no |
| AWACS | `^NOBADET` / `^NOTOREP`, `^BANSVIR`, contact composition | 50/50, by range | no |
| Mission end | `^MISSACC`; `^NOTPLSD`, `^NOMEDAL`, `^BLEWIT`, `^MESSUP`, `^SERIOUS`; `^ALMSTHM` | once each; failure 1 of 5 | no |

Every stem above is present in the local `FA_2.LIB` except `^FIRGUN`.

## Unresolved branches and next research

1. **Which AI weapon releases announce.** The AI release service announces on
   one path only. Next: name the release states of that service.
2. **AWACS report trigger.** No caller found. Next: search non-executable
   modules and key tables for the symbol name.
3. **Radio silence default.** Next: trace the preference file loader.
4. **Clock and high/low thresholds, visibility scaling.** Next: read the
   relative-position and visibility helpers named in the format notes.
5. **Contact report state 31 and target state ranges.** Next: map these AI
   state numbers to mission-facing names (shared with B46's open states).
6. **Leadership hand-over situations** and the death-call preference bit.
7. **Vietnam voice set mapping per event.** Next: finish reading the
   formatter; the stems it can produce are listed in the format notes.
8. **Playback overlap.** Whether a new call interrupts one in progress. Next:
   read the sample start routine's handle behaviour.
9. **Recording volume setting.** Next: find the menu control that writes the
   volume word the playback reads.
10. **Evade, Bug out and relative-position ("He's at") producers.** The
    comment-system triggers are in [cockpit voice](cockpit-voice.md).

## Source notes

- Poster and routing: `MSGSend` 0x4180a0, receipt `MSGReceive` 0x4185a0
  (player poll in `MessagesToPlayer` 0x414523), observer `SAYMsg` 0x48d350.
  Say procedure `_PLANESayProc` 0x48d780, jump table 0x48e0bc.
- Senders: launch `PROJAdd` 0x4c109f; hit/kill and friendly fire
  `PROJDamageProc` 0x4c1a57 / 0x4c1b2e; warnings, SAM/AAM, death, I'm hit,
  replies `PLANEEventProc` 0x49e181, 0x49e244, 0x49e73e, 0x49e8ea, 0x49ee80,
  0x49efb0; contact `GRPSetStateTarget` 0x45fa3d; waypoint `WPSetupCurrent`
  0x499412; fuel `SAYLowFuelMessage` 0x48eb20 with levels from
  `PLANECheckFuel` 0x49fb70.
- Radio silence toggle: `FlightKey` 0x4159dc; key per
  [flight controls](../FLIGHT-CONTROLS.md). Manual: "RADIO SILENCE?" option.
- Full byte-level notes: [radio metadata](../formats/radio.md).
