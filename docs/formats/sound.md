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

## Sound preferences

Research on 2026-09-28, same FA.EXE 1.02F and FA_2.LIB, plus FA_1.LIB SHA-256
`657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f` and FA.SMS
SHA-256 `e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0`
(symbol names). Static disassembly and bounded resource decoding only; nothing
was executed. [Player-visible result](../spec/sound-prefs.md).

### Globals and persistence

Nine words and one byte. The words are 0 to 100; the swap byte is 0 or 1.

| Global | Slider | Default | `EA.CFG` offset |
| --- | --- | ---: | --- |
| `overallVol` `0x5718f0` | OVERALL | 75 | word `0xe5` |
| `engineVol` `0x5718f8` | ENGINE | 80 | word `0xe7` |
| `lockVol` `0x5718f4` | WEAPON LOCK | 60 | word `0xe9` |
| `rwrVol` `0x5718dc` | RWR | 50 | word `0xeb` |
| `stallVol` `0x5718e4` | STALL WARN | 80 | word `0xed` |
| `radioVol` `0x5718ec` | RADIO MSG | 95 | word `0xef` |
| `flightMusicVol` `0x5718d8` | IN-FLIGHT | 75 | word `0xf1` |
| `otherMusicVol` `0x5718e8` | OTHER | 75 | word `0xf3` |
| `stereoSeparation` `0x571c00` | STEREO SEPARATION | 80 | word `0xf5` |
| `_stereoSwap` `0x4f3ce0` | SWAP LEFT/RIGHT | 0 (NO) | byte `0xe4` |

`UCONFIG_Initialize` `0x4b2bd0` loads `EA.CFG` (`0x50c844`) through
`UCONFIG_load_EA_CFG` `0x4b2930`, which accepts only a 347-byte (`0x15b`) file
whose first dword is `0x24`. Accepted values are copied at
`0x4b2cd1..0x4b2d51` without range checks; otherwise the defaults above are
written at `0x4b2eb3..0x4b2f42`. The local user install's `EA.CFG` holds exactly
those defaults. `UCONFIG_save_EA_CFG` `0x4b2980` (reached through
`_WriteConfig` `0x41e8e0`) writes the same layout. `_ChooseActivity` calls it
right after the dialog returns (`0x4a0de9..0x4a0dee`), and `_FlightMenu` calls
it when the flight menu closes (`0x475e84`).

### Dialog

`_SoundPrefs` `0x4a2480` loads `SNDPREF.DLG` when the screen width
(`0x55c06a`) is at least 640, otherwise `SOUND320.DLG`. It is called from the
main menu (`_ChooseActivity` `0x4a0de9`, menu code `0x207`) and the flight menu
(`_FlightMenu` `0x475348`, menu code `0x302`). The flight menu has already set
`_timeCompression` to `0x7fff` (paused) at `0x474828` and restores it on close
(`0x475e7d`). After the dialog the flight menu loop continues (`0x47534d`).

`SNDPREF.DLG` (extracted SHA-256
`46a74e184df1ff2e6b6650c5dd35b1f44a387983a450fae6e0d2446366f37475`) is a PE
module whose CODE section holds a header and a chain of item records.
`_DialogSetup` `0x487a63` walks them with the per-type sizes at `0x487e5c`
(type 0 action 38 bytes, 7 vertical slider 48, 8 toggle 31, `0xa` ends).
Header: preload `_SndPrefPreload`, origin (128,55), size (0,0), background name
`SNDPREF`. Size 0 takes the background's 382 × 376. `_SndPrefPreload`
`0x4897d0` calls `_TopCenterDialog` `0x489710`, which replaces the origin with
x = (screen width − 382) / 2, y = (screen height − 376) / 3: (129,34) at
640 × 480. Item numbers count from 1:

| Item | Type | Record (dialog-local) | Meaning |
| --- | --- | --- | --- |
| 1 | action, Escape role 2, `_cancelString` | (284,296), width 0 | Cancel |
| 2 | action, Enter role 1, `_okString` | (284,330), width 0 | OK |
| 3 | toggle | draw (324,216); both hit boxes (320,216,35,48) | `_stereoSwap` |
| 4 | slider | knob x 60, track (60,89,34,79) | `overallVol` |
| 5 | slider | knob x 122, track (122,89,34,79) | `engineVol` |
| 6 | slider | knob x 166, track (166,89,34,79) | `lockVol` |
| 7 | slider | knob x 210, track (210,89,34,79) | `rwrVol` |
| 8 | slider | knob x 254, track (254,89,34,79) | `stallVol` |
| 9 | slider | knob x 298, track (298,89,34,79) | `radioVol` |
| 10 | slider | knob x 43, track (43,250,34,79) | `flightMusicVol` |
| 11 | slider | knob x 101, track (101,250,34,79) | `otherMusicVol` |
| 12 | slider | knob x 194, track (194,236,34,79) | `stereoSeparation` |

Slider record fields from the item start: `+0xa` horizontal flag (0), `+0xb`
inverted flag (1, maximum at the top), `+0xc` value, `+0xe` maximum (100),
`+0x10` knob position, `+0x14` knob size (26 × 30), `+0x18` track box, and
`+0x24`, `+0x28`, `+0x2c` press, change and release callbacks (all null here).
The DLG values (80, 30, 50, 50, 80, 70, 40, 70, 80) are overwritten by
`_DialogSetValue` `0x489430` before display. The `dialog_geometry` example
reports byte `+0x17` of an action as an action id. `DialogUpdate`
`0x488a82..0x488aea` uses it as the key role (1 or 4 answer Enter, 2 or 3
answer Escape), and `_DrawAction` `0x489c00` draws role 1 with the
default-button `ACTDFT`/`ACTDFD` art. A zero width becomes the label width plus
24, at least 61 (`0x489f11..0x489f31`), with a 20-pixel-tall hit box.

`_SoundPrefs` sets items 3 to 12 from the globals (`0x4a24a8..0x4a254d`), then
loops on `GetKey`/`DialogUpdate`. Each pass compares item 11 with its last value
and calls `MusicVolume` `0x432b40` when it changed (`0x4a2582..0x4a25a0`).
`DialogUpdate` returns the activated item number; 1 and 2 end the loop. Only 2
copies items 3 to 12 back (`0x4a25bb..0x4a268f`). A changed `stallVol` also
stops `stallSnd` `0x4f818c` and `stallWarnSnd` `0x4f8190` so they restart at the
new level. After either button, outside flight (`_curScreen` `0x520a50` ≠
`0x10`) it calls `MusicVolume(otherMusicVol)`, and when that is 0 waits 128
ticks (`_timerTicks`, 256 per second, `0x486c26..0x486c4c`) and calls
`SoundAllOff`. In flight with `flightMusicVol` 0 it calls `ScoreOff`.

Slider arithmetic. Value to knob, `0x4891a0`: y = track y + (track h − knob h)
× (max − value) / max, integer division. Knob to value while dragging,
`0x489220`: the knob top follows the pointer, clamped to the 49-pixel travel,
and value = max − ((offset × max + 48) / 49). `0x489170` clamps to 0..max.
Pressing inside the track box (`0x488fd0`) grabs the knob at the press offset
when the press is on the knob, otherwise at half the knob size (13,15), and
drags until the button is released (`0x489070`). `_DrawSliderVert` `0x48bc60`
draws `SLIDETOP.PIC` (34 × 9) at (track x − 6, track y − 6), tiles
`SLIDEMID.PIC` (34 × 8) from track y + 2 for track h − 12, draws `SLIDEBOT.PIC`
(34 × 15) at track y + track h − 10, and `SLIDERV.PIC` (26 × 30) at the knob
position. No slider path plays a sound.

Toggle, `_DrawToggle` `0x48b930`: value 1 shows `TOGGLE00.PIC`, value 0
`TOGGLE04.PIC` (format `TOGGLE%02d.PIC` `0x4fcd9c`). A click in the hit box
flips the value (`0x488e3c`). The change animates 03, 02, 01, 00 towards 1 or
01, 02, 03, 04 towards 0, restoring a 39 × 70 background patch first and
holding each frame 26 ticks. On frame 02 it plays `&SWITCH.11K` (`0x4fcd90`)
at level 255: through `SoundNoMixer` with pan x / 4 − 80 when paused in flight,
otherwise through `SingleSound`, centred.

### Mixer consumers

Every successful `SoundOn` start calls `SoundSetup` `0x433d80` (`0x433c09`),
and `SoundPoints` `0x433480` repeats it for every live voice each flight frame
(`FlyingLoop` `0x404eae`). `SoundSetup` ends by clamping the level to 255 and
multiplying by `overallVol` / 100 (`0x43436c..0x43439b`). `SetVolPitchPan`
`0x435a00` sends level / 2 to the 0..127 driver volume. While paused it sends 0
for every mixer voice except the no-mixer channel `0x5402c0`; `PollMod` applies
that once when a pause begins (`0x435578..0x4355a3`).

- Engine class: a source whose object kind is 4 (aircraft) and whose type
  nibble (`MODSPEC+0x2a & 0xf000`) is non-zero and not `0x3000` is multiplied by
  `engineVol` / 100 at `0x4341f8..0x434211`. `ServiceSounds` gives those types
  to each aircraft's engine loop (`0x5000`, or `0x7000` with the afterburner
  loop for the player), `&TRBLNCE.5K` (`0x2000`), `&TIRES.5K` or `&GRIND.11K`
  (`0x4000`), `&WIND.11K` (`0x1000`) and `&JETDAM.11K` (`0x8000`). With
  `engineVol` 0 that whole per-aircraft block is skipped (`0x434bb6`), which
  also skips the touchdown `&SQUEAL.5K`/`&CRASH.5K` requests inside it.
  `_ServicePlayer` scales the afterburner light-off (`&AFTBURN.11K` or
  `&AFTB2.11K`) to `engineVol` × 240 / 100 (`0x416cc6..0x416cdd`), the start
  recording to max(throttle, 50) × `engineVol` / 50 and the stop recording to
  max(previous throttle, 50) × `engineVol` / 80 (`0x416d70..0x416e0c`). The
  four recording slots `0x50d2e9..0x50d2f5` are the PT engine, afterburner,
  start and stop clips listed in the [aircraft notes](aircraft.md).
- Lock and RWR: `ServiceSounds` `0x4352b2..0x4353b8` computes eight channel
  levels from base words at `0x4f3c60` (255, 255, 200, 200, then 200 four
  times): the first four × `lockVol` / 100, the last four × `rwrVol` / 100, then
  × `overallVol` / 100, and stores that as the voice's base level. The
  per-frame `SoundSetup` then applies `overallVol` again. Channel recordings,
  from `0x4f3c20`: `&IR1.11K` twice, `&RDRTRY.5K`, `&RDRLOCK.5K`, `&RWRIR.5K`,
  `&RWRLOCK.5K`, `&RWRDTCT.5K`, `&RWRLOCK.5K`; their flags are wired in
  `InitSound` `0x43339a..0x4333ef`.
- Stall: `_FMFlight` starts `&STALL.5K` (`0x4f81b8`) at `stallVol` × 255 / 100
  and `&STALLWR.5K` (`0x4f81ac`) at `stallVol` × 140 / 100 (`0x47b425..0x47b4a0`,
  `0x47b9a2..0x47b9c8`).
- Radio: speech stems play at `radioVol` × 255 / 100 (`0x48d610`, see the
  [radio notes](radio.md)). The player's ejection shout is a separate
  `SingleSound` at 255 (`0x414ce8..0x414d0e`).
- No category slider (OVERALL only): gear, flaps, hook, bay and brake requests
  from `FMGear`/`FMFlaps`/`FMHook`/`FMBay`/`FMBrakes`, `&BOMB.11K`, weapon fire
  `0x4c27df`, chaff and flare `0x444842`, explosions (type `0x3000`,
  `0x44367f`), passing sounds `0x43506b`, `&HRTBEAT.11K` (`0x6000`) and the
  shell `&BUTTON.11K`, `&CLICK.11K`, `&switch1.11K`, `&SWITCH.11K`.
- Music: `MusicVolume` `0x432b40` stores `volDMusic` = min(value, 100) × 255 /
  100 for the digital path. `DMusicOn` `0x4329a0` starts a phrase at
  `volDMusic`, and `SoundActive` `0x432a90` rewrites the playing phrase's level
  to `volDMusic` × `overallVol` / 100 on every `ScoreUpdate` or
  `ShellMusicUpdate` pass. `usnfmain` applies `flightMusicVol` on entering
  flight (`0x4043e7`) and `otherMusicVol` on every return to the shell
  (`0x403a15`, `0x403ab6`, `0x4040cb`, `0x4049ac`, `0x4049fb`). `PollMod`
  `0x4354b0`, a 30 Hz sound timer registered at `0x43318c`, reapplies the
  screen's value when a pause ends (`0x435503..0x435522`). `ScoreUpdate` stops
  shell music with `otherMusicVol` 0 (`0x432cc8`) and flight scores with
  `flightMusicVol` 0 (`0x432d28`).
- Pan: `ViewPan` `0x4343b0` folds the camera-relative azimuth into −90..90
  degrees (182 units per degree, `0x4344cc`) and applies `stereoSeparation` s:
  above 50 the angle a becomes a + (s − 50) × a / 16, below 50 a + (s − 50) × a
  / 50, each truncated towards zero, then clamped to ±90 (`0x4344da..0x43453e`).
  `SoundSetup` negates any non-zero pan, positioned or fixed, when `_stereoSwap`
  is set (`0x433efd..0x433f0e`). The driver pan is (pan + 90) × 0.711
  (`0x4e92b0`), clamped to 0..127.

| Resource | Size | SHA-256 of extracted bytes |
| --- | ---: | --- |
| `SNDPREF.PIC` | 382 × 376 | `0c0226fb67063f3374f205597a84ca90d99b2ae40c71b7ae8aaef99d81c98451` |
| `SLIDERV.PIC` | 26 × 30 | `a74e7d8735dc02f4853aaf00996a3a7ad28c5c3800a803c55a953e13f961bc5c` |
| `SLIDETOP.PIC` | 34 × 9 | `0edeea83535d9f87e6e1ffd423cc8d6a105d96b3fa5b2151a086ba9761471f9b` |
| `SLIDEMID.PIC` | 34 × 8 | `951db038f84de65f7eb4214c4a457e13e9e73f8878804ce328713d2731d8551e` |
| `SLIDEBOT.PIC` | 34 × 15 | `cf35026e59beb4204aa9467c2dfd75c0fdb56b1bf25daeec6559f4cf7e16c01a` |
| `TOGGLE00.PIC` | 35 × 42 | `4449d701ad622bb001b4398e5e9ea2685e5f435a2f5902d0b23f690d36e288eb` |
| `TOGGLE01.PIC` | 36 × 43 | `3af633ee94800666e8f7620bcb021e12450b0f98d2c9e65e7c7220d7664babc0` |
| `TOGGLE02.PIC` | 38 × 54 | `24532428a130da84da1939a2d3b8d7279ad4a68277f36848ac77dee1909723f8` |
| `TOGGLE03.PIC` | 37 × 65 | `5fcbca73460bb27bc52b8489d0d6ee529f4a1411e70eb7cd9d7bc1908f7b0e75` |
| `TOGGLE04.PIC` | 36 × 64 | `3bd4a3c3185910f491f8f8909db5d74a0906b572996e7a2ddecf95224cff98c1` |
| `&SWITCH.11K` (FA_2.LIB) | 1,840 bytes | `1970bafa9fb520753c5527d2497fada9590148f712adc77d53e8537f3c737477` |
