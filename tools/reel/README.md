# First Flight promo reel

Implementation mode. The production tooling for the **v0.1 FIRST FLIGHT** promo
reel, made on the `reel/v0.1-promo` branch from main `fb9c2ec` (0.1.3) and
merged to main on 2026-10-01.
[Recon and substitutions](RECON.md) explain the source capabilities and every
editorial choice. [Credits](CREDITS.md) list what is retail, what is authored
and what is licensed.

## Rerender

Run from the repository root:

```sh
tools/reel/render.sh /path/to/your/Fighters-Anthology
```

Requires the pinned Rust toolchain, Python 3.11+ with venv/pip, ffmpeg/ffprobe
with x264, VP9 and Opus encoders, and a working headless wgpu GPU adapter.
No display or audio device is opened. The script installs pinned Pillow and
NumPy into `out/reel/venv`, builds with `--locked`, imports the supplied media
into an isolated profile, validates all imported maps, records the scenarios,
exports the retail score phrases, captures every clip twice, compares frames,
HUD layers and audio stems, composes two matching landscape renders plus the
portrait and 15-second edits, mixes and masters the sound, encodes the
deliverables and runs the output checks. A full run takes about 30 minutes on
a 12-core desktop with an RTX 4070.

All imported media, recordings, audio, frames, videos, the virtual environment
and validation evidence stay in ignored `out/reel/`. Allow about 15 GB of disk
space; only the landscape master is kept lossless after encoding. Nothing is copied into a normal player profile. Runtime fonts for the
lettering are OFL licensed and supplied with the recipe, see [credits](CREDITS.md).

Outputs in `out/reel/`:

* `tore_reel_32s_1080p.mp4`: H.264 High, 1920x1080, 60 fps, yuv420p, AAC 192k, faststart.
  The name follows the recipe's `duration`; John's longer end card made it 32 s.
* `tore_reel_32s_1080p.webm`: VP9 with Opus 192k.
* `tore_reel_32s_vertical.mp4`: 1080x1920, reframed shot by shot (see below).
* `tore_reel_15s_1080p.mp4`: boot, launch, the whole HUD-to-cockpit reveal, the
  whole replay sequence and a longer end card, with its own music edit.
* `tore_reel_32s_4k.mp4`, `tore_reel_32s_vertical_4k.mp4`, `tore_reel_15s_4k.mp4`:
  3840x2160 and 2160x3840 versions, H.264 High at level 5.2. The engine captures
  at 1080p; these double each lossless composition with Lanczos and then draw
  the tube curvature at full size, so the CRT finish stays smooth.
* `*_no_music.mp4` beside each of the above, and `tore_reel_32s_no_music.wav` and
  `tore_reel_15s_no_music.wav` (24-bit): the same effects, voices and Foley with
  the score removed, at the finished film's levels, for cutting in other music.
* `stills/`: lossless PNG frames chosen in `shots.toml`.
* `validation/`: repeat hashes, text and logo bounds, loudness, speech-edge and
  audio-step checks, the end-card hold, and contact sheets of decoded frames
  immediately before, on and after every cut, plus the HUD pullback.

Development stages:

```sh
tools/reel/render.sh /path/to/install --stage record
tools/reel/render.sh /path/to/install --stage capture [--only launch,night]
tools/reel/render.sh /path/to/install --stage edit
tools/reel/render.sh /path/to/install --stage verify
```

`--only` restricts the capture step while framing shots. A complete final run
must cover every clip. Recordings are reused only when source-media hashes and
scenario inputs match. Hardware or driver changes require new verification;
cross-GPU bit identity is not promised.

## The recipe

`shots.toml` is the single source of truth and is meant to be edited by hand.

* `[[scenario]]`: a real Quick Mission probe: aircraft, theater, start time,
  wind and probe arguments, flown by the unchanged researched flight model at
  120 Hz. The recording and its event export land in `out/reel/replays` and
  `out/reel/events`.
* `[[capture]]`: one window of one recording, drawn by the engine at 1920x1080.
  `in_tick`, `frames`, `speed` (2 recorded ticks per output frame at 1, -4 for
  the viewer's rewind) and `freeze_tick` choose the recorded time. `anchor` is
  the recorded aircraft whose heading frame holds the camera; camera keys are
  feet, X right, Y up, Z forward, with vertical `fov`. `interp = "pchip"`
  passes smoothly through every key without overshoot; `"smoothstep"` eases
  into each key, so two equal keys hold. FOV always interpolates in zoom space.
  An optional `aim = [x, y]` key places the `target` point at that normalised
  screen position (-1..1, x right, y up) instead of the centre; the look
  direction is solved per frame. `eye_tick` fixes the eye in the anchor's frame
  at that tick while the target keeps following. `view` is `external`,
  `cockpit` or `replay` (the real viewer with its transport, timeline, timer,
  labels and subtitles). `clock` sets visual minutes of day, `[-1, -1]` keeps
  the recording's. `hud_layer = true` also writes the visible HUD symbols as
  their own lossless layer. `tail` captures extra frames for sound only.
* `[[shot]]`: the edit, contiguous output frames. `layout` is `boot`, `full`,
  `aircraft`, `theater`, `cockpit`, `replay` or `end`. `transition` is how the
  shot begins: `cut`, `flash`, `wobble` (a brief sync wobble across the cut),
  `static`, `tape`, `slam` or `power-on`. `bloom` scales highlight halation.
  `tape = [hold, gone]` holds the videotape treatment of the HUD camera and
  fades it out while the engine zoom pulls back. `audio_offset` advances a
  shot's captured sound by whole frames and `audio_tail` lets it ring on over
  the next shot, both from the capture's own tail. `speech = "none"` drops a
  shot's radio. `vcrop` keys the portrait crop centre (0..1 of the landscape
  width) and `vzoom` the showcase band's magnification.
* `[music]`: the score bed and its ending, measured beat grids, the frame of
  the launch downbeat, the end-card frame for the final hit, ducking depth and
  the retail Foley clips exported with the music.
* `[cutdown]`: the 15-second edit as `[shot, first frame, frames]` segments of
  the same shots. Its end card may be longer than the master's.

Every cut after the boot lands within one frame of a beat of the bed. The
configuration check enforces it, so moving a cut means moving it by whole beats
(25.22 frames at 142.72 BPM).

## Picture

The engine draws each output frame at an exact recorded tick: two 120 Hz ticks
per 60 fps frame, independent of wall-clock speed. The compositor then applies
a restrained 1990s CRT finish: soft-knee halation, slight black lift, triple-line
scanlines that thin on bright detail, a vignette and fine grain, all seeded by
frame number. The final encode adds tube curvature with a slightly different
curvature per colour gun, a radial colour fringe that is zero at the centre.

The HUD close-up is the same F-22's cockpit capture magnified 2.5x by the
engine's own zoom, which scales the scene, HUD raster and cockpit art together
about the forward datum, so the target box and gun pipper stay registered. It
is treated as an old videotape recording of the HUD camera: soft luma with edge
ringing, smeared and delayed chroma, red and blue fringes, grain, oxide dropouts,
line jitter, a rolling tracking band and head-switching at the bottom. The
separate HUD symbol layer lets the symbology halo the way a bright recorded HUD
does. The treatment fades while the zoom pulls back into the full cockpit.

Portrait gameplay is a full-resolution 1080 square cut around the subject in the
portrait's central square, over a darkened blur of the same picture. Showcases
use a 16:9 band cut around the aircraft or coast with the roster below, and the
replay keeps its complete window. All portrait lettering stays in the central
1080x1080 square, so a square crop of the portrait keeps every title; landscape
lettering stays inside the 4:3 safe area.

## Sound

`--reel-music` exports every imported score phrase with the game's PCM phrase
player and fixed seed. John picked the music from loudness-matched exports. The
bed is the briefing track `XFI204CA`, stretched without changing pitch from its
measured 138.34 BPM onto the 142.72 BPM cut grid, its first downbeat on the
launch. On the first frame of `kick_shot` it kicks into bar 1 of `AIR003`
from the `M_SUCC.MUS` success score, which plays whole bars, then splices on
the beat into its own final stinger. The `[music]` table holds the measured
tempos, beats, the stinger point and the loudness-matching gains.

Each capture writes its effects mix and, separately, the radio and crew speech
it heard, with the sample at which each recording starts. A recorded call is
heard whole or not at all: it must start inside its shot and finish within
20 ms of the cut, or the edit leaves it out. Captures with radio carry a few
extra frames so a call's true end is known, and the checks compare every kept
call in the final mix with its source, sample for sample. The bed ducks 12 dB under
the actual speech (12 ms attack, 200 ms release), the ending 6 dB.
Gun sounds start on each recorded burst; the replay's missile blast is advanced
six frames to meet its fireball and rings on over the paused frame. The boot is
voiced with the retail cockpit switch and button recordings. A transparent
look-ahead limiter at -3.6 dBFS precedes one linear gain to -14 LUFS, leaving
true peaks near -2 dBTP before AAC and Opus encoding.

The probe does not record crew voice or the live situation music selector. The
score bed, visual clocks, HUD magnification and tape treatment are editorial.
No damage, weapons, AI steering or flight-model behavior is modified. The
published renders also used three renderer fixes that are not on main yet:
MiG-21 surface visibility, a runway texture depth tie and shared vertex-position
invariance. They are planned with the multiplayer work. Until they land, a
rerender can show the MiG-21's underside through its upper wing and flickering
runway paint.

## Validation status

See the [measured production baseline](../../docs/baselines/reel-production.md)
for the current results, the evidence files and the platform and listening
limits. Earlier cuts are preserved locally in `out/reel/previous*/`.
