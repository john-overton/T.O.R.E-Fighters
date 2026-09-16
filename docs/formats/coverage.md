# Importer coverage

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Research notes — research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Requirements, gates and remaining
> work described here are research-mode scope; they are not acceptance gates
> for gameplay. Parity is measured by expression of feature — see
> [AGENTS.md](../../AGENTS.md). Player-visible behaviour is specified in
> [docs/spec/](../spec/).


Status is for this **Rust rebuild**, not the reference project's decoders. The available Fighters Anthology installation is an aggregate source; title-by-title disc coverage is not established. All other title columns remain not started until independently inventoried.

| Format | Fighters Anthology | Scope/evidence |
| --- | --- | --- |
| LIB / EALIB | Decoded for supplied installation | All 7,520 unique resources from five archives extracted; no cross-title validation yet |
| DCL | Partial | All 7,372 raw-literal entries extracted; 22 menu resources match reference output; coded literals unimplemented |
| PIC / embedded PAL | Partial | Menu/creator backgrounds, action pieces, glyph strips, theater maps/variable numbered terrain textures and SKY0 rendered; malformed-input bounds checks |
| Standalone PAL | Decoded | Aircraft palette and cockpit overlays; viewer uses recovered LAY palette data |
| DLG | Partial | CHOOSEAC runtime; bounded import/relocation geometry inspection validated on 26 creator/ordnance/selector dialogs; runtime text regions and callbacks remain open |
| MNU | Partial | Bounded FMENUD tree used in flight; QM_MENU and ARMPLANE hierarchy/shortcuts validated with shared reader and inspection example; setup handlers/visibility and other editions unported |
| 5K / 11K | Partial | 99 recorded music tracks, lossless WAV previews, PT-selected engine/AB/start/stop and actuator samples including speed brakes; bounded PCM8 mono reader, authored mixer gains |
| FNT | Partial | Bounded bitmap-writing grammar; WIN11 instrument/menu and HUD11 flight fonts rendered |
| ESA | Not started | Loose LIB installation used |
| LAY / PL weather | Partial | Bounded records across 24 supplied modules; typed callback imports and persistent seeded fog updates; corrected altitude-haze tint. Live tint/smoothing and ordered indexed haze remaps; original celestial/cloud geometry decoded and rendered. Special horizon, glare and retail acceptance remain open. [Evidence](../baselines/weather-foundation.md) |
| MUS | Partial | All nine FA score grammars parsed; NORMAL drives recorded free-flight phrases, native host events/priority deferred; four missing PCM references reported |
| XMI / instrument banks | Excluded from current playback scope | User selected original recordings without MIDI/synthesis; general raw extraction remains available |
| PT / PTS / SH / HUD | Partial | FA F18 PT fields, Hornet static SH/device geometry and cockpit artwork; PTS and complete native HUD/shape VM remain unimplemented |
| T2 / BIT2 | Partial | All 16 grids parsed; native packed layout, heights and lookup verified; All 16 base theaters render as fixed-triangle previews (Kurile has no tmap textures) |
| JT / SEE / ECM | Partial | Named schemas, dependency closure and 135 JT definitions extracted; combat/sensor execution not complete |
| OT | Partial | Bounded STRIP/166 metadata and explicit main shape reference; other classes, placement, callbacks and runtime absent |
| NT | Not started | Directory inventory only |
| M / MM | Partial | All 75 selected MM layouts plus named mission environment/tmap fields decoded; isolated eight-field STRIP placement inputs decoded separately; missions and object execution absent |
| MT / campaigns / saves / Pro Mission Creator | Not started | Raw Ukraine resources preserved; runtime remains absent |
| CB8 / VDO / FBC / INF | Not started | No video/reference playback |

No M1a completion or all-title format validation is implied. See [theater recovery](theater.md), [menu extraction](menu.md) and [baseline](../baselines/main-menu.md).

Initial weapons audit (before implementation, 2026-09-14): 135 JT and 170
dependencies extracted, but shared native effect roots were missing. All 70 literal JT references
across 145 PTs are present; PTS/compatibility and ordnance/sensor behavior remain
open. No format status is promoted by this research. [Plan](weapons.md) and
[evidence](../baselines/weapons-research.md).

### F/A-18D slice

FA PT: typed bounded reader for the reviewed F18/660 layout, all source G rows and hardpoints exported; runtime physics is an authored adapter. FA JT/SEE/ECM: named schema decoding and raw data extraction, including transitive shape/texture/audio dependencies; weapon/sensor execution is partial for the ten F18/Rafale default slots, including contact ECM and supported automatic equipment faults. GAS: checked tank configuration and raw preservation. SH: nearest-detail static Hornet geometry and observed device endpoint branches, not a general native VM. FNT: bounded bitmap-writing glyph grammar, WIN11 used in instrument windows. HUD: associated source artwork/data preserved, general native HUD composition not decoded. [Detailed scope](aircraft.md).

## Native flight research

The separate `--native-flight` extraction mode inventories PE32/i386 FA.EXE and
FA.SMS statically. It writes local symbol spans, hashes and reviewed-build PT
references. Pure Rust flight helper translations have synthetic checks and an
imported-Hornet report; they are not a complete native simulation. See
[native flight coverage](native-flight.md) and [validation](../baselines/native-flight.md).

Native flight second pass: 18 reviewed static regions, partial instance-state map, typed PT component profiles, departure timers/severity and spin branch, drag assembly, landing classifier, scalar velocity and movement-angle stages. Static-only and diagnostic-only; complete force/movement/contact/clock integration remains open. See [native flight coverage](native-flight.md).

Native flight third pass: 28 reviewed regions plus the inert 321-word trig table; angle conversion/body-rate transform, lift/gravity/vector thrust, weight/drag loading and position/wind now have diagnostic Rust translations. Matrix/display composition, complete contacts and whole-tick scheduling remain open.

Native-flight fourth pass adds bounded 514-word atan table extraction and
43 reviewed static regions. Matrix/cockpit composition, contact arithmetic/latch,
resolved equipment mass, loaded control bounds and clock/RNG helpers are diagnostic
translations. Terrain/carrier query producers and whole-tick ordering remain open;
see [native-flight.md](native-flight.md).

Fifth-pass native research: 52 reviewed regions with incoming entry references;
translated landing nearest-object selection/distance, ground query flags,
signed-word RNG reseeding/chance and object-due comparison. Collision geometry,
queue rescheduling, remaining state producers and whole-tick parity stay open.


Rafale C follow-up: reviewed `RAFALE.PT`/660 identity now has typed BRF/envelope
coverage alongside F18; the shared per-aircraft resolver imports its 95-resource
loose-media closure. RAF.SH neutral geometry, original atlas/cockpit and source
equipment/audio are consumed in selectable free flight. Other Rafale variants,
native Rafale animation laws and whole native dynamics remain unimplemented;
a reviewed-part presentation rig is now available. This is
not general SH/HUD or full aircraft-system parity. [Scope](aircraft.md#rafale-c-import-and-runtime-selection--2026-09-14).

The shared BRF aircraft reader now reviews F18.PT and RAFALE.PT (Rafale C), both
FA plane type 5/size 660, with named extraction reports and transitive dependencies.
`--validate-flight` runs identical hybrid flight acceptance on either identity.
Other Rafale variants remain rejected; RAF.SH is preserved but does not receive
F18-specific animation assumptions. See [shared model](../FLIGHT-MODEL.md) and
[acceptance evidence](../baselines/shared-flight-model.md).

Cockpit mirrors: reviewed F18/RAF high-resolution PIC flat fills supply three
four-connected runtime silhouettes per aircraft. Bounded exact-color extraction
preserves the source rims and rejects empty/overlapping/oversized regions. Live
GPU rear views are fitted presentation, not decoded native mirror optics.
[Validation](../baselines/mirrors.md).

## Combat import and diagnostic components — 2026-09-14

All 135 JT, 51 SEE, 30 ECM and 4 GAS configurations now have bounded typed reads.
The shared app/CLI resolver retains reviewed shared effect roots, both aircraft's
PTS modules and explicit dependency/provider evidence. PTS module semantics,
generated effects and native callbacks remain open; preservation is not decoding.
The headless combat module implements isolated native movement, trigger/ammunition,
loading and sensor gates. Complete firing/guidance/damage/rendering remain open.
[Implementation and acceptance evidence](../baselines/combat-components.md).

## Connected development combat

The two reviewed aircraft now use their actual PT/JT stations in `--live-fire`:
trigger/ammo/spawn, source movement helpers, approximate guidance/contacts/damage,
live instrument readouts, static missile geometry and sampled original explosion
art. The corresponding native lifecycle/SH VM/sensor parity cells remain partial.
[Validation and limits](../baselines/live-fire.md).

Manual weapons follow-up: ten PT-default JT stations pass all five damage-class
fixtures. Runtime now consumes VIS340/F18R acquisition data, source category
mapping and failed-station flags, with carried weapon body geometry and bounded
combat-service tapes. Native sensor/SH/damage parity remains partial; alternative
catalog loadouts and non-default ordnance are not enabled by this acceptance.
[Evidence and non-AI gaps](../baselines/manual-weapons.md).

Manual systems follow-up: PT/ECM source data now feeds bounded player damage,
automatic hardpoint/sensor fault handling and ECM contact probability for F18/
Rafale. [Evidence](../baselines/weapons-systems.md). Format/lifecycle parity remains
partial: no decoy lifecycle, complete subsystem dispatch or alternate loadouts.

Menu research follow-up: bounded five-entry ordnance action dispatch plus aligned
creator/default/input and ordnance fuel/quantity/compatibility spans.
[Contracts and evidence](../baselines/menu-behavior-mapping.md). These are static
research outputs, not a runtime dialog interpreter or accepted loadout flow.

## Creator runtime import — 2026-09-14

The shared `ui::creator` reader verifies the reviewed FA.EXE SHA-256 before
reading inert active selector lists. The app stores only bounded `TOREQM01`
option data (33 fields and 16 target lists, exact cardinalities), never executable
bytes. Synthetic tests cover truncation, trailing data, bounds and SHA-256 vectors.
The shared `--creator` extraction profile includes PT/JT metadata, weapon thumbnails
and creator/ordnance UI resources. Full dynamic aircraft eligibility, auxiliary
store catalogs and other executable builds remain open.

`Hardpoint.location` exposes the bounded source byte used for station headings.
`tore-sim::combat::loadout` validates supported station compatibility, ammunition,
fuel and weight before constructing live combat state. See
[implementation evidence](../baselines/creator-ordnance.md).

## Weather and aircraft-effect review — 2026-09-15

The [weather audit](../baselines/weather-review.md) corrects the earlier static-midday
coverage description and reviews the 11 local weather commits. SH streamer
definitions are read for both aircraft; a separate static import/re-entry inspector
checks aircraft-embedded device code without executing it. General SH control flow
and game-wide absence of contrails/broader vapor are not established. Raw source
creator options are retained; the editor omits the duplicate overcast label.

Weather LAY: bounded root `+0x6c` shade headers/index remaps and tint reduction
fields decoded; named sky/ocean deck resources shared by CLI/app. See
[weather](weather.md) for ordered ray composition and remaining horizon gaps.

Weather SH: bounded straight-line point/circle/billboard grammar added separately
from aircraft projection. LAY sun background remap decoded at root `+0x50`.

Weather clouds: hash-gated FA EXE descriptor/call reader, bounded inert cache,
shared sky/ocean/cloud extraction fixtures and CLOUD1 sheet consumer. Imported
code is never executed. CLOUDS primitives decode without a global placement claim.

Weather continuation: normal full-detail horizon bands and above-sky selection,
shared indexed aircraft fog/palette and indexed cockpit selective tint now render.
Cockpit prefix and exterior no-palette contracts are validated at import. Later continuation entries below supersede its horizon/HUD gaps;
retail comparisons remain open. See
[weather contract](weather.md#horizon-and-shared-aircraft-palettes--continuation-2026-09-15).

Weather continuation: bounded `TORE_FLARE_V1` import recovers the nine reviewed
FA circle descriptors; LAY remaps 265/266 drive lens-flare composition. Source
whitening order/gates and moon world basis are implemented. GPU color-index
reconstruction and circle projection remain adaptations; see weather.md.

Cloud placement now covers detail 0/1 versus 2, source heading-sector relocation,
SH bounds and signed coordinate-range rejection. Native generic frustum work
rejection uses GPU clipping; CLOUDS.SH placement remains unverified after the
resource/producer audit recorded in weather.md.

SH static projection now preserves opcode CA fog control, including conditional
weather flag 0x40 suppression. Horizon coverage includes original indexed
deck/ground transitions and lower solid size/inversion gates; analytic GPU
projection remains distinct from native integer scanline acceptance.


LAY light-map counts/pointers and SH polygon normal flags now feed the reviewed
solid/texture light-before-fog path. Source signed dot and row boundaries are
tested; float animated orientation remains an adaptation. See weather.md.


HUD CODE+0x72 primary color is bounded by the native copied root; both supported
aircraft select cockpit index 40. Source brightness applies before sun whitening
and outside fog tint ranges. Remaining HUD geometry is authored; see weather.md.


Weather textures now retain single indexed texel/cutout sampling; native sky
intermediate resolution/scanlines are adapted to GPU rays. Ordinary map-0 rows
are identity in all reviewed LAYs. INFO2 override and alternate CP maps are
identified display consumers, deferred with their actual views. See weather.md.

2026-09-15 presentation follow-up adds no binary grammar: original LAY/SH/PIC
coverage is unchanged. Fractional-time palette/altitude interpolation and spatial
shade blending are authored GPU presentation, with stepped diagnostics retained.
Sun/moon use a common fitted ×4 projection calibration against user-supplied
retail default-zoom captures; native projection acceptance remains open.
See [smoothing and size evidence](../baselines/weather-smoothing.md).

CE attachment semantics corrected (2026-09-15): right/up/forward vectors and
right/forward heading hinges, verified against native consumers and exact
F18/Rafale neutral mesh vertices. No new aircraft or format coverage is implied.
[Evidence](../baselines/wind-turbulence-vapor.md).

## Land-contact foundation — 2026-09-15

Static FA slices, tested preference/cache helpers and vertical cell geometry/angle projection;
a bounded shape F2 contact-offset reader and external square-root table reader.
No runtime contact support or complete collision-record grammar. Selected UKR/STRIP/RUNWAY resources
are extracted with provenance; OT schema, placement/callback/shape closure and
collision acceptance remain open. [Foundation](../baselines/native-land-foundation.md),
[geometry evidence](../baselines/native-land-geometry.md).

Candidate-angle and requested-heading projection arithmetic is translated/tested;
world dispatcher/cache activation remains open. [Evidence](../baselines/native-land-angles.md).

STRIP precursor: bounded F2 contact-box list and native signed midpoint arithmetic
are tested. Mission conversion/add-callback ordering is source-established only;
full OT/placement/callback runtime and drawing closure remain open.
[Contract](native-strip.md), [evidence](../baselines/native-strip.md).
