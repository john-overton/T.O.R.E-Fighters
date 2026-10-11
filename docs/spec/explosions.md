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
