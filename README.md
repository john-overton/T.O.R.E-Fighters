# T.O.R.E-Fighters

Tasteful Opinionated Reverse Engineered: a native Rust rebuild of Fighters Anthology, following [the roadmap](docs/ROADMAP.md). The first game-facing work will be the original menus.

The development baseline opens a blank, dark window and prints the graphics adapter/backend. Menus, importing, audio, and gameplay are not implemented yet. No retail game data ships in this repository.

## Run on this Mac

Rust 1.91.1 is pinned through rustup. From the repository root:

```sh
source "$HOME/.cargo/env"
cargo run --locked -p tore-app
```

Close the window or press Escape to quit. To check startup, render one frame, and exit:

```sh
cargo run --locked -p tore-app -- --smoke-test
```

See [development setup](docs/DEVELOPMENT.md) for fresh-machine setup, Linux/Windows prerequisites, checks, and troubleshooting.

## Project guide

- [Roadmap](docs/ROADMAP.md): milestones and parity goals.
- [Development](docs/DEVELOPMENT.md): environment and everyday commands.
- [Architecture](docs/ARCHITECTURE.md): baseline choices and boundaries.
- [Local references](docs/REFERENCES.md): media and TypeScript reference locations, menu starting points.
- [Baseline evidence](docs/baselines/environment.md): what has actually been verified.
- [Agent instructions](AGENTS.md): automated contributor conventions.

`crates/tore-app/` contains the native shell. `tools/` contains the asset guard. GitHub Actions is configured to build and check macOS, Linux, and Windows.

Your game files belong in ignored `gameassets/fighters-anthology/`. The ignored `USNF-ATF/` checkout supplies reference specifications; it is not required to build or run this baseline.
