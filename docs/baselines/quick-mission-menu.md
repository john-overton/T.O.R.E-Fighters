# Quick Mission menu validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation-mode validation on Linux, 2026-09-16. Behaviour is defined in the
[menu specification](../spec/quick-mission-menu.md).

Formatting, warnings-denied workspace Clippy, locked workspace tests and build
passed. All 393 Rust tests and 40 Python tests passed. Source and both debug
binary asset checks passed, as did documentation header checks.

CPU snapshots of the normal page, aircraft selector and theater selector were
inspected. Both selectors show all 15 wells, including non-interactive empty rows.
The original 20 by 27 `ACTDFLT.PIC` cap was extracted and visually inspected; it
contains the striped marker and missing left rim. The local runtime cache
automatically re-imported successfully with this newly required asset.
Button labels were lowered two pixels. The aircraft capture contains exactly A-4E Skyhawk, F-14D Tomcat,
F/A-18D Hornet, Rafale C and X-31 EFM from the local imported profiles.
Synthetic tests cover translucent glyph edge blending, metadata/alias exclusion, empty-list acceptance, page
navigation and cancellation. Existing tests cover theater identity mapping.
Captures remain ignored under `.local/quick-regular.ppm`,
`.local/quick-aircraft.ppm` and `.local/quick-theaters.ppm`.

Display smoke tests passed for the main menu, Quick Mission page and aircraft
selector on NVIDIA GeForce RTX 4070 / Vulkan. No manual pointer-to-flight run,
Windows/macOS execution or retail executable comparison was performed. Open-font sizing, field spacing, panel texture placement, diagonal markers and bevel geometry remain
fitted presentation, not a claim of exact original rendering.
