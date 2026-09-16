# Rafale C and briefing selectors, 2026-09-14

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


Host: Omarchy x86_64, Wayland, RTX 4070 Vulkan, Rust 1.91.1. Work starts from
`c7347b5b751f314114176eeb3380cf84f79b05c2` plus the local Linux setup/GPU fixes.
No commit or push was made. This is playable asset-import and menu acceptance,
not whole native flight-model acceptance.

## Source identity and extraction

The supplied FA `RAFALE.PT` identifies Rafale C and references `RAF.SH`. It is
distinct from RAFALEF/RAFALEE and from the already supported F18.PT/F/A-18D.
Decoded-resource SHA-256:

| Resource | SHA-256 |
| --- | --- |
| RAFALE.PT | `fca3e30c372b3cabffffa95d399c023da49d7ce21d21936d704703786f9eb5b2` |
| RAF.SH | `7da4f1d7e2a296022634567b37b0aebd697542a526ad0fb0e25fbe649b19e081` |
| RAFALE.HUD | `b555f9febf6041297393c8d92e9e70343b1c5c9439316b1c0c1ec0721fe39228` |
| _RAF.PIC | `7ff9f5b3aacb9a896b9297ba9557fe4f41ee1503465893f85d8c52f2a8ec463e` |

```sh
python3 tools/extract_assets.py --aircraft rafale --exclude-archive 'disc*/*' --out .local/rafale-import
```

Passed: 95 resources, zero errors (32 from FA_1.LIB, 63 from FA_2.LIB).
The Python entry point verified/reused the initial CLI extraction and added
archive/output SHA-256 provenance to `.local/rafale-import/extraction-report.json`.
The shared resolver also imports both aircraft into the runtime cache. Old
caches refresh automatically from local media. No retail inputs were modified.

Rafale loads 206 neutral faces, a 256×457 source atlas, 14 G rows, nine hardpoints,
its forward cockpit and mirror overlays, DEFA/250-round metadata and its source
engine clips. F18R.SEE and F18.ECM are explicit Rafale PT references. Source
default external stores are preserved in extraction; free flight keeps them off.
Shape layout/state-word checks prevent applying the Hornet's rig to Rafale data.

## Quick Mission

Visual target: `gameassets/reference-photos/quick-mission-creator-screen.jpg`.
The native QUIKMIS3 background, raster fonts and original button pieces remain.
The blue OK button uses ACTDFT0L/M/R; green Cancel uses ACTION0L/M/R.
Friendly/Enemy Situation columns replace the temporary map/Flight Setup layout.

Click Wing 1's aircraft name or the theater name in the flight briefing to select.
The Aircraft menu opens the same two-aircraft list. OK starts clean free flight.
Opponents and unsupported mission settings are dim text fields without hit areas.
Popup geometry, field fills, keyboard traversal and hover/press behavior are
authored to fit the photo, not claimed decoded DLG handlers. A source palette
and font do not establish exact native layout/interaction parity.

Selection refreshes the aircraft profile, GPU atlas/cockpit, pending camera
previews, instrument state and flight initialization. Mouse activation requires
matching press/release; keyboard navigation commits only on Enter. Escape cancels
a selector before leaving; cancelled screens do not retain an open popup.
Hover/focus is silent; aircraft/theater selection uses the original button cue.

## Validation

- Formatting, warnings-denied Clippy, locked workspace tests/build passed.
- 113 Rust tests passed, including Rafale identity/variant rejection, independent
  and combined dependency closures, missing-shape rejection, selector traversal,
  cancellation, disabled enemies and click/release isolation. Fixtures are synthetic.
- Nine Python tests passed. Source and both debug executable asset guards passed.
- Real creator, viewer, F18 flight and Rafale flight smoke tests passed on RTX 4070.
- Rafale exterior, cockpit and Other View camera-panel GPU captures were inspected;
  the weapons raster displays DEFA, 250 RDS, clean external stations, 30 chaff
  and 30 flares from imported metadata. No combat execution is implied.
- Live cockpit window checks at 1280×720 and 640×900 logical sizes retain the
  body-fixed cockpit/HUD and screen-edge instruments. The window closed with exit 0.
- Headless Rafale pull: 1,200 ticks, 285.942 knots, 8,380.516 ft, `crashed=false`.
- Complete-loop probes with a 10,800-tick budget: Rafale completes at 2,104 ticks,
  F18 at 2,590; both report vertical/inverted/completed and no crash. These exercise
  the unchanged authored adapter with different imported profiles, not native trajectories.
- The static native-helper report also accepts the Rafale PT; it remains diagnostic.

Local evidence lives in `.local/rafale-review/`: command logs, `quick.ppm`,
`aircraft-selector.ppm`, `theaters.ppm`, `cockpit.ppm`, `exterior.ppm`,
`camera-panel.ppm`, `weapons.ppm`, wide/tall compositor captures and PNG conversions.
Compositor captures include host opacity/decoration and are not native pixel baselines.

## Remaining work

Rafale exterior device/control animation is not recovered: it currently renders
the neutral SH pose, including its baked nozzle artwork, with the existing
provisional geometry scale and lighting limitations. The HUD's `~RAF_W` reference
is absent from the supplied PIC catalog; its raw HUD remains preserved and no
replacement is invented. Mirrors are still source fills and the forward cockpit
plane does not supply full rear/overhead geometry. Native dynamics, complete HUD
callers, systems, loadout, opponents and combat remain open. No Windows/macOS
run, audible source-parity comparison or sustained frame-time claim was made here.

Follow-up: the neutral-only exterior and cockpit-switch limitation are superseded
by [Rafale animation/cockpit checks](rafale-animations.md). Exact native behavior
remains open.
