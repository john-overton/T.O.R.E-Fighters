# X31 source and acceptance

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-16. All resources below come from the user's
Fighters Anthology base archives. No USNF/ATF or toolkit SWPATCH data is used.

| Resource | SHA-256 |
| --- | --- |
| FA_2.LIB/F31.PT | 129b616db8a1a06e5f8e415d459a61230f448a791756feb74efb20fd0fbe1c0c |
| FA_2.LIB/F31.SH | 96f838de26b7e867bd46f9795f48f15d3399684254af7ed1fa00bf5b1ecd9201 |
| FA_2.LIB/F31.HUD | 3e39e2178d59f4fb9de9a484ad20a303b0b652838ad14eea78e11102ea1330b7 |
| FA_1.LIB/~F31H.PIC | e7d3231598da4b50e8d2566b37acde6866684a059c1c1fd9f74e17282f364df0 |

The [shared acceptance record](aircraft-fa-expansion.md) owns commands, validation,
platform limits and remaining work. The [behavior spec](../spec/additional-aircraft.md)
owns source flight values and fitted response/animation choices. Dependency
edges, unresolved executable symbols and archive hashes are retained locally in
`.local/aircraft-fa/extraction-report.json`. These hashes establish resource
identity; they do not establish retail gameplay parity.
