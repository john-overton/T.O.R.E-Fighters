# Importer coverage

Status is for this **Rust rebuild**, not the reference project's decoders. The available Fighters Anthology installation is an aggregate source; title-by-title disc coverage is not established. All other title columns remain not started until independently inventoried.

| Format | Fighters Anthology | Scope/evidence |
| --- | --- | --- |
| LIB / EALIB | Decoded for supplied installation | All 7,520 unique resources from five archives extracted; no cross-title validation yet |
| DCL | Partial | All 7,372 raw-literal entries extracted; 22 menu resources match reference output; coded literals unimplemented |
| PIC / embedded PAL | Partial | Menu/creator backgrounds, action pieces, glyph strips, theater maps/variable numbered terrain textures and SKY0 rendered; malformed-input bounds checks |
| Standalone PAL | Decoded | Aircraft palette and cockpit overlays; viewer uses recovered LAY palette data |
| DLG | Partial | CHOOSEAC rectangle and eight action labels/positions recovered at runtime |
| MNU | Partial | Bounded FA FMENUD sibling/child tree, labels and accelerators decoded and used by in-flight menu; native handlers/flags and other editions unported |
| 5K / 11K | Partial | Menu effects/music plus PT-selected engine/AB/start/stop and actuator samples; authored mixer scheduling |
| FNT | Partial | Bounded bitmap-writing grammar; WIN11 instrument/menu and HUD11 flight fonts rendered |
| ESA | Not started | Loose LIB installation used |
| LAY / PL weather | Partial | Bounded CODE/RVA palette reader and native ramp mapping; fixed DAY2 keyframe, no runtime interpolation |
| XMI / MUS / instrument banks | Not started | Directory inventory only; PCM preview does not count as synthesis |
| PT / PTS / SH / HUD | Partial | FA F18 PT fields, Hornet static SH/device geometry and cockpit artwork; PTS and complete native HUD/shape VM remain unimplemented |
| T2 / BIT2 | Partial | All 16 grids parsed; native packed layout, heights and lookup verified; All 16 base theaters render as fixed-triangle previews (Kurile has no tmap textures) |
| JT / SEE / ECM | Partial | Named schemas, dependency closure and 135 JT definitions extracted; combat/sensor execution not complete |
| OT / NT | Not started | Directory inventory only |
| M / MM | Partial | All 75 selected MM layouts plus named mission environment/tmap fields decoded; missions and object execution absent |
| MT / campaigns / saves / Pro Mission Creator | Not started | Raw Ukraine resources preserved; runtime remains absent |
| CB8 / VDO / FBC / INF | Not started | No video/reference playback |

No M1a completion or all-title format validation is implied. See [theater recovery](theater.md), [menu extraction](menu.md) and [baseline](../baselines/main-menu.md).

### F/A-18D slice

FA PT: typed bounded reader for the reviewed F18/660 layout, all source G rows and hardpoints exported; runtime physics is an authored adapter. FA JT/SEE/ECM: named schema decoding and raw data extraction, including transitive shape/texture/audio dependencies; weapon/sensor execution remains partial/unimplemented. GAS: raw preservation and BRF validation. SH: nearest-detail static Hornet geometry and observed device endpoint branches, not a general native VM. FNT: bounded bitmap-writing glyph grammar, WIN11 used in instrument windows. HUD: associated source artwork/data preserved, general native HUD composition not decoded. [Detailed scope](aircraft.md).

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
