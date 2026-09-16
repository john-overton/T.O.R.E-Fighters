# Graphical HUD bank scale — 2026-09-15

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


The user clarified the bank display with an F-16 HUD reference image, requesting
an arc and selective numeric labels. This supersedes the earlier `BANK ±N DEG`
line. The authored scale uses 10-degree ticks, longer ticks and numeric labels
every 30 degrees, and a fixed triangular index. Marks rotate within a ±60-degree
arc around that index, wrapping through the full aircraft bank range. At level,
0 is centered with 30 and 60 marked to either side. Higher bank brings 90/120/
150/180 into view rather than clamping to a misleading maximum.

The scale uses aircraft attitude, not camera roll or head-look. It shares the
original HUD font/color and cockpit projection. The HUD clip extends downward
to accommodate the arc and labels, preserving existing speed/altitude outlines,
AGL/V/S values and instruments. Existing crash/engine-off alerts take priority
instead of the scale. This is a user-requested design adaptation, not recovered
Fighters Anthology or verified F-16 avionics behavior.

Evidence is ignored under `.local/bank-scale/`. Synthetic angular tests cover
left/right alignment and wrap through inverted and complete-roll attitudes.

Validation: 287 Rust tests, 24 Python tests, formatting, warnings-denied Clippy,
locked build and source/app/extractor asset guards passed. Linux creator/viewer
smokes, F18/Rafale cockpit captures at 1280×720 and 720×960, and left/right-bank
F18 probes passed. Visual inspection confirmed arc/label clearance and mirrored
left/right indication. Windows/macOS runtime checks remain unavailable here.
