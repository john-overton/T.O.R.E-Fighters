# Menu music startup regression

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation-mode investigation and validation, 2026-09-22, against the
[menu music startup specification](../spec/menu-music.md).

## Cause and scope

Commit `bed7bdf` (2026-09-14, recorded music playback) changed startup to move
`assets.sounds` into Audio before constructing Menu. Menu retained its older
`AIR003.11K` presence check as the initial Music preference. That map was now
empty on normal audio-enabled launches, so fresh profiles started muted. This
predates the installer work. Silent snapshots/smoke tests did not move the
samples and therefore concealed the faulty default.

Saved preferences are applied later and can override it. The local preference
file contained `music true`, all eight main-menu recordings were present in the
active cache, and the bounded real-audio test initialized CPAL successfully.
John confirmed that music played in that test. This does not reproduce failure
with an explicit saved Music On choice, and no such cause is claimed here.

The fix separates the initial preference from sample-buffer ownership. Explicit
saved Off remains Off; it is not overwritten or automatically migrated because
a false saved value cannot be distinguished from an intentional user choice.

## Validation

A synthetic regression exercises the actual Menu constructor after transferring
the sample map, then applies a serialized saved mute and toggles M. It fails
on the old default and passes with the fix, including preservation of an
explicit saved Off choice and re-enabling with M.

Formatting, warnings-denied Clippy, locked workspace build/tests, all 75 Python
tests, the source and both debug-binary asset guards, documentation headers and
diff checks passed. Rust results: 1,209 passed, three explicit GPU tests ignored.
The main-menu GPU smoke test passed on NVIDIA RTX 4070 / Vulkan.

A separate profile containing the imported cache and no preferences opened the
fixed app with real CPAL output at 44,100 Hz stereo, without audio initialization
or stream errors. This windowed test was intentionally stopped after 12 seconds
by its timeout, which returns 124. It confirms device/startup operation, not a
new manual listening comparison. The known missing phrases in non-menu flight
scores remain separately reported and were not changed.

Local evidence is retained under `.local/menu-music-regression/`. User-owned
music and cache data remain ignored. Original-game listening comparison and
Windows/macOS execution were not performed.
