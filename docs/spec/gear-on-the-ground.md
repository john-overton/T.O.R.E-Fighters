# Gear retraction on the ground

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Requested behavior

Implementation mode. John requested this on 2026-09-29. It is `opinionated`: the
retail game's behaviour for a gear-up on the runway has not been traced. The
numbers are agent decisions.

Retracting the gear on the ground stays possible, but it is physically
consistent. An aircraft that already flies retracts as normal; one that is not
in its lift envelope settles on its belly.

The rule uses the existing contact code (the same gear-position measure the
gear-up landing uses: gear below 0.99 of its travel means the belly is the
contact surface); there is no second path. Airborne, an unsafe touchdown with the
gear up is still the existing landing crash.

While the aircraft is on the ground and the gear is not fully down:

- **Scrape.** The share of the belly on the ground grows with the retraction: none
  with the gear down, full once the gear is half retracted.
- **Brake.** A belly slide decelerates the aircraft at up to 45 ft/s^2 (about
  1.4 G) times the scrape and the wheel load, on top of the tires' own rolling
  resistance (the larger applies), so no fighter's thrust carries it through the slide.
- **Load.** Below the aircraft's 1 G stall speed the whole aircraft rests on the
  belly, at least 0.8 of its weight, whatever the elevator asks for. Above that
  speed the wheel load falls as lift takes over, which is why retraction at
  rotation speed or in the air has no effect.
- **Damage.** The slide wears the airframe: 1 percent of its hit points a second
  plus 6 percent a second at 250 ft/s (148 knots), in proportion to speed, times
  the scrape and the wheel load. The player loses whole hit points from the
  combat hit points; an AI aircraft loses them from its target row. Finishing
  the aircraft credits no kill: an AI aircraft whose belly wear takes its hit points to
  zero is recorded lost without credit (`Ledger::lose_without_credit`) even if a shooter had
  damaged it earlier, and the debrief counts it as a lost aircraft. Invulnerable spares the player.
- **Message.** `Gear up on the ground: belly scraping`, once per slide.

Sparks and smoke: not added, the effects layer has no hook for ground contact.

Already flying: retraction is normal, with no scrape or message. The legacy
adapter has no ground contact of this kind and is unchanged. Only the researched
(hybrid) contact model has the rule.

## What it looks like

With the [weight-scaled stall speed](takeoff-ground-contact.md#weight-scaled-stall-speed)
every aircraft is below its stall speed at 80 knots. An F/A-18D pulled to gear up at 80
knots on the runway with full afterburner slows to about 5 knots (thrust against belly
friction) and loses 26 percent of its airframe in 75 seconds; the Su-27 (88 percent) and
Su-25 (15 percent) belly-slide too, where before the change they flew on; a MiG-29 or X-31
with more thrust than the friction creeps up in speed, wears more than its whole airframe
and, in the game, is lost. The F-22 family's thrust carries it through the slide to its
liftoff speed of about 99 knots with 1 percent wear. A gear-up 50 feet above the runway
is normal for every aircraft.

## Tests

`retracting_the_gear_on_the_ground_slides_on_the_belly_and_wears_the_airframe` and
`retracting_the_gear_in_the_air_is_normal` in `flight.rs`;
`belly_scrape_wears_hit_points_in_whole_points_and_invulnerable_spares_it` in
`combat/live.rs`. Battery: `flight-belly-early-*` (`--maneuver takeoff-gear-early`, gear up
at 80 knots) and `flight-belly-airborne-*` (`--maneuver takeoff-gear-airborne`) for
all 14 aircraft, and the windowed replay lane `replay-script-takeoff-gear-early`
(input script pressing G at about 45 knots). The replay lane's takeoff scripts
now press G after liftoff.

## Numbers (agent decisions, 2026-09-29)

| Number | Value |
| --- | --- |
| Belly deceleration at full scrape | 45 ft/s^2 |
| Full scrape at | gear half retracted |
| Minimum load below stall speed | 0.8 of weight |
| Wear | 1 percent a second, plus 6 percent a second at 250 ft/s, in proportion to speed |
