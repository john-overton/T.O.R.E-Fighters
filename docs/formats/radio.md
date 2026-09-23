# Radio metadata

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, updated 2026-09-23. Local FA.EXE SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`.
The reviewed i386 PE image has base 0x400000. Its .data section contains
8-byte pairs of absolute pointers to NUL-terminated phrase and sample stem.
The bounded reader selects only reviewed entries, never executes code, checks
all pointers within file-backed section data, limits text to 127 ASCII bytes
and stems to eight safe characters beginning with ^. Unknown layouts fail closed.
The caller treats import failure as optional radio unavailability.

The reviewed table spans 0x4fef10..0x4ff8f0 in 1.02F and holds 329 pairs
naming 299 distinct stems. `tore_formats::radio::STEMS` lists one address per
stem; where a stem appears in several pairs, the standalone phrase is kept over
a sentence fragment (for example `^RDYCAT` "Ready on the cat" rather than
", ready on the cat"). On 2026-09-23 all 329 pairs were compared between the
1.0 disc build and 1.02F: every pair matches at the single 0x4608 shift.

| Table VA range | Contents |
| --- | --- |
| 0x4fef10..0x4ff168 | Numbers, military letters, colors, contact and waypoint report fragments |
| 0x4ff170..0x4ff288 | Player wing orders, weapon release calls, "You're the Wingleader now" |
| 0x4ff290..0x4ff3d0 | Hit and kill confirmation variants |
| 0x4ff3d8..0x4ff470 | Engagement acknowledgements, tally, hit and damage calls |
| 0x4ff478..0x4ff4c8 | Missile warnings, death cries, ejection, SAM/AAM launch |
| 0x4ff4d0..0x4ff570 | Launch report fragments, mission result lines, friendly-fire complaints |
| 0x4ff5a8..0x4ff688 | Radar, fuel, G strain, feet wet/dry, approaching target |
| 0x4ff690..0x4ff838 | Offensive and defensive coaching |
| 0x4ff840..0x4ff8f0 | Takeoff, catapult, landing and wind reports |

Only `^FIRGUN` has no matching recording in the local FA_2.LIB. The archive
also holds 703 speech recordings in all: 618 with the `^` prefix and 85 with
`#`. Recordings without a pair include callsigns, aircraft and weapon names,
clock positions and compass directions. The importer now keeps every `^`/`#`
`.5K` recording with a stem of up to eight safe characters, about 3 MB in all.
Importing a recording assigns it no meaning; only a reviewed consumer selects
it. Cache marker `TORE_SPEECH_V1` makes older caches reimport.

Say procedure 0x48d780 selects the wing pairs by event and parameters. Playback
0x48d610..0x48d6d2 appends the extension at 0x4ee2c4 when none is present.
The local archive census finds matching .5K entries in FA_2.LIB. This is table
and resource evidence, not filename-based inference of dialogue meaning.
B46 sender/reply behavior and unresolved branches remain in
[the AI specification](../spec/ai.md#b46-wing-command-receiver-contract).
FMENUD.MNU is imported by the existing bounded menu reader. Inspection of its
menu tree found no wing-order submenu. Host shortcuts are independent, documented in [input](../INPUT.md#player-wing-orders).

Cache entries TORE_RADIO_<stem> hold one ASCII phrase; the key names the reviewed
stem. Only listed stems have phrase text. Samples retain original resource names and
are parsed by the existing bounded PCM reader. No retail metadata or audio is
embedded into the application. Old caches keep text-only command operation until
reimport; audio initialization reports which optional airport voices are missing. Unknown report phrases are never mapped to plausible recordings.

The two airport mappings are in the same executable pointer-pair table as the
reviewed wing phrases. The local user-owned `FA_2.LIB` contains both exact
resources. `^CLRLAND.5K` is 4,307 decoded bytes with SHA-256
`4fd166d3538d360562dbe2ead54ba3868972b44711b2358acba919db881735bf`.
`^WELHOME.5K` is 4,032 decoded bytes with SHA-256
`80c55b6973d2c4e8e6bea345c54eacc60a47049b437e788c107aaa6cf8eab3b7`.
The parent review also found the airport consumer: APCommentProc loads pair
0x4ff8c8 at 0x48fbc8 and passes it to the phrase/stem buffer helper at 0x48fbd4.
Later it passes group 0x4ff978 with count 2 to 0x48e150, which selects one of
its adjacent eight-byte pairs, including 0x4ff980, then calls the same helper.
This establishes airport use of these recordings, not the complete original
state, eligibility or timing conditions. Source: the same hash-reviewed EXE and
its local `.local/weapons-research/native/fa-disassembly.txt`.

This establishes the phrase/sample pairs. It does not establish that retail
exposes TORE's authored select, repeat or cancel commands. TORE uses the
clearance recording for a successful player landing request and its repeat, and
the welcome recording after its deterministic landing-completion event and when
that latest reply is repeated. Those
event bindings are fitted. Selection, cancellation, rejection and runway
invalidation remain text only because no matching retail event recording has
been established.

Ejection clips also enter the existing serial speech queue from discrete escape
and cockpit warning transitions. Their reviewed filenames, source call sites
and unresolved speaker assignments are in [ejection source notes](ejection.md).
They do not require speculative phrase-to-speaker mappings.
