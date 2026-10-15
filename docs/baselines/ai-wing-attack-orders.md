# Wingman ground attack orders

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation validation for nr-a7atk, based on b35285cb, protocol 34,
2026-10-15. Retail comparison was unavailable. This is not retail evidence.

John's recording names Truck L79, object `0x4000004f`, in Kurile. In the
matching imported scene it is hostile side 2, class `0x0200`, 50 hp. The
existing order resolver excludes neutral and friendly ground objects.
The old recording confirms acceptance and zero wingman launches, but does not
contain the strike controller's selected weapon or release solution. Those
were investigated by source inspection and headless reproduction.

| Run | Result |
| --- | --- |
| Original recording, order at 174.317 s to Disengage at 654.5 s | Both wingmen accepted, neither launched |
| Diagnostic class-order run, two default A-7Es, 240 s | Mk 82 straight passes, zero releases; sampled closest cues about 97 and 70 ft from the Truck |
| Fixed class-order run, three-ship flight, 10,000 ft, Ace | Both wingmen launch AGM-65Gs at 1.125 s; first hits for 50 damage and destroys the Truck at 7.175 s; second reaches the wreck at 7.2 s |
| Fixed bomb-only class order, 7,800 ft, Ace, near the original order geometry | Level re-attack releases at 105.450 s; the bomb destroys Truck L79 at 130.808 s |
| Separate level Mk 82 probe, 10,000 ft, Ace | Release at 22.192 s; target falls from 50 to 20 hp; final miss 402 ft with unchanged scatter and prediction |

These are not identical flight-input replays. The diagnostic watched aircraft
started at 7,800 ft near the original order geometry; its second wingman began
at the QMC spawn. The fixed regression puts both wingmen beside the lead at
10,000 ft near the target. It begins near the attack order, not at the start
of John's 20-mile flight. Both diagnostic and fixed class-order runs remove
the two static ZSUs, already destroyed before John's recorded order.

Reproduce the fixed regression through battery scenario
`ai-wing-a7-attack-vehicles`. Its `--record {work}/a7-attack-vehicles.tore-replay`
keeps both aircraft's launches and ground decisions without rendering. Scenario
`ai-wing-a7-bomb-vehicles` removes the watched A-7's Mavericks and checks its
level bomb release. See the [driver](../testing/lane-flight.md#ai-strike-driver)
and [order behaviour](../spec/ai-strike.md#orders-from-the-flight-lead).

Evidence is local only under
`.local/tmp-nr-a7atk/{before-class.txt,before-class.tore-replay,after-class.txt,after-class.tore-replay,after-log/}`.
The final test and guard results are in the slice report
`.local/next-round/reports/nr-a7atk.md`.

Known reporting limitation: the replay summary says zero surface hits/kills for
this run, although its detailed `weapon.outcome` and `surface.wreck` events
record the hit and owner. Use those events and the combat ledger as evidence.
