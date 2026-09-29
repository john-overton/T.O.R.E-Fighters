# Overspeed

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Requested behavior

Implementation mode. John requested this on 2026-09-29. It is `opinionated`, not
retail behaviour: the manual (p. 90) says only that below about 36,000 ft, beyond
the structural limit, "air resistance begins to weaken the airframe and the wings
will eventually tear off", and gives no numbers. Every number below is an agent
decision.

The reference speed is the aircraft's **top speed at its current altitude**: the
right edge of its 1 G envelope polygon, the same figure the envelope window and
the flight probe use. The ratio is airspeed divided by that top speed. Above the
envelope's ceiling there is no speed range, so there is no ratio and no rule.

| Ratio | What happens | Scope |
| --- | --- | --- |
| Below 0.95 | Nothing. | |
| 0.95 to 1.0 | The cockpit shakes, rising smoothly (a smoothstep) from nothing to the clear maximum. | The player, in the cockpit and every exterior view. |
| 1.0 and above | Full shake, and the cockpit message `OVERSPEED` every four seconds. | The player. |
| 1.5 and above | The aircraft is lost through the ordinary destroyed path. | Every aircraft, the player and the AI. |

The shake is a view offset only. It is a pure function of the ratio and the
simulation time (two noise terms at 23 and 31 Hz, 60 percent and 40 percent), so
it repeats exactly, does not depend on the simulation tick rate and never touches
flight state. Full strength is 0.024 radians (about 1.4 degrees), two and a half
times the G-effect shake. It is added to the look after the G-effect shake.

At 1.5 times the top speed the airframe fails: the same structural failure a
fatal hit produces, so the player gets the destroyed event, the wreck tumbles and
explodes as in [destroyed aircraft](destroyed-aircraft.md), and the AI aircraft
crashes and its wreck is handled by combat. The cause is recorded as `overspeed`.
The debrief shows it (a `Cause` row and a `cause=overspeed` suffix in the summary
line), the mission recording carries it as the `reason` field of the destroyed
event, and the headless flight probe prints `loss: cause=overspeed`. No kill is
credited, because nobody shot the aircraft down.

## What must not fail

Normal flight stays far from the rule: a full afterburner level flight settles at
92 to 100 percent of the top speed, an F-22 supercruises inside its envelope and
the AI sprint cases run to about the top speed. Tests: `an_aircraft_is_lost_at_one_and_a_half_times_its_top_speed`
and `normal_flight_stays_under_the_overspeed_limits` in `flight.rs`, the shake
test in `g_effects.rs`, and the `flight-overspeed-*` battery scenarios (every
aircraft lost at 1.6 times its top speed, and a 60 degree afterburner dive from
40,000 ft that never goes past 1.52 times).

In the dive scenario only the Su-25 reaches 1.5 times its top speed; the others hit
the ground first at 0.86 to 1.4 times.

## Numbers (agent decisions, 2026-09-29)

| Number | Value |
| --- | --- |
| Shake starts | 0.95 of top speed |
| Full shake | 1.0 of top speed, 0.024 rad |
| Loss | 1.5 of top speed |
| Message repeat | 4 s |

Not done: a rumble or sound for the shake (John allowed it as optional).
