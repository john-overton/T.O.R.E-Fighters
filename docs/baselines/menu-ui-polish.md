# Menu UI polish validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-23, against the changes based on `0c72833`.
Covers the [Quick Mission controls](../spec/quick-mission-menu.md),
[debrief centering](../spec/debrief.md#presentation), and
[main-menu presentation](../spec/main-menu-presentation.md) requested by John.

## Observed behavior

- The existing creator input checks cover left/right clicks on the three
  unavailable fields and the surrounding sentence, keyboard/Shift activation,
  dismissal and blocked input through the popup. Draft values stay unchanged.
- The separation checks cover all ten choices, forward/reverse cycling,
  default/fallback behavior, and conversion using 6,076.12 feet per nautical mile.
  The rendered list includes 100 and 150 between 50 and 200.
- CPU snapshots were inspected for the unavailable popup, separation list,
  successful and failed first debrief pages, and the main menu. Both debrief
  sentences share the heading's centering axis and fit inside the paper.
- The main-menu badge has transparent corners and the version beside it.
  Enlarged button crops were compared with John's marked centerlines and retail
  screenshot; the two-pixel label adjustment aligns with the raised faces.
  The screenshot's executable build is unknown, so this is visual layout
  validation, not an original-behavior finding.
- Main-menu and Quick Mission `--smoke-test` runs passed on Linux/Wayland with
  NVIDIA RTX 4070 / Vulkan; the creator smoke accepted `--separation 150`.
- App tests passed: 463 passed, five existing ignored. The earlier focused pass
  passed all 33 Quick Mission tests. The committed badge is a 16 KiB derivative
  of the project logo; the generator's total icon budget check passed.

Commands, logs and render derivatives are under ignored
`.local/quick-mission-controls/`. No retail image or font bytes were added to
the repository. Windows/macOS desktop interaction was not run in this pass.
