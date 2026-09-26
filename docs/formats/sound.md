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

## Countermeasure release sound

Research on 2026-09-26, same FA.EXE build and FA_2.LIB, static disassembly
only. [Player-visible result](../spec/countermeasures.md#release-sound).

Device creation `0x4447a0` requests the sound at `0x444842`, only after the
device object exists: `&CHAFF.5K` (`0x4f4e88`) for device kind 0xc and
`&FLARE.5K` (`0x4f4e94`) for kind 0xd. Other kinds request nothing. It has two
callers: the device launcher `0x4c39a0` and the network release message at
`0x46d684`. The launcher's callers are the Insert and Delete handlers
(`0x415c4c`, `0x415c9d`) and the aircraft device schedule (`0x4739bc`), so
player, AI and remote releases share one request per device. The only other
references to the two names sit in the resource name list at `0x4fb918`, which
`0x486010` passes to resource routine `0x4a6ae0` (flags 0x10c) without playing
anything.

Sound request `0x433680` takes 15 stack arguments (`ret 0x3c`). The ones this
request uses:

| Argument | Release value | Use |
| --- | --- | --- |
| 1 | recording name | Upper-cased; `.5K`, `.8K` or `.11K` picks the rate, anything else is refused |
| 2 | 40 | Priority when all 16 voices are busy |
| 3 | 200 | Level on a 0 to 255 scale |
| 4 | 0 | Nonzero keeps a voice that starts out of range |
| 5 | current object `0x4f6fbc` | Source object; for a release, the releasing aircraft |
| 6 | 4,000 | Distance in feet at which the level reaches zero |
| 7 | 100 | Distance in feet within which the level is full |
| 8, 9 | 0, 0 | No pitch offset and no distance-driven pitch |
| 15 | -1 | Take a voice from the 16-voice pool |

For comparison, weapon launch (`0x4c27df`) and the `&BOMB.11K` requests,
including external fuel jettison, pass level 255; flaps pass 75 or 100 and
landing gear 180 or 220.

The source position is re-read every frame, so the sound follows the aircraft.
Distance from the camera position `0x4eb650` uses the approximation `0x4c66cc`
(largest axis plus a quarter of each other axis). A request farther than
20,000 ft is refused; one farther than argument 6 is refused unless argument 4
is set (`0x433a4e..0x433a8c`). Voice update `0x433d80` scales the level
linearly from argument 7 to argument 6 (`0x433f13..0x433f4a`), then by the
effects volume percentage `0x5718f0` (`0x43437e..0x43439b`), one of four volume
words written together at `0x4b2ce3..0x4b2d28`. Pan `0x4343b0` rotates the
camera-relative position by the camera matrix at `0x4eb69c`; preference
`0x4f3ce0` mirrors it. A source equal to the camera's object `0x4eb64c` while
camera flag `0x4eb64a` bit 0 is set has no position: full level and centered
(`0x433a22`, `0x433e4f`). Only cases 0 to 2 of view setter `0x40e470` set that
bit (flags 0x63, 0x41, 0x6b), and case 0 places the eye with a per-resolution
value (`0x52160b`, chosen at `0x4068d5..0x4068f9`). That is the evidence for
reading the bit as a cockpit view; which key selects each case was not traced.

## Decoded recording identities

| Resource | Decoded bytes | SHA-256 |
| --- | ---: | --- |
| `&IR1.11K` | 18,061 | `05acd75d5f8bab63f68f678eb8aa01b3a8a02eb89b519c8572a6c90e28dfaa76` |
| `&AIRPASS.11K` | 9,883 | `090dabd49200d445447691ad509a68c2d5826a57260ce0928d51d972d2b53279` |
| `&MPASS.5K` | 4,112 | `be3fcb3417513223a3b97884a639b599a606a89b413f5a750a636437a1d5b96a` |
| `&SNCBOOM.11K` | 53,705 | `38469e555b45b713c94a72f035b8cc28678486ede3f0884997992f2d1171a42b` |
| `&CHAFF.5K` | 6,343 | `3b8ec4e328fb13dffea8f20632ff43b2e6174f5ef706337b826fdb9909e2749b` |
| `&FLARE.5K` | 13,120 | `30c91c203b8c2e491fd506d780d945b8100b156a745516717bb1e1e8f76c9844` |

[Extraction, import and playback validation](../baselines/flight-sound.md).
