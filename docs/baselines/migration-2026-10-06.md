# Host migration and rejoin: acceptance and measurement (stage K, slice K10)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Measured on 2026-10-06 on the development machine (Ryzen 9 7900X, 24
threads, Linux, other agents' builds and windowed tests running beside it),
branch `mp/k10-accept` on `multiplayer` at `47461483` (the headless migration
scenarios passed again after a rebase onto `718f6233`), protocol 16, the
default snapshot rates (60 a second near, 4 far: slice D12). The design is
[host migration and rejoin](../ARCHITECTURE.md#host-migration-and-rejoin);
the target is the plan's stage K acceptance
([multiplayer plan](../multiplayer-plan.md#stages)): with real processes,
clients fly on within 5 seconds of the host's loss, missiles in flight
continue, the debrief keeps the kills from before, and a dropped player
rejoins its reserved aircraft. This is the lead's part of the acceptance;
John's three machines come after ([his checklist](#johns-checklist-three-machines)).

## In short

- **A host killed in a fight, warm standbys, real processes:** every pilot's
  snapshots come again 3.0 to 3.5 seconds after the kill on the real 15
  against 15 mission, and 1.9 seconds after it on the guide's mission (12
  aircraft). Within the plan's 5 seconds.
- **That needed one change (agent decision, K10).** Until now the new host
  held its clock for the full 1.5-second resume window waiting for the old
  host's own player, which was lost with the host it ran. On the real 15
  against 15 that put the pilots' snapshots 5.1 to 5.9 seconds after the
  kill. The window no longer waits for that player.
- **A host that leaves on purpose (handover):** snapshots again 0.33 to 0.48
  seconds after it on the real mission.
- **Cold standbys** (another system or processor type, as on John's three
  machines) could not be run as real processes on one machine. On the
  simulator the takeover on the real mission replays 15 seconds of journal,
  which took 2.3 to 2.7 seconds of real time; added to the warm figures that
  puts a cold kill on the real 15 against 15 at an estimated 5.2 to 6.7
  seconds, over the target. Small missions replay far faster.
- **The standby stream** at the new snapshot rates is unchanged: a warm
  standby 2.1 KB/s, a cold one 68 KB/s on average, its busiest second on the
  wire 119.4 KB/s against John's 1 Mbit/s.
- **Bugs found and fixed:** a player who joined after the last Succession
  could not follow a migration; the game that takes over named the other
  standby in its "Moving the game to" line; after the lobby moved to a
  better host the old King came back as a rejoiner without the crown.

## What was run

| What | How |
| --- | --- |
| The standby stream, real 15 against 15, four humans, a warm and a cold standby, 300 s | `TORE_DATA_DIR=<an import> K3_WIRE=1 cargo test --release --locked -p tore-session --lib real_data_15_against_15_with_a_warm_and_a_cold_standby -- --ignored --nocapture` (slice K3's test) |
| Takeovers on the simulator, real 15 against 15, four humans, warm and cold, a kill and a handover after 25 s of fight | `TORE_DATA_DIR=<an import> cargo test --release --locked -p tore-session --lib real_data_15_against_15_takeovers_warm_and_cold -- --ignored --nocapture` (new in K10) |
| Real processes, guide mission: K9's `net-migrate-kill`, `net-migrate-handover`, `net-migrate-relay`, `net-reach-upload` and K5's `net-server-rejoin` | `python3 tools/battery.py --scenario 'net-migrate*' --scenario net-reach-upload --scenario net-server-rejoin`, once with the release binaries (`--bin target/release/tore-app --bot-bin target/release/tore-bot --server-bin target/release/tore-server`) and once with the debug ones |
| Real processes, real 15 against 15: a hosting `tore-bot --host` (plane 0) and three `tore-bot --standby on` pilots (planes 1, 2 and 5), release, 40 s of fight then SIGKILL to the host (three runs), or the host's time running out (two handovers) | A driver script outside the repository: the mission is three wings of five F/A-18s against three of five MiG-29s, 10 nm apart at 10,000 feet (`tests/host_load.rs`'s), as mission text |
| Windowed (debug build): K7b's `net-window-rejoin` and `net-window-migrate`, and the lead's smoke test `net-window-migrate-smoke` (new in K10) | `python3 tools/battery.py --scenario net-window-rejoin --scenario net-window-migrate --scenario net-window-migrate-smoke --jobs 1 --windows 1`, each run alone |

## The standby stream at 60 and 4 snapshots a second

Four humans on the real 15 against 15 for 300 simulated seconds, a 40 ms
round trip, bots whose sticks move nearly every tick. "On the wire" is what
the host's transport sent the standby's game over what it sent a player who
stands by for nobody, so it carries the two players' differing snapshots too.

| Standby | Stream | On the wire, mean | Busiest second on the wire | Checks |
| --- | --- | --- | --- | --- |
| Warm | 2.12 KB/s (journal 4.4 bytes a seat a tick) | 5.7 KB/s | 38.0 KB/s | 60 of 60 equal |
| Cold | 68.1 KB/s (29 checkpoints of 0.56 to 1.09 MB, each sent in 7.96 to 11.31 s) | 81.1 KB/s | 119.4 KB/s | |

Both standbys together: 83.8, 71.5, 66.3, 60.7 and 69.8 KB/s minute by
minute. The figures are byte for byte the same at protocol 15 and 16. Against
slice KP's run at 30 and 2 snapshots a second
([baseline](standby-stream-2026-10-05.md#kp-the-framing-margin-re-measured)):
the warm stream 2.0 KB/s then, the cold 70.6 KB/s, the cold on the wire
83.5 KB/s with a busiest second of 128.0 KB/s. The stream itself does not
depend on the snapshot rate; only the warm standby's busiest second on the
wire rose (25.8 to 38.0 KB/s), from the doubled snapshots of a furball
second, which the over-a-player comparison cannot separate out.

## The takeover's times

"After the kill" is from the SIGKILL (or the handover's start) to each
pilot's first snapshot from the new host. A client notices the loss after
1.5 seconds without a packet; the new host goes live once it has stepped
from T to the present (the fast-forward).

### Real processes, warm standbys

| Mission, build | Case | Taken over | Live after the takeover | Fast-forward | Snapshots again after the kill |
| --- | --- | --- | --- | --- | --- |
| Guide, release | kill, resume window waiting (before K10) | 1.5 s | 1.76 s | 390 ticks | about 3.3 s (1.78 s after noticing) |
| Guide, release | kill | 1.5 s | 0.34 s | 221 ticks | about 1.9 s (0.41 s after noticing) |
| Guide, release | kill, a relayed bot in the game | 1.5 s | 0.37 s | 224 ticks | about 1.9 s (0.40 to 0.43 s after noticing) |
| Guide, release | handover | at once | 0.25 s | 33 ticks | 0.25 to 0.31 s |
| Guide, debug | kill | 1.5 s | 0.91 s | 290 ticks | about 2.5 s (1.02 s after noticing) |
| Real 15 v 15, release | kill, window waiting (before K10), three runs | 1.55 s | 3.25 to 4.18 s | 570 to 681 ticks | 5.08, 5.71, 5.94 s |
| Real 15 v 15, release | kill, three runs | 1.55 s | 1.18 to 1.40 s | 321 to 349 ticks | 3.05, 3.15, 3.48 s |
| Real 15 v 15, release | handover, two runs | 0.1 s | 0.24 s | 34 ticks | 0.33, 0.48 s |

In the real 15 against 15 kills the fight had 11 aircraft kills and two to
five guided missiles in flight; the new host's first world line held the
same kills and missiles. The fast-forward cost about 2.8 to 3.4 ms a tick on
the new host (a bot running its host and its client on one thread, the
machine shared), so it gains on the present only about 2.5 times faster
than real time: every second the new host waits before stepping costs about
0.7 seconds more of catching up. That is why the 1.5-second wait for a
player who cannot come back cost more than 2 seconds in all.

### The simulator, warm and cold, real 15 against 15

Simulated time on a 40 ms round trip, so a replay and a fast-forward cost
no simulated time; their real cost is beside them.

| Case | Taken over | Live after the takeover | Snapshots again after the loss | Replay at the takeover (real time) |
| --- | --- | --- | --- | --- |
| Warm kill | 1.515 s | 0.144 s (199 ticks) | 1.66 to 1.69 s | none |
| Warm handover | 0.023 s | 0.148 s (20 ticks) | 0.18 to 0.21 s | none |
| Cold kill | 1.515 s | 0.144 s | 1.66 to 1.69 s | 1,804 ticks in 2.34 s |
| Cold handover | 0.023 s | 0.148 s | 0.18 to 0.21 s | 1,805 ticks in 2.67 s |

Before K10's change the warm kill was live 1.503 s after the takeover with
362 ticks fast-forwarded, snapshots again 3.03 to 3.04 s after the loss.

**A cold standby, estimated.** The cold standby replayed 15 seconds of
journal, not the 10 the checkpoint cadence suggests: a 0.6 to 1.1 MB
checkpoint takes 8 to 11 seconds to send at its pace, so the newest complete
one is 10 to 21 seconds old (1,200 to 2,500 ticks, 1.6 to 3.5 s of replay
at 1.3 to 1.5 ms a tick). Its real time comes on top of the warm figures,
and the fast-forward must then also make up the replay's seconds: with the
replay at 2.5 s and a fast-forward tick costing 1.5 to 3 ms, every pilot's
snapshots would come again about 5.2 to 6.7 seconds after a kill, and 3.4 to
4.4 seconds after a handover, on the real 15 against 15. A cold standby
in a furball passing 5 seconds is what the design expected
([standbys](../ARCHITECTURE.md#standbys)); a mission of the guide's size
replays much faster. Not run as real processes: a cold standby needs a game
on another system or processor type.

## The plan's acceptance

| Item | Result |
| --- | --- |
| Clients fly on within 5 s of the host's loss, real processes | Passed with warm standbys: 1.9 s on the guide's mission, 3.0 to 3.5 s on the real 15 against 15 (after K10's change; 5.1 to 5.9 s before it). Every pilot's own aircraft flies on throughout. Cold standbys: estimated over 5 s on the real 15 against 15; for John's machines |
| Missiles in flight continue | The simulator's dogfight test checks that the missiles in flight at T end on the new host, the world at T equal to the old host's to the byte; with real processes the new host's world carries the missiles in flight on (the 15 against 15 runs: 2, 2 and 4 in flight in the new host's first second) |
| The debrief keeps the kills from before | `net-migrate-kill` and `net-migrate-relay` check that the new host's Results hold at least the old host's kills (passed, release and debug); the 15 against 15 runs' new host held all 11 |
| A dropped player rejoins its reserved aircraft | `net-server-rejoin` (a server), `net-window-rejoin` (the game, in the window) and the smoke test (a bot dropped from the game's new host after a migration, back through the master with its token) |

## The lead's smoke test

`net-window-migrate-smoke`: the game hosts in the window, listed on a
`tore-master` on this machine; two pilots that stand by join it directly, a
bot joins through the master's relay, and Viper keeps its rejoin token in a
file. The hosting game is killed in the fight; a pilot takes over, the master
moves the relay channel, and Viper, killed after the migration and started
again through the master with its token, is welcomed back into plane 4.

Passed (debug build, one window, 159 s). The hosting game was killed at tick
3244; Pilot1 took over at once from its warm copy ("replayed 0 ticks"), 5
players expected back. Every HUD line was right, Pilot1's own included
("Lost contact with the host. Moving the game to Pilot1..."). Pilot1, Pilot2
and Viper resumed 293 to 310 ms after the takeover and the relayed bot 526
ms after it (its first datagrams go through the master); the new host was
live 1,078 ms after the takeover with 310 ticks fast-forwarded; every
player's snapshots came again 0.95 to 1.25 s after it noticed the loss. The
master moved the listing and its one relay channel to the new host. The old
house was dropped 5 s after the takeover with its plane kept; Viper, killed
after the migration, was dropped the same way, sent its token through the
master, read "Welcome back, Viper: your aircraft is waiting." and was seated
in plane 4 again.

The other windowed runs, one window at a time: `net-window-rejoin` passed
(the game killed in flight, the server saw it go silent, kept plane 0 and
welcomed it back); `net-window-migrate` passed (the game's log and HUD say
"Lost contact with the host. Moving the game to Pilot1..." and "The game
moved to Pilot1." 0.16 s apart). In that run the game itself resumed only
2.47 s after the takeover although it had joined the new host 0.16 s after
the loss: the resume window waited its full 1.5 s for it, so the pilots' snapshots
came 2.35 s after they noticed the loss. The game ran on a spare workspace
that is never shown, where its frames may be held back; slice K7b measured
the same 1.5 s. A game on a visible screen was not timed here: John's run
will show it.

## Bugs found and fixed

- **A late joiner could not follow a migration** (found by slice D11 in
  `net-reach-upload`). The host sent its Succession (the ready standbys and
  their addresses) only when it changed, to the players connected then; a
  player who joined or resumed later never heard of the standbys and dropped
  5 seconds after the handover. The host now sends the last Succession to
  every player as it connects. Simulator test `a_late_joiner_follows_a_migration`;
  `net-reach-upload` now asks that the bot that does not stand by follows the
  handover.
- **The game that takes over named the other standby** ("Lost contact with
  the host. Moving the game to Pilot2..." on Pilot1's own HUD), because its
  race leaves its own address out and the words took the race's first
  target. The words now name the succession's first standby.
- **After the lobby moved to a better host, the old King lost the crown.**
  `Host::resume` dropped the old house as having left for every handover, so
  after a lobby move its player came back through its token as a rejoiner,
  and the crown had passed on. A handover in the lobby now keeps the old
  house expected back (agent decision); if it left the lobby instead, it is
  dropped at 5 seconds as having left. Simulator test
  `a_lobby_handed_over_keeps_the_old_house_and_its_crown`.
- **The resume window waited for a player who could not come back** (above;
  agent decision).

## Known limits

- **No ready standby for about 15 seconds after a takeover** on the real 15
  against 15: the new host appoints two standbys at once, but each needs a
  checkpoint of about 1 MB at the paced rate (in one run they were building
  until 10 s after the takeover, cold at 15 s and warm at 20 s). A second
  loss in that time ends the game.
- **Cold standbys replay up to 20 seconds of journal** (above). Levers, none
  built: replay the journal in the background on a cold standby (it then
  costs a core like a warm one), a shorter cadence or smaller checkpoints.
- The fast-forward on a bot's single thread cost 2.8 to 3.4 ms a tick; the
  game runs its host on its own thread, which should be quicker, but was not
  timed here on the real mission.
- **A bot with no `--standby` runs no peers router**, so it answers no reach
  test, and a player who never reaches a candidate leaves that candidate
  ineligible: in a game a player hosts, one such bot means no standby is ever
  appointed. A real game always runs its router (its switch only says
  whether it may host), and `tore-bot --standby off` does too; the smoke
  test gives its non-standby bot `--standby off` for that reason.
- **Writing the smoke test showed that `net-window-rejoin` never killed its
  game** under Hyprland (the wrapper was killed, the game flew on and left
  cleanly); fixed in K10, both scenarios kill the game's own process.
- `net::session::keepalive_tests::a_joined_game_stalled_for_fifteen_seconds_is_kept_and_recovers`
  failed twice on this machine while other agents' builds ran (92 ticks
  repeated against its 120) and passed three times alone; it is timing
  sensitive and was not changed.

## John's checklist (three machines)

The same build on all three machines, each with its own import. Machine A
hosts; B and C join. Leave **Let my game take over hosting** on (Options, the
default) on B and C. On three different systems every standby is cold, so
expect the takeover to take longer than on one machine: a few seconds on a
small mission, possibly more than 5 on a crowded one.

1. **Set up.** A hosts a mission with some AI (the guide's mission is fine)
   from Direct Connection or the Internet Lobby. B and C join and take
   planes. In the lobby, select B or C: one says "Stands by to host:
   first." and shows the outlined house.
2. **One relayed player, if you can.** Start C's game with
   `TORE_JOIN_PATH=relay` in its environment and join A through the Internet
   Lobby: C's Messages say "Connected through the relay." A relayed player
   never stands by, so B takes over in step 4.
3. **Fight.** Fly into the fight until someone has a kill and a missile is
   in the air.
4. **Kill the host.** End A's game hard: `kill -9`, Task Manager's End task,
   or Force Quit. Do not use the menus.
   - Within about 1.5 seconds B and C read "Lost contact with the host.
     Moving the game to B..." on the HUD (B names itself), then "The game
     moved to B."
   - Your own aircraft never stops. The other aircraft hold still and then
     jump on, within 5 seconds of the kill if all goes to plan.
   - Missiles in the air keep flying and can still hit.
   - Note roughly how many seconds the others held still.
5. **Rejoin.** Start A's game again with the same data folder. Direct
   Connection (or the Internet Lobby, once the game is selected) marks the
   game **Rejoin**. Join: "Welcome back, A: your aircraft is waiting." and A
   is back in its own aircraft, which the AI flew meanwhile (the lobby shows
   "AI (A away)"). The token lasts 24 hours and the aircraft must still be
   alive.
6. **The debrief.** At the mission's end, the Results keep the kills from
   before the migration.
7. **Send back.** The seconds in step 4, anything unexpected, and B's and C's
   `logs/tore-*.log` and `logs/net-*.tsv`. "No other game could take over"
   means no standby was ready: say how long the mission had flown.
