# A4E source and acceptance

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
| FA_2.LIB/A4E.PT | fcaf24149ca78dec9143e8ebd9873f69be17cfa28198f557b82343b5a46649e9 |
| FA_2.LIB/A4.SH | ee98333ab0fc419e0fb2abdd31e57ae24e672d6e4d919790c53b4bfb62d4053b |
| FA_2.LIB/F4.HUD | 4569b2042a88f34c93586ed042de60573eb406e1fb5f0ac84f8c199f1190c2ef |
| FA_1.LIB/~F4H.PIC | 5f5314c72f5915d1377cccaa1f6f7712607e4d615923ef76c94284c86a7eea93 |

The [shared acceptance record](aircraft-fa-expansion.md) owns commands, validation,
platform limits and remaining work. The [behavior spec](../spec/additional-aircraft.md)
owns source flight values and fitted response/animation choices. Dependency
edges, unresolved executable symbols and archive hashes are retained locally in
`.local/aircraft-fa/extraction-report.json`. These hashes establish resource
identity; they do not establish retail gameplay parity.

## Requested roll-response validation

On 2026-09-16 John requested roughly 90% of a researched roll rate. The
[evidence and fitted rule](../spec/additional-aircraft.md#a-4-roll-tuning) record
why the hybrid peak is 648 degrees/s and why this is not an A-4E test-data claim.
Synthetic aircraft integration tests verify both roll directions, quarter/full
stick scaling, deterministic replay and neutral release. The same test verifies
legacy remains at 180 degrees/s. The 65-scenario aircraft flight suite passes.
Physical controller handling has not been playtested by the agent.
