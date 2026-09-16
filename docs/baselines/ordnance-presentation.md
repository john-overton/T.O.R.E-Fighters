# Load Ordnance presentation validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation-mode validation on Linux, 2026-09-16, against the
[presentation specification](../spec/ordnance-presentation.md).

The original ORD_AIR3 background and all 16 small dial orientations were decoded
and visually inspected locally. DIAL00 visibly contains the unwanted white corner;
DIAL13 and DIAL11 point toward the air/surface indicators and fit the white frame.
LIGHTON and LIGHTOFF supply both lamps with blue outer rims, replacing the
white inactive rim baked into the background. A close-up capture confirmed the
inactive surface lamp rim; available weight and page count were lowered two pixels. New required assets successfully triggered the
existing automatic re-import from local user-owned media.

CPU captures of both categories, Quick Mission and the main menu were inspected.
The surface capture initialized the existing category state to one for inspection;
normal startup was restored to zero before final validation. Captures remain
ignored: `.local/ordnance-pass.png`, `.local/ordnance-surface.png`,
`.local/quick-bars.png` and `.local/menu-bars.png`.

Formatting, warnings-denied Clippy, locked workspace tests/build, 40 Python tests,
source and both debug binary asset guards, documentation headers and diff checks
passed. The 394 Rust tests include a synthetic glyph-padding test proving visible
text is centered independently of transparent rows in the font image.

`--validate-creator` passed for all five exact imported identities: 9 supported
store/placement cases for F/A-18D, 9 for Rafale C, 10 for F-14D, 3 for A-4E and
9 for X-31 EFM, including edited fuel, empty stations and accepted-load restart.
These checks cover loadout state, not a manual pointer-to-flight playthrough.
Main-menu and ordnance GPU smoke tests passed on NVIDIA RTX 4070 / Vulkan.

Font choice, text offsets, blue/yellow text colours and dial placement are fitted
presentation. No retail executable comparison, Windows/macOS execution or manual
full loadout editing playthrough was performed. No retail assets were committed.
