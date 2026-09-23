# Load Ordnance presentation validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation-mode validation on Linux, 2026-09-22 and 2026-09-23, against the
[presentation and drag specification](../spec/ordnance-presentation.md) and
[mission-wide guns-only rule](../spec/quick-mission-menu.md#mission-wings).

The ordnance-only message strip was checked with the `ordnance-message` and
`ordnance-message-long` CPU snapshot fixtures. The Cheat notice has a compact
background sized to its text. The deliberately long fixture stays on one line,
ends in an ellipsis and stays inside the 580-pixel limit. Both captures were
visually inspected; other screens retain their existing notice rendering.
Images and check logs remain ignored in `.local/ordnance-messages/` in the
ordnance-catalog worktree. Formatting, warnings-denied workspace Clippy, all
1,397 Rust tests, the locked workspace build, 75 Python tests, source and both
binary asset guards, documentation checks and the ordnance GPU smoke passed
for this change. Three existing GPU tests remain explicitly ignored. No new
automated tests were added for this layout-only edit.

The screen retains the original ORD_AIR3 background, thumbnails, category dial,
lamps and button pieces. CPU captures of the loaded, empty and dragging states were
visually inspected. Empty stations retain red thumbnail outlines, with location
headings above them and no weapon image, name or quantity. The carried thumbnail
preserves its transparency and has no surrounding card or text. Local captures
remain ignored in `.local/ordnance-refinements/loaded.png`, `empty.png` and
`.local/ordnance-fixes/drag.png`. Pixel checks against the imported black wells
confirmed equal two-pixel horizontal margins around all eight catalog boxes and
five F/A-18D station boxes, in both loaded and empty captures. Thumbnail pixels
are centered within their outlines in both directions.

Six synthetic UI tests cover thumbnail-only rendering, catalog loading, station
transfers, unloading over occupied and unused catalog space, rejected drops,
same-station drops, unrelated controls, Escape/focus/canvas cancellation, empty
internal and external cards, click loading and right-click decrement. Loaded
station clicks add exactly one, preserve the installed weapon despite a prior
catalog selection, and stop at capacity. The same one-round rule is checked on
a gun station whose keyboard/right-click quantity step is 100. Simulation
tests cover the quantity boundaries 100/101 and 300/301, source and destination
limits, replacement and compatibility rejection. A guns-only test checks that
internal-bay missiles are cleared while the aircraft's own gun retains its
accepted ammunition.

The loadout pass in `--validate-creator` passed for all 14 selectable aircraft,
including exact F/A-18D and Rafale C identities. For each aircraft it checked
29 members across all six wings, rebuilding guns-only inventories on restart,
restoring standard inventories, player accepted-ammunition restart, catalog
unload/reload and compatible station transfers. This pass completes before the
probe's unrelated flight appearance checks. Log: `.local/ordnance-fixes/creator.log`.

The full creator probe does **not** pass: it stops at the unchanged assertion
`F18: damage region 3 at 0.1 has no distinct finite geometry`. That assertion
expects visible partial damage, conflicting with the current
[requested intact appearance below destruction](../spec/damage-smoke.md).
This pass does not change that damage behavior or its old assertion.

Formatting, warnings-denied Clippy, locked workspace tests/build, 70 Python tests,
source and both debug binary asset guards, documentation headers and diff checks
passed. Rust results: 1,148 passed and three explicitly ignored GPU tests.
Main-menu and ordnance GPU smoke tests passed on NVIDIA RTX 4070 / Vulkan.
Current command logs remain ignored in `.local/ordnance-refinements/`.
The earlier all-aircraft loadout probe remains in `.local/ordnance-fixes/`; the
centering and single-click follow-up did not rerun that broader probe.

Drag threshold and thumbnail positioning are fitted input details; the requested
interaction scope and empty-card treatment are opinionated requirements. No
retail executable comparison, Windows/macOS execution or manual full
pointer-to-flight playthrough was performed. No retail assets were committed.
