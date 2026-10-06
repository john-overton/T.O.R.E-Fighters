# Host selection: the CPU threshold, the upload test and the reach test's bytes (stage K, slice K6)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Measured on 2026-10-05 on the development machine (Ryzen 9 7900X, 24
threads, Linux, other agents' builds running beside it), branch
`mp/k6-select`. The design is [host selection](../ARCHITECTURE.md#host-selection);
the code is `tore_session::host::succession`, `tore_session::client::candidate`
and `tore_net::peers`.

## The CPU threshold (fitted)

The design asks for a threshold on each game's **CPU measure** (the lobby's
mission built once more and stepped 240 ticks, all AI) such that the busiest
minute of the real-data 15 against 15 mission stays under half of one core:
4,166 microseconds a tick at 120 ticks a second.

`host::succession::tests::cpu_measure_against_the_busiest_minute` (ignored;
release; the same mission as the [checkpoint baseline](checkpoint-2026-10-05.md#real-data-the-15-against-15-mission))
takes the measure, then flies the mission five minutes and times each minute:

| Run | The measure | Minute 1 | Minute 2 | Minute 3 | Minute 4 | Minute 5 | Busiest over the measure |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 1,434 | 1,224 | 498 | 480 | 501 | 467 | 0.853 |
| 1 | 1,786 | 1,490 | 515 | 455 | 437 | 424 | 0.834 |
| 2 | 1,658 | 1,343 | 602 | 500 | 461 | 433 | 0.810 |

Microseconds a tick. What that shows:

- **The first minute is the busiest**, at about three times the later ones
  (all 30 aircraft acquiring each other, then the missiles), as the
  checkpoint baseline found.
- **The measure overstates it slightly**: its first two seconds cost 1.2
  times the first minute's mean. The highest ratio, 0.85, is kept as
  `CPU_BUSY_PER_MILLE` (850): a measure up to **4.9 ms a tick** passes, and
  `cpu_percent` reports the predicted busiest minute against the budget.
- On this machine the measure is 1.4 to 1.8 ms, about a third of the budget.
  A machine three times slower still passes; one four times slower is
  warned about.
- The measure counts only the mission's own ticks. A host also sends each
  player's snapshots (0.1 to 0.2 ms a player a tick, [net baseline](net-2026-09-30.md)),
  which the upload test, not this one, stands for.

## The upload test on a throttled link

`host::succession::tests::an_upload_test_on_a_link_at_half_the_need_fails_and_at_the_full_need_passes`:
a game a player hosts and one other player on the network simulator, the
player's sends through a token bucket (the simulator's links have no rate of
their own). Two players need 38 KB/s: 28 KB/s for the other player and
10 KB/s for one standby.

| The link's rate | Arrived | Result |
| --- | --- | --- |
| Half the need (19 KB/s of datagrams) | 53.6 percent | Fails |
| The need and the packets' headers (1.05 times) | 100 percent | Passes |

The burst goes in packets of 1,100 bytes of Filler (1,123 bytes on the wire),
so headers are 2 percent of it; the half-rate link carries a little over half
because the bucket starts full (2,400 bytes).

## The reach test's bytes

`host::succession::tests::a_30_player_reach_test_is_bounded`: the worst case,
29 direct players and three candidates, every player with eight IPv6
addresses.

| What | Bytes |
| --- | --- |
| One Reach peers (28 players of 8 addresses) | 4,105 |
| One Reach test (3 candidates of 8 addresses) | 442 |
| One Reach report | 14 |
| The host sends (3 Reach peers, 29 Reach tests) | 25,133 |
| A candidate's Reaches and their answers (28 players, 8 addresses, 5 each) | 49,280 |
| A player's Reaches and their answers (3 candidates, 8 addresses, 5 each) | 5,280 |
| The whole test, across 30 machines | 326,499 |

A test runs at most once every 10 seconds. With the usual two or three
addresses a player, every figure is a third of these.

## The reach test along the punching table

`tore_net::peers::tests` runs every row of the [punching table](../ARCHITECTURE.md#hole-punching)
with the peers routers alone, and `host::succession::tests::the_eligible_rows_of_the_punching_table_are_the_punching_rows`
runs them end to end: a game a player hosts, the candidate behind the table's
host router and a player behind its player router, each game's peers router
in front of its joined socket. On every row the candidate is reached, and so
eligible, exactly when the row punches: the relay rows (a symmetric router
against one filtering by port, two symmetric routers, a carrier's router with
no IPv6) are not. A symmetric candidate is reached at the port its own Reach
came from, as a race reaches a symmetric host at the port its punch came
from.
