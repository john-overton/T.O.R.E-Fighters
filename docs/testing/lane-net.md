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
game that hosts, host migration with bots that host and stand by, and the master server under its flood tool. It uses the same imported data as every other lane, so it needs
the game imported and the server and bot programs built. Its scenarios live in
`tools/battery_scenarios/net.py`, the windowed ones for the observer screen in
`net_observe.py` and those for the game's rejoin and migration lines in `net_screens.py`.

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
fight, 75 seconds of flight; the rest run beside it. `net-server-pvp` takes about
20 seconds since slice BOT, when the bots began to land the kill that ends it (it ran
to its two-minute time limit before; its limit is four minutes now, only a bound for
a busy machine). Each scenario takes its own
ports (the runner hands out ports nothing on the machine is using), so they run in
parallel and a server left running elsewhere on 26900 is no trouble. The
`net-window-*` scenarios open a window through `tools/agent-run.sh` and count
against `--windows`; the quick check leaves them out unless a change reaches the
game's own network code (see "Choosing scenarios by change").

A hosting game asks the router to forward its port by default (J4b), so the
runner sets `TORE_NO_PORT_MAPPING=1` for every scenario, `quick_check.py` and
`tools/agent-run.sh` do too, and the Rust tests never choose the real router.
There is no scenario for port mapping itself: a real one would change a real
router, so it is the manual `tore-app --map-port SECONDS` check (slice IJ7);
the keeper, the hosts and the server are tested against fake gateways on
loopback in the crates.

## What each scenario checks

| Scenario | Seconds | What it does | What has to hold |
| --- | --- | --- | --- |
| `net-server-check` | 1 | `tore-server --check` on the example mission in [the server guide](../DEDICATED-SERVER.md#the-mission-file), read straight from the guide | Exit 0. The mission summary, all twelve planes in order (six friendly), the runway list and the content manifest line are printed, and since slice L3 the import's content: its source line, its counts with the shared data, and at least one aircraft, theater, weapon and shared item line |
| `net-server-fight` | 79 | A server and two `tore-bot` players fly a fight 5 nm apart for 75 seconds, then leave | Both join, are seated, fly (figures every five seconds), get a debrief and leave cleanly with exit 0. A bot fired at least one burst. The server logs the start, a status line with both players, both joins with their path ("joined as Bot1 (path: local network)", slice J6), both departures, "everyone left" and its own stop, and exits 0 by itself (`after-end quit`). Its log file in the data folder holds the same and a once-a-minute figures line for each player. No refusal, protocol error, bad packets, silence or fault anywhere |
| `net-server-chat` | 25 | Both bots send free text to all, to friendlies and to an empty enemy side, and a quick line from `CHAT.TXT` | Each line reaches the other bot with its sender and receiver, a line nobody can hear is answered "No one hears you", the quick line carries its sound tag, and the server log records every line with how many heard it |
| `net-server-scores` | 65 | A server with a one-minute time limit (`time-limit 1`) and two `tore-bot` players fly the guide's mission 5 nm apart until the time limit ends it (slice F2-S) | Each bot prints at least one `scores:` line that lists both players with time left to fly, and its last scores, with `0:00 left`, before "Mission ended: the time limit"; the server logs that end and stops by itself with exit 0. No refusal, protocol error, bad packets, silence or fault |
| `net-server-results` | 61 | A PvP server (`mode pvp`, `time-limit 1`) on the guide's mission 5 nm apart, one `tore-bot` in plane 0 (Blue) and one in enemy plane 6 (Red), until the time limit ends it (slice F2-D) | Each bot prints exactly one `results:` line, before "Mission ended", with a row for every aircraft: both callsigns on their planes, AI rows for the rest, and the winner of the final scores ("a draw" or a side). The server logs the end and stops with exit 0. No refusal, protocol error, bad packets, silence or fault |
| `net-server-kick` | 6 | Three bots fly; the driver types the console's `players`, `status`, `kick SEAT`, `kick-player ID REASON` and `end` | The players table shows each bot in its plane. The seat kick reads "kicked by the server" at the bot and "left: kicked" in the log; the id kick reads "The server removed you from the game: REASON"; `end` gives the player still flying a debrief and "Mission ended by the host", and the server stops |
| `net-server-observe` | 39 | Two bots fly a fight 5 nm apart for 35 seconds; once the mission flies a third, `--observe 0`, joins with no plane and watches for 20 seconds, its camera on plane 0 (stage F phase 2's observer stream) | The observer joins, its observer flight starts ("observing from tick N, 0 s behind"), the lobby marks it "no slot observing", it draws frames with aircraft in them, takes no seat and gets no debrief, and leaves cleanly with exit 0; the flyers are seated and exit 0; the server logs its join and its departure. No refusal, protocol error, bad packets, silence or fault anywhere |
| `net-server-king` | 64 | A server with `king first-player` (slice F2-1): the first bot, `--king mode=pvp,friendly-fire=off,time-limit=60,lives=3`, wears the crown, changes the settings and starts the mission; a second bot then joins with `--slot 6`, an enemy plane | The King bot prints that it wears the crown, its change, the lobby's crown and its seat in plane 0; both bots print the settings (mode pvp, friendly fire off) and hear the King's one-minute time limit end the mission; the second bot flies plane 6 and never wears the crown. The server logs "King wears the crown", the King's change, the start and the end, and stops by itself. No refusal, protocol error, bad packets, silence or fault |
| `net-server-pvp` | 20 | PvP from the server's file (`mode pvp`, `kill-limit 1`, `kill-owner total`, `time-limit 4`): one bot in plane 0, one in enemy plane 6, the guide's mission 5 nm apart with every AI wing on weapons hold (slices F2-1, F2-S and BOT) | Both bots fly their planes and shoot at each other; the scores put a player on the enemy side and name the limit ("ends at 1 kill in all"). A kill by either bot (or both, on one pass: a draw with two scorers) must end the mission by the kill limit, about 20 seconds in; the time limit's draw is a failure now. The AI is on weapons hold because its missiles kill a bot on the AI's side about 15 seconds in, before the bots' pass. No refusal, protocol error, bad packets, silence or fault |
| `net-server-hunt` | 20 | A lone bot (Hunter, plane 0) on the guide's mission 5 nm apart in PvP with `kill-limit 1`, every enemy wing a dummy (straight, level, 400 knots, no evasion) and every AI wing on weapons hold (slice BOT) | The bot's pursuit and gun aiming shoot a dummy down within about 20 seconds: "Mission ended: the kill limit", the bot's tally holds the kill and its side wins. No refusal, protocol error, bad packets, silence or fault |
| `net-server-delay` | 54 | A PvP server with `observer-delay 10`; two bots fly, and once the mission flies an observer bot, `--observe 0`, watches for 30 seconds (slices F2-1 and F2-O1) | The observer's lobby settings show the delay, its observer flight starts "10 s behind", it draws frames with aircraft in them, takes no seat and gets no debrief, and exits 0; the server logs "Owl is watching the mission". No refusal, protocol error, bad packets, silence or fault |
| `net-server-revive` | 40 | A server whose file sets `respawn revive` (slice F2-1's keys) and one `tore-bot --revive 8` on the guide's mission; the bot ejects 8 seconds into its flight, confirming as a player must, and flies again (slice F2-V) | The bot prints its ejection, "revival: Press Enter to fly again", "spawned plane 12 in Friendly wing 1" and a second seating in plane 12, in that order, then a debrief and a clean leave with exit 0; the server's log has it taking plane 0 and plane 12. No refusal, protocol error, bad packets, silence or fault |
| `net-server-replies` | 50 | A server on the guide's mission with every AI wing on weapons hold, a `tore-bot --slot 0 --order 14,break-left --reply 26,engaging` (the lead) and, once it flies, a `tore-bot --slot 1 --reply 12,winchester` (its wingman) (slice F2-R) | The wingman prints the lead's order as a radio line, `radio: Red one: '...'`; the lead prints the wingman's reply under its place, `radio: Red two: 'Winchester'`; the wingman prints its own reply as itself, `radio: YOU: 'Winchester'`; the lead's own reply is refused with `line: You lead this flight.` and neither bot hears an "Engaging" call. Both bots get a debrief and leave with exit 0. No refusal, protocol error, bad packets, silence or fault |
| `net-server-datalink` | 50 | A server on the guide's mission with every AI wing on weapons hold, a `tore-bot --slot 0 --order 20,sort` (the lead) and, once it flies, a `tore-bot --slot 1` (its wingman) (slice G7) | The lead prints `line: Sort: N assigned`; both bots print the Link event `link: plane 0 assigned plane 1 bandit N (Sort)`, the wingman its readout's `link: assigned: bandit N by plane 0` for the same bandit and the lead's assignment call as a radio line, `radio: Red one: '... attack bandit ...'`; no bot prints a Link event about a plane outside its flight. Both bots get a debrief and leave with exit 0. No refusal, protocol error, bad packets, silence or fault |
| `net-server-datalink-lead` | 50 | A server on the guide's mission, weapons free, and one `tore-bot --slot 1`: the AI leads plane 0 and flies planes 2 and 3 (slices G4 and G7) | Once the AI lead commits it sorts or shares: the bot prints the Link event `link: plane 0 assigned plane 2 bandit N (Sort)` (or plane 3, or `EngageMyTarget`), its readout's mark for that flightmate, `link: marked: bandit N assigned to 3` (the member's number from one), and the lead's call, `radio: Red one: 'Three, attack bandit ...'`. It never prints an assignment of its own (an AI lead never moves a human) or a Link event about another flight, and leaves with exit 0 after its debrief. No refusal, protocol error, bad packets, silence or fault |
| `net-server-away` | 44 | A server on the guide's mission with `idle-ai` at its default, and one `tore-bot --slot 0 --away 8,6`: its game says it is away 8 seconds into its flight and back 6 seconds after the AI took the plane (slice F2-A) | The bot prints its seating in plane 0, "away: the AI flies plane 0", its observer flight ("observing from tick N, 0 s behind"), "back at the controls" and a second seating in plane 0, in that order, then a debrief and a clean leave with exit 0; the server's log has "Viper is away: the AI flies plane 0", "Viper is back: takes plane 0 from the AI" and Viper taking plane 0. No refusal, protocol error, bad packets, silence or fault |
| `net-server-rejoin` | 60 | A server on the guide's mission with a long `empty-timeout`, a bot Stay in plane 1 and a bot Viper in plane 0 that keeps its rejoin token in a file (`tore-bot --token-file`); the driver kills Viper with SIGKILL in flight, waits for the server to drop it (5 seconds of silence), then starts a second Viper with the same file (slice K5) | The server logs "Viper dropped out: the AI flies plane 0, kept for it"; nobody else takes plane 0. The second bot prints "rejoining with its token", "Welcome back, Viper: your aircraft is waiting." and a seating in plane 0, in that order, then a debrief and a clean leave with exit 0. The server logs "Viper rejoined with its token: plane 0 is waiting" and Viper taking plane 0 twice, and no protocol error, bad packets or fault (the first bot's silence is the point, so the check leaves it out) |
| `net-migrate-kill` | 116 | Host migration (slice K9): a hosting `tore-bot --host` (Lead, plane 0, `--players 4 --wait-standbys 2`) and three pilots in one `tore-bot --count 3 --standby on` process fly the guide's mission 5 nm apart. Once the host's world has booked a kill, and a guided missile flies when one does in time, the driver kills the host with SIGKILL, in the fight | The first standby takes the game over ("took the game over at tick N: replayed ... players expected back", then "live at tick N"); the other pilots print "Lost contact with the host. Moving the game to ..." and "The game moved to Pilot1."; the new host sees each resume; every pilot's "migrate: snapshots again N ms after the loss was noticed" is within 3.5 seconds (the plan's 5 seconds from the loss, less the 1.5 seconds a client waits before it notices; scaled by `--timeout-scale`, since a loaded machine steps the fast-forward slower); the new host's world carries on from at least the old host's last tick with at least its kills, and its Results at the end hold at least those kills; the counts line says one migration resumed, none failed, none corrected; the pilots exit 0. If the new host's fast-forward costs more than 2 ms a tick (a debug build on a busy machine, which cannot meet the 5 seconds) the check asks instead that the snapshots follow the host going live within 1.5 seconds. Which missiles end where is `host::resume_tests`' to check on the simulator, to the byte |
| `net-migrate-handover` | 125 | As `net-migrate-kill`, but the hosting bot's time is up (`--seconds 80`) and it leaves on purpose | The host prints "handing the game over to player N" and "the game was handed over" (it did not say the host left) and exits 0. The first standby takes the game over at once, the other pilots follow within 2.5 seconds, the world carries on from the old host's last tick with its kills, and the pilots exit 0 |
| `net-migrate-relay` | 135 | A hosting bot lists its game on a `tore-master` on this machine (`--master`), two direct pilots stand by and a third bot joins through the master's relay (`--path relay`, never a standby). The driver kills the host in the fight (slices K8 and K9) | As `net-migrate-kill` with the relayed bot among the pilots, and: the new host resumes the listing from the part the old host journaled ("listing: resumed from the old host's part"), the master logs "relay moved listing=... channels=1" and "moved id=...", the relayed bot keeps its channel (no "The relay closed"), is never appointed a standby, and flies on with the new host |
| `net-reach-upload` | 105 | Host selection on loopback (slices K3 and K6): a hosting bot waits for one ready standby (`--wait-standbys 1`). Aa joins with `--standby on`, Bb with `--standby off` (its game says it may not host) | The host's reach tests and its upload test pass Aa on loopback, so the mission starts; Aa is appointed first standby, warm, with every check equal; Bb is never appointed; the host hands over when its time is up and everyone exits 0 |
| `net-convert-capture` | 40 | A server and a `tore-bot --capture` fly the example mission, started at 40,000 feet, for 25 seconds; the game converts the bot's capture with `--convert-capture`, the exports read the replay, it converts twice, and a copy cut at 60 percent converts too | The replay finishes normally and names the network flight, the callsign, the player as `You` in `F/A-18D`, 12 aircraft and `net.stats` events; the conversion says what it made again and made contrails (every aircraft is above its onset altitude), motor smoke when the host sent launches, and gun rounds exactly when it sent gun bursts (found in the Tacview file made with `--guns`); info, debug log and Tacview work; the two conversions are byte for byte the same; the cut one says it is cut short and its footer says `end=cut`. No window. See [network flights](../REPLAYS.md#network-flights) |
| `net-content-missing` | 43 | Stage L (slice L3): a server on the guide's mission, which flies the Su-27; `tore-bot --callsign Hawk --drop-resource SU27.PT --expect-unable` for 15 seconds and a second bot, Viper, for 35 | Hawk prints the dropped profile, "Your game has no Su-27 ..., which this mission flies. ... Re-import Fighters Anthology (Pref, Re-import) to add it.", "gaps: aircraft SU27.PT (Hawk lacks it)" and "Your game differs from the host's: no Su-27 ...", takes no seat and leaves cleanly with exit 0; Viper prints the same gap, "Hawk's game differs from the host's: no Su-27 ...", the lobby with Hawk unable, a seating and a debrief. The server's log holds "content Hawk: ...; lacks aircraft SU27.PT", the gaps line, "Hawk cannot play the mission: Hawk's game has no Su-27 ..., which this mission flies." and "gaps: none" once Hawk has gone. No network problem from Viper |
| `net-content-builds` | 107 | Stage L (slice L3): `tore-server --import` makes a 1.0 import of `gameassets/fighters-anthology/disc1` in the run (about 60 seconds; the scenario fails, saying so, without the disc), `tore-bot --content-report` reads it and `tore-server --check` the profile's 1.02F import; then a server on the profile and a bot, Old, on the 1.0 import fly 30 seconds | The report says "Fighters Anthology 1.0" and the check "1.02F", and their item lines (kind, key, digest) are the same line for line. Old prints "gaps: none" and no build or difference line (L5 removed the build line), is seated, gets a debrief and leaves cleanly. The server prints its own content line and logs "content Old: Fighters Anthology 1.0, ...; the same items as the host" and no gap. The 1.0 import (about 170 MB) is removed at the end |
| `net-discovery` | 5 | `tore-app --find-games` against a running server, then on a free port | The server's line (this build, name, mission, `0/6 players`, `lobby`, `king -`, `open`, `not full`) is printed on its port. A port with nothing prints `No games found.` and exits 0. The console's `quit` stops the server with exit 0 |
| `net-window-stall` | 17 | A game joins the server and its script blocks the whole main loop for 4 seconds in flight | The server logs "game stalled, flying neutral" and "game back after 4.0 s" (the keepalive held the seat), then a clean leave. The game wrote `logs/net-DATE.tsv` (header, join, mission, seating and once-a-second figures with the right columns) and a capture in `replays/` |
| `net-window-lobby` | 115 to 200 | The lobby screen's phase 2 panels in the window (slice F2-L, `battery_scenarios/net_lobby.py`): the game hosts from Direct Connection's New (the King), the script turns the Game type to PvP (which opens the enemy planes, so a `tore-bot` may join, and takes a slot), turns Friendly fire off, the Scoring page's kill limit and the Realism page's Damage, right-clicks the bot's slot closed and open, selects the bot in Players and gives it the crown with Players...; the bot wears the crown and leaves 40 seconds in, when the crown comes back. A scripted click that lands before the menu answers is tried once more (about half the first tries) | The game's log holds, in Messages' words, the bot joining, the settings ("Settings: mode pvp, ..." with PvP's reset values, "Settings: friendly-fire off.", "Settings: kill-limit 7."), the cheat sent as a mission change ("The mission is now: ..."), the slot closed ("Plane 0's slot is closed: the AI flies it.") and open, "Bot is the King now.", "Bot left the game." and "Viper is the King now.". The bot prints the settings, "The King closed plane 0's slot: the AI flies it.", the lobby with itself as King and "wears the crown". Nine script pictures are written (the menu layer's pictures hold no text: the sharp text is drawn over them). No network problem anywhere |
| `net-window-gaps` | 80 | Stage L (slice L4, `battery_scenarios/net_lobby.py`): the game hosts from Direct Connection's New (the King); the script turns the Game type to PvP (the default mission has room for the hosting player alone, and a full game refuses the bot), then `tore-bot --observe none --drop-resource RAFALE.PT` joins with no slot; the script selects the bot in Players, opens Mission..., the first wing's aircraft list, chooses the Rafale and presses OK, then Esc | The game's log holds, in Messages' words, "Bot joined the game." and "Bot's game differs from the host's: no Rafale C.", and the hosting game's own "Host: tick N: content Bot: ...lacks aircraft RAFALE.PT" and "gaps: aircraft RAFALE.PT (Bot lacks it)" lines; the bot prints the dropped profile and the gap. Five script pictures are written for a person to look at: the lobby, the lobby after the bot joined, the aircraft list with the Rafale dimmed, and the creator with the refusal in its notice ("Not everyone can fly the Rafale C: Bot's game has no Rafale C."), then the lobby again (the lobby screen's own text is drawn over the menu layer, so those pictures show no words). No network problem anywhere |
| `net-window-rejoin` | 24 | The game's rejoin in the window (slice K7b, `battery_scenarios/net_screens.py`): the game joins a `tore-server` with `--connect` as Viper in plane 0 and flies; the driver kills it with SIGKILL once it holds a token in `rejoin-v1.conf`; the server keeps plane 0 for it; the game started again in the same data folder joins, and its script leaves after a few seconds | The game's log holds "Welcome back, Viper: your aircraft is waiting." and a second seating in plane 0, and nothing about a token refused; the server logs "Viper dropped out: the AI flies plane 0, kept for it" and "Viper rejoined with its token: plane 0 is waiting"; `rejoin-v1.conf` held the token before the kill |
| `net-window-migrate` | 125 | The game's HUD through a host migration (slice K7b, `battery_scenarios/net_screens.py`; needs the hosting bot of K9): a hosting `tore-bot` (Lead), two pilots that stand by and the game (Viper, plane 3, its "Let my game take over hosting" switch off in `network-v1.conf`, so a pilot takes over) fly the guide's mission 5 nm apart; nine seconds into the flight the driver kills the hosting bot with SIGKILL; the game's script takes fourteen pictures of the flight, half a second apart, from tick 1200 | The game's log holds "Lost contact with the host. Moving the game to PilotN..." and "The game moved to PilotN." and no take-over by the game or session given up; the pilots print their snapshots again after the loss and exit 0; fourteen pictures are written (the HUD's two lines are at the foot of the second, for a person to look at) |
| `net-window-observe` | 130 to 370 | The observer screen in the window (slice F2-O2, `battery_scenarios/net_observe.py`): a `tore-server` flies the guide's mission 5 nm apart with two `tore-bot` players, and the game, its settings file naming the server's address, joins from Direct Connection (Connect to, Up, Enter) with no plane, presses **Watch** in the lobby, and the replay viewer opens in its live mode; the script takes a picture of the live view, presses Tab (another aircraft), Home (leaves live) and End (back to live) with a picture after each, then Esc and Enter on the menu's **Stop Watching** row, back in the lobby. Scripted clicks need a second or two over a target before they land on a loaded machine, so the waits are long, and a script that did not open the screen is tried again, up to three times | The game's log holds Watch's line in Messages, "Observer screen: watching the mission", "Observer screen: back to the lobby" and "You stopped watching."; nothing says the screen could not open; the server logs "Viper is watching the mission" and "Viper stopped watching" and the bots fly to the end with no network problem; eight script pictures are written (four of the menu layer, which holds no text, and four of the live view, each the viewer's own rendered picture with its bar: LIVE, then the next aircraft, then the start of the kept window at 1x, then LIVE again after End). The scenario cannot judge a picture: look at them |
| `net-window-away-watch` | 100 | An away player watches its own plane (slice F2-O3, `battery_scenarios/net_observe.py`): a `tore-server` on the guide's mission with every AI wing on weapons hold (so the AI that flies the idle plane is not shot down while the script looks) and the game joined with `--connect` as Viper in plane 0. The script waits four seconds of flight, presses Esc (the flight menu: the controls are neutral behind it), and after the King's `idle-ai` 10 seconds the host gives the plane to the AI and the game opens the observer screen on it; the script takes pictures, holds the Up arrow (the first flight input) for a second and is seated in plane 0 again; then the same, and Esc then Enter on the observer screen's menu (**Stop Watching**, which in a game with no lobby screen takes the plane back too) | The game's log holds, twice each, "away for the idle-ai seconds", "Observer screen: watching the player's own aircraft" and "Observer screen: back to the flight", once "a flight input; taking the aircraft back from the AI" and once "Stop Watching; taking the aircraft back from the AI", and three seatings in plane 0; nothing says the screen could not open or the AI lost the aircraft. The server logs "Viper is away: the AI flies plane 0" and "Viper is back: takes plane 0 from the AI" twice each. Seven script pictures are written, for a person to look at (the third and fourth show the observer screen on the player's own aircraft with the banner, the fifth and seventh the cockpit again). No network problem |
| `net-master-flood` | 11 | A `tore-master` on 127.0.0.1 (its probe port the main port + 1, status every 2 seconds), then `tore-master flood` at it for 10 seconds; the console's `status`, `listings` and `quit` | The flood exits 0 with "limits held": no port answered with more bytes than it sent, and every browse from 127.0.0.2 during the flood answered (at least 8). The master printed both ports, a `limit source=127.0.0.1` line, a status line with `dropped(limit)` above 0, `listings=0` (the flood made no listing) and `Stopped`; its `state/telemetry/DATE.tsv` counted none of the flood's reports |
| `net-master-listing` | 2 | A `tore-master` on 127.0.0.1 and a `tore-server` with `broadcast on` and `master 127.0.0.1:PORT`; the server's console `status`, `broadcast off`, `broadcast on` and `quit`; the master's `listings` and `quit` (slice I3). Slice I4 adds `tore-app --browse` (against this scenario's own master, never the built-in one): it lists the server with its mission, then says `No games listed.` after `broadcast off`, and exits 1 saying the master "does not answer" for a port with nothing on it | Judged from the master's own output: a `listed` line from the server's game port with its name, the master's `listings` showing it at `players=0/`, an `unlisted ... reason=unregistered` line after `broadcast off`, the browse's line `"T.O.R.E server"  0/6 players, lobby, open, not full, this build, dedicated server; mission "UKR, ..."; king -; players -` and `1 game listed.`, a second `listed` after `broadcast on`, a second `unlisted` at `quit`, `listings=0` at the end and no listing left to expire. The server printed its `Broadcast: on` start line, `Broadcasting: listed`, the listing at the end of its status line and both console lines, its log holds the listing, and no network problem or silent master anywhere |
| `net-master-introduce` | 34 | A `tore-master` on 127.0.0.1, a `tore-server` with `broadcast on` listed on it, and `tore-bot --master 127.0.0.1:PORT --listing "T.O.R.E server" --seconds 30` (slice J2) | The bot finds the listing, runs the mapping test, is introduced, joins "through the Internet Lobby, path punched" (on one machine the host's seen address answers), is seated, gets a debrief and leaves cleanly with exit 0, with no "no direct path" line. The server ends with "everyone left" and its log holds the join, with its path ("joined as Bot (path: punched)", slice J6), and the leave. A master status line counts the introduction (`introductions/min` above 0). No network problem anywhere |
| `net-window-internet` | 130 | A `tore-master` and a listed `tore-server` on this machine; the game's window is driven by an input script (its settings file names the master and a callsign) through the Multi menu to the Internet Lobby, selects the listed server, presses New (the lobby opens as King; a second `tore-app --browse` runs meanwhile), leaves, then presses Join on the server, takes a plane, readies and flies about five seconds (slice I4, with J2's joiner). A scripted click that lands before the menu answers is tried once more | The game's log holds the one-time notice, "Asking the Internet Lobby at 127.0.0.1:PORT for games...", "1 game is listed", "Hosting Viper's game ... Listing it on the Internet Lobby.", the host's "Listed on the Internet Lobby", then Join's "Trying 1 address for 'T.O.R.E server'...", "Connected directly (punched through).", "seated in plane 0" and the mission flying; the master lists "Viper's game" and the second browse prints it beside the server; the server logs the join, the slot, the seating, and the master counted an introduction; seven script pictures are written; nothing mentions the built-in master |
| `net-window-internet-relay` | 48 | As the join half of `net-window-internet`, with `TORE_JOIN_PATH=relay` in the game's environment (slice J5): the game opens the Internet Lobby, selects the listed server, presses Join, takes a plane, readies and flies about five seconds, joining only through the master's relay (on one machine every direct path works). A scripted click that lands before the menu answers is tried once more | The game's log holds "Asking the Internet Lobby to introduce you to 'T.O.R.E server'...", "Joining 'T.O.R.E server' through the relay only (TORE_JOIN_PATH=relay)...", "Asking for the relay...", "The relay is open; joining through it...", "Connected through the relay.", "joined [100::1:...]:0, path relay", "seated in plane 0" and the mission flying, and nothing of a direct join; its `logs/net-DATE.tsv` has a `path relay` line; the master logs `relay opened` between the server's game port and the game and a status line with `relayed=1`, and refused nothing; the server logs the join and the slot; both script pictures are written; nothing mentions the built-in master |
| `net-master-relay` | 34 | As `net-master-introduce`, with `tore-bot ... --path relay` (slice J3): on one machine every direct path works, so the bot asks for the relay at once and never races | The master's start lines say `relay ACTIVE` and the month's figure. The bot is introduced without racing, asks for the relay, says the relay is open, joins "through the Internet Lobby, path relay", is seated, flies with figures, gets a debrief and leaves cleanly with exit 0, with no race, punched path or lost relay. The master logs `relay opened` between the server's game port and the bot, `relay closed ... reason=closed by an end` with at least 10 KB to the host and 100 KB to the bot, a status line with `relayed=1 channels=0` and a `relay-month` figure in KB or MB, no channel closed by the master or refused, and its `state/relay-YYYY-MM.txt` holds at least 100 KB. The server ends with "everyone left" and its log holds the join, with its path ("joined as Bot (path: relay)", slice J6), and the leave. No network problem anywhere |
| `net-window-host` | 41 | `tore-app --host` flies the example mission; the driver waits for it to listen, searches, joins a bot, and the host's script leaves after 20 seconds of flight | The search finds the hosted game by name with its King. The bot joins, is seated, flies, hears "Mission ended: the host left the game.", gets its debrief and exits 0. The host exits 0 and wrote the same net log and a `HOSTED` capture |

All of the lane's output is checked for the same general problems as every other
lane (panic, `NaN`, infinity, stack overflow, fatal error), across every process.

## Bots that host and stand by (stage K, slice K9)

The migration scenarios need a game that hosts and games that take over, with no
window, so `tore-bot` does both:

- `tore-bot --host MISSION --port N --slot 0 --players 4 --wait-standbys 2` hosts
  the mission file as the game's hosting thread does (its own player is the house,
  on the in-process link) and starts the mission once the lobby holds `--players`
  players and `--wait-standbys` standbys are ready. `--master ADDRESS` lists the
  game on a master on this machine, `--standby off` appoints none. When its
  `--seconds` are up it hands the game over to a ready standby, or says the host
  left, and exits.
- `tore-bot --connect HOST:PORT --standby on` (default off) makes a joined bot a
  game that may take over hosting: it reports its candidates, answers the host's
  reach tests, keeps a standby on a thread, and takes the game over when the host
  is lost. A bot that took the game over leaves after its guests, at most 25
  seconds past its `--seconds`.
- A hosting bot prints `NAME: host: ...` lines (its log, the resume notes, once a
  second the world's tick, guided missiles in flight and aircraft kills), a bot
  prints `NAME: standby: ...`, `NAME: migrate: ...` and `NAME: listing: ...` lines.
  The drivers read them with plain functions (`world_lines`, `migrate_problems`)
  that `tools/test_battery_net.py` tests.

The scenarios take about two minutes each (most of it the lobby, where the host
runs its reach and upload tests before it has two ready standbys, and the fight).
A debug build on a busy machine steps the new host's fast-forward slowly: see the
note in `net-migrate-kill`'s row.

## How a scenario works

A net scenario sets `driver=` to a Python function instead of `args`. The runner
gives it a `Drive` ([tools/battery.py](../../tools/battery.py)) and does the rest
the way it does for a single run: the per-scenario copy of the data folder
(`d.data`), a work folder (`d.work`), the timeout (`--timeout-scale` applies to
every wait limit), the output capture and the final checks.

- `d.start(label, [d.server, ...], stdin=True, window=False)` starts a process in
  its own session. `d.app`, `d.server` and `d.bot` are the three programs (a scenario can start any number of bots, one of them hosting).
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

`python3 tools/battery.py --changed` maps the network crates to eleven families (the
map is in `tools/battery_selection.py`):

| Family | Scenarios | Chosen when these change |
| --- | --- | --- |
| `net-check` | `net-server-check` | `tore-server` |
| `net-fly` | `net-server-fight`, `-chat`, `-kick`, `-observe`, `-scores`, `-revive`, `-away` | `tore-codec`, `tore-net`, `tore-session`, `tore-server` |
| `net-convert` | `net-convert-capture` | `tore-codec`, `tore-net`, `tore-session` (its capture conversion files, `tore-bot` and the rest), `tore-replay`, the app's `replay/net_convert.rs` |
| `net-discovery` | `net-discovery` | `tore-net`, `tore-session`, `tore-server`, the app's `net/search.rs` |
| `net-window` | the `net-window-*` scenarios | `tore-codec`, `tore-net`, `tore-session`, and in the app `net/`, `direct_screen/`, `internet_screen/`, `lobby_screen/` and `widgets/` |
| `net-master` | `net-master-*` | `tore-master`, and `tore-net`'s `master/` module |
| `net-listing` | `net-master-listing` | `tore-server`, the app's `net/browse.rs` and `internet_screen/` |
| `net-introduce` | `net-master-introduce` | `tore-net` and `tore-session` (the transport's race and the bot); `tore-master` and `tore-net`'s `master/` reach it through `net-master` |
| `net-relay` | `net-master-relay` | `tore-net` and `tore-session` (the sockets and the bot); `tore-master` and `tore-net`'s `master/` reach it through `net-master` |
| `net-content` | `net-content-missing` | `tore-session`, `tore-server`, `tore-world`'s `content.rs` |
| `net-builds` | `net-content-builds` (slow: it imports the 1.0 disc) | only the content code: `tore-session`'s `host/content*` and `client/content.rs`, `tore-world`'s `content.rs`, `tore-import`'s `source.rs` |

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
- **The screens.** Only `net-window-internet` and `net-window-internet-relay` (the Internet Lobby) and `net-window-lobby` (Direct Connection's New and the lobby's panels) click a screen. The screens'
  looks are the menus lane's `menus-snap-direct*` and `menus-snap-lobby*` and `menus-snap-internet*` states, and
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
