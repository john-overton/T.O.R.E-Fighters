# In-flight cheats

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-23. The behaviour below is **John's recollection of the
retail game**, given in session on 2026-09-23. Retail comparison is unavailable,
so none of it is verified against the original. Numbers John did not give are
marked **proposed**: they are agent proposals awaiting John's review, and ship as
`fitted` until he confirms or replaces them.

## What the player sees and does

Escape opens the flight menu. Its **Cheat** menu is read from the player's
imported `FMENUD.MNU` and contains, in order:

| Entry | Choices |
| --- | --- |
| Damage | Invulnerable / Normal / Realistic |
| Unlimited ammo? | on / off |
| Unlimited fuel? | on / off |
| Easy aiming? | on / off |
| No crashes? | on / off |
| No spins? | on / off |
| No turbulence? | on / off |
| Pull extra G? | on / off |
| Ignore weapon weights? | on / off |
| No sun whiteout? | on / off |
| No redout or blackout? | on / off |
| No screen-shaking? | on / off |
| Enemy AI? | Novice / Average / Unchanged |
| Ignore midair collisions? | on / off |
| Easy targeting? | on / off |
| Air combat guns only? | on / off |

Every cheat takes effect immediately, mid-flight, and all start off. They last
for the session and survive Restart, like the two that already work (No
turbulence, No sun whiteout). **Proposed:** they are not saved to preferences.

## Behaviour of each cheat

**Damage.** Invulnerable means weapon hits do no damage: no hit-point loss, no
system faults, no pilot kill and no breakup. It also survives a midair
collision (John, 2026-09-23). Hits still physically jolt the
aircraft (see [missile hit jolt](#missile-hit-jolt)). Normal is the current
damage model. Realistic is not yet described. Invulnerable does not prevent
ground crashes; that is the separate No crashes cheat.

**Unlimited ammo.** Gun rounds and stores never run out. **Proposed:** applies to
the player only.

**Unlimited fuel.** Fuel never decreases. **Proposed:** player only, and it also
covers leaks from damage.

**No crashes.** The aircraft ricochets off the ground instead of crashing: it
bounces and keeps flying. **Proposed:** the same applies to water and to
buildings. Off the ground or water it keeps its horizontal speed and leaves
at half its impact speed, never slower than 20 ft/s, and a nose pointing down
kicks up to half that angle above the horizon. A safe landing on a runway is
still a landing. Off a building it backs out, turns around and bounces away at
half speed.

**No spins.** The aircraft never departs into a spin. **Proposed:** stall buffet
and stall lift loss still happen; only spin entry is prevented.

**Pull extra G.** Every aircraft can pull up to **9 G**, whatever its normal
limit. **Proposed:** 9 G is available regardless of weapons and fuel load, and
the limit becomes the higher of 9 G and the aircraft's own limit.

**Ignore weapon weights.** Stores add neither weight nor drag. **Proposed:** fuel
carried in external tanks still counts as fuel weight.

**No redout or blackout.** Turns off [G effects](#g-effects).

**No screen-shaking.** Turns off the [high-G screen shake](#high-g-screen-shake).

**Enemy AI.** Changes the skill of every enemy aircraft at once, live. Unchanged
restores each aircraft's mission skill. Friendly AI is unaffected.
**Proposed:** straight-flight fixture aircraft are unaffected, and decisions an
aircraft has already timed keep their old timing until they come due.

**Ignore midair collisions.** Aircraft pass through each other. With the cheat
off, [midair collisions](#midair-collisions) happen.

**Easy targeting.** Gives the player awareness of where the target is at all
times. The radar keeps the target selected while it is off the scope, for as
long as it is flying, so swinging around after it in a merge keeps it; it is
tracked again as soon as it is back on the scope. Outside the HUD the target
square floats over the target instead of becoming an edge arrow, at the HUD's
own size, line weight and brightness (John, 2026-09-23). It is awareness only:
missiles still guide within their normal limits and behaviour, and a target
the sensors have lost gives no radar support.
Depends on [target selection](#target-selection).

**Air combat guns only.** Every aircraft, player and AI, can fire only its gun.
An aircraft without a gun cannot fire. **Proposed:** missiles already in flight
continue; turning the cheat off restores the stores. The player's weapon
selection skips every other station, and a missile selected when the cheat
turns on switches to the gun, or to NAV without one. The Quick Mission Guns
only setting, which removes the other stores at launch, is separate.

**Easy aiming.** Aircraft hitboxes are **50% larger**, missiles get extra
maneuverability, and missile seekers have a **25% wider** tracking cone.
**Proposed:** it helps the player's weapons only; enemy fire against the player
is unchanged. The hitbox scale covers the gun hit sections and the missile fuze
contact size, but not the fuze's own radius. The extra maneuverability is 50%
more turn rate for the player's missiles in flight, and the wider cone applies
to a missile's seeker in flight, not to the lock before launch.

## Systems the cheats need

These are ordinary game behaviour, present with the cheats off.

### G effects

Sustained high positive G greys the view out from the edges inward and, pulled
harder, blacks it out; negative G turns it red. John asked on 2026-09-23 for
the thresholds to follow real human tolerance. Published figures:

- Relaxed tolerance is about 3.5 to 5 G; untrained people black out between 4
  and 6 G.
- A G-suit adds about 1.5 to 2 G, and the anti-G straining manoeuvre adds
  about 3 G more; with both, a trained pilot sustains 9 G.
- Vision goes in order: greyout, tunnel vision, blackout, then loss of
  consciousness, which comes within about 4 to 6 s of sustained high G.
- Redout comes at about -2 to -3 G.

The game has no straining control, so the model is a pilot wearing a G-suit
who is not straining. **Proposed numbers** from those figures:

| Value | Proposed |
| --- | --- |
| Greyout onset | sustained above +5 G |
| Delay | 5 s just over 5 G before any greying, shorter the harder the pull (John, 2026-09-23): 1 s less per extra G, never under 1 s, so 4 s at 6 G and 2 s at 8 G |
| How much vision goes | in proportion to G beyond 5 G: half at 6.25 G (tunnel vision), fully black at 7.5 G and above |
| Closing in | once the delay has passed, at half the view per second, so full blackout about 2 s after the delay |
| Examples | 7.5 G blacks out fully after about 4.5 s; 9 G after about 3 s; 6 G narrows the view to 40% loss after 4 s and holds there |
| Redout onset | sustained below -2 G |
| Redout delay | 3 s before any reddening (John, 2026-09-23) |
| Full redout | at -3 G and below, about 2 s after the delay |
| Recovery | vision returns over about 3 s once G is back inside the limits |
| Delay after an unload | the used delay drains over the same 3 s, so a brief unload does not reset it |
| Appearance | the edges darken first, then the whole view; redout is a deep red |
| Views | every flight view; the map and menus stay readable |
| Controls | still respond; only vision is affected (John, 2026-09-23). There is no loss of consciousness |

Sources: [G-LOC](https://en.wikipedia.org/wiki/G-LOC),
[G-suit](https://en.wikipedia.org/wiki/G-suit),
[Greyout](https://en.wikipedia.org/wiki/Greyout),
[Redout](https://en.wikipedia.org/wiki/Redout).

### High-G screen shake

The view shakes naturally under stress, starting at 6 G and growing stronger
with G. **Proposed:** none below 6 G, rising smoothly to full strength at 9 G,
about 4 pixels at 640 by 480 in the default view, at a rapid, irregular rate
(about 12 to 18 Hz). It shakes the cockpit and outside views, not the external
camera views.

### Missile hit jolt

A missile hit knocks the aircraft around, whether or not it does damage, and
also with Invulnerable on. It applies to AI aircraft too. Gun hits do not jolt.
**Proposed numbers:**

| Value | Proposed |
| --- | --- |
| Roll kick | 60 degrees per second, away from the side of the burst; at least 30% of that for a burst straight behind |
| Pitch kick | 30 degrees per second, away from a burst above or below |
| Yaw kick | 15 degrees per second, away from the side of the burst |
| Push | 15 ft/s away from the burst |
| Fade | the kick halves about every 0.1 s and is gone within 1.5 s |
| Warhead scale | the warhead's damage against that aircraft over 100, between 0.5 and 2 (an AIM-9M is 1, an AIM-54C is 2) |

### Midair collisions

Aircraft that touch collide, and a midair collision is always fatal to every
aircraft involved, player and AI, except a player with Invulnerable on, who
survives while the other aircraft is still destroyed. Ignore midair collisions
turns collisions off. **Proposed:** aircraft touch when their paths come
within 56 ft of each other (two 28 ft contact spheres, the size the game
already uses for weapon hits), only airborne live aircraft collide, and a
collision credits no kill.

### Target selection

Easy targeting builds on the retail targeting controls: T cycles radar
contacts, Enter selects a visible sensor contact, and a target the radar loses
drops completely. The rules and numbers are in the
[radar specification](radar.md#target-selection-keys).

## Loadout cheat

The loadout screen has its own **Cheat** button, next to Unload All. Pressing it
unloads every station and toggles cheat loading; pressing it again unloads and
returns to normal loading. With cheat loading on, the airbase's stock limits no
longer apply and any store can go on any station, up to that station's
capacity. Normal rules still apply to fixed stations such as internal guns, to
a store the station carries by default, and to a missile or bomb type the
station's default rules out. The loadout still accepts only stores the rebuild
supports in flight. Details: [ordnance menu format](../formats/ordnance-menu.md).

## Edge cases

- Cheats can be toggled while paused; they apply on the next simulation step.
- Turning Pull extra G off while above the normal limit lets the aircraft
  unload to its limit through the normal control response, not instantly.
- Enemy AI set to Novice clears extra remembered contacts at the next target
  choice, as the Novice memory limit already does.

## Unknown

- **Damage, Realistic:** how it differs from Normal.
- Whether Unlimited ammo and Unlimited fuel covered AI aircraft.
- Loadout cheat: what "participant count above one" means to a player, and the
  exact projectile flag the station default checks.
- All numbers marked proposed above.

## Source notes

- Menu labels and order: the player's imported `FMENUD.MNU`, read with
  `tore_formats::ui::flight_menu`. See [menu format](../formats/menu.md).
- Behaviour: John's recollection, 2026-09-23.
- Loadout cheat: recovered from the original executable, see the
  [ordnance menu format](../formats/ordnance-menu.md).
- The native flight path carries the original cheat switches for extra G, no
  spins and empty weight (`crates/tore-formats/src/flight_model/`). Its extra-G
  switch adds 1 G beyond the envelope rather than allowing 9 G.
