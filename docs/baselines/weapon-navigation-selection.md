# Weapon and navigation selection validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation validation on 2026-09-21, Linux with NVIDIA GeForce RTX 4070
Vulkan. [Behavior specification](../spec/weapon-navigation-selection.md).

## Results

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: passed, 996 tests, 3 existing ignored tests.
- `cargo build --workspace --locked`: passed.
- Python tools suite: 68 tests passed.
- Asset checks: repository, `target/debug/tore-app`, and
  `target/debug/tore-extract` passed.
- Documentation header check and `git diff --check`: passed.
- `cargo run --locked -p tore-app -- --smoke-test`: passed on the display host.

Synthetic coverage verifies forward/backward selection wrapping through NAV,
NAV fire inhibition, weapon arming, retired keys, pointer/hardware MFD buttons,
page wrapping, airport allegiance and permission exclusions, destroyed runway
exclusion, distance sorting with deterministic ties, selection retention across
reordering, empty mission lists and supplied waypoint ordering. At this checkpoint, missile tests
verified no-designation bore silence without changing acquisition or release,
and continued tone availability after designation. The later requested IR bore
audio is covered in [flight sound validation](flight-sound.md). A synthetic HUD raster check verifies
that the friendly X adds only centered diagonal pixels and retains the target box.
Combat command serialization
roundtrips the new selection commands.

GPU captures with `--instrument-page 8` and `--instrument-page 6` were visually
inspected. The WEAPONS panel showed imported short names, grouped ammunition,
selected gun marker, CHAFF/FLARE counts and three labeled controls without overlap.
The NAV panel showed the honest empty mission state and source button. Further
GPU captures verified NAV and LCOS at the shared status position. These remain
local as `.local/nav-hud-label.ppm` and `.local/lcos-hud-label.ppm`. The friendly
X was checked with a synthetic pixel test, not a live mission capture. Captures
remain local in `.local/weapons-selection.ppm` and `.local/nav-selection.ppm`.
Populated airport rows were covered by data/button tests, not a GPU capture.

## Limits

No retail execution/comparison, Windows or macOS runtime checks, or human
listening test was performed. Mission route import remains absent; the ordered
waypoint selector is tested with synthetic coordinates. Airport landing
eligibility uses the existing host allegiance/permission policy and does not
promise freedom from nearby combat threats. All local retail-derived captures
remain ignored and are not repository artifacts.
