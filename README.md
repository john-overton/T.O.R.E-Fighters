# T.O.R.E-Fighters

Tasteful Opinionated Reverse Engineered: a native Rust rebuild of Fighters Anthology, following [the roadmap](docs/ROADMAP.md). The first game-facing work will be the original menus.

The app launches into the original **Choose Activity** menu using artwork, button pieces, proportional fonts, and sounds imported from your own Fighters Anthology files. Buttons animate and show placeholder responses; `?`, `Pref`, and `Multi` open dropdowns. Gameplay and navigation to other screens are not implemented yet. No retail game data ships in this repository.

## Run on this Mac

Rust 1.91.1 is pinned through rustup. From the repository root:

```sh
source "$HOME/.cargo/env"
cargo run --locked -p tore-app
```

On first run, local `gameassets/fighters-anthology/` media is imported into platform application data. Later launches use that cache. To import another location or refresh the menu assets:

```sh
cargo run --locked -p tore-app -- --import /path/to/fighters-anthology
```

Use **? → Exit to Desktop** or close the window to quit. Escape dismisses a dropdown, Tab/arrows and Enter navigate, and M toggles music. `Pref` also toggles music and effects. Replay/continue are disabled until those systems exist.

To check startup, render one frame, and exit without audio:

```sh
cargo run --locked -p tore-app -- --smoke-test
```

See [development setup](docs/DEVELOPMENT.md) for fresh-machine setup, Linux/Windows prerequisites, checks, and troubleshooting.

## Project guide

- [Roadmap](docs/ROADMAP.md): milestones and parity goals.
- [Development](docs/DEVELOPMENT.md): environment and everyday commands.
- [Architecture](docs/ARCHITECTURE.md): baseline choices and boundaries.
- [Local references](docs/REFERENCES.md): media and TypeScript reference locations, menu starting points.
- [Menu extraction](docs/formats/menu.md): recovered assets, geometry, fonts, and fidelity boundaries.
- [Menu baseline](docs/baselines/main-menu.md): validation, screenshots, archive census, and remaining work.
- [Baseline evidence](docs/baselines/environment.md): what has actually been verified.
- [Agent instructions](AGENTS.md): automated contributor conventions.

`crates/tore-app/` contains the native shell. `tools/` contains the asset guard. GitHub Actions is configured to build and check macOS, Linux, and Windows.

Your game files belong in ignored `gameassets/fighters-anthology/`. The ignored `USNF-ATF/` checkout supplies reference specifications; it is not needed by the Rust importer or runtime. Music currently previews recovered `AIR003.11K`; its original activity-menu mapping is not confirmed. Run with `--no-audio` for a silent session.
