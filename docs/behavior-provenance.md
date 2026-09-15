# Behavior provenance and acceptance

User direction, 2026-09-15: prioritize recovering and implementing features that
exist in Fighters Anthology. A plausible simulator feature is not evidence that
FA contains it. This policy applies to implementation plans, coverage summaries,
format research and acceptance reports.

## Describe origin separately from completion

| Origin | Meaning | Required documentation |
| --- | --- | --- |
| Native | Behavior established from the reviewed original executable/resources or a controlled observation of retail | Source build/hash or linked source identity; routine/field/caller or recording; trigger, inputs, units, ordering and outputs; unresolved branches |
| Fitted | An implementation approximation we authored because a native contract is missing or because of host integration | Exact rule/constants, reason, known difference or uncertainty, runtime scope, and the native work it cannot close |
| User-directed opinionated | An intentional addition or change explicitly requested by the user | Request/date, intended departure from native behavior, scope/default and acceptance criteria; retain the native reference separately |
| Unknown | Insufficient evidence to establish existence, meaning or behavior | Missing evidence and next research step; keep unavailable/unimplemented where required |

“Authored” alone is ambiguous: say whether the choice is fitted or user-directed
opinionated. An implementation choice made by an agent is **not** user-directed
merely because the user requested the broader feature. Do not retroactively
attribute a fitted constant or design to the user. A user request to investigate
a feature is not permission to invent its native behavior.

A feature may contain several origins. Split the feature into components rather
than applying one “native” label to an entire hybrid system. Native assets or PT
parameters do not make the equations consuming them native. Native Rust code
means the host technology; it does not establish original-game provenance.

## Native implementation steps

Track these independently; a later step must not be implied by an earlier one:

1. **Source established:** identify the behavior, conditions, producers, consumers
   and exact build. State whether evidence is static code, resource data or retail
   observation. A symbol name alone is not a behavioral specification.
2. **Translated and tested:** implement the reviewed contract with explicit inputs,
   state, time and randomness. Test source-derived expected outputs and boundary
   conditions. Label diagnostic helpers as diagnostic.
3. **Runtime connected:** connect verified producers and consumers in the correct
   order. Record any fitted boundary that remains; keep movement, body attitude
   and presentation offsets distinct.
4. **Retail compared:** compare matched aircraft/loadout, inputs, conditions and
   outcomes with the original game; report differences and unavailable checks.

Synthetic tests can validate arithmetic and invariants. Deterministic replay
can validate repeatability. Neither establishes retail trajectory parity.
Static source recovery can establish expected branch behavior without running
native modules; it does not automatically establish a complete flight tick.

## Planning and reporting rules

- Current work focuses on native feature recovery. Do not add new fitted flight
  laws or gameplay effects to fill unknown behavior. Research the missing contract
  or list the gap. Existing fitted behavior remains explicitly identified; do not
  silently remove it, expand it or promote it to native acceptance.
- Keep native tasks and fitted/user-directed tasks separately labeled. A fitted
  substitute cannot close a native checklist item. Separate completed component
  work from incomplete end-to-end integration.
- Use `docs/formats/` for source contracts, the relevant guide/plan for integration
  and sequence, and `docs/baselines/` for methods and measured evidence. Link
  between them rather than duplicating specifications.
- Correct stale status claims in place. Historical measurements remain valid as
  measurements, but supersede misleading completion/acceptance descriptions.
- Scope “parity”: name the feature and remaining mismatch. Separate missing
  implementation from missing original-game comparison. Do not make exact legacy
  clock artifacts or cross-platform bit identity new product requirements when
  the roadmap excludes them; document numerical differences that affect behavior.

## Current flight examples

| Component | Origin | Current completion boundary |
| --- | --- | --- |
| Warning/stall timers, spin entry/recovery predicates | Native | Translated/tested; selected hybrid connections, with fitted initial stall classification |
| Stall control/lift attenuation | Native arithmetic | Connected in hybrid; clean-envelope reference speed and later force integration remain fitted |
| Timed warning-transition rotation (“tumble”) and stalled movement fall | Native source-backed research | Joined diagnostic stage tested with both PTs/imported tables; not enabled in either live adapter |
| Clean-envelope stall-entry gate | Fitted | Runtime; does not close native current-G/difficulty/device classification |
| Response filters, trim/alignment, continuous spin coupling | Fitted | Runtime; not original force/control-law acceptance |
| `sideslip_drag=0.5` in both aircraft models | Fitted | Added by the implementation, not requested as an opinionated change and not extracted from FA |
| Achieved G/applied-rate diagnostic snapshot | Authored diagnostic instrumentation | Measures our adapter; does not prove that retail exposes equivalent measured channels |
| Sustained controller rumble requested in the response plan | User-directed addition; mapping remains to be designed | Deferred while native behavior is recovered; native sound triggers and any authored haptic mapping need separate acceptance |

Diagnostic tooling is not a new gameplay feature. Document its purpose and limits
without pretending it is recovered retail behavior or a user-chosen flight law.
