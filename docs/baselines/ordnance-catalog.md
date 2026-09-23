# Load Ordnance catalog validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-23, on Linux with pinned Rust 1.91.1 and locked
dependencies. Implements the
[catalog availability rule](../spec/ordnance-presentation.md#catalog-availability),
an opinionated request from John. The filter is spec-derived and shares the
flight launch support list. No weapons, aircraft identities or flight adapters
were added or retuned.

The 12 synthetic ordnance UI tests pass. They cover supported compatible
alternatives loading and passing the Fly check, hidden unsupported weapons even
when a station accepts them, Cheat showing supported weapons outside normal
compatibility, fixed-gun placement restrictions, unload/reset when toggling back,
and empty categories with no selectable cards or extra pages. Existing drag,
quantity, sound and rocker tests also pass. Fixtures contain no retail bytes.

Formatting, warnings-denied workspace Clippy, locked workspace tests and build,
75 Python tests, source and both debug binary asset checks pass. The Rust suite
passes 1,397 tests, with three existing GPU tests explicitly ignored. Local logs
are in `.local/ordnance-catalog/`.

An isolated `.local/dev-profile` was refreshed from the user's installed media
because the copied cache predated the current radio import format. CPU ordnance
snapshots for `f18`, `rafale` and `a4e` were visually inspected: supported catalog
cards retain their layout, the A-4E air-to-air category has only its gun, and
unused cards remain empty. Main-menu and ordnance `--smoke-test` runs pass on
NVIDIA RTX 4070 / Vulkan. `--headless-flight 1200 --no-audio` passes on the default
hybrid adapter with 1,200 ticks and no crash. Captures and imported media stay
ignored and local.

`--validate-creator --no-audio` passes guns-only setup, player/wing restart,
standard loads and ordnance dragging for all 14 selectable aircraft. The full
probe then stops at `F18: damage region 3 at 0.1 has no distinct finite geometry`,
the [previously documented unrelated assertion](ordnance-presentation.md).
This is not a full creator-probe pass. The current damage appearance was not
changed to satisfy that assertion.

No retail comparison, Windows/macOS runtime execution or manual flight
playthrough was performed. This change exposes only existing connected weapon
behaviour; its fitted flight/guidance limits remain. The
[roadmap passes](../ROADMAP.md#weapon-catalog-update-passes) plan the remaining
weapons without claiming them implemented.
