# Radar research validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-16, followed by an implementation-mode validation pass on
the same day, recorded in the final section. The research sections are local
static inspection only. No retail execution or matched retail comparison is
claimed anywhere on this page.

## Inputs and method

Recomputed SHA-256 on the supplied installation:

- FA.EXE: `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`
- FA.SMS: `e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0`

Read nine distinct radar SEE records and the twelve imported aircraft PT
hardpoint lists from the existing ignored
`.local/weapons-research/supplement/FA_2.LIB/` extraction, whose report identifies
the source FA_2.LIB records, offsets and decoded hashes. Original extraction
method is in [weapon research](weapons-research.md). Inspected the existing full
FA disassembly and SMS inventory under `.local/weapons-research/native/` plus
reviewed FOV/lock spans under `.local/systems-pass/native/reviewed/`.

The ignored reference checkout's sensor notes were leads only. Its engine was
not reused and its other-title values were not substituted for FA records.

## Reproduction and follow-up results

The bounded local audit is `.local/radar-research/audit.py`. From the repository
root, `python3 .local/radar-research/audit.py` asserts the FA.EXE hash, resolves
PT default hardpoint pointers, decodes SEE fields and reads the range/mode tables
through PE section bounds. It writes ignored `roster.json` with per-record hashes.
This is local research tooling, not a new runtime dependency or committed retail
fixture. Recreate the extracted inputs using the linked weapons baseline.

Observed: twelve aircraft, one radar each, nine distinct radar records. The
follow-up also reads OBJECT sigs[2] and sigs[3] from all twelve PT prefixes using
the existing schema field order. Their IR/radar values are listed in the spec;
these are relative game statistics, not measured physical cross sections. All
bindings agree with the existing Rust identity mapping. Extra IR/laser records
are listed separately in the spec. Static inspection confirms the six-position
range ladder, the display-mode labels, automatic range-versus-track mode selection
and the ordinary RWS target-cycle restriction. No original module was executed.

The USNF manual is supporting context only; the FA tables and consumer checks
establish the new range/mode claims. Complete mouse/history/support transitions
remain unknown. The scope reset's 10-mile default and the app's different range
ladder and default were a real implementation gap at the time of this research,
not another radar profile; the implementation pass below closed it.

## Results and limitations

The [spec](../spec/radar.md) records the measured nominal ranges, signatures and
parameters. The [component guide](../radar.md) follows John's subsequent
request for an authored signature-based model, with agent-authored look-down,
notch, equipment-generation and jammer tuning, plus the subsequently requested
jammer-generation matchup and directional scope interference. The public Naval
Air Warfare Center handbook is qualitative background for this authored model,
not evidence of retail behaviour; the component guide links the relevant section.
None of its gameplay constants was validated against retail. They were
implemented in simulation by the pass recorded below, which likewise makes no
retail comparison.
[Source notes](../formats/radar.md) record the newly inspected look-down and
Doppler branches, unresolved flags and next evidence needed. A provisional flat
look-down range reduction was rejected after consumer inspection. Implementation
edits made before the planning clarification were removed; gameplay is unchanged.

No new behavioural tests or rendering tests applied to this research-only change.
All nine required repository checks passed on the research-only documentation
change: formatting, Clippy, Rust tests/build with --locked, Python tests, three
asset scans and documentation headers. A rendered smoke test was not run because
no rendering or gameplay code changed at that point. The range/angle boundaries,
terrain-relative look-down examples, capability modes, mouse identity selection
and missile acquisition versus sustained illumination are covered by the
implementation pass below.

## RCS scope evidence

The user supplied an RCS panel image, retained outside the repository. Checked
FA CPComputeRCS 0x43e8c0..0x43ea33, its display call at 0x43ea4e and COSig call
at 0x478339. The latter proves that the helper also affects radar signature.
Read the original FA manual's RCS/RWR discussion through a public mirror, linked
in the RCS spec. No original executable or reference oracle was run. The old
reference checkout's prior oracle results are not presented as this pass's tests.

[The RCS spec](../spec/rcs.md) records retail facts. The component guide's 1/2/4
aspect weights, configuration multipliers and reference contour are agent tuning,
not retail results. Full glyph-state/contact eligibility and original RCS zoom
steps remain unresolved. No renderer or simulation code changed in this research
pass.

## History, selection and A2A scope review

Re-read the local USNF manual's historical-mode, contact-symbol, IR and missile
guidance sections;
recomputed the transcript hash recorded in the source notes. No history timing
was found in those passages. Audited the scope/control and live-combat code of
the day for Y/I action conflicts and HP-based contact suppression. That review
was read-only research, not implementation or live validation.

The scope includes Y history and IR A2A, persistent current-contact selection,
retained detectable destroyed aircraft and a single shared fire-control track.
These directions are John's; the trail length/cadence, IR range law, key
migration and detailed loss transitions remain labelled agent choices.
Reviewed the per-projectile launch target snapshot and weapon-specific
radar-dependency gate. John's clarification preserves independent missile targets
for sequential fire-and-forget shots while continuous-lock weapons still require
support for their own target. The acceptance cases named there were planned at
the time; they were written and run in the implementation pass below.
Existing target-view IFF stays gamified. A2G/HARM and detailed ground/remnant
systems are deferred. No new code was implemented or committed in this review.

All nine required repository checks passed again after this scope review:
formatting, Clippy, Rust tests/build with --locked, Python tests, three asset
scans and documentation headers. No rendering smoke test was run because that
change only updated research and planning documents. Those checks did not
validate the radar behaviour or establish retail parity.

## Implementation-mode validation, 2026-09-16

Implementation mode. The shared sensor component described in the
[component guide](../radar.md) is in code and validated locally. Host identity
for every runtime result below: Linux x86_64, NVIDIA GeForce RTX 4070 with
Vulkan, pinned Rust 1.91.1, locked dependencies. **No retail comparison was made
and none is claimed.** Nothing here establishes retail parity; it establishes
that the local contracts in the component guide hold.

### What was run

- **Workspace checks, all clean.** `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked` and `cargo build --workspace --locked`.
  **447 Rust tests pass.** The sensor component contributes **35** of them, its
  unit tests plus the scenario tests in the `sensors::acceptance` module; the
  combat integration cases are inside the tore-sim total, and the scope
  projection, mouse pick and hover tests are in the app crate.
- **Python tests.** `python3 -m unittest discover -s tools -p 'test_*.py'`,
  **40 tests pass.** The three asset scans and the documentation header check
  pass.
- **Per-aircraft capability review.**
  `cargo run -q --locked -p tore-app -- --sensor-summary` prints one line per
  registered aircraft: radar record with search/track volumes, look-down
  coefficient and preset; infrared record; visual record; jammer record with
  generation and strength; and the PT radar and infrared signatures. All twelve
  imported aircraft report the expected equipment, and the radar records,
  nominal ranges and look-down coefficients match
  [the spec](../spec/radar.md#aircraft-capabilities). The saved output is
  `capability-summary.txt` in the evidence directory below.
- **The recovered automatic mode rule reproduces on the imported profiles.**
  F/A-18D reports TWS at the 25 and 50 settings and RWS at 100 and 150. A-4E and
  MiG-29 both report TWS at 25 and RWS at 50. In RWS a clicked contact stays
  selected while the weapon reports RWS SEARCH ONLY, so a search selection never
  supplies a lock.
- **Combat smokes.** `--combat-smoke` passes for **all twelve aircraft**, and the
  Rafale run additionally records a version-3 tape and replays it to an identical
  state. Scripted probes observe for one step before designating and for the full
  60-step acquisition before firing, the same wait a player has.
- **Rendered smoke.** `cargo run --locked -p tore-app -- --smoke-test` passes on
  Linux x86_64, NVIDIA GeForce RTX 4070, Vulkan.
- **Formula recheck.** Every formula in the component guide was recomputed by
  hand against the code, including the 63.64-nmi signature-50 case,
  L = 0.75/0.85/1.0, N = 0.20/0.45, J = 1 at B and 0.40 at 2B, burn-through at
  18.26 and 3.95 nmi, and the 12.5/17.68/25-nmi contour radii. All agree. The
  same review found and fixed five presentation and equipment-state defects and
  added the two missing noise-presentation rules; the results are described in
  the guide.
- **Captures inspected by eye.** The radar page showing TWS at the 10-mile
  setting with a selected and acquired contact and TRACK status; the same page
  with the target jammer powered, showing the directional noise band with
  sidelobe haze and the contact still burned through at close range; the infrared
  channel with history trails; and the RCS page showing the exposure contour
  wider abeam than nose-on.

Evidence is ignored under `.local/radar-implementation/`: `capability-summary.txt`,
`scope-tws-track.png`, `scope-jammer-noise.png`, `scope-infrared-history.png`,
`rcs-exposure-contour.png` and `implementation-notes.md`. None of it is committed.
Headless captures can set the scope with `--sensor-channel radar|ir`,
`--scope-range 5|10|25|50|100|150` and `--scope-history`.

### What was not run

- No retail execution, no matched retail recording, no side-by-side comparison
  against the original game. There is no oracle for any number below.
- No side-by-side tuning review of the twelve aircraft against each other. Stage
  5 of the component guide's delivery table is therefore only partly done: the
  capability summary and the combat smokes pass for every aircraft, but nobody
  has sat down with the twelve results together and tuned them.
- No multi-hour or soak run, no cross-CPU bitwise comparison, and no hardware
  input or haptics acceptance specific to the sensor keys.
- No automated GPU test of the scope raster. The capture inspection above is a
  human looking at images, not an assertion in a test.

### What stays approximate

- Every detection, look-down, notch, jamming, selection, history and RCS constant
  is agent-authored gameplay tuning. They are labelled `opinionated` and were
  chosen for play, not measured from retail or from any real system.
- The preset and jammer-generation groupings are agent classifications assigned
  per installed record. They are reviewable in code and in the component guide,
  and they are not recovered retail categories.
- Band compatibility is 1 for every current radar and jammer pair, because the
  retail records establish no frequency coverage.
- The infrared channel uses the base PT infrared signature only. Engine and
  throttle heat, rear-aspect bonuses, weather attenuation and post-destruction
  cooling are not modelled.
- The destroyed-aircraft ballistic fall is a minimal fitted addition, not a
  breakup or debris simulation.
- Terrain visibility keeps the existing sampled height-query approximation, not
  recovered native masking.
- The remaining departures from the plan and from retail data behaviour are
  listed in the component guide's
  [deliberate departures](../radar.md#deliberate-departures-and-known-approximations).
