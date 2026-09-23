# Homepage validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-23. This pass builds the
[static homepage](../index.html) on branch `website/boxed-homepage`, in the
separate `T.O.R.E-Fighters-homepage` worktree based on `7502413`. No game code,
adapter defaults, or simulation behavior changed. Design choices, image sources,
the requested screenshot exception, and publication instructions are in the
[website guide](../WEBSITE.md).

## Gameplay captures

All three selected images are real 1920 × 1080 GPU captures from the rebuilt
game, using the default researched flight model, the Ukraine theater, and a
separate `.local/homepage-profile` data directory. They use the user's existing
imported media cache. No source game media or cache is added to the website.
The game ignores saved display preferences during diagnostic captures, so
the original instrument windows remain visible. Paused labels remain in the
missile and takeoff images.

Build first with `cargo build --workspace --locked`. From the worktree root,
after preparing the isolated imported-media profile:

```sh
export TORE_DATA_DIR="$PWD/.local/homepage-profile"
export TORE_LOG_DIR="$PWD/.local/homepage-review/logs"
mkdir -p .local/homepage-review/captures

TORE_WEATHER_TIME=07:20 target/debug/tore-app \
  --aircraft f18 --weather-condition 3 \
  --flight-view 1 --flight-look -95,-20 --flight-zoom 2.4 \
  --window-size 1920x1080 \
  --capture-flight .local/homepage-review/captures/dawn-final.ppm --no-audio

target/debug/tore-app --aircraft f18 --live-fire --weapon-slot 2 \
  --combat-command seeker-mode --combat-probe-ticks 25 \
  --flight-view 1 --flight-look 95,-25 --flight-zoom 2.4 \
  --window-size 1920x1080 \
  --capture-flight .local/homepage-review/captures/missile-final.ppm --no-audio

target/debug/tore-app --aircraft f18 --ground-start 2 \
  --maneuver takeoff --flight-probe-ticks 1220 \
  --flight-view 1 --flight-look -75,-9 --flight-zoom 2.4 \
  --window-size 1920x1080 \
  --capture-flight .local/homepage-review/captures/takeoff-final.ppm --no-audio
```

All captures exited successfully on the Linux display host. These are automated
display/GPU captures, not headless flight screenshots. Each selected PPM was
converted directly to WebP at quality 90, without cropping, color adjustment,
generated additions, or removal of game interface elements. The screenshot
subject and rendering were visually reviewed. These images show the current
development build, not a comparison with retail or a claim of retail parity.

## Website checks

Chromium loaded the page from a local server using the project URL prefix
`/T.O.R.E-Fighters/`. Automated checks and rendered previews covered 1440, 768,
390, and 320 pixel viewport widths:

- All images load, all fragment targets exist, and there is no horizontal overflow.
- Images, fonts, and layout are local. Optional release and star-count updates
  make two independent public GitHub API requests per page load; the page works
  without either request.
- Desktop and phone screenshots were visually reviewed.
- The roadmap navigation reaches the correct section.
- The first keyboard focus target is the visible skip-to-content link.
- Reduced-motion preference disables smooth scrolling.
- Direct `file://` loading also resolves the relative image paths.

The hero patch's requested 50% enlargement was reviewed in desktop and mobile
layouts. Desktop dimensions are 198 × 198 pixels and tablet dimensions are
157.5 × 157.5 pixels; it remains hidden in the existing narrow-phone layout.

The top GitHub link's star count was checked against the live repository API
and controlled zero, one, and 1,234-star responses. Compact display, the exact
count in its tooltip and accessible label, invalid-count rejection, offline
fallback, and disabled JavaScript passed. Desktop, tablet, and phone previews
at 1440, 768, 390, and 320 pixels had no horizontal overflow. The count is
fetched independently of downloads; the link always opens the repository.

The latest-release section was tested in Chromium with controlled API responses:

- The newest published pre-release wins over an older stable release; drafts
  are excluded. Stable and pre-release badges, tag, UTC date, and release notes
  use the selected release.
- Windows MSI, Apple Silicon DMG, Intel DMG, and Linux AppImage links resolve to
  their exact matching uploaded assets. File sizes display in megabytes.
- Linux's tar.gz appears as an alternate, or the primary download when no
  AppImage exists. Missing platform assets link to release details and are not
  substituted with another architecture or an older release.
- Empty feeds, rate limits, offline errors, malformed responses, the real
  eight-second timeout, and disabled JavaScript retain usable GitHub links.
- Unfinished, empty, foreign-project, and off-site download assets are ignored.
  A tag containing HTML renders as text.
- Populated download cards have no horizontal overflow at 1440, 390, and 320
  pixel widths. The versioned download state was visually reviewed using
  synthetic release metadata, not a fabricated public release.

The live unauthenticated GitHub request returned an empty release list on
2026-09-23. The page correctly shows **Coming soon** and the source-build link.
No published installer was available for an end-to-end package download check.
Release behavior is documented in [the website guide](../WEBSITE.md#latest-release-downloads).

The local review scripts, browser output, and full-page screenshots are under
ignored `.local/homepage-review/`. No browser automation dependency was added to
the project. GitHub documentation links use real repository paths; an actual
GitHub Pages deployment was not performed. Safari, Firefox, Windows, and macOS
browser rendering were not run.

## Repository checks

The following passed in this worktree:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`: 1,421 passed, 3 ignored, 0 failed.
- `cargo build --workspace --locked`
- `python3 -m unittest discover -s tools -p 'test_*.py'`: 83 passed.
- `python3 tools/check_assets.py`
- `python3 tools/check_assets.py target/debug/tore-app`
- `python3 tools/check_assets.py target/debug/tore-extract`
- `python3 tools/check_docs.py`
- `TORE_DATA_DIR=.local/homepage-profile cargo run --locked -p tore-app -- --smoke-test`

The display smoke test presented successfully. Existing graphics-driver warnings
were emitted, and the head-tracker listener reported that UDP port 4242 was
already in use; neither prevented capture. No retail comparison was run.
The feature matrix was reviewed; this website pass adds no gameplay feature row.
