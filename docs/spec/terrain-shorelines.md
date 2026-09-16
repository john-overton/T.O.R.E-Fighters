# Terrain shorelines

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

The beach and land in the original theater artwork remain visible. Pixels
identified as water in that artwork show the same ocean/horizon as neighboring
open-water cells, without rectangular strips of the underlying land color.
A texture placement can contain beach or land even when its base cell is water.
Untextured land retains its T2 palette color.

## Provenance and scope

The original index-255 cutout marker and asset layout are **native data**,
recorded in [theater recovery](../formats/theater.md). The GPU presentation is
**fitted**: resolve the four bilinear samples through the live palette, weight
opaque samples, and discard coverage below 0.5 without writing depth. Remaining
samples use the normalized artwork color. Untextured color-255 cells emit no
opaque terrain mesh. These holes reveal the existing ocean/horizon renderer.
The coverage threshold is an agent implementation choice, not a user-requested
visual departure or a claim about the retail rasterizer.

This corrects visible rendering only. T2 elevations, collision/height queries,
texture rotations, weather selection and aircraft materials retain their
existing behavior. Terrain subdivision, fallback land materials and exact
retail shoreline rasterization remain outside this correction. Retail comparison
is unavailable; [acceptance](../baselines/ukraine-viewer.md#shoreline-correction-2026-09-16)
checks the reported artifact and existing application behavior.
