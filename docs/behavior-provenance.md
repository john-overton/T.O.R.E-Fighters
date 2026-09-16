# Behavior provenance and acceptance

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Provenance answers **where a behaviour came from**. It does not decide whether a
behaviour is acceptable. Acceptance is decided by the parity target in
[AGENTS.md](../AGENTS.md): does a player experience what they experience in
Fighters Anthology?

User direction, 2026-09-15: prioritize recovering and implementing features that
exist in Fighters Anthology. A plausible simulator feature is not evidence that
FA contains it. This policy applies to implementation plans, coverage summaries,
format research and acceptance reports.

## Describe origin separately from completion

| Origin | Meaning | Required documentation |
| --- | --- | --- |
| **Spec-derived** (default) | Implemented from a behaviour spec in [`spec/`](spec/) that describes what the player experiences | The spec file, the numbers tested against, and anything the spec left unknown |
| Native | A code path reconstructed from the reviewed original executable's control flow | Source build/hash or linked source identity; routine/field/caller; trigger, inputs, units, ordering and outputs; unresolved branches |
| Fitted | An implementation approximation authored because a behaviour is not yet specified, or because the host needs something the original did not have | Exact rule/constants, reason, known difference or uncertainty, and runtime scope |
| Opinionated | A deliberate design choice. Say whether John requested it (with request and date) or an agent chose it | Intended departure from original behaviour, scope/default and acceptance criteria |
| Unknown | Insufficient evidence to establish existence, meaning or behaviour | Missing evidence and next research step; keep unavailable/unimplemented where required |

**Spec-derived is the default for gameplay code.** "Native" is a label that
records where a behaviour came from. It is not a requirement, not an acceptance
gate, and not a reason to block, revert or withhold working behaviour. A
`fitted` or `opinionated` component is acceptable as shipped behaviour and does
**not** have to be replaced by a `native` one before acceptance. Replace it when
a spec shows the player would notice the difference.

“Authored” alone is ambiguous: say whether the choice is fitted or opinionated,
and for opinionated say who chose it. An implementation choice made by an agent
is **not** user-directed merely because the user requested the broader feature.
Do not retroactively attribute a fitted constant or design to the user. A user
request to investigate a feature is not permission to invent its behaviour.

A feature may contain several origins. Split the feature into components rather
than applying one label to an entire hybrid system. Native assets or PT
parameters do not make the equations consuming them native. Native Rust code
means the host technology; it does not establish original-game provenance.

## Research-mode recovery steps

These steps apply **in research mode only**, when recovering behaviour from the
original executable. They track how far a recovery has progressed. They are
**not** completion columns for a gameplay feature, a gameplay feature is
complete when it matches its spec, whatever its provenance.

1. **Source established:** identify the behaviour, conditions, producers,
   consumers and exact build. State whether evidence is static code, resource
   data or retail observation. A symbol name alone is not a behavioural
   specification.
2. **Translated and tested:** confirm the reviewed contract with explicit inputs,
   state, time and randomness. Test source-derived expected outputs and boundary
   conditions. Label diagnostic helpers as diagnostic.
3. **Specified:** write the player-visible behaviour and its numbers into
   [`spec/`](spec/). This is where research ends and implementation begins.

Two former columns are retired. **"Runtime connected"** described wiring a
translated code path into the running game; under parity by expression of
feature, implementation works from the spec instead, so it is no longer tracked
for gameplay. **"Retail compared"** remains unavailable: John confirmed on
2026-09-15 that a useful retail flight comparison cannot be run. That is a
recorded evidence limitation, not a prerequisite and not a claim of parity.

Synthetic tests can validate arithmetic and invariants. Deterministic replay can
validate repeatability. Neither establishes retail trajectory parity.

## Planning and reporting rules

- Work from specs. When a spec is missing a number, that is a research task:
  say so, choose a documented value, and label the component `fitted` or
  `opinionated`. Do not invent a *feature* Fighters Anthology does not have.
- Existing fitted behaviour stays explicitly identified. Do not silently remove
  it or expand it, and do not relabel it `native` without the evidence, but it
  needs no replacement to be acceptable.
- Research-mode labels and implementation status are tracked separately. A
  fitted substitute does not close a *research* item; it can perfectly well close
  a gameplay one.
- Use [`formats/`](formats/) for recovered source facts, [`spec/`](spec/) for
  player-visible behaviour, [`baselines/`](baselines/) for methods and measured
  evidence, and [`parity-plan.md`](parity-plan.md) for sequence. Link between
  them rather than duplicating specifications.
- Correct stale status claims in place. Historical measurements remain valid as
  measurements, but supersede misleading completion/acceptance descriptions.
- Scope “parity”: name the feature and the remaining mismatch a player would
  notice. Separate missing implementation from missing original-game comparison.
  Exact legacy clock artifacts and cross-platform bit identity are not product
  requirements; document numerical differences that affect behaviour.

## Current flight examples

Labels describe origin only. None of these is a blocker.

| Component | Origin | Notes |
| --- | --- | --- |
| Warning/stall timers, spin entry/recovery predicates | Native | Translated and tested; initial stall classification is fitted |
| Stall control/lift attenuation | Native arithmetic | Connected in hybrid; clean-envelope reference speed and later force integration are fitted |
| Timed warning-transition rotation (“tumble”) and stalled movement fall | Native | Tested with both PTs and imported tables; legacy/hybrid unchanged |
| Loaded normal controls, rudder/auxiliary rates, departure→force→movement | Native translations with authored driver | Tested for both PTs; turbulence bypass and host clock/device/fuel producers are fitted |
| Ground, terrain and object contact | **Opinionated** | Reclassified 2026-09-15. Contact behaviour is authored to match what a player experiences on a runway and deck; it is no longer waiting on a recovered native producer |
| Clean-envelope stall-entry gate | Fitted | Runtime behaviour; acceptable as shipped |
| Response filters, trim/alignment, continuous spin coupling | Fitted | Runtime behaviour; acceptable as shipped |
| `sideslip_drag=0.5` in both aircraft models | Fitted | Chosen by the implementation, not requested by John and not extracted from FA |
| Achieved G/applied-rate diagnostic snapshot | Diagnostic instrumentation | Measures our adapter; does not prove retail exposes equivalent channels |
| Sustained controller rumble | Opinionated, requested by John | Mapping still to be designed |

Diagnostic tooling is not a new gameplay feature. Document its purpose and limits
without pretending it is recovered retail behavior or a user-chosen flight law.
