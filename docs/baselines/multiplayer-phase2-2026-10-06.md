# Multiplayer phase 2: the lead's acceptance run (slice F2-X)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Run on 2026-10-06 on the development machine (Ryzen 9 7900X, 24 threads,
RTX 4070, Linux, Hyprland; other agents' builds and windows ran beside it).
Branch `mp/f2x-accept`, rebased onto the local `multiplayer` branch after
slices D12, B1, B2 and B3 (protocol 16). All four programs (`tore-app`,
`tore-server`, `tore-bot`, `tore-master`) were built at that one commit
before any scenario ran. Everything ran on loopback; nothing contacted the
public master. This is the lead's part of the row
[F2-X](../ARCHITECTURE.md#phase-2-slices). John's three-machine play is
still to do and has its checklist at the end.

## What the run covers

Each item of the smoke test, how it was checked and what came out. The
scenarios are in the [net lane](../testing/lane-net.md); every one passed
in the final run unless a row says otherwise.

| Item | Checked by | Result |
| --- | --- | --- |
| PvP with a kill limit | `net-server-pvp` (two bots, one kill ends it), `net-server-hunt`, and the PvP smoke test (`net-server-smoke-pvp`, limit 2 in all, four bots and an observer) | Pass. The scores name the limit ("ends at 2 kills in all"), the kill limit ends the mission, the winner is named, and an observer ten seconds behind is given the results |
| A hosting game and a joining player in PvP | `net-window-lobby` (the game hosts from Direct Connection's New, turns PvP, a bot joins and takes a slot), `net-window-host` (a hosted co-op mission, a bot joins and flies), and the host-leave scenarios below (a hosting window in PvP with a bot pilot) | Pass |
| Revival | `net-server-revive` (co-op), the PvP smoke test and the co-op smoke test (a bot ejects, hears "Press Enter to fly again", is told of plane 12 and is seated in it), and `net-window-away-lost` (Spawn in Aircraft in a window) | Pass |
| An observer with a delay | `net-server-delay` and the PvP smoke test (10 seconds behind, frames with aircraft in them, no plane, no debrief, results at the end), `net-server-observe` (live) and `net-window-observe` (the window: Watch, Tab, Home, End, Stop Watching) | Pass. The delay is a PvP setting (its row is greyed in co-op), so the co-op smoke test checks the observer is live |
| The crown passed | `net-server-king` (a server's first player wears it and changes the settings), `net-window-lobby` (the King gives the crown to a bot, which changes a setting as King, and gets it back when the bot leaves) | Pass, after a script fix (finding 2) |
| The idle AI, joined as a player | `net-server-away` (a bot says it is away and back), `net-window-away-watch` (a `tore-server` with `idle-ai 1`: away behind the flight menu, the AI flies, the observer screen of its own plane, a held Up arrow takes nothing back, Take Back Flight does, then Leave Game) | Pass (one flake, finding 4) |
| An away player's view in PvP | The PvP smoke test | Pass. An away player watches its plane 10 seconds behind, like every observer in a PvP game with a delay (by design: [the handoff](../ARCHITECTURE.md#the-ai-flies-an-idle-players-aircraft)) |
| Spawn in Aircraft after the plane is lost | `net-window-away-lost` (new): the AI flies the away player's F/A-18D, ten aces shoot it down, the observer menu's first row reads Spawn in Aircraft and Enter flies the player again in a new plane | Pass, after the fix in finding 1 |
| Leave Game as the host with a ready standby | `net-window-host-leave-handover` (new): a hosting window set to `idle-ai 1`, a bot pilot standing by; the host is away, the observer menu reads "Leave Game", one Enter | Pass. The game logs "Leave Game (handing the game over)" and "handed the game over to player 1"; the pilot takes the game over at tick 11,685, is live 182 ms later with 22 ticks fast-forwarded, and its world's tick keeps advancing |
| Leave Game as the host with no standby | `net-window-host-leave-confirm` (new): the same with a pilot that does not stand by | Pass. The row reads "Leave Game  ends the game for everyone"; the first Enter changes it to "choose again to end it" and leaves nothing; the second logs "Leave Game (ending the game)", and the pilot gets "Mission ended: the host left the game." |
| Leave Game on a dedicated server | The last step of `net-window-away-watch` | Pass. "Leave Game (not the host)": no hand-over and no confirmation |
| The live observer menu's words | The pictures of `net-window-away-lost` and `net-window-host-leave-*` | Title "MENU - THE MISSION GOES ON", first button "Back to watching" |
| Co-op, the same way | `net-server-fight`, `-revive`, `-away`, `-observe`, `-kick`, the co-op smoke test, `net-window-host`, `net-window-away-watch`, `net-window-away-lost` | Pass |

The final run's numbers: 13 headless net scenarios passed (about 85 s of
wall clock with 6 at once) and the windowed ones passed one at a time (`net-window-lobby` 115 s,
`-observe` 134 s, `-away-watch` 226 s, `-host` 27 s,
`-host-leave-handover` 330 s and `-host-leave-confirm` 261 s). The first
version of `-away-lost`, on the guide's mission, passed twice before the
rebase and failed on the rebased build (the AI's plane survived); it was
moved to the lone mission below and passed twice there (195 s).

## What the run found

1. **A mission ended before Spawn in Aircraft could be chosen (fixed).** A
   game a player hosts has no empty timeout, so when the AI lost the aircraft
   of its only away player the mission ended at once. A dedicated server's
   lone player had only its 60-second empty timeout to choose in. The host
   now counts a watching player whose lost plane the revival rules still
   allow a new one for as present (`Host::anyone_awaiting_spawn`, three tests
   in `host::tests::away_tests`; a player who stopped watching, or whom no
   rule lets revive, does not count). Not covered: a lone player whose
   aircraft the King released (no loss is noted for it), which the mission
   still does not wait for.
2. **`net-window-lobby` clicked the wrong row (fixed).** Since stage K's Host
   row the Game page's rows are 19 pixels apart; the script's click for
   Friendly fire landed on Lock sides. The scenario failed on a stale script,
   not on the game.
3. **The pictures of the multiplayer menu screens have no text.** Since John's
   sharp-text request (2026-10-05) the lobby, Settings and Direct Connection
   text is drawn on the graphics card at the window's resolution, after the
   captures the scripts take (`snapshot` and `shot` both). The observer
   screens and flight menus are drawn the old way and show their text. The
   lane's checks read the game's logs, which hold what the screens say, so
   nothing was lost, but a person looking at those pictures sees empty boxes.
4. **One flake: `net-window-away-watch` did not go away the second time (not
   reproduced).** On the first windowed pass, with headless scenarios running
   beside it, the second handoff never came (the game was not away a minute
   after the second Escape). It passed in the next three runs (including the
   final one). The server's log showed no stall beyond the screenshots' own
   (each `shot` stalls the window 3 to 5 seconds). It may be the second
   Escape landing while the observer screen was still closing; a repeat on a
   quiet machine would settle it.

## The new scenarios

`tools/battery_scenarios/net_accept.py`, listed in the lane's table:
`net-server-smoke-pvp`, `net-server-smoke-coop`,
`net-window-host-leave-handover`, `net-window-host-leave-confirm` and
`net-window-away-lost`. Choices worth knowing (all agent decisions):

- A hosting window is set up through the lobby's own panels: the Game type
  turned to PvP (the default Quick Mission has room for the hosting player
  alone, so no bot may join until it does), the idle time to 1 minute, and
  the Realism page's Damage to invulnerable, so the enemy cannot shoot down
  the plane the AI flies while the script waits.
- A bot that only watches (`--observe none`) can be a standby, but a bot
  pilot keeps the mission flying after the host leaves, so the hand-over is
  seen carrying on (the new host's tick advances for at least five lines).
- The away-lost scenario flies one F/A-18D against ten aces 50 nm away. The
  guide's mission is no use: its AI wings sometimes win, and at 20 nm the
  enemy arrives before the idle minute is up. In the lone mission the loss
  came 110 seconds into the mission in every run (twice headless, twice in the
  window).
- The smoke tests are headless and take 25 s (PvP) and 68 s (co-op).

## John's checklist for the three-machine play

Play it from the menus, one machine hosting (Multi, Direct Connection, New),
the others joining. Set the idle time to **1 minute** in the lobby's Settings
(Game page, "AI flies idle aircraft after") so you do not wait five.

PvP:

1. The King turns the Game type to PvP, sets a kill limit of 2 and the
   observer delay to 10 seconds, and turns revival on (Revival page). Check the
   other machines' Settings panels show the same and are greyed.
2. Two players take opposite sides, one more joins and presses Watch. Fly. The
   score board (the key in the flight) shows the limit; shoot someone down.
3. Eject once and press Enter when the prompt says so: you fly again in a new
   plane. Check the observer is about 10 seconds behind what you see.
4. The King gives the crown to another player (Players panel) and takes it
   back; the other player changes a setting while it wears it.
5. A kill limit is reached: everyone gets the SCORES and RESULTS pages and
   the same winner.

Idle AI (do this on the hosting machine, then on a joined machine):

6. Open the flight menu (Esc) and leave the controls alone for a minute. The
   AI flies your plane and you watch it on the observer screen; pressing the
   stick or keys takes nothing back.
7. Esc: the first row is **Take Back Flight**. Choose it: you fly again from
   where the AI left the plane.
8. Go away again and let the enemy shoot the plane down (or use a mission
   where it will). Esc: the first row is now **Spawn in Aircraft**; choose it
   and you fly a new plane under the revival rules.
9. Go away once more and choose **Leave Game**. On a joined machine it just
   leaves. On the hosting machine with a second machine standing by (Players
   panel shows it) the game carries on under the other machine; with none
   the row says it ends the game for everyone, and asks twice.

Co-op: repeat 1 to 3 and 6 to 9 with the Game type left on co-op (no kill
limit, no observer delay), including a revival after an ejection.

Look for: text on the live menu reading "MENU - THE MISSION GOES ON" with a
"Back to watching" button; no jump or freeze when the hosting machine hands
over; the sharp text on the lobby screens on each platform.

## What was not run

- Two game windows at once (a hosting window and a joining window in one
  scenario). Each window scenario here has one window and bots for the other
  players; John's three-machine play covers two real games.
- The crown passed during a flight (the lobby test passes it in the lobby),
  and a King's Release of an away player's aircraft in a window.
- The multiplayer debrief's SCORES and RESULTS pages in a window; the bots
  print the same rows and the pages have their own headless render tests
  (slice F2-D).
- The full single-player baseline: no simulation or single-player code
  changed (the one host fix is multiplayer only).
- A repeat of `net-window-away-watch` on a quiet machine (finding 4).
