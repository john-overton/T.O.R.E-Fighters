# Battery lane: net

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

The net lane plays multiplayer the way a player or an operator does, on one
machine, over real UDP on 127.0.0.1: a dedicated server with bots, the console,
the local-network search, a joined game that freezes for a few seconds, a
game that hosts, and the master server under its flood tool. It uses the same imported data as every other lane, so it needs
the game imported and the server and bot programs built. Its scenarios live in
`tools/battery_scenarios/net.py`.

The lane is for the things the Rust network tests cannot do: they run the host and
the clients in one process on a simulated network with synthetic data, while this
lane starts the real programs (`tore-server`, `tore-bot`, `tore-app`), feeds them
through their real command lines and console, and reads what they print and log.
The Rust tests that stay outside the battery (the five-minute matrix and the
loopback test) are listed in
[Network tests outside the battery](README.md#network-tests-outside-the-battery).

## Running it

```sh
cargo build --locked -p tore-app -p tore-server -p tore-session -p tore-master   # tore-session builds tore-bot
TORE_DATA_DIR=.local/bugbash-data target/debug/tore-app --import gameassets/fighters-anthology --import-only
python3 tools/battery.py --lane net --jobs 7 --windows 1 --tag net
python3 tools/battery.py --scenario 'net-server-*'              # headless only
python3 tools/battery.py --scenario net-window-stall --windows 1
```

The runner finds `tore-server` and `tore-bot` beside the game binary given by
`--bin` (by default `target/debug/`). `--server-bin PATH` and `--bot-bin PATH`
name them when they are elsewhere, for example in a release build. A missing
program is named before anything runs. The `net-master-*` scenarios take
`tore-master` from beside the game binary and fail with the build command
when it is missing.

The whole lane takes **79 seconds** of wall clock on the dev machine (Ryzen 9
7900X, debug build, 7 jobs, 1 window; 2026-10-05). The longest scenario is the
fight, 75 seconds of flight; the rest run beside it. Each scenario takes its own
ports (the runner hands out ports nothing on the machine is using), so they run in
parallel and a server left running elsewhere on 26900 is no trouble. The two
`net-window-*` scenarios open a window through `tools/agent-run.sh` and count
against `--windows`; the quick check leaves them out unless a change reaches the
game's own network code (see "Choosing scenarios by change").

## What each scenario checks

| Scenario | Seconds | What it does | What has to hold |
| --- | --- | --- | --- |
| `net-server-check` | 1 | `tore-server --check` on the example mission in [the server guide](../DEDICATED-SERVER.md#the-mission-file), read straight from the guide | Exit 0. The mission summary, all twelve planes in order (six friendly), the runway list and the content manifest line are printed |
| `net-server-fight` | 79 | A server and two `tore-bot` players fly a fight 5 nm apart for 75 seconds, then leave | Both join, are seated, fly (figures every five seconds), get a debrief and leave cleanly with exit 0. A bot fired at least one burst. The server logs the start, a status line with both players, both departures, "everyone left" and its own stop, and exits 0 by itself (`after-end quit`). Its log file in the data folder holds the same and a once-a-minute figures line for each player. No refusal, protocol error, bad packets, silence or fault anywhere |
| `net-server-chat` | 25 | Both bots send free text to all, to friendlies and to an empty enemy side, and a quick line from `CHAT.TXT` | Each line reaches the other bot with its sender and receiver, a line nobody can hear is answered "No one hears you", the quick line carries its sound tag, and the server log records every line with how many heard it |
| `net-server-kick` | 6 | Three bots fly; the driver types the console's `players`, `status`, `kick SEAT`, `kick-player ID REASON` and `end` | The players table shows each bot in its plane. The seat kick reads "kicked by the server" at the bot and "left: kicked" in the log; the id kick reads "The server removed you from the game: REASON"; `end` gives the player still flying a debrief and "Mission ended by the host", and the server stops |
| `net-server-observe` | 39 | Two bots fly a fight 5 nm apart for 35 seconds; once the mission flies a third, `--observe 0`, joins with no plane and watches for 20 seconds, its camera on plane 0 (stage F phase 2's observer stream) | The observer joins, its observer flight starts ("observing from tick N, 0 s behind"), the lobby marks it "no slot observing", it draws frames with aircraft in them, takes no seat and gets no debrief, and leaves cleanly with exit 0; the flyers are seated and exit 0; the server logs its join and its departure. No refusal, protocol error, bad packets, silence or fault anywhere |
| `net-discovery` | 5 | `tore-app --find-games` against a running server, then on a free port | The server's line (this build, name, mission, `0/6 players`, `lobby`, `king -`, `open`, `not full`) is printed on its port. A port with nothing prints `No games found.` and exits 0. The console's `quit` stops the server with exit 0 |
| `net-window-stall` | 17 | A game joins the server and its script blocks the whole main loop for 4 seconds in flight | The server logs "game stalled, flying neutral" and "game back after 4.0 s" (the keepalive held the seat), then a clean leave. The game wrote `logs/net-DATE.tsv` (header, join, mission, seating and once-a-second figures with the right columns) and a capture in `replays/` |
| `net-master-flood` | 11 | A `tore-master` on 127.0.0.1 (its probe port the main port + 1, status every 2 seconds), then `tore-master flood` at it for 10 seconds; the console's `status`, `listings` and `quit` | The flood exits 0 with "limits held": no port answered with more bytes than it sent, and every browse from 127.0.0.2 during the flood answered (at least 8). The master printed both ports, a `limit source=127.0.0.1` line, a status line with `dropped(limit)` above 0, `listings=0` (the flood made no listing) and `Stopped`; its `state/telemetry/DATE.tsv` counted none of the flood's reports |
| `net-window-host` | 41 | `tore-app --host` flies the example mission; the driver waits for it to listen, searches, joins a bot, and the host's script leaves after 20 seconds of flight | The search finds the hosted game by name with its King. The bot joins, is seated, flies, hears "Mission ended: the host left the game.", gets its debrief and exits 0. The host exits 0 and wrote the same net log and a `HOSTED` capture |

All of the lane's output is checked for the same general problems as every other
lane (panic, `NaN`, infinity, stack overflow, fatal error), across every process.

## How a scenario works

A net scenario sets `driver=` to a Python function instead of `args`. The runner
gives it a `Drive` ([tools/battery.py](../../tools/battery.py)) and does the rest
the way it does for a single run: the per-scenario copy of the data folder
(`d.data`), a work folder (`d.work`), the timeout (`--timeout-scale` applies to
every wait limit), the output capture and the final checks.

- `d.start(label, [d.server, ...], stdin=True, window=False)` starts a process in
  its own session. `d.app`, `d.server` and `d.bot` are the three programs.
  Everything a process prints is kept, line by line, and joined into the scenario's
  log as `[label] line`, so `Scenario.expect`, `forbid` and `check_work` see it all.
- The process handle can `send("quit")` to a console, `wait_for(regex, seconds)`,
  `finish(seconds, expect_exit)` (waits, and reports a process that does not end),
  `stop()`, and `expect(regex)` or `forbid(regex)` against what it printed.
- `d.run(label, argv)` runs a process to its end; `d.port()` gives a free UDP port;
  `d.sleep(s)`; `d.problem(text)` records a failure; `DriveError` stops the driver
  with a message.
- When the time runs out the runner stops every process and the driver, and the
  scenario fails as timed out. A process the driver leaves running is stopped and
  reported. A driver that opens a window sets `window=True` on its `Scenario` and
  starts that process with `window=True`; it then goes through `tools/agent-run.sh`.
- The processes share the scenario's data folder, so the server's `logs/server-DATE.log`
  and a game's `logs/net-DATE.tsv` and capture land in the same place. The drivers
  clear `logs/` and `replays/` in their copy first, so every file they check was
  written by this run.

The pure checks (the report, the players table, the net log, the figures lines) have
unit tests in `tools/test_battery_net.py`; the runner's driver support is tested in
`tools/test_battery.py` with the running Python standing in for the programs.

## Choosing scenarios by change

`python3 tools/battery.py --changed` maps the network crates to five families (the
map is in `tools/battery_selection.py`):

| Family | Scenarios | Chosen when these change |
| --- | --- | --- |
| `net-check` | `net-server-check` | `tore-server` |
| `net-fly` | `net-server-fight`, `-chat`, `-kick` | `tore-codec`, `tore-net`, `tore-session`, `tore-server` |
| `net-discovery` | `net-discovery` | `tore-net`, `tore-session`, `tore-server`, the app's `net/search.rs` |
| `net-window` | both `net-window-*` | `tore-codec`, `tore-net`, `tore-session`, and in the app `net/`, `direct_screen/`, `lobby_screen/` and `widgets/` |
| `net-master` | `net-master-*` | `tore-master`, and `tore-net`'s `master/` module |

A change under the app's `net/` or its screens counts as touching windowed code, so
the quick check opens a window for it. A change to a crate only (`tore-session`, say)
runs the cheapest flying scenario headless and reports `net-window` as needing a
window; `--with-windows yes` adds it. The fight (79 seconds) is over a third of the
default 120 second budget, so the quick check never adds it as an extra; run it by
name when the change is about flying itself.

## What it cannot tell you

- **Bad networks.** Everything runs on loopback at 5 ms with no loss. Round trips of
  50 to 300 ms, loss and duplicates are the matrix's job (a Rust test on the
  network simulator).
- **Other machines and platforms.** One Linux machine. The three-platform check is
  John's test, and the loopback test in CI covers the platforms with synthetic data.
- **The screens.** No scenario clicks Direct Connection or the lobby. The screens'
  looks are the menus lane's `menus-snap-direct*` and `menus-snap-lobby*` states, and
  their behaviour is Rust tests; the hosted game and the joined game here run from the
  command line.
- **How it feels.** Nothing judges how the fight looks or flies, how chat sounds, or
  lag; the bots only prove the protocol and the host work end to end.
- **Firewalls.** The search runs on this machine; whether a broadcast crosses a router
  is a manual check (see the [server guide](../DEDICATED-SERVER.md#discovery-and-the-firewall)).

## Needs a human eye or ear

- **The hosted game's window.** `net-window-host` and `net-window-stall` check what the
  programs print and write, not what the window shows: the lobby, the flight screen
  and the debrief with a second player in the sky.
- **Chat sound.** The quick lines carry a sound tag; whether they play, and at what
  volume, is for an ear.
- **Join feel.** The time from "Joining..." to flying, and whether a freeze shows as a
  jerk for the frozen player, need a person on two machines.

## Found and not fixed

Nothing found yet; the lane was built on 2026-10-05 against the multiplayer branch with
the first set of scenarios all passing. The notes that came out of building it:

- `tore-app --find-games` takes the game port when it is free. Polling it while a
  hosting game starts can take the port first and make the host's start fail, so
  `net-window-host` waits for the game to hold its port before searching.
- A server's seat numbers follow the order games ask for them, so scenarios read a
  player's seat from the `players` table instead of assuming it.
- A hosting game that nobody has joined yet shows `king -` in the search while it is in
  the lobby, and `king Host` once its own player has joined.
