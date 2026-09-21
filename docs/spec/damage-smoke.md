# Aircraft damage appearance and combat smoke

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, requested by John on 2026-09-17. Reuse the user's FA damaged
aircraft shapes/textures and smoke artwork. Local source inspection establishes
resource availability and visible geometry, not original damage thresholds or
smoke scheduling. [Resource evidence](../formats/objects-and-shapes.md#combat-damage-and-smoke-resource-review).

John requested incremental localized damage on 2026-09-20. The six-region
model below is an agent-selected fit. Each direct aircraft hit is projected into the aircraft basis and
assigned to one of six regions: nose, cockpit, central fuselage, left wing,
right wing or tail. Region damage accumulates independently. A structural body
and its paired fragment appear only when one region has received at least 75%
of the aircraft's starting hit points. Damage elsewhere cannot make the F/A-18D
nose disappear. The first region to cross the threshold owns the retained
breakup choice through destruction.

The reviewed F/A-18D A/B pair represents nose and cockpit loss and its C/D pair
represents inner or trailing wing loss. The reviewed Rafale A/B pair represents
major left-wing loss and C/D represents vertical-fin loss. The reviewed F-22
A/B pair has the clearer left-wing loss. These reviewed bodies are selected only
for F/A-18D nose/cockpit or left-wing damage, Rafale left-wing or tail damage,
and F-22 left-wing damage. All other aircraft and regions retain the intact body
with fitted surface marks or mesh tears. These are
alternate appearances, not alphabetical severity levels. Render destroyed
target bodies while airborne, using their existing ballistic motion. The paired
B/D piece detaches once at the structural transition. Use each shape's own
texture and never apply intact animation address ranges to a damaged shape. The
original model scale is retained; variant scale parity remains unverified.

Before breakup, persistent local marks use the reviewed dark patch from
`_F18_A.PIC` on every supported aircraft. This cross-aircraft reuse and placement
are agent-selected fitted presentation rules, not evidence that the original
shared this texture. Both textured and flat-colored aircraft surfaces receive
marks, including the Rafale's flat-colored wings. Marks begin at 4% regional damage. Their deterministic face
density is one fifth at 4%, two fifths at 15%, and four fifths at 35%.
Panels of at least eight square feet always receive a mark, an agent-selected
fit so low-polygon aircraft such as the Su-27 cannot omit all light wing damage.
Patch size
is respectively 22%, 40%, and 58% of the marked face. At 35% regional damage,
wing and vertical-fin surfaces begin fitted face clipping. Source-mesh
face centers classify wings beyond 30% of maximum absolute lateral extent and
tail faces behind 25% of maximum absolute longitudinal extent. Elevated aft
faces above 35% of maximum height also count as fins, covering aircraft such as
the Su-35 whose fin centers sit ahead of that longitudinal boundary. They retain
82% of span or height at 35% and 52% at the 75% structural threshold. The
renderer mirrors side-specific clipping, so a right-wing hit does not remove a
reviewed left-wing chunk. Reviewed whole-body pairs are used only for the exact
region mappings above. Other wing and tail regions use the fitted tear and do
not spawn an unrelated B/D fragment. Nose, cockpit and core damage without a
matching reviewed body retain marks without removing unrelated geometry.

Cockpit contact from a gun round is a fitted pilot kill. Direct-hit tests use
bounded volumes inside the broad aircraft collision sphere: cockpit right/up/
forward coordinates are -0.22..0.22, 0.08..0.48 and 0.08..0.48 aircraft radii;
the central critical volume is -0.28..0.28, -0.28..0.22 and -0.38..0.18. The
nose volume is -0.32..0.32, -0.30..0.32 and 0.42..0.92; left and right wing
volumes are -0.92..-0.25 and 0.25..0.92 laterally, -0.18..0.18 vertically and
-0.28..0.38 longitudinally. The tail volume is -0.34..0.34, -0.25..0.40 and
-0.92..-0.34. A gun
round through the central volume is a fitted critical kill when its reduced
damage is at least half the aircraft's starting hit points. Missile blast
contact does not use these direct-hit kill regions. Global hit points and
existing subsystem damage continue to accumulate normally. A critical kill
records only the round's reduced physical damage in its region, so killing the
pilot does not by itself tear off the nose or another structure.

Gun damage is an opinionated change requested by John on 2026-09-20. The six
reviewed aircraft-gun records apply the integer floor of one third of their
configured damage. Values below three therefore apply zero damage. Missile,
bomb and rocket damage is unchanged. The source weapon records remain unchanged. Original runtime regional
thresholds remain unknown.

Emit white missile smoke only during the movement model's powered interval,
including supported compatibility weapons. Guns emit none. Emit dark aircraft
smoke at or below 50% remaining health, while the target remains airborne.
Ownship emits while damaged and alive; residual puffs persist after destruction.
No dark damage smoke is emitted by an undamaged aircraft. Missile motors emit
no smoke before ignition or after burnout. Existing smoke continues to disperse after its source stops or disappears.

Smoke samples use fixed 120 Hz simulation time. As an opinionated change requested
by John on 2026-09-21, missile puffs emit every 8 ticks (15 per second), aircraft
damage puffs every 12 ticks (10 per second), and missile puff radius is halved
throughout its life. Missile/aircraft damage puffs last 4/8 seconds. At 1,200
feet/second missile centers are 80 feet apart; at 600 feet/second aircraft damage
centers are 60 feet apart before rise. Missile/aircraft damage radii start at
2/8 feet and grow by 3/8 feet per second. These retain fitted lifetimes, rise of
2 feet per second and linear fade from 0.65 opacity. Original timing remains
unknown. Rendering continues at the normal frame rate.

Engine contrails are an opinionated addition requested by John on 2026-09-21.
Each engine emits 10 pale puffs per second behind its outlet, including healthy
aircraft. As requested by John on 2026-09-21, each puff now lasts two minutes
(14,400 simulation ticks). It keeps opacity 0.65 through one minute (7,200
ticks), then fades linearly to zero over the final minute. At 1:30 its opacity
is 0.325; at 2:00 it is removed. This replaces the distance-based trail limit.
Speed, turns and distance from the aircraft do not affect puff opacity or life.
Pausing freezes puff age. Puffs remain at their emitted world positions.
Contrails use the reduced missile radius and growth, capped at 14 feet after
four seconds, an agent-selected fit.

Player emission requires an airborne, living aircraft with engine power and
fuel. Other living airborne aircraft emit from their rendered engine positions;
individual target engine power is unavailable, so their emission is fitted.
As requested by John on 2026-09-21, each aircraft has a randomly selected onset
altitude between 30,000 and 35,000 feet above sea level. Both engines share that
threshold. Emission begins at or above it and stops below it, with previously emitted
puffs continuing their normal two-minute life and fade. The agent-selected fit uses
a deterministic pseudorandom hash of aircraft instance ID and sortie counter,
starting at zero and incrementing on reset. This keeps the threshold stable
within a sortie and headless runs reproducible. No weather threshold is imposed.
One outlet is used for A-4E,
X-31, MiG-21 and MiG-23; other supported types have two. Attachment uses the
center of reviewed nozzle lateral/vertical bounds and the aftmost nozzle point,
plus 2 feet aft. A-4E, Su-25, F-22 and F/A-XX use a fitted fallback: 2 feet behind
the model's aftmost point, at body-center height, with twin outlets offset by
15% of the model half-span. These fallback points are not recovered engine
coordinates. Stopping emission or losing the source does not shorten existing
puffs' lives. Restart clears them.

As requested by John on 2026-09-21, contrails, aircraft damage smoke and missile
motor smoke use the same weather lighting as cloud layers. Preserve the original
palette indices until rendering so every view applies its current weather/time
palette, altitude-dependent haze remaps, directional sunset glow and cloud/air
occlusion. White trails must darken or tint with the surrounding clouds at dusk
and night instead of retaining the launch-time aircraft palette. Dark damage
smoke retains its source dark tones under that same lighting. The shared cloud
lighting is fitted host presentation, not newly recovered retail behavior.
Transparency, emission, size and lifetime rules above are unchanged. Bilinear
sampling filters coverage separately from color; apply lighting to the covered
color, then premultiply it by final coverage/opacity for blending. Transparent
edges must not become glowing rectangles when fog or sunset light is applied.

All smoke has no gameplay sensor effect. Combat smoke retains its 8,192-puff
budget. Agent choice: contrails have a separate 72,000-puff budget, enough for
all 30 Quick Mission aircraft with two engines each, 10 puffs per second, and
120 seconds of history. Only populations beyond that bound evict old puffs.
The renderer supports both budgets together, submitting one instance per visible
puff and culling billboards outside each camera view without removing history.
Reset clears all smoke and outlet history. Cosmetic contrails are separate from
combat-service replay state, whose tapes do not record engine power.
Original 43-pixel smoke cells at x=0 (dark) and x=94 (pale), with
palette index 255 keyed transparent, are camera-facing, blended, depth-tested and ordered
back to front. Size, lifetime, placement and opacity are fitted, not retail parity.

Load Ordnance does not show the straight-flight dummy description. Validation
errors and useful loading feedback remain.


## Gun dispersion and luminous tracers

John requested glowing tracers and a 0.5-degree gun cone on 2026-09-20.
The agent-selected convention is a 0.5-degree full cone, at most 0.25 degrees
from the commanded firing direction. Each reviewed gun projectile samples a
uniform solid angle within this cone once at release, deterministically from
its projectile identity. The shared rule covers player and other gun releases for every identity in the
selectable roster: all twelve retail imports plus the F/A-XX concept. Gun
recognition comes from each identity's canonical gun record;
missiles, rockets and bombs retain their existing trajectories. It changes the
actual round trajectory and hit location, not only the drawn tracer. At 1,000
feet, the spread circle is approximately 8.73 feet across.

Tracer luminosity is an agent-selected fitted presentation: an additive warm
white core with a soft amber halo around the actual swept gun segment. The
halo half-width is 1.2 feet and the core is approximately 0.18 feet wide at
half brightness. Soft end fading avoids rectangular streak ends. When viewed end-on, a fitted
0.5-foot-long camera-facing glow preserves visible area. Tracers are
self-lit in daylight and darkness, depth-tested against solid geometry, and
attenuated by fog and dense cloud. They do not write depth or cast shadows.
The halo simulates optical glow; it does not illuminate nearby aircraft or
terrain. No original tracer glow or dispersion value is claimed.

### Individual cannon rounds

John requested a steady stream of individual bullets and intermittent tracers
on 2026-09-21. The agent-selected spacing is one tracer every three bullets,
starting with the first bullet. A visible gun round draws one luminous ribbon;
it does not also draw the source projectile shape. The other two bullets remain
physical collision projectiles without a luminous marker.

The fitted rate preserves the existing host ammunition consumption:
`4 * gameRoundsInBurst * actualRoundsPerGame / gameBurstT` bullets per second,
with zero-valued count/timing fields treated as one. This interprets the existing
quarter-second host burst interval, not an established retail timing unit.
[Source timing uncertainty](../formats/weapons.md#confirmed-gaps-and-fa-checks)
remains unresolved. Individual release times are quantized to the fixed 120 Hz
simulation, with fractional intervals retained across shots. Trigger release
must stop pending bullets; pressing again must not bypass the rate limit.

The six selected canonical records, M61, DEFA, MK12, GSH301, GSH23 and GSH6_30,
all contain four representative rounds, two ammunition units per representative
round and a burst interval of one. Their fitted rate is therefore 32 bullets
per second, with 3- or 4-tick gaps and about 10.67 visible tracers per second.
These are game-model rates, not claims about real cannon cyclic rates.

Each physical bullet consumes one ammunition unit. Over each
`actualRoundsPerGame` group, its damage shares sum to the previous representative
projectile's reduced damage, `floor(configured damage / 3)`. Integer division
assigns the quotient to every bullet and one extra point to the first remainder
bullets. This preserves aggregate all-hit damage instead of multiplying it when
splitting a representative shot. Bullet ordinals persist between trigger pulls.
This exact sum describes the weapon's damage budget against a target class;
the ownship damage path also applies its existing per-hit random attenuation
and rounding, so realized damage is not guaranteed identical to a representative
hit. For class zero, M61/DEFA expected ownship damage per pair changes from
7.475 to 7.0; MK12/GSH23 changes from 2.5 to 2.0. This is a known fitted
difference from independently hittable physical bullets, not an additional
configured damage multiplier. Critical cockpit hits remain lethal physical contacts even when the
target-class damage share rounds to zero. Such contacts do not add fictitious
regional structural damage. Gun dispersion applies to each bullet independently.
Missile, rocket and bomb release rules are unchanged.

Actor-owned guns use the same spacing for physical bullets within an authorized
release. Existing actor decisions still determine when a release is authorized,
so short groups may have longer gaps between them. Their ammunition is already
debited by that service before the bridge receives a release. The bridge must
not debit it again. Existing allocation failures can therefore drop paid-for
bullets; they cannot create free rounds. No autonomous targeting or engagement
policy is changed by this weapon-mechanics correction.

## Detached pieces and ground cleanup

Requested by John on 2026-09-17. A/B and C/D are fitted body/piece pairings.
A piece inherits the aircraft's complete velocity and orientation. Its starting
location is fitted from the largest bounding-box extent lost between the intact
and damaged body, aligning the fragment center to that missing region. This is
not an original attachment transform. Gravity is 32.174 feet/second squared;
a fitted world-axis tumble uses 0.8/0.5/0.2 radians/second. Rendering uses the
fragment's own original geometry and texture, without intact-aircraft animation.

The first swept terrain contact removes the piece immediately and creates one
15-foot, 0.375-second ground-hit animation from `GRDLRGA.PIC`. The animation uses
12 frames in a 3x4 grid of 80x63 source cells, keyed with palette index 255. Its
center is 6 feet above the contact so terrain does not hide it. This small use of
the original ground-explosion art is an agent-selected fit, not a recovered
bullet-impact mapping. There is no resting wreck part or collision obstacle.
Further hits cannot respawn the same piece. Reset clears detached pieces and
impact effects. A 256-piece visual budget bounds the simulation; pieces have no
AI, damage, radar return or independent weapon behavior. Original breakup choices,
trajectories and animation timing remain unverified.
