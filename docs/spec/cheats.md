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
| Easy flight physics? | on / off |

The last row, **Easy flight physics?**, is not in the retail menu: it is
appended after the imported rows (opinionated, John, 2026-10-08).

Every cheat takes effect immediately, mid-flight, and all start off. They last
for the session and survive Restart, like the two that already work (No
turbulence, No sun whiteout). **Proposed:** they are not saved to preferences.

## Behaviour of each cheat

**Damage.** Invulnerable means weapon hits do no damage: no hit-point loss, no
system faults, no pilot kill and no breakup. It also survives a midair
collision (John, 2026-09-23). Hits still physically jolt the
aircraft (see [missile hit jolt](#missile-hit-jolt)). Invulnerable does not
prevent ground crashes; that is the separate No crashes cheat. It prevents the
time-based [overspeed](overspeed.md) loss (the shake and the `OVERSPEED`
message remain) but not the [out-of-bounds](world-edge.md) loss 105 nautical miles
past the map (John, 2026-09-29).

Normal, the starting choice, takes hit points only: each hit lowers the
aircraft's hit points by its damage and the aircraft is destroyed when they run
out, with no [system faults](systems-damage.md) (John, 2026-09-28). Realistic
takes the same hit points and adds the system faults (John, 2026-09-28).
**Agent decision:** the two one-shot gun rules also belong to Realistic, since
they are not hit-point damage: a direct gun hit on the cockpit kills the pilot,
and a direct gun hit on the core worth at least half the aircraft's hit points
destroys it. Under Normal those hits take their own damage like any other.
Damage covers the player only. AI aircraft, friendly and enemy, always take
Realistic damage whatever the setting (John, 2026-09-28); see
[AI aircraft](systems-damage.md#ai-aircraft). Damage smoke, the damaged look
and breakup on destruction follow hit points and appear under both.

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

**Easy flight physics.** Opinionated, John, 2026-10-08 (decision 1 of the
[VTOL and helicopter overhaul](powered-lift-flight.md#easy-flight-physics)); the numbers are fitted and
John expects to tune them. The six powered-lift aircraft (AV-8, Yak-141, V-22,
AH-64, Mi-24 and CH-47) fly the same physics as ever, with the hazards that
punish a careless pilot removed. Fixed-wing aircraft ignore it. It turns off:

| Hazard | With the cheat on |
| --- | --- |
| Main rotor torque | Torque and the tail rotor's imbalance cancel; a collective change does not yaw the aircraft. The CH-47's two rotors and the V-22's two proprotors turn opposite ways and cancel anyway, so for them nothing changes |
| Vortex ring state | Momentum inflow only: no extra sink, no buffet, no loss of cyclic authority, so full collective arrests a vertical descent |
| Retreating blade stall | No pitch-up, roll or thrust loss past the never-exceed speed; the vibration cue (shake and `blade_stall` warning) stays |
| Rotor stall | Rotor speed cannot fall below 85 percent in flight (the V-22 on its downstops flies at 84 percent, so its floor is 84), so an engine failure never makes the rotor unrecoverable (autorotation still needs the collective down to keep lift); on the ground it is not held |
| Harrier and Yak-141 low-speed roll-off | The intake momentum drag's yawing moment and the jet-induced dihedral are removed |
| Undamped puffers at stability level Off | A jet at Off gets the Damper's rate damping on its puffers (hydraulics permitting). Helicopters at Off stay at Off |
| Dynamic rollover | Off; tipping on the ground follows the normal contact rules |

Measured on the starting values (design document, P8 and P8b notes). With the
cheat on, a 30 percent collective step with the pedals fixed turns the CH-47
under 1 degree a second (0.4 without the cheat at Off, the two rotors' torques
cancel either way) and the AH-64 under 1 (against 26). Full collective stops a
vertical descent at one hover induced velocity in 0.6 s (CH-47), 2.4 s (V-22)
and 1.9 s (Mi-24), against more than 3 s with the hazard; the AH-64 takes 5.9
s because its rotor responds slowly. A CH-47 dived past its never-exceed speed
pitches up 7 degrees in 2 s with the cheat against 23 without. Level top
speed, climb rate and the loaded-jet hover limit are the same with the cheat
on (within 1 percent for the CH-47 and V-22 top speeds, within 2 percent for
their climb rates).

It also gives the helicopters, and the V-22 in proportion to its helicopter
mode, a weak **attitude retention** at stability levels Damper and Off
(opinionated, John, 2026-10-08; fitted). Release the stick and pitch and roll
slowly return to the trim attitude. The V-22's share of it is full with the
nacelles at 75 degrees and above, fades out to nothing by 30 degrees and is
absent in airplane mode (which has its own handling). Where it acts it is the Attitude level's hold at 0.6 of its
gain and limited to 10 percent of travel (the Damper's cap is 20), so a few
taps of forward cyclic trim (Ctrl+Up) settle the aircraft in steady forward
flight hands-off. With the cheat on the cyclic trim keys move the cyclic and
the attitude it returns to together (10 percent of trim is 5 degrees of
pitch), and Trim set captures the current attitude. A CH-47 with 10 percent
of forward trim settles at about 130 kt at Off and 157 at Damper. It holds nothing else:
no speed, height, position or heading, and it does not hover the aircraft.
The Damper itself stays rate damping only, and with the cheat off nothing
changes. The AH-64 with 10 percent of forward trim settles at about 90 kt.

It keeps: weight, power and thrust limits, inertia, translational lift,
ground effect, the rule that a loaded jet cannot hover, engine failure, the
V-22's conversion corridor protection and rotor strike. The aircraft still
needs to be flown. A trim made while it is on (a start, or the trim routine)
is trimmed without the hazards. When a human gives an aircraft back to the
AI, the cheat goes with the human (an AI aircraft carries no cheats).

In a multiplayer session it changes the simulation, so only the server sets it,
like the other mission cheats: the Cheat row is not in a session's menu, the
King's Realism page edits it with the other mission cheats, a dedicated
server's `cheats` list takes `easy-physics`, and every client's prediction
runs the same rules because the cheat travels in the exact flight state. A
recording notes it switching on and off like any cheat.

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
supports in flight. Its catalog hides unsupported weapons in both modes and
shows all imported supported weapons with Cheat on, following the
[catalog availability rule](ordnance-presentation.md#catalog-availability).
Details: [ordnance menu format](../formats/ordnance-menu.md).

## Edge cases

- Cheats can be toggled while paused; they apply on the next simulation step.
- Turning Pull extra G off while above the normal limit lets the aircraft
  unload to its limit through the normal control response, not instantly.
- Enemy AI set to Novice clears extra remembered contacts at the next target
  choice, as the Novice memory limit already does.

## Unknown

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
