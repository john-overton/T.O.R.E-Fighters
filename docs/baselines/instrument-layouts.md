# Instrument layouts checkpoint

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


2026-09-13, Apple M3/macOS, Rust 1.91.1, wgpu Metal. The supplied `gameassets/reference-photos/f-14-cockpit-on-catapult.jpeg` guides large-window placement; the Hornet artwork remains unchanged.

Large uses 160×156 windows at (8,8), (8,316), (472,316), (472,8) on the 640×480 canvas. Small uses 96×94 windows at Y=378, X=8/110/212 and 332/434/536. Both retain eight-pixel outer margins; the small groups have six-pixel gaps and a 24-pixel center gutter. These are authored presentation coordinates matching the user's requested layout.

The source `Large windows?` menu item now changes layouts. Each mode retains its own page selection. Toggling a page beyond the current capacity evicts its oldest selected page; toggling layouts restores the other mode's selections. Mouse hit coordinates are converted back into the existing 160×156 instrument raster, so scaled buttons remain clickable. Layout and page changes cancel pending presses.

Validation: 56 Rust tests, five Python tests, formatting, Clippy with warnings denied, locked build and asset guards. Focused tests cover both scaled button transforms, mismatched releases, screen margins, window non-overlap, capacities and independent selection restoration. Metal captures of both layouts are stored locally as `.local/f18-research/instruments-{large,small}.ppm`/`.png`. Creator, viewer, flight menu and small camera-instrument GPU smoke checks also pass. Windows/Linux and manual native-game comparison remain open.

```sh
cargo run --locked -p tore-app -- --free-flight --instrument-layout large
cargo run --locked -p tore-app -- --free-flight --instrument-layout small
```

For captures, create an ignored output directory and replace `--free-flight` with `--capture-flight .local/layout.ppm`. Small windows use area-filtered reduction of the existing raster to preserve thin glyph and scope strokes; no new font, retail asset or dependency was added.
