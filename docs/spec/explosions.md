# Explosions, craters and crash sites

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## What the player sees and hears

Every hit, kill and ground strike shows one of the original's 24 explosion
types, each with its own animation sheet, size, length and set of recordings.
A weapon names three types: one for hitting an aircraft or object, one for
land and one for water. A gun strike puffs dirt or splashes water, a missile
hit bursts in a large fireball, and a heavy bomb raises a big ground blast.
Some types sometimes show a related type instead, so repeated hits vary.
A destroyed aircraft adds its own type 30 blast; a destroyed ground object adds
its unit's own blast (a large ground blast when it has none).

Each impact is drawn at its weapon's size: a cannon shell's burst is half
its type's size, the AC-130's 105 mm like a 500 lb bomb, and a bomb or warhead grows
with its weight, so a 2,000 lb bomb's blast is clearly bigger than a 500 lb
bomb's (see [size by weapon](#size-by-weapon)). Every impact also throws a
brief flash of light that lights the ground around it at night and a spray
of glowing sparks ([impact light and sparks](#impact-light-and-sparks)).

Every large ground or sea explosion also throws out a shockwave: a low skirt
of dust (white spray on water) that races out from the blast, slows and fades
while the fireball burns. What the blast throws into the air depends on the
theater's climate ([climate](#climate)): in a humid theater a bomb or a big
warhead is wrapped in a brief white condensation cloud, in an arid one it
kicks up wide tan dust that hangs over the spot for 30 to 60 seconds; a
smaller missile throws a smaller version and a gun hit none.

A weapon that strikes land with a crater size leaves one of three crater
pictures lying flat on the ground, sized with the explosion by weapon, and
a gun round leaves a small pock mark. Every crater and pock mark stays for
the rest of the mission, however many there are
([craters by weapon](#craters-by-weapon)).

As John requested on 2026-09-28, every aircraft that crashes on land, the
player's or any AI's, leaves a crash site for 15 minutes: a large ground blast,
a crater, a fire and a column of dark smoke rising from the spot. The fire
crackles for anyone within 2,000 feet and fades out over its final minute. A
crash into the sea shows a large water blast and leaves nothing behind. An
aircraft that explodes in the air leaves no site.

## Ground mark occlusion

Opinionated requirement, John, 2026-10-14: bomb and weapon craters, gun pock
marks and wreck/building scorch marks stay beneath vehicles, people,
buildings and wrecks, including in magnified target cameras and the AC-130
console. The four console picture modes and replays use the same geometry.
The depth tie break is fitted by the agent: sixteen reversed
floating-point raster depth bias units plus half a unit of slope-scaled bias
(half the mark's steepest depth change per pixel), with zero world-space lift
and no zoom multiplier. The constant alone left a triangle of a crater
missing on a 15 degree hillside at an external view, and at grazing views
even on level ground; the slope term follows the view angle and zoom, and
anything from 0.1 to 4.5 passes the whole `depth_tests` sweep (15 and 30
degree slopes and level ground, external to 600,000 ft sensor ranges,
grazing and overhead, craters and pock marks, with six-foot roofs staying
uncovered); above 5 a roof is covered at grazing views.
It replaces the old view-ray pull of up to 20 ft. Standing flames, smoke and
explosions remain depth-tested effects, not flat ground marks. Colour-6 ground
shadows use the separate [scenery path](../formats/objects-and-shapes.md#ground-shadow-decals-2026-10-13).

Each ground mark samples terrain or runway pavement at its four corners,
keeping its horizontal footprint and joining those heights with two triangles
(agent implementation choice, following lead review on 2026-10-14). Sampling
uses double-precision world coordinates so distant map coordinates do not
bury corners through rounding. Buildings and vehicles are not receivers.
Standing fire and billboard effects do not sample terrain. No camera-facing
pull or world-space lift is added; the raster bias still resolves
coplanar ties. Across a terrain crease, the two triangles interpolate the
four sampled corners rather than subdividing at every terrain edge.

## Numbers

Explosion types (FA table, [format notes](../formats/explosions.md)):

| Type | Used by | Sheet | Size | Length | Recordings | Full within / silent at |
| --- | --- | --- | ---: | ---: | --- | --- |
| 15 | Gun on land | `GRNDSML` | 75 | 1 s | `&BULLTS1`, `&BULLTS4` | 100 / 10,000 ft |
| 16 | Variant of 15 | `DIRTEXP` | 45 | 1 s | as 15 | 100 / 10,000 ft |
| 17 | Gun or missile in water | `WATSML` | 50 | 1 s | `&SPLASH3` | 100 / 10,000 ft |
| 18 | Gun on an aircraft | `AIRSML` | 50 | 1 s | `&BULLTS2`, `&BULLTS3` | 100 / 10,000 ft |
| 19, 20 | Variants of 18 | `AIRSMLA`, `AIRSMLB2` | 60, 40 | 1 s | as 18 | 100 / 10,000 ft |
| 21 to 23 | Missiles and bombs on land | `GRNDMED`, `GRNDMED3` | 400 | 2 s | `&MEDEXP1-2`, `&EXPL3`, `&EXPL7`, `&EXPL9`, `&EXPL10` | 1,000 / 20,000 ft |
| 24 to 26 | Variants of 18 and 30 | `AIRMED`, `AIRMED2`, `AIRMED3` | 200 | 1 s | `&AIREXP1-2` | 1,000 / 25,000 ft |
| 27 | Heavy flak | `FLAKA` | 130 | 2 s | `&AIREXP1-2`, `&EXPL3`, `&EXPL9` | 1,000 / 25,000 ft |
| 28, 29 | Variants of 18 and 30 | `FLAKB`, `FLAKC` | 170, 180 | 1 s | `&AIREXP1-2`, `&AIREXP4-5` | 1,000 / 25,000 ft |
| 30 to 33 | Missile hits, aircraft kills | `AIRLRG`, `AIRLRGAG`, `AIRLRGC`, `AIRLRGD` | 300 | 1 s | `&AIREXP1-3` | 2,000 / 25,000 ft |
| 34 | Missiles and bombs in water | `WATLRG` | 500 | 2 s | `&WTREXP1-2` | 1,000 / 25,000 ft |
| 35 to 37 | Heavy bombs, SAMs on land | `GRNDLRG`, `GRNDLRG2`, `GRDLRGA` | 400, 400, 380 | 2 s | `&BIGEXP1-2` (35 also `&EXPL12`) | 3,000 / 25,000 ft |
| 38 | Electromagnetic pulse | `EMPEX` | 250 | 1 s | `&EMPEXP` | 1,000 / 25,000 ft |

Sizes are rolled at 66 to 131 percent and capped at 255. Types 15 to 17, 21 to
23 and 34 to 37 sit on the surface; the others float at the point of impact. Every recording plays at the original's full level.

Variety: type 18 shows 19, 20, 28 or 29 about half the time and 24 to 26 one
time in thirty; type 30 shows 24 to 26 a quarter of the time, 31 to 33 about
42 percent of the time and 28 or 29 about 2 percent; 15 shows 16 half the time; 21 shows 23 about a third of
the time; 35 shows 36 or 37 two times in three. Exact chances are in the
[format notes](../formats/explosions.md#explosion-table).

Flak: the KS-12's 85 mm shell bursts as type 27 (`FLAKA`, two seconds) and the
KS-19's 100 mm shell as type 28 (`FLAKB`, one second, heavier sounds, size
170); the retail records of both name 27, so the larger calibre's type is
fitted. Each burst throws a flash of light and leaves a dark puff that hangs for
about four seconds ([surface defenses](surface-defenses.md#flak-bursts-gunfire-launches-and-light)).

Craters: half width 16 feet per unit of the weapon's crater size, at most 333
feet. In the records guns have no crater; air-to-air missiles 3 (96 feet across); rockets 1
or 2; anti-ship missiles 6; air-to-ground missiles and 500 and 1,000 lb class
bombs 9; SAMs 12; guided 2,000 lb bombs 15; the Mk 84 18 (576 feet across).
Since 2026-10-12 the drawn crater follows the weapon's scale instead
([craters by weapon](#craters-by-weapon)).

Shockwave (agent design, 2026-10-10, overhauled in slice B12 on
2026-10-12): explosion types 21 to 23 and 35 to 37 throw out a low skirt of
`SMOKE.PIC`'s grey dust puff in two staggered rows, 24 on the leading edge
and 12 larger, fainter ones behind it at 62 percent of its radius; each puff
is squat (55 percent as tall as a round puff), stands on the ground, and has
its own size (within 25 percent) and place (within 8 percent of the radius),
drawn from the blast's position. The edge reaches 1.35 times the explosion's
drawn width in half a second (a cubic ease-out, as a blast wave slows: about
690 feet across for a 500 lb bomb, 1,090 feet for a 2,000 lb bomb), then
drifts out another 0.2 widths while it fades over the rest of the
explosion's two seconds; it fades in over four ticks, the leading row at 80
percent and the trailing row at 50 percent opacity, and the puffs grow from
0.14 to 0.46 of the width across. Everything is in widths, so a missile's
ring and a heavy bomb's look alike at their own sizes. The ring is coloured
by the climate: tan in dry air, the puff's own pale grey in humid. Type 34
in the water keeps the earlier white spray ring unchanged (John,
2026-10-12): 32 white puffs out to 1.6 widths in 0.7 seconds, drifting a
further 0.15 widths, each growing from 0.12 to 0.5 of the width. No other
type has a ring. The puffs are lit by the scene like the craters, not
self-lit like the fireball, and count against the effect sprites' budget (the
1,024 of the 4,096 not kept for craters, the nearest first): 36 a large
blast for its two seconds.

### Climate

Opinionated, John, 2026-10-12 (slice B12, the shockwave overhaul): each
theater has a fixed humidity, and a large blast throws vapor or dust by it.
The original has no such setting; the look is custom, not retail.

Humidity, 0 (desert) to 1 (jungle), fitted (John named the arid and humid
ends; the rest are the agent's estimates from each region's climate):

| Theater | Humidity | Reads as |
| --- | ---: | --- |
| Egypt (EGY) | 0.10 | arid |
| Iraq (IRA) | 0.15 | arid |
| Persian Gulf (PGU) | 0.20 | arid |
| Pakistan (SPA) | 0.30 | dry |
| Greece (GRE) | 0.40 | dry summer |
| Ukraine (UKR) | 0.50 | temperate |
| France (FRA), Falklands (LFA), Vladivostok (VLA) | 0.60 | temperate |
| Baltics (BAL), South Korea (NSK) | 0.65 | temperate, damp |
| Kuril Islands (KURILE) | 0.80 | foggy maritime |
| Taiwan (WTA), Cuba (CUB) | 0.85 | humid |
| North Vietnam (TVIET) | 0.90 | humid |
| Panama (APA) | 0.95 | humid |

The mission's weather pushes it toward humid: fog (or rain, which the
retail weather choices do not have) adds 0.35, an overcast deck 0.15, at
most 1. A Ukraine mission in fog therefore throws the humid look.

Which blasts: only a weapon's own hit or ground strike drawn as one of the
large land types (21 to 23, 35 to 37). A kill's or crash's own blast, a
dud's puff, an air burst and a water strike raise nothing (water keeps its
spray). The effect carries the weapon's scale but not its kind, so the class
is read from the scale (agent decision):

| Class | Scale | Weapons |
| --- | --- | --- |
| Full | 79 % or more (a 250 lb bomb's) | every bomb, the AC-130's 105 mm (drawn like a 500 lb bomb), and warheads of about 250 lb or more: AGM-65G, Harpoon, AS-14, SS-N-9 |
| Small | under 79 % | smaller missiles and rockets: AGM-65A, Sidewinders, 2.75 in rockets |
| None | | gun hits (types 15, 16, 18 to 20), small splashes (17) |

- **Condensation cloud** (humid). Vapor strength rises smoothly from 0 at
  humidity 0.45 to full at 0.85 (a smoothstep: none in the arid and dry
  theaters, a trace in Ukraine, a third in France, half in the Baltics and
  South Korea, nearly full in the Kurils, full in Taiwan, Cuba, Vietnam and
  Panama). 16 white puffs
  (the small version 8, at 70 % of the size) in two staggered rings round
  the blast, two in three low (12 degrees up from the blast's middle) and
  the rest higher (40 degrees), none over the fireball, which shows through.
  The rings swell to 0.75 of the drawn width in 0.35 s times the square root
  of the scale, the puffs growing from 0.2 to 0.38 of the width across, at 35
  % opacity times the vapor strength, fading in over 6 ticks and out
  quadratically, gone after 2.5 s times the square root of the scale (2.5 s
  for a 500 lb bomb, 3.2 s for a 2,000 lb).
- **Dust.** 10 plus 8 per unit of scale puffs (at most 24: 18 for a 500 lb
  bomb, 23 for a 2,000 lb); the small version 3 plus 4 per unit (at most 8)
  at 70 % of the size. They lie out to the drawn width from the blast (15 to
  100 % of it), roll out a further 35 % over 6 s, rise 1.2 ft/s times the
  square root of the scale, stand on the ground, and grow as they thin.
  Each is the white smoke puff tinted from tan (0.88, 0.72, 0.50) in the
  driest air to grey-brown (0.56, 0.52, 0.46) in the wettest, 0.34 of the
  width across at first, wider in dry air (75 % of that width in the wettest,
  100 % in the driest). Opacity 40 % times 0.35 plus 0.65 times the
  dryness, fading in over 0.4 s behind the fireball, held for the first 40 %
  of its life, then thinning out. The puffs' edges are feathered and they
  dim to 35 % after dark, so they do not glow over the night ground.
- **How long dust hangs** (John: 30 to 60 s by blast size): in the driest air
  30 s at a 250 lb bomb's scale, growing in a straight line with the scale to
  60 s at a 2,000 lb bomb's and beyond (a 500 lb bomb about 38 s, 1,000 lb
  about 48 s); the small version 15 s at the 50 % floor to 30 s at 79 %. Damp
  air settles it sooner, down to half in the most humid air (agent decision):
  a 2,000 lb bomb's dust hangs 57 s in Egypt and 33 s in Vietnam.
- **Budget** (agent decision): the newest 32 impacts that raise dust are kept
  up; a 33rd retires the oldest, which fades out over 3 s instead of
  vanishing, at most 8 fading at once. At most 960 dust and vapor puffs are
  drawn in a picture, the newest impacts first (40 impacts at the full 24).
  They are drawn with the smoke puffs, not the effect sprites, so they never
  take room from the craters' or the explosions' 4,096 sprites, and they are
  remembered apart from the 96 impacts kept for flashes and smoke, so a long
  gun burst cannot push a bomb's dust out.
- Presentation only: everything comes from the effect (where, which type,
  its scale and how long ago) and the mission's humidity, which is fixed by
  its theater and weather, with each puff's place and size drawn from the
  blast's exact position. A networked client and a replay show the same; no
  simulation state, wire field or recording changes (protocol stays 26). As
  with the impact smoke, dust from before a replay seek or before a player
  joins is not shown.
- Dust does not drift with the mission wind (the picture does not know the
  wind); it rises and rolls out in place.

### Size by weapon

Opinionated, John, 2026-10-12 (bugs B8 to B10). Before it, an explosion's
size came from its type alone: a 20 mm and a 105 mm round, or a 500 lb and a
2,000 lb bomb, drew the same. Now the impact effect carries a scale, the
percent of its type's rolled size it is drawn at, and the fireball and its
shockwave ring both grow with it:

| Weapon | Scale | Example |
| --- | --- | --- |
| Guns up to 40 mm: every aircraft gun, the AC-130's 25 mm and 40 mm, AAA (ZSU-23, 2S6, M163, Phalanx, AK-630 and AK-230, 37 mm M1939), vehicle guns and small arms | 50 % on the ground and on aircraft | a 20 mm dirt puff about 37 ft wide instead of 75 |
| The AC-130's 105 mm | 100 %, like a 500 lb bomb (John retuned it from 200 % on 2026-10-12: kinetic plus explosive) | its type 22 ground blast 255 ft wide, as a Mk 82's (the type's rolled width is always capped at 255) |
| 57 mm (ZSU-57), the 85 mm and 100 mm flak guns, the 120 mm and 125 mm tank guns | 100 % | unchanged |
| Bombs | cube root of the bomb's weight over 500 lb (a pack's weight is shared by its bombs) | 250 lb 79 % (about 200 ft wide), 500 lb 100 % (255 ft), 1,000 lb 126 %, 2,000 lb 159 % (405 ft), GBU-28 (4,500 lb) 208 % |
| Missiles and rockets | cube root of the warhead's weight over 500 lb, at least 50 % | AGM-65G (300 lb) 84 %, Harpoon (488 lb) 99 %, SS-N-9 (1,100 lb) 130 %; a Sidewinder or a 2.75 in rocket 50 % |

- The scale applies to a weapon's own impact: its hit on an aircraft or
  object, its ground or water strike and a flak burst. A target's death
  blast, a crash and a launch keep their type's size.
- The retail weapon records carry no warhead weight, so the missiles' and
  rockets' come from a table of the real weapons' published warhead weights,
  rounded (fitted); fictional records (the AAM-L, the AEMP-1) take an agent's
  estimate, and a record the table does not name keeps its type's size.
- The 50 % floor for missiles and rockets is an agent decision (2026-10-12):
  a 21 lb Sidewinder warhead would draw at 35 %, smaller than a cannon shell.
- The scale is a byte (1 to 255 percent); 0 marks a dud bomb's inert dirt
  puff, drawn at full size with no flash and no sparks. It rides the
  simulation's effect, the network's Effect event
  ([protocol 26](../formats/net-protocol.md#protocol-26-explosions-sized-by-weapon))
  and recordings (the [effect scales section](../REPLAYS.md#versions-and-damage)),
  so every machine and every replay draws the same size. Older recordings
  draw every explosion at its type's size.
- Sounds are unchanged.

### Impact light and sparks

Opinionated, agent design within John's request of 2026-10-12 (bug B11:
every impact lights the night and throws sparks; he wants sharp visibility
aids, no glint). Every weapon's hit, ground strike and kill blast, but not a
dud's puff, a flak burst (which has its own flash) or a small round's
splash in the water:

- **Flash of light.** A point light at the burst, lifted 15 % of the
  explosion's drawn width above the ground for a surface blast, at 2.5
  flares per 150 ft of drawn width (a fifth of a flare to twelve flares; a
  20 mm hit about 0.6 of a flare, a 500 lb bomb about 4, a 2,000 lb bomb
  about 7, the 105 mm about 4 like a 500 lb bomb). It holds two ticks and fades over 10 ticks plus
  14 per unit of the weapon's scale (about 0.15 s for a cannon shell, 0.2 s
  for a 500 lb bomb and the 105 mm, 0.27 s for a 2,000 lb bomb), the quadratic fade of a flak
  burst's flash. It rides the flare light path, so like every flare light
  it counts up to four times as much against the night scene and hardly
  shows by day. A picture lights at most its 24 newest impacts, so a long
  gun burst cannot crowd out the flares (the scene lights its 16 strongest
  lights).
- **Sparks.** 6 plus 10 per unit of scale glowing streaks (at most 30): 11
  for a cannon shell, 16 for a 500 lb bomb or the 105 mm, 22 for a 2,000 lb bomb. Each leaves
  at 60 to 160 ft/s times the square root of the scale, upward and outward
  from a ground burst (15 to 75 degrees up) and every way from an air burst,
  falls under gravity, and lives 0.2 to 0.4 s plus 0.1 s per unit of scale,
  cooling as it goes; one that lands goes out. A spark is drawn as a short
  tracer streak (the last 0.05 s of its path, half a tracer's width, 2.2 times its radiance when fresh),
  so it glows additively day and night. Water strikes throw none. At most
  1,500 sparks are drawn, the newest impacts first.
- **Smoke.** A ground burst (not one in the water, and not an air burst)
  leaves dark dust and smoke that grows and lingers with the weapon's size
  (opinionated, John, 2026-10-12: the whole effect, light, fireball and
  smoke, scales by the same rule; the numbers are agent choices): 1 plus 4
  per unit of scale puffs, at most 10 (3 for a cannon shell, 5 for a 500 lb
  bomb or the 105 mm, 7 for a 2,000 lb), each growing from 0.15 to 0.45
  of the explosion's drawn width across and rising 6 ft/s times the square
  root of the scale, fading in over a quarter second behind the fireball and
  gone after 6 seconds times the scale (3 s for a cannon shell, 4.7 s for a
  250 lb bomb, 6 s for a 500 lb or the 105 mm, 9.5 s for a 2,000 lb).
  Up to 96 recent impacts are remembered for it.
- The flash's strength grows with the drawn width, so a bigger weapon's
  light is both brighter and reaches farther before it fades into the
  night; the fireball and the shockwave ring scale with the width too.
- **No fires.** An impact starts no fire (John, 2026-10-12): fires stay with
  crash sites and destroyed units and buildings.
- All of it comes from the effect alone (where, which type, its scale and how far
  into its life), with each spark's direction, speed and life drawn from the
  burst's exact position, so a networked client and a replay show the same
  and nothing reaches the simulation, the wire or a recording.

### Night glow

Opinionated extension, John, 2026-10-15: the whole fire and explosion sheet
emits its unshaded art, using the existing glow's atmospheric visibility at
night. The common `device_night()` signal blends from day at +2 degrees sun
elevation to night at -6 degrees. The CPU computes the common factor from
the visual sun elevation sine and uploads it in the group-0 scene uniform;
`device_night()` reads that value without requiring surface-light resources.
Day retains the previous pixels. At the
midpoint the sheet mixes ordinary haze visibility and glow visibility equally.
Cloud occlusion remains. Cold crater art and dust stay non-emissive; flames
over craters use the fire sheet. The existing additive halo is retained.

Correction requested by John, 2026-10-15 (nr-fx2, opinionated): restore the
pre-nr-nightfx scene firelight. Fire, fireball and impact lights use the
original flare point law, with no finite sphere, minimum radius, extra warm
surface term or long tail. Strength, source height, inverse-square softening
of 25 square feet, 1,500 ft base reach and fourfold night gain are unchanged.
A light with a radius (a fire, a fireball, an impact flash) returns that
orange off the surface brightness and 30 percent of the surface hue, so green
ground stays a warm fire glow (John, 2026-10-16: burning wrecks on grass were
glowing green). Snow and sand take the same mix and stay warm. Water keeps
its fitted dark-sea tint times the orange flare colour, and clouds and smoke
keep their own art times that colour. A point source still multiplies the art
colour. The removed nr-nightfx warm term stays removed. Green tracer ribbons
do not colour fires.
The whole sprite emits its full daylight palette, without the former 1.6
palette multiplier. Near-view night haze does not dim it; distant haze and
cloud occlusion still apply. The close halo returns to 0.8 times art width,
18 percent near level and the original 0.35 bloom with exponent 0.6.
The distant point's 4 to 9 px radius and 1 to 3 brightness floors stay.
These halo numbers are agent choices restoring the earlier settings, pending
GPU review. Daylight keeps its existing shader branch and light selection.

Night slot stability is an agent implementation decision: reserve up to eight
persistent fire lights within 3 nm, ordered by distance then world position,
before considering transient fireball embers. Reserve those fires again in
the final sixteen-light scene budget. Muzzle and impact flashes use remaining
slots, ranked by strength over distance squared, then position. A flash cannot
evict a fire. Fires can still change when extinguished, when the camera moves
past the 3 nm boundary, or when a closer persistent fire competes for the eight
slots. No simulation state, random sequence or multiplayer messages change.
The deterministic `--night-light-slots` probe checks both selection stages and
camera-relative upload across 240 frames with eight fires and up to 64 remote
transient lights. GPU appearance and byte-identical day captures require the
lead's offscreen validation.

Opinionated, John, 2026-10-13 (nr-fx3): at night, explosions and fires
glow at every distance, and a bigger fire glows more. The rules and numbers
are agent decisions tuned on renders (fitted). Code:
`crates/tore-app/src/night_glow.rs` and `night_glow.wgsl`.

- **Glow points.** Every explosion, fireball, cluster bomblet flash and
  fire (crash sites, destroyed units and buildings, burning city fill
  buildings) is also drawn as an additive point of light at the middle of
  its art. A hill or a building in front hides it; a cloud between hides
  it. It shows at night and fades in through dusk; by day there is none.
- **Size and brightness by yield.** A glow's strength is the light it
  gives in flares. An explosion's is its flash ([above](#impact-light-and-sparks),
  by the weapon's [size](#size-by-weapon)) plus the fireball's ember after
  it, 0.3 of the flash's peak fading with the square of the time left of
  its art, so a 2,000 lb bomb's glow is brighter and lasts longer than a
  250 lb bomb's. A fire's is 1 flare for a 30 ft fire, growing with its
  area (width squared), capped at 32 flares: a 24 ft city building fire
  0.64, a 90 ft building fire 9, a 160 ft fuel or ammunition fire about 28.4.
  This stronger near-field lighting is agent-selected tuning for John's
  2026-10-13 request to make night fires emit light comparable to their size.
  Impacts still start no fire (John, 2026-10-12), so for a bomb "its fire"
  is its fireball.
- **Visible at every distance.** Once the art shrinks below about 16
  pixels the point takes over from it: at least 4 pixels in radius
  (`4 * (0.8 + 0.45 * sqrt(flares))`, at most 9), and its brightness
  `0.8 * sqrt(flares)` between 1.0 and 3.0, so it never fades for being
  small. Closer in it uses the moderate halo specified above. Before nr-fx3,
  a night explosion or fire
  shrank below a pixel and the night haze (which here reaches full by about 3.5 nm on a
  clear night) took it out by 5 to 10 miles.
- **Haze.** A light is seen through the night air about ten times as far as
  the ground: the glow takes the scene's haze ramp at a tenth of its
  distance, and only the ramp (the haze already over everything near at
  night does not dim it). On a clear night it fades out by about 35 nm; in
  fog, far nearer.
- **Budget.** The nearest 4,096 glow points are drawn, plus, of a burning
  city, the nearest 2,048 buildings past its 1,024 fire sprites. A glow is
  one small quad: well under 0.1 ms of GPU even at the cap, and the CPU work
  is a pass over what the frame already draws (measured in
  [the baseline](../baselines/surface-defenses.md)).
- **Fire light on the ground.** A burning fire lights the ground and
  objects around it as a flare does, at its glow strength, from 0.35 of its
  width above its base. The reach of every flare light now grows with its
  strength past one flare (the cut-off at 1,500 ft times the square root of
  the strength in flares), so the light ends where it is as dim as a
  flare's at 1,500 ft: a big fire, and a big bomb's flash, light farther.
  A fading fire keeps its full width and source height while its light
  strength follows its opacity linearly, reaching zero when extinguished.
  A detonating explosion also lights surfaces through the remainder of its
  fireball: 0.3 of its peak flash strength times the square of the fraction
  of animation left, positioned at the center of its art. This adds only
  the ember contribution, so the separately tracked flash is not counted
  twice. Duds, debris puffs and shockwave dust add no sustained light.
  Day keeps the nearest eight fire/fireball lights within 3 nm, sharing the
  sixteen strongest scene lights. Night reserves persistent fires as specified
  above. Ground light is culled by distance; the glow point is not.
- All of it comes from the picture's effects, marks and burning city, so a
  networked client and a replay show the same. `TORE_NIGHT_GLOW=0` turns
  the glow points and fire light off (a developer aid for comparing and
  timing; it leaves the longer reach of strong lights).

### Craters by weapon

Opinionated, John, 2026-10-12. Bombs and guns leave craters that follow the
same yield scale as the explosion, on land only (a water strike keeps its
spray and leaves nothing; a hit on an aircraft or object leaves nothing):

| Weapon | Crater | Drawn across |
| --- | --- | --- |
| Bombs with a crater in their record (not cluster bombs) | a 500 lb bomb's (size 9) at the bomb's scale | 250 lb about 230 ft, 500 lb 288 ft, 2,000 lb about 458 ft, GBU-28 about 600 ft |
| The AC-130's 105 mm | a 500 lb bomb's | 288 ft |
| Missiles and rockets | their record's size at their scale | AGM-65G (9) about 242 ft |
| Tank guns (120 mm, 125 mm) | their record's size (6) | 192 ft |
| Every other gun round (aircraft guns, AAA, 57 mm, small arms) | a pock mark: crater size 0, 5 ft across at the 50 % gun scale (10 ft at 100 %) | 5 ft |

- Every crater's drawn half width is at most 333 ft. The retail sizes for
  bombs (9 for both the Mk 82 and the FAB-1000, 18 for the Mk 84) do not
  follow weight, so every bomb now takes the 500 lb bomb's size at its own
  scale (agent decision within John's rule): the Mk 84's drawn crater
  shrinks from 576 to about 458 ft.
- **No cap** (John, 2026-10-12): every weapon crater and gun pock mark
  stays for the whole mission; the old 256-crater cap is gone. Crash fires
  keep their 64 cap.
- **Drawing** (agent decisions): a mark is drawn out to 4,000 times its
  half width (a pock mark to 10,000 ft, where it is half a pixel; a 500 lb
  bomb's crater to about 95 nmi), at most 3,072 marks at once, the nearest,
  so a long mission's marks never crowd out the explosions. Measured on
  2026-10-12 with 20,000 pock marks and 1,000 craters: about 0.4 ms of a
  frame's CPU time (release build), 1.5 MB of sprites, 1 MB of simulation
  state, 0.9 MB of checkpoint (a standby's first full state; later states
  code only what changed), about 0.5 MB of recording, and 13.5 bytes of
  network event each.
- **Online**: a new mark reaches every client as it is made. A player
  seated later is told of every crater and fire but only the newest 1,000
  gun pock marks (about 13.5 KB; agent decision), since 20,000 would hold
  every other event back for a minute in the snapshots' event share; older
  pock marks are missing only on that late joiner's screen.
- A pock mark is a crater of size 0, which an older build or replay viewer
  draws as nothing. The crater code is shared: `blast::mark_half_width`
  sizes any crater by its scale and the effect renderer's `visible` budget
  culls flat marks, for the runway craters slice B16 adds.

Cluster bombs (agent design, 2026-10-10): the five cluster records explode as type 18 on land and water with no crater, and a ground burst of that family also draws 28 small bomblet flashes over 1,200 feet, derived from the burst's position alone; see [ordnance presentation](ordnance-presentation.md#cluster-bomb-flashes).

Crash site: crash explosion type 35 on land, 34 in water; crater size 6
(192 feet across); fire size 100 feet; `&FIRE.5K` loop full within 100 feet and
silent at 2,000 feet; smoke puffs every tenth of a second from 20 feet
above the fire, each rising at 20 knots (about 34 feet per second) in its own
direction within 5 degrees of straight up and carried by the mission wind,
growing from 16 feet across by 6 feet a second, fully dark until 1,300 feet
above the ground and fading out by 1,500 feet (about 44 seconds). Everything lasts 15 minutes; the fire and its sound fade
over the last minute.

## Edge cases

- A weapon strike on water never leaves a crater.
- Every crater and pock mark is kept for the mission (no cap since
  2026-10-12); at most 64 fires of limited life (crash sites, burning
  vehicles and ships, a parked aircraft's hulk) burn at once, the oldest
  going first. A destroyed building's or fixed object's fire lasts the whole
  mission and is outside the cap (nr-fx2, John 2026-10-13; [wreck
  fires](surface-defenses.md#wreck-fires)).
- An aircraft is given one crash site, however its crash is detected.
- A missing sheet or recording skips that picture or sound; the rest play.

## Implementation in TORE

Spec-derived: the table, variety chances, sheets and frame layouts, weapon type
selection, crater sizes and water refusal, recording lists and sound distances.

Fitted, by the agent:
- An explosion's size byte is drawn as its width in feet; the original's
  world scale for it is unknown. A fire is drawn 100 feet wide.
- Variety and size rolls use a separate presentation stream so they never
  change combat randomness; size, recording and crater style come from the
  effect's exact position, so a replay shows the same.
- A destroyed ground object explodes as its unit record's type (`expType`)
  when the world gives one, else as type 35; one destroyed by splash damage
  explodes where it stands, at the bottom of its box.
- A weapon without a reviewed type uses 18, 15 or 30 by its effect kind.
- Explosions and fire face the camera; surface types stand on their point.
  Craters lie flat, drawn slightly toward the eye so they stay above uneven
  ground.
- The fire cap (64).
- A debris piece landing keeps its 15-foot `GRDLRGA` puff and stays silent.

Opinionated:
- Explosion size by weapon (John, 2026-10-12) and the impact light and
  sparks (agent design on John's request of the same day): see
  [size by weapon](#size-by-weapon) and
  [impact light and sparks](#impact-light-and-sparks) for the rules, which
  numbers are fitted and which are agent decisions.
- The climate look, the per-theater humidity table and the shockwave ring's
  overhaul (John, 2026-10-12, slice B12; the humidity values are fitted, the
  shapes, counts, timings, colours and budget agent decisions): see
  [climate](#climate).
- The shockwave ring (agent, X1, 2026-10-10; John asked on 2026-10-10 that
  ground explosions show a shockwave). The original has no shockwave art or
  effect: neither `EXP.SH` nor any picture in the archives is a ring or wave
  (`WAVE01` and `WAVE02` are blank sheets, the unreferenced `AIRMEDA2` is a
  falling-debris strip), and air explosions draw only their sheet. The ring
  is built from the original's own smoke puffs; its counts, sizes, timing and
  opacity are agent choices tuned on the
  [preview renders](../DEVELOPMENT.md#explosion-inspection). It is drawn from
  the explosion alone, so replays and networked clients show it with no new
  data.
- Crash sites for every aircraft and their 15-minute life, requested by John on
  2026-09-28, and the column's ten puffs a second, 20-knot rise, 5-degree
  spread, wind drift and fade between 1,300 and 1,500 feet, which John set the
  same day after seeing the first versions. The crash explosion types, crater size
  6, puff size and darkness (0.5), and the fire's one-minute fade and loop
  level (0.3) are agent choices. All other smoke, contrails, flares, flare smoke and chaff
  drift with the wind too ([smoke](damage-smoke.md)).

## Unknown

- The original's world size for explosion and fire sizes.
- Whether the original leaves anything after an aircraft crash; aircraft files
  give crater size 0.
- How long the original keeps craters when many are made.
- The fire loop's level in the original.

Next research: trace the explosion shape's size use in the shape interpreter,
and the object pool limit in the object creator.

## Source notes

Static evidence from FA 1.02F and the user's FA_2.LIB is in the
[format notes](../formats/explosions.md); what was run is in the
[evidence record](../baselines/explosions.md). Sound travel, the cockpit
filter and the loop mix are in the [audio guide](../audio.md).
