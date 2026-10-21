# Shadow distance validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-21. This validates the opinionated
[shadow distance setting](../spec/graphics-options.md#shadows), not retail
parity. The before build is `099bab0317987fc48c6f2cf293df69eedfa928fc`;
the after renderer is the implementation in this change on `shadow-distance`.

GPU access was confirmed before coding: NVIDIA GeForce RTX 4070, driver
615.78.08, 12,282 MiB VRAM. The ignored sun-disc visibility test passed and
an offscreen capture selected that adapter and wrote an image. The existing
GPU terrain/object shadow regression also passed after implementation.

All captures used the task's isolated imported profile. No retail art is
committed. Logs, binaries, scripts and images are retained under
`/home/john/Development/T.O.R.E-Fighters/.local/next-round/shadow-distance/`.
`gpu-check.md` records the initial experiment and `report.md` records the
final commit and checks. Every GPU/window run used `tools/agent-run.sh`.

**GPU cost.** Whole-frame GPU milliseconds at 3840x2160, default 4x MSAA,
High vegetation density and its default 4 nm distance, static ground objects
loaded, clear weather, 08:00. Each entry is the median of seven eight-frame
timestamp-query means from `--vegetation-preview` with
`TORE_VEG_GPU_TIMING=1`, `TORE_VEG_PREVIEW_STATIC=1`, and
`TORE_VEG_PREVIEW_SIZE=3840x2160`. Quality and distance were selected by
`TORE_SHADOWS` and `TORE_SHADOW_DISTANCE`. The 4 nm Med column uses Trees,
so it also exercises the default. The final table is the second pass, after
compilation and other GPU checks finished; logs are `warm-*.log` and the
reproduction script is `measure-warm.py`. The first new-default timing pass
overlapped compilation and is retained separately, not mixed into this table.

| Scene | Med 1 nm | Med Trees (4 nm) | Med 8 nm | High 1 nm | High 4 nm | High 8 nm |
| --- | --- | --- | --- | --- | --- | --- |
| Ukraine airfield, 500 ft | 10.16 | 10.03 | 10.10 | 10.77 | 11.82 | 11.95 |
| Ukraine forest, 300 ft | 6.36 | 6.43 | 6.49 | 8.40 | 8.67 | 8.74 |
| Hanoi, 2,000 ft | 11.60 | 12.60 | 13.24 | 11.24 | 12.80 | 13.38 |

At High, 1 to 8 nm added 1.18 ms at the airfield, 0.34 ms over forest,
and 2.15 ms over Hanoi. These are short still-camera samples, not a frame-rate
guarantee. Differences of a few tenths of a millisecond are noisy, and the
number of shaded pixels and map filtering can change with reach. Trees
beyond the vegetation draw distance are not loaded just to cast shadows.
Low and Ultra were not benchmarked in this pass.

The reusable camera records are in `cameras-morning.txt` and
`cameras-dusk.txt`. Positions are world feet; yaw and pitch are radians:

| Scene | Position x, y, z | Yaw | Pitch |
| --- | --- | --- | --- |
| Ukraine airfield | 949400, 500, 809400 | -0.81 | -0.30 |
| Ukraine forest | 852961.5, 300, 820206.375 | 0.61086524 | -0.14 |
| Hanoi | 734400, 2000, 1154432 | 0.785 | -0.44 |

**Pictures.** `before-default/` and `after-default/` contain the three
matching 4K morning PNGs. The airfield and forest retain the previous
near/middle-map look. ImageMagick normalized mean absolute channel differences
are 0.000000512 and 0.000004623 respectively. Hanoi differs by 0.020273:
buildings now cast out to 4 nm instead of the old 0.5 nm. Therefore the new
default is not pixel-identical everywhere. The outer terrain map also ends
at tree distance instead of 131,072 ft, as required by the new control.

`dusk-short/` and `dusk-long/` contain the same cameras at 18:30, High
quality, 1 and 8 nm. The longer choice extends shadows across the city and
woods without scaling up the 256 ft near map. Finite map resolution and
existing view-based building/tree-page culling remain limitations.

**Menu and live application.** Five Graphics snapshots and six windowed
captures covering every distance flag passed the menu battery. The
`graphics-shadows` snapshot covers the six-button row next to Shadows.
`live-settings-verified.script` opened Graphics over a paused free flight:
1 nm stayed selected while vegetation changed to 6 nm, then Trees was
selected and vegetation changed to 8 nm. The four `live2-*.png` snapshots
show IN USE AND SAVED. The final file contains `vegetation-distance 8` and
omits `shadow-distance`, as required for Trees. The earlier `live-*.png`
files are a navigation probe that reached Controls, not validation evidence.

**Checks.** Workspace format, Clippy with warnings denied, tests and build
passed. There were 6,947 passing Rust tests and 103 ignored; the app's
2,235 ordinary tests passed. Python tools tests passed (486, four skipped).
Source and both executable asset checks, documentation checks, and the
wrapped display smoke test passed. The sun-disc and geometry-shadow GPU
tests were run explicitly and passed. Linux/RTX 4070 was exercised;
Windows, macOS and other adapters were not run. No simulation, network wire,
replay format, flight adapter or AI behavior changed.
