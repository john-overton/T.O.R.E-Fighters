# FA clock, turbulence and aircraft vapor research

Static follow-up, 2026-09-14. Addresses refer only to the reviewed FA.EXE/SMS
pair in [native flight research](native-flight.md). No original routine is
executed. [Reproduction and validation](../baselines/weather-research.md).

## Continuous time is confirmed

The user's recollection is supported by the executable:

- `_TIMEInit` at `0x486a10` accepts initial ticks, hour and minute. At
  `0x486a34–0x486a5e` it computes `(hour * 60 + minute) * 60` and writes
  `_startTimeOfDay` (`0x5528e4`) and `_currentTimeOfDay` (`0x552934`).
- `_TIMEUpdate` at `0x486aa0` obtains elapsed timer ticks, applies compression
  and pause gates, and advances `_currentTicks` (`0x552928`). Counter helper
  `0x486bf0` scales performance-counter elapsed time by 256 before division
  by frequency: this clock uses 256 units per second.
- At `0x486b7d–0x486bbe`, time of day becomes start seconds plus the prior
  `_currentTime` unsigned word (`0x5528e0`), modulo 86,400 seconds. It then
  updates that elapsed-seconds word from `currentTicks >> 8`. This is continuous
  simulation time with whole-second time-of-day sampling and a one-update lag.
- `0x486af2–0x486b6f` sets frame ticks to zero for `MPPaused()` or compression
  sentinel `0x7fff`; other compression values shift the delta. Native minimum/
  maximum frame increments and preference-dependent scaling also exist.
- Main-loop span `0x404c7e–0x404cc8` calls time update and subsequently the
  weather view update at `0x4b3480`.

Elapsed seconds use a 16-bit word: the isolated path has a 65,536-second wrap
exposure. Other resets and long-session behavior remain unaccepted. Do not
accidentally reproduce that artifact in the authored 120 Hz clock.

### Weather consumes the changing clock

`0x4b3750–0x4b3816` scans 352-byte weather records until the flag-bit-0
sentinel. Record `+0x02` / `+0x06` are inclusive start/end time-of-day bounds.
Matching records can invoke the native callback at `+0x136`, then copy into the
active list. Matching adjacent altitude fields at `+0x0a` enter interpolation
through `0x4b3820`; the caller uses signed word arithmetic and quarter-scaled
time differences. Full field/interpolation translation remains open.

View update schedules selection again after one elapsed second when a callback
ran, otherwise ten seconds, with additional current interval checks. Lighting
at `0x4b4170` and celestial draw gating around `0x4ab205` also consume current
time and active-layer fields. This establishes environment evolution beyond an
advancing clock UI. Native callbacks must be translated, never loaded.

## The LAY record layout is decoded and confirmed against retail data

`0x4b3750–0x4b3816` walks the loaded table at `[0x580e24]`, compares
`_currentTimeOfDay` against two signed dwords and copies 0x58 dwords (352 bytes)
into `?curLayers@@3PAULAYER@@A` at `0x583250`. `_WRGetLayer@8` `0x4b3190`
truncates an 8.8 altitude, clamps it at zero and selects by two more dwords.

| Offset | Width | Meaning | Evidence |
| --- | --- | --- | --- |
| `+0x00` | word | flags; bit 0 is the table sentinel, bit `0x40` is night hazing | `0x4b3767`, `0x4b353c` |
| `+0x02` | dword | inclusive start time of day, seconds | `0x4b3775` |
| `+0x06` | dword | inclusive end time of day, seconds | `0x4b377c` |
| `+0x0a` | dword | inclusive low altitude, feet | `0x4b31a3` |
| `+0x0e` | dword | inclusive high altitude, feet | `0x4b31ad` |
| `+0x3e` | 31 × RGB | sky ramp, 6-bit, copied to palette 224 | `0x4b364a` |
| `+0x9b` | 32 × RGB | terrain ramp, 6-bit, copied to palette 192 | `0x4b365c` |
| `+0x136` | dword | native callback; translated only, never loaded | `0x4b3783` |
| `+0x14e` | 5 bytes | effect strengths by selector | `0x4b475a` |
| `+0x153` | 13 bytes | NUL-terminated record shape name | retail data |

All 24 imported retail modules parse under this layout. Their contents confirm
the interpretation rather than merely fitting it:

| Module | Records | Banding |
| --- | --- | --- |
| `DAY1` / `DAY2` (+5 theater variants each) | 5 | time only, full altitude column |
| `CLOUD1` (+5 variants) | 3 | altitude only: 0–5,000, 4,500–9,500, 9,000+ feet |
| `FOG1` (+5 variants) | 2 | altitude only: 0–8,000, 7,500+ feet |

`DAY2.LAY`'s five windows are 00:00–07:05, 07:00–07:10, 07:06–19:04,
19:00–19:08 and 19:05 onward: night, a ten-minute dawn, day, an eight-minute
dusk and night again. Exactly the two night records carry flags `0x72`, which
sets bit `0x40`; the three daylight records carry `0x2e`. That is the day/night
cycle, and it is the same bit that suppresses wing vapor.

Adjacent records deliberately overlap — 300 seconds at dawn, 240 at dusk, 500
feet between every cloud and fog band. Those overlaps are the interpolation
windows: `0x4b37ab` only interpolates when two adjacent records agree on
`+0x0a`, and passes quarter-scaled signed word time differences into `0x4b3820`.

### Effect selectors

Selector 0 is visibility: `@WRCanSee@8` `0x4b4b30` and `0x48d98d` scale a
distance by it. The five retail values line up with that reading and with each
module's purpose:

| Record | 0 | 1 | 2 | 3 | 4 |
| --- | --- | --- | --- | --- | --- |
| `DAY2` day | 100 | 100 | 100 | 100 | 100 |
| `DAY2` dawn/dusk | 75 | 100 | 100 | 100 | 100 |
| `DAY2` night | 25 | 100 | 125 | 100 | 100 |
| `CLOUD1` below | 75 | 100 | 100 | 100 | 100 |
| `CLOUD1` inside | 10 | 50 | 10 | 75 | 75 |
| `FOG1` low | 10 | 50 | 10 | 100 | 100 |

`_WRWeatherEffects` seeds its accumulator with 100 and takes the span minimum,
so the night value of 125 in selector 2 can never leave that path above 100.
Whether another consumer reads the byte uncapped is UNRESOLVED, as are the
meanings of selectors 1 through 4 and of flag bits 1, 2, 3, 5 and 7.

## Physical turbulence exists beyond hard-stick maneuvering

`_FMFlight` calls `_FMTurbulence` at `0x47c7b6`. The located routine spans
`0x477590–0x477d06`:

| Source | Confirmed behavior | Interpretation boundary |
| --- | --- | --- |
| `0x477593–0x4775af`, `0x477ce4` | Ground state or preference bit `0x01000000` suppresses events, clears interval and delays reconsideration by 512 ticks | Preference UI label untraced |
| `0x4777d4–0x47781e` | Queries ground; strength zero at/above 1,000 ft AGL, otherwise `100 - trunc(100 * height_f8 / 256000)` | Consistent with a low-altitude thermal approximation; native physical label unverified |
| `0x477826–0x477a2e` | Scans other active aircraft moving at least 293 fps; rejects distance >= 2,000 ft; distance and aircraft-relative geometry weighting can raise strength | Wake-like disturbance is a strong inference; exact volume needs validation |
| `0x477870–0x4778c7` | Nearby-aircraft weighting uses type field `+0x3f`, clamped 75–200 | Shared schema names this `sigs[0]`; not established wingspan/weight |
| `0x477a4b–0x477a89` | Reads PT `turbulencePercent` at `cpt+0x100`; outside inclusive 07:00–19:00 divides it by four; in daytime a ground-query flag selects two-thirds scaling | Final surface meaning of the flag remains open |
| `0x477a8b–0x477cfa` | Randomizes interval, recurrence, signed three-axis amplitudes and vertical disturbance; scales by strength, aircraft coefficient and speed | Exact RNG/scheduler replay and source ranges remain open |

Without low-altitude or nearby-plane strength, reconsideration is delayed five
seconds. Events store next-update/start/end, period, vertical rate and axis
amplitudes at `cp+0x1d4..0x1ec`. Active events use a sine phase and service delta
to change altitude and movement/display orientation. This is physical disturbance,
not merely camera shake; vertical displacement is not a three-dimensional wind field.

The reviewed routine reads no wind-speed, wind-heading or LAY record directly.
Do not make fog/cloud strength drive its amplitude on this evidence. Upstream
indirect coupling is not excluded. A renderer-independent service should take
explicit terrain, neighbor geometry, time and aircraft coefficient, with
per-aircraft mutable disturbance state rather than an invented global gust field.

## Maneuver effects and sound are separate

A different function named `Turbulence`, at `0x434550`, belongs to the sound
path. It reads G at aircraft `+0x19b`, roll rate at `+0x17f`, rudder, departure
mode, device/state flags and speed. It combines these into sound intensity,
including a maximum with `abs(roll_rate) / 37 + 384`. The player sound branch
calls it at `0x434d76`, testing against 512 before continuing dispatch.

This verifies maneuver-responsive sound logic, not an aerodynamic buffet force
or exact audible sample. FM G/AoA, departure and control-disturbance producers
remain distinct. Complete hard-pull/push buffet needs further producer/consumer
tracing; do not equate all fields or functions called turbulence.

## Wing vapor trails are a dedicated streamer subsystem

The generic smoke leads were the wrong trail. A dedicated per-aircraft
**streamer** subsystem implements wing vapor, and its whole contract is
recoverable: `_StreamersInit@0` `0x4a0010`, `_StreamersUpdate@0` `0x4a0250`,
`_StreamersShutdown@0` `0x4a02d0`, `?FindStreamerDef@@YIPAUSTREAMER_DEF@@PAE@Z`
`0x49fd70` and `_DrawStreamer@12` `0x49fd90`.

### Attachment comes from the aircraft shape, not authored art

The shape byte-code vector table at `0x5183a0` (dword entries indexed by
`2 * opcode`, so opcodes are even) resolves two dedicated opcodes:

| Opcode | Handler | Meaning |
| --- | --- | --- |
| `0xce` | `0x4d47a4` `do_streamer_def` | 38-byte inline `STREAMER_DEF` payload |
| `0xd0` | `0x4d47b8` `do_streamer_draw` | one inline word selects the side, then calls `_DrawStreamer@12` |

`FindStreamerDef` looks at shape offset `+0x0e`, optionally skips an `0xf2`
opcode and its four bytes, and accepts the record only when the next word is
`0xce`. Aircraft without that opcode have no streamer and fall back to the
object origin (`0x4a012c–0x4a014b`). The 38-byte record is:

| Offset | Width | Meaning |
| --- | --- | --- |
| `+0x00` | dword ×3 | hinge pivot; all-zero disables the hinge (`0x4a0181`) |
| `+0x0c` | word | hinge scale applied to the driving value |
| `+0x0e` | dword ×3 | side 0 attachment point, X mirrored |
| `+0x1a` | dword ×3 | side 1 attachment point |

`0x4a0110` resolves one world attachment point per side per update:
rotate the side point about the pivot by
`182 * (swing_wing * def[+0x0c]) / 32767` in binary angle units, add the pivot,
mirror X back for side 0, rotate by the object's display orientation words
(`+0x1d`, `+0x1f`, `+0x21`) and add the object position. The driving value at
object `+0x1a1` is the **swing-wing position**: `0x4ab7d4` copies the same word
straight into `_PLswingWing`. Fixed-wing aircraft therefore hold a static hinge.

### The trail is a sampled position history

Each plane owns two 20-byte sample rings (side 0 at `0x571428`, side 1 at
`0x570ef8`, stride `0x14`), initialized to capacity 10, commit interval 25 ticks
and interpolation mode 2 (`0x4a006f–0x4a00a1`). `@SampleInit@8` `0x4124e0`
allocates `capacity * 16` bytes and fills every 16-byte `{tick, x, y, z}` slot
with the current tick and point. `@SampleUpdate@8` `0x412570` always overwrites
entry 0 with the live point, and only shifts the ring down when at least 25
ticks have passed since the previous commit. At 256 clock units per second the
retained history is `9 * 25` ticks, about 0.88 seconds.

`_SampleGet@12` `0x4125c0` clamps negative ticks to zero, walks back to the
bracketing pair and interpolates. Mode 0 is a plain linear blend of the three
components; mode 1 blends through `_InterpAngle@16`; mode 2 is the positional
blend the streamers use.

### Trigger, intensity and fade

`_DrawStreamer@12` is the complete visual contract:

| Source | Behavior |
| --- | --- |
| `0x49fda8–0x49fdaf` | returns immediately when `_currentLayer & 0x40`; the same bit sets `_nightHazing` at `0x4b353c` |
| `0x49fdb5–0x49fde2` | returns unless the object id is in the active `_planes` list |
| `0x49fe17–0x49fe2e` | `excess = abs(g_f8 - 0x100) - 0x300`; **no trail at all unless it is positive** |
| `0x49fe34–0x49fe4b` | clamps `excess` to `0x300` and scales it to `0..64` |
| `0x49fe4d–0x49fe88` | when object flag `+0x10 & 0x80` is set, subtracts up to 50 percent using `min(50, 50 * abs(roll_rate) / 0xb400)` |
| `0x49fe8a–0x49feb5` | six `_SampleGet@12` calls at `currentTicks - i * intensity / 5` for `i` in `0..6` |
| `0x49febf–0x49ff76` | builds six `0x20` vertex records and five `0x2e` line records with colors `0x10d` down to `0x109` and constant `0x96` |
| `0x49ff7d–0x49fff5` | `_NeedClip`, `_ulineSkipLastPixel = 1`, `@GRExec@4`, then restores the clip and shift buffers |

Because `g_f8` stores 1 G as `0x100`, the trigger is `abs(G - 1) > 3` — roughly
above 4 G or below -2 G — reaching full length at `abs(G - 1) = 6`. Full length
looks back 64 clock units, a quarter second of flight path. The five decreasing
color indices are the only fade; there is no particle lifetime, no growth and no
drift. The trail is recomputed every frame from the shared position history, so
it is attached to the aircraft's recent path rather than emitted into the world.

### Still unresolved

Object flag `+0x10 & 0x80` guarding the roll-rate reduction is untraced, and the
`0x10d..0x109` and `0x96` values are shape-interpreter color/shade arguments
whose palette roles are not yet confirmed. No separate **engine contrail** and no
**broader wing-induced vapor** has been identified in the reviewed scope: only
two streamers exist per aircraft, chosen by a one-word side selector. Generic
`_GRAPHICAddSmoke` / `_GRAPHICAddSmokeAdder` at `0x443e80` / `0x443f90` remain
damage and projectile producers. Absence of a symbol is not proof of absence;
these two remain open parity questions rather than verified-absent.
