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

John requested hiding visual airframe damage below 100% on 2026-09-21, pending
a later visual pass. All surviving aircraft retain intact geometry and textures:
no local damage marks, wing/fin tears or missing body sections, including at 92%
and 99%. Damage smoke and ordinary device/control animations remain in use.
This is a presentation gate only. Six-region damage, component failures, flight
penalties, damage reports and destruction continue to work independently.

The six-region model remains an agent-selected fit. Direct hits are assigned to
nose, cockpit, central fuselage, left wing, right wing or tail in aircraft space.
At 100% total damage (zero hit points), regional damage selects the appropriate
reviewed breakup body and paired fragment. Damage elsewhere cannot select the
F/A-18D nose. Internally, the first region to accumulate 75% of starting hit points
reserves that choice, without displaying it or releasing debris while alive.

The reviewed F/A-18D A/B pair represents nose and cockpit loss and its C/D pair
represents inner or trailing wing loss. The reviewed Rafale A/B pair represents
major left-wing loss and C/D represents vertical-fin loss. The reviewed F-22
A/B pair has the clearer left-wing loss. These reviewed bodies are selected only
for F/A-18D nose/cockpit or left-wing damage, Rafale left-wing or tail damage,
and F-22 left-wing damage. All other aircraft and regions retain the intact body
with fitted surface marks or mesh tears. These are
alternate appearances, not alphabetical severity levels. Render destroyed
target bodies while airborne, using [aerodynamic wreck motion and airbursts](destroyed-aircraft.md). The paired
B/D piece detaches once at the structural transition. Use each shape's own
texture and never apply intact animation address ranges to a damaged shape. The
original model scale is retained; variant scale parity remains unverified.

At destruction, persistent local marks use the reviewed dark patch from
`_F18_A.PIC` on every supported aircraft. This cross-aircraft reuse and placement
are agent-selected fitted presentation rules, not evidence that the original
shared this texture. Both textured and flat-colored aircraft surfaces receive
marks, including the Rafale's flat-colored wings. Marks begin at 4% regional damage. Their deterministic face
density is one fifth at 4%, two fifths at 15%, and four fifths at 35%.
Panels of at least eight square feet always receive a mark, an agent-selected
fit so low-polygon aircraft such as the Su-27 cannot omit all light wing damage.
Patch size
is respectively 22%, 40%, and 58% of the marked face. Partial wing/fin tears
retain 82% of span/height at 35% regional damage and 52% at or above 75% regional
damage. This retained tear renderer runs only at 100% total damage for now.
Reviewed whole-body breakup pairs and detached
B/D pieces still require 100% total damage. Source face centers classify wings
beyond 30% of maximum lateral extent and tails behind 25% of maximum longitudinal
extent; elevated aft faces above 35% of maximum height also count as fins.
Side-specific clipping and reviewed body mappings remain unchanged. Nose,
cockpit and core damage without a matching body retain marks without removing
unrelated geometry. [Ownship flight penalties](systems-damage.md#regional-structural-damage)
use the same regional fractions, even while visual tears are suppressed.

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
Ownship emits while damaged and alive, and continues emitting while its destroyed
wreck remains airborne, as requested by John on 2026-09-21. Pilot death does not
stop this trail. Ground impact or an airburst stops new emission; existing puffs
persist and fade through their remaining lifetime.
No dark damage smoke is emitted by an undamaged aircraft. Missile motors emit
no smoke before ignition or after burnout. Existing smoke continues to disperse after its source stops or disappears.
Unguided rockets smoke like missiles while their motor burns (bug B6, fixed
2026-10-12, slice BF): every record with a motor smokes, not only one with a
seeker signature, so a rocket pod's salvo leaves a white trail per rocket.
The bright motor flame drawn over a missile or rocket's path burns only while
the motor does and goes out at burnout; a bomb never shows one (bug B7). Both
follow the same powered interval, so a networked client and a replay agree
with the host. Smoke already in the air keeps drifting and fading after
burnout (agent decision: "stop rendering at burnout" read as no new trail and
no flame, since puffs vanishing in mid-air would look wrong).

Smoke samples use fixed 120 Hz simulation time. As an opinionated change requested
by John on 2026-09-21, missile puffs emit every 8 ticks (15 per second), aircraft
damage puffs every 12 ticks (10 per second), and missile puff radius is halved
throughout its life. Missile/aircraft damage puffs last 4/8 seconds. At 1,200
feet/second missile centers are 80 feet apart; at 600 feet/second aircraft damage
centers are 60 feet apart before rise. Missile/aircraft damage radii start at
2/8 feet and grow by 3/8 feet per second. These retain fitted lifetimes, rise of
2 feet per second and linear fade from 0.65 opacity. As John requested on
2026-09-28, every smoke puff and contrail puff also drifts with the mission
wind from the moment it is released. Original timing remains unknown. Rendering continues at the normal frame rate.

Burning flares leave their own short trail with the white missile puff; its
rules are in [countermeasure presentation](countermeasures.md#flares).

Engine contrails are an opinionated addition requested by John on 2026-09-21.
Each engine emits 10 pale puffs per second behind its outlet, including healthy
aircraft. As requested by John on 2026-09-21, each puff now lasts two minutes
(14,400 simulation ticks). It keeps opacity 0.65 through one minute (7,200
ticks), then fades linearly to zero over the final minute. At 1:30 its opacity
is 0.325; at 2:00 it is removed. This replaces the distance-based trail limit.
Speed, turns and distance from the aircraft do not affect puff opacity or life.
Pausing freezes puff age. Puffs drift from their emitted world positions only
with the mission wind.
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

The red daytime baseline is an agent-selected fitted presentation: an additive
warm white core with a soft amber halo around the actual swept gun segment.
[Night adaptation](#night-tracer-visibility) and [green ammunition](#tracer-colour-families)
modify this baseline as specified below. The baseline halo half-width is 1.2 feet and the core is approximately 0.18 feet wide at
half brightness. Soft end fading avoids rectangular streak ends. When viewed end-on, a fitted
0.5-foot-long camera-facing glow preserves visible area. Tracers are
self-lit in daylight and darkness, depth-tested against solid geometry, and
attenuated by fog and dense cloud. They do not write depth or cast shadows.
The halo simulates optical glow; it does not illuminate nearby aircraft or
terrain. No original tracer glow or dispersion value is claimed.

### Night tracer visibility

John requested brighter, longer-range night tracers on 2026-10-15 and revised
them on 2026-10-16: redder streaks with a hot core, a soft halo at about 75%
of a flare's glow, and twice the previous night visibility range. These are
**opinionated** presentation requests, not recovered retail behaviour. His
night-tracer photograph is a visual reference, not evidence of original gameplay.
All flagged gun rounds share this rule: aircraft, helicopters, AAA, ship guns,
small arms and all three AC-130 guns, in live, multiplayer and replay views.
Impact sparks are excluded. Flak and tank shells remain untraced. No cadence,
trajectory, lifetime, damage or networking changes.

The following numbers are **agent decisions**, fitted to the request and pending
GPU review. Use the existing `device_night()` ramp:
`night = 1 - smoothstep(-0.104528, 0.0348995, sun elevation sine)`.
Keep emitted ribbon radiance at `1 + night`. Evaluate haze and aerial opacity
at `distance / (1 + 5 * night)`, replacing `distance / (1 + 2 * night)`.
At day/midpoint/night the distance divisors are now 1/3.5/6, formerly 1/2/3.
For any atmospheric visibility function V at fixed eye/source altitudes,
`V((2*d)/6) = V(d/3)`: this doubles the old night distance envelope, including
nonlinear aerial opacity. It does not guarantee a measured detection distance
in every view. Dense cloud transmission still uses the actual source distance;
opaque terrain still depth-tests the entire ribbon and halo. The far plane
remains 2,200,000 ft. Projectile lifetime does not grow.

Linear RGB colours, before the retained radiance multiplier:

| Component | Red day (unchanged) | Red night | Green day | Green night |
| --- | --- | --- | --- | --- |
| Core (peak 3) | (1, .92, .68) | (1, .72, .38) | (.78, 1, .68) | (.72, 1, .38) |
| Body (peak .65) | (1, .25, .025) | (1, .09, .025) | (.09, 1, .025) | (.09, 1, .025) |
| Added halo | absent | (1, .06, .025) | absent | (.06, 1, .025) |

The red ribbon's unattenuated centre changes from night RGB
(7.3, 5.845, 4.1125) to (7.3, 4.437, 2.3125), excluding the added halo.
The hot centre can still saturate toward yellow-white. Colours interpolate
continuously through twilight. No random pink variant is introduced: the
reference guides the hot streak and soft red surround, not a new ammunition mix.

Keep the existing 2 render-pixel full ribbon-width floor and unresolved core
Gaussian coefficient 120 toward 6. Side-on ribbons have a 6-pixel length floor
at night so they remain short streaks. End-on rounds keep a 2-pixel length floor.
The halo expands half-width toward `max(core_half_width, 8 pixels)` and length
toward `max(core_length, 16 pixels)`. The narrow core is remapped inside this
larger quad, not enlarged to fill it. Resolved streaks retain their physical
length; close physical widths are not capped. The added expansion is bounded,
not an unbounded world-size glow. All dimensions interpolate through twilight
and use the current view's resolution and zoom. The 105 mm keeps its 1.5
radiance multiplier and 1.75 physical width multiplier; unresolved pixel floors
are common, as before.

**Flare comparison.** Countermeasure flares have an additive post-image glare
halo, plus separate flame/core art, canopy cross-streaks and scene light.
The tracer uses the same halo profile `1.1*exp(-10*r*r) + .4*exp(-3.2*r)`,
in its existing depth-tested additive ribbon draw, with a soft edge window
from .85 to 1 of the quad. At full night its unattenuated peak dominant-channel
halo radiance is **1.125, or 0.75 of the flare halo profile's 1.5 peak**.
This compares the profiles at equal apparent size before any distance or
atmospheric attenuation, excludes hot cores and canopy cross-streaks, and does
not compare the final pixels of a distant countermeasure flare.

The halo uses only the tracer's atmospheric visibility envelope, with the same
unscaled cloud transmission. It has no separate inverse-square glare falloff.
Lead GPU review on 2026-10-16 found that the flare's 3,000 ft glare falloff
made the first implementation's 4 nm halo invisible. Removing that factor is
an agent implementation decision following the lead's requested correction.
The bounded 8-pixel half-width, short streak geometry and colours stay unchanged.
The 105 multiplies radiance by 1.5 as before, giving an unattenuated halo peak
of 1.6875. Twilight halo gain is `night`. Red daytime output is unchanged.

Agent decision: no tracer scene lights, no fire light slots, no additional draw
or vertex upload. Halo and ribbon fade together through the weather-dependent
visibility envelope; the halo does not disappear first because of another
close-range distance law.
An unresolved round occupies at most 16 x 16 pixels at full night, versus the
old 2 x 2 minimum: 5,000 such rounds cover at most 1.28 million quad pixels
before overlap/MSAA (about 62% of 1080p). Long resolved streaks cost more but
only gain bounded width. This is a fill estimate, not measured GPU timing.

TV and ordinary seeker views use visible-light adaptation. NVG receives that
visible scene through its existing logarithmic gain and bloom, without an extra
tracer-specific NVG multiplier. WHOT/BHOT and thermal seeker views retain their
existing tracer geometry, attenuation and heat conversion, with no night boost.
GPU appearance and cost require validation on a display-capable host.

### Tracer colour families

John requested weapon-origin green tracers on 2026-10-16, **opinionated**.
The exact, case-insensitive weapon record selects the family. Every unlisted
record defaults to red. These assignments are **agent decisions for review**,
not claims that every ammunition loading of a real gun has this tracer colour.
Record identities come from [aircraft records](../formats/fa-aircraft.csv),
[weapon records](../formats/fa-weapons.csv) and the surface gun tuning table.

| Green records | Reason for assignment |
| --- | --- |
| GSH23.JT | Soviet GSh-23 family; MiG-21, J-7E and Q-5 records use it |
| GSH30.JT | Soviet GSh-30 family; Yak-141 and one MiG-17 gun record |
| GSH301.JT | Soviet GSh-301 family; MiG-29, Su-25 and Su-27/33/34/35 records |
| GSH6_23.JT | Soviet GSh-6-23 family, Su-24 record |
| GSH6_30.JT | Soviet GSh-6-30 family; MiG-23/27/31 and Su-7 records |
| T12_4.JT | Mi-24's Soviet 12.7 mm rotary gun record |
| T30_1.JT | Ka-50's Soviet 30 mm gun record |
| ZSU23.JT | Shilka's Soviet 23 mm ammunition |
| 2S6.JT | Tunguska's Soviet 30 mm ammunition |
| ZSU57.JT | ZSU-57-2/ZIF31 Soviet 57 mm gun record |
| AAA30.JT | Soviet AK-630 class ship gun record |
| AAA30BAD.JT | Soviet AK-230 class ship gun record |
| M1939.JT, A_M1939.JT | Soviet 61-K 37 mm gun and barrage records |
| BMP2.JT | Soviet 2A42 30 mm vehicle gun |
| BTR80.JT | Soviet KPVT 14.5 mm vehicle gun |
| KS12.JT, KS19.JT | Soviet flak family; no visible effect because these rounds remain untraced |
| T72.JT | Soviet tank-gun family; no visible effect because these rounds remain untraced |

There is no separate S-60 record in the reviewed surface tuning or weapon
catalogue, so none is invented. SMLARMS.JT is shared across sides and stays red;
colour never follows faction. PHALANX.JT, M113.JT and M2.JT stay red. The
catalogue's sometimes surprising aircraft-to-gun mappings are preserved.
Green applies in day and night, with the same night geometry, glow and range.
Thermal views retain their old geometry, palette and heat response.
Red daytime vertex payload and shader arithmetic remain unchanged; full-frame
byte identity awaits the lead's GPU comparisons of red-only scenes.


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

Actor-owned live guns use this same physical cadence and per-round damage.
Their [employment contract](ai-gun-employment.md) supplies burst/recovery timing,
fixed-barrel lead and interruption rules. The host debits each physical round
only after capacity and alignment checks succeed. A refused release consumes
no ammunition. Earlier prepaid representative-group behavior remains in legacy
synthetic fixtures, not imported gun employment.

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
its 12 frames of 76x62 in three columns, as `EXP.SH` lays the sheet out
([format notes](../formats/explosions.md#expsh)), keyed with palette index 255.
It is silent. Its base is 6 feet above the contact so terrain does not hide it. This small use of
the original ground-explosion art is an agent-selected fit, not a recovered
bullet-impact mapping. There is no resting wreck part or collision obstacle.
Further hits cannot respawn the same piece. Reset clears detached pieces and
impact effects. A 256-piece visual budget bounds the simulation; pieces have no
AI, damage, radar return or independent weapon behavior. Original breakup choices,
trajectories and animation timing remain unverified.
