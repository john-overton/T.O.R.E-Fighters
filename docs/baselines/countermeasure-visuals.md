# Chaff and flare presentation validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-26, macOS 26.6 on an Apple M3 (Metal), locked
Rust toolchain. The behaviour and its constants are in
[countermeasure presentation](../spec/countermeasures.md#presentation). No retail
bytes are committed. Captures are local to `.local/` in the feature worktree and
were taken with `--countermeasure-preview`, as described in
[development](../DEVELOPMENT.md). These are T.O.R.E presentation checks, not
comparisons with a running retail copy.

## Automated checks

- Simulation (`tore-sim`, `combat::countermeasures`): a flare release is a
  mirrored pair 19 to 30 feet to each side after one second and more than
  250 feet behind a 400-knot aircraft. A flare burns 30 seconds, flickers
  within plus or minus 15 percent, sputters and dims over its last 3 seconds,
  rests 1.5 feet above the ground and keeps burning there. Smoke puffs rise
  within 30 degrees of vertical and none survive 200 feet of flare path.
  Chaff stops within about 80 feet at 675 ft/s and fades over its last
  5 seconds. Budgets retire the oldest devices, and devices leave from the
  tail in any attitude.
- A pair counts as one flare, decoy rolls are unchanged, and devices replay
  identically at 30, 60 and 144 frames per second (`combat::live` tests).
- Renderer (`countermeasure_renderer`): the 16 lights nearest in strength over
  distance squared are chosen, with exact offsets a million feet from the
  origin. Smoke glow falls with inverse square, is capped at one sun and ends
  at 1,500 feet. Instance packing covers every burning flare and cloud.
- GPU (`gpu_burning_flare_lights_the_ground_beneath_it`, run with
  `--ignored`): at midnight a flare 40 feet above a grey plane brightens the
  ground under it warmly and less at the plane's edge. The existing smoke,
  shadow, airport and glare GPU tests still pass with the larger lighting
  uniform and the extra palette row.

## Captures

- **Day, 0.5 seconds after release** (Ukraine, 13:00): two orange-yellow
  flame heads with white trails. Puffs rise, and the flame stays visible at the
  head of each trail.
- **Night, just behind the tail**: two white balls with halos and soft
  streaks. The glare overlaps the releasing aircraft, and the aircraft's tail
  takes flare light.
- **Night over flat desert** (Egypt, 120 feet): a warm pool on the sand under
  the flares, in the terrain's daylight colors. Before the daylight palette
  row was added, the pool was near black and then magenta; both results are
  what led to the row.
- **Chaff, day**: a speckled silver cloud. Two frames 4 ticks apart show
  different strips flashing white.
- **Chaff and flares, night**: chaff strips near the flares glitter warm, and
  the flare smoke glows near its head.
- **Original graphics and no anti-aliasing**: flares, glare and smoke render
  in both, including the single-sample glare pass.

## Not run

- A Quick Mission with many AI salvos was not profiled. The bounds are 128
  flares, 64 chaff clouds (38,400 strips) and 12,288 flare smoke puffs.
- Windows and Linux GPUs were not checked. The shaders only use features the
  existing passes already use.
