# Current progress

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Current feature status and execution order live in [the parity plan](parity-plan.md).
Earlier research substeps remain in the [archived progress log](research/progress.md).

The user-scheduled shoreline rendering correction (2026-09-16) is implemented:
water cutouts no longer fill with land colors, and open-water cells reveal the
existing ocean pass. [Behavior and remaining parity scope](spec/terrain-shorelines.md).
[Acceptance evidence](baselines/ukraine-viewer.md#shoreline-correction-2026-09-16).
Remaining menu screens are deferred until explicitly scheduled.
