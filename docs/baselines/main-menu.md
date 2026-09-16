# Main-menu baseline — 2026-09-13

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


## Result

The Rust app imports original menu resources from the user's Fighters Anthology installation, randomly selects one of five original Choose Activity backgrounds, composites original enabled/disabled action pieces and proportional fonts, and supports animated placeholder buttons and top-bar dropdowns. The user's photo matches `CHOOSEV.PIC`. No custom TypeScript menu controls were carried over. Hover/focus is silent; sounds play on clicks/toggles.

The M3 Mac successfully presented the menu through `Apple M3 (Metal, IntegratedGpu)`. Normal startup opened `MacBook Air Speakers` at 48,000 Hz stereo for original PCM effects and the optional music preview. This verifies device initialization/playback submission; it is not a measured audio-fidelity comparison.

## Local media census

| Archive | Bytes | Directory entries | Contents relevant to next work |
| --- | ---: | ---: | --- |
| `FA_1.LIB` | 28,515,814 | 2,001 | 1,986 PIC images, 15 FNT resources |
| `FA_2.LIB` | 31,546,692 | 5,405 | 92 DLG, 12 MNU, 145 PT, 37 PTS, 1,275 SH, 46 HUD, 16 T2, 78 XMI, 9 MUS, 114 11K, 781 5K, other mission/object resources |
| `FA_4B.LIB` | 34,670,738 | 77 | Compressed PCM music recordings |
| `FA_4D.LIB` | 13,756,838 | 22 | Stored PCM recordings |
| `swpatch.lib` | 219,722 | 15 | Toolkit metadata, seven aircraft images and seven shape overrides |

The patch resources concern B1, F111, F14, MIG23, TOR, TU160, and TU26. None overlaps the selected menu resources. Patch precedence is not implemented for future aircraft imports. PT/SH counts do not mean that many flyable aircraft: shapes include other objects and variants. This is an installation census, not the full per-title disc census required by M0.

## Evidence

- `python3 tools/explore_assets.py` validated directories and wrote source hashes, entry offsets, compression headers, and counts to ignored `.local/exploration/inventory.json`.
- All 22 native-decompressed imported menu resources matched the reference Python decoders byte for byte. Decoded SHA-256 values and equality results are recorded in `.local/exploration/decoder-comparison.json`.
- Headless CPU-compositor previews were generated for normal, hover, pressed, Help, Pref, and Multi states. Images live in `.local/exploration/menu-*.ppm` / `.png`. Normal/pressed/dropdown previews were visually inspected. The normal background and button/font reconstruction match the supplied reference in content and geometry; no numerical photograph-parity score is claimed.
- `cargo run --locked -p tore-app -- --smoke-test` initialized Metal and presented the actual main menu successfully. This is a GPU smoke test; it does not automate OS mouse/keyboard input.
- Rust unit tests exercise decompression, malformed archives/PIC data, click cancellation, disabled controls, dropdown exit/dismissal, keyboard traversal, PCM resampling, and letterbox coordinate mapping with synthetic inputs.
- Final follow-up checks passed: formatting, Clippy with warnings denied, workspace build, 19 Rust unit/integration tests on macOS, five Python asset-guard tests, and source/both-executable asset scans.
- Cached startup also passed from `/tmp`, confirming that an imported menu runs independently of the checkout/current working directory.
- Follow-up background selection was traced in the original executable, including the five-way random choice and three menu-bar origins. All five variants were rendered to local previews; the shifted CHOOSE3 bar and CHOOSEAC variant were visually checked. See [the address-level evidence](../formats/menu.md).
- The shared extractor successfully unpacked all 7,520 resources (301,951,459 decoded bytes), including all 7,372 compressed entries, into `.local/extracted/`. Archive hashes before and after extraction match. The JSON report records successful outputs and provenance; source media was unchanged.
- A second full extraction returned `unchanged` for all 7,520 outputs with zero errors. The portable Python entry point also successfully listed a single archive from `/tmp`, independent of the caller's working directory.
- Linux/Windows build configuration includes ALSA development headers on Linux for CPAL. Those runners were not executed locally.

Images, packs, and retail derivatives remain outside tracked source. Source and executable scans allow decoder format-name constants while rejecting plausible embedded archives/PIC payloads.

## Remaining fidelity work

Main button art, label glyphs, screen palettes, background choices/bar origins, and DLG geometry are recovered. Hover/press treatment and placeholder messages are authored. Dropdown contents/chrome still need native menu-tree and executable validation. Music previews `AIR003.11K`; original menu track selection and loop behavior remain unconfirmed. See [detailed findings](../formats/menu.md).

No quick-mission screen, loadout, aircraft, campaigns, networking, title sequence, XMI synth, or saved preferences are implemented. The importer reads selected files from loose FA archives; no disc installer/ESA flow exists. M1a remains in progress.
