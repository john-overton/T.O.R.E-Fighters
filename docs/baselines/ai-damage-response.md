# AI damage response and rebase validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Build and integration

Implementation mode, 2026-09-28. Work remains in
`/home/john/Development/T.O.R.E-Fighters-ai-awareness`, branch
`fix/visual-awareness-under-fire`.

The AI branch was clean and already committed before this task. Its final base
is main `1166af669847cb13d311b18ddb431beadac0bd7f`, including Realistic NPC
system damage from `57d83cd` and the player's Normal/Realistic distinction.
The prior AI tip is preserved locally as
`backup/ai-awareness-before-damage-rebase-20260928`. The awareness, research and
gun commits are now `ed38ef5`, `4ab8fe5` and `30bb8ff`.

The single conflict was adjacent gun-contact test code in `combat/live.rs`.
Both the physical gun tests and main's cheat-aware cockpit-hit helper were kept.
The initial integration on `57d83cd` passed 1,844 Rust tests before fault-response
changes. Main's later release-documentation update rebased cleanly and changed no
Rust code in the validated response. Full checks were repeated on the final base.

The response implementation is the change containing this baseline. Pre-commit
probe headers identify its earlier rebased gun parent `4dca289`. Behavior and all
agent-authored constants live in the
[systems damage contract](../spec/systems-damage.md#ai-pilot-response-to-faults).
No HP capacities, damage milestones, fault rolls, repair mechanics or dummy
flight behavior were changed. The existing once-per-second 70% ejection rule
is preserved, including its per-pilot random stream.

## Component and integration evidence

Nine new tests cover the policy and actual flight:

- Component state chooses the response. A treated pilot wound stops being an
  active wound despite its historical fault count. Equipment-only faults retain
  their existing capability gates.
- Forty seconds at protective throttle preserves the damaged compressor's
  remaining power. Full throttle exhausts its existing 30-second exposure budget.
  Oil-pump damage heats at less than half the full-throttle rate in this fixture;
  reducing throttle does not repair or cool away the underlying fault.
- A flameout follows the existing six-second timer and throttle cycle; permanent
  zero power never receives a fictional successful restart.
- Positive and negative requested structural loads stay below the existing
  weakened-structure threshold in the component checks.
- A wounded F/A-18D begins recovery, drops offensive selection, rejects renewed
  attack/formation orders, accepts an explicit landing destination and preserves
  it through Hold Fire. It lands and parks alive within 850 seconds, before the
  first wound's 900-second deadline, and receives the existing ground treatment.
- Compressor power protection and low-speed exceptions run through both flight
  adapters. A jammed lever ignores the pilot's protective request physically.
- Faulty departures hold idle and brakes. A dummy with engine, fire, destroyed
  wing and pilot faults keeps exactly its fixed velocity for ten seconds and
  never enters recovery or ejection.
- Airborne fire uses the existing ejection monitor: no immediate ejection, then
  a successful first one-second draw with the fixture's known seed. The same
  grounded fire does not eject. Seats are explicitly present in this fixture.
- All 14 selectable profiles, in legacy and researched flight, react to a wound
  and an anonymous hit: defend first, then resume recovery with weapons held.
  With no known runway or home position they do not invent one or acquire a
  hidden target. The test also checks their requested G limit.

The first landing exercise exposed a control-policy problem: a 30-degree bank
limit at the old high approach speed made very wide turns. The documented
recovery speed and airbrake policy corrected that; the passing test validates
actual landing and wound treatment, not just a Returning to base label.

## Imported headless runs

Twelve 90-second probes, 10,800 fixed simulation ticks each, with F22.PT as the
player and two Average SU27.PT enemies. Eight controlled-fault runs inject at
10 seconds, using the ordinary target fault-count delivery bridge. They start
50 nautical miles apart with AI guns-only stores, so the fault response can be
observed separately from close combat. This injection does not prove a fault's
combat probability. All twelve recordings pass verification at all 10,801
snapshots, including tick zero, with no missing or differing frames.

| Run | Adapter | Observed result at 90 seconds |
| --- | --- | --- |
| Pilot wound, fault 34 | Researched | Recovery starts on the next tick, 10.008 s, weapons held; alive and inbound to the known runway |
| Pilot wound, fault 34 | Legacy start | Same response; landing uses the existing transition to researched flight |
| Compressor, fault 7 | Researched | Alive and inbound; remaining power 75%, actual throttle 9.7%, no compressor failure after 80 seconds |
| Oil pump, fault 12 | Researched | Inbound, oil pressure 50%, temperature 14.084%, full remaining power, actual throttle 9.9% |
| Flameout, fault 4 | Researched | Engine restarts; full remaining power at the end, recovery continues |
| Weak structure, fault 30 | Researched | Alive and inbound with restricted maneuver requests; no structural failure |
| Engine fire, fault 11 | Researched | Pilot ejects at 11.000 s, one second after injection; aircraft loss is recorded |
| Throttle jam plus compressor, faults 29 and 7 | Legacy start | Lever remains at 100%; compressor fails and available power reaches zero. Policy does not override the jam |
| Guns, no injected fault | Legacy | 121 physical gun rounds, 46 hits, one kill across the encounter; no shooter self-hits |
| Guns repeat | Legacy | Same events and all 91 shared state checksums match |
| Mixed missiles and guns, no injected fault | Researched | Four missiles plus 16 physical gun rounds; real combat faults and aircraft losses recorded |
| Healthy distant control | Researched | No injected fault or spurious damage-response transition |

The player's Normal damage mode and AI Realistic damage are those inherited
from main. They differ from the older gun baseline, so its damage/kill counts
are not an acceptance target for the combined branch. The repeat on the combined
build has identical launches, hits, kills, outcomes, AI decisions, communications
and aircraft/system events. Only wall-clock recording time differs.

Local evidence: `.local/ai-damage-response/`, including `run_probes.py`, per-case
`.log`/`.tore-replay` files, exported `log.jsonl`/`summary.txt`, and
`determinism.log`. The short `display` probe separately records the current
human-readable power policy and passes 1,801 snapshot checks. No imported media,
recordings, exports or captures are committed.

## Reproduction and required checks

From the AI worktree:

```sh
TORE_DATA_DIR=/home/john/Development/T.O.R.E-Fighters/.local/dev-profile \
  target/debug/tore-app --ai-probe-ticks 10800 --aircraft f22 \
  --probe-enemy-aircraft su27 --probe-enemy-skill average \
  --probe-flight-model researched --separation 50 --probe-ai-guns-only \
  --probe-fault 1200:7 --record-mission NEW_PATH.tore-replay \
  --verify-render --no-audio
```

Use `--recording-log FILE --out NEW_DIR` for events and thought records, or
`--recording-diff A B` for determinism. Fault identities and the fixture's bounds
are documented in [headless development](../DEVELOPMENT.md#visual-contact-and-incoming-fire-probes).

Validation on Linux x86-64: formatting, locked all-target workspace Clippy with
warnings denied, 1,853 Rust tests with eight existing ignored tests, locked
workspace build, 97 Python tool tests, source and both binary asset checks, and
documentation checks. Existing healthy-flight and combat golden expectations
were not rewritten. GPU smoke and the damage-response replay capture completed.

Windows/macOS execution, a human-flown damage-response fight, and a full imported
roster landing campaign were not run. Numerical AI response rules remain fitted.
No nearby-field diversion or emergency belly-landing procedure was invented;
missing/unreachable runways, jammed controls, fire and severe loss of lift or power
can still cause a loss. Imported 90-second wound probes validate recovery in
progress; the full landing/treatment check uses a synthetic runway.
