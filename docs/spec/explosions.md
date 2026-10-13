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

Every large ground or sea explosion also throws out a shockwave: a ring of
dust (white spray on water) that races out from the blast, slows and fades
while the fireball burns.

A weapon that strikes land with a crater size leaves one of three crater
pictures lying flat on the ground. It stays for the rest of the mission.

As John requested on 2026-09-28, every aircraft that crashes on land, the
player's or any AI's, leaves a crash site for 15 minutes: a large ground blast,
a crater, a fire and a column of dark smoke rising from the spot. The fire
crackles for anyone within 2,000 feet and fades out over its final minute. A
crash into the sea shows a large water blast and leaves nothing behind. An
aircraft that explodes in the air leaves no site.

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
feet. Guns have no crater; air-to-air missiles 3 (96 feet across); rockets 1
or 2; anti-ship missiles 6; air-to-ground missiles and 500 and 1,000 lb class
bombs 9; SAMs 12; guided 2,000 lb bombs 15; the Mk 84 18 (576 feet across).

Shockwave (agent design, 2026-10-10): explosion types 21 to 23 and 35 to 37
throw out a ring of 32 grey `SMOKE.PIC` dust puffs, type 34 a ring of white
spray puffs; no other type has one. The ring grows from the blast to 1.6
times the explosion's drawn width (about 410 feet across for the big types,
whose drawn width is 255 feet) in 0.7 seconds, easing out as a blast wave
slows, then drifts out another 0.15 widths while it fades to nothing over the
rest of the explosion's two seconds. Each puff stands on the ground, growing
from 0.12 to 0.5 of the width across, and is lit by the scene like the
craters, not self-lit like the fireball.

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
  night; the fireball and the shockwave ring scale with the width too. A
  weapon's impact leaves no fire of its own today (fires belong to crash
  sites and destroyed units, sized by the unit), so there is no impact fire
  to scale.
- All of it comes from the effect alone (where, which type, its scale and how far
  into its life), with each spark's direction, speed and life drawn from the
  burst's exact position, so a networked client and a replay show the same
  and nothing reaches the simulation, the wire or a recording.

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
- At most 256 craters and 64 crash fires are kept; the oldest goes first.
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
- Crater and fire caps (256, 64).
- A debris piece landing keeps its 15-foot `GRDLRGA` puff and stays silent.

Opinionated:
- Explosion size by weapon (John, 2026-10-12) and the impact light and
  sparks (agent design on John's request of the same day): see
  [size by weapon](#size-by-weapon) and
  [impact light and sparks](#impact-light-and-sparks) for the rules, which
  numbers are fitted and which are agent decisions.
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
