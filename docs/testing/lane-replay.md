# Battery lane: replay

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

The replay lane tests everything that surrounds flight rather than the flight itself:
mission recordings and the viewer, the exports (info, log, summary, Tacview, diff), radio and
crew lines, audio start-up, controller input and profiles, input tapes, start-up
diagnostics, first-run import errors, and the game's key handling. Its scenarios live in
`tools/battery_scenarios/replay.py` and the `_replay_*.py` modules beside it.

## Running it

```sh
cargo build --locked -p tore-app
python3 tools/battery.py --lane replay --jobs 5 --windows 2 --tag replay
python3 tools/battery.py --scenario 'replay-rec-*'          # headless recordings only
python3 tools/battery.py --scenario 'replay-keys-*' --windows 2   # key-pressing scenarios
```

The whole lane is 464 scenarios (118 of them open a window) and takes about 45 minutes on the
dev machine with five jobs and two windows, of which `replay-validate-creator` alone is about
eight minutes. The last full run passed all of them except that scenario, which timed out under
the old 10 minute limit and now has 30 minutes; the combat-smoke scenarios were known failures then and pass now that the
flight lane brought the probe up to date. The recording, corrupt-file, option, snapshot and input scenarios are
headless and take a few seconds each. Anything that opens a window goes through
`tools/agent-run.sh` and counts against `--windows`. The import scenarios copy the local retail
media with a copy-on-write copy (no extra disk space on btrfs) and need `gameassets/` present.

## What each group checks

| Group | Scenarios | What has to hold |
| --- | --- | --- |
| `flight-f12-hold` | Live Mk 82 release and replay F12, through impact and the return | The camera stops during the 328-foot approach and holds that eye for exactly 180 game ticks after impact. Reverse seeking reproduces the held frame and F10 interrupts it. The release probe keeps running while requested post-impact captures remain. |
| `replay-rec-*` | Fights 1v1 to 15v15 (and 1v15, 15v1, 4v9), every aircraft as enemy and as player, four skills by three geometries, six mission presets, guns, missiles with chaff and flares, three threat kinds, nine faults, ground starts at three airports with wings of 1, 3 and 5, four wing orders, and one data link probe (`replay-rec-datalink`) | `--verify-render` passes with nothing missing or differing. `--recording-info` says finished normally with no problems and lists the probe's aircraft. `log.jsonl` is valid JSON with one header, unique aircraft ids and monotonic time. `summary.txt` has its sections and one line per aircraft. The Tacview file has a header, monotonic time, no NaN, no object back from the dead and one object per aircraft. A recording diffed against itself matches. For the fights, guns and missiles a second identical run makes a recording that matches the first, and the probe prints the same output with recording off. The data link probe's `log.jsonl` has a `datalink.member` for each of the six planes, the player's `datalink.lock`, the lead's `datalink.assign` to the first wingman and its `datalink.clear` by order, and as many assignments as the probe printed |
| `replay-rec-m2000-2v2` (slice M2K-9, 2026-11-10) | The Mirage 2000 as the player and in both wings, a 2 v 2 recorded three times (`--aircraft m2000 --probe-friendly-aircraft m2000 --probe-enemy-aircraft m2000 --probe-fight 2:2`), read back by `--recording-info`, `--recording-log`, `--recording-acmi` and `--recording-diff` | `--verify-render` passes, the two recordings are identical, the identity reads back as `M2000.PT` (checkpoint code 40, `SELECTABLE` index 40, inside the 6-bit wire field) |
| `replay-rec-f15e-2v2` (slice F15E-9, 2026-11-10) | The F-15E as the player and in both wings, a 2 v 2 recorded three times (`--aircraft f15e --probe-friendly-aircraft f15e --probe-enemy-aircraft f15e --probe-fight 2:2`), read back by `--recording-info`, `--recording-log`, `--recording-acmi` and `--recording-diff` | `--verify-render` passes, the two recordings are identical, the identity reads back as `F15E.PT` (checkpoint code 41, wire index 41, inside the 9-bit wire field), never as `F15.PT` |
| `replay-rec-gripen-2v2` (slice GRI-9, 2026-11-10) | The Saab Gripen as the player and in both wings, a 2 v 2 recorded three times (`--aircraft gripen --probe-friendly-aircraft gripen --probe-enemy-aircraft gripen --probe-fight 2:2`), read back by `--recording-info`, `--recording-log`, `--recording-acmi` and `--recording-diff` | `--verify-render` passes, the two recordings are identical, the identity reads back as `GRIPEN.PT` (checkpoint code 42, `SELECTABLE` index 42, inside the 9-bit wire field) |
| `replay-rec-f5e-2v2` (slice F5E-9, 2026-11-10) | The F-5E Tiger as the player and in both wings, a 2 v 2 recorded three times (`--aircraft f5e --probe-friendly-aircraft f5e --probe-enemy-aircraft f5e --probe-fight 2:2`), read back by `--recording-info`, `--recording-log`, `--recording-acmi` and `--recording-diff` | `--verify-render` passes, the two recordings are identical, the identity reads back as `F5E.PT` (checkpoint code 43, `SELECTABLE` index 43, inside the 9-bit wire field) |
| `replay-rec-mf1-2v2` (slice MF1-9, 2026-11-11) | The Mirage F1 as the player and in both wings, a 2 v 2 recorded three times (`--aircraft mf1 --probe-friendly-aircraft mf1 --probe-enemy-aircraft mf1 --probe-fight 2:2`), read back by `--recording-info`, `--recording-log`, `--recording-acmi` and `--recording-diff` | `--verify-render` passes (`missing=0 differing=0`), the two recordings are identical, the identity reads back as `MF1.PT` (checkpoint code 45, `SELECTABLE` index 45, inside the 9-bit wire field) |
| `replay-rec-tornado-2v2` (slice TOR-9, 2026-11-11) | The Tornado IDS as the player and in both wings, a 2 v 2 recorded three times (`--aircraft tornado --probe-friendly-aircraft tornado --probe-enemy-aircraft tornado --probe-fight 2:2`), read back by `--recording-info`, `--recording-log`, `--recording-acmi` and `--recording-diff` | `--verify-render` passes over all 2,401 ticks (the AI Tornados fly at 545 to 580 kt, so the swing wing is mostly swept: the check covers the speed and flap devices its sweep is drawn from), the two recordings are identical, the identity reads back as `TOR.PT` (checkpoint code 44, 9-bit wire index 44). The swivelling stations' turn is derived from those devices and not recorded, so `verify-render` leaves it out of the stores comparison (before TOR-9 it failed with `aircraft 0's stores differ` at tick 1). The unit test `replay::convert::tests::a_replayed_tornado_draws_the_wing_sweep_it_flew` checks the replayed sweep against the flown one at 700 kt (fully swept), 425 kt, and with half flaps |
| `replay-rec-tornadoc-2v2` (slice TOR-13, 2026-11-12) | The Tornado IDS (Clean) as the player and in the friendly wing against the Combat version in the enemy wing, a 2 v 2 recorded three times (`--aircraft tornadoc --probe-friendly-aircraft tornadoc --probe-enemy-aircraft tornado --probe-fight 2:2`) | the same checks as `replay-rec-tornado-2v2`: the Clean identity (checkpoint code 46, recorded key `tornadoc`, type name `Tornado IDS (Clean)`) reads back, plays back with nothing missing and records the same twice. |
| `replay-rec-f117-2v2` (slice F117-9, 2026-11-15) | The F-117A as the player and in both wings, a 2 v 2 recorded three times (`--aircraft f117 --probe-friendly-aircraft f117 --probe-enemy-aircraft f117 --probe-fight 2:2`); nobody fires | the same checks as `replay-rec-tornado-2v2`: `--verify-render` clean, the two recordings identical, the identity reads back as `F117.PT` (checkpoint code 49, `SELECTABLE` index 49) |
| `replay-rec-b2-2v2` (slice B2-9, 2026-11-15) | The B-2 as the player and in both wings, a 2 v 2 recorded three times (`--aircraft b2 --probe-friendly-aircraft b2 --probe-enemy-aircraft b2 --probe-fight 2:2`), read back as the other `replay-rec-*` rows | `--verify-render` passes and two runs record the same: the B2.PT identity (checkpoint code 48) round-trips |
| `replay-rec-b52-2v2` (slice B52-9, 2026-11-16) | The B-52 as the player and in both wings, a 2 v 2 recorded three times (`--aircraft b52 --probe-friendly-aircraft b52 --probe-enemy-aircraft b52 --probe-fight 2:2`), read back as the other `replay-rec-*` rows | `--verify-render` passes and two runs record the same: the B52.PT identity (checkpoint code 47), eight engines and the slewed tail gun's rounds round-trip |
| `replay-rec-b1b-2v2` (slice B1B-9, 2026-11-16) | The B-1B as the player and in both wings, a 2 v 2 recorded three times (`--aircraft b1b --probe-friendly-aircraft b1b --probe-enemy-aircraft b1b --probe-fight 2:2`), read back as the other `replay-rec-*` rows | `--verify-render` passes (2,401 ticks, none missing or differing) and two runs record the same (969 flight and aircraft events each): the B1.PT identity, four engines, the three bay rows and the speed-driven wing sweep round-trip |
| `replay-rec-tu160-2v2` (slice T160-9, 2026-11-16) | The Tu-160 as the player and in both wings, a 2 v 2 recorded three times (`--aircraft tu160 --probe-friendly-aircraft tu160 --probe-enemy-aircraft tu160 --probe-fight 2:2`), read back as the other `replay-rec-*` rows | `--verify-render` passes (2,401 ticks, none missing or differing) and two runs record the same: the `TU160.PT` identity round-trips |
| `replay-rec-tu26-2v2` (slice T26-9, 2026-11-16) | The Tu-26 as the player and in both wings, a 2 v 2 recorded three times (`--aircraft tu26 --probe-friendly-aircraft tu26 --probe-enemy-aircraft tu26 --probe-fight 2:2`), read back as the other `replay-rec-*` rows | `--verify-render` passes (2,401 ticks) and two runs record the same: the TU26.PT identity, the swing wing's state and the slewed tail gun's rounds round-trip |
| `replay-rec-tu95-2v2` (slice T95-9, 2026-11-16) | The Tu-95 as the player and in both wings, a 2 v 2 recorded three times (`--ai-probe-ticks 4800 --aircraft tu95 --probe-friendly-aircraft tu95 --probe-enemy-aircraft tu95 --probe-fight 2:2`), read back as the other `replay-rec-*` rows | `--verify-render` passes (4,801 ticks compared, nothing missing or differing) and two runs record the same: the TU95.PT identity (four turboprops, the belly bay, the 16 AS-16) and the slewed tail turret's rounds (230 shots by 4,800 ticks; none by 2,400, which is why this run is longer than the B-52's) |
| `replay-rec-su34-2v2` (slice SU34-9, 2026-11-16) | The Su-34 as the player and in both wings, a 2 v 2 recorded three times (`--aircraft su34 --probe-friendly-aircraft su34 --probe-enemy-aircraft su34 --probe-fight 2:2`), read back as the other `replay-rec-*` rows | `--verify-render` passes (2,401 ticks, none missing or differing) and two runs record the same: the SU34.PT identity (checkpoint code 55), the Flanker-family rig and the gun's and AA-11's rounds round-trip |
| `replay-a2g-b2-ripple` (slice B2-9, 2026-11-15) | A B-2 bay ripple: twelve Mk 82 from the left bay's grid through `--a2g-run --attack release --record --verify-render` | 6,459 ticks compared with none missing or differing, and `--recording-log` reads back 11 releases and 11 outcomes |
| `replay-a2g-b52-ripple` (slice B52-9, 2026-11-16) | A B-52 bay ripple: eight Mk 82 from the forward bay row through `--a2g-run --attack release --station 2 --count 9 --record --verify-render` | 6,280 ticks compared with none missing or differing, `--recording-log` reads back seven releases and outcomes, and the recorded bay device is shut at the start and the end and at 0.95 or more at every release |
| `replay-a2g-b1b-ripple` (slice B1B-9, 2026-11-16) | A B-1B bay ripple: eight 0.1 s presses 0.5 s apart on 20 Mk 82 in the forward bay row through `--a2g-run --attack release --station 0 --count 20 --hold 0.1 --record --verify-render` | 6,329 ticks compared with none missing or differing, `--recording-log` reads back eight releases and outcomes (seven until BMB-AI2 held the press made while the doors open, round 7, 2026-11-16: the check now pins exactly eight), and the recorded bay device is shut at the start and the end and at 0.95 or more at every release. A one-tick tap (the default hold) still loses a press to the release interval on this aircraft (6 of 8 now, 5 of 8 before BMB-AI2, 9 of 12); the 0.1 s press does not |
| `replay-a2g-tu160-ripple` (slice T160-9, 2026-11-16) | A Tu-160 bay ripple: six Mk 82 from the forward bay row through `--a2g-run UKR QUCOL --attack release --repeat 6 --gap 0.5 --aircraft tu160 --station 0 --load MK82 --count 6 --record --verify-render` | 6,156 ticks compared with none missing or differing, `--recording-log` reads back five releases and outcomes of the six presses, the recorded bay device shut at the start and the end and at 0.95 or more at every release |
| `replay-rec-f111-2v2` (slice F111-9, 2026-11-16) | The F-111F as the player and in both wings, a 2 v 2 recorded three times (`--aircraft f111 --probe-friendly-aircraft f111 --probe-enemy-aircraft f111 --probe-fight 2:2`) | `--verify-render` passes (2,401 ticks) and two runs record the same: the F111.PT identity round-trips with its swivelling racks (before F111-9 every tick differed: `same_stores` did not leave out `Rack::swing`) |
| `replay-rec-f111-takeoff` (slice F111-9) | An AI F-111F pair takes off from Athens, 30,000 ticks recorded once with `--verify-render` | PASS (30,001 ticks); the log's flaps-up samples draw the wing through at least 8 degrees of sweep (the schedule: spread to 250 knots, held at 26 degrees by the default load's outer stores from 321, 26 degrees in all) |
| `replay-a2g-f111-ripple` (slice F111-9) | A pylon ripple, four Mk 82 from the outer pair at 650 knots (`--a2g-run --attack release --station 2 --count 4 --record --verify-render`) | 2,571 ticks compared with none missing or differing, four releases and outcomes read back. The bay cannot be flown by hand (its two Mk 82 are over the maximum takeoff weight): it is covered by `ai-strike-f111-bay` |
| `replay-a2g-tu26-ripple` (slice T26-9, 2026-11-16) | A Tu-26 bay ripple: eight Mk 82 from the belly bay row through `--a2g-run --attack release --station 4 --count 8 --repeat 10 --gap 0.5 --record --verify-render` (ten presses: before BMB-AI2 two were lost while the doors opened and the check took 7 or 8; now all eight leave and it pins exactly eight bombs, round 7, 2026-11-16; eight one-tick presses 0.5 s apart still give seven, the release interval's loss) | 6,315 ticks compared with none missing or differing, `--recording-log` reads back eight releases and outcomes (the bombs' own weapon, a surface weapon's miss is in the log too), and the recorded bay device is shut at the start and the end and at 0.95 or more at every release |
| `replay-a2g-tu95-ripple` (slice T95-9, 2026-11-16) | A Tu-95 bay ripple: six Mk 82 from the belly bay through `--a2g-run --attack release --station 1 --count 6 --record --verify-render` | 6,154 ticks compared with none missing or differing, `--recording-log` reads back five or six releases and outcomes, and the recorded bay device is shut at the start and the end and at 0.95 or more at every release (`tu95_check`) |
| `replay-a2g-su34-ripple` (slice SU34-9, 2026-11-16) | A Su-34 ripple: four Mk 82 from the wing row's pylons through `--a2g-run --attack release --station 2 --count 4 --record --verify-render` | 3,388 ticks compared with none missing or differing, `--recording-log` reads back four releases and outcomes (three at least pass), and the recorded bay device never leaves shut (the Su-34 has no bay) |
| `replay-a2g` | Ten short passes recorded by `--a2g-run --record --verify-render` (about 80 seconds in all): a Mk 82 AIR 7-pack, a Mk 82 that lands before its fuze arms, an LAU-61 held for a second, a CBU-87, a GBU-10 on the aircraft's own laser, the same with the laser switched off mid-fall, a GBU-10 on a buddy's laser (not replayed), a GBU-30 JDAM and two HARMs (BORE and cued from the HARM page); then the pack again and the format 2 recording (slice RA) | Each pass replays tick for tick as the run drew it, every bomb's fall included. The pack is one `weapon.release` listing seven rounds in order with the CCIP sight, and seven bombs fall in the Tacview file; the player's stores are recorded (`stores.layout` and its stations, one loaded) and a `stores.items` entry shows the pack's station empty, and since slice BP3 every tick's stores replay as drawn too. The dud has a `weapon.dud` and a missed outcome that says why, and damages nothing. The rockets are the rocket class with salvo numbers 1 to n and the pod running down. The cluster bomb has a `weapon.cluster`. The laser bomb is released guiding on its own spot, its outcome and the tank's loss name the laser (`hit by a GBU-10E guided on the shooter's own laser`); with the laser switched off it goes blind and the words say `your laser went off`; the buddy's bomb follows a spot owned by the other seat; the JDAM stores its point; the HARMs launch in `boresight` and `cued` and the radar's loss names the AGM-88, with the HARM page's designations. A moving laser spot is followed by `player.laser_spot` entries no more than ten a second, a guided weapon by `weapon.aim`, and each bomb ends in a `weapon.impact` (with the miss distance from its aim point, and `blind` after the laser went off) (FX-R). Two identical pack runs make recordings that match; the format 2 recording still opens |
| `replay-a2g-f117` (slice F117-9, 2026-11-15) | The F-117A's IRADS laser pass (`--a2g-run UKR QUCOL --attack laser --aircraft f117 --load PAVEWA3`, `--record --verify-render`) read back at five samples a second, the HUD camera and the four Sensor Views drawn twice at tick 3000, then an AI F-117 wingman's release on the lead's spot (`--pilot ai --order engage-my-target`), recorded the same way | Both recordings replay tick for tick; one guided release from station 0 on the shooter's own spot; the bay station recorded at x -3.0 ft, location 131, `GB27.SH`; the human's recorded `bay` device is shut at the start and end and fully open at the release, the AI's shut throughout; the five stills draw the same twice and differ from each other |
| `replay-surface` | The Ukraine armored column (`~QUCOL`, seed 1, heavy defenses) recorded for 560 seconds by `--surface-objective --record --run-on` (about 100 seconds), then a vulnerable pass at the same column, a pass at the Vietnam flak (`~QTAAA`, 6,000 ft) and the format 2 recording the `tore-replay` tests keep (slice RP1) | The format 3 recording replays every one of its 67,000 ticks as the run drew them (`--verify-render`: the tanks' poses, debris, flak, craters, fires, rounds and the aircraft). `--recording-info` says format 3, names the ground target and lists the surface units. `log.jsonl` is valid; its header holds the ground target; at least five column tracks move more than 500 ft and the three destroyed tanks stay where they died; every rearm event has the matching rail changes on its tick (and no truck rearms before 240 s) and every refill the matching magazine record; every surface burst and launch belongs to a surface id; each destroyed tank has a `surface.wreck` with a burning fire. The summary names the units (`SA-6 #9`, never a raw id), counts rearms and says when the targets fell. Two identical runs make recordings that match. The vulnerable pass records `it was shot down by an SA-6` on the player's loss and a hit by a surface unit, and the summary line names it; the flak pass records flak with its own effect code; the format 2 recording still opens and exports with no surface content |
| `replay-city-strike` | `--surface-objective UKR QUCOL --city-strike 12 --run-on --seconds 40 --record --verify-render`: twelve Mk 84s, four a pass, on the city fill buildings nearest the column, then `--recording-info` and `--replay-stills` at three ticks at night (nr-fx3) | The run flattens at least 12 buildings and replays tick for tick; the recording's `City` line says the same number were knocked down, in 3 changes (one a pass); the viewer opens the recording and writes the three stills (before the strike, after the first pass, at the end) offscreen |
| `replay-city-smoke-seek` | The same recorded city strike, then `--replay-stills` of the burning city three ways, each ending at tick 5000 by day: played through (frames every few hundred ticks), opened cold on tick 5000, and reached by jumps forward and back (nr-fx4) | The `Replay city smoke` line (burning buildings, fires, puffs, tallest column, a digest of every puff) at tick 5000 is identical in all three; the columns grow during the play-through; a jump back to tick 2450 shows only the first pass's buildings with young columns. `TORE_REPLAY_CITY_CLOCK=0` brings the old wall-clock columns back and the scenario fails |
| State checks on every log | (inside the above) | Alive with zero hit points, dead with hit points, more hit points than the maximum, negative fuel, outside the world, over 20 G, a launch after the shooter was lost, two outcomes for one shot, a decoy count that goes up or below zero, an aircraft that comes back to life, a delivered radio line with no speaker or text, the same line twice within 0.2 s, an aircraft speaking more than half a second after it was lost |
| `replay-corrupt-*` | Thirteen kinds of damage to a real recording: empty, 100 bytes, text, random bytes, bad magic, future version, half and a third of the file, missing tail, one byte short, flipped bytes, zeroed block, duplicated tail | Unreadable files exit 1 with a clear message. Readable but damaged files still export and always say `Problem`. No panic, no bare error |
| `replay-watch-corrupt-*` | The same damaged files given to the viewer (windowed) | Unreadable ones exit 1 with the same messages. Damaged ones open, draw a real frame and exit 0 |
| `replay-view-*`, `replay-ticks-*`, `replay-aircraft-*`, `replay-drone-*`, `replay-ui-*`, `replay-panels`, `replay-menu-pages` | About 140 captured frames: all twelve flight views, first tick, second, last, one past the end and far past it, each aircraft, the drone, look-at objects (including missing ones), every interface part, every debug panel, all seven Escape menu pages | Every frame is a real picture (not blank, not black, the timeline strip present unless hidden), the run reports the capture, and frames that should differ do differ. A person still has to look at them (see below) |
| `replay-view-model-*` | Each of the fourteen aircraft as the player and as the enemy, seen from outside in the viewer | A real picture. The agent looked at all fourteen: each shows its own model, and the F/A-XX differs from the F-22N |
| `replay-flight-panels-*` | The debug panels (thought, telemetry, guidance, comms, right-click menu, all together) over a live flight in the air and on the ground, by GPU capture | A real picture |
| `replay-speed-*` | 1/8x to 16x forwards and backwards with `TORE_PERF_FRAMES` | The timing report prints |
| `replay-live-*` | Real windowed flights that record themselves: every aircraft in free flight, three Quick Mission separations, ground starts at three airports with wings of 1, 3 and 5, with and without sound | The recording is the only one in `replays/`, finished, reads back through info, log, Tacview and diff, and passes the same state checks. Ground starts must produce tower radio |
| `replay-keys-*` | Key presses sent to the game's own window: bookmarks, end flight (Ctrl+Q), chaff and flares, target designation and fire, Alt+F4 over the controls, graphics and sound screens (flight and viewer) and on the main menu, creator, terrain viewer, flight help, flight menu, flight map and first-run locate screen, and 17 seeded random key sequences in free flight, Quick Mission, ground start and the viewer (20 more seeds were run by hand once, all clean) | The game never crashes or hangs, exits on Alt+F4 (a random key may pick the menu's Exit to Desktop first, which is fine), and the recording it made is finished and valid with no pause without a resume. The chaff and flare keys leave four countermeasure events from the player |
| `replay-cli-bad-*` | 41 wrong options, paths and probe settings (numbers that are words, out-of-range rates and ticks, unknown presets, empty or missing output paths) | Exit 1 with a message that names the option and what was typed; no panic; no bare parser text |
| `replay-quick-restart-*` | Quick Mission launch and restart over 31 setups: five airports by three wing sizes, eleven separations (75 added by the lobby pass: `replay-quick-restart-sep75`, windowed), five mission presets | The restart restores the accepted start (`Quick Mission restart: PASS`) |
| `replay-settings-corrupt-*` | A damaged preferences, graphics, sound, replays or controls file in the data folder, then a real launch | The game starts on defaults and the session log says which file was ignored; a damaged controls file stops the start with a message that names the file and says to fix or delete it |
| `replay-screen-auto-delete-and-delete` | The Replays screen on real files, reached from the viewer's End Replay | Auto-delete keeps the newest N and everything marked Keep, never touches files that are not recordings, and Delete asks first and removes only the chosen one |
| `replay-import-keeps-good-data-*` | A failed import into a data folder that already holds a good import | The existing pack and remembered source are unchanged and the menu still draws |
| `replay-snapshot-*` | 22 menu screens from `--snapshot-state`, including replays, controls, sound, graphics and the locate screens | A real picture |
| `replay-diag-*` | `--diagnostics-self-test` in every mode and `tools/check_startup_diagnostics.py` | Right exit codes, right messages, the report and session log written |
| `replay-audio-*` | Start-up with sound on and the audio device missing or wrong (`PULSE_SERVER`, `PIPEWIRE_REMOTE`, `ALSA_CONFIG_PATH` pointed nowhere), and a replay played with sound | Never a crash. A missing device is reported as `Continuing without audio` |
| `replay-input-*`, `replay-tape-*` | `--list-inputs`, `--write-input-profile` then load it, refusing to overwrite, nine kinds of bad profile, eight kinds of bad tape, hand-made pilot input tapes replayed twice on every aircraft | Profiles that cannot load fail with a message that names the file. Tapes that cannot load exit 1 with a message. Two replays of one tape give identical results on all fourteen aircraft |
| `replay-import-bad-*` | Eleven kinds of bad game media given to `--import` in a fresh data folder | Exit 1 with an actionable message, nothing left behind. A damaged archive names the file and says what to do |
| `replay-combat-smoke-*` | The weapons acceptance probe on each aircraft | All 14 aircraft pass (the flight lane fixed the probe) |
| `replay-validate-creator` | The creator acceptance probe | Passes |
| `replay-script-*` | Hand-flown windowed missions from `--input-script` files: a ground-start takeoff (afterburner, pulsed stick, gear and flaps up once airborne; the weight-scaled stall speed gives 100 to 160 knot liftoffs and up to 16 s of roll, so the A-4E has a longer script) on all 14 aircraft, the same with the tower's landing clearance, a takeoff that presses G at about 45 knots with the wheels down, where the ground sensor must refuse it with its message and keep the gear down ([gear on the ground](../spec/gear-on-the-ground.md)), gear, flaps, hook, airbrake, afterburner and countermeasures on all 14 (hooks only on carrier types, no afterburner on the A-4E and Su-25), a held gun trigger, a missile fired at a designated target, an ejection, autopilot and NAV modes, a 6 G pull and roll, views, the pause menu and bookmarks, all sixteen cheats on and off through the menu, and a flight of three with an enemy ahead for the flight data link's cues (`replay-script-link-cues`: the wingmen are told to attack on contact and lock the player's bandit, so the sort warning reads; `replay-script-link-assign`: Engage my target assigns both, so it does not; `replay-script-link-sort`: Alt+0 then Alt+A deals three wingmen a bandit each, the HUD says `Sort: 3 assigned` and the calls follow 3.5 s apart; frames of each go to the `TORE_SCRIPT_OUT` folder) | The recording holds the events the keys should cause: device positions over time, exactly one chaff and one flare, launches from the player, `aircraft.ejected`, autopilot effects, three bookmarks, paired `system.cheat` events |
| `replay-script-*-mouse` | The mouse: the Replays screen's Keep, Tacview, Debug log, Delete with confirmation and auto-delete panel; the debrief's NEXT, PREV and OK; the Escape menu's tab, row, Keyboard shortcuts, Restart and Resume | Files written and removed, settings saved, five distinct debrief pages, a restart recording pair, and every screen a real picture |

## Bugs found and fixed

| Symptom | Cause | Fix |
| --- | --- | --- |
| Alt+F4 did nothing while the controls, graphics or sound screen was open in flight, so the game could not be quit from the keyboard there (found when a random key sequence opened the controls screen, in the fuzz scenario) | Those screens take every key press first and the quit shortcuts were only checked after them, against the controls screen's own note and INPUT.md | `10663de`. The quit shortcuts are checked first. Regression scenarios `replay-keys-alt-f4-*` (three fail without the fix) |
| A flight started with `--free-flight` never recorded, even with `TORE_RECORD_MISSIONS=1`, against REPLAYS.md and DEVELOPMENT.md | Only the menu's Free Flight action started the recorder | `c0558da`. A direct start records like a menu start. `replay-live-*` cover it |
| The debug panels said "Mission recording is off (TORE_RECORD_MISSIONS=0)" in runs where that variable was not set | One message for every reason recording is off | `c0558da`. It lists the real reasons |
| A truncated or empty `FA_1.LIB` or `FA_2.LIB` ended an import with "invalid archive sentinel" or "failed to fill whole buffer" | The reader's error was passed up unchanged | `b6ce6d9`. The message names the file and says to copy it again or reinstall. Unit test in `tore-import/src/media_source.rs` |
| Recordings, summaries and Tacview files called the F-22N and the F/A-XX "F-22" | Both borrow the F-22's PT name | `441ecab`. They use their own labels, F-22N Raptor and F/A-XX |
| Alt+F4 did nothing on the first-run locate screen (nothing detected), so it could only be left with the mouse or the desktop's own close binding | The screen handled only its own keys | `f5f6f04`. It quits on Alt+F4 and Command-Q like every other screen. `replay-keys-alt-f4-on-locate-screen` |
| A damaged `graphics-v1.conf` was ignored without a word, unlike the other settings files | The loader threw the error away | `6e6b95e`. It logs the file and the reason |
| A damaged `input-v1.conf`, which loads by itself at start, stopped the game with a message that did not say what to do | Only the file name and parser text were given | `2aff9c2`. It said to fix the file or delete it. Since 2026-09-29 the game falls back to the default controls with a logged warning instead of stopping (an explicit `--input-profile` still fails), checked by `replay-settings-corrupt-input*` |
| `--ai-probe-ticks 0` said "AI probe tick limit exceeded"; a word for `--ai-probe-ticks` or `--ground-start` gave the parser's bare message | Same | `a609062`. They name the option and the range |
| `--record-mission ""` ended with "the mission recording could not be finished; see the session log" | The empty path was accepted | `00b524c`. Refused up front with a clear message |
| The ground start hint said "PageUp adds throttle", but PageUp stopped setting throttle when the Fighters Anthology keys replaced the development ones on 2026-09-26 | The text was never updated | `ddba864`. It names key 5, full throttle. Checked by the ground start `replay-live-*` scenarios |
| An input profile that could not load (empty, wrong header, bad action, bad bytes) did not say which file | The parse error was passed up without the path | `9e1feb6`. Every profile error starts with the path |
| Name labels of aircraft close together in the replay viewer printed on top of each other and could not be read | Each label was placed at its aircraft with no regard for the others | `e3a97f7`. Labels stack upward until clear |
| The mission summary reported a parked wingman as "airborne 0:20.1" for the whole flight | The airborne flag means "in play" and stays set on the ground | `98db86d`. Only frames off the ground count. Unit test in `tore-replay` |
| A cockpit message from the game (the ground start hint) read "someone: ..." in the transcript | A missing speaker fell back to "someone" | `b0a0007`. Cockpit messages read "cockpit". Two text golden files updated |
| `--rate abc`, `--ids a,b`, `--from x`, `--replay-tick -5`, `--replay-speed fast`, `--replay-aircraft x` said "invalid float literal" or "invalid digit found in string" | Parser errors passed up bare | `f4e2a9d`. They name the option and what was typed |
| `--recording-log --out FILE` and `--recording-acmi --out FOLDER` said "File exists (os error 17)" or "Is a directory" | Bare I/O errors | `cb1fd77`. They name the path and what it was used for |
| `--recording-log --from 20 --to 10` wrote an empty log, and `--ids` with an aircraft that is not in the recording silently dropped it (round two) | Nothing checked either | Both are clear errors now (`--from 20 is later than --to 10`, `--ids names aircraft 999, which is not in the recording`) |
| `--import notes.txt` named the file's folder, not the file (round two) | The fallback to the folder lost the chosen path | The message names the file and its folder |
| Every remaining numeric option (about twenty: `--separation`, `--flight-zoom`, `--window-size`, `--probe-*`, `--weapon-slot` and others) gave the parser's bare message (round two) | Parse errors were passed up unchanged | All say `--option needs a number (why)`; 23 scenarios cover them |
| `--list-inputs --no-controllers` still probed and warned about devices (round two) | The diagnostics ignored the flag | It opens no device and says so |

Round two also added the `--input-script` development option (docs/DEVELOPMENT.md) so tests can
press any key and click at any point in a windowed run; the keyboard handler is now a method
shared by the window and the script.

## Found and not fixed

- **`--combat-smoke`** failed on 13 of the 14 aircraft when this lane first ran. The flight lane
  found the probe was stale rather than the game, and fixed it; all 14 pass now.
- **AI behaviour flags in the recordings, for the AI lane.** The summary flags `ai_flipping` (an
  AI aircraft changing activity six times in a few seconds), `track_lost_early` (a missile losing
  its track within seconds of launch) and `control_oscillation` (a pitch control reversing eight
  times in under a second). The scenarios and recording files that carry them are listed in
  `.local/battery/ai-churn-flags.txt` (regenerate it from a run's `work/*/log/summary.txt`).
  None was investigated here.
- Running several windows at once, the head tracker warns "UDP 4242: Address already in use".
  Harmless.
- The recording's `fuel_lb` is internal fuel only, so a flight that burns its external tanks
  first (the default loadout) shows no fuel used in `summary.txt`.
- No landing was flown. A key script cannot fly an approach open loop; the tower's landing
  clearance is checked, the touchdown is not.

## Needs a decision

- **Direct `--free-flight` recording.** The fix makes a direct start record. Say if a developer
  launch should stay unrecorded instead (the docs and `TORE_RECORD_MISSIONS=1` say it records).
- **The airborne flag's meaning.** In a recording `airborne` means "in play" and `on_ground`
  is separate, so a parked aircraft is both. The summary now counts only the frames off the
  ground; Tacview and the log still write the flag as recorded.
- **Retracting the gear on the runway.** A scripted takeoff pressed G at 80 kt with the wheels
  on the ground: the gear came up, the aircraft kept rolling on its "wheels" and took off with
  the gear already up, with no crash or message. No spec or doc says what the game should do
  (retail refuses on the ground, but that is recollection, not evidence here). The scenarios
  do not press G until the aircraft is airborne.

## Needs a human eye or ear

- **Sound.** The lane proves that start-up with sound on works and that a missing device
  degrades to silence. It cannot judge how anything sounds: the radio and crew lines, tower
  voice, seeker and RWR tones, situation music, replay sound at 1x, and the volume mixer.
- **The captured frames.** The checks find blank, black and broken frames. They cannot judge
  taste, exact placement or art: the twelve views, name labels, trails, the timeline markers,
  the comms panel, the Escape menu pages. The agent that ran the lane looked at the views,
  interface parts, panels, menu pages and ground-start frames and found the label overlap
  above and nothing else. Frames after that fix were not all reviewed again.
- **Anything the mouse does that a snapshot cannot show.** The Replays screen, the debrief and
  the Escape menu are clicked through by script and checked by the files, settings and screens
  that result, but the main menu's own buttons, the Quick Mission creator's controls and the
  loadout screen are left to the menus lane.
- **The viewer's keys.** The replay viewer takes its own window events, so `--input-script` keys
  do not reach it (its mouse does); its keys are covered by the Hyprland driver, which reaches
  named keys and Ctrl combinations only.
- **The feel of flying.** The scripts hold a stick position for a set time; nothing judges how
  the aircraft responds beyond G, bank, speed and events.
- **Real controllers.** No gamepad or joystick was attached; profiles were tested as files.

## How the checks work

`tools/battery.py` runs each scenario's main command, then any `then` steps (more app runs, or
plain Python), and hands the work folder and the joined output to `check_work`. A scenario can
carry `known_failure`, which reports its failure as known and fails the run once it passes.
`_replay_checks.py` holds the Tacview, log and summary checkers (with unit tests in
`tools/test_replay_checks.py`). `_replay_tools.py` damages recordings, builds damaged game
media and tests captured frames. `_replay_drive.py` sends keys to a game window through Hyprland by process id. A window on a hidden
workspace has no focus, and the game ignores most keys without it (as it should: a focus loss
pauses the flight and drops held controls), so only named keys and Ctrl combinations that go
straight to the menus reached the game. That, not a defect, is why letter and digit keys did
nothing through `hyprctl`. The hand-flown scenarios therefore use `--input-script` (see
[DEVELOPMENT.md](../DEVELOPMENT.md#windowed-runs-from-scripts-and-agents)), which feeds keys and
mouse events through the game's own handlers with focus assumed, and the driver stays for the
replay viewer and for quitting with Alt+F4. The driver's scenarios set `TORE_PERF_ACTIVE=1`
with a large `TORE_PERF_FRAMES` (which keeps a timing run unpaused) and `TORE_RECORD_MISSIONS=1`
(which makes it record); the script ones need neither.

The driver starts its keys only once Hyprland lists the game's window (then waits `DRIVE_SETTLE`, 4 s by
default), for up to 90 s. Before 2026-10-13 it counted from the game's process start, and on a loaded machine
the window opened 5 to 8 s later (1.7 s on an idle one), so the first keys were dropped with
`send_shortcut: window not found` and `replay-keys-bookmarks` recorded 2 or 3 bookmarks instead of 4. The
driver prints `driver: window up after N s`.

## Unfired stores

`replay-stores-unfired` runs `--replay-stores-check` without a window or GPU.
It builds Su-27, F/A-18D, F-14, F-15C, F-16C, Mi-24, A-4E, Su-25 and F-4B loads from the import, records
station and count events without any projectile catalog, and compares replayed
store vertices with live flight in forward and reverse order. It also checks
each release point against the hung stores, using the same numbers printed by
`--stores-preview`. This covers missiles, bombs, pods, tanks, shoulder and
tunnel rails, separate pylons and twin racks.

`--replay-stores-check FILE` additionally checks every recorded layout's shape
availability and reports which shapes the old projectile-only loader missed.
Add `--render DIRECTORY` to draw before/after offscreen comparisons, one per
aircraft type at its first recorded layout. These are posed store inspections
in daylight over Ukraine, not captures of the recording's camera or terrain.
The before pictures reproduce the old loader with the same recorded stores;
the after pictures use the viewer's current loader. Use a private data profile.

`replay-stores-release-f15` and `replay-stores-release-f16c` fly each
aircraft's combat-smoke slots and round-trip their combat tapes after the
2026-10-15 station moves. Evidence stays inside the scenario's work
directory. The Mi-24 has no such scenario: its combat smoke already fails
on the base (the AT-2 guidance probe reports no target).
`replay-stores-unfired` checks the moved coordinates of all three. The geometry rules are in
the [stores specification](../spec/stores-on-pylons.md#the-f-16c-and-fleet-rail-audit).

## Selected object banner

`replay-banner-objects` records an air fight and a falling-bomb pass, then runs
`--replay-banner-check` on each without a GPU. The check visits every selectable
identity and samples continuing tracks once a second. Unit tests additionally
cover missing old fields, guidance and engagement changes, backward seeks,
selection and menu toggling. `--out DIRECTORY` writes CPU UI layout images at
4K, 1080p and small-window sizes; full scene renders still need a GPU. See
[the banner contract](../REPLAYS.md#selected-object-banner).
