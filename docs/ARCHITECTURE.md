# Initial architecture

This implements the environment portion of roadmap M0. M0's title census, salvage inventory, parity specification, and AI VM decision remain open. M1a menu work follows this foundation.

| Component | Choice | Purpose |
| --- | --- | --- |
| Language | Rust 2024, compiler 1.91.1 | Reproducible native builds |
| Workspace | `crates/tore-app` | Application entry point and desktop shell |
| Window/input | `winit` 0.30 | Native window lifecycle and input |
| Graphics | `wgpu` 27 | Metal on macOS; native backends for Windows/Linux |
| Startup bridge | `pollster` 0.4 | Wait for GPU initialization without a general async runtime |
| Checks | Cargo, Python standard library, GitHub Actions | Local and CI checks |

These are baseline-compatible versions, not automatically the latest versions. Exact versions live in `Cargo.lock`. See upstream [wgpu 27 documentation](https://docs.rs/wgpu/27.0.1/wgpu/) and [winit 0.30 documentation](https://docs.rs/winit/0.30.13/winit/) for API contracts.

The shell creates its window on the main event loop, obtains a compatible adapter, and clears a surface. It handles resizing, zero-sized windows, and recoverable surface loss. Fatal graphics/startup errors return a failing process status. It redraws on demand to keep an idle shell inexpensive on a laptop.

## Boundaries for menu work

Add format/import crates when decoding starts; keep them independent of windowing and GPU APIs. The future simulation likewise needs to run headlessly with deterministic inputs. Avoid empty placeholder crates or premature engine abstractions.

Menu rendering should consume decoded palettes, indexed images, font data, and recovered layout geometry. Recover specifications from the TypeScript reference; implement runtime behavior in Rust. Do not introduce a web shell, copy the Three.js engine, or select a modern widget toolkit before checking retail geometry requirements.

Import user-owned media at runtime into platform application data. The exact application ID, cache schema, and import interface remain to be designed with the importer. `gameassets/` is a local source-media convenience, not a runtime bundle or future save-data location.
