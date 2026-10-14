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

Every PT record's identity and headline fields, for all 145 aircraft types in the
supplied installation, are inventoried in [the catalog](fa-catalog.md). Catalog
presence is not flyable support.

Current combat behavior is documented in [weapons/systems integration](../baselines/weapons-systems.md)
and the preceding [manual weapons pass](../baselines/manual-weapons.md).
It supersedes the first-pass unimplemented weapon/instrument states below;
ordinary free flight stays externally clean while the explicit range loads PT defaults.

## Import contract

`tools/extract_assets.py --aircraft f18` invokes the native extractor and shared dependency resolver. It starts from PT/HUD/shape and cockpit variants, includes the cockpit family's instrument window frame and fonts, then follows available resource references through PT, JT, SEE, ECM, GAS, SH and HUD. PT's default weapon/sensor/tank references and their shape/texture/audio dependencies are included automatically. `--weapons` additionally starts from every available JT. Combining a theater with aircraft/weapons selects their **union**; `--include` subsequently narrows it. Directory discovery skips non-EALIB installer libraries; exclude the unrelated LHX archives as documented in EXTRACTION.

Source hierarchy and archive boundaries are retained. The script's report includes archive/output hashes, offsets, decoded sizes, named PT/object/flight fields, all G polygon points, hardpoints with unresolved fields explicitly raw, and named JT/SEE/ECM fields. GAS and remaining dependencies are preserved as original decompressed files. A complete extraction report establishes extraction success, not runtime or 1:1 acceptance. Dependency discovery decompresses selected metadata even for `--list`/`--dry-run`, without writing output. Missing referenced BRF shapes/stores/samples fail instead of disappearing silently.

The app calls the same resolver against FA_1/FA_2, builds its runtime cache, and validates the PT and instrument font. Older caches missing the aircraft or actuator audio are re-imported automatically when local media is available. The whole aircraft dataset remains external to the binary. The HUD also names `~F18_W`, which is absent from the supplied loose PIC catalog; it is recorded as an unresolved optional reference, not replaced with invented art.

## PT and equipment

BRF is a bounded textual data language: byte/word/dword, caret-marked scaled integers, pointers, symbols, strings and labels. Rust validates statement kinds and counts before applying the recovered schema. Hex words are sign-folded; bytes are retained unsigned. Unsupported aircraft identities/layouts, bad pointers, malformed polygons and mass bounds fail. Original fields, including unknown semantics, remain in raw files and export reports. `Aircraft::parse` then applies the reviewed record corrections (`aircraft_corrections.rs`: the V-22, CH-47, Su-25, MiG-17F and C-130, see [aircraft records](../spec/aircraft-records.md)) only while every replaced value still holds its copied retail number. The imported files are never changed.

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

Sixteen combined endpoint poses are decoded at load. The runtime requires the reviewed 26,934-byte CODE layout and observed guard words before applying this device mapping; different layouts fail for review. The renderer now animates reviewed original parts continuously from the fully deployed source pose; three-second device travel, folding hinges and door sequencing are authored. See [animation evidence](../baselines/f18-animations.md). Exact native timing/continuous hinges and control-surface schedules remain unported. Other guard words are not assigned guessed meanings. Scale is a uniform one-third foot per model coordinate, giving a 56-foot neutral length. Retail draws every shape, aircraft included, at `2^(e-8)` feet per unit, about three times real size; the third is the fitted factor that brings it to real size, and placed buildings and surface units share it since 2026-10-10 ([placed object scale](objects-and-shapes.md#placed-object-scale-2026-10-10)). No independent axes are stretched. The baked hot nozzle faces use an authored dark material when the exhaust effect is off.

## Cockpit, fonts and instrument windows

`F18.HUD` has a 699-byte CODE data section at RVA 0x1000; its strings name the cockpit variants, HUDSYM, WINFONT and GEAR/FLAP/BRAKE/HOOK. The current loader uses its associated high-resolution artwork; it does not implement every HUD record/caller. `~F18H.PIC` is 1280×490, side mirror-mask overlays are 117×200 and the center mirror-mask overlay is 323×84; these are not side/rear cockpit views. Forward art is shown with original span transparency. The cockpit world viewport now covers the whole flight canvas. Forward art uniformly covers the actual window aspect (cropped only as needed); there is no lower PANEL fill. Instrument windows overlay the scene independently. This replaces the rejected provisional 640×245 world viewport. The presentation mapping remains authored pending full native HUD caller recovery. Mirror silhouettes are now bounded runtime floods of the original flat fills, compositing an every-frame rear GPU view with fitted optics; separate overlay/gauge composition and native HUD symbology remain open; the current authored flight HUD uses HUD11.FNT and renderer-consistent pitch/bank projection.

The bare HUDSYM name resolves to mode-specific `HUDSYM00/01/11.FNT` resources, not a HUDSYM.PIC. All three are preserved for the remaining native HUD symbol mapping. This was caught by strict required-art validation; guessing an extension from the string alone was incorrect.

FNT glyph routines are decoded as a strictly bounded bitmap-writing grammar (stores, row advance, cursor advance, return). Unknown instructions and out-of-cell writes fail; this is not a general x86 emulator. WIN11 is used for instrument text after comparing mode fonts. WIN01's narrow double-height appearance was rejected in visual review; WIN00 was too small for the supplied references.

Each window is a **162×160 raster**: the aircraft's `~<cockpit>_P.PIC` frame at double size, with a **138×114 screen** at (12, 20). The frame, text colours and geometry come from the [instrument bezel spec](../spec/instrument-bezel.md); this replaced the earlier blue-gray procedural chrome fitted to `rwr-50nm.png`, `systems.png` and `target-view.png`. FA.EXE's RWR code around 0x43ed2f/0x43ee0f uses base extents 0x39/0x45 shifted by video-mode globals; 114/138 corresponds to doubling those extents. Full caller/video-mode validation remains open. This supersedes reusing the reference app's authored 200×230/176×170 window.

| Page | Current input/behavior | Remaining parity |
| --- | --- | --- |
| 1 Envelope | Filled PT G bands, current/all/locked-target comparison modes, live readouts and cycling square, scaled per aircraft | Exact retail palette, scaling and animation cadence remain fitted; [behavior spec](../spec/envelope.md) |
| 2 Front View | GPU terrain view from ownship, rasterized at 138×114, with an opinionated short horizon bar, flight path marker and TAS/MSL overlay in the HUD color | Native projection, cockpit/mirror variants |
| 3 Other View | GPU exterior view of imported ownship | Native capture/follow modes |
| 4 Radar/Visual | NO TARGET in target-free flight | Target acquisition, zoom, damage/skill overlays |
| 5 RWR | Range buttons, axes/rings, ownship, powered JAM state | Threat receiver, native RCS outline, detection/history |
| 6 Nav | Heading, MSL altitude; no waypoint in free flight | Mission waypoints, ETA and native navigation |
| 7 Systems | Live throttle, damage-driven temperature/oil/hydraulics, internal and external fuel | Exact retail rates and gauge palette remain fitted; [systems behavior](../spec/systems-damage.md) |
| 8 Weapons | Selected source weapon, ammo/readiness, range payload, player HP and visual/radar/ECM fault state; ordinary flight remains externally clean | Alternate loadouts, full subsystem dispatch and native display parity |
| 9 Radar | Shared sensor component: automatic RWS/TWS or IR label, recovered range ladder, contacts with heading tails, selection and acquired-track markers, stale plots, history dots and directional jammer noise | Native scan animation, false contacts, air-to-ground and HARM channels |

No fake targets or threat diamonds are inserted to resemble the screenshots. Healthy system percentages now come from the explicit ownship systems model. Scope symbology, colors, layout and controls are presently a visual adapter; original engine/native system behavior is not claimed. Four large or six small windows can be open, with Shift-0..9 toggles; page 0 draws the authored exposure contour, received emitter symbols and view scale described in the [sensor component guide](../radar.md). Camera windows request updates at 10 Hz using bounded asynchronous GPU readback; the last completed raster remains visible while pending. Offline captures wait explicitly. Direct GPU panel composition remains future optimization work. Hover remains silent and button activation requires matching press/release.

## Next parity gates

- Trace and test the native flight helpers against this FA build, including loading, devices, negative G, fuel timing, sound speed and crash/landing.
- Recover full HUD/window layout and native draw rounding; compare equivalent states against the supplied captures and actual retail flight.
- Refine the documented fitted thermal, fluid and failure rates against original evidence; [systems model](../spec/systems-damage.md).
- Implement RWR threat reception, camera target tracking, waypoints and remaining weapon execution. Radar, infrared and contact state now come from the [shared sensor component](../radar.md).
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
It also reads the instrument window frame name, a NUL-terminated string in the
13 bytes at +0x275 (`~f4_p` in F4.HUD; empty is allowed, an unterminated or
non-printable name is rejected), and three cockpit palette indices: +0x2a8 for
the window title and number, +0x2a9 for button letters and +0x2aa for the button
press square. For every flyable aircraft the frame is `~<cockpit stem>_P.PIC`;
loading checks that the HUD names exactly that picture. Per-aircraft values are
in the [instrument bezel spec](../spec/instrument-bezel.md).
The app shares weather/sunlight/brightness palette resolution with cockpit art;
HUD geometry remains authored. Source brightness and palette ordering are in
[weather.md](weather.md#hud-palette-consumer-continuation-2026-09-15).

## End-to-end aircraft guide

The [aircraft import and acceptance guide](../aircraft-import.md) joins extraction,
existing flight/presentation/systems coverage and all per-aircraft acceptance
gates. F-14D, A-4E and X-31 EFM now have initial FA-only ports; see
[acceptance](../baselines/aircraft-fa-expansion.md). Their roots contain the same
219 typed fields as the existing profiles. Type sizes are identity-specific:
F14.PT 636, A4E.PT 612 and F31.PT 660. The short/long source names are checked
before admitting these variants. F31's null hudName is accepted as an existing
nullable pointer grammar; its player HUD resource is F31.HUD. A4E uses F4.HUD
and the F4 cockpit family. F14's PT controller HUD reference is F14CC.HUD;
its selected player presentation uses F14.HUD and the F14 cockpit family.
The F14 radar is F14R.SEE; A4E uses F4BR.SEE; F31 uses F18R.SEE.
All three use their own PT-selected sound and station references.

## Roster expansion

All seven new roots use the existing 219-field BRF grammar. Require exact
short/long names and PT identity, not a filename alias. Review evidence and
hashes are in [roster acceptance](../baselines/aircraft-roster-expansion.md).

| PT | Type size | Player HUD | Cockpit family | Radar | Gun |
| --- | ---: | --- | --- | --- | --- |
| MIG29.PT | 636 | SU33CC.HUD | SU33 | MIG29R.SEE | GSH301.JT |
| SU27.PT | 660 | AV8.HUD | AV8 | SU27R.SEE | GSH301.JT |
| MIG21.PT | 612 | MIG21.HUD | M21 | MIG21R.SEE | GSH23.JT |
| SU25.PT | 660 | SU33CC.HUD | SU33 | SU24R.SEE | GSH301.JT |
| MIG23.PT | 660 | SU33CC.HUD | SU33 | MIG27R.SEE | GSH6_30.JT |
| SU35.PT | 660 | SU35.HUD | SU35 | SU27R.SEE | GSH301.JT |
| F22.PT | 636 | F22.HUD | F22 | F22R.SEE | M61.JT |
| F22N.PT | 636 | F22N.HUD | F22 | F22R.SEE | M61.JT |

MiG-29, Su-27, Su-25 and MiG-23 lack PTS resources in the reviewed base media.
PTS is inert and unused by flight or the PT-backed loadout editor. Its absence
is allowed only for these reviewed identities. Their PT HUD pointers select
the shared cockpit families above. SU35.PT has a null HUD pointer; SU35.HUD is
an explicit player-presentation choice, documented in the behavior spec.
F22 and M21 cockpit families have no mirror overlay resources; SU33 has side
overlays but no center overlay, and since 2026-10-12 those side overlays are
live mirrors like the others (see the [mirror review](../baselines/mirrors.md#2026-10-12-review-of-every-cockpit-picture)). Missing required flight/cockpit resources still
fail import or loading. VIS240 joins VIS340 as a reviewed source visual sensor
for the new default stations. Default guns/stores join the existing manually
controlled service and loadout allowlist, not a claim of all-JT support.

## F-4 variant comparison

Research, 2026-10-05, for the [variety batch](../spec/aircraft-variety.md).
Compared the four decompressed PT records from the catalog extraction using
`tools/catalog_fa.py` and the shared Rust field schema. The installed
`FA_2.LIB` SHA-256 matches the [catalog source identity](fa-catalog.md).
This is a data comparison, not a flight test or retail trajectory comparison.

| PT | Empty lb | Internal fuel lb | Maximum takeoff lb | Military thrust lbf | Afterburner thrust lbf |
| --- | ---: | ---: | ---: | ---: | ---: |
| F4B.PT | 30,328 | 12,290 | 61,795 | 23,260 | 35,800 |
| F4J.PT | 30,328 | 12,290 | 61,795 | 23,260 | 35,800 |
| F4E.PT | 31,853 | 12,290 | 51,000 | 23,620 | 35,800 |
| F4.PT, F-4G | 31,853 | 10,790 | 51,000 | 23,620 | 35,800 |

All four decoded G-envelope tables are identical, including every point in
every row from -4 through +7 G. B and J have identical decoded PLANE fields.
Their object records, stations and equipment are not thereby identical.
Against J, E/G differ in flags, rudder bounds and acceleration/deceleration,
spinExit, military thrust, afterburner consumption and maximum takeoff weight.
G additionally differs in internal fuel. E and G PLANE fields differ only in
internal fuel. The source afterburner consumption values are 19 for B/J and
27 for E/G; conversion to a player-visible burn rate needs the existing fuel
contract, not an assumed unit. Raw spinExit is 75 for B/J and -1 for E/G;
the resulting departure/recovery behavior needs review before model acceptance.

All four name F4.HUD. B and J reference F4J.SH; E references F4E.SH and G
references F4.SH. Default radar records are F4BR.SEE for B, F4JR.SEE for J,
and F18R.SEE for E/G. B has no default gun record; J names SUU16.JT, while
E/G name M61.JT. Do not infer an internal gun from the catalog's gun column:
it identifies default gun-class records, and station flags still need review.
G names AGM88.JT, E names AGM65G.JT. These are source references, not proof
that those weapons' complete behavior is supported. Full defaults stay in
the [catalog](fa-aircraft.csv).

Decompressed PT SHA-256 values:

| Resource | SHA-256 |
| --- | --- |
| F4B.PT | `40ee72698c9e2966bad34466038e3692c351f5ec4cd2c0c572012a043e6567d9` |
| F4J.PT | `3ebedc917d157403e11b99695f0e6d2845c5643ff388b5c93020648102f6bf13` |
| F4E.PT | `3c1ca544abd442fa20ceb82b16e0dfc4afc26ad5075a16bbfddda89321d8164c` |
| F4.PT | `bbd6812a392a222c46273c1dfc1de0235437dca6444b8d4546ee31e54360dd38` |

Open: full dependency closure, variant equipment/capability interpretation,
shape branches, fuel/departure behavior and flight validation. Next step:
review these differences against the hybrid configuration consumers and
write the numerical behavior contract before registering the family.

## Rotorcraft fuel records

Research, 2026-10-13, slice HF2, for John's request to find out why the Mi-24 has
18 minutes of fuel. Build: the supplied Fighters Anthology `FA_2.LIB` PT records,
read with the existing aircraft reader (`internalFuel` is a dword, `fuelConsumption`
and `aftFuelConsumption` are words; the object record's `weight` is the empty
weight). The reader's reading is **whole pounds a second for all the engines
together at full military throttle** (`fuelConsumption`), and the same at full
afterburner (`aftFuelConsumption`); the flight model debits that times the
throttle each second. The retail unit itself is not established (the native
`fuel_rate` returns the value in 24.8 fixed point and the burn is scheduled in
five-second steps, see [native flight](native-flight.md)); the reading is
checked below against real aircraft, not against retail play.

| PT | Engines | Empty lb | Internal fuel lb | `fuelConsumption` | `aftFuelConsumption` | Full tank at the record's flow |
| --- | --- | --- | --- | --- | --- | --- |
| AH64.PT | 2 | 18,298 | 2,000 | 1 | 0 | 33 min |
| V22.PT (as stored; corrected below) | 2 | 18,298 | 2,000 | 1 | 0 | 33 min |
| MI24.PT | 2 | 18,078 | 3,307 | 3 | 0 | 18 min |
| CH47.PT | 2 | 18,078 | 3,307 | 1 | 0 | 55 min |
| AV8.PT | 1 | 13,968 | 7,759 | 2 | 0 | 65 min |
| YAK141.PT | 1 | 25,685 | 9,700 | 2 | 17 | 81 min |
| F18.PT, F16C.PT (for scale) | 2, 1 | 23,050, 14,567 | 11,220, 6,972 | 2 | 16 | 93, 58 min |
| A10.PT, C130.PT, E2.PT (for scale) | 2, 4, 2 | 25,600, 76,469, 39,373 | 10,700, 45,012, 12,400 | 2, 1, 1 | 0 | 89, 750, 207 min |

**Findings.**

- **The weight is right, the flow is the outlier.** The Mi-24's 3,307 lb is 1,500 kg,
  the Mi-24's published internal fuel (1,500 kg, Air Force Technology; 1,720 kg
  elsewhere). Its 3 lb/s is 4,900 kg an hour: five to six times the real aircraft's
  cruise burn (about 800 to 1,000 kg an hour, an endurance of about two hours on
  internal fuel; the AH-64D's published combat endurance is 2.5 hours at about
  1,000 lb an hour). A tank cannot be 18 minutes of anything a Hind does.
- **The unit is not the mistake.** Read the same way the jets and transports are
  plausible: the A-10 (2 lb/s, 89 minutes at full military power), the C-130 (1 lb/s
  total) and the F/A-18 (afterburner 16 lb/s, against roughly 10 to 12 real). The integers
  are coarse (1, 2, 3) and set by hand, so the smallest, 1 lb/s, is a floor that no
  aircraft burning 0.3 to 0.6 lb/s can match, and the rotorcraft records are not
  tied to the power their rotors draw.
- **The rotorcraft records are copies.** `V22.PT` and `AH64.PT` carry the same
  empty weight, fuel, thrust and flow. `CH47.PT` carries the Mi-24's empty weight
  and fuel (the real CH-47 is a 24,000 lb aircraft carrying several thousand pounds more fuel; corrected below since slice HF3),
  an implausible thrust (135,795 lbf, set aside by the rotor overhaul) and a flow
  of 1 where the Mi-24 has 3. The flow of the Mi-24 and of the CH-47 therefore
  cannot both be a measurement of anything. Retail never makes a rotorcraft
  a fuel-limited aircraft: its flow had no player-visible consequence, which is
  how a placeholder survives.
- **The VTOL jets are consistent.** The AV-8 (7,759 lb, real about 7,500 lb) and the
  Yak-141 (9,700 lb) carry a fighter's flow and a believable tank; the models add the
  lift engines' burn (docs/spec/powered-lift-flight.md). No change.

**Decision (agent, 2026-10-13, John's request).** The **flow of a rotorcraft is not
read**: the flight model burns by the power the rotor draws
([FLIGHT-MODEL](../FLIGHT-MODEL.md#rotorcraft-fuel), fitted). The AH-64 and Mi-24
weights are kept (they size the rotor models and the mass checks). The measured
result is in the [rotorcraft fuel baseline](../baselines/rotorcraft-fuel.md).

**V-22 record correction (slice HF3, opinionated: John, 2026-10-13, "V-22 tank: fix
(real fuel)").** `V22.PT` is the AH-64 record with a new name: its empty weight,
fuel, maximum takeoff weight and thrust are the AH-64's. The reader now replaces
those four numbers, and only when every one of them still holds the copied value (an
edited or different record is read as it is). The table lives in
`crates/tore-formats/src/aircraft_corrections.rs` and applies inside
`Aircraft::parse`, so every reader (flight model, loadout, live combat, wreck) sees
the same aircraft. The choice of figures is an agent decision.

| Field | Record (AH-64 copy) | Corrected | Source |
| --- | --- | --- | --- |
| `weight` (empty), lb | 18,298 | 31,818 | MV-22B empty weight (Pub, Wikipedia "Bell Boeing V-22 Osprey", specifications) |
| `internalFuel`, lb | 2,000 | 11,700 | MV-22B 1,721 US gal (Pub, Bell Boeing V-22 brochure) at 6.8 lb/gal (JP-5) |
| `maxTakeoffWeight`, lb | 23,810 | 52,600 | Vertical takeoff maximum (Pub, Wikipedia) |
| `thrust`, lbf | 26,280 | 58,057 | The record's thrust-to-maximum-weight ratio kept (Derived) |

`CH47.PT` carries the Mi-24 record's empty weight, fuel and maximum weight (John's
answer of 2026-10-13: "check other records for copying issues as well"). The same
table corrects them to the CH-47F's; its own thrust (135,795 lbf, implausible and
already set aside by the rotor model) is not a copy and stays.

| Field | Record (Mi-24 copy) | Corrected | Source |
| --- | --- | --- | --- |
| `weight` (empty), lb | 18,078 | 24,578 | CH-47F empty weight (Pub, Wikipedia "Boeing CH-47 Chinook", specifications) |
| `internalFuel`, lb | 3,307 | 7,340 | CH-47F 1,080 US gal (Pub, Wikipedia) at 6.8 lb/gal (JP-5) |
| `maxTakeoffWeight`, lb | 28,660 | 54,000 | CH-47F gross weight (Pub, Wikipedia; the rotor model already scaled its power by it) |

Checked and left as they are (agent decision, 2026-10-13; no plain copy and no large
factor in the fields the game uses):

| PT | Record: empty / fuel / maximum, lb | Real | Verdict |
| --- | --- | --- | --- |
| MI24.PT | 18,078 / 3,307 / 28,660 | Mi-24V 8,500 kg (18,740 lb) empty, 1,500 to 1,720 kg fuel, 11,200 kg normal and 12,000 kg (26,455 lb) maximum takeoff | Right; the maximum is 8 percent high. Its thrust (1.67 times the gross weight) is set aside by the rotor model |
| AH64.PT | 18,298 / 2,000 / 23,810 | AH-64A about 11,400 lb empty, 375 US gal (about 2,500 lb) fuel, 21,000 to 23,000 lb maximum | The record's own (the V-22 copied it). The empty weight is about 1.6 times the real one: empty plus fuel, 20,298 lb, is a loaded Apache's mission weight. The rotor model's power comes from the record's thrust at this weight (within 10 percent of the real 2 x 1,690 shp), so the aircraft flies as a loaded AH-64 does; not changed, listed for John. The fuel is 20 percent under the real tank |

Sources disagree on the MV-22B's internal fuel (a fact sheet gives 1,448 gal with the
aft sponson tank); 1,721 gal is the manufacturer's brochure figure. The rotor model's
power scales with the corrected maximum weight, so it is the engines' full rating
([powered-lift flight](../spec/powered-lift-flight.md#mass-power-and-rotors)).
The fixed-wing records are slice nr-rec's audit.

## Fixed-wing record audit

Research, 2026-10-13, slice nr-rec, for John's request (opinionated, 2026-10-13)
to fix the Su-25 record and check every other record for values copied from
another aircraft. Build: the supplied Fighters Anthology `FA_2.LIB`, all 145 PT
records read in the schema order of `aircraft_schema.rs`. "Record" columns are
the retail values; "published" columns are rounded real-aircraft figures, from
each aircraft's Wikipedia specifications section unless a source is named.
The published figures of rows left as decoded were not individually re-checked
(agent's reference figures); the three corrected rows name their sources in
[aircraft records](../spec/aircraft-records.md). Thrust is all engines together,
military / afterburner. Top speed is the decoded 1 G row's fast edge.

Exact duplicates were found by comparing every PT's empty weight, internal fuel,
maximum takeoff weight, military and afterburner thrust and envelope rows with
every other PT. Variants of one aircraft (F18C, RAFALEE, F16E, MIG29M and the
like) sharing values are expected and not listed.

| PT (aircraft) | Empty lb, record / published | Internal fuel lb | Max takeoff lb | Thrust lbf | 1 G top kt | Finding |
| --- | --- | --- | --- | --- | --- | --- |
| SU25.PT (Su-25) | 41,885 / 21,605 | 22,664 / 6,614 | 90,390 / 42,549 | 35,270 / 0 against 19,860 | 604 / 526 at sea level | Mass, fuel, maximum weight, thrust and flow are SU24.PT's word for word. **Corrected.** Top speed 15 percent high and ceiling (23,812 ft, real 23,000) are its own: left. |
| MIG17F.PT (MiG-17F) | 6,000 / 8,640 | 4,534 / 2,520 | 13,380 / 13,380 | 5,732 / 7,451, matches | 622 / 590 at sea level | Fuel is MIG21.PT's (also J7E, Q5), 1.8 times real; flows 3 / 20 lb/s are 4.7 and 5.3 times the fleet median per pound of thrust. **Fuel and flows corrected.** Empty weight 31 percent low, not a copy: left. |
| C130.PT (C-130) | 76,469 / 75,800 (C-130H) | 45,012 / about 45,000 | 155,000 / 155,000 | 108,192 / 0; no static figure (4 x 4,590 shp) | 338 / 320 at 20,000 ft | Thrust 2.7 times AC130.PT's 40,000 for the same airframe and engines (thrust-to-weight near 0.9). **Corrected to 40,000.** |
| F18.PT (F/A-18D) | 23,050 / 23,000 | 11,220 / 10,400 to 10,900 | 49,224 / 51,900 | 17,687 / 32,000 against 22,000 / 35,800 | 1,033 / 1,030 (Mach 1.8) | Thrust pair shared with F16C (and F16, M2000, F29, CMCHE): a template, dry 20 percent low. Left. |
| RAFALE.PT (Rafale C) | 17,100 / 21,700 | 9,900 / 10,400 | 40,100 / 54,000 | 24,000 / 32,000 against 22,500 / 33,700 | 1,154 / 1,030 | Empty weight is M2000.PT's, 21 percent low; maximum weight 26 percent low. Left. |
| F14.PT (F-14D) | 40,104 / 43,700 | 15,741 / 16,200 | 74,349 / 74,350 | 28,435 / 41,800 against 27,600 / 55,600 | 1,345 / 1,340 | Burner 25 percent low. Left. |
| A4E.PT (A-4E) | 10,800 / 10,000 | 4,434 / 5,400 | 25,000 / 24,500 | 11,200 / 0 against 8,500 | 598 / 585 | Thrust is the A-4M's J52-P-408, 32 percent over the A-4E's J52-P-6A; not another record's. Left. |
| F31.PT (X-31 EFM) | 16,225 / 11,410 | 9,975 / 4,136 | 40,200 / 15,935 | 21,000 / 32,000 against 10,600 / 16,000 | 1,481 / about 735 (Mach 1.28) | About twice the demonstrator throughout, not a copy: FA's X-31 is a notional armed fighter. Left as game design. |
| MIG29.PT (Fulcrum-C) | 18,025 / 24,000 | 13,000 / 7,700 to 8,000 | 36,375 / 40,000 to 43,000 | 22,220 / 36,600, matches | 1,256 / 1,320 | Not a copy. Empty 25 percent low, fuel 1.6 times; empty plus fuel (31,025) is close to the real sum. Left. |
| SU27.PT (Su-27) | 36,000 / 36,100 | 19,000 / 20,700 | 60,000 / 67,100 | 40,755 / 59,510 against 33,800 / 55,100 | 1,465 / 1,350 | Dry thrust 21 percent high, shared with SU33, SU34, SU35 (one engine family). Left. |
| MIG23.PT (Flogger-B) | 23,589 / 22,500 | 11,704 / 10,000 | 41,556 / 39,000 | 17,310 / 24,728 against 18,300 / 27,600 | 1,351 / 1,350 | Within 15 percent. Left. |
| SU35.PT (Su-27M) | 40,564 / 37,500 | 22,000 / 22,600 | 74,956 / 74,960 | 40,755 / 58,642 | 1,347 / 1,350 | Within 10 percent. Left. |
| F22.PT, F22N.PT (F-22) | 30,000 / 43,340 | 25,000 / 18,000 | 72,000 / 83,500 | 64,000 / 78,000 against 52,000 / 70,000 | 1,315 / 1,300 | 1990s pre-production figures, not a copy (F22N is the same design in retail): empty 31 percent low, fuel 39 percent high, their sum within 10 percent. Left. |
| AC130.PT (AC-130U) | 76,469 / C-130H airframe | 45,012 | 155,000 / 155,000 | 40,000 | 338, fitted 261 | Weights are C130.PT's: the same airframe. Left. |
| E3.PT (E-3) | 169,552 / 185,000 | 157,839 / about 155,000 | 332,500 / 347,000 | 84,000 / 84,000 (4 x TF33) | 462, fitted VMO | Mass shared with IL76, E8, KC135, P3, ATL (a four-engine template); within 10 percent. Ceiling 30,000 against 41,000. Left. |
| IL76.PT (A-50 Mainstay) | 169,552 / 204,000 (Il-76MD) | 157,839 / not checked | 332,500 / 418,900 | 84,000 against 105,800 (4 x D-30KP) | 462 / 460 | E3.PT's mass and thrust word for word, 20 to 25 percent low in proportion, so thrust-to-weight is right. Left. |
| E2.PT (E-2C) | 39,373 / 40,200 | 12,400 / 12,400 | 53,267 / 53,000 | 16,000 (2 x 5,100 shp) | 326 / 350 | Matches. |
| AV8.PT (AV-8B) | 13,968 / 13,968 | 7,759 / 7,759 | 31,000 / 31,000 | 33,800 / 0 against 23,500 | 581 / 585 | Mass exact; thrust 44 percent high, offset in the hover by the fitted 0.75 vertical efficiency ([powered lift](../spec/powered-lift-flight.md)). Left. |
| YAK141.PT (Yak-141) | 25,685 / 25,684 | 9,700 / 9,700 | 42,990 / 42,990 | 19,840 / 34,170, burner matches | 972 / 975 | Matches. |
| F4B.PT, F4J.PT (F-4B, F-4J) | 30,328 / 28,000 to 30,800 | 12,290 / 13,000 | 61,795 / 56,000 | 23,260 / 35,800 against 23,740 / 35,800 | 1,268 / 1,260 | Within 10 percent. Ceiling 42,000 against 56,000 to 60,000. Left. |
| F4E.PT (F-4E) | 31,853 / 30,328 | 12,290 / 12,900 | 51,000 / 61,795 | 23,620 / 35,800 | 1,268 | The F-4E's published empty and maximum weights sit in F4B/F4J; within 20 percent. Left. |
| F4.PT (F-4G) | 31,853 / about 30,300 | 10,790 / about 12,900 | 51,000 | 23,620 / 35,800 | 1,268 | Fuel is F8J.PT's, 16 percent low. Left. |
| A7.PT (A-7E) | 19,127 / 19,127 | 9,900 / 9,300 to 10,200 | 42,000 / 42,000 | 15,000 / 15,000 | 604 / 600 | Matches. |
| F15.PT (F-15C) | 28,600 / 28,000 | 13,662 / 13,450 | 68,000 / 68,000 | 32,558 / 47,860 against 29,200 / 47,500 | 1,434 / 1,435 | Envelope ceiling 100,000 against a 65,000 ft service ceiling. Left. |
| F16C.PT (F-16C) | 14,567 / 18,900 (Block 50) | 6,972 / 7,000 | 33,000 / 42,300 | 17,687 / 32,000 against 17,155 / 29,500 | 1,145 / 1,150 | Mass is F16.PT's (F-16A): empty 23 and maximum 22 percent low for a Block 50. Left. |
| F104.PT (F-104) | 14,082 / 14,000 | 4,924 / 5,800 | 28,779 / 29,000 | 12,177 / 17,900 against 10,000 / 15,600 | 1,256 / 1,150 | Envelope tops at +5 G against +7.33. Left. |
| A10.PT (A-10A) | 25,600 / 24,960 | 10,700 / 11,000 | 50,000 / 50,000 | 18,130 / 18,130 | 450 / 381 (450 is Vne) | Envelope ceiling 22,812 against a 45,000 ft service ceiling. Left. |
| B747.PT (747) | 399,000 / 394,000 | 331,880 / 383,000 | 800,000 / 875,000 | 227,000 / 230,000 to 252,000 | fitted VMO | Within 15 percent (AF1 shares it). Left. |
| A310.PT (A310) | 157,975 / 176,000 | 92,240 / 108,000 | 313,050 / 313,000 (-200) | 96,000 / about 100,000 | fitted VMO | The A310-200's weights. Left. |

The fleet's fuel flows are consistent with one another, not with real engines:
over the 28 jets and transports above that were not corrected, the median is 1.12 lb/s of
military flow per 10,000 lbf of military thrust (the A-7, the F-4 family and the
F-104 are highest at 1.6 to 2.0; the C-130 lowest because of its thrust) and
5.0 lb/s of burner flow per 10,000 lbf of burner thrust (4.4 to 7.6). The SU24 copy
in SU25.PT (1.70) and MIG17F.PT (5.2 and 26.8) were the outliers.

**Rotorcraft** (owned by slice nr-hf3, recorded here, not changed by this audit):
AH64.PT and V22.PT are one template also used by AH1, GAZ, LYNX, TIGRE, UH60 and
KA50 (18,298 / 2,000 / 23,810 lb, 26,280 lbf, the same envelope); a real AH-64A is
about 11,400 lb empty, a real V-22 about 33,100 lb empty with about 12,000 lb of
fuel. MI24.PT, CH47.PT, MI17.PT and SFR.PT share 18,078 / 3,307 / 28,660 lb and one
envelope: right for the Mi-24 (8,500 kg empty, 1,500 kg fuel), not for a CH-47
(about 23,400 lb empty, 7,000 lb fuel, 50,000 lb maximum); CH47's 135,795 lbf is
SFR.PT's. Slice nr-hf3 corrects the V-22 and CH-47 records (see the record
corrections above). See also [rotorcraft fuel records](#rotorcraft-fuel-records).

**Not audited.** A PT carries no dimensions; the size comes from the shape, whose
coordinate scale differs between files, so sizes need the renderer's host scale
and are left for a separate pass. Hit points, damage and sensor fields were not
compared with real figures.

## F-14 default external fuel

Research, 2026-10-05, for [selectable tanks](../spec/external-fuel-tanks.md).
The catalog's F14.PT hardpoint at zero-based index 5 names F250.GAS and has
`maxItems = 2`, which the Rust aircraft reader exposes as the station count.
It is one source station representing two tanks, not one tank. The source
GAS record decodes through the existing five-field tank grammar:

| Item | Source value |
| --- | ---: |
| Each F250 tank empty mass | 198 lb |
| Each F250 tank fuel capacity | 1,650 lb |
| Default count | 2 |
| Total external fuel under the current default-load rule | 3,300 lb |
| Total empty tank mass | 396 lb |
| Total full tank mass | 3,696 lb |

F14.PT SHA-256 is
`f2bcbcfa275147f7641356fb6d8c5bf6f3772b9131d0634ddba06400d6a861a1`.
F250.GAS SHA-256 is
`974e49de6eb22237bf7a1bd24b3ed9308300bbb95270a9c0fe7104892514f0de`.
The local `aircraft-fa/extraction-report.json` records F250.GAS from FA_2.LIB
at offset 7,233,849, 152 stored bytes and 205 decoded bytes; its output hash
matches the inspected file. F14.PT is from the catalog extraction with the
[catalog archive identity](fa-catalog.md).

These are source values and current load-rule arithmetic. They do not prove
the user's live starting state, original fill policy, shell jettison behavior,
or correct visual placement. Next: reproduce the loadout-to-flight path and
review tank placement and removal separately.

## Hook capability in original-game exports

The reviewed F22.PT PLANE_TYPE flags at assembled offset 0xba are 0x91. Bit
0x02 enables the original hook command. The reviewed F22N.PT flags are 0xd3:
the same base flags plus hook bit 0x02 and carrier capability bit 0x40. Since
2026-09-22 the concept exporter uses the F-22N donor and copies its flags
unchanged; it rejects a donor whose flags are not 0xd3. The earlier F-22A
export set only bit 0x02 to produce 0x93. The original hook command updates
deployed flag 0x400 in the aircraft instance; the shape-state producer maps it
to `_PLhook` 0 or 1. The [packaging baseline](../baselines/fa-xx-packaging.md)
records the inspected addresses and the exporter validation. This does not
alter T.O.R.E's flight adapters or establish complete carrier-operation support
for the export. F22N.PT otherwise differs from F22.PT in a handful of words
recorded in the packaging baseline; T.O.R.E takes them as donor values.
