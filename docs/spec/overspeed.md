# Overspeed

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Requested behavior

Implementation mode. John requested the time-based failure rule on 2026-10-01,
replacing the immediate loss at 1.5 times top speed. This is `opinionated`, not
retail behaviour. The manual (p. 90) says air resistance eventually tears off the
wings beyond the structural limit, but supplies no timing or probabilities.

The limit is the aircraft's top speed at its current altitude: the right edge
of its 1 G envelope, also used by the envelope window (rotorcraft with a
structural speed are the exception below). Above the envelope's
ceiling there is no speed range, so the timer resets. The native-table research
adapter remains outside this rule.

**Rotorcraft (agent decision, 2026-10-08).** A helicopter's envelope top speed
is a performance figure fitted to its published level speed (the AH-64's 158 kt,
the Mi-24's 178 kt), well under the speed at which its airframe is at risk. For
the aircraft whose rotor table sets a structural speed, the limit is that speed
instead, in true airspeed, with everything else in this document unchanged: the
AH-64 at 197 kt and the Mi-24 at 190 kt, their never-exceed speeds (Vne). The
rotor's retreating blade stall starts at the same speed at the reference blade
loading, and earlier when the blades are loaded harder or the air is thin, so a
dive toward Vne shows the stall (vibration, nose up, roll toward the retreating
blade) before the five safe seconds begin. The envelope still supplies the
ceiling rule: above the top of the 1 G polygon there is no limit and the timer
resets. The speed is `structural_kt` in the rotor table of
`crates/tore-sim/src/models/variety/lift.rs`; `None` keeps the envelope's top
speed, which is what the CH-47 does until its own slice sets a value. The
V-22's value is calibrated airspeed: 280 KCAS, its published airplane-mode
never-exceed speed, or with the nacelles up the conversion corridor's maximum
at their angle (100 KCAS in the hover band), whichever is lower, turned into
true airspeed at the current altitude (VTOL overhaul slice P5, agent
decision 2026-10-08).
It applies on the hybrid adapter only, so the legacy adapter, the restricted
native path and every fixed-wing aircraft are unchanged.

| Condition | Behavior |
| --- | --- |
| Below 0.95 of the limit | No shake. |
| 0.95 to 1.0 | Shake grows by smoothstep to full strength. |
| At or above 1.0 | Full shake and the player message `OVERSPEED` every 4 seconds. |
| Strictly above 1.0, through 5 continuous seconds | Timer runs, no loss risk. |
| At 6, 7, 8 and 9 continuous seconds above 1.0 | One independent 25 percent destruction roll per second. |
| At 10 continuous seconds above 1.0 | Guaranteed destruction if still alive. |
| At or below 1.0 | Timer resets to zero. |

The first roll is at 6 seconds, confirmed by John on 2026-10-01. Time is counted
in 120 Hz simulation ticks, so pause and render cadence cannot change it. Rolls
use the aircraft's seeded simulation generator, the existing researched-adapter
RNG or the legacy adapter's fixed-seed generator (seed 1, agent decision).
The timer and complete RNG state travel in exact multiplayer state snapshots.
Replay playback displays recorded outcomes; deterministic resimulation reproduces
rolls from the same input and state. There is no app/server duplicate of this rule.

An Invulnerable player retains the timer, shake and warning but is not destroyed
and consumes no failure rolls. At or below the limit the timer still resets.
An aircraft without that cheat uses the ordinary structural destruction path,
with loss cause `overspeed`, wreck motion, debrief and recording events. No kill
is credited to a previous attacker. Autonomous decisions are unchanged.

Shake is a view offset only, evaluated from ratio and simulation time. The
existing fitted constants remain 0.024 radians at full strength, with 23 and
31 Hz terms weighted 60 and 40 percent. Shake begins at 0.95 of the limit and
reaches full strength at 1.0. It does not affect the flight state.

Tests exercise all roll boundaries, many seeds including ten-second survivors,
reset, invulnerability and exact snapshot restore immediately before a roll.
Headless `flight-overspeed-*` scenarios hold a documented overspeed fixture for
up to ten seconds and check its cause and timing. The normal-flight tests retain
level-flight and ceiling checks. Retail comparison and optional shake audio
remain unavailable and unimplemented respectively.
