# Radar warning receiver tones

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research facts behind the [RWR warning tones](../spec/rwr.md#warning-tones).
Research on 2026-09-28 read FA.EXE 1.02F, SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`, and
FA_2.LIB, SHA-256
`fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`.
Only static disassembly and bounded data reads were used; no original code ran.
The warning-state arrays and their producers were already recovered for flight
music; this page links to them rather than repeating them.

## Recordings

Names at `0x4f3b60` (`&RWRDTCT.5K`), `0x4f3b6c` (`&RWRLOCK.5K`) and `0x4f3b78`
(`&RWRIR.5K`). The only other references are the resource preload list at
`0x4fb97c..0x4fb990`. `&RWRMISS.5K` has no string anywhere in FA.EXE or the
other install files; only the archive directory names it, so this build never
plays it.

| Resource | Decoded bytes | Length at 5,512 Hz | SHA-256 |
| --- | ---: | ---: | --- |
| `&RWRDTCT.5K` | 17,536 | 3.18 s | `9e8e471a5ada5f52d9b5e5271855e545e4e5bcc3ed5a73caa5892946e686c8d8` |
| `&RWRIR.5K` | 5,924 | 1.07 s | `82b76d1704243fedc1f30e1a8fd11f620dec133a1a9320d566f639a8665efeca` |
| `&RWRLOCK.5K` | 8,096 | 1.47 s | `7d850e1262385c0f415bbecec06f732ea461aa622392b6396836237aedebc6f8` |
| `&RWRMISS.5K` | 5,875 | 1.07 s | `51f4d0eb962221642b0c4a32720095db538658f56f3d6c317f5828b517668030` |

Decoded with `tore-extract`; the same run reproduced the `&MPASS.5K` hash in
the [sound notes](sound.md#decoded-recording-identities). A rough amplitude
envelope (55-sample windows, 35% threshold) shows `&RWRDTCT.5K` as about four
beep groups at roughly 0.8 s spacing, `&RWRIR.5K` and `&RWRMISS.5K` as six
even beeps about 0.18 s apart, and `&RWRLOCK.5K` as irregular beeps about
0.25 s apart. These cadences are approximate listening aids, not decoded
fields.

## Tone channels

`InitSound` (`0x433367..0x4333f6`) fills eight 12-byte channel records at
`0x5380b8`: +0 voice handle (-1 idle), +2 pointer to a flag byte, +6 priority
word from `0x4f3c10`, +8 recording pointer from `0x4f3c20`. The service loop
reads each channel's base level from words at `0x4f3c60`.

| Channel | Recording | Flag byte | Priority | Base level | Preference word |
| ---: | --- | --- | ---: | ---: | --- |
| 0 | `&IR1.11K` | `0x4f3c40` | 26 | 255 | weapon lock `0x5718f4` (plus shot percentage) |
| 1 | `&IR1.11K` | `0x4f3c44` | 42 | 255 | weapon lock `0x5718f4` (plus shot percentage) |
| 2 | `&RDRTRY.5K` | `0x4f3c48` | 26 | 200 | weapon lock `0x5718f4` |
| 3 | `&RDRLOCK.5K` | `0x4f3c4c` | 42 | 200 | weapon lock `0x5718f4` |
| 4 | `&RWRIR.5K` | `0x4f3c50` | 25 | 200 | RWR `0x5718dc` |
| 5 | `&RWRLOCK.5K` | `0x4f3c54` | 28 | 200 | RWR `0x5718dc` |
| 6 | `&RWRDTCT.5K` | `0x4f3c58` | 25 | 200 | RWR `0x5718dc` |
| 7 | `&RWRLOCK.5K` | `0x4f3c5c` | 28 | 200 | RWR `0x5718dc` |

## Selection

`?ServiceSounds` (`0x4349d0`, called every pass of the flying loop at
`0x404d46`) clears all eight flags at `0x434e5c..0x434e8f`, then sets at most
one of them. The warning arrays are `_trackLockEndT` `0x58f100`,
`_projLocksOnPlayer` `0x58f1d8` (both u16/i16 by seeker class, index = offset
/ 2) and the quarter-second clock `_currentT` `0x5528c8`; their producers are in
the [flight music notes](music.md#producers).

Gates, any failing leaves every flag clear (`0x435240`/`0x435245`):
`_playerId` `0x520a1c` nonzero, that object's active bit (`+1 & 1`,
`0x434eaa`), object kind 4 (`0x434ebe`), and state byte `+0xe3` (`0x50cf63`)
nonzero (`0x434ecb`). Nothing tests the RWR's installed or damaged state, the
view, or the RWR window.

| Order | Test | Flag set | Site |
| ---: | --- | --- | --- |
| 1 | `_projLocksOnPlayer[3]` (`0x58f1de`) > 0, signed | `0x4f3c5c`, channel 7 | `0x434ed8..0x434ee5` |
| 2 | `_projLocksOnPlayer[2]` (`0x58f1dc`) > 0, signed | `0x4f3c54`, channel 5 | `0x435088..0x435092` |
| 3 | `_trackLockEndT[3]` (`0x58f106`) > `_currentT`, unsigned | `0x4f3c58`, channel 6 | `0x4350a3..0x4350b2` |
| 4 | `_trackLockEndT[2]` (`0x58f104`) > `_currentT`, unsigned | `0x4f3c50`, channel 4 | `0x4350c3..0x4350d2` |
| 5 | otherwise | player seeker channels 0 to 3 | `0x4350e3` onward |

The first match jumps straight to the channel service at `0x435283`, so a
warning also leaves the player's seeker flags clear and silences the growl.
`_searchLockEndT` (`0x58f1c0`) and classes 1 and 4 feed only the music
chooser, never a tone.
The inbound count follows `@PROJSetTarget@4` `0x4c0870`; decoy retargeting in
`_PROJRetargetMissilesOnDevice` (`0x4c3af0..0x4c3c31`, call at `0x4c3bd4`)
moves a decoyed missile off the player through it.

Seeker class is the weapon record byte `+0xb4`. The seeker branch compares it
with 2 at `0x435166` and `0x4351d4` for the IR channels; the radio producers
send the "Atoll" and "Apex" inbound calls for 2 and 3
([radio notes](radio.md)). Class 2 is infrared and class 3 radar, as in the
[cockpit voice spec](../spec/cockpit-voice.md#missile-warnings).

### Lock producer stage

`_NPCWeaponsProc` `0x4736f0` is the weapons procedure of both the AI aircraft
selector (`0x4bd5e3`) and the ground vehicle selector `_GVProc` (`0x473dd2`).
At `0x473887..0x4738f3` it refreshes `_trackLockEndT[class] = _currentT + 4`
when its target is the player, a station is selected, the station record kind
is 7, and stage byte `+0x11d` (`0x50cf9d`) is 3 or more; below 3 it refreshes
`_searchLockEndT[class] = _currentT + 16` instead. The stage comes from
`_PROJServiceWeapon` `0x4c4700` (argument 3, stored at `0x4c4f96`): 1 searches
for a target (`0x4c49cd`), 2 picks a store and runs `_PROJLock` `0x4c2f20`;
when the lock succeeds and `_COLTerrainBlocking` `0x42e4e0` reports a clear
path, the stage becomes 3 with a delay from weapon byte `+0xe5`
(`0x4c4a78..0x4c4a9a`). From 3 the lock is checked again and the shot is taken
through `_PROJFire` `0x4c2170` (`0x4c4e34`); the stage then becomes 4 plus a
round count (`0x4c4dfb..0x4c4e08`), each further round counts down to 4
(`0x4c4e69..0x4c4e74`), and a pass at 4 drops back to 2 or to target search
(`0x4c49a2..0x4c49c8`). So stage 3 and above covers "locked, waiting out the
tracking delay" and the firing sequence. A failed lock or blocked path stays
at 2 (`0x4c48d7`, `0x4c4b0c`, `0x4c4b25`). The call rate of
`_NPCWeaponsProc`, and therefore the refresh rate, was not traced.

## Playback

The service loop `0x435283..0x43546b` runs for every channel:

- Level: base level times `0x5718dc` / 100 for channels 4 to 7
  (`0x435386..0x43539d`), then times Overall `0x5718f0` / 100
  (`0x43539f..0x4353b8`), each step truncating.
- Flag set, no voice: `0x433640(name, level, priority, 0)` at `0x4353cf..0x4353e0`.
- Flag set, voice playing: the new level is written to the voice's base and
  output bytes (`0x4353e5..0x435418`), so slider changes apply at once.
- Flag clear, voice playing: stopped by `0x433ce0` and the handle reset
  (`0x435452..0x43545d`).

Wrapper `0x433640` calls sound request `0x433680` with priority in argument 2,
level in argument 3, argument 4 = 1, source object 0, arguments 6 and 7 = -1,
no pitch terms, argument 13 = 0 and argument 15 = -1. See the
[sound request contract](sound.md#countermeasure-release-sound).

- **Loop.** Argument 4 nonzero clears voice byte `+0x4a` (`0x433b89..0x433ba6`).
  Sample start `0x4359a9..0x4359bf` passes `+0x4a ? 1 : 0` to
  `_AIL_set_sample_loop_count` (import `0x593774`, `wail32.dll`); in the Miles
  API a count of 0 repeats without end. One-shot requests (argument 4 = 0) get
  count 1. Argument 4 therefore also means "loop", in addition to the
  out-of-range exemption recorded in the sound notes.
- **No position.** Source 0 has class 0 and id 0, so the request skips the
  distance refusal (`0x4339d0..0x4339df` to `0x433a9d`) and the voice update
  marks it unpositioned (`0x433e2e..0x433e3d`): no distance falloff (argument 7
  is -1, `0x433f13`), no Doppler term, and the pan comes from voice byte
  `+0x4d` (`0x433eeb`), which is 0 here, so the tone is centered.
- **Overall applied twice.** The per-frame voice update `0x433480` (flying loop
  `0x404eae`, after `ServiceSounds`) recomputes each playing voice's output
  level from its base byte `+0x52` times Overall / 100 (`0x43437e..0x43439b`).
  Because `ServiceSounds` has already multiplied by Overall, the heard level is
  `200 * RWR / 100 * Overall / 100 * Overall / 100`, truncating at each step.
  The seeker channels share this. Defaults (`0x4b2eb3..0x4b2eed`): Overall 75,
  RWR 50, giving 56. The Sound/Music Prefs dialog writes the RWR word from
  control 7 (`0x4a25fa..0x4a2604`). Controls 4 to 8 write Overall, `0x5718f8`,
  weapon lock, RWR and `0x5718e4`, the same order as the Overall, Engine,
  Weapon Lock, RWR and Stall Warn sliders on `SNDPREF.PIC`; the dialog layout
  itself belongs to the sound preferences research.
- **Output.** `0x435a00` sets the Miles volume to output level / 2
  (`0x435a6b..0x435a87`).
- **Voice limit.** With all 16 voices busy, a request takes the quietest voice
  whose priority is at or below its own (`0x4338ca..0x43392d`). A warning at 25
  or 28 can therefore lose its voice to a louder-priority sound such as chaff
  (40) or an explosion (90); the channel keeps its stale handle until the flag
  clears. That starvation is an artefact of the 16-voice mixer, not behaviour
  to reproduce.

## Pause and fades

- **Pause.** In the audio timer `0x4354b0`, `0x435a00` sets every voice's
  volume to 0 while `_timeCompression` `0x5528f8` is `0x7fff` (pause) or
  `0x46ff70` reports a multiplayer pause (`0x435a51..0x435a70`). The looping
  sample keeps running silently. The clock is frozen, so no warning expires
  during pause.
- **Heartbeat fade.** At `0x4349ea..0x434a4d` `ServiceSounds` derives word
  `0x4f3c0c` from `0x580e2c` (doubled, capped at 510, zero below 20) and, when
  nonzero, starts `&HRTBEAT.11K`. The voice update scales every other voice by
  `(255 - 0x4f3c0c / 2) / 255` (`0x433dcc..0x433dfe`), so at the cap the
  warning tone is silent. What `0x580e2c` measures was not identified.

## Open items

| Item | Next step |
| --- | --- |
| Refresh rate of the lock warning, which decides whether it is continuous during a lock | Trace how often the object procedure dispatches selector 5 (`_NPCWeaponsProc`) per object |
| Whether a missile that detonates or is destroyed leaves the inbound count at once or at the next 2 s rebuild | Two detonation paths clear the target beside `0x4c1720` (`0x4c1469..0x4c1470`, `0x4c1eae..0x4c1eb5`); check the remaining removal paths for `PROJSetTarget(0)` |
| Remote human players: their locks write no lock warning here; network paths not checked | Review multiplayer message handlers for `_trackLockEndT` writers |
| Meaning of player state byte 0 (death, ejection) | Trace `_playerId` and `+0xe3` after ejection and destruction |
| Heartbeat fade source `0x580e2c` | Sound or G-effects research on `&HRTBEAT.11K` |
