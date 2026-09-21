# In-flight map validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-21. [Behavior specification](../spec/flight-map.md).
No retail comparison was run and no retail parity claim is made.

## Checks and observations

Linux, NVIDIA GeForce RTX 4070, Vulkan. All required checks passed:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`: 966 passed, two existing tests ignored.
- `cargo build --workspace --locked`
- Python tool tests: 68 passed.
- Source and both binary asset checks.
- Documentation header check and `git diff --check`.
- `cargo run --locked -p tore-app -- --smoke-test`.

Synthetic tests cover north-up projection and 25-mile scale displacement,
bounded 25-to-800-mile zoom, pan reset, map toggle and Escape without pausing,
buildings-off defaults, building/defense separation, matching button press and
release, cancellation and selection retention,
radar-only anonymity, visual identification, lost-contact removal, destroyed
contacts, surface observations without air-to-air designation, radar power and
terrain obstruction. No retail fixtures are used in committed tests.

GPU captures were inspected for UKR and EGY. The final EGY capture used
`--flight-map --theater EGY --capture-flight .local/map-art/egy-map.ppm
--dummy-aircraft mig21,1`. It shows original runway symbols and briefing artwork,
a visually identified MiG-21, unknown surface placeholders, a player marker and
the five right-side category buttons. Buildings are off in this capture;
airfields and other surface contacts remain visible.
An earlier UKR capture used the explicit live-fire probe to exercise dense
surface contacts before adding the category rail. Pixel artifacts and command logs stay ignored under `.local/map-art/`.
The local asset cache was re-imported through the application importer to include
MCICONS.PIC. No extracted resource or generated retail derivative is committed.

## Limits

Theater-image registration and surface sensing are fitted. Unknown surface
objects share a generic placeholder until a classification is available.
Surface-defense spawning and AI are outside this work. Keyboard transitions were
unit-tested; interactive piloting, all sixteen theater captures, Windows and
macOS were not exercised in this pass. The map continues normal simulation;
only the explicit combat capture probe pauses its scene.
