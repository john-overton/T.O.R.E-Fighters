# Explosion, crater and fire resources

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research facts behind the [explosion specification](../spec/explosions.md).
Research on 2026-09-28 read FA.EXE 1.02F, SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`, and the
user's FA_2.LIB shapes `EXP.SH`
(`240e61e2bbd3b8ba582db45ea241133b66ef8e322773f1553d2b97b365f89056`),
`CRATER.SH` (`a77b13f96999f20d8f3316a1705b0817aeebe4eac8b7265b5137dd88b0638ab6`)
and `FIRE.SH` (`b4e9d933f5624ab488013985783e54cddaedf4b55cd8cffbc3214a2a28f271b9`).
Only bounded data reads and static disassembly were used; no original code ran.
[Evidence record](../baselines/explosions.md).

## Explosion table

The explosion routine at `0x4432d0` indexes a table at `0x4f46c8` with 48-byte
records, one per explosion type. Only types 15 to 38 hold records; the bytes
before type 15 are unrelated strings. Record layout:

| Offset | Size | Meaning |
| --- | --- | --- |
| +0 | word | Size before the size roll |
| +2 | word | Lifetime in seconds, passed to the object creator (`0x443b70`), which ends the object 256 clock units per second later |
| +4 | word | Object flags; bit 0x4 places the object on the surface |
| +6 | word | Debris piece count, passed to the debris routine `0x4441d0` |
| +8 | word | Debris spread, passed to the same routine |
| +10 | 8 dwords | Recording name pointers; the list ends at the first null |
| +42 | word | Distance in feet where the sound level reaches zero |
| +44 | word | Distance in feet within which the sound is at full level |
| +46 | byte | Sound request argument 14, not interpreted |

The size roll is `size * (66 + rand(66)) / 100`, capped at 255, written to the
object's start and end size bytes. A recording is drawn at random from the
record's list. The sound request (`0x433680`) passes priority 90, level 255, the
record's two distances as arguments 6 and 7, and a random 0 to 29 as argument 9.
Argument meanings are in [the sound notes](sound.md).

Before the table is read, the routine substitutes related types for effects the
local machine creates. The chances are exact from the routine; the draw order
is an implementation detail and is not reproduced:

| Type | Substitution |
| --- | --- |
| 18 | 66%: by a 0 to 99 roll, below 20 becomes 19, below 40 becomes 20, below 60 becomes 29, below 80 becomes 28, else stays 18. Otherwise 10%: becomes 24, 25 or 26 (below 50, below 75, else) |
| 30 | 25%: becomes 24, 25 or 26 as above. Otherwise 75%: below 25 becomes 31, below 50 becomes 32, below 75 becomes 33, else stays 30. Otherwise 10%: 28 or 29, even odds |
| 15 | 50%: becomes 16 |
| 21 | 35%: becomes 23 |
| 35 | Roll 0 to 99: up to 33 becomes 37, up to 66 becomes 36, else stays 35 |

## Weapon and object fields

JT projectile records carry `expType` (hitting an object), `expTypeForLand`,
`expTypeForWater` and `craterSize`. `tore-formats` reads them as
`weapons::Effects`. PT object records carry `expType` and `craterSize` too;
every one of the 13 reviewed FA aircraft gives 30 and 0.

## EXP.SH

`EXP.SH` embeds a switch on the object's type byte (`+0x45` of the shape's
object) whose cases point at 33-byte picture records. Each record is a
15-byte name field followed by nine words: sheet height, frame width, frame
height, first frame x, first frame y, horizontal gap, vertical gap, columns and
frame count. The switch maps:

| Types | Sheet | Types | Sheet |
| --- | --- | --- | --- |
| 15 | `grndsml` | 27 | `flaka` |
| 16 | `dirtexp` | 28 | `flakb` |
| 17 | `watsml` | 29 | `flakc` |
| 18 | `airsml` | 30 | `airlrg` |
| 19 | `airsmla` | 31 | `airlrgag` |
| 20 | `airsmlb2` | 32 | `airlrgc` |
| 21, 22 | `grndmed` | 33 | `airlrgd` |
| 23 | `grndmed3` | 34 | `watlrg` |
| 24 | `airmed` | 35 | `grndlrg` |
| 25 | `airmed2` | 36 | `grndlrg2` |
| 26 | `airmed3` | 37 | `grdlrga` |
| | | 38 | `empex` |

Its drawing code picks the frame from the elapsed share of the object's life
and interpolates the size between the start and end bytes. The code's check
rejects sizes of 790 or more. The meaning of the size in world units is not
established; the implementation's reading is fitted.

## CRATER.SH

`CRATERS.PIC` is 256 by 66 and holds three crater pictures side by side at x
1, 81 and 161, each 78 wide. `CRATER.SH` picks one from the object's type byte
(1, 2 or 3) and writes a flat square's corners at plus and minus
`16 * size` feet, capped at 333, or 200 without an object. The crater routine
(`0x443d00`) refuses a size of 0, refuses points the terrain query reports as
water, draws the type at random from 1 to 3 and creates the object with no end
time. A projectile's crater call passes its `craterSize`.

## FIRE.SH and the fire routine

`FIRE.SH` draws `FIREA.PIC`, 15 frames of 76 by 62 in three columns, choosing
the frame from the low byte of the clock, which loops every second. The fire
routine (`0x444020`) creates type 14 with the caller's start and end sizes,
names `&FIRE.5K` as the object's loop and 2,000 feet as its distance. Its one
reviewed caller (`0x442ea9`) passes sizes 100 and 100 and no end time, then
requests smoke.

## Unknown

- The world size of an explosion's size byte and a fire's size.
- How long the original keeps a crater when many are made; its object pool
  limit was not traced.
- Which object path the ground collision at `0x4693c1` serves; it passes type
  15 on land or 34 on water and crater size 30.
- The loop level of a fire's `&FIRE.5K`.
