# AI awareness and search validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. This pass covers observation/memory and search integration
from the [development specification](../spec/ai-awareness.md). All committed
fixtures are synthetic; no retail bytes are embedded or added.

## Delivered behavior

- Skill-scaled circular visual attention and terrain-masked sensor observations.
- Actor-owned snapshots with source timestamps, frozen lost-contact poses,
  precise expiry, unlimited Ace retention and one remembered Novice hostile.
- Current observations alone supply combat targeting. Lost records supply only
  search guidance. Destroyed/removed actors and restart clear the relevant state.
- Search approach/orbit, live reacquisition, an Ace investigation limit and
  Target-view activity. Wing commands and fuel recovery preempt searching.
- Production sensor import errors no longer select the omniscient fixture path.

## Validation

Validated on Linux with Rust 1.91.1, using the locked workspace and synthetic
fixtures. Source base: `2c26ae3`; the implementation accompanies this baseline.

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | 1,051 passed; 3 existing ignored tests |
| `cargo build --workspace --locked` | Passed |
| `python3 -m unittest discover -s tools -p 'test_*.py'` | 68 passed |
| `python3 tools/check_assets.py` | Passed |
| Asset scan of `target/debug/tore-app` and `target/debug/tore-extract` | Both passed |
| `python3 tools/check_docs.py` | Passed |
| `target/debug/tore-app --ai-roster-probe-ticks 3600 --no-audio` | All 48 imported aircraft/skill cases passed, two aircraft per case, 3,600 simulation ticks each; zero dropped launches |
| `cargo run --locked -p tore-app -- --smoke-test` | Main-menu frame presented on NVIDIA GeForce RTX 4070 using Vulkan |

Focused regression cases cover exact range/cone and expiry boundaries, source
refresh, hidden target turns, Novice kill/reacquisition, radar contacts beyond
visual range, terrain masking, designation clearing, search without ammunition
use, live reacquisition, wing-order interruption, fuel priority, clockwise
horizontal orbit geometry and the Ace uninterrupted-investigation limit.
Target-window goal mapping and activity labels are validated by unit tests;
interactive visual review of a search encounter was not performed.

The 30-aircraft synthetic fixture runs two independent missions for 120 ticks
and compares all actor flight, controller and awareness states on every tick.
The isolated test body took 0.07 seconds in the unoptimized test build on this
host, including both simulations and equality assertions. This is a small
regression workload, not a rendering benchmark or performance guarantee.
The imported roster probe took 11.28 seconds wall time for all 48 cases. It
checks existing flight/input and launch integration, not complete missile AI
or encounter balance. Imported media was read locally at runtime only.

## Limits

Missile-driven warnings, defensive timing, RWR missile plots, mission roles and
escort priorities remain pending. Existing launch-warning behavior is unchanged.
Cloud/night visibility is not yet passed into AI perception. Mission routes and
objective assignments remain unavailable, so search returns to existing
formation or heading behavior. Search motion is fitted through the existing
flight model; no original-game visual comparison is available and no retail
parity claim follows from the tests.
