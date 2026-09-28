# Sound/Music Prefs

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research specification, 2026-09-28, research mode. Build: FA.EXE 1.02F, SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`, with the
user's FA_1.LIB and FA_2.LIB. Evidence is static disassembly, the FA.SMS symbol
names and the decoded `SNDPREF.DLG`, `SNDPREF.PIC` and slider and toggle art.
Nothing was run and no retail session was observed. Addresses and record
layouts live in [the sound format notes](../formats/sound.md#sound-preferences).

Every rule below is **native** provenance (reconstructed from the reviewed
executable's control flow and data) unless it says otherwise.

## What the player sees and does

**Pref > Sound...** on the main menu bar, and **Pref > Sound...** on the in-flight
menu bar, open the same **SOUND/MUSIC PREFS** dialog. The manual only says it
adjusts "the volume of music and sound effects" and defers to the Install Guide.

The dialog has nine vertical sliders and one lever switch:

- **SOUND VOLUME**: OVERALL, ENGINE, WEAPON LOCK, RWR, STALL WARN, RADIO MSG,
  each from OFF at the bottom to MAX at the top.
- **MUSIC VOLUME**: IN-FLIGHT and OTHER, OFF to MAX.
- **MISC PREFERENCES**: STEREO SEPARATION MAGNITUDE, MIN to MAX, and SWAP
  LEFT/RIGHT CHANNELS, a lever that is up for YES and down for NO.
- **Cancel** and **OK** buttons. OK is drawn as the default button.

The player drags a red knob with the mouse. Pressing on the knob picks it up
where it was grabbed. Pressing anywhere else in a slider's track makes the knob
jump so its centre sits under the pointer, and it then follows the pointer until
the button is released. The pointer may leave the track while dragging; the knob
stops at the ends. Sliders are continuous, not detented: the tick marks painted
on the panel are decoration. There is no keyboard or wheel control of a slider,
and moving a slider makes no sound.

Clicking the lever flips it with a short four-frame animation and a switch
click.

Nothing takes effect until **OK**, with one exception: dragging OTHER changes the
loudness of the music playing in the menus at once, as a preview. **Cancel**
discards every change and puts the menu music back to its saved loudness.
**Enter** is OK and **Escape** is Cancel. No other key does anything.

The settings are saved to the game's configuration file and survive a restart.
From the main menu they are saved as soon as the dialog closes. From flight they
are saved when the player leaves the flight menu.

In flight, the flight menu pauses the game before the dialog opens, and it
stays paused until the player leaves the flight menu. Everything the flight was
playing, music included, is silent while paused. Only the dialog's own clicks
are heard.

## Numbers

### Layout

Coordinates are in the 640 × 480 canvas. The dialog's top-left corner is
centred horizontally and placed a third of the way down: x = (screen width −
382) / 2, y = (screen height − 376) / 3, which is **(129, 34)** at 640 × 480. The
same rule applies to the in-flight dialog on a larger flight screen.

| Element | Art | Position |
| --- | --- | --- |
| Background with all labels | `SNDPREF.PIC`, 382 × 376 | (129, 34) |
| Cancel | Shell action button, ordinary style, label `Cancel` | top-left (413, 330) |
| OK | Shell action button, default style (`ACTDFT` art), label `OK` | top-left (413, 364) |
| Swap lever | `TOGGLE00.PIC` (YES) to `TOGGLE04.PIC` (NO), all drawn from one top-left | (453, 250) |
| Swap lever click area | | x 449 to 483, y 250 to 297 (35 × 48) |

Both buttons are sized to their label: label width plus 24 pixels, at least 61
pixels, with a 20-pixel-tall click area from the button's top-left. They use the
same pieces as the other shell dialogs.

Each slider is a track and a knob. The track column is drawn over the empty slot
in the background: `SLIDETOP.PIC` (34 × 9) at its top, `SLIDEMID.PIC` (34 × 8)
repeated below it, and `SLIDEBOT.PIC` (34 × 15) at its bottom. The knob is
`SLIDERV.PIC` (26 × 30).

| Slider | Track column x | Track column y | Knob x | Knob top at MAX | Knob top at OFF/MIN | Click area |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| OVERALL | 183 | 117 to 206 | 189 | 123 | 172 | (189, 123) 34 × 79 |
| ENGINE | 245 | 117 to 206 | 251 | 123 | 172 | (251, 123) 34 × 79 |
| WEAPON LOCK | 289 | 117 to 206 | 295 | 123 | 172 | (295, 123) 34 × 79 |
| RWR | 333 | 117 to 206 | 339 | 123 | 172 | (339, 123) 34 × 79 |
| STALL WARN | 377 | 117 to 206 | 383 | 123 | 172 | (383, 123) 34 × 79 |
| RADIO MSG | 421 | 117 to 206 | 427 | 123 | 172 | (427, 123) 34 × 79 |
| IN-FLIGHT | 166 | 278 to 367 | 172 | 284 | 333 | (172, 284) 34 × 79 |
| OTHER | 224 | 278 to 367 | 230 | 284 | 333 | (230, 284) 34 × 79 |
| STEREO SEPARATION | 317 | 264 to 353 | 323 | 270 | 319 | (323, 270) 34 × 79 |

Track pieces: top cap at the column's first row, the middle tiles from 8 rows
lower for 67 rows (the last tile cut short), the bottom cap 75 rows below the
top cap. The knob sits 6 pixels right of the column's left edge.

### Slider values

| Rule | Value |
| --- | --- |
| Range | 0 (OFF or MIN, knob at the bottom) to 100 (MAX, knob at the top) |
| Knob travel | 49 pixels |
| Knob position shown for a value v | knob top = MAX row + floor(49 × (100 − v) / 100) |
| Value from a dragged knob d pixels below the MAX row | v = 100 − floor((100 × d + 48) / 49) |
| Distinct values reachable by dragging | 50 (100, 97, 95, 93, ... 2, 0) |
| While dragging | The knob follows the pointer pixel by pixel; the value follows the knob |

Some defaults (80, 60, 50) are not among the 50 draggable values, so once moved
they cannot be set back exactly. The values stored for a fresh install are:

| Control | Fresh-install value | Knob top shown at 640 × 480 |
| --- | ---: | ---: |
| OVERALL | 75 | 135 |
| ENGINE | 80 | 132 |
| WEAPON LOCK | 60 | 142 |
| RWR | 50 | 147 |
| STALL WARN | 80 | 132 |
| RADIO MSG | 95 | 125 |
| IN-FLIGHT | 75 | 296 |
| OTHER | 75 | 296 |
| STEREO SEPARATION | 80 | 279 |
| SWAP LEFT/RIGHT CHANNELS | NO | lever down |

A configuration file that is missing, of the wrong size or of another version
gives these values. The configuration file already present in the reviewed
user install (`EA.CFG`) holds exactly these values.

### Swap lever

| Rule | Value |
| --- | --- |
| YES | Lever up, `TOGGLE00` |
| NO | Lever down, `TOGGLE04` |
| Changing to YES | Frames 03, 02, 01, 00 |
| Changing to NO | Frames 01, 02, 03, 04 |
| Frame time | 26/256 s, about 102 ms, so about 0.41 s for the four frames |
| Sound | `&SWITCH.11K`, full level (255) before OVERALL, starting as frame 02 appears |
| Input during the animation | None; the dialog waits for the animation to finish |

The whole 35 × 48 click area flips the lever in either state.

## What each slider controls

Levels below are on the game's 0 to 255 scale; the output loudness is
proportional to the final level. "× OVERALL" means multiplied by OVERALL / 100
after the level is limited to 255. All multiplications are linear on these
scales, not in decibels.

| Slider | Sounds it scales | Law |
| --- | --- | --- |
| OVERALL | Every sound effect, every UI click, radio speech and the digital music | final level = level × OVERALL / 100 |
| ENGINE | The engine loop of every aircraft within 20,000 ft (the player's and others'), the player's afterburner loop, afterburner light-off, engine start and stop, and the player's wind, turbulence, tyre roll, runway grind and damaged-engine loops | level × ENGINE / 100, then × OVERALL |
| WEAPON LOCK | The player's own seeker tones: the IR growl `&IR1.11K` (searching and locked) and the radar `&RDRTRY.5K` / `&RDRLOCK.5K` | IR base 255, radar base 200; × WEAPON LOCK / 100; then × OVERALL twice |
| RWR | The four threat-warning loops: `&RWRIR.5K`, `&RWRDTCT.5K` and `&RWRLOCK.5K` (IR and radar lock) | base 200 × RWR / 100, then × OVERALL twice |
| STALL WARN | The stall warning loop `&STALLWR.5K` and the stalled loop `&STALL.5K` | `&STALLWR.5K` at STALL × 140 / 100, `&STALL.5K` at STALL × 255 / 100, then × OVERALL |
| RADIO MSG | All radio and intercom speech: wingmen, other flights, tower and AWACS, and the player's own crew (RIO or co-pilot) | RADIO × 255 / 100, then × OVERALL |
| IN-FLIGHT | The situation scores during flight | IN-FLIGHT × 255 / 100, then × OVERALL |
| OTHER | Music everywhere outside flight: menus, briefing, creators, debrief and campaign screens | OTHER × 255 / 100, then × OVERALL |
| STEREO SEPARATION | How far positioned sounds are panned | See [stereo](#stereo) |
| SWAP LEFT/RIGHT | Mirrors every pan | See [stereo](#stereo) |

Details a player can hear:

- **OVERALL twice for WEAPON LOCK and RWR.** These tones are scaled by OVERALL
  once when their level is worked out and again when the voice is updated each
  frame. At OVERALL 75 they play at 56% of their WEAPON LOCK or RWR level, not
  75%. At the defaults (OVERALL 75, WEAPON LOCK 60) the IR lock growl at full
  shot quality plays at 255 × 0.60 × 0.75 × 0.75 ≈ 86.
- **IR growl** is further scaled by shot quality and halved while only
  searching, as described in the [sound specification](sound.md). The radar
  tones are not.
- **ENGINE at OFF** does not start the engine-class loops at all, for any
  aircraft. Because the same pass also plays the touchdown tyre squeal
  (`&SQUEAL.5K`) and the belly-landing scrape (`&CRASH.5K`), those are silent
  too at ENGINE OFF, although their own level does not follow ENGINE.
- **Afterburner light-off** plays at ENGINE × 240 / 100. **Engine start** plays
  at max(throttle %, 50) × ENGINE / 50 and **engine stop** at max(previous
  throttle %, 50) × ENGINE / 80, each limited to 255.
- **The player's ejection shout** plays at full level, scaled only by OVERALL,
  not by RADIO MSG.
- **Only OVERALL** scales: explosions and impacts, gunfire and weapon launch,
  bomb release and fuel tank jettison, gear, flaps, hook, bay doors and speed
  brake, chaff and flares, passing aircraft and missiles, the sonic boom, the
  G-load heartbeat, and the menu and dialog clicks.
- **Music and OVERALL.** The music is the game's recorded (digital) score, so
  OVERALL scales it too. Turning OVERALL down lowers the music as well.

Which RWR recordings play for which threat is owned by the
[RWR specification](rwr.md).

### Stereo

Sounds that come from a place in the world are panned by their bearing from the
camera. A bearing behind the listener is mirrored to the front, so the pan angle
a is always between 90 degrees left and 90 degrees right. STEREO SEPARATION s
then widens or narrows it:

| s | Pan angle heard | Hard left or right from |
| ---: | --- | --- |
| 0 (MIN) | 0 for everything: mono | never |
| 25 | a / 2 | never; the widest pan is half way |
| 50 | a | 90 degrees |
| 75 | a × 2.56 | 36 degrees |
| 80 (default) | a × 2.875 | 32 degrees |
| 100 (MAX) | a × 4.125 | 22 degrees |

The rule: above 50, a + (s − 50) × a / 16; below 50, a + (s − 50) × a / 50; each
rounded towards zero in whole degrees and limited to 90. Sounds with no place in
the world are centred and are not affected by STEREO SEPARATION: the player's
own aircraft while the view is a cockpit view, UI clicks, speech, music, the
seeker and RWR tones and the stall warnings.

SWAP LEFT/RIGHT CHANNELS at YES mirrors every pan, positioned or fixed, so a
sound heard on the left is heard on the right. NO is the game's normal channel
order.

## When changes take effect

| Setting changed | From the main menu | From the in-flight menu |
| --- | --- | --- |
| OTHER | Live while dragging; kept on OK, reverted on Cancel | Stored; heard on return to the menus |
| IN-FLIGHT | Stored; used from the next flight | Used when flight resumes after leaving the flight menu |
| OVERALL | Music and new sounds from OK | Everything when flight resumes |
| ENGINE, WEAPON LOCK, RWR | Next flight | Loops pick up the new level when flight resumes |
| STALL WARN | Next flight | A stall warning that was sounding restarts at the new level when flight resumes |
| RADIO MSG | Next flight | From the next spoken line |
| STEREO SEPARATION, SWAP | Next sound played | When flight resumes |

## Edge cases

- **OTHER at OFF** when the dialog closes outside flight: after half a second
  (128/256 s) every sound stops, the menu music included, and no menu music
  plays while OTHER is at OFF. Raising it again later lets the menu music carry
  on with its next phrase.
- **IN-FLIGHT at OFF** set in flight: the score stops at once and no score is
  chosen while it is at OFF. Raising it again starts a fresh choice, as the
  [flight music specification](flight-music.md) describes. Set from the main
  menu, the next flight has no music.
- **Dragging OTHER in flight** has no audible effect, because the paused flight
  is silent. The flight music comes back at the IN-FLIGHT level on resume.
- **Cancel** after a drag of OTHER outside flight restores the saved loudness at
  once.
- A value outside 0 to 100 in a hand-edited configuration file is shown clamped
  by the dialog and saved clamped on OK. What such a value does before that was
  not examined.

## Implementation in TORE

Implementation mode, 2026-09-28, requested by John: the retail dialog in place
of the two placeholder Pref rows (Music and Effects On/Off), with every control
hooked to the in-game mixer.

| Component | Behaviour in TORE | Provenance |
| --- | --- | --- |
| Dialog | `SNDPREF.PIC`, `SLIDERV.PIC` and `TOGGLE00..04.PIC` at the positions above; Cancel and OK from the shared action-button art. The slots are the ones painted in `SNDPREF.PIC`: the whole picture is redrawn each frame, so the track pieces the original uses to erase a knob are not drawn (drawn over the picture they left a second slot bottom, John 2026-09-28) | spec-derived; track pieces opinionated |
| Slider values | 0 to 100, the pixel mapping above; press on the knob grabs it, press on the track centres the knob and drags | spec-derived |
| Lever | Four frames of 26/256 s, `&SWITCH.11K` on frame 02, input ignored while it moves | spec-derived |
| When changes apply | OK applies and saves; only OTHER is heard while dragging; Cancel and Escape restore; Enter is OK | spec-derived |
| In flight | Pref > Sound... opens the same dialog over the paused flight menu; new levels are heard when flight resumes | spec-derived |
| Defaults | OVERALL 75, ENGINE 80, WEAPON LOCK 60, RWR 50, STALL WARN 80, RADIO MSG 95, IN-FLIGHT 75, OTHER 75, STEREO 80, SWAP NO | spec-derived |
| Loudness | Each slider scales its sounds linearly by its level over its default, so TORE's existing mix is what the defaults sound like. MAX is therefore louder than TORE was before, and the original's absolute levels are not claimed | opinionated (agent, 2026-09-28) |
| OVERALL | Every effect, UI click, speech and music; the seeker and RWR tones twice | spec-derived |
| ENGINE | The player's engine and afterburner loops, other aircraft's engine loops and the engine start and stop sounds | spec-derived; TORE has no wind, turbulence, tyre, grind or light-off sounds yet |
| WEAPON LOCK, RWR, STALL WARN, RADIO MSG | Seeker tones; [RWR warning tones](rwr.md#warning-tones); both stall warnings; all speech queued as radio or crew voice | spec-derived |
| IN-FLIGHT, OTHER | Flight scores; menu and briefing music. IN-FLIGHT at OFF stops the score and nothing is chosen | spec-derived |
| Stereo | The angle law above on every positioned sound, with swap negating the pan; centred sounds stay centred. TORE keeps its own equal-power pan of the resulting angle's sine | spec-derived law; pan curve fitted |
| Saved file | `sound-v1.conf` in the application data directory, not `EA.CFG`. A profile saved before the dialog existed carries its Music Off to both music sliders at OFF and Effects Off to OVERALL at OFF, once | opinionated (agent, 2026-09-28) |
| Keyboard and controller | Tab or Left and Right move a focus outline over sliders, lever and buttons; Up and Down move a slider 5 levels, PageUp and PageDown 20, Home and End to the ends; Space works the focused control; the wheel moves the slider under the pointer | opinionated (agent, 2026-09-28): the original has none |
| Main menu M key | Removed with the placeholder Music row it toggled | opinionated (agent, 2026-09-28) |

Not reproduced, by agent decision: OTHER at OFF stopping every sound half a
second after the dialog closes, and ENGINE at OFF also silencing the touchdown
squeal and belly scrape. Both are side effects of how the original is built
rather than settings a player chooses.

## Unknown and next research

- **Absolute pan direction.** The research shows that YES mirrors the normal
  order, but not which side a negative bearing reaches in the normal order.
  Next step: trace the sign of the bearing routine `_Angles` against the camera
  matrix, or compare with a retail recording.
- **Multiplayer.** The flight menu sets the pause state on entry, but whether a
  networked game really stops, and what the other players hear, was not traced.
  Next step: follow `MPPaused` and the flight menu's multiplayer branches.
- **Dialog shadow.** The dialog is drawn with the shell's standard drop shadow
  pieces; their art and offsets were not decoded here. Next step: decode
  `ShadowBox` `0x40cfe0` and its `shadowLL`, `shadowUR`, `shadowLR`, `shadowH`,
  `shadowV` pieces, shared with every shell dialog.
- **Button labels and fonts.** The Cancel and OK label strings and the font
  chosen for each button role come from the shared action-button code and were
  not re-read for this dialog.
- **The 320 × 200 variant** (`SOUND320.DLG`, used below 640 pixels wide) was not
  decoded. It is outside the remake's canvas.
- **Wind loop.** `&WIND.11K` is one of the ENGINE-scaled player loops; the
  condition that starts it was not traced.

## Source notes

- Build identity, addresses, the DLG record layout, the mixer consumers and the
  art hashes are in [the sound format notes](../formats/sound.md#sound-preferences).
- The manual (`.local/missile-update/manual.txt`) names the menu items only and
  defers details to the Install Guide, which is not available.
- The IN-FLIGHT and OTHER split, the zero-volume stops and the score rules are
  consistent with [the music format notes](../formats/music.md) and
  [flight music](flight-music.md).
- The radio speech volume rule was already recorded in
  [the radio notes](../formats/radio.md); this spec only places it under the
  RADIO MSG slider.
- `&TOGGLE1.5K`, extracted beside this dialog, is not used by it. The lever uses
  `&SWITCH.11K`.
