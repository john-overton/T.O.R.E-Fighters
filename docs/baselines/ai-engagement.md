# AI mission engagement and group objectives validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. Source base: `d2c7bff`. This pass validates proactive escort
assessment and the existing shared missile warnings against the
[mission rules](../spec/ai-awareness.md#mission-roles-and-rules-of-engagement).
All committed fixtures are synthetic. Group-selector presentation has its own
[validation](mission-objectives.md).

## Validation

Validated on Linux with Rust 1.91.1 and locked dependencies.

| Check | Result |
| --- | --- |
| Formatting and workspace Clippy with warnings denied | Passed |
| `cargo test --workspace --locked` | 1,129 passed; 3 existing ignored tests |
| `cargo build --workspace --locked` | Passed |
| Python tool tests | 70 passed |
| Source and both executable asset scans | Passed |
| Documentation headers and diff whitespace | Passed |
| Imported escort probe, 2,400 ticks | Both friendly escorts fired; both enemy aircraft destroyed; no dropped launches |
| Display smoke test | Main menu passed on NVIDIA GeForce RTX 4070 using Vulkan |

Sixteen mission integration scenarios exercise protected-threat priority,
same-side assigned reporting, unknown bearing cues without hostile memory or
firing, exact report expiry without forwarding refresh, hidden attacker
rejection, escort leash and formation, accepted explicit orders, weapons hold
with missile defense, CAP boundaries, distant assigned targets and role-gated
memory investigation. The added scenarios exercise actor-owned radar acquisition
and supported weapon release beyond Ace visual range, terrain-masked contacts,
supported-missile defense, active-missile warning onset at seeker acquisition,
and next-tick warning delivery from a protected AI aircraft without leaking its
attacker's identity. Seventeen policy tests include the exact protection range
and time boundaries, relative charge motion, departing/passing tracks,
confirmed-attack priority, and search-only permission for frozen observations.

`target/debug/tore-app --ai-probe-ticks 2400 --ai-mission escort --no-audio`
ran the imported Quick Mission fixture with four AI aircraft for 20 simulated
seconds. The player did not attack. The same command on the source base
reproduced the reported inactivity:

| Encounter | Friendly escort shots | Total shots | Dropped launches |
| --- | ---: | ---: | ---: |
| Source base | 0 | 3 | 0 |
| Updated policy | 2, one per escort | 4 | 0 |

Both friendly escorts survived with 116 hit points each; the player retained
85 hit points. Both enemy aircraft reached zero hit points. Final checksum was
`56f6eee45c9f919a`. This verifies encounter behavior, not combat balance or retail
parity. No human review of a complete escort encounter was performed. The full
imported roster probe was not repeated for this policy-only change.

## Limits

Presets, protection-zone geometry, report-identification tolerances, report
expiry and pursuit limits are authored rules. Warnings with an unidentified
attacker can cue a search but never identify a launcher or produce a firing
solution by themselves. An independently detected aircraft may qualify as a
prospective threat based on its observed proximity and relative course.
Assignments identify duties, not hidden enemy positions. Group settings persist
within the session and mission restart; saved campaigns and mission scoring
remain outside scope. Missing patrol routes use the existing formation/heading
or explicit protected-aircraft/patrol-center guidance. Dummy aircraft remain
non-combatants. Full weather visibility and encounter-balance review remain open.
