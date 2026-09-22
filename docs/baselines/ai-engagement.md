# AI mission engagement and group objectives validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. Source base: `064da4c`. This pass connects the
[mission rules](../spec/ai-awareness.md#mission-roles-and-rules-of-engagement)
and [six group objective stamps](../spec/ai-awareness.md#quick-mission-objective-stamps).
All committed fixtures are synthetic.

## Validation

Validated on Linux with Rust 1.91.1 and locked dependencies.

| Check | Result |
| --- | --- |
| Formatting and workspace Clippy with warnings denied | Passed |
| `cargo test --workspace --locked` | 1,117 passed; 3 existing ignored tests |
| `cargo build --workspace --locked` | Passed |
| Python tool tests | 68 passed |
| Source and both executable asset scans | Passed |
| Documentation headers and diff whitespace | Passed |
| Imported roster probe, 3,600 ticks per case | All 52 aircraft/skill cases passed |
| Display smoke tests | Main menu and direct Quick Mission escort launch passed on NVIDIA GeForce RTX 4070 using Vulkan; restart check passed |

Twelve mission integration scenarios exercise protected-threat priority,
same-side assigned reporting, unknown bearing cues without hostile memory or
firing, exact report expiry without forwarding refresh, hidden attacker
rejection, escort leash and formation, accepted explicit orders, weapons hold
with missile defense, CAP boundaries, distant assigned targets and role-gated
memory investigation. Additional policy, creator, group-resolution and
Target-window tests verify both sides and separate objective/activity fields.

All six inherited presets completed `--ai-probe-ticks 1200 --ai-mission PRESET
--no-audio`, with four AI actors in the imported Quick Mission fixture. The
player did not attack during this probe. These results verify delivery and
bounded operation, not relative combat effectiveness:

| Preset | Shots | Dropped launches | Current warning records |
| --- | ---: | ---: | ---: |
| free | 4 | 0 | 4 |
| cap | 4 | 0 | 4 |
| intercept | 3 | 0 | 1 |
| escort | 1 | 0 | 0 |
| self-defense | 0 | 0 | 0 |
| hold | 0 | 0 | 0 |

The original-art Quick Mission creator and its objective popup were captured
and visually inspected in `.local/mission-objective-review/`. All six stamps
fit their group rows; the ten-choice popup is readable without overlap. The
long inherited self-defense label was also captured and inspected. Tests
cover the Shift-4 assignment readout and extra camera/text margin; no human
review of a complete escort encounter was performed. Captures and imported
media remain local, not committed.

## Limits

Presets, report-identification tolerances, report expiry, pursuit limits and
objective layout margins are authored rules. Warnings with an unidentified
attacker can cue a search but never produce an aircraft firing solution.
Assignments identify duties, not hidden enemy positions. Group settings persist
within the session and mission restart; saved campaigns and mission scoring
remain outside scope. Missing patrol routes use the existing formation/heading
or explicit protected-aircraft/patrol-center guidance. Dummy aircraft remain
non-combatants. Full weather visibility and encounter-balance review remain open.
