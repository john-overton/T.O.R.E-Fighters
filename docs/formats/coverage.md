# Importer coverage

Status is for this **Rust rebuild**, not the reference project's decoders. The available Fighters Anthology installation is an aggregate source; title-by-title disc coverage is not established. All other title columns remain not started until independently inventoried.

| Format | Fighters Anthology | Scope/evidence |
| --- | --- | --- |
| LIB / EALIB | Partial | Five archives inventoried; selected resources from three archives imported |
| DCL | Partial | Raw-literal mode; 18 selected menu resources match reference output; coded literals unimplemented |
| PIC / embedded PAL | Partial | Background, six action pieces, four glyph strips rendered; malformed-input bounds checks |
| Standalone PAL | Not started | Main menu uses its background's embedded palette |
| DLG | Partial | CHOOSEAC rectangle and eight action labels/positions recovered at runtime |
| MNU | Partial research only | Selected files extracted; labels inspected through reference decoder; runtime tree remains authored |
| 5K / 11K | Partial | Three menu effects and one optional music recording imported and played |
| FNT | Not started | PIC glyph strips serve the menu; compiled font resources not implemented |
| ESA / LAY | Not started | Loose LIB installation used; LAY is not assumed to be UI |
| XMI / MUS / instrument banks | Not started | Directory inventory only; PCM preview does not count as synthesis |
| PT / PTS / SH / HUD | Not started | Directory counts only; no aircraft import or rendering |
| T2 / OT / JT / NT | Not started | Directory counts only |
| M / MT / campaigns / saves / Pro Mission Creator | Not started | Menu actions are placeholders |
| CB8 / VDO / FBC / INF | Not started | No video/reference playback |

No M1a completion or all-title format validation is implied. See [menu extraction](menu.md) and [baseline](../baselines/main-menu.md).
