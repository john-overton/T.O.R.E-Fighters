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

The next user-scheduled slice (2026-09-16) adds ocean motion. The user refinement removes whitecaps,
shortens ripples and adds near pixelation with distance/altitude smoothing while
retaining retail textures and colors. The follow-up reduces reflection strength
by 25%. The current user-proposed trial keeps ripple sizes fixed and fades
the whole effect from 2,700 feet to five statute miles, replacing the preceding
horizon-angle and larger-wavelength trial. The user approved this appearance;
reflection strength now also follows the distance fade to soften distant grain.
John tested the final version in game and approved keeping it on 2026-09-16. [Component provenance and remaining source gaps](spec/ocean.md),
[acceptance evidence](baselines/ocean.md). This is visual presentation only;
wind/sea-state coupling and water-contact physics are not changed.
