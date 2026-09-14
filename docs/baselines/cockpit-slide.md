# Sliding cockpit and zoom — Linux, 2026-09-14

Replaces the perspective-plane presentation in directional-cockpit.md at the
user's request. Reviewed ignored reference files:
`engine/src/flight/RetailCockpit.ts::cockpitLook` and `cockpit-layout.ts`.
Reference presentation slides 1.2% viewport width per yaw degree and 1.5% height
per pitch degree, fading at 45–65 degrees yaw / 35–55 degrees pitch. These are
reference-authored rules, not recovered retail projection.

The cockpit and HUD stay pointed aircraft-forward, not screen-centered. A shared
translation follows the aircraft-forward datum projected with the world camera's
60-degree focal length and zoom. Looking right moves both left, looking left
moves both right, and looking up moves both down. Translation is never clamped
to spare image width: that clamp made the artwork appear to follow the camera.
The entire raster stays flat, preserving a level lower edge during horizontal
look. Reference fade thresholds remain, with no forward art in rear/up views.

Zoom-in crops about the eye line; zoom-out anchors the bottom and limits source
shrinking at viewport width. HUD glyphs scale once with zoom. Instruments remain
screen-anchored. This fitted flat presentation does not establish conformal
full-field HUD targeting or native cockpit geometry. Mirrors remain fills;
rear/up interior is unavailable.

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
