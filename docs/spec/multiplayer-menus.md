# Multiplayer lounges and game lobby menus

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. **Opinionated**, requested by John on 2026-10-22 in the
mp-menus task and annotated screenshots 64 through 68. These are T.O.R.E
presentation and interaction rules, not new claims about the retail game.
The [retail multiplayer specification](multiplayer.md) remains the evidence
for original behaviour. Coordinates and shortcuts below are agent decisions.

## Names and navigation

The direct browser's page title is **DIRECT NETWORK LOUNGE**, and its panel
heading is **Direct Network Lounge**. The internet browser uses **INTERNET
LOUNGE** and **Internet Lounge**. The main menu's Multi entries are **Direct
Connection...** and **Internet...**. The latter's hint reads “Open the Internet
Lounge to find or host a game.” Messages about the listing service itself
retain **Internet Lobby**.

After New or Join, the page title is **DIRECT NETWORK GAME LOBBY** or
**INTERNET GAME LOBBY**, determined by the originating lounge. Settings and
Players keep that title behind their panels. Mission and Loadout retain their
own existing page titles. All four multiplayer titles use the existing open
licensed Liberation Sans lettering, shadow, transparent canvas and imported
NETIPX3 background. No retail pixels are shipped.

On the 640 by 480 canvas, each lounge's Games list has five rows starting at
(48,185), 245 pixels wide, with 19 pixel spacing and 17 pixel row art. Its
16 pixel scroll bar is at (296,185), 93 pixels high. Players has the same
five-row height at (346,185), rows 225 pixels wide, and its bar at (574,185).
The five rows fill the available box height with the existing border insets.
Both lists support wheel, arrow buttons, track paging and thumb dragging.
Tab reaches Players after Games. Up/Down, Home/End and PageUp/PageDown move
the selected row and reveal it. On a controller, Left/Right move between
lounge controls and Up/Down operate the selected list.

## The ? menu

Every lounge, its Options panel, game lobby, and lobby sub-screen offers
**Return to Main Menu** and **Exit**. Click ? or press F1 to open it. The
controller Menu button opens it; Up/Down select and A accepts. Keyboard
Up/Down or Tab select, Enter accepts, M returns to main, X exits, and Escape
closes the dropdown. M and X act only while the dropdown is open.

Both destinations use the existing client disconnect and host shutdown path.
A client tells its host and releases its seat. A host uses the same shutdown
and succession behaviour as Leave/host quit. Its listing is withdrawn and
its sockets and router mappings are released. Return closes any lobby page
and both lounges and shows the root main menu. Exit closes the application.
Unsaved Settings edits are discarded by either destination.

## Settings draft

Agent decision: Game, Revival, Scoring and Realism share one local draft, so
Escape cancels consistently across tabs.
Left/right clicks and arrow keys change the draft shown on screen. Enter
stages the name or password text. **Close**, or F10, sends one combined
settings change if any accepted value differs. **Escape** discards the draft;
there is no separate Cancel button. Up/Down also reaches Close after the last
row; Enter or controller A applies there. This navigation is an agent decision
to preserve controller editing. Switching tabs neither applies nor
cancels. Closing an unchanged or fully reverted draft sends nothing.

Realism, Friendly Fire and Loadouts previously rebuilt the mission on every
click. They now rebuild once at most, together, when Settings closes. Other
settings and staged text are included in the same apply. Every player receives
the accepted settings and mission. Host validation still rejects forbidden
values. Losing the crown or changing flight phase discards the old draft.
Realism remains fixed during flight. An actual Realism change clears ready
marks, as the previous mission-change path did.

The existing Settings message gains an optional Realism payload under
[protocol 65](../formats/net-protocol.md). A wire extension is needed because
the former separate Settings and Mission messages would rebuild twice when
Friendly Fire or Loadouts and Realism changed together. No new message kind
or disconnect protocol is introduced.

## Minimum players

Implementation mode. **Opinionated**, requested by John on 2026-10-22.
The agent chose the defaults and permission policy below.

Settings > Revival keeps **Minimum players** beside AI aircraft. The row
uses the existing imported controls and font. Its choices are **none**, then
1 through the mission's human capacity, limited by the player limit and closed
slots. Keyboard arrows and controller navigation edit the same draft as mouse
clicks. Close or F10 applies it once; Escape discards it. Only the King can edit,
only between missions. A dedicated server also requires `king-mission open`;
otherwise its operator's value stays authoritative.

AI-on and AI-off games share a ready-player minimum. A counted player holds a
slot, is ready, can play the mission, and is not leaving. A first-player server
starts when the minimum is met. A King still presses Fly once all slot holders
are ready, or Force Start when some are not. Force Start requires the minimum
number already ready and at least one ready player; its existing 10 second
countdown can bring the remaining slot holders in with their current loadouts.
An empty slot never blocks either start. With AI off it stays empty: no AI
replacement, AI respawn or idle handoff. Join in progress continues to admit
later players into those slots when enabled.

An initial configuration without `min-players` uses 2 for AI-off PvP, 1 for
AI-off co-op, and 0 (**none**) for AI-on. An explicit 0 means no extra minimum
beyond the first ready player. AI-on `start now` retains its existing exception:
it can start empty with 0. AI-off always waits for at least one ready player.
Defaults are chosen when the configuration is loaded; later lobby edits to
AI or game type keep the displayed minimum. Changing those controls does not
silently replace a minimum the King has seen or chosen. Rotation overrides
remain round-scoped. The effective minimum is capped at human capacity, so a
closed slot or smaller mission cannot make the wait impossible.

The lobby and server log share the wait text, for example:
`Waiting for 2 more ready players (minimum 4).` There is no separate AI-off wait.
The minimum gates mission start only; departures in flight do not pause it.
Existing settings messages and `mission_locked` carry everything needed, so
this change does not extend the wire format.
