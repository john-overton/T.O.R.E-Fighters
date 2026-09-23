# Flight sound recording selection

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research on 2026-09-23 used FA.EXE 1.02F, SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`,
and FA_2.LIB, SHA-256
`fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`.
Only bounded data inspection and static disassembly were used.

## Reviewed evidence

`InitSound`, `0x433367..0x4333ef`, copies eight recording pointers from
`0x4f3c20` into channels associated with tracking/lock flags. The first two
pointers both equal `0x4f3b9c`, the string `&IR1.11K`. Their flag pointers are
`myIRTracking` (`0x4f3c40`) and `myIRLocked` (`0x4f3c44`). The next two point
to `&RDRTRY.5K` and `&RDRLOCK.5K`. Neither IRTRY nor IRLOCK is referenced by
this executable's selector. Their presence in the archive did not establish
that they were the correct recordings.

`ServiceSounds`, `0x435158..0x435232`, requires an eligible mounted weapon,
uses signature 2 to obtain a hit-chance-derived percentage, caps it at 100,
and routes signature 2 plus flag `0x10000` to the IR channels. The other branch
sets the radar-named channels. `0x4352b2..0x4353e0` scales channel gain by the
lock-volume preference, additionally scales IR by the percentage, suppresses
tracking while locked, halves the tracking gain, smooths the IR amplitude,
and loops the selected recording. Host playback must reproduce the audible
relationship, not the original mixer state machine. The full hit-chance and
clock contracts remain unresolved.

`ServiceSounds`, `0x434ef6..0x43506b`, selects `&AIRPASS.11K` at
`0x434f93`, `&MPASS.5K` at `0x434fd9`, and `&SNCBOOM.11K` at
`0x434f8c`. The boom branch compares speed against `SpeedOfSound` and tests
view state. The nearby `&BPASS.5K` branch tests projectile flag `0x80`.
Complete original observer eligibility and source history were not recovered.

The bounded PCM reader already supplies the rates in the
[behavior specification](../spec/sound.md). The normal combat dependency import
loads these names from the user's archive at runtime. Existing caches without
them require the normal reimport path. No PCM, WAV, disassembly or generated
retail derivatives belong in version control.

## Decoded recording identities

| Resource | Decoded bytes | SHA-256 |
| --- | ---: | --- |
| `&IR1.11K` | 18,061 | `05acd75d5f8bab63f68f678eb8aa01b3a8a02eb89b519c8572a6c90e28dfaa76` |
| `&AIRPASS.11K` | 9,883 | `090dabd49200d445447691ad509a68c2d5826a57260ce0928d51d972d2b53279` |
| `&MPASS.5K` | 4,112 | `be3fcb3417513223a3b97884a639b599a606a89b413f5a750a636437a1d5b96a` |
| `&SNCBOOM.11K` | 53,705 | `38469e555b45b713c94a72f035b8cc28678486ede3f0884997992f2d1171a42b` |

[Extraction, import and playback validation](../baselines/flight-sound.md).
