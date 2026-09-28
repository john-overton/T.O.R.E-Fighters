# Explosions, craters and view-dependent sound, 2026-09-28

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research then implementation. Worktree `T.O.R.E-Fighters-sound-design`, branch
`sound-design`, base `a9dc5a6`. [Behaviour](../spec/explosions.md),
[format notes](../formats/explosions.md) and the
[audio guide](../audio.md#cockpit-and-external-views) are separate.

## Research inputs

Static only; no original code was executed. FA.EXE 1.02F
(`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`) was read
with a bounded PE section map and GNU objdump over `0x442c00..0x4447c0`,
`0x4693c1..0x469430` and the callers of `0x443d00` and `0x444020`. The saved
research disassembly under `.local/weapons-research/native/` had misaligned
past the jump table at `0x443b28`, so the object creator at `0x443b70` was
disassembled afresh. `EXP.SH`, `CRATER.SH` and `FIRE.SH` from the user's
FA_2.LIB were read as bytes; their switch and records were decoded by hand
and cross-checked against the sheet sizes of the pictures they name.

All 135 JT files in `.local/combat-implementation/catalog/FA_2.LIB/` parsed
with the existing reader; their explosion types and crater sizes are
summarized in the [spec](../spec/explosions.md#numbers). 13 FA aircraft PT files
parsed: every one gives explosion type 30, crater size 0, an engine loop
reaching 15,000 feet and Doppler on.

## Import

The first start after the change found the dev profile's cache stale and
re-imported it from the remembered source: 4,082 resources, up from 4,066, with
all 16 new recordings. The 23 explosion sheets, `CRATERS.PIC` and `FIREA.PIC`
were already in the cache. The pack limit is 4,096 resources.

## Visual check

`TORE_EFFECT_PREVIEW=1` captures from free flight
([how](../DEVELOPMENT.md#explosion-inspection)) on Linux, Vulkan: the row of
24 types in the air and on the ground, three weapon craters, and a crash site
with fire, crater and column, from a chase camera looking level and 40 degrees
down. Air types face the camera; a first build stood the surface types and the
fire upright about the vertical, which looked edge-on from above, so they now
face the camera with their base on the ground. Not yet checked: a full mission
with real hits and crashes, and night lighting.

John played the first build the same day: explosions and sound "looking and
sounding good", but the first column (40-second puffs growing to 350 feet,
rising 12 feet per second) drew as a solid dark cone. At his direction it now
releases ten puffs a second rising at 20 knots, each within 5 degrees of
vertical and carried by the wind, fading out between 200 and 300 feet above
the ground. After seeing that, John raised the column to fade between 1,300
and 1,500 feet and had all smoke, contrails and flare smoke drift with the
wind.

## Automated checks

- `tore-sim`: explosion table, variety families and shares, repeatable size,
  recording and crater style, crater half widths, fire fade, every recording
  an imported combat resource; crash sites once per aircraft, none in water,
  a 50-puff column after 5 seconds, gone after 15 minutes; the column's
  20-knot rise, 5-degree cone, wind drift and 200 to 300 foot fade; craters kept for
  the mission and capped at 256; straight-line explosion fade and the 20,000
  foot refusal; cockpit gain and cutoff; loop mix; recorded sound names;
  Doppler ratios.
- `tore-replay`: every effect code round-trips; explosion, crater, fire and
  crash-smoke kinds survive a file; the column rises 12 feet per second.
- `tore-app`: each explosion type draws the sheet `EXP.SH` names; every
  frame of every sheet lies inside the texture; frames follow each type's
  life; craters flat and first, fires loop and fade; explosion types play
  their own recordings, at 40 percent from the cockpit; the engine is louder
  and panned outside and Doppler shifted past a still camera; fire loops fade
  out when gone; recorded explosion types and marks rebuild in a replay within
  the caps.

`cargo fmt`, workspace Clippy with `-D warnings`, the workspace tests, the
workspace build, the Python tool tests, the asset guards and the docs check
passed on Linux. `--smoke-test` presented a frame with the new shader.

## Not checked

Nothing was heard: the cockpit and outside mix, the engine outside, fires and
Doppler have tests but no listening. Windows and macOS were not run. Retail
comparison is unavailable.
