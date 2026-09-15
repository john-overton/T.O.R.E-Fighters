# Active Quick Mission tables and dialog geometry

Recovered from the reviewed FA.EXE/SMS pair on 2026-09-14. This specifies source
data and consumers; it does not implement mission generation. Full extracted
catalogs remain ignored, not embedded in the application or tests.

## Active selector dispatch

`0x42e720` subtracts 3 from the field ID, bounds it against 59 and indexes the
60-entry table at `0x42e86c`. Static branches return double-NUL-terminated lists.
Aircraft branches construct filtered catalogs; ground targets dispatch by theater
through 16 entries at `0x42e95c`. The research reader accepts only the reviewed
constant-pointer grammar for static branches; it never executes code.

| Field IDs | Meaning | Active source contract |
| --- | --- | --- |
| 3, 20 | Friendly/enemy nationality | Same 60-entry list, source order/spelling preserved |
| 4, 7, 10; 21, 24, 27 | Wing counts | Six choices, 0 through 5 |
| 5, 8, 11; 22, 25, 28 | Wing skills | Four choices, novice through ace |
| 6, 9, 12; 23, 26, 29 | Wing aircraft | Dynamic GetNames; ID 6 uses player filter, others use other-wing filter |
| 13 | Theater | 16 source names; source index 14 is Ukraine |
| 14 | Altitude | 5,000 / 10,000 / 20,000 / 40,000 feet |
| 15 | Conditions | Dawn, clear, cloudy, overcast, foggy, sunset, night |
| 16 | Situation | Advantage, neutral, disadvantage |
| 17 | Separation | 1 / 2 / 5 / 10 / 20 / 50 miles |
| 18 | Load | Standard or custom |
| 19 | Air combat | Guns only or guns and missiles |
| 30 | Ground target | Theater-specific list, including none |
| 31, 32 | AAA / SAM strength | Four levels, none through heavy |

Theater target counts, including none, in source index order:

| Index | Theater | Count |
| ---: | --- | ---: |
| 0 | Baltics | 9 |
| 1 | Cuba | 7 |
| 2 | Egypt | 8 |
| 3 | Falklands | 7 |
| 4 | France | 8 |
| 5 | Greece | 6 |
| 6 | Iraq | 9 |
| 7 | Kuril Islands | 8 |
| 8 | North Vietnam | 10 |
| 9 | Pakistan | 7 |
| 10 | Panama | 7 |
| 11 | Persian Gulf | 7 |
| 12 | South Korea | 7 |
| 13 | Taiwan | 7 |
| 14 | Ukraine | 9 |
| 15 | Vladivostok | 8 |

Map source indices to shared theater identities explicitly and revalidate target
selection on theater changes. IDs 33–62 are multiplayer extensions: player
aircraft/wing assignments, revive settings, scoring and end conditions. They are
reported, not added to single-player setup. No BARCAP or runway/carrier-start
option occurs in this single-player dispatch; recover downstream contracts separately.

## Shared selector and interaction

`0x430680` calls the option producer and uses QUICK14 (reference at `0x430732`).
List control 3 receives the runtime list at `0x430751–0x430767`. Its compiled
record does not contain the active strings. QUICKB lists are legacy evidence.

The handler supports modal selection and direct cycling. Modifier/input state
and a field-ID class choose the branch; exact modifier naming remains unverified.
Cycling wraps in both directions. Modal results 1/2 distinguish accept/cancel;
dynamic aircraft lists are released after use. Do not assume every click opens
a popup. Availability helper `0x4300e0` returns true for one participant; other
branches concern multiplayer ownership, not complete mission validity.
Initialization includes RNG/catalog choices: screenshot values are not fixed defaults.

## Static DLG records

`ui::dialog::parse` resolves bounded PE/PL imports and HIGHLOW relocations,
identifies inert imported draw thunks, and reads reviewed action/list/dial/rocker
position fields. Text-draw payloads remain opaque. Action labels are literal
strings or imported label symbols. No callback or generic widget VM is executed.

Coordinates are in the original 640×480 canvas. Stored action widths are not
inferred face/hit widths or sprite-shadow extents.

| Dialog | Origin | Stored size |
| --- | --- | --- |
| QUIKMISS | (12, 88) | 615×378 |
| QUICK14 | (185, 100) | 270×370 |
| LOADORD | (115, 356) | 470×102 |

| Control | Local position | Absolute position | Stored width |
| --- | --- | --- | ---: |
| QUIKMISS OK | (375, 331) | (387, 419) | 85 |
| QUIKMISS Cancel | (480, 331) | (492, 419) | 85 |
| QUICK14 OK | (32, 337) | (217, 437) | 85 |
| QUICK14 Cancel | (127, 337) | (312, 437) | 85 |
| QUICK14 list | (20, 15) | (205, 115) | 230 |
| LOADORD Fly | (378, 58) | (493, 414) | 80 |
| LOADORD Select Plane | (248, 58) | (363, 414) | 100 |

LOADORD's page rocker is local (145,52), fuel rocker (432,0), and both dial records
share (33,38). QUICK14's rocker starts at (0,0); this is not established final
placement. Native setup/drawing determines state-dependent art and hit regions.
LOADORD's rectangle covers bottom controls, not the whole illustrated screen.

## Runtime briefing geometry

Compositor `0x42fde0` indexes 29 records at `0x4f1d30`, each five signed 16-bit
words: x, single-player y, multiplayer y, width, height. It adds the current
dialog origin; text drawing subtracts one from y. Most regions are 264×16.

Single-player sentence layout relative to QUIKMISS:

| Lines | x | y | Purpose |
| --- | ---: | --- | --- |
| 0 | 23 | 50 | Friendly nationality |
| 1–3 | 23 | 78, 92, 106 | Friendly wings |
| 4–7 | 23 | 134, 148, 162, 176 | Theater, altitude/conditions, situation, separation |
| 8–9 | 23 | 204, 218 | Load and weapon restriction |
| 10 | 328 | 50 | Enemy nationality |
| 11–13 | 328 | 78, 92, 106 | Enemy wings |
| 14 | 328 | 134 | Ground target/AAA/SAM, 264×48 |

Remaining rows serve multiplayer composition. Inline field rectangles come from
text rendering and are written into dialog controls; they are not fixed boxes in
the zeroed QUIKMISS records. Reconstruct them with original font metrics/wrapping.

## Ordnance card placement

`0x41a428–0x41a739` initializes catalog anchors at (68,108), alternates x=68/188,
and advances y by 68 per pair. Page arithmetic uses eight entries per page.
`0x41a75b–0x41aadd` initializes station anchors at (350,121), alternates x=350/469
and advances y by 71 per pair. Store art/text have offsets from these anchors;
anchors are not complete card/hit rectangles. This agrees with the photos' layout.

## Remaining gates

Dynamic aircraft filtering/defaults, mode visibility, post-selection dependencies,
exact modifier mapping, final rocker/dial/card art geometry and font hit regions
remain open. The app still uses its earlier fitted briefing. Wire verified data
into state and rendering before claiming screen parity.
[Validation](../baselines/menu-options-geometry.md).

## Initialization and dependent fields

`0x42f2e0–0x42f876` initializes the setup; the single-player field state is
`0x537360 + 4 * field_id`. These are initial values, not saved-session defaults.
The function seeds its RNG using prior RNG/clock/input state. A fixed diagnostic
seed would be authored, not a recovered retail startup seed.

| Fields | Initial single-player value |
| --- | --- |
| 4, 5 | One friendly Wing 1 aircraft, average skill |
| 7, 8, 10, 11 | Zero aircraft and novice skill for friendly Wings 2/3 |
| 6 | Random player-catalog entry, rejecting catalog flag `2` |
| 9, 12, 23, 26, 29 | Random other-wing catalog entries |
| 13 | Random theater from 16 entries |
| 14, 15, 16, 17 | 5,000 feet; clear; neutral; 5 miles |
| 18, 19 | Standard load; guns and missiles |
| 21, 22 | Random 2–3 enemy aircraft; random experienced/ace skill |
| 24, 25, 27, 28 | Zero aircraft and novice skill for enemy Wings 2/3 |
| 30, 31, 32 | No ground target, AAA or SAM |

`0x4308a0–0x4309f8` sets friendly nationality to index 0 (American) and enemy
nationality by theater. This runs at initialization and after theater changes.

| Theater | Enemy nationality index / label |
| --- | --- |
| Baltics, Kuril, Ukraine | 10 / Russian |
| Cuba | 33 / Cuban |
| Egypt | 14 / Islamic Egyptian |
| Falkland | 57 / Argentinean |
| France | 3 / French |
| Greece | 41 / Turkish |
| Iraq | 23 / Iraqi |
| North Vietnam | 20 / North Vietnamese |
| Pakistan | 37 / Indian |
| Panama | 34 / Panamanian |
| Persian Gulf | 24 / Iranian |
| South Korea | 9 / North Korean |
| Taiwan, Vladivostok | 2 / Chinese |

Keep the source mapping even where a geographic assumption would suggest another
country. These are nationality indices, not simulation allegiance IDs.

After an accepted selection, `0x430843–0x430893` enforces friendly Wing 1 count
at least one in single player. Thus its raw list contains zero, but an accepted
single-player setup cannot retain zero. If the ground target is none, both defense
fields are cleared. Next, a changed theater clears the target and resets both
nationalities. The order matters: this span does not clear defenses again after
clearing the target on a theater change. Do not claim immediate defense reset in
that particular path without tracing the outer update loop.

## Selector input contract

`0x430680–0x43089b` uses `GetKeyFlags() & 3`. The input producer at `0x411600`
assigns bits 1/2 to right/left Shift; Ctrl is 4 and Alt is 8.

- Scalar fields cycle directly; Shift opens the shared QUICK14 list dialog.
- Aircraft fields invert that choice: direct activation opens the list, Shift
  cycles. The field set includes the multiplayer aircraft extensions.
- Cycling wraps at either end. Shell button bit 1 selects increment; its absence
  selects decrement. Physical mouse-button naming still needs producer review.
- The popup populates control 3 with the current list/index. Action 1 accepts its
  index; action 2 closes without copying it into the draft.
- The post-selection constraints above run after acceptance, not popup cancel.

`0x4301a0–0x4303d4` refreshes text and copies changed fields to the corresponding
shadow setup. Conditions index 6 also sets the night flag at `0x4f1c80`.
This is not evidence of file persistence or an immutable launch transaction.

## Aircraft filter masks

Normal single-player initialization uses catalog mask `0x02400007`, player-list
mask `0x02400807`, and other-wing mask `0x02000807`. The low player bits are **7**
in this branch, not the **3** used in the era branches. A hidden modifier path
also exists; it must not become the normal player-availability rule.

With Fly all enabled, the four era choices use base masks `0x04000003`,
`0x08000003`, `0x10000003`, `0x20000003`; player lists add `0x800`.
Other-wing masks are `0x06000807`, `0x0a000807`, `0x12000807`, `0x22000807`.

GetNames uses 48-byte records: flags at 0, resource name at 4, display name at 17.
The filter span `0x41d209–0x41d383` checks category overlap and requested flag
requirements, including `0x1000` (Alt bypass), `0x400000`, era bits when Fly all
is set, `0x1000000`, `0x2000`, and `0x40000000`. Tilde-prefixed resources have
special modifier gates. Flag construction, pluralization and final catalog
identity mapping remain separate from these verified call masks; do not treat
any nonzero overlap as full eligibility, or catalog presence as flyable support.

## Runtime implementation checkpoint

The app now consumes the fingerprinted active tables through `ui::creator` and an
inert cache; scalar controls, list acceptance/cancel, nationality/target dependencies
and supported airborne setup are wired. Aircraft catalog metadata is imported at
runtime; native eligibility/era flags remain incomplete. Popup geometry and sentence
fitting are provisional. [Behavior and acceptance](../baselines/creator-ordnance.md).
