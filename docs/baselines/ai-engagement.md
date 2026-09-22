# AI mission engagement and group objectives validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. Source base: `0f42f41`. This pass validates
[neutral formation and leader authorization](../spec/ai-awareness.md#formation-and-leader-authorization)
and the existing mission engagement rules. All committed fixtures are synthetic.
Group-selector presentation has its own [validation](mission-objectives.md).

## Validation

Validated on Linux with Rust 1.91.1 and locked dependencies.

| Check | Result |
| --- | --- |
| Formatting and workspace Clippy with warnings denied | Passed |
| `cargo test --workspace --locked` | 1,140 passed; 3 existing ignored tests |
| `cargo build --workspace --locked` | Passed |
| Python tool tests | 70 passed |
| Source and both executable asset scans | Passed |
| Documentation headers and diff whitespace | Passed |
| Imported free-fire and escort probes, 2,400 ticks each | Both sides remained in formation; zero shots, warnings or dropped launches |
| Display smoke test | Main menu passed on NVIDIA GeForce RTX 4070 using Vulkan |

Twenty-three mission integration scenarios cover protected-threat priority,
radar acquisition, terrain masking, missile warning onset and sharing, report
expiry, escort leash, mission-gated memory, and the added neutral command rules.
The new cases verify both-side neutral starts, preserved objectives and memory,
recipient-specific engagement, cancellation of pursuit/designation/search,
leader response confined to its own wing, and human-led wings remaining neutral.
A real missile snapshot verifies that a report waiting for delivery when recall
arrives cannot release the wing through later warning refreshes. A new missile
can trigger a new leader command; missile evasion remains available throughout.

App tests additionally exercise targetless Protect Me, Attack on Contact after
recall, rejection of invalid engage orders, and cancellation of queued physical
gun shots. The gun test emits one shot, recalls with either formation selection
or disengage, advances 20 ticks and verifies that no further queued shots appear.
The previously accepted burst ammunition remains debited. Intentional
cancellation does not increase the failed-launch counter.

## Imported encounter probes

Each command used the normal Quick Mission build path, four imported F/A-18D AI
aircraft and 20 simulated seconds. The player issued no attack orders or shots.

```sh
target/debug/tore-app --ai-probe-ticks 2400 --ai-mission free --no-audio
target/debug/tore-app --ai-probe-ticks 2400 --ai-mission escort --no-audio
```

Both cases ended with all four AI aircraft alive and reporting `In formation`,
582 rounds per actor, zero shots, no live projectiles and player hit points 232.
Both checksums were `188ab0d788ff2e82`. Preset and objective assignment therefore
do not bypass neutral startup. Authorized engagement and fresh-attack release
are verified by the synthetic command and missile integration scenarios.

## Limits

Neutral startup and recall are requested opinionated behavior. The automatic
AI leader's attack-triggered release, protection-zone geometry, warning/report
association and timing remain authored rules. Unknown missile launchers never
become synthetic aircraft targets. Physical evasion remains possible after
recall even when the same missile can no longer authorize offensive pursuit.
No manual review of a complete encounter was performed. The full imported
roster probe was not repeated for this command-policy change. Campaign routes,
mission scoring, broader leader tactics and full weather visibility remain
outside this pass; retail comparison remains unavailable.
