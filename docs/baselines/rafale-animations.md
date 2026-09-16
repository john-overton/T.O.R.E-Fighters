# Rafale cockpit and moving parts — 2026-09-14

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


## Cockpit selection correction

`CockpitRenderer::prepare` returned early whenever any cockpit binding existed.
Starting directly with `--aircraft rafale` therefore looked correct, but selecting
Rafale after the default Hornet retained the Hornet cockpit. Preparation now
replaces the art texture/binding on aircraft selection. The original `~RAFH.PIC`
and `~F18H.PIC` remain external; no new cockpit artwork was authored.

A real Quick Mission keyboard run selected Rafale from an initial Hornet, flew,
returned with Ctrl-Q, selected Hornet and flew again. Compositor captures confirm
the different original cockpit frames in both directions. H in the Rafale shows
“Hook unavailable for this aircraft” without toggling the device or playing a
control sound. A nonzero hook inspection fraction also fails explicitly.

## Source parts and fitted presentation

Static enumeration of all 64 combinations of RAF.SH's six reviewed state guards
is preserved locally in `.local/rafale-animation/poses.json`. Source identity and
hashes remain those recorded in [the import baseline](rafale-quick-mission.md).
The runtime validates CODE length, state words, atlas references and moving-part
counts before applying its separate Rafale rig.

| Source evidence | Presentation |
| --- | --- |
| 0x5b50 adds 16 exhaust faces | Scale original flames from the nozzle plane; darken baked hot nozzle faces when off |
| 0x5b56 adds four airbrake faces | Two original dorsal panels rotate about fitted forward edges |
| 0x5b62 selects gear-open belly and three gear/door assemblies | Three-second actuator travel, fitted folding axes, doors open during first quarter |
| 0x5b6e/74 select left/right flap branches | Retain neutral original trailing panels and apply fitted flap/pitch/differential-roll mixing |
| 0x5b7a selects rudder branch | Rotate the original two-sided trailing rudder; fixed fin stays intact |
| Canards at 0x3d26/3d3d and 0x3d81/3d98; `_PLcanardPos` import | Original paired polygons respond to pitch about fitted spanwise hinges |

Nonzero flap branches contain native arithmetic beyond the bounded reader's
reviewed grammar; the rig deliberately transforms the neutral polygons. It does
not execute or translate that unreviewed native code. Device endpoints, textures,
UVs and normal conventions are retained. Visual rotations never feed back into
authoritative 120 Hz flight state; existing presentation interpolation applies.

The retail Rafale module imports `_PLafterBurner`, `_PLbrake`, `_PLcanardPos`,
`_PLgearDown`, `_PLgearPos`, `_PLleftFlap`, `_PLrightFlap` and `_PLrudder`, but no
`_PLhook`. Its generic HUD's HOOK text is not evidence of a supported hook in this
model. This finding concerns the game asset, not real-world emergency equipment.

## Reproduction and validation

```sh
cargo run --locked -p tore-app -- --quick-mission
cargo run --locked -p tore-app -- --aircraft rafale --flight-view 1 --flight-look 120,-30 --flight-devices 1,1,1,0,1 --flight-controls 1,0,1 --capture-flight .local/rafale-open.ppm
TORE_PERF_FRAMES=330 TORE_PERF_ACTIVE=1 TORE_PERF_VIEWS=1 cargo run --locked -p tore-app -- --free-flight --aircraft rafale --no-audio --window-size 1280x720
```

Linux/Wayland, RTX 4070 Vulkan, Rust 1.91.1, locked development build:

- Formatting, Clippy with warnings denied, workspace build, 115 Rust tests and
  nine Python tests passed. Source and both debug binary asset guards passed.
- Synthetic tests check Rafale device endpoint preservation, reversible partial
  poses, UV/color preservation, unit normals, rigid control-surface motion in both
  directions, unchanged body geometry and unavailable hook state.
- Creator, viewer, Hornet and Rafale window smoke tests passed. GPU captures cover
  closed/half/open gear and exhaust, control deflections, and a camera instrument.
- Live cockpit captures cover 1280×720 and 640×900 logical windows, plus actual
  Hornet → Rafale → Hornet selection. Full-canvas cockpit/HUD and edge instruments
  remain aligned. Screenshots and command logs are ignored under
  `.local/rafale-animation/`.
- Bounded active run: 330 frames, first 30 excluded, zero paused frames; CPU frame
  interval mean 16.78 ms, p95 16.91 ms, maximum 32.90 ms. Mean simulation/cameras
  0.14 ms, UI composition 0.61 ms, submission/presentation 0.57 ms. This run had
  zero completed live camera readbacks. These are CPU wall intervals including
  presentation backpressure, not GPU timestamps or verified displayed FPS.

## Remaining parity

Hinges, deflection limits, control mixing, exhaust transition and dark nozzle
material are fitted. The original closed/open belly topology switches at the
start/end of gear motion; native continuous door/well behavior is unverified.
The source's planar gear textures and low-polygon mesh are preserved, without new
volumetric wheels or wells. Native lighting/material fidelity, wheel spin,
suspension, steering, nozzle petals, canopy operation, exact native animation
schedules, native HUD composition and whole-tick dynamics remain open. Full
rear/overhead cockpit coverage is also unavailable in the recovered forward art.

## Shared simulation integration

Before pushing, remote commit `51efbcc` introduced the shared `tore-sim` kernel,
independent aircraft laws and typed model configurations. The merge preserves
those changes and the separate app-side Rafale rig. Hook availability now derives
from the selected shared model instead of duplicating a mutable capability flag.
Animation regression tests remain in the app and use shared simulation state.

The combined tree passed 122 Rust tests, 11 Python tests, formatting, Clippy with
warnings denied, locked build and source/both debug binary guards. Both portable
`--validate-flight` extraction workflows passed all 13 scenarios per aircraft.
Creator, viewer, both aircraft, device-pose and camera-panel GPU checks passed;
Quick Mission cockpit switching in both directions was also exercised with
`--researched-flight` enabled. The merged 330-frame active benchmark measured
mean CPU frame interval 16.78 ms, p95 16.86 ms and max 32.29 ms, with zero paused
frames and zero camera readbacks. These remain CPU wall timings, not displayed
FPS or native flight parity. Logs are local in `.local/merge-review/` and
`.local/rafale-animation/`. Windows/macOS checks were not rerun on this Linux host.
