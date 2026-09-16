# F14D source and acceptance

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
| FA_2.LIB/F14.PT | f2bcbcfa275147f7641356fb6d8c5bf6f3772b9131d0634ddba06400d6a861a1 |
| FA_2.LIB/F14.SH | 28be633d94fefdc286df4bc2cbe602749ebc91eb2cdbdf7c93cf50707f4a5441 |
| FA_2.LIB/F14.HUD | b2cdcb28e70a639c659d2bea51e0ff6bfc1caef96a825273547b413062e52675 |
| FA_1.LIB/~F14H.PIC | 3b43ddf9b6fe3b0ea40605aaae4be2742787c450089e677e4053c276294a19da |

The [shared acceptance record](aircraft-fa-expansion.md) owns commands, validation,
platform limits and remaining work. The [behavior spec](../spec/additional-aircraft.md)
owns source flight values and fitted response/animation choices. Dependency
edges, unresolved executable symbols and archive hashes are retained locally in
`.local/aircraft-fa/extraction-report.json`. These hashes establish resource
identity; they do not establish retail gameplay parity.
