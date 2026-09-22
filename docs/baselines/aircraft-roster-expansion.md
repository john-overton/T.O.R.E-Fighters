# REDFOR and F-22A import acceptance

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-16. John requested seven player aircraft. The
agent selected the existing shared response fit and initial device travel.
[Behavior contract](../spec/roster-aircraft.md). No autonomous behavior was added.

## Source identity

All aircraft records and geometry come from the user's base FA_2.LIB, with
art and dependencies from FA_1.LIB/FA_2.LIB. No SWPATCH overlays or reference
checkout runtime/assets are used. Original modules are read as bounded data,
never executed. Local source-build hashes:

- `FA.EXE` SHA-256 `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`.
- `FA_1.LIB` SHA-256 `657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f`.
- `FA_2.LIB` SHA-256 `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`.

| Identity | PT SHA-256 |
| --- | --- |
| MiG-29 Fulcrum-C (`MIG29.PT`) | `261c3938c6a864e706f8655a755262d28d3953bd0d9290b533856ac298803075` |
| Su-27 Flanker-B (`SU27.PT`) | `77694834047eceb18a8466ef826a297dd477a17f0eff9f20bbdde3fef39e2ac5` |
| MiG-21 Fishbed (`MIG21.PT`) | `bf4b1c2302045f7d20893ba09bb4f7382dab619bdfdc38b4acd54c79dd38e9cd` |
| Su-25 Frogfoot-A (`SU25.PT`) | `2c865df7a38f667090ab46ddf79fb90ccc7ba48f235b92271e8ade9f5d54deed` |
| MiG-23 Flogger-B (`MIG23.PT`) | `8704a2c0ce241a2f3116dac50e0920d6412bbc6d77025c2770b02831b6b470ab` |
| Su-35 (`SU35.PT`) | `92d5824b64e6584b0512d0f14257f3a362604e9a72e4ad331a1074530ab53452` |
| F- 22A Raptor (`F22.PT`) | `e6b0009e2cfd48b53f18a80abcf2ae41d2c77bb8c66a5d7401b5ad0a62f86a15` |
| F- 22N Raptor (`F22N.PT`, added 2026-09-22) | `5ac12358639abba3119d6b94b631ff20e62c682052aa1f6804394a86ef9476bc` |

Exact displayed F22 long name is `F- 22A Raptor`; the UI normalizes spacing to
F-22A Raptor. MiG-29 is Fulcrum-C, Su-27 is Flanker-B, MiG-23 is Flogger-B,
and Su-25 is Frogfoot-A. Su-35 is only named Su-35, not a reviewed Su-35S.
The source Su-25 mass and thrust are intentionally preserved despite differing
from common real-aircraft expectations. Type sizes and presentation resources
are recorded in [aircraft formats](../formats/aircraft.md#roster-expansion).

## Reproduction

```sh
python3 tools/extract_assets.py --aircraft mig29 --aircraft su27 --aircraft mig21 --aircraft su25 --aircraft mig23 --aircraft su35 --aircraft f22 --exclude-archive 'disc1/*' --exclude-archive 'disc2/*' --exclude-archive 'swpatch.lib' --out .local/roster-aircraft --validate-flight
cargo run --locked -p tore-app -- --import gameassets/fighters-anthology --import-only
cargo run --locked -p tore-app -- --free-flight --aircraft su35
cargo run --locked -p tore-app -- --free-flight --aircraft f22
cargo run --locked -p tore-app -- --validate-creator
```

Extraction selected 312 resources. `.local/roster-aircraft/extraction-report.json`
records per-resource hashes, providers, dependency edges and unresolved module
candidates. MiG-29, Su-27, Su-25 and MiG-23 have no PTS companion; those known
absences are explicit and do not bypass required PT/HUD/shape validation.
The app refreshed the runtime pack with all twelve registered aircraft.

## Coverage and validation

All seven have independently owned typed flight configurations, original
exteriors, referenced cockpit art, source audio references, gear and applicable
brake/afterburner presentation. Controls use each PT's response values through
the documented fitted laws. The subsequent [animation and material pass](aircraft-animation.md) adds fitted
control surfaces, visual sweep, F-22 main bays and reviewed round-outlet materials.
Original schedules and complete systems parity remain open.

The expanded flight suite passes 13 scenarios per aircraft, 91 total, with
deterministic replay. Stall/spin probes now start at half each aircraft's own
clean-envelope minimum at 15,000 ft; the previous absolute 180 ft/s did not
put Su-25 below its clean envelope. Expectations for stall, recovery, loops,
wind and contact were retained. F-22 correctly honors spinEntry=2.
Each aircraft also passes a 1,200-tick roll probe in legacy and hybrid modes.
Logs: `.local/roster-suite.log` and `.local/roster-*-headless.log` / `*-legacy.log`.

Default manual weapon smoke cases pass for each new aircraft. They cover gun,
missile or ballistic release as applicable, five damage classes for aimed
weapons, incoming ECM, ammo/mass, station failures, damage and replay. Damage
validation now tries separate source-weapon histories when the first fatal
missile selects no subsystem fault. Jettison checks retain internally flagged
stores, including Su-35's AA11B station. No damage values, flags or RNG behavior
were changed to satisfy tests. Creator validation checks all twelve aircraft,
compatible loadout edits, empty stations, fuel and accepted-load restart.
Logs: `.local/roster-*-combat.log` and `.local/roster-creator.log`.

Linux GPU cockpit and deployed-device exterior captures pass for all seven.
Contact sheets were visually inspected; source shapes, textures and cockpit
families render without substituted aircraft art. Su-27 and Su-35 center mirror
fills use the existing rear camera. Other new mirror regions remain disabled.
Captures and driver logs: `.local/roster-gpu/`.

All required checks passed: formatting, Clippy with warnings denied, 396 Rust
tests, locked workspace build, 40 Python tests, source and both binary asset
guards, documentation headers and the required Linux/Vulkan rendering smoke.
The final regression suite passes 156 scenarios across all twelve aircraft.
The seven-aircraft extraction plus flight-validation command also passes end
to end. Su-25 rejects an afterburner capture override through the typed source
capability check. Wide and tall cockpit views and deployed exteriors produce
21 captures; all contact sheets were inspected. Quick Mission's selector
capture visibly lists all twelve exact aircraft labels. Su-27 and Su-35 each
report one reviewed center mirror region. Final logs use `.local/roster-*`.


## Limits

These are initial player ports, not claims of full original-game or real-world
parity. Retail comparison, Windows/macOS execution, subjective audio audition,
controller feedback and manual flight sessions were not performed. Original
continuous animations, bay doors, complete instrumentation, shadow/damage/LOD
shapes and complete exterior store placement remain unvalidated. The restricted
native-table adapter retains its existing limitations. No retail data or
derivatives were added to tracked files. No commit or push was made.
