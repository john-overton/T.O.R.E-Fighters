# Cockpit visibility and HUD zoom

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, requested by John on 2026-09-17. These are opinionated
presentation requirements. The local FA manual, printed p. 104, establishes
magnification controls but not the precise visibility or anchor rules below.
Retail execution comparison is unavailable.

At magnification below 1x, hide cockpit artwork and its mirrors. Retain the HUD
and independently selected instrument windows. At 1x and above, show the cockpit
again if its normal visibility toggle is on. Zoom must not override that toggle.

In centered forward view, the HUD's central datum stays at exactly half the
viewport width and height at every magnification. Scale HUD artwork about that
point; never lower the HUD when zooming out. Existing head-look still projects
the aircraft-forward datum opposite camera movement and applies directional
fading. Zoom does not change seeker search angles or flight/sensor state.
[Controls](../FLIGHT-CONTROLS.md) and [validation](../baselines/cockpit-slide.md).
