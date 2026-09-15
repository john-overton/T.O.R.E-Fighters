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

## Contrails and wing-induced vapor

The user confirms retail contains contrails and vapor trails and requests
wing-induced vapor beyond wingtips. Both are required parity scope. Earlier
"wind-induced" wording was corrected to **wing-induced**; it is not evidence
for an additional wind-triggered effect.

Track engine contrails, wingtip vapor trails and broader wing-induced vapor
separately. Recover altitude/weather, speed, load/AoA and engine triggers;
original geometry/art; attachment points; cadence; growth/fade/lifetime;
visibility and any drift. Do not invent humidity behavior or substitute damage
smoke without evidence.

Generic `_GRAPHICAddSmoke` / `_GRAPHICAddSmokeAdder` at `0x443e80` / `0x443f90`
have multiple damage/projectile callers. No dedicated contrail/vapor symbol was
identified in this pass. These names and SMOKE resources do not identify the
trail implementations. Next trace aircraft render callbacks (`0x48d780`,
`0x48ec40`) and shape/effect consumers, including alternate graphics paths.
Their emission contracts remain open; presence is user-confirmed.
