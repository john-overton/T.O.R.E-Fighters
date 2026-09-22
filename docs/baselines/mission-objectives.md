# Quick Mission objectives and Target view validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. Source base: `282ba41`. This pass covers sentence-style
objective selection, independent group survival requirements and the
[player-relative Target view labels](../spec/target-window.md).

## Behavior checked

A synthetic mission with two populated enemy groups assigns group 1 to the
player and its wingmen. Every group 1 member is a destroy objective and every
group 2 member is excluded. Switching to free fire removes designated destroy
objectives and admits observed eligible hostiles from either group. Existing
self-defense interruptions remain unchanged.

Survival requirements resolve through actual group membership, independently
of combat orders. Friendly escorted or required aircraft produce Survive;
assigned enemies produce Destroy. An enemy-side survival flag cannot produce
a player-side Survive label. Unknown and unrelated contacts have no label.

## Visual checks

The default creator, a two-enemy-group objective example and a ground-start
layout were rendered with runtime assets and inspected. All six objective
sentences and survival fields fit, including the airport row above the buttons.
Fields use the existing briefing font, bevel and colors. Local captures are in
`.local/objective-controls-review/` and are not committed.

The original WIN11 instrument font was decoded through the existing bounded
reader and used to render both objective labels. `Obj: Survive` occupies 61
pixels and `Obj: Destroy` 64 pixels, within the 134-pixel text area; all glyphs
exist. The font's original glyph shapes/capitalization remain in use. Target
framing returns to its ordinary 52% vertical allowance with a single objective
row at y=111.

## Repository validation

Validated on Linux with the pinned Rust toolchain and locked dependencies.

| Check | Result |
| --- | --- |
| Formatting and workspace Clippy with warnings denied | Passed |
| `cargo test --workspace --locked` | 1,119 passed; 3 existing ignored tests |
| Workspace build | Passed |
| Python tool tests | 68 passed |
| Source and both executable asset scans | Passed |
| Documentation headers and diff whitespace | Passed |
| Main-menu display smoke | Passed |
| Direct Quick Mission intercept launch with Target window enabled | Passed; restart check passed |

Visual snapshots exercised the default menu, explicit opposing-group targets
and free fire, required survival, and the longest ground-start layout. Capture
state `objectives` configures the explicit example without changing normal
startup defaults.

## Limits

Requirements and assignments persist within the session and mission restart.
Campaign/save persistence and mission result/scoring evaluation remain outside
this UI/data slice. A survival flag does not rewrite an aircraft's flight orders.
No retail comparison or complete human-flown encounter review was performed.
