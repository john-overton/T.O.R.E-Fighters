# Sliding cockpit and zoom, Linux, 2026-09-14

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Measured evidence, research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature; see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


Replaces the perspective-plane presentation in directional-cockpit.md at the
user's request. Reviewed ignored reference files:
`engine/src/flight/RetailCockpit.ts::cockpitLook` and `cockpit-layout.ts`.
Reference presentation slides 1.2% viewport width per yaw degree and 1.5% height
per pitch degree, fading at 45–65 degrees yaw / 35–55 degrees pitch. These are
reference-authored rules, not recovered retail projection.

The cockpit and HUD stay pointed aircraft-forward. At centered look the HUD
datum is at screen center for every zoom. A shared
translation follows the aircraft-forward datum projected with the world camera's
60-degree focal length and zoom. Looking right moves both left, looking left
moves both right, and looking up moves both down. Translation is never clamped
to spare image width: that clamp made the artwork appear to follow the camera.
The entire raster stays flat, preserving a level lower edge during horizontal
look. Reference fade thresholds remain, with no forward art in rear/up views.

Zoom uses the HUD center as its anchor. Below 1x, cockpit art and mirrors are
hidden, while HUD glyphs still scale once with zoom. Instruments remain
screen-anchored. This follows the [requested zoom rules](../spec/cockpit-zoom.md). This fitted flat presentation does not establish conformal
full-field HUD targeting or native cockpit geometry. Mirrors now show live rear views; rear/up interior is unavailable.

Validation: 122 Rust tests, 11 Python tests, locked build, formatting and Clippy
with warnings denied. Synthetic layout checks cover wide/tall/ultrawide sizes,
0.5/1/2/4 zoom, both pan directions, forward-datum displacement without clamping and rear/up fading.
Creator, terrain viewer, Hornet, rear-view and camera-panel GPU smoke tests pass
on NVIDIA RTX 4070 / Vulkan. Rafale live captures at logical 1280×720 and 640×900
cover yaw 30 degrees with 0.5/1/2 zoom and overhead look. Reviewed captures show
level lower framing, scaled HUD and no perspective trapezoid. Local evidence:
`.local/cockpit-slide/` (ignored). No Windows/macOS checks on this host.

Reproduce with `--free-flight --aircraft rafale --flight-look 30,0
--flight-zoom 2 --window-size 1280x720` (one command line). `--flight-zoom` accepts
finite 0.5–4 values; +/- adjusts it interactively and F1 resets to 1.

Latest validation repeats wide/tall captures for left/right 30-degree look and
20-degree upward look, plus creator/viewer GPU smoke tests and workspace checks.

Implementation update, 2026-09-17: the manual at local
`/home/john/Downloads/fa-manual_compress.pdf`, printed p. 104, documents view
magnification but does not establish the requested below-1x cockpit rule. That
rule and the corrected center anchor are labeled user-requested presentation,
not measured retail behavior. Synthetic layout checks verify a stable center
and proportional scale at 0.5/0.75/1/2/4x across 4:3, widescreen and portrait views.

The current 960x720 GPU captures at 0.5x, 1x and 2x were inspected. At 0.5x the
cockpit/mirrors are absent and the HUD is centered. At 1x the cockpit returns;
at 2x the magnified HUD retains its center. Captures are ignored local evidence:
`.local/zoom-half.png`, `.local/zoom-one.png`, `.local/zoom-two.png`.
The required `cargo run --locked -p tore-app -- --smoke-test` passed on NVIDIA
RTX 4070/Vulkan. All AGENTS checks passed: formatting, warnings-denied Clippy,
470 Rust tests, workspace build, 40 Python tests, source/binary asset guards and
documentation headers. Windows/macOS execution and retail comparison were not
available; there were no unavailable required Linux checks.
