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
| AI decision, timing, pursuit, targeting, weapon-service, wing, threat and route components | Spec-derived | `tore-sim::ai`, from [the AI spec](spec/ai.md) |
| Per-actor AI controller sequencing those components | Spec-derived | `ai::controller`; the sequence is the spec's proposed host API, the rules stay in the components |
| Per-actor AI runtime: own sensors, own stores, own flight model, ammunition debited before a launch event | Spec-derived | `ai::mission`; missile physics are not duplicated, the host's existing combat code realises each launch |
| Quick Mission launch payload carrying side, wing, member, type and resolved experience | Spec-derived | `ai::launch`; every member of a wing carries the wing's menu level, with no jitter |
| AI steering curve shapes: linear roll-in below 7/8 maximum bank, cosine pitch authority, opposing-bank suppression, ceiling-before-terrain ordering | Fitted | The spec gives thresholds and floors, not curves; labeled in `ai::steering` |
| AI pursuit offset sign convention (lateral right, longitudinal ahead) and chased-displacement sign draw | Fitted | Signs unresolved in the source; labeled in `ai::pursuit` and `ai::tactics` |
| AI weapon-service half-second gate scope, 15 s window handling, blocked-path outcome | Fitted | Spec silent on the consequence; labeled in `ai::weapon_service` |
| Atomic allocate-then-debit release option | Opinionated, agent choice 2026-09-17 | Default off; the original debits before allocation |
| AI engagement pitch: half the relative altitude closed over the horizontal distance, bounded to plus or minus 30 degrees, zero when B04 refuses a climb | Fitted, agent choice 2026-09-17 | The evaluator is known not to be the line-of-sight pitch; `ai::fitted` |
| AI base pitch rate equal to the B44 turn rate for the same loaded G limit and speed | Fitted, agent choice 2026-09-17 | The spec gives no separate pitch rate; `ai::fitted`, `ai::steering::pitch_rate_deg_per_s` still reports it unresolved |
| AI zero-duration completion axis: the axis whose remaining difference over its rate is largest, heading winning ties | Fitted, agent choice 2026-09-17 | The spec names the inputs, not the selection function; `ai::fitted` |
| AI last-ditch candidate suitability: a split S needs 1.375 turn radii of altitude, a loop needs that and 100 ft/s above minimum speed, otherwise an equal draw | Fitted, agent choice 2026-09-17 | The spec records "some candidates redraw" without the conditions; `ai::fitted` |
| AI random-tactic menu: an equal draw among straight climb, straight dive, break left, break right and turnaround | Fitted, agent choice 2026-09-17 | Menu contents unrecovered; the five B13 maneuvers needing no target geometry; `ai::fitted` |
| AI remaining tactics after the best-attack and random draws both fail: pursuit | Fitted, agent choice 2026-09-17 | The spec says best attack ordinarily selects pursuit; `ai::fitted` |
| AI lead prediction: the target flies its own heading and pitch at its scalar speed for range divided by store speed | Fitted, agent choice 2026-09-17 | The B44 speed estimator and prediction time are open; the recovered 20000 ft bypass and 1600 ft ramp still apply; `ai::fitted` |
| AI burst and reload pacing after a shot: the weapon service restarts from search with the aircraft's own search delay | Fitted, agent choice 2026-09-17 | B42 leaves the consequence open; `ai::fitted` |
| AI store hit chance: fifty points scaled by how far inside its employment angular limit the store points | Fitted, agent choice 2026-09-17 | The original routine is opaque; `ai::fitted` |
| AI leader and singleton return to base: the private landing route a wingman flies | Fitted, agent choice 2026-09-17 | B48 closes this for a wingman with an AI leader only; `ai::fitted` |
| AI tactical re-evaluation cadence of 8, 6, 5 and 4 quarter seconds by level | Fitted, agent choice 2026-09-17 | The spec forbids a per-tick reroll but does not give the cadence; `ai::fitted` |
| AI host maneuver state numbers 19 and 20 | Opinionated, agent choice 2026-09-17 | B46 accepts 19 and 20 and rejects the rest; the original's state names are unknown, so the host only ever produces accepted numbers; `ai::fitted` |
| AI control deflection, turning bank and throttle mapping from a B44 attitude request | Fitted, agent choice 2026-09-17 | The spec bounds the attitude, not the stick; named constants in `ai::steering_adapter` |
| AI loaded speed limits and G limit read from the flight model's own envelope block | Fitted, agent choice 2026-09-17 | The spec names "loaded envelope limits" without the query; `ai::mission` |
| Quick Mission AI enabled by default and separate delta formations per wing | Opinionated, requested by John 2026-09-17 | Player leads friendly wing 1; five other independent leaders. Compatibility remains under `--fixture-wings`; [mission wings](spec/quick-mission-menu.md#mission-wings) |
| Quick Mission wing placement and formation steering projection | Fitted, agent choice 2026-09-17 | 512 ft slot spacing, 4096 ft between wing leader offsets, level stacking and a three-second leader-heading projection. Exact rule and missing research in [mission wings](spec/quick-mission-menu.md#mission-wings) |

Diagnostic tooling is not a new gameplay feature. Document its purpose and limits
without pretending it is recovered retail behavior or a user-chosen flight law.

AI runtime boundary details have one home in
[the live integration rules](spec/ai.md#live-integration-and-authored-boundaries).

| Component | Provenance | Rule and limitation |
| --- | --- | --- |
| AI geometric completion tolerance | Fitted, agent choice | One degree; original B13 equality is stricter |
| AI attack-state producer for warning delay | Fitted, agent choice | Current target equals launcher; original state producers remain unknown |
| AI preparation readiness | Fitted, agent choice | Selected store's required emission/support gates replace the former universal radar-off flag |
| AI default inventory | Opinionated, agent choice 2026-09-17 | Import each aircraft's PT default loadout; the universal four-missile/500-round fit is synthetic only |
| AI weapon identity and launch envelopes | Spec-derived | Actor-owned record, imported range/angle/altitude/class/support fields; host geometric projection is fitted |
| AI representative projectile count | Fitted, agent choice 2026-09-17 | One projectile per imported release, separate from imported actual-round debit |
| AI compatibility missile steering | Fitted | Owned weapon movement and actor emission; no full AI seeker activation or pitbull |
| AI device timing and decoy rolls | Spec-derived | Individual quarter-second releases and independent susceptibility-times-effectiveness rolls |
| AI decoy presentation and aftermath | Fitted, agent choice 2026-09-17 | Glint and unguided coasting, with constants in the live integration rules |
| AI achieved attitude and damaged authority coupling | Fitted, agent choice 2026-09-17 | Enforce B44 attitude after model stepping; health scales G and roll linearly; normal commands select the capped other-state branch |
| Quick Mission home airport | Fitted, agent choice | Spawn point until the mission supplies an airport |
| Player wing keyboard shortcuts | Opinionated, agent choice 2026-09-17 | Documented in the input guide; recipient scope is friendly wing 1 |
