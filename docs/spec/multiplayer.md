# Multiplayer

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode. This file records what a Fighters Anthology player experiences in
multiplayer, from the retail manual. T.O.R.E's own multiplayer design is
deliberately different in several places and lives in its own guide,
[MULTIPLAYER.md](../MULTIPLAYER.md), which lists every departure from this
page. Delivery stages are in the [multiplayer plan](../multiplayer-plan.md).

## What the player sees and does

**Connecting.** The Choose Activity screen's MULTI menu offers Serial (E),
Modem (M), IPX/SPX Network (X), TCP/IP Network (T) and Disconnect (D).
Disconnect is grayed out until connected and works any time before flight. Each
connection screen asks for a callsign. One player is the host and the others
"answer" or "join". The manual advises that the player with the fastest computer
host. On a LAN, the host clicks NEW, the others click JOIN, callsigns appear in a
Players dialog and the host clicks START. Over TCP/IP, players exchange the
host's IP address by other means (the manual suggests a chat room) and type it
under OPTIONS. A message window in the upper right shows connection status and
player messages.

**Choosing a mission.** Once connected, everyone returns to Choose Activity. The
host alone chooses Single Mission, Airbase Assault or a Quick Mission and sets
its parameters. Each participant then chooses FRIENDLY or ENEMY. In a single
mission the host is always friendly. Everyone sees what the host is doing; the
settings a client cannot change are grayed out on their screen.

**Quick Mission.** The host uses the normal Quick Mission Creator plus extra
scoring parameters (below). Each player chooses their own aircraft type and
arms it on the normal Load Ordnance screen. Everyone must press FLY before the
mission starts. All players start airborne.

**Single Mission aircraft.** Any aeroplane in the game may be flown, including
ones not offered in single player, but not helicopters or blimps. The aircraft is
chosen from the Mission Map screen, not the Aircraft Selection screen.

**Dying and revival.** A player who dies presses Enter to re-enter the battle in
the same aircraft type, with the host's revival weapons, just outside the battle
zone at the host's revival distance. The host limits the number of lives.

**In flight.** Any player can pause the whole game with Ctrl+P or Esc. There is
no time compression. Only the host can change mission parameters or use the
CHEAT menu. The host can choose SHOW PLAYER SCORES from the in-flight MULTI menu
to show scores and the time remaining. The find-nearest cheat keys (Ctrl+A,
Ctrl+Z, Ctrl+X) work when the Multi menu allows them
([keyboard](keyboard.md)).

**Telling friend from foe.** An X appears in the middle of the missile lock box
when the target is on the player's own side. With SHOW TARGET INFO on (Pref
menu), players' names appear beneath their callsigns. An IFF squawk (U) on the
selected target returns a Friendly message for a same-side target.

**Messages.** Before flight only SEND TO ALL is available. In flight a player
can message any aircraft they can see. Messages appear at the bottom of the
cockpit; opening the in-flight menu bar suspends sending.

| Key | Action |
| --- | --- |
| `~` | Start a message to the current receiver (default SEND TO ALL) |
| Tab, or Alt with the arrow keys | Cycle receivers |
| Enter | Send |
| Esc | Abort the message |
| Left Shift+`~` | Send to friendlies (once sides are chosen) |
| Right Shift+`~` | Send to enemies (once sides are chosen) |
| Alt+`~` | Send to the designated wingman (in flight only) |
| Ctrl+`~` | Send to the currently locked target (in flight only) |
| F1 to F12 | Send the matching line of `CHAT.TXT` |

`CHAT.TXT` holds up to 12 single-line messages. A line may start with
`SEND TO FRIENDLIES\`, `SEND TO ENEMIES\`, `SEND TO WING\` or `SEND TO TARGET\`;
lines without a prefix go to everyone unless the player picks another receiver.

**Leaving.** If the host exits to Windows, chooses DISCONNECT or switches
application with Alt-Tab, everyone is disconnected and the game ends. A client
can leave the same ways without affecting the game. Any player who exits the
mission with Ctrl+Q ends it for everyone.

**Debrief.** INCOMPLETE appears as an outcome only in multiplayer and the
airbase mode ([debrief format notes](../formats/debrief.md#page-contents)).

## Numbers

| Value | Setting or limit | Source |
| --- | --- | --- |
| Players, IPX/SPX LAN | 2 to 8 | p. 31, p. 36 |
| Players, TCP/IP | Up to 8, "4 players optimum"; the MULTI menu text says up to 4 | p. 31, p. 47 (conflict recorded below) |
| Players, modem or serial | 2 | p. 31 |
| Modem speed | 14,400 baud or faster | p. 34 |
| FIGHT TYPE | SIDES or FREE FOR ALL | p. 43 |
| KILL TALLY | TOTAL KILLS, TOTAL DAMAGE (to opponents) or KILL RATIO (kills to deaths) | p. 43 |
| TIME LIMIT | 1 to 30 minutes | p. 43 |
| KILL LIMIT | 1 to 10 kills; reaching it ends the game | p. 43 |
| KILL OWNER | TOTAL, BY ONE SIDE or BY ONE PLAYER | p. 43 |
| NUMBER OF REVIVALS | 0 to 10, or UNLIMITED | p. 43 |
| REVIVAL DELAY | 0 to 5 minutes | p. 43 |
| REVIVAL DISTANCE | 1 to 40 miles from the battle | p. 43 |
| REVIVAL WEAPONS | WITH MISSILES; WITHOUT MISSILES (keeps air-to-ground missiles); WITH BULLETS (guns only); HALF BULLETS (guns only, half the usual rounds) | p. 43 |
| Scored targets | Aeroplanes and helicopters only; ships, tanks and ground targets do not count | p. 43 |
| Kill before ejection | Counts as two kills | p. 43 |
| `CHAT.TXT` | Up to 12 lines, F1 to F12 | p. 46 |

## Airbase Assault

A multiplayer-only contest (manual pp. 47 to 49). Each player defends a land
airbase or a carrier group and attacks the enemy's. Players pick a side (at
least one blue and one red), then one of four red or four blue bases. The host
picks one of the 16 theaters and sets aircraft count, base defence strength,
support craft, weather, revival location and standard or custom loads. Each
player picks markings and up to six aircraft types held as reserves at their
base. After a loss, Shift+1 to Shift+6 revives the player in the matching type
until none remain; Shift+I lists the available aircraft. The game ends when all
human-occupied bases on one side are destroyed. Objectives differ per theater
and per base type; generally the support structures or escort ships are the
targets, not the carrier or base itself.

## Edge cases

- Switching application while connected disconnects that player; a host doing
  so ends the game for everyone.
- Settings a client may not change are shown grayed out, not hidden.

## Unknown

- The TCP/IP player limit: the manual says both 4 and 8. Next step: inspect the
  retail connection screen and its limits with the retail media.
- The wire protocol, TCP port number, what state the host sends and how the
  retail game handles lag. Not player-visible beyond its effects and not needed:
  T.O.R.E does not interoperate with retail multiplayer.
- The in-flight MULTI menu's full contents beyond SHOW PLAYER SCORES. The labels
  are in the recovered `FMENUD.MNU` tree ([menu format](../formats/menu.md)).
  Next step: list the Multi subtree from an imported copy.
- The mission builder's multiplayer parameters (manual p. 214). Belongs to M4.
- How revival picks a position "just outside the battle zone", and whether the
  scoring screen is part of the debrief.
- What the X in the lock box and the names under callsigns look like on screen.
  Next step: retail screenshots.

## Source notes

Fighters Anthology manual, chapter 2 "Multi-Player Games" (print layout dated
1999-05-24, pp. 29 to 49), read from the local text extraction in
`.local/missile-update/manual.txt`. The debrief INCOMPLETE rule comes from the
debrief format notes. No retail multiplayer session has been run.
