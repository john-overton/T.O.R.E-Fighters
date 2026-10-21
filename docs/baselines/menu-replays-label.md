# Replays top-bar alignment validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-21. Behaviour is specified in
[main-menu presentation](../spec/main-menu-presentation.md#replays-entry).
The comparison is against the imported font in the rebuild. The original
executable was not run, and this does not establish retail gameplay parity.

## Evidence and method

The isolated task profile records retail build **1.02F**, imported by T.O.R.E
0.1.3 at `4439b172bff81f16e392049633075321991123f9`. Its decoded
`MENUFONT.PIC` SHA-256 is
`20f0106aa5688a0d98d6e15867d4886eda06f3515691132833cae5690468ed70`.
The before build is the `menu-replays-label` branch base, `d44460b2`.

`Menu::render` draws all four labels with that font, including ?, Pref and
Multi. Those letters are not baked into the background image. Individual
visible-bounds centering lifted Replays by one canvas pixel because its p
and y extend two rows below the other letters. The fix uses Multi's bounds
as the common vertical reference.

The test script captured the 640 by 480 CPU canvas and real GPU windows
through `tools/agent-run.sh` on Linux with an NVIDIA RTX 4070 Vulkan
renderer, with normal, Multi hover, Replays hover,
Replays pressed, Replays keyboard focus and the Multi dropdown open.
`grim -T` captured only the hidden test window. The desktop scale is 1.6:
requesting a 2400 by 1350 logical window produced the measured 3840 by 2160
image. Its menu is 2880 by 2160, at 4.5 times base scale, with 480-pixel
side bars. The requested 640 by 480 window produced 1024 by 768 physical
pixels; the base-resolution evidence uses its unscaled CPU snapshots.

All captures, measurement scripts and logs stay local under
`.local/next-round/bugfix3/menu-replays-label/` in the primary checkout.
`comparison-640-{normal,hover}.png` and
`comparison-4k-{normal,hover}.png` show enlarged before/after bar crops side
by side. Additional comparisons cover Multi hover, pressed, focus and the
open Multi dropdown. `pixel-measurements.txt` records every state.

## Measured pixels

Coordinates are zero-based and inclusive. Cap height is measured from the
M and R dark cores, x-height from u and a. The baseline is the last dark row
of the shared l glyph, excluding the p/y descenders and the light shading.

| Canvas / state | Multi cap rows | Replays cap rows | Multi / Replays x-height | Multi / Replays baseline |
| --- | --- | --- | --- | --- |
| 640 by 480, before | 42..50 (9 px) | 41..49 (9 px) | 7 / 7 px | 50 / 49 |
| 640 by 480, after | 42..50 (9 px) | 42..50 (9 px) | 7 / 7 px | 50 / 50 |
| 3840 by 2160, before | 189..229 (41 px) | 185..224 (40 px) | 32 / 31 px | 229 / 224 |
| 3840 by 2160, after | 189..229 (41 px) | 189..229 (41 px) | 32 / 32 px | 229 / 229 |

The noninteger 4.5 scale explains the former one-pixel height difference in
the window capture despite equal cap heights on the base canvas. After the
fix, the shared l has identical dark-pixel masks, including stroke width.
The imported dark ink remains RGB (24,40,154). Font edge shading, palette,
letter advances and horizontal placement are unchanged. The eight empty
base columns between the visible Multi and Replays glyph bounds remain.

There is no hover underline in the decoded font or in the live menu.
Multi and Replays both use the existing RGB (190,200,215) rectangle at
base rows 38..56. The yellow lines in the supplied marked image are not
present in captured game frames. No underline was added. Hover, pressed
and keyboard-focus captures are identical for Replays, before and after.

## Coverage and limits

The five main-menu backgrounds (CHOOSEV, CHOOSEU, CHOOSEM, CHOOSEAC and
CHOOSE3) and all three bar origins were checked. Normal and open-Multi
before/after comparisons change pixels only within the Replays hit area;
`compare.log` records their bounds. There is no separate Replays label on
the Direct Connection, Internet Lobby or multiplayer lobby bars.

The battery's `menus-replays-bar-*` scenarios compare the shared l glyph
on every background in six states. The check rejects the real before
capture and synthetic shifted or missing labels. The existing
`menus-snap-{normal,hover,pressed,help,pref,multi,notice}` scenarios cover
the rest of the main-menu snapshots. No full-screen golden expectations
were replaced, and no retail-derived images are committed.

All 37 selected menu battery scenarios passed, including the 30 new bar
checks. The 17 Python menu tests passed, and the real-window smoke test
presented the requested screen successfully. `cargo test --locked -p tore-app`
passed 2,235 tests with 45 ignored; workspace Clippy with warnings denied,
formatting and documentation checks passed. Full command logs are linked
from the task's local `report.md`. Windows and macOS execution and comparison with
the running retail game were not performed. No gameplay, bindings, replay files or multiplayer behavior
changed.
