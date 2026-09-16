# Full-canvas cockpit, HUD and desktop controls checkpoint

> **T.O.R.E — we trace what the player does, not what the code did.**
> This project reverse-engineers *player interaction*: what you press, see, hear
> and feel in Fighters Anthology, and the numbers behind it. It does not
> reproduce the original program byte by byte. Anything here about the original
> executable is evidence toward a behaviour spec — never a specification for what
> we build. If a sentence below reads like an instruction to reproduce the
> original's internals, it is out of date.
> <!-- tore-header v1 -->

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


2026-09-13, Apple Silicon M3/macOS, Rust 1.91.1, wgpu Metal. This follows the [initial F/A-18D slice](f18-free-flight.md) and replaces its provisional split cockpit layout. No new dependency, retail bytes or derived pictures are tracked.

## Outcome

- The world fills the flight canvas. The source forward frame uniformly covers it, with separate instrument overlays; the lower opaque PANEL region is removed.
- The source HUD11 font draws flight data and a perspective-projected attitude/path display. Visual review moved heading and numeric tapes inside the enlarged combiner aperture.
- The runtime reads all **133 linked nodes / 123 labeled entries**, including nine roots and anonymous submenu containers, in the supplied FMENUD. There are **34 shortcut rows / 31 distinct accelerators** (four time-scale rows share C).
- Escape opens a pausing menu. Original menu shortcuts are dispatched to supported actions or explicit unavailable notices. Desktop bindings and remaining non-menu key-table recovery are documented separately in [FLIGHT-CONTROLS](../FLIGHT-CONTROLS.md).
- Pause, focus loss, menu transitions and modifier changes clear or suppress flight inputs; paused wall time is not integrated on resume. Source camera views and development bindings no longer conflict with target cycling.

## Verification

**54 Rust tests and five Python tests pass.** Formatting, Clippy with warnings denied, locked workspace build and repository/both executable asset guards pass. Tests include bounded/cyclic menu links, nested keyboard navigation, matching mouse release, physical shifted/Option key normalization, modifier isolation, pause without tick catch-up, fixed-tick time scaling and HUD heading/projection checks. The existing format, flight-model, extraction and menu tests remain green.

Twelve actual Metal checks passed: Choose Activity, Quick Mission Creator, developer terrain viewer, five flight views (front/chase/oblique/back/up), instrument pages 0/2/3, and the Escape menu. Main cockpit and Escape captures were inspected visually. Captures verify GPU startup/composition; they do not establish manual acceptance of every native menu handler or joystick input. Windowed default 4:3 captures were used; wider/portrait layout acceptance remains open.

Shared script extraction with `--aircraft f18 --weapons` selects **388 resources, zero errors**: the prior 384 resources remained unchanged and four were added (three HUD font modes and FMENUD). The app detected the old cache's missing HUD11 and re-imported successfully from local media. All 16 theater resources remain in the runtime cache; this follow-up's flight GPU checks use Ukraine, not a new native acceptance run for every theater.

Reproduce from the repo root (create the ignored output directory first):

```sh
mkdir -p .local/f18-research
cargo run --locked -p tore-app -- --capture-flight .local/f18-research/cockpit-view-0.ppm
cargo run --locked -p tore-app -- --flight-menu --capture-flight .local/f18-research/escape-menu.ppm
cargo run --locked -p tore-app -- --flight-view 3 --capture-flight .local/f18-research/back.ppm
cargo run --locked -p tore-app -- --instrument-page 0 --capture-flight .local/f18-research/rcs.ppm
python3 tools/extract_assets.py --aircraft f18 --weapons --exclude-archive 'disc1/LHX/*' --out .local/f18-import
```

Local evidence: `.local/f18-research/cockpit-tests.log`, `cockpit-gpu-checks.json`, `cockpit-extract.log`, `cockpit-view-{0..4}.ppm`, `cockpit-page-{0,2,3}.ppm` and `escape-menu.ppm`. Extracted resource identities:

| Resource | SHA-256 |
| --- | --- |
| FMENUD.MNU | `d667bbf13d3a8392ecbbb32053dadd2394edb232b2ca1bb42ae5d5b90403f529` |
| HUD11.FNT | `21e0b85e903b3df64acc1a33e1c6e583a8445f0bebc160f7d16c34a756168e9d` |

## Lessons and remaining acceptance

A wide cockpit bitmap does not justify reducing the world viewport to its aspect ratio. World projection and cockpit art composition are independent; keeping the raster's aspect by cropping it avoids the previous split and stretched cockpit geometry.

The menu modules contain usable linked data even though native handlers remain unported. Anonymous containers, different root/row headers and accelerator control characters must be handled explicitly. Recovering the tree corrected several provisional desktop bindings and exposed the missing RCS window. A menu item is not evidence that its subsystem works; unsupported preferences/cheats/combat commands report that limitation.

Keep the renderer's projection/roll conventions and HUD projection synchronized. FNT text and procedural line symbology are different recovery problems: the original font is now used, while native HUD callers and HUDSYM glyph identities remain unported.

Windows/Linux runtime, audible comparison, gamepad/joystick support, native desktop-key-table completeness and original-game visual/flight acceptance remain open. Mirrors are still flat source fills, the current dynamics are authored, and flight/combat/navigation systems retain the limitations documented in [aircraft coverage](../formats/aircraft.md).
