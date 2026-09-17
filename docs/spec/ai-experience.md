# AI experience and behaviour families

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-17. Partial specification from the local Fighters
Anthology media and static executable inspection. No AI implementation or retail
flight comparison is claimed. Build identity and validation live in the
[research baseline](../baselines/ai-research.md); addresses and data contracts
live in [AI source notes](../formats/ai.md). Delivery stages live in
[M1e](../ROADMAP.md#1e-ai).
The [main AI specification](ai.md) defines behavior and proposed host API inputs;
this companion owns experience numbers and the ported roster.

## Experience channels

An individual aircraft has one experience level: 0 Novice, 1 Average,
2 Experienced, 3 Ace. These are not four independent abilities or four different
aircraft models. Mission-wide friendly-air, enemy-air, friendly-ground and
enemy-ground selections are separate assignment channels. Preserve those channels
and the resulting per-object level separately. Do not equate either with player
flight difficulty, nationality, equipment capability or radar generation.

The manual describes each Quick Mission wing's selection as a range of skills;
the generator chooses the exact skill for each pilot. The FA Quick Mission
distribution is **unknown** in this pass. Static tracing now establishes that
the wing text writer emits its selected skill argument for every member.
The writer-to-loader path must explain the manual's promised variation before
we claim the final runtime distribution. Do not make every aircraft in a wing
identical by silently copying the menu selection, and do not substitute the
editor's distribution below without evidence.

The mission editor's bulk assignment uses a one-level downward adjustment for
33 of 100 possible draws, no change for 35, and an upward adjustment for 32.
Clamp the result to 0 through 3. This gives the following nominal distribution:

| Selected level | Novice | Average | Experienced | Ace |
| --- | ---: | ---: | ---: | ---: |
| Novice | 68% | 32% | 0% | 0% |
| Average | 33% | 35% | 32% | 0% |
| Experienced | 0% | 33% | 35% | 32% |
| Ace | 0% | 0% | 33% | 67% |

These percentages express draw thresholds, not a promise to copy the original
random sequence. Whether every mission-loading path preserves the saved skill
without reassignment remains to be checked. Explicit per-object mission values
must remain distinguishable from generation settings in the future data model.

## Fighter tactical choices

The shipped fighter source has these experience-dependent decision thresholds.
The corresponding packed constants also occur in its compiled module. These
are conditional choices at the stated decision point, not probabilities per
simulation tick or promises about how often a maneuver occurs in a whole fight.
Geometry predicates are in the main spec; compiled branch correspondence remains research.

| Situation or choice | Novice | Average | Experienced | Ace |
| --- | ---: | ---: | ---: | ---: |
| Prefer best attack, target ahead and facing | 11% | 20% | 74% | 84% |
| Otherwise choose random tactic in that situation | 50% | 34% | 14% | 6% |
| Prefer best attack, target ahead and facing away | 8% | 20% | 72% | 84% |
| Otherwise choose random tactic in that situation | 72% | 40% | 12% | 6% |
| Prefer best attack, target behind and facing | 16% | 20% | 72% | 90% |
| Otherwise choose random tactic in that situation | 42% | 20% | 10% | 4% |
| Prefer best attack, target behind and facing away | 16% | 20% | 72% | 84% |
| Otherwise choose random tactic in that situation | 42% | 20% | 14% | 6% |
| Fly straight on entering the random-tactic menu | 52% | 24% | 0% | 0% |
| Pursuit-point vertical displacement when chased | 35% | 50% | 75% | 95% |

The second choice is reached after the first fails. For a target ahead and
facing, the nominal combined probabilities are therefore 11% best attack,
44.5% random and 44.5% remaining tactics for Novice; 84%, 0.96% and 15.04%
for Ace. These numbers describe this branch only. Earlier decisions can bypass
it. A Novice with a target behind and facing has an earlier 40% straight-flight
choice before that situation's best/random choices.

In ordinary fighter pursuit, each initial target-offset component is drawn
from 0 through 99, 49, 29 or 9 respectively. This is pursuit-point variation,
not a universal gun-dispersion rule. A head-on condition can zero all three
components, and the chased condition can replace the vertical component with
+2000 or -2000 feet. The source requests nominal durations of 6 seconds below
a target distance of 5000 feet and 12 seconds otherwise. The main spec owns
the [timing limits](ai.md#b13-basic-maneuvers-and-command-limits) and
[pursuit frame and speed regulation](ai.md#b15-pursuit-reference-frame-and-speed-regulation);
weapon lead, sign conventions and interruption still need closure.

## Other experience effects

Aircraft field update applies an experience-dependent available-G adjustment
to levels 0 and 1, behind an exemption flag whose human-control meaning still
needs verification. It subtracts 1 G from the positive limit with a 2 G floor
and adds 1 G to the negative limit with a -2 G ceiling. Levels 2 and 3 skip
that adjustment. Treat the exemption as **unknown** until traced; do not change
player flight limits based on this finding.

A launch-reaction path uses nominal thresholds of 35%, 50%, 75% and 90% to
schedule device release. The successful path requests 2 or 3 releases. The
weapon service consumes this request and calls the device-launch routine.
This is stronger evidence than the older checkout's unconnected hypothesis,
but event eligibility, chaff/flare selector mapping and inventory effects remain
**unknown**. Successful device launches schedule the next request one
quarter-second later; a failed launch disables this schedule. The initial
request uses a bounded draw with bound 1, which is zero, so it can become due
on the current clock count. Service scheduling still determines when it runs.
The same successful reaction postpones a finite weapon-service deadline by
2 seconds. These timings use the clock established in the main spec.
It is not a universal probability of
evading a missile or surviving a shot.

No general multiplier for radar range, weapon damage or aircraft speed has
been established from experience. Do not invent one under a retail label.

## Currently ported aircraft

The initial AI implementation and acceptance scope includes all twelve aircraft
in `tore-formats::aircraft::AircraftId::ALL`, checked on 2026-09-17. Each exact
FA PT record below names `f.BI` through `ctName`. The common fighter/strike
family supplies behavior choices; each aircraft retains its own flight limits,
equipment, sensors, fuel and compatible stores.

| Ported aircraft | Exact retail definition | AI family |
| --- | --- | --- |
| F/A-18D Hornet | `F18.PT` | Fighter/strike |
| Rafale C | `RAFALE.PT` | Fighter/strike |
| F-14D Tomcat | `F14.PT` | Fighter/strike |
| A-4E Skyhawk | `A4E.PT` | Fighter/strike |
| X-31 EFM | `F31.PT` | Fighter/strike |
| MiG-29 Fulcrum-C | `MIG29.PT` | Fighter/strike |
| Su-27 Flanker-B | `SU27.PT` | Fighter/strike |
| MiG-21 Fishbed | `MIG21.PT` | Fighter/strike |
| Su-25 Frogfoot-A | `SU25.PT` | Fighter/strike |
| MiG-23 Flogger-B | `MIG23.PT` | Fighter/strike |
| Su-35 | `SU35.PT` | Fighter/strike |
| F-22A Raptor | `F22.PT` | Fighter/strike |

AI acceptance must cover all 48 aircraft/experience combinations, with eligible
scenarios selected from each aircraft's actual capabilities. A shared family
does not give A-4E or Su-25 afterburners, supply absent weapons or change X-31
and F-22 departure behavior. Mission role remains separate from family: Su-25
uses the fighter/strike family, not a guessed bomber controller.

These aircraft are already ported; their AI controllers and live hookup are
pending. The [additional-aircraft spec](additional-aircraft.md) and
[REDFOR/F-22A spec](roster-aircraft.md) retain their existing handling limits.
Reconcile this table with `AircraftId::ALL` when the ported roster changes.
Broader retail family recovery below does not require importing every retail
aircraft before these twelve receive AI coverage.

## Wider retail behaviour coverage

The FA catalog binds aircraft to families, rather than to a separate program
for every experience level. Counts include catalog variants and unusual records;
they do not mean those aircraft are flyable in T.O.R.E.

| Family | FA aircraft definitions | Behaviour to specify |
| --- | ---: | --- |
| Fighter/strike | 99 | Pursuit, merge, defensive maneuvers, attack runs and wingman cooperation |
| F-117 | 1 | Fighter-family air combat and its distinct ground-attack pass |
| Helicopter | 18 | Helicopter pursuit, evasion and ground attack, without assuming fighter aerobatics |
| Bomber | 6 | Ground attack and evasion; source air-to-air handler exits |
| AC-130 | 1 | Repeated circling attack and evasion |
| Large aircraft | 7 | Hit and evade responses |
| Airliner | 11 | Hit and evade responses |
| Special MOTH family | 2 | Fighter-family behavior with altered attack/egress rules |

F18.PT remains F/A-18D and RAFALE.PT remains Rafale C. Both bind to the fighter
family. Behaviour-family membership never authorizes substituting variants.

Surface behaviour is a separate specification task. The hydrofoil source binds
to SARAN.NT and attacks ships only: approach until horizontal distance is at
most 5000, depart on target bearing plus 80 through 99 degrees until at least
10000 feet away, then command a left 90-degree turn and repeat. The shared
horizontal-distance evaluator establishes feet; complete surface turning and
engine target selection still need confirmation.
This program does not establish SAM, anti-aircraft gun, tank, other ship or
carrier behaviour. Most surface records have no named maneuver program.

## Unknowns before complete implementation

Recover observable maneuver trajectories, durations, interruption rules,
target selection and loss, firing eligibility, missile support, ammunition and
fuel responses, formation orders, route behavior and return-to-base behavior.
Keep all four experience levels in each applicable scenario. Surface experience
effects must be demonstrated in surface consumers; aircraft tables do not
automatically apply there. The [source notes](../formats/ai.md) give the next
trace for each gap. Stop tracing when the player-visible rule can be specified.
