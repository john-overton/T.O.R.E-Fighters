# Importer coverage

Status is for this **Rust rebuild**, not the reference project's decoders. The available Fighters Anthology installation is an aggregate source; title-by-title disc coverage is not established. All other title columns remain not started until independently inventoried.

| Format | Fighters Anthology | Scope/evidence |
| --- | --- | --- |
| LIB / EALIB | Decoded for supplied installation | All 7,520 unique resources from five archives extracted; no cross-title validation yet |
| DCL | Partial | All 7,372 raw-literal entries extracted; 22 menu resources match reference output; coded literals unimplemented |
| PIC / embedded PAL | Partial | Menu/creator backgrounds, action pieces, glyph strips, theater maps/variable numbered terrain textures and SKY0 rendered; malformed-input bounds checks |
| Standalone PAL | Preserved | Extracted with environment profile; viewer uses recovered LAY palette data |
| DLG | Partial | CHOOSEAC rectangle and eight action labels/positions recovered at runtime |
| MNU | Partial research only | Selected files extracted; labels inspected through reference decoder; runtime tree remains authored |
| 5K / 11K | Partial | Three menu effects and one optional music recording imported and played |
| FNT | Not started | PIC glyph strips serve the menu; compiled font resources not implemented |
| ESA | Not started | Loose LIB installation used |
| LAY / PL weather | Partial | Bounded CODE/RVA palette reader and native ramp mapping; fixed DAY2 keyframe, no runtime interpolation |
| XMI / MUS / instrument banks | Not started | Directory inventory only; PCM preview does not count as synthesis |
| PT / PTS / SH / HUD | Research / raw preservation | Sun/moon/stars/cloud SH and named PIC dependencies extracted; SH rendering and aircraft import remain unimplemented |
| T2 / BIT2 | Partial | All 16 grids parsed; native packed layout, heights and lookup verified; All 16 base theaters render as fixed-triangle previews (Kurile has no tmap textures) |
| OT / JT / NT | Not started | Directory inventory only |
| M / MM | Partial | All 75 selected MM layouts plus named mission environment/tmap fields decoded; missions and object execution absent |
| MT / campaigns / saves / Pro Mission Creator | Not started | Raw Ukraine resources preserved; runtime remains absent |
| CB8 / VDO / FBC / INF | Not started | No video/reference playback |

No M1a completion or all-title format validation is implied. See [theater recovery](theater.md), [menu extraction](menu.md) and [baseline](../baselines/main-menu.md).
