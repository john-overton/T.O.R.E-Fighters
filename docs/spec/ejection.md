# Ejection and pilot survival

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Recovered behaviour

Research mode followed by implementation mode, requested by John on 2026-09-23.
The manual, printed page 161, specifies **Shift+E, pressed twice**. Page 173
says the aircraft is lost and campaign rescue or capture can determine whether
the pilot continues. Campaign rescue/capture is outside the current mission host.
The manual identity is recorded in [envelope](envelope.md).

The reviewed executable refuses aircraft without an ejection seat. Ejection
creates a separate pilot object, retaining aircraft motion with an additional
seat impulse. The parachute opens while descending below 4,000 feet above the
surface. Seat launch and chute opening have separate sounds. A living pilot and
a destroyed aircraft are separate states. Evidence, asset identities and remaining
retail uncertainties live in [the source notes](../formats/ejection.md).

## Host rules

The control and separate escape object are spec-derived. John requested living
pilots to escape unrecoverable aircraft, including a dive that cannot be recovered,
and inability to raise the nose and sustain lift. The numerical rules below are
**fitted, agent decisions**, rather than measured original AI behaviour.

- Two distinct presses within 240 simulation ticks (2 seconds) eject. Key repeat
  does not count. The first press displays a confirmation prompt. A later press
  starts a fresh confirmation window. Dead or already ejected pilots cannot eject.
  Aircraft destruction alone does not inhibit a surviving pilot's seat. Ground
  impact or an aircraft airburst before escape does inhibit it.
- Seat availability comes from imported PLANE flags bit 0x10. On launch, disable
  pilot/AI control, radar and weapons; the abandoned aircraft uses existing wreck
  motion. The pilot starts 6 feet along aircraft up, retaining aircraft velocity
  plus an 80 ft/s impulse along that axis. Inverted or low diving escapes can fail.
- Seat flight lasts 90 ticks (0.75 seconds). Gravity is 32.174 ft/s². Free-fall
  quadratic drag is 0.0015 per foot on each velocity component. Below 4,000 feet
  AGL and while descending, inflate the chute over 120 ticks (1 second). An open
  chute approaches an 18 ft/s sink rate and zero horizontal speed with a
  1.5-second time constant. Ground contact before full inflation, or above
  30 ft/s downward speed, kills the pilot. Otherwise the pilot lands alive.
- Pilot wounds continue during descent. Landing alive receives the existing
  wound treatment. An abandoned aircraft's later destruction never kills its
  separated pilot. Restart clears confirmation, pilot and aircraft escape state.
- The camera follows the separated pilot in exterior view. Ejection itself does
  not end or pause the mission. A cockpit view cannot reattach to the abandoned
  aircraft. The host represents the player's pilot, not separately simulated
  additional crew members.

## Fitted AI decision

Run the same assessment for every non-dummy AI aircraft, before weapons and
movement. Player assessment gives a warning only, never automatic ejection.
John additionally requested on 2026-09-23 that an undamaged aircraft above
200 feet AGL must never eject automatically. This is a hard guard for both
recovery cases, measured above local terrain rather than sea level. An aircraft
is undamaged when airframe/region damage and subsystem fault counters are zero,
its wings are intact, it is not burning and hydraulic authority is nominal.
A damaged aircraft still needs the recovery calculation below.

Dead pilots, aircraft without seats, supported aircraft and completed escapes
are excluded. A destroyed airborne aircraft with a living pilot is unrecoverable.

For a descending aircraft, use its imported speed/altitude G envelopes and its
remaining pitch, roll, hydraulic and wing authority. AI airframe damage scales
available authority by its existing remaining-health rule. Estimate wings-level
roll time from bank and available roll rate. Estimate pullout radius as
`speed² / (32.174 * (available_G - 1))`. Clearance required is the roll-time
altitude loss plus `radius * (1 - cos(flight_path_angle))` plus 75 feet. Sample
terrain along the forward recovery path at 9 equally spaced points, capped at
10 seconds. A descending path whose clearance is insufficient triggers ejection.

A nose below -5 degrees with downward speed above 20 ft/s and at most 1.05 G
available counts as inability to sustain lift if impact is within 8 seconds or
remaining authority is at most 25%. For this branch the danger episode must
last 240 ticks before ejection eligibility, unless the descent will hit within
2 seconds. Other failed pullouts
must persist for 30 ticks; impact within 2 seconds bypasses that eligibility
delay. John additionally requested a 70% random ejection chance per second on
2026-09-23. Poll at 120, 240, 360 and subsequent 120-tick boundaries of each
continuous danger episode, only once the eligibility delay is satisfied. There
is no roll at time zero. A failed draw leaves the pilot aboard until the next
second, so impact can occur before a successful attempt. Recovery resets the
interval; changing from one danger reason to another does not postpone it.
Each pilot owns a separate deterministic stream seeded from mission
seed and actor identity; recovery never reseeds it. Draws in 0..99 below 70
succeed. Manual ejection has no chance roll. Healthy level flight, intentional
recoverable dives, ground operations and dummy targets must not eject. These estimates do not assert a
perfect aerodynamic reachability solver or retail AI parity.

## Audio and art

Load retail resources at runtime. Play pilot ejection speech on launch, friendly
AI ejection speech only when that aircraft ejects, and the emergency cockpit
warning once per danger episode. The cockpit warning uses the recovered
repeated-eject recording; its assignment as a RIO or pilot voice is not
established by the reviewed call sites. Do not invent radio chatter
for enemies or use the fuel-empty ejection line without the corresponding event.
The cockpit engine bed stops after separation; the abandoned aircraft remains
a spatial sound source. Seat and chute effects follow their phase transitions
once, with pause and
restart obeying the existing audio controls. Ejection selects the imported
M_EJECT score. Missing optional media must leave simulation usable and report
which resources need reimport. Exact retail speaker selection and timing remain
unknown where marked in the source notes.

Art uses the imported indexed textures and pose geometry. Source BC/0x96 line
records provide the fitted suspension-cord interpretation; GPU ribbons are
0.05 feet wide. Camera distance is 60 feet aft and 22 feet up, pitched down
0.34 radians, all fitted presentation choices. Independent extra crew,
post-separation weapon damage to pilots and rendered pilot shadows remain open.
