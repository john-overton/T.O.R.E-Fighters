# Fighters Anthology recorded music and actuator audio

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Research notes, research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Requirements, gates and remaining
> work described here are research-mode scope; they are not acceptance gates
> for gameplay. Parity is measured by expression of feature, see
> [AGENTS.md](../../AGENTS.md). Player-visible behaviour is specified in
> [docs/spec/](../spec/).


## Playback decision

On 2026-09-14 the user selected original recorded PCM playback, without MIDI
conversion or synthesis. This supersedes the earlier XMI/synth milestone for this
FA slice. General raw archive extraction still preserves XMI when requested;
`--music` and the app select PCM and MUS only. No external soundfont, synth,
reference checkout or new dependency is required.

## Source evidence

Reviewed FA.EXE SHA-256:
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`.
FA.SMS SHA-256:
`e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0`.
These are the existing native-flight reviewed build, not the USNF executable.
SMS names were read through `tools/extract_native_flight.py::symbols`; executable
sections were read through its bounded section mapper. Nothing was executed.

Repeat the static disassembly with LLVM, retaining output locally:

```sh
llvm-objdump -d --x86-asm-syntax=intel --start-address=0x4328b0 --stop-address=0x43317a gameassets/fighters-anthology/FA.EXE
llvm-objdump -d --x86-asm-syntax=intel --start-address=0x451b60 --stop-address=0x451df9 gameassets/fighters-anthology/FA.EXE
```

`ScoreUpdate` at `0x432ca0` waits for the current digital sound/MIDI sequence to
finish before interpreting further score instructions. Its dispatch table is at
`0x432f54`. `_dMusic` selects the digital path: format strings at `0x4f3cf0` and
`0x4f3ce4` are `%03d.11K` and `%03d.XMI`. Digital playback calls `DMusicOn`
at `0x4329a0`; MIDI calls `MusicOn` at `0x4329e0`. We use only the former's
recorded resources, through our own PCM mixer.

`ShellMusicUpdate` at `0x432f80` selects an initial random index and then cycles
each context's table. Data tables are main `0x4f3c70` (8), briefing `0x4f3c90`
(6), win `0x4f3ca8` (3), loss `0x4f3cb8` (5). The bounds are confirmed by the
consumer's random/modulo operations. Shared constants in `tore-formats::music`
preserve their ordering, including the repeated AIR14A briefing entry.

| Resource group | Archive | Recovered content |
| --- | --- | --- |
| Numbered AIR PCM | FA_4B.LIB | 77 recordings, 34,922,940 bytes |
| Shell recordings | FA_4D.LIB | 22 recordings, including table selections and FINAL6 (not assigned a new trigger) |
| Situation scripts | FA_2.LIB | NORMAL, AIR, DANGER, DECK, LAUNCH, HOME, EJECT, SUCC, VALK |

All 99 selected recordings are unsigned PCM8 mono without WAV headers; 11,025 Hz
continues the reviewed `.11K` rate convention. Their total is 48,679,357 bytes.
WAV previews preserve every sample, add headers/padding, and do not resample.
The PCM reader also accepts bounded RIFF PCM8 mono and honors its header rate.
Other RIFF codecs/channels/sample widths are rejected, not treated as raw noise.

## MUS data grammar

`tore-formats::music::Score` reads only the inert PL/PE CODE section. It checks
reachable instruction/operand boundaries, absolute CODE-relative jump targets,
ASCII alphanumeric prefixes and chance/track operands. Limits: 64 KiB CODE,
4,096 decoded instructions, 32-byte prefix search and 1,024 instructions per
phrase request. Nonproductive cycles stop with a diagnostic. Unreachable trailing
bytes remain in the original extracted file and are counted in the report.
The reviewed scripts each have one prefix; a different second prefix is rejected
pending review. No imported x86 or native module is executed.

| Opcode | Meaning |
| --- | --- |
| 00 | Skip |
| 01–F8 | Track number |
| F9 | Request host reevaluation |
| FA | Chance byte and absolute u32 jump |
| FB | Chance byte and track number |
| FC | Stop |
| FD | Count byte and random-choice track numbers |
| FE | Absolute u32 jump |
| FF | NUL-terminated filename prefix |

FA's three-digit PCM naming is preserved, including unresolved references:

| Score | Available / referenced unique PCM phrases | Missing |
| --- | --- | --- |
| NORMAL | 43 / 43 | None |
| AIR | 28 / 29 | AIR029.11K |
| DANGER | 21 / 22 | AIR041.11K |
| DECK | 3 / 3 | None |
| LAUNCH | 5 / 5 | None |
| HOME | 4 / 4 | None |
| EJECT | 4 / 5 | AIR015.11K |
| SUCC | 5 / 5 | None |
| VALK | 0 / 1 | VALK001.11K |

No alternate spelling, MIDI render or unrelated track fills those gaps.
`VALK01.XMI` exists but is outside the chosen playback path.
The extraction report separates successfully extracted files from missing score
references: `complete: true` establishes extraction success, not a complete score
library. `missing_pcm` uses the non-excluded source archive catalog, before
`--include` filtering; it does not promise a deliberately narrowed output is playable.

## Runtime scope and boundaries

The app imports the same music resource predicate as the CLI. Music archives
FA_4B/FA_4D are optional; missing archives produce diagnostics and silence in
affected contexts. Cache marker TORE_MUSIC_V1 requires older caches to refresh.
Copying optional archives into media after a partial import requires `--import`
again. Music PCM resources are excluded from the retained theater/airframe resource map
and kept as bytes in the mixer, rather than four-byte floats for every sample.

Main menu uses the recovered main playlist; Quick Mission Creator and the
development viewer use the briefing playlist. Free flight for either aircraft
starts M_NORMAL. Returning to a context selects a fresh start; restarting flight
restarts its score and clears aircraft loops. Context resets and mapping the
development creator/viewer to briefing are authored. AIR003 is no longer an
arbitrary menu loop. Gain (0.16 music), resampling and sample-boundary transitions
remain authored; no native mixer/device or RNG/timing parity is claimed.

All nine scripts are prepared at audio initialization. The retail in-flight
selection rules are in [in-flight score selection](#in-flight-score-selection)
and [flight music](../spec/flight-music.md). Before 2026-09-23 flight selected
NORMAL until the player ejected, then EJECT; imported score data and missing-track diagnostics are unchanged. F9 is retained as a host flag; no missing mission, carrier,
threat or combat event is invented to respond to it. Score and playlist state
belongs to audio, independent of authoritative 120 Hz simulation. A local xorshift
RNG chooses phrases; it does not consume simulation RNG. Music uses wall-clock
audio samples and is not sped up by flight time scaling.

Mute and flight pause retain playheads, with no hidden catch-up. UI clicks have a
separate bounded voice pool and remain audible while the flight menu is paused.
Local effects pools permit eight voices each; [spatial effects](../audio.md) have
a separate sixteen-voice limit. Ordinary playback resolves clips before
opening the audio device and does not allocate clips, read files or run a synth
in its callback. Missing selected phrases and exhausted score budgets stop that
music context; diagnostics are emitted outside the callback.

Main-menu M / Pref still controls saved music preference. In-flight Sound still
controls effects only; a full flight volume/settings mixer remains deferred.
The [startup preference rule](../spec/menu-music.md) keeps the initial Music
setting independent of which subsystem owns the imported sample buffers.

## In-flight score selection

Reviewed FA.EXE 1.02F, SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`. Static
LLVM disassembly only; nothing executed. Player-visible rules live in
[flight music](../spec/flight-music.md). Positions are fixed8 feet, speed
`cp+0x34` is fixed8 ft/s, `_currentTime` `0x5528e0` is game seconds (u16),
`_currentT` `0x5528c8` is `currentTicks >> 6`, quarter seconds (u16; ticks are
256 per second, `0x486bc4..0x486be0`).

### Call sites

`?FlyingLoop` calls `_MISSIONCheckSuccess` at `0x404caf`, then `_ChooseScore` at
`0x404eb3` immediately followed by `ScoreUpdate` at `0x404eb8`, every loop.
`_MISSIONInit2` `0x480a30` calls `_SAYInit2` `0x48d2f0` then `_ChooseScoreInit`
at `0x480acc`. `ScoreUpdate` is also called from `@DialogUpdate@4` `0x4884b3`.

### Score table and rank

Pointer table `0x4f47a0`, nine entries, index = rank (higher wins):
0 `m_normal.MUS`, 1 `m_deck.MUS`, 2 `m_home.MUS`, 3 `m_danger.MUS`,
4 `m_air.MUS`, 5 `m_launch.MUS`, 6 `m_eject.MUS`, 7 `m_succ.MUS`,
8 `m_valk.MUS`. This differs from `tore_formats::music::SCORES` order
(NORMAL, AIR, DANGER, DECK, LAUNCH, HOME, EJECT, SUCC, VALK).

### Globals

| Address | Meaning |
| --- | --- |
| `0x53da18` dword | Current score rank, -1 = none |
| `0x53da14` word | Next check time, `_currentTime` seconds |
| `0x53da20` word | `_combatScoreTime`, AIR hold end |
| `0x53da24` byte | SUCC already started |
| `0x53da1c` byte | HOME already started |
| `0x4f3ba8` byte | `_scoreNeedCheck`, set by F9 |
| `0x4f4798` byte | `?valkyriesScore`, static, initial 0 |
| `0x552fcc` byte | `_home`, home condition reached |
| `0x552fe0` / `0x552fc8` byte | Success / failure radio call made |
| `0x551698` dword | `_missionSucceeded` (1, 0, -1) |
| `0x58f1c0` u16[5] | `_searchLockEndT`, `_currentT` units, by class |
| `0x58f100` u16[5] | `_trackLockEndT`, by class |
| `0x58f1d8` i16[5] | `_projLocksOnPlayer`, counts by class |
| `0x58f1d0` word | Next `_PROJLockUpdate` time, seconds |

### `_ChooseScoreInit` `0x441c60..0x441c8e`

Current = -1. `al = _CallMissionProc(_missionName, 1)`; both once flags
(`0x53da24`, `0x53da1c`) = `al`. Next check = 0, `_combatScoreTime` = 0.
`_CallMissionProc` `0x481940` returns 1 for an empty name; message 1 calls the
mission DLL callback `0x4f6fb8` if loaded, else `_MISSIONSuccess` `0x481a70`.
Only the low byte is stored, so any nonzero result (1 or -1) disables SUCC and
HOME for the flight.

### `_ChooseScore` `0x441c90..0x441f73`

Guards, any failing returns: music initialised `0x4eb5f0`; `_curScreen`
`0x520a50` == 0x10 requires `?flightMusicVol` `0x5718d8` != 0, other screens
require `?otherMusicVol` `0x5718e8` != 0; `_timeCompression` `0x5528f8` !=
0x7fff; `_playerId` `0x520a1c` != 0. Then `GetCurObj(_playerId)`. If next check
> `_currentTime`, `PutCurObj` and return.

Target: `esi = _objPtrs[cp+0xee]` only when `cp+0xee` != 0, `(target+9 ^
cp+9) & 0x80` (opposite side) and `target+1 & 1` (active). If `ScorePlaying()`
(`scoreStart` `0x4f3bd4` != 0) is false, current = -1.

First match, in order:

| Test | Rank |
| --- | --- |
| `valkyriesScore` != 0 | 8 |
| `_MISSIONSucceededForThisPlayer()` > 0 and `0x53da24` == 0 | 7 |
| `cp+0` kind != 4 | 6 |
| `cp+0xe3` in 8..0x12 | 5 |
| target kind 4 and `_Dist(cp+0x11, target+0x11)` < `0x9c4000` (40,000 ft) | 4 |
| `_currentTime` < `_combatScoreTime` | 4 |
| target kind 2 or 4 | 3 |
| any `_searchLockEndT[1..4]` or `_trackLockEndT[1..4]` > `_currentT` (unsigned) | 3 |
| any `_projLocksOnPlayer[1..4]` > 0 (signed) | 3 |
| `_home` != 0 and `0x53da1c` == 0 | 2 |
| `cp+0xe3` in 1..7 or 0x16..0x1a | 1 |
| otherwise | 0 |

Switch rule `0x441ec7..0x441f72`: if `_scoreNeedCheck`, clear it; if chosen ==
current return, else current = -1. If chosen > current: current = chosen,
`ScoreOff`, `RMAccessHandle(table[chosen], 0x104)`. On success set `0x53da1c`
for rank 2 or `0x53da24` for rank 7, `ScoreOn(handle, 1)`, next check =
`_currentTime + 1`. On failure next check = `_currentTime + 10` (current stays
set to chosen). Chosen <= current without the F9 flag: no change.

### Producers

- `_combatScoreTime`: `_DAMAGEDoHit@12` `0x40f987..0x40f9a0`, when `_curId` ==
  `_playerId`, `= _currentTime + 30`. Only direct caller is
  `?PROJDamageProc` `0x4c192b`; `0x4c18fc..0x4c191e` skips the call when the
  shooter exists, shooter `+0x10 & 0x80` is clear and sides match.
- `_searchLockEndT` / `_trackLockEndT`: `_NPCWeaponsProc` (`0x4736f0`)
  `0x473887..0x4738f3`: when `cp+0xee` == `_playerId`, station `cp+0x11c` !=
  -1, `0x452770(station)` returns a record with byte 0 == 7; class =
  record `+0xb4`. `cp+0x11d` >= 3 writes track = `_currentT + 4` (1 s), else
  search = `_currentT + 16` (4 s).
- `_projLocksOnPlayer`: `@PROJSetTarget@4` `0x4c0870` decrements the old
  target's class slot (floor 0) and increments for the player, then undoes the
  increment when the projectile type name is `AIM120.J` (`0x4f465c`) and
  distance > `0x76ac00` (30,380 ft, 5.0 nm). `_PROJLockUpdate@0` `0x4c0960`
  rebuilds the counts every 2 s over active kind-6 objects with `+0xe4` ==
  `_playerId`, same AIM-120 exclusion. Class = projectile type `+0xb4`.
- `?ServiceSounds` `0x435088..0x4350d0` reads classes 2 and 3 of these arrays
  for warning tone flags `0x4f3c54`, `0x4f3c58`, `0x4f3c50`.
- `_missionSucceeded`: `_MISSIONCheckSuccess` `0x486860`, single-player/host
  only, every 4 s, `= _CallMissionProc(_missionName, 1)`.
  `_MISSIONSucceededForThisPlayer` `0x4868b0` negates it when the player's side
  differs from player slot 0 (`0x4eb610` table).
- `_home`: `_SAYInit2` clears it, sets it (and `0x552fe0`) when success > 0 at
  init. `_PLANECommentProc` `0x48ed55..0x48edb4`, every 4 s while success > 0
  and `_home` == 0: `_home = 0x481b80()`; if set and not `_OnTheGround`, sends
  message 0x21. `0x481b80`: `_APHomeAirport` object `+0xe6`, distance <
  `0xa50000` (42,240 ft) and `cp+0x15` (Y) < `0x4e2000` (20,000 ft); no home
  airport returns false. `_PLANESayProc` `0x48e065` also sets `_home` when
  message 0x21 is spoken. Radio strings: 0x1f `Mission accomplished!`
  `^MISSACC`, 0x20 `MISSION FAILURE!` `^NOTPLSD`, 0x21 `We're almost home!`
  `^ALMSTHM` (pointer pairs at `0x4ff4f8`, `0x4ff500`, `0x4ff528`).
- `valkyriesScore`: only writer `@FlightKey@4` `0x415b3e`, toggles then calls
  `ScoreOff`. Reached for key code `0x2f02` at `0x4148c9`. FlightKey requires
  `cp+0xe3` != 0, kind 4 and active.
- `_ServicePlayer` state entries used by the chooser ranges: 1 (ground, idle),
  7 (catapult-type position, `APTakeoffType` == 7, speed < 1 ft/s, heading
  within `0x1554` = 30 degrees, `0x4173a4..0x4173d2`), 7 to 8 when `_throttle`
  `0x5451f4` > 100 (`0x4165e8..0x4165f6`), 0x11 when state 1..0x12 and speed
  >= `0x700` (7 ft/s) on the ground (`0x41731b..0x417339`), 0x12 airborne
  within `0x61a800` (25,000 ft) of an airport with gear bit `+0x16f & 0x40`,
  speed <= `0x3b9ff` and altitude < ground + `0xfa000` (4,000 ft)
  (`0x41740d..0x4174b0`), else 0x1f. Landing: `APLanding` enters 0x16
  (arrested) or 0x17 on touchdown, 0x19, 0x1b when stopped; `_ServicePlayer`
  enters 0x1a while rolling after landing (flag `0x4000000`) and 1 when stopped.
- Object kinds (`_T_AddObj` dispatch `0x4a7a1c`): type 1 -> kind 0, type 3 ->
  kind 2, type 5 -> kind 4 (aircraft), type 7 -> kind 6 (projectile).

### ScoreUpdate details relevant to transitions

`ScoreUpdate` `0x432ca0` returns while the current digital phrase is still
playing (`0x432a90`), and does nothing while paused (`0x7fff`) or MP paused
(`0x46ff70`) or with flight music volume zero. Otherwise it interprets until a
track: F9 (`0x432df3`) sets `_scoreNeedCheck` and continues, so the next phrase
starts in the same update and the chooser acts on the next loop. FC dispatches
to `ScoreOff` (`0x432f12`), clearing `scoreStart`. Dispatch table `0x432f54`:
F9 `0x432df3`, FA `0x432dff`, FB `0x432e2c`, FC `0x432f12`, FD `0x432e4e`, FE
`0x432e70`, FF `0x432e79`. Names are prefix + `%03d.11K` (`0x4f3cf0`) or
`%03d.XMI` (`0x4f3ce4`): VALK asks for `valk001`, which neither `VALK001.11K`
nor the present `VALK01.XMI` satisfies.

`ScoreOff` `0x432c70` clears `scoreStart` and stops the digital phrase
immediately (`DMusicOff` `0x432bd0`) or the MIDI sequence (`MusicOff`
`0x432c00`). `ScoreOn` `0x432c30` calls `ScoreOff`, stores start and handle
flag, zeroes offset and prefix and clears `_scoreNeedCheck`. Other `ScoreOff`
callers: `usnfmain` `0x404682`, `FlyingLoop` `0x40511f` (flight exit),
`_FlightMenu` `0x475e54` (key `0x1002`), `_SoundPrefs` `0x4a26d0` (flight music
volume set to 0).

### Script structure (FA_2.LIB, decoded)

| Script | F9 placement | Termination |
| --- | --- | --- |
| M_NORMAL | After each chance phrase in the 29..172 body; the 179..203 run (tracks 90, 91, 92, 94, 7, 9, 31, 38, 44) has none | Jumps to 29, loops |
| M_AIR | After each phrase | Jumps to 17, loops |
| M_DANGER | After each phrase | Jumps to 17, loops |
| M_DECK | After each phrase | Jumps to 23, loops |
| M_LAUNCH | None | FC after 50% 7, 90% 9, 25% 44, 25% 31, 50% 38, 25% 44 |
| M_HOME | None | FC after random(26, 16), 75% 25, 75% 26, 50% 40 |
| M_EJECT | After each phrase | random(10, 19), then loops random(22, 39), random(15, 22, 39) |
| M_SUCC | None | FC after random(3, 11, 20, 49, 16) |
| M_VALK | None | 100% track 1 forever, prefix `valk` |

Phrase durations (bytes / 11,025): LAUNCH 7 = 68.8 s, 9 = 46.1, 44 = 45.6,
31 = 23.8, 38 = 45.1; HOME 26 = 46.9, 16 = 43.5, 25 = 49.1, 40 = 34.6; SUCC
3 = 25.3, 11 = 24.5, 20 = 16.9, 49 = 19.7, 16 = 43.5.

## Speed brakes

FA `FMBrakes` at `0x451d70` selects these strings before calling sound playback
at `0x451df3`: released `0x4f6964` = `&flapcls.5K`; deployed airborne
`0x4f6970` = `&flapopn.5K`; deployed with contact predicate true `0x4f697c` =
`&squeal.5K`. TORE now dispatches these on actual fixed-tick brake state changes
for F18.PT and RAFALE.PT. Only explicit hybrid `research.on_ground` supplies the
ground branch; arbitrary height samples do not establish contact. Full native
wheel-brake/hold behavior, gains and timing remain unverified.

The shared aircraft resolver already included flap sounds and now includes
SQUEAL. Duplicate desired-state commands and burner commands do not trigger
speed-brake cues. The existing hook mapping to &HOOK remains an open separate
actuator audit: native flap/hook routines reference flap recordings and their
argument polarity requires further review.

See [extraction](../EXTRACTION.md) and [acceptance evidence](../baselines/audio.md).

[Escape behaviour](../spec/ejection.md) does not depend on score availability.
