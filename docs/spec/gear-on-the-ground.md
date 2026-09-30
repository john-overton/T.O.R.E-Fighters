# Gear retraction on the ground

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## The rule

Implementation mode. The retail rule is `spec-derived` from John's observation of
the retail game (2026-09-30): the gear cannot be raised while the aircraft has
weight on its wheels, and the gear key does nothing then. The retail game shows no
message and its rule has not been traced further. The message is `opinionated`,
requested by John on 2026-09-30.

- **Weight on the wheels** means the researched contact model has the aircraft
  rolling on the ground. It is the same state that gates the takeoff roll, the
  brakes and the landing grade, and it ends the same moment the aircraft counts
  as lifted off (the wheel load is gone and the aircraft is climbing).
- **On the ground, the gear key does nothing.** A press that would raise the gear
  (the gear key, or a raise command from any other input) leaves it down. Pressing
  it to lower the gear, or when the gear is already down, is not affected.
- **The message.** Every refused press shows `Ground sensor preventing gear
  retraction`, once per press, in the seat that pressed it. It is a seat-addressed
  message cue like every other cockpit message, so in a multiplayer mission each
  human-flown plane gets its own and no other seat sees it.
- **Every aircraft.** The rule is in the flight state, so it applies to every
  human-flown plane in every seat and to AI aircraft alike. An AI aircraft raises
  its gear only once it is climbing away, so the battery never sees it refused.
- **Once the main wheels leave the ground** the key works as before: the gear
  retracts at the aircraft's own speed. Raising it a moment after liftoff is
  normal. A touchdown with the gear not fully down is still the existing landing
  crash; nothing changed there.
- **The legacy adapter** has no ground contact, so it never holds the gear.

This replaces the belly slide that the gear key started on the runway (added
2026-09-29). With the gear held down, no gear-up ground contact is reachable in
play: an airborne touchdown with the gear up is the landing crash, and nothing
else puts an aircraft on the ground with its gear retracted. The belly contact
(scrape, brake, load, wear, its message, and the wear carried by an AI aircraft or
handed over with a plane) was removed rather than kept unreachable. Losses to
overspeed and to the map edge still credit no kill; only belly wear is gone from
that list. See
[behaviour provenance](../behavior-provenance.md).

## What it looks like

An F/A-18D at 80 knots on the runway with the gear key pressed carries on rolling
with the gear down, and the cockpit shows the message; the same at rotation speed
and up to the moment it lifts. A second later, in the air, the key raises the gear
and the gear-up sound and animation play as before.

## Tests

`the_ground_sensor_keeps_the_gear_down_with_weight_on_the_wheels`,
`the_gear_key_works_again_once_the_wheels_leave_the_ground`,
`the_legacy_adapter_has_no_ground_sensor` and `retracting_the_gear_in_the_air_is_normal`
in `flight.rs`; `a_refused_gear_press_is_a_message_for_its_own_seat_only` in the
world tick tests carries the seat addressing. Battery: `flight-belly-early-*`
(`--maneuver takeoff-gear-early`, key pressed at 80 knots: one message, gear still
down) and `flight-belly-airborne-*` (`--maneuver takeoff-gear-airborne`, no message,
gear up) for all 14 aircraft, and the windowed replay lane
`replay-script-takeoff-gear-early` (input script pressing G at about 45 knots: the
message in the debrief-side event log, gear still down). The scenario names keep
"belly" so their history stays comparable. The replay lane's takeoff scripts press G
after liftoff.
