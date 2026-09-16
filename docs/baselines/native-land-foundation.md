# Native land-contact foundation checkpoint

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


2026-09-15. NE-00.1a complete; NE-00.1 / NE-01.1 / NE-03.1 remain researching.
**Native static predicates translated/tested; no new runtime contact branch.**
[Source contract](../formats/native-land-contact.md),
[living dependencies and next action](../research/native-environment-systems-plan.md).

## Source and selected discovery

Started from clean `7a141c8`. The static extraction verifies both reviewed
EXE/SMS hashes recorded in [native flight](../formats/native-flight.md).
It now emits **84 reviewed regions**, 107 symbol spans and 3,829 symbols;
five regions were added for ground entry, dispatcher/cache, slope projection,
candidate commit and landing-object preference. A reviewed region can include
untranslated branches; its existence does not establish whole-producer acceptance.

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/source
python3 tools/extract_assets.py --include 'STRIP.OT' --include 'UKR.MM' --include 'UKR.T2' --include 'RUNWAY.SH' --exclude-archive 'disc1/LHX/*' --exclude-archive 'disc1/WB/*' --out .local/native-environment/land-discovery
python3 tools/inspect_shape_effects.py .local/native-environment/land-discovery/FA_2.LIB/RUNWAY.SH
```

Selected extraction: **4 resources, zero errors**, all from FA_2.LIB SHA-256
`fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`.
This filtered report is not a cumulative catalog, complete runway closure or
full-media census. Unrelated LHX and installer-provider exclusions are explicit.

| Resource | Decoded bytes | SHA-256 |
| --- | ---: | --- |
| UKR.MM | 47704 | c36e68b16ae98691e4b3356baf5f15fec2c031907caab9553ee3bcd062b2bbbc |
| UKR.T2 | 126899 | df08148af8a6b58ceb46a3d1031abe52896968ba6a1df5c0da0a3c620d77789e |
| STRIP.OT | 1256 | 9f6e97126be473e16db7966ae62012fbce89302f3787512dd7ca237952eeddbe |
| RUNWAY.SH | 8704 | c591e8fb2fc76f78d815884678270947013b79205a3f30499556f1dffc661221 |

The shape inspector found no import/re-entry candidates. This byte-pattern
result is not a complete drawing/dependency or absence proof. `_STRIPProc`
remains an explicit native callback edge. No original-asset runway preview,
collision overlay or live runway was accepted in this checkpoint.

## Tests and provenance limits

Two new synthetic tests exercise native source expectations:

- Landing preference: active flag, zero/nonzero signed field, inhibition flag,
  special instance kinds 2/4 versus other kinds, and +0xe3 gating.
- Cache expiry: branch precedence, fixed deadline versus relative time, all
  four legal draw values, exact optional-draw consumption, invalid draw rejection,
  positive/zero/negative speed, both altitude boundaries and dword wrap.

The cache helper consumes an explicit sample; it does not choose or advance a
global RNG stream. No fitted flight law or new host clock was introduced.
World-cache and query-RNG transactions, collision geometry, type offset and
object initialization remain unimplemented. Existing diagnostic rollback tests
still use supplied read-only queries. They do not validate mutable native caches.

Repository checks passed on Linux: formatting, warnings-denied all-target
workspace Clippy, **338 Rust tests**, locked workspace build and **24 Python
tests**. Logs: `.local/native-environment/checks/`.

Repository/app/extractor asset guards pass, as does `git diff --check`.
The existing native live-API replay probe passes **28 cases / 33,600 updates**
for F/A-18D and Rafale C. Linux / NVIDIA RTX 4070 / Vulkan / Immediate presentation
passes creator, viewer and both native-airborne cockpit smoke tests. These are
single-frame startup regressions, not active flight handling or new contact
acceptance. The host reports an unreadable controller evdev path; no physical
controller acceptance is claimed and no system configuration was changed.

```sh
cargo run --locked -p tore-sim --example native_live -- .local/native-environment/source/tables/sine-q15.bin .local/native-environment/source/tables/atan-pa.bin .local/flight-response/validated-f18/FA_2.LIB/F18.PT .local/flight-response/validated-rafale/FA_2.LIB/RAFALE.PT
target/debug/tore-app --quick-mission --smoke-test
target/debug/tore-app --viewer --smoke-test
target/debug/tore-app --free-flight --aircraft f18 --native-flight-tables .local/native-environment/source/tables --smoke-test
target/debug/tore-app --free-flight --aircraft rafale --native-flight-tables .local/native-environment/source/tables --smoke-test
```

Run the everyday commands in [development](../DEVELOPMENT.md) for the workspace,
Python and asset checks. Source artifacts, filtered extraction report and all
regression logs remain under `.local/native-environment/`.
Windows/macOS runtime/build, physical handling/audio and retail comparison are
unavailable for this checkpoint. No source-derived artifacts are committed.
