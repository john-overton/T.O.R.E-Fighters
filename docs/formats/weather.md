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

### Fog tint consumer follow-up — 2026-09-15

Source tracing for [dependency step 1](../weather-plan.md#numbered-dependency-sequence--2026-09-15)
rechecked the reviewed executable/symbol hashes and followed the existing static
disassembly. The implementation checkpoint below distinguishes live callback state
from the diagnostic palette helpers.

- `_WRFogLayerUpdate` at `0x4b4320` changes record `+0xfe` by
  `Rand(51) - 25`, then clamps it to `217..235`. Selection invokes the callback
  on the loaded record before copying it; the mutation persists between calls.
- View update at `0x4b3674..0x4b36a7` copies the selected record's `+0xfb` RGB
  to `0x5843d0`, publishes the low word of `+0xfe` to `0x583aa4`, and sets the
  smoothing increment at `0x50c8d4` to 16.
- The later palette pass at `0x4b3f28..0x4b3f74` forms a nonnegative target
  from signed-word `[0x583aa4] - [0x580da0]`. Mutable word `0x583930` moves
  toward that target by the increment, clamping overshoot. The producer of the
  subtracted adjustment and complete update cadence still need review.
- Calls at `0x4b4017` and `0x4b403c` pass the tint RGB and smoothed strength to
  helper `0x4c8f10`. Relative to palette base `0x583aa8`, their destinations are
  index 64 with count 191 and index 47 with count 14, respectively. The second
  call caps strength at 92. This is not a uniform full-palette tint.
- Helper `0x4c8f10` uses byte arithmetic, a halved strength and signed multiply,
  with a separate strength-256 path. Preserve those details when translating;
  generic floating-point RGB interpolation is not established as equivalent.

Follow-up source trace: `0x4b36ae..0x4b373f` produces the subtracted adjustment.
It requires the layer-query overlap flag and a positive record dword `+0x12e`,
then draws every three elapsed seconds. The bound is `+0x12e` times a clamped
signed word obtained from the selected object's `+0x34 >> 8`, divided by
record `+0x132`. Supplied low CLOUD1/FOG1 records store 102 and 733 in those
fields. The object's field producer/units and complete view-update semantics
remain unverified; the implementation does not guess them.

The Rust reader now resolves a nonzero callback only through its six-byte
`ff 25` alias and bounded `.idata` entries. Only the reviewed `main.dll`
`_WRFogLayerUpdate` and `_T_HorizonProc` contracts are supported; unknown symbols,
ordinals, malformed aliases and unsupported image bases fail explicitly.
All six supplied FOG variants reference the fog callback on their low record;
the other supplied records have null callbacks. An unused horizon import in a
module does not make its records invoke that callback.

`Environment` owns mutable record copies and a dedicated seeded RNG outside
immutable configuration. It invokes matching callbacks before copying/blending,
then schedules selection in one second if any callback ran, ten otherwise;
leaving the first active interval also forces selection. The host 120 Hz clock,
dedicated RNG stream/default seed 1 and omission of retail elapsed-word wrap
remain authored adapters. Camera queries do not advance either state.

`weather::palette` translates selective tint and smoothing in the reviewed
0..255 strength domain. It retains six-bit colors, odd-strength truncation,
signed-product rounding and the index-47..60 strength cap of 92. The helper's
separate strength-256 wrapping branch is outside this API. These helpers remain
diagnostic pending view-state integration, palette-pass scheduling and the
other ordered palette effects. Existing rendered palette expansion still
ignores the tint scalar, so the callback alone does not finish visible fog.

### Celestial dispatch trace — 2026-09-15

Static region `0x4aaca0..0x4aacdd` loads STARS, MOON and SUN into pointers
`0x580ba8`, `0x580bb0` and `0x57cd08`. The draw-list builder at
`0x4ab0af..0x4ab309` emits stars at zero rotation and moon at record
`+0x13e/+0x140` when flag `0x10` is set. Sun dispatch requires flag `0x08`,
inclusive sunrise/sunset time bounds and elevation at least signed `0xf8e4`
(-1820 binary-angle units). These are dispatch gates, not complete clipping or
material contracts. The bounded static extraction now includes both spans.

Imported shape diagnostics currently report: SUN reaches unsupported opcode
`0x13` at module VA `0x1034`; MOON, STARS and CLOUDS produce no accepted static
geometry; CLOUD1 yields two faces. Existing static SH projection skips commands,
so that result does not establish complete cloud materials or behavior. Shapes
remain imported dependencies and are not yet celestial/cloud render commands.

## Reviewed LAY fields are confirmed against retail data

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

### The rest of the record, and how it is interpolated

`0x4b3820` blends a source record into a destination as `position` runs across
`span`. Below zero it keeps the destination; at or above `span` it copies all 352
bytes. Otherwise `factor = (position << 8) / span` drives
`*d += ((s - *d) * factor) >> 8` per dword (`0x4b3b60`) and per color component
(`0x4b3b80`). That reveals the remaining fields:

| Offset | Width | Treatment | Meaning |
| --- | --- | --- | --- |
| `+0x12`, `+0x16` | dword ×2 | interpolated | visibility ramp: distance and haze density (0..256) at the near end |
| `+0x1a`, `+0x1e` | dword ×2 | interpolated | the same at the far end |
| `+0x22` | dword | interpolated | maximum see distance |
| `+0x26`..`+0x32` | dword ×4 | interpolated | altitude haze ramp: two heights above the band floor and their blends |
| `+0x36` | RGB | interpolated | haze color; `0x4b3ad0` resolves the nearest remap table and caches it at `+0x3a` |
| `+0xfb`, `+0xfe` | RGB + dword | interpolated | global palette tint color and target |
| `+0x102` | 14 + dword ×2 | replaced when non-empty | deck A: name, altitude in feet, tile-size exponent |
| `+0x118` | 14 + dword ×2 | replaced when non-empty | deck B |
| `+0x13e`, `+0x140` | word ×2 | kept | night light azimuth and elevation |
| `+0x142`, `+0x146` | dword ×2 | kept | sunrise and sunset seconds |
| `+0x14a`, `+0x14c` | word ×2 | kept | sun azimuth before and after noon |

### The distance unit is 256 feet

`_WRSetRemaps@8` at `0x4b31f7` adds a global bias to a 24.8-foot distance and
shifts it right 16 before comparing it against `+0x12` and `+0x22`, which makes
one record unit 256 feet. `_WRWeatherEffects` reaches the same scale from the
other side: it returns `see_distance << 16` and `@WRCanSee@8` compares that
against a 24.8-foot distance. Both give 256 feet per unit, and the retail values
then read as ordinary aviation figures:

| Record | Haze starts | Full haze | See distance |
| --- | --- | --- | --- |
| `DAY2` day | 7 nm, none | 30 nm, 80 percent | 261 nm |
| `DAY2` night | 0, 50 percent | 3.5 nm, total | 43 nm |
| `CLOUD1` inside deck | 0, 10 percent | 0.8 nm, total | 0.4 nm |
| `FOG1` low | 0, 10 percent | 0.8 nm, total | 0.8 nm |

`DAY2`'s daylight records also name a `SKY*08.PIC` deck at 75,000 feet with
131,072-foot tiles and an `OCEAN*06.PIC` deck at sea level with 32,768-foot
tiles. The `*` is a load-time choice among a numbered range (`0x4b4680`), which
is what SKY0 through SKY8 are for.

### Altitude haze

`0x4b3cb0`, gated on flag `0x02`, blends a record's own ramps toward its tint
RGB at `+0xfb` (not the remap shade at `+0x36`) before that record takes part in any altitude blend. The weight comes from
the `+0x26`..`+0x32` ramp, measured in 256-foot steps above the band floor. The
terrain ramp takes the full weight; the sky ramp fades it out linearly from
index 30 down to index 16 and leaves 0 through 15 untouched.

Bounds take the union rather than blending: `+0x0a` takes the minimum, `+0x0e`,
`+0x06` the maximum and `+0x02` the minimum. The flag byte is merged at
`0x4b39a6`: bit `0x10` follows whichever record the factor is nearer to, bit
`0x20` becomes an intersection when the destination carries `0x80`, and every
other bit is the union.

Both overlap windows call the same routine. Time uses
`position = (now - source.start) >> 2` and `span = (destination.end -
source.start) >> 2` (`0x4b37b8`), and only when the two records share `+0x0a`
(`0x4b37b4`). Altitude uses `position = altitude - source.low` and
`span = destination.high - source.low` (`0x4b3c37`), unconditionally.

Retail `DAY2.LAY` resolved through this produces a real dawn and dusk. Scalar 0
runs 0 at night, 82 at the transition midpoints and 165 by day; scalar 4 runs
1,031 to 6,187; the horizon color runs `[2, 1, 3]` to `[38, 40, 51]`; and the
night-hazing bit clears partway through. Twelve of the day's 1,440 minutes fall
inside a transition. The day records also name `OCEAN*06.PIC` as a dependency
and the night records do not.

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

## The mission weather choices are a recovered table

The source mission writer at `0x495fa0` emits `map %s`, `layer %s %d`,
`clouds %d` and `wind %d %d`, which settles three previously unverified fields:

- the `layer` line's second value is the **weather choice index** itself;
- `clouds` is a **cloud deck altitude in feet**, not a count — `0x42a8f9` gives
  choices 0, 3 and 4 a fifty percent chance of a deck between 7,000 and 20,000
  feet, and zero otherwise, which is why `UKR.MM` reads `clouds 0`;
- `wind` is whole degrees and feet per second, as `0x495ff2` divides the stored
  binary angle back by 182 to print it.

`0x42a7ea` and `0x42a8cc` are parallel tables indexed by that choice:

| Choice | Module | Launch time | Scattered deck |
| --- | --- | --- | --- |
| 0 | `day2` | 12:00 | possible |
| 1 | `cloud1` | 12:00 | no |
| 2 | `fog1` | 12:00 | no |
| 3 | `day2` | 07:01 | possible |
| 4 | `day2` | 19:01 | possible |
| 5 | `day2` | 00:00 | no |

So cloud and fog are altitude-banded modules while dawn, noon, dusk and night
are the same day module entered at different times. `0x42a80c` then appends a
per-theater letter when the map name begins with B, E, F, T or V, after skipping
a `~` or `$` campaign prefix; every other theater uses the unsuffixed module.
That is exactly why the archive ships six variants of each.

The creator offers **seven** labels — dawn, clear, cloudy, overcast, foggy,
sunset and night — against these six choices, and no table joining the two lists
was found. The editor now omits overcast per the user’s clarification that it
duplicates cloudy; cloudy selects `CLOUD1`. Imported source lists remain intact.
The six editor rows map by label, not by assuming their indices equal the native
weather table indices.

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

## Turbulence generator and wind-line contracts

The mission `wind` line is a compass heading in whole degrees and a speed in
feet per second. `0x481e70` multiplies the heading by 182 into a binary angle
and stores the speed unscaled; `0x476f3d` then advances position by
`speed * ticks` rotated by that angle, and `_Rotate2@8` turns `(0, d)` into
`(d sin h, d cos h)`, so zero is north and ninety is east. A mission without a
wind line gets `heading = Rand(0xfff0)` and `speed = Rand(0x16) + 7`, that is
7 to 28 feet per second, about 4 to 17 knots. Whether the heading names the
direction the wind blows towards or comes from is UNRESOLVED; the arithmetic
drifts the aircraft towards the stated heading.

`_FMTurbulence`'s event generator is recovered at the following boundaries
(the implementation corrections and remaining adapter differences are recorded
in [the review](../baselines/weather-review.md)):

| Source | Behavior |
| --- | --- |
| `0x477a3a` | with no strength, reconsider in 1,280 ticks; `0x477ce9` suppresses and reconsiders in 512 |
| `0x477a57` | outside 07:00 to 19:00 the aircraft's `turbulencePercent` is quartered; inside, a ground-query flag takes two thirds |
| `0x477a90` | an event lasts `Rand(0x1e00) / 100 + 89` ticks, and the sine phase spans twice that |
| `0x477ab7` | the gap to the next event is `Rand(2 * 15360 / (0.6 p + 15))`, where `p` combines speed, strength and percent, so stronger turbulence is also more frequent |
| `0x477b20` | a speed shape rising to full at 146 fps, flat to 293, then falling away to nothing at 586 |
| `0x477b73` | yaw, pitch and roll amplitudes are `Rand` over 1.82, 7.28 and 12.74 units per point of strength, each negated on a coin flip: at full strength about 1, 4 and 7 degrees per second |
| `0x477c3f` | the vertical rate uses its own speed factor peaking at 733 fps, and a rate at or past `0xc00` also shakes the view |
| `0x4775b5` | while active, the vertical rate moves height directly and the three amplitudes drive a sine over the doubled period, reaching movement heading, pitch and roll as well as the display angles |

At `0x477a9e`, AX is the quotient of division by 100, giving 89–165 ticks;
the previous modulo interpretation was incorrect. Active events take priority
over the next-update timer (`0x4775b5`), and generation returns without applying
the new event on that call.

The draw order is length, gap, three amplitudes, three sign flips, vertical
rate, one more sign flip. Reproducing that order with the already-translated
native generator gives replayable events from a shared seed. The nearby-aircraft
strength term at `0x477826` needs contact geometry and is not applied.

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
ticks have passed since the previous commit. It writes the live point **before**
shifting, so entries 0 and 1 coincide on a commit. At 256 clock units per second the
retained history is `9 * 25` ticks, about 0.88 seconds.

`_SampleGet@12` `0x4125c0` clamps negative ticks to zero, walks back to the
bracketing pair and interpolates. Mode 0 is a plain linear blend of the three
components; mode 1 blends through `_InterpAngle@16`; mode 2 is the positional
blend the streamers use.

### Trigger, intensity and fade

`_DrawStreamer@12` establishes the following visual behavior:

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

### Both reviewed aircraft carry the definition

`F18.SH` and `RAF.SH` both match `FindStreamerDef` exactly: the word at shape
offset `+0x0e` is `0xf2`, so it skips four bytes, and the word at `+0x12` is
`0xce`. The 38-byte record therefore starts at shape offset `0x14`. Neither has
a hinge — the pivot is all zero and the scale is zero, which is right for two
fixed-wing aircraft — and the attachment points are plain wingtips:

| Aircraft | Side 0 | Side 1 |
| --- | --- | --- |
| F/A-18D | `(-54, 1, -17)` | `(55, 0, -16)` |
| Rafale C | `(-54, -1, -30)` | `(54, -1, -30)` |

Those are source units; a third of a foot each, in the shape's right, forward
and up order. The app’s existing one-third-foot scale puts them about 18 feet outboard;
that scale remains provisional, not independent retail size acceptance.

### The trail colors are patterned fills, not palette entries

`@G_SetColor@4` at `0x497689` treats any color at or above `0x100` differently:
it takes the low byte of a named-color word at `0x55b9e0`, a **fill type** from
`0x560d98`, and a **remap table** from `0x55ba28`. `_WRInit` seeds the named
colors `0x100..0x12c` as an identity, so `Remap` at `0x4cc4a1` passes them
straight through, and `0x55be28` onward is filled from the LAY header's
fill-pattern pointers. Logical color `0x100 + n` therefore selects header
fill-pattern table `n`, and the streamer's `0x109` through `0x10d` are tables 9
through 13.

So wing vapor is drawn as five patterned, partly transparent fills rather than
five solid lines. The tables are located but not decoded, so the current implementation
marks color/fade as fitted. Its host sampling, float interpolation, shape scale
and unresolved roll-rate gate also prevent a claim of exact geometry/timing.

### Contrails and broader wing vapor: bounded negative findings

The previous claim of an exhaustive, verified absence was too strong. Effect
filenames, direct smoke callers and neighboring opcode slots cannot rule out
procedural geometry, indirect dispatch or aircraft-embedded drawing programs.
The streamer subsystem's two attachments describe that subsystem, not a global
limit on all aircraft effects. No dedicated engine contrail producer has been
confirmed in the inspected FA paths. This is **unconfirmed**, not proof that the
whole game lacks contrails.

#### Aircraft-embedded code inspected statically — 2026-09-15

Not executing imported modules does **not** prohibit disassembling their code or
translating reviewed contracts. `tools/inspect_shape_effects.py` now inventories
imports, local aliases and bounded `0xf0` re-entry candidates independently of
the app's neutral-pose decoder. Full outputs remain local; source hashes and
reproduction are in [the review evidence](../baselines/weather-review.md).

| Shape | Inspected re-entry blocks | Afterburner guards (CODE offsets) | Streamer draws (CODE offsets, side) |
| --- | ---: | --- | --- |
| `F18.SH` | 36 | `0x3087`, `0x5b64`, test alias `0x7900` against 1 | `0x32df`/0, `0x36d2`/1, `0x5990`/0, `0x5a92`/1 |
| `RAF.SH` | 44 | `0x20b6`, `0x3f4c`, test alias `0x5b50` against 1 | `0x2103`/1, `0x23e8`/0, `0x3d10`/0, `0x41cd`/1 |

The aliases resolve through each module's import table to `_PLafterBurner`, not
to a guessed state-word name. F18 additionally imports bay, brake, gear, hook and
left/right flap state. Rafale imports brake, canard, gear, flaps and rudder state.
Both import `do_start_interp` to return to the shape interpreter. The inspected
blocks only test these device words or write device-angle operands before
re-entry. They reveal no additional altitude/G/temperature-gated vapor branch.
The repeated streamer draws belong to different shape/detail paths, not four
independent emitters. The attachment record still supplies the two wingtips.

Rafale also has an unmatched `f0 00` byte-pattern candidate at CODE `0xe9f`;
raw byte matches are not automatically native instructions. The tool explicitly
retains unmatched candidates rather than declaring a complete control-flow graph.
This pass covers these two supplied aircraft shapes and their named imports;
other aircraft, all indirect paths and retail visual comparison remain open.
Broader wing-induced vapor is **not found in these inspected blocks**, not
proven absent throughout the game. Future optional contrails are planned in
[W7](../weather-plan.md#w7--optional-engine-contrails-after-retail-weather).

### Corrections and remaining gaps

The aircraft render leads in the previous pass were wrong: `0x48d780` is
`_PLANESayProc` and `0x48ec40` is `_PLANECommentProc`, both radio chatter. The
trail code is `0x49fd70–0x4a0310`.

The five streamer colors `0x10d` down to `0x109` pass through `Remap`
`0x4cc44c`, which consults `_effects` bits `0x8` and `0x10`. They are logical
indices, not palette entries, so the rendered fade depends on the active remap
table. Object flag `+0x10 & 0x80`, which gates the roll-rate shortening, and
which axis of the body-rate triple `+0x17f` selects, both remain UNRESOLVED.
`?wasAfterburn@@3DA` has no reference anywhere in the image and appears to be
dead.

## Palette remap and deck consumer contract — 2026-09-15

- Bounded root `+0x6c` reader decodes 48-byte shade headers and up to ten
  256-entry index remaps. FA `0x4b3ad0` chooses the first minimum Manhattan RGB
  distance; `0x4b3410` quantizes density and saturates the final level.
- Tint reduction at `0x4b36ae` is gated by layer overlap. The selected object's
  `+0x34` is signed 24.8 speed, confirmed by the object/context copy into
  `0x50ceb4`. Fields `+0x12e/+0x132` supply maximum reduction/speed cap.
- The palette worker at `0x486e80` invokes the palette pass every fourth 15 ms
  iteration. Host presentation owns smoothing/reduction/RNG and uses nominal
  60 ms fixed-tick passes; pause behavior, startup phase and independent seeded
  streams are authored scheduling, not native replay parity.
- Imported terrain remaps run before palette lookup/filtering. Aircraft retain
  their own palette and fitted RGB haze. Native cross-altitude ray composition
  (`0x4b31f0`) remains open; GPU distance/filtering are not the integer rasterizer.
- Named sky/ocean decks use the plane altitude, `2^exponent` feet tile size and
  reversed Z texture coordinate verified in `0x447aa5` and `0x448400`. Wildcards
  resolve once in record/deck order. CLI/app extraction now includes OCEAN PICs
  and rejects old caches missing them. The native special horizon fill and
  above-sky branches remain open; horizon minification is visibly aliased.
- Validation: 263 Rust tests, Clippy, build and 24 Python tests passed. Linux
  viewer and F18 capture passed; `.local/weather-foundation/planes.png` was
  visually inspected. These establish a working GPU path, not retail equality.
