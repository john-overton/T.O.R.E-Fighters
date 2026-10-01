# First Flight reel production validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-30 to 2026-10-01. This is the second pass requested
by John: his music picks (the `XFI204CA` briefing bed with `AIR003` kicking in at
the replay shoot-down), a videotape-style HUD camera instead of the phosphor
treatment, Windows, Linux and Apple marks on the end card, a badge-only end card
with sprite sparkles, a longer end-card hold, and 4K and no-music exports. The
longer end card makes the master 32 seconds. Production ran on the
`reel/v0.1-promo` branch from main `fb9c2ec` (0.1.3). The tooling merged to main
on 2026-10-01 without the three renderer fixes these renders used; see
[renderer fixes](#renderer-fixes). Simulation and AI are as on main.
The [recipe](../../tools/reel/README.md) owns rerender instructions and
[recon](../../tools/reel/RECON.md) owns capabilities and editorial substitutions.

## Host and media

Linux x86_64, NVIDIA GeForce RTX 4070, Vulkan, driver 610.57.04. The repository
Rust toolchain and locked dependencies were used, with ffmpeg n9.0.1,
Pillow 11.3.0 and NumPy 2.3.3. The user-owned retail source is 1.02F, with
FA.EXE SHA-256 `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`.
Media is imported into ignored `out/reel/profile`. No retail code is executed.
No retail assets, recordings or generated media are committed.

## Results

Eight Quick Mission recordings supply thirteen capture clips, 1,513 frames in
all. Two consecutive capture passes matched every clip's RGBA frames, effects
mix and speech stem. The two independently composed landscape versions matched
across all 1,920 frames, and repeat encodes were identical.

| Deliverable | Frames | Duration | Size | Codecs | LUFS | dBTP |
| --- | ---: | ---: | --- | --- | ---: | ---: |
| `tore_reel_32s_1080p.mp4` | 1920 | 32.000 s | 1920x1080 | H.264, AAC | -14.02 | -1.69 |
| `tore_reel_32s_1080p.webm` | 1920 | 32.008 s | 1920x1080 | VP9, Opus | -13.99 | -1.46 |
| `tore_reel_32s_vertical.mp4` | 1920 | 32.000 s | 1080x1920 | H.264, AAC | -14.02 | -1.69 |
| `tore_reel_15s_1080p.mp4` | 900 | 15.000 s | 1920x1080 | H.264, AAC | -14.02 | -2.43 |

Every video is 60 fps with every MP4 frame at exactly n/60 s. The Opus container
is 8 ms longer because of codec pre-roll. The 4K versions
(`tore_reel_32s_4k.mp4`, `tore_reel_32s_vertical_4k.mp4`, `tore_reel_15s_4k.mp4`)
and the no-music exports were encoded from the same compositions; the table's
loudness and timing checks were run on the four 1080p deliverables.

## Edit and audio checks

The master has fourteen cuts and the cutdown seven. Decoded frames before, on
and after each cut, and twelve frames across the HUD pullback, were written to
contact sheets. All 195 unique title and logo boxes pass the landscape 4:3 and
portrait central-square checks. The last moving replay scene equals the first
freeze scene above the transport bar; the real control changes from play to
pause. The end card holds 4.53 seconds in landscape and portrait and 2.67
seconds in the cutdown.

The bed's first downbeat is on frame 90, the launch. `AIR003` kicks in on frame
1502 of the master (three bars, then its stinger) and frame 594 of the cutdown
(two bars). No audio edit is an isolated spike; the largest local step near a cut is
0.27 full scale, at master frame 922. Seven recorded
speech calls were considered for the master and six kept; one night call would
have been cut by the edit and was left out. Audio was checked by decoded
samples, loudness and true-peak measurement, not by ear on hardware.

## Renderer fixes

These renders were made with three renderer fixes on the production branch:
MiG-21 source-normal face visibility, so its differently divided underside does
not show through the upper wing; one reversed-depth ULP with no slope bias on
the textured airport detail pass, for runway paint ties; and an invariant terrain
clip position across matching GPU passes. None moved contact geometry or
simulation state. They are not part of the merged tooling and are planned with
the multiplayer work, so a rerender from main can show those artifacts and will
not reproduce the hashes above.

## Repository checks

On the merge to main on 2026-10-01 the [development checks](../../AGENTS.md#development-checks)
passed on the merged tree: Rust formatting, workspace Clippy with warnings
denied, 2,026 Rust tests, the locked workspace build, 161 Python tests, the
source and both binary asset guards, and the documentation check. Two reel plan
tests still expected the first pass's 30-second timeline and bed tempo; they now
read the recipe's `duration` and `grid_bpm`, as its own configuration check does.
The production itself was not rerun for the merge.

## Evidence and limits

Receipts and decoded cut sheets are in `out/reel/validation/`, with
`report.json` holding the measurements above. Earlier cuts are preserved in
`out/reel/previous*/`. These generated files stay ignored.

Production was validated on this Linux/GPU combination. Windows, macOS and
cross-GPU bit identity were not tested. This demonstrates this engine's current
behavior, not original-game parity. HUD readiness/ammunition readouts are not
reconstructed, and the beat treatment and other editorial choices remain
documented in [recon](../../tools/reel/RECON.md).
