# The host's standby stream: bytes before and after protocol 14 (stage K, slice K3)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Measured on 2026-10-05 on the development machine (Ryzen 9 7900X, 24
threads, Linux, other agents' builds running beside it), branch
`mp/k3-stream`. The design is [standbys](../ARCHITECTURE.md#standbys) and
[the journal](../ARCHITECTURE.md#the-journal-one-door-into-the-world); the
bytes are the [standby stream](../formats/net-protocol.md#the-standby-stream);
the code is `tore_session::host::standby` and `tore_session::journal`.

## In short

- **Protocol 13** coded a seat's input with the checkpoint trait against its
  last: each stick that moved cost about 8 bytes as a float, two empty
  command lists 2 bytes, the seat byte and the 16-bit command number 3. With
  bots' sticks that was 13 to 23 bytes a seat a tick: 53 KB/s to a standby
  with 30 humans, five times the 10 KB/s a standby that host selection
  allows.
- **Protocol 14** codes the controls as the Inputs section does (the wire's
  quantized frame against the last, every input the host steps lying on
  that grid), the command lists behind one presence bit, the view as Inputs
  does and the command number as one bit when unchanged. 3.4 to 5.6 bytes a
  seat a tick: 12.8 KB/s with 30 humans, 2.3 KB/s with four.
- **Still over:** 30 humans pass the 10 KB/s a warm standby by about 28
  percent, and a cold standby's checkpoints need up to 1 Mbit/s on the real
  mission. Both are host selection's upload need to revisit (slice K6's
  `NEED_PER_STANDBY`).

## What was run

| What | Command |
| --- | --- |
| The crowd fight, three humans, a warm and a cold standby, 30 s (release) and 5 min (debug) | `cargo test --release --locked -p tore-session --lib a_warm_and_a_cold_standby_follow_the_crowd_fight -- --nocapture`; `cargo test --locked -p tore-session --lib a_warm_and_a_cold_standby_follow_a_five_minute_crowd_fight -- --ignored --nocapture` |
| Thirty humans in PvP, no kill limit, a minute (release) | `cargo test --release --locked -p tore-session --lib the_stream_of_thirty_humans_is_measured -- --ignored --nocapture` |
| The real 15 against 15 mission, four humans, 5 min (release) | `TORE_DATA_DIR=$PWD/.local/DATA cargo test --release --locked -p tore-session --lib real_data_15_against_15_with_a_warm_and_a_cold_standby -- --ignored --nocapture` |

Every run is on the network simulator with a 40 ms round trip; the bots fly
`bot::ScriptedPilot`, whose stick moves nearly every tick, so the figures are
near the top of what people's sticks cost. "On the wire" is what the host's
transport sent the standby's game a second over what it sent a player who
stands by for nobody: the stream with its framing and resends, give or take
the two players' snapshots. The protocol 13 runs were made before slice
K3 rebased onto K6 (their rates over the whole run, their lobby seconds a
few); the protocol 14 runs count from when every player flew.

## Before and after

| Run | Protocol | Bytes a seat a tick | Warm standby | On the wire | Cold standby | On the wire |
| --- | --- | --- | --- | --- | --- | --- |
| Crowd fight, 3 humans, 5 min | 13 | 12.7 | 4.6 KB/s | 7.9 KB/s | 10.2 KB/s | 13.2 KB/s |
| | 14 | 4.1 | 1.5 KB/s | 3.9 KB/s | 7.2 KB/s | 9.4 KB/s |
| 30 humans, PvP, 1 min | 13 | 14.7 to 18.1 | 53 KB/s | | 70 KB/s | |
| | 14 | 3.4 | 12.8 KB/s | 16.2 KB/s | 28.9 KB/s | 34.6 KB/s |
| Real 15 v 15, 4 humans, 5 min | 13 | 19.0 | 9.4 KB/s | 10.4 KB/s | 70 KB/s | 81 KB/s |
| | 14 | 4.8 | 2.3 KB/s | 1.8 KB/s | 64 KB/s | 72 KB/s |

Where protocol 13's bytes went, seat by seat, on the 30-human stream: 3.0
bytes the seat and the command number, 9.5 the three sticks, 0.6 the
throttle, 3.0 the command lists, trigger, scope controls and view.

On the real mission the cold standby's checkpoints were 0.52 to 1.1 MB, each
taking 8.0 to 10.9 seconds (the furball's longer than the 10-second cadence,
so the next waited); its busiest second on the wire was 126 KB/s against
John's 1 Mbit/s, a few KB of it the two players' snapshots differing. Every
Check of every warm standby was equal.

## Not built: the players part

The players part goes whole each time any player's state changes: about 1.7
KB with 30 players, 0.3 to 3.3 KB/s in a 30-human fight depending on how
often players die and come back. Coding it per player against the last one
sent would cut it to the players that changed. Left as a follow-up: under
protocol 14 it is a tenth of the warm stream with 30 humans.
