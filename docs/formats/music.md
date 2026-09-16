# Fighters Anthology recorded music and actuator audio

> **Research notes — research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Requirements, gates and remaining
> work described here are research-mode scope; they are not acceptance gates
> for gameplay. Parity is measured by expression of feature — see
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

All nine scripts are prepared at audio initialization, but only NORMAL is
selected by flight. F9 is retained as a host flag; no missing mission, carrier,
threat or combat event is invented to respond to it. Score and playlist state
belongs to audio, independent of authoritative 120 Hz simulation. A local xorshift
RNG chooses phrases; it does not consume simulation RNG. Music uses wall-clock
audio samples and is not sped up by flight time scaling.

Mute and flight pause retain playheads, with no hidden catch-up. UI clicks have a
separate bounded voice pool and remain audible while the flight menu is paused.
Each effects pool permits eight voices. Ordinary playback resolves clips before
opening the audio device and does not allocate clips, read files or run a synth
in its callback. Missing selected phrases and exhausted score budgets stop that
music context; diagnostics are emitted outside the callback.

Main-menu M / Pref still controls saved music preference. In-flight Sound still
controls effects only; a full flight volume/settings mixer remains deferred.

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
