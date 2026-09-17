# Radar research validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-16. Local static inspection only. No retail execution,
matched retail comparison or runtime radar acceptance is claimed.

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
remain unknown. The scope reset's 10-mile default and the current app's different
range ladder/default are a real implementation gap, not another radar profile.

## Results and limitations

The [spec](../spec/radar.md) records the measured nominal ranges, signatures and
parameters. The [component proposal](../radar.md) now follows John's subsequent
request for an authored signature-based model, with agent-proposed look-down,
notch, equipment-generation and jammer tuning, plus the subsequently requested
jammer-generation matchup and directional scope interference. The public Naval
Air Warfare Center handbook is qualitative background for this authored model,
not evidence of retail behaviour; the component guide links the relevant section. None of its new gameplay constants
was validated against retail or implemented in simulation.
[Source notes](../formats/radar.md) record the newly inspected look-down and
Doppler branches, unresolved flags and next evidence needed. A provisional flat
look-down range reduction was rejected after consumer inspection. Implementation
edits made before the planning clarification were removed; gameplay is unchanged.

No new behavioural tests or rendering tests apply to this research-only change.
All nine required repository checks passed on the research-only documentation
change: formatting, Clippy, Rust tests/build with --locked, Python tests, three
asset scans and documentation headers. A rendered smoke test was not run because
no rendering or gameplay code changed. Future implementation must test
range/angle boundaries, terrain-relative look-down examples, capability modes,
mouse identity selection and missile acquisition versus sustained illumination.

## RCS scope evidence

The user supplied an RCS panel image, retained outside the repository. Checked
FA CPComputeRCS 0x43e8c0..0x43ea33, its display call at 0x43ea4e and COSig call
at 0x478339. The latter proves that the helper also affects radar signature.
Read the original FA manual's RCS/RWR discussion through a public mirror, linked
in the RCS spec. No original executable or reference oracle was run. The old
reference checkout's prior oracle results are not presented as this pass's tests.

[The RCS spec](../spec/rcs.md) records retail facts. The component guide's 1/2/4
aspect weights, configuration multipliers and reference contour are proposed
agent tuning, not retail results. Full glyph-state/contact eligibility and original
RCS zoom steps remain unresolved. No renderer or simulation code changed.

## History, selection and A2A scope review

Re-read the local USNF manual's historical-mode, contact-symbol, IR and missile
guidance sections;
recomputed the transcript hash recorded in the source notes. No history timing
was found in those passages. Audited current scope/control and live-combat code
for Y/I action conflicts and HP-based contact suppression. This is read-only
research, not implementation or live validation of the requested new behaviour.

The plan now includes Y history and IR A2A, persistent current-contact selection,
retained detectable destroyed aircraft and a single shared fire-control track.
These directions are John's; the trail length/cadence, IR initial range law,
key migration and detailed loss transitions remain labelled agent proposals.
Reviewed the current per-projectile launch target snapshot and weapon-specific
radar-dependency gate. John's clarification preserves independent missile targets
for sequential fire-and-forget shots while continuous-lock weapons still require
support for their own target. Acceptance cases are planned, not implemented.
Existing target-view IFF stays gamified. A2G/HARM and detailed ground/remnant
systems are deferred. No new code was implemented or committed in this review.

All nine required repository checks passed again after this scope review:
formatting, Clippy, Rust tests/build with --locked, Python tests, three asset
scans and documentation headers. No rendering smoke test was run because this
change only updates research and planning documents. These checks do not validate
the proposed new radar behaviour or establish retail parity.
