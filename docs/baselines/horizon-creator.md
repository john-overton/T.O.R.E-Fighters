# Horizon reference and creator reverse cycling

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-21. Requested corrections to the
[HUD layout](../spec/hud-layout.md) and
[Quick Mission interaction](../spec/quick-mission-menu.md).

## Behavior and regression evidence

The zero-degree bar now uses the unchanged world-direction projection shared
with the velocity marker. The nonzero numbered rungs retain calibrated compact
motion, finite width and +/-90-degree coverage. There is only one zero bar.
Synthetic tests check level-velocity alignment across seven pitch attitudes,
seven banks and three zooms, unchanged numbered-rung projection, and rendered
pixels at the true horizon with no residual compact zero bar. Full-loop
visibility and pitch calibration regressions remain green. Spacing immediately
beside the true horizon can differ from the compact numbered scale by design.

The creator routes right-button events through QuickMission, which delegates
to ordnance when visible. Inline fields decrement on matching press/release;
values wrap, the player's wing skips zero, dependencies still apply, and empty
lists do nothing. Synthetic tests cover numeric and weather values, wraparound,
left-click compatibility, defense reset, empty lists, mismatched release, focus
cancellation, modal/help isolation, and refusing OK/Cancel/Exit on right-click.
No mission execution or autonomous behavior was changed.

## Validation

All required checks passed on Linux with NVIDIA RTX 4070 / Vulkan:

- Formatting, warnings-denied workspace/all-target Clippy, locked workspace build.
- Locked workspace tests: 972 passed; two existing GPU unit tests ignored.
- Python tools: 68 passed.
- Source and both executable asset guards, documentation headers, diff whitespace.
- Explicit application smoke test and a GPU flight capture after 2,400 level-probe ticks.

Logs and generated images remain ignored in `.local/horizon-creator-review/`.
The inspected GPU capture is `level.png`. The isolated `horizon-review.png`
harness uses the current projection and rung-drawing source with the user's
runtime-decoded HUD11 font. Its orange crosses mark a level-velocity direction
for inspection only; they are not product symbols. Frames cover level, nose-up
4 and 8 degrees, banked nose-up flight, 70, 85, +90 and -90 degrees. No font,
retail resource or generated derivative is committed.

Exact alignment follows the synthetic projection/raster tests, rather than an
inferred attitude from a screenshot. Windows, macOS, interactive pilot acceptance
and live mouse clicking were not exercised in this pass. Right-button event
routing was reviewed and creator input transitions were unit-tested.
