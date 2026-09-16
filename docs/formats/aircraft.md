# Aircraft recovery and free-flight adapters

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Research notes, research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Requirements, gates and remaining
> work described here are research-mode scope; they are not acceptance gates
> for gameplay. Parity is measured by expression of feature, see
> [AGENTS.md](../../AGENTS.md). Player-visible behaviour is specified in
> [docs/spec/](../spec/).


The 2026-09-13 port uses the supplied Fighters Anthology `F18.PT`, not the distinct `F18C.PT`. Its retail name is F/A-18D. The reference checkout's `docs/aircraft-porting.md`, `formats/{pt,jt,sh,hud,instrument-windows}.md` and bounded Python readers were research guides. No TypeScript runtime, terrain engine, converted reference bundle, external font or retail bytes are required by the Rust application or extractor.

This is a development port with recovered data and a playable adapter, **not completed native flight, cockpit, instrument or weapon parity**. In particular, extracting executable modules does not implement all their behavior. The user clarified that 1:1 instrument rendering means the separate RWR/radar/systems/target windows; cockpit-frame resolution is a different question.

The subsequent shared-model pass also reviews `RAFALE.PT` (Rafale C) for
extraction and headless flight. `--aircraft rafale --validate-flight` reuses the
same reader and simulator; Rafale visual/cockpit animation acceptance remains
separate. The original flight description below describes the legacy adapter;
[FLIGHT-MODEL](../FLIGHT-MODEL.md) documents the selectable hybrid model.

Current combat behavior is documented in [weapons/systems integration](../baselines/weapons-systems.md)
and the preceding [manual weapons pass](../baselines/manual-weapons.md).
It supersedes the first-pass unimplemented weapon/instrument states below;
ordinary free flight stays externally clean while the explicit range loads PT defaults.

## Import contract

`tools/extract_assets.py --aircraft f18` invokes the native extractor and shared dependency resolver. It starts from PT/HUD/shape and cockpit variants, includes instrument artwork/fonts, then follows available resource references through PT, JT, SEE, ECM, GAS, SH and HUD. PT's default weapon/sensor/tank references and their shape/texture/audio dependencies are included automatically. `--weapons` additionally starts from every available JT. Combining a theater with aircraft/weapons selects their **union**; `--include` subsequently narrows it. Directory discovery skips non-EALIB installer libraries; exclude the unrelated LHX archives as documented in EXTRACTION.

Source hierarchy and archive boundaries are retained. The script's report includes archive/output hashes, offsets, decoded sizes, named PT/object/flight fields, all G polygon points, hardpoints with unresolved fields explicitly raw, and named JT/SEE/ECM fields. GAS and remaining dependencies are preserved as original decompressed files. A complete extraction report establishes extraction success, not runtime or 1:1 acceptance. Dependency discovery decompresses selected metadata even for `--list`/`--dry-run`, without writing output. Missing referenced BRF shapes/stores/samples fail instead of disappearing silently.

The app calls the same resolver against FA_1/FA_2, builds its runtime cache, and validates the PT and instrument font. Older caches missing the aircraft or actuator audio are re-imported automatically when local media is available. The whole aircraft dataset remains external to the binary. The HUD also names `~F18_W`, which is absent from the supplied loose PIC catalog; it is recorded as an unresolved optional reference, not replaced with invented art.

## PT and equipment

BRF is a bounded textual data language: byte/word/dword, caret-marked scaled integers, pointers, symbols, strings and labels. Rust validates statement kinds and counts before applying the recovered schema. Hex words are sign-folded; bytes are retained unsigned. Unsupported aircraft identities/layouts, bad pointers, malformed polygons and mass bounds fail. Original fields, including unknown semantics, remain in raw files and export reports.

| F18.PT fact | Retail value / use |
| --- | --- |
| Object type / type size | Plane 5 / 660; reviewed FA layout |
| Empty weight / internal fuel / MTOW | 23,050 / 11,220 / 49,224 lb |
| Engines / total military / AB thrust | 2 / 17,687 / 32,000 lbf |
| Envelope rows | -4 through +9 G; speeds ft/s, altitude ft |
| Engine clips | `&JET1N.11K`, `&JET1A.11K` |
| Start/stop clips | `&POWERUP.5K`, `&POWERDN.5K` |
| Hardpoints | 9, including equipment and gun |
| Visual / radar / countermeasures | VIS340.SEE / F18R.SEE (APG-65) / F18.ECM |
| Internal gun | M61.JT, 570 source rounds; connected manual live fire |
| Default external stores, preserved | F150.GAS, AIM120.JT, AGM65G.JT, AIM9M.JT |

The retail ECM labels itself ALQ-161; the importer preserves that identity rather than replacing it with a real-world specification. Station compatibility masks, weight classes, coordinates, weapon timing and several sensor fields still have unresolved units/semantics. The clean free-flight configuration omits all external stores and their mass. The original default loadout remains available in extracted data for the later loadout page.

Flight runs at 120 fixed ticks/second using f64 state. A renderer-independent adapter intersects the PT polygons at altitude, derives control authority and loading penalties, consumes source fuel, and integrates thrust, speed, bank/pitch, gravity and position. Drag normalization against the 1G upper envelope, throttle/controller response, lapse, and actuator timing are authored. This does not reproduce the original native force/control helpers. Crash freezes the flight; runway support, landing, stall/spin parity and a full six-degree-of-freedom model remain open. Display-rate determinism is tested on this host, not certified across CPU architectures.

## Exterior and device recovery

For component origins, texture state, LODs, damage/shadow resources and practical
editing constraints, see [objects and shapes](objects-and-shapes.md). Its local
F18C examples are research references, not an alias for the supported F18.PT.

SH is an inert PL/PE module containing an interpreted drawing program plus native re-entry blocks. The Rust projector bounds sections, shared vertex slots, calls, scopes, polygons, textures, state guards and instruction counts. It recognizes limited static re-entry patterns; it never OS-loads or executes x86. Unknown records fail. Nearest-detail and neutral/static device poses are the supported scope, not a complete VM or animation system.

F18.SH resolves to 287 source polygons in the reference neutral projection, including five unconfigured decal polygons. Rust omits those transparent decal faces and retains 282. Source vertices use X/right, Y/forward, Z/up; world presentation uses X/right, Y/up, Z/north. The source atlas is **256×644**, retained without forced square resampling. UVs use texel centers and reversed source V. Keyed texture-only faces discard index 255; filled textured faces blend paint over recovered base colors. Stored normals select visible faces. Terrain and aircraft share GPU depth, with a one-foot near plane for nearby geometry.

Static branch comparison in this exact F18.SH established these geometry differences:

| State word | Observed branch | Difference from neutral |
| --- | --- | --- |
| 0x7900 | Crossed rear flame faces | +8 source polygons |
| 0x790c | Upper rear airbrake | +2 |
| 0x7912 | Gear struts/wheels/doors | +24 |
| 0x791e | Lower rear hook | +2 |

Sixteen combined endpoint poses are decoded at load. The runtime requires the reviewed 26,934-byte CODE layout and observed guard words before applying this device mapping; different layouts fail for review. The renderer now animates reviewed original parts continuously from the fully deployed source pose; three-second device travel, folding hinges and door sequencing are authored. See [animation evidence](../baselines/f18-animations.md). Exact native timing/continuous hinges and control-surface schedules remain unported. Other guard words are not assigned guessed meanings. Scale is a provisional uniform one-third foot per model coordinate, giving a 56-foot neutral length. Native scale and real-world dimension acceptance remain open; no independent axes are stretched. The baked hot nozzle faces use an authored dark material when the exhaust effect is off.

## Cockpit, fonts and instrument windows

`F18.HUD` has a 699-byte CODE data section at RVA 0x1000; its strings name the cockpit variants, HUDSYM, WINFONT and GEAR/FLAP/BRAKE/HOOK. The current loader uses its associated high-resolution artwork; it does not implement every HUD record/caller. `~F18H.PIC` is 1280×490, side mirror-mask overlays are 117×200 and the center mirror-mask overlay is 323×84; these are not side/rear cockpit views. Forward art is shown with original span transparency. The cockpit world viewport now covers the whole flight canvas. Forward art uniformly covers the actual window aspect (cropped only as needed); there is no lower PANEL fill. Instrument windows overlay the scene independently. This replaces the rejected provisional 640×245 world viewport. The presentation mapping remains authored pending full native HUD caller recovery. Mirror silhouettes are now bounded runtime floods of the original flat fills, compositing an every-frame rear GPU view with fitted optics; separate overlay/gauge composition and native HUD symbology remain open; the current authored flight HUD uses HUD11.FNT and renderer-consistent pitch/bank projection.

The bare HUDSYM name resolves to mode-specific `HUDSYM00/01/11.FNT` resources, not a HUDSYM.PIC. All three are preserved for the remaining native HUD symbol mapping. This was caught by strict required-art validation; guessing an extension from the string alone was incorrect.

FNT glyph routines are decoded as a strictly bounded bitmap-writing grammar (stores, row advance, cursor advance, return). Unknown instructions and out-of-cell writes fail; this is not a general x86 emulator. WIN11 is used for instrument text after comparing mode fonts. WIN01's narrow double-height appearance was rejected in visual review; WIN00 was too small for the supplied references.

The supplied `rwr-50nm.png`, `systems.png` and `target-view.png` are the current visual references. Each window has a **160×156 raster**, with a **138×114 content surface**, blue-gray chrome and four pale square buttons. These outer coordinates/buttons are fitted to the supplied screenshots, not claimed native geometry. FA.EXE's RWR code around 0x43ed2f/0x43ee0f uses base extents 0x39/0x45 shifted by video-mode globals; 114/138 corresponds to doubling those extents. Full caller/video-mode validation remains open. This supersedes reusing the reference app's authored 200×230/176×170 window.

| Page | Current input/behavior | Remaining parity |
| --- | --- | --- |
| 1 Envelope | Actual PT G polygons and current speed/altitude marker | Native row selection, comparison, exact plot |
| 2 Front View | GPU terrain view from ownship, rasterized at 138×114 | Native projection, cockpit/mirror variants |
| 3 Other View | GPU exterior view of imported ownship | Native capture/follow modes |
| 4 Radar/Visual | NO TARGET in target-free flight | Target acquisition, zoom, damage/skill overlays |
| 5 RWR | Range buttons, axes/rings, ownship, powered JAM state | Threat receiver, native RCS outline, detection/history |
| 6 Nav | Heading, MSL altitude; no waypoint in free flight | Mission waypoints, ETA and native navigation |
| 7 Systems | Live throttle and remaining internal fuel; external zero | TEMP/OIL/HYD intentionally `---` until model/threshold recovery |
| 8 Weapons | Selected source weapon, ammo/readiness, range payload, player HP and visual/radar/ECM fault state; ordinary flight remains externally clean | Alternate loadouts, full subsystem dispatch and native display parity |
| 9 Radar | Power, range capped by APG-65 source search range, mode/grid | Search/track/seeker physics, contacts, authentic mode logic |

No fake targets, threat diamonds or healthy-system percentages are inserted to resemble the screenshots. Scope symbology, colors, layout and controls are presently a visual adapter; original engine/native system behavior is not claimed. Four large or six small windows can be open, with Shift-0..9 toggles; page 0 is an explicit RCS placeholder. Camera windows request updates at 10 Hz using bounded asynchronous GPU readback; the last completed raster remains visible while pending. Offline captures wait explicitly. Direct GPU panel composition remains future optimization work. Hover remains silent and button activation requires matching press/release.

## Next parity gates

- Trace and test the native flight helpers against this FA build, including loading, devices, negative G, fuel timing, sound speed and crash/landing.
- Recover full HUD/window layout and native draw rounding; compare equivalent states against the supplied captures and actual retail flight.
- Recover temperature/oil/hydraulic/system health and map damage, annunciators and failures without fabricated values.
- Implement radar/RWR/seeker/contact state, camera target tracking, waypoints and weapon execution.
- Recover remaining SH control surfaces, continuous device poses, scale, shadow, damage/LOD and mirror views.
- Run maneuver, visual and audible acceptance on all supported platforms. See the current [baseline](../baselines/f18-free-flight.md).


## Full-canvas cockpit and desktop follow-up

See [control reference](../FLIGHT-CONTROLS.md) for recovered/menu/manual versus development bindings and [baseline](../baselines/cockpit-controls.md) for checks. The aircraft extraction profile now requires `FMENUD.MNU` and `HUD11.FNT`, and preserves the HUD00/01/11 family alongside HUDSYM00/01/11. The app validates the additional font cache requirement and re-imports older caches when local media is available. Native executable HUD callers and symbol-glyph mapping remain separate recovery tasks.

## Momentum and vertical-flight follow-up

The authored flight adapter now integrates a separate world velocity from thrust, drag, lift and gravity. Control rates settle over 0.2 seconds for roll and 0.1 seconds for pitch; load-factor response retains its 0.25-second filter. A fitted 0.7/s alignment response now tracks a bounded load/speed-dependent AoA target rather than zero nose/velocity separation; body-yaw transport also accounts for banked turn geometry. See [banked-pull evidence and unresolved PT fields](../baselines/banked-pull-aoa.md). These are development response parameters, not newly decoded PT fields or native flight equations. Source thrust/fuel/envelopes/drag inputs remain in use.

Attitude updates use Rodrigues rotation of an orthonormal body basis; yaw/pitch/bank are derived presentation coordinates, not singular Euler-rate integration. Canonical pitch may still read ±90 degrees while heading/bank change through vertical, but the physical orientation continues. Rendering interpolates basis vectors and restores orthonormality rather than blending Euler angles across that change. Velocity integration supplies true vertical speed and lateral/vertical flight-path-marker displacement. Stall/spin parity, angle-of-attack lift curves, inertia tensors, control moments and native force-law recovery remain open; this is not a complete six-degree-of-freedom implementation.

Cockpit and HUD share a body-fixed forward-plane projection; the source does not supply complete rear/up geometry. See [directional rendering](../baselines/directional-cockpit.md). [Validation](../baselines/flight-response-sky.md).

### Native flight consumer research

The [FA native flight pass](native-flight.md) now traces the consumers of
`gpullAOA`, `lowAOASpeed`, and `lowAOAPitch`, alongside control, envelope, power and
fuel helpers. This supersedes their earlier wholly unresolved status. Pure Rust
translations and repeatable probes exist; the full native tick and its integration
remain open. Visual device animation remains separate from native physics schedules.

## Rafale C import and runtime selection, 2026-09-14

The shared `AircraftId` catalog now admits reviewed FA `F18.PT` and `RAFALE.PT`
identities with the same bounded 660-byte BRF schema. `RAFALE.PT` names itself
RAFALE / Rafale C and references `RAF.SH`; `RAFALEF`, `RAFALEE` and `F18C` remain
separate, unsupported identities. CLI and app use the same per-aircraft dependency
closure; the app imports their union. `--aircraft rafale` is accepted by both the
app and cross-platform extraction script.

Rafale facts: 17,100 lb empty, 9,900 lb internal fuel, 40,100 lb maximum takeoff
weight, two engines, 24,000 lbf military / 32,000 lbf afterburner thrust, 14 G
rows and nine hardpoints. Its source includes DEFA.JT with 250 rounds,
`&JET1N.11K` and `&JET6A.11K`. F18R.SEE and F18.ECM are explicit references in
this Rafale PT, not substituted Hornet metadata. External stations stay clean;
the source default stores are preserved for later loadout work.

`RAF.SH` decodes to 206 neutral faces with `_RAF.PIC` (256×457); its reviewed CODE
length is 19,334 and state-word set is 0x5b50/56/62/6e/74/7a. The six guards select
afterburner, brake, gear, left/right flap and rudder branches respectively.
A separate Rafale rig now animates original gear/doors, airbrakes, trailing
elevons, canards, rudder and exhaust. Hinges, control mixing and actuator timing
are fitted, not translated native laws; the one-third-foot scale remains
provisional. The reader retains neutral flap/rudder polygons because nonzero
branches contain unreviewed native arithmetic. It never executes that code.
Gear uses the original closed/open belly branches, switching at the start/end
of travel; native continuous door/well topology remains unverified.
`RAF.SH` imports `_PLcanardPos` and the corresponding device globals, but no
`_PLhook`. H is unavailable for this imported model (not a claim about physical
emergency equipment on real aircraft). `~RAFH.PIC` supplies the forward cockpit, using
the existing body-fixed art/HUD projection and screen-anchored instruments.
Full rear/overhead interior and native HUD composition remain unavailable.

The 95-resource standalone profile extraction succeeded with provenance hashes;
all selected bytes remain external. Runtime selection refreshes the aircraft
atlas, cockpit, camera previews, instruments and initial state. Native helper
reports can consume Rafale fields; whole-tick parity remains gated by
[native flight acceptance](native-flight.md). Native dynamics, exact native animation laws,
combat/loadout and complete systems are not complete. See the
[cockpit-switch and animation follow-up](../baselines/rafale-animations.md). [Validation](../baselines/rafale-quick-mission.md).

Mirror source seeds, safeguards, optics and acceptance evidence are recorded in [live mirrors](../baselines/mirrors.md). Native mirror-camera recovery remains open.


### HUD primary color (2026-09-15)

The bounded HUD reader consumes CODE+0x72 after requiring the native 0x2b2-byte
copied root. F18.HUD and RAFALE.HUD both select private cockpit palette index 40.
The app shares weather/sunlight/brightness palette resolution with cockpit art;
HUD geometry remains authored. Source brightness and palette ordering are in
[weather.md](weather.md#hud-palette-consumer-continuation-2026-09-15).

## End-to-end aircraft guide

The [aircraft import and acceptance guide](../aircraft-import.md) joins extraction,
existing flight/presentation/systems coverage and all per-aircraft acceptance
gates. F-14, A-4E and X-31 are scheduled after the flight-response slice; they
are not supported identities yet.
