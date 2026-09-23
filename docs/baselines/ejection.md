# Ejection validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research followed by implementation, 2026-09-23, validated against main
569dc9b, including the airbrake, RWR, envelope and instrument-frame fixes.
The [behaviour specification](../spec/ejection.md) owns all gameplay values;
[the source notes](../formats/ejection.md) own recovered facts and uncertainties.

## Research and imported media

Recomputed the local FA.EXE and FA.SMS hashes and confirmed they match
[the AI input identities](ai-research.md#inputs-and-identity). Reviewed the
rendered manual page 161, bounded executable disassembly, EJECT.NT and SH
state guards. The ignored USNF-ATF documentation was consulted only as a source
index. No original program or module was executed.

Extracted 17 selected ejection resources from the installed FA_2.LIB with zero
errors. The regular app importer then built an isolated `.local/dev-profile`
cache, including EJECT.SH, the four original textures, &EJECT/&CHUTE effects,
and the reviewed ejection voice clips. Extraction, disassembly and rendered
retail derivatives remain under ignored `.local/ejection/`.

## Behaviour checks

Synthetic tests cover two-press confirmation, timeout, repeat-key suppression,
rebinding token, the `key:Shift-e` alias with modifier/menu guards, tape serialization, missing seats, dead pilots, previous
impact, living-pilot escape from a wreck, launch momentum and exact phase
boundaries, successful landing, fatal low inverted escape, wound progression,
and survival after the aircraft's later destruction.

AI tests cover recoverable versus low-altitude dives, projected terrain,
insufficient lift/control, the strict healthy-aircraft guard above 200 feet AGL,
manual escape remaining available, weapons being preempted, and pilot descent
continuing after AI aircraft control stops. The deterministic random test uses
seed 4: draw 78 fails at tick 120 and draw 4 succeeds at tick 240. There are no
intermediate polls. Recovery resets the interval without reseeding; another
pilot's draws do not change the replay.

Original F18.PT (F/A-18D) and RAFALE.PT (Rafale C) both completed a 72,000-tick
headless replay using eject commands at ticks 1 and 12. Both ended with
`ejection=Landed`, `pilot_alive=true`, and an independently crashed aircraft.
The pilot reached the imported terrain height of 512 feet. The ordinary
1,200-tick headless flight completed without a crash. This is host validation,
not retail execution comparison.

## Rendering and checks

The display-capable Linux host used an NVIDIA GeForce RTX 4070 Vulkan renderer.
The normal `cargo run --locked -p tore-app -- --smoke-test` passed. Captures with
`--ejection-preview seat`, `--ejection-preview freefall` and
`--ejection-preview chute` showed the original seat, pilot and textured canopy,
including suspension lines. Fresh F18 and Rafale chute captures after rebasing
also verified their distinct original instrument frames from main's bugfix pass.
Additional pilot shadows and original animation timing remain outside this pass.

Final required checks passed: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked` (1,324 passed, three existing ignored tests),
and `cargo build --workspace --locked`. Python tool discovery passed 75 tests.
Asset checks passed for repository source, `target/debug/tore-app` and
`target/debug/tore-extract`. `python3 tools/check_docs.py` and `git diff --check`
also passed. The regenerated `controls_doc` check passed. Shift+E is documented
in the master controls list, flight-command table, input-binding guide, F11 help
and CLI flight help. The aircraft-import, audio, architecture, source, spec and
planning guides link the ejection contract. Current integration logs are local
under `.local/ejection/rebase/`; earlier research remains in `.local/ejection/`.

No original-game comparison, Windows/macOS runtime test, physical controller
ejection test or audible speaker-identification test was performed. Exact RIO,
pilot and wingman routing is not claimed from filenames. The host voice event
assignments are fitted. M_EJECT retains the existing missing AIR015.11K branch
and diagnostics; no replacement music is invented. Campaign rescue/capture,
separate additional crew and post-separation weapon damage remain unimplemented.
