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
The file's exact format, its shipped lines and its limits are
[below](#chattxt).

**Leaving.** If the host exits to Windows, chooses DISCONNECT or switches
application with Alt-Tab, everyone is disconnected and the game ends. A client
can leave the same ways without affecting the game. Any player who exits the
mission with Ctrl+Q ends it for everyone.

**Debrief.** INCOMPLETE appears as an outcome only in multiplayer and the
airbase mode ([debrief format notes](../formats/debrief.md#page-contents)).

## Retail connection screens

What a player sees between Choose Activity and the first mission, from the
installed game's data and John's two retail screenshots of 2026-10-01. Build
identity: `FA.EXE` 1.02F (SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`), `FA_1.LIB`
and `FA_2.LIB` of the same install; the 1.0 disc holds the same pictures byte
for byte. Evidence labels: **R** read from a retail resource, **S** static
reading of the executable, **M** measured on a screenshot, **I** inference,
**U** unknown. Byte layouts, rectangles, fonts and pieces are in the
[menu format notes](../formats/menu.md#multiplayer-connection-screens-dlg-records-panels-and-widget-pieces-ef0-2026-10-01);
this section is what the player gets. T.O.R.E's own lobby does not copy these
screens; it reuses their art and pieces (see the [guide](../MULTIPLAYER.md)).

**Choose Activity's Multi menu (R, `CHOOSEM.MNU`).** The bar is `?`, `Pref`
(Graphics prefs, Screen resolution 320 by 200 to 1024 by 768, Sound prefs) and
`Multi` with six rows, each with a letter: `Serial...` (e), `Modem...` (m),
`IPX/SPX Network...` (x), `TCP/IP  Network...` (t, two spaces in the label),
`Disconnect...` (d) and `Airbase Assault` (a). Disconnect is grayed out until
connected (the manual; the menu data holds no state, so how the game decides is
U).

**The shared frame.** Every connection screen has a 640 by 480 background with
its title text, the ANTHOLOGY badge and the grey `?` bar baked in, and a
second menu bar at the top (the `?` with `Exit to Windows [Alt-F4]`, R). There
is no title font: the words MODEM CONNECTION, NETWORK CONNECTION and DIRECT
SERIAL CONNECTION are part of each picture (R).

**NETWORK CONNECTION (TCP/IP and IPX/SPX, R, S, M).** Background `NETIPX3`
(the steel gantry photograph), a grey bolted panel 619 by 395 at (10, 80)
drawn by the game, and `NEWNET`'s controls. Both protocols use this one screen
(the title reads `TCP/IP Network connection` or `IPX/SPX Network connection`,
S). Top to bottom:

- The title, centred in the panel, white on the panel.
- **Callsign:** and an entry field 139 pixels wide (empty on the screenshot).
- **Games**, a box holding a list of four rows (rows 18 pixels apart, black
  recessed bars 200 wide), a rocker beside it labelled PREV above and NEXT
  below, and a page counter under the rows: `PAGE`, then a small black box
  reading `1  of  0` (the current page number right aligned, then ` of ` and
  the page count). The list shows the games found on the network; more than
  four go on further pages.
- **Players**, an empty box to the right (254 by 105), for the players of the
  game selected (I).
- **Messages**, a wide box (549 by 88, eight lines of ten pixels) for connection
  and chat lines. In a game the heading line also carries a **Send Message**
  button (S; not on the screenshot, U when it appears).
- A row of four buttons 123 pixels apart, in this order: **New** (the default,
  blue, with the striped marker at its left), **Join**, **Options**,
  **Cancel**; each 85 pixels wide including its shadow.

Exact rectangles, fonts and pieces: the
[layout table](../formats/menu.md#what-newnet-adds-on-top-of-the-panel-s-m).
A headless render built from that table matches the screenshot to within image
resampling (`.local/mp-notes/stage-ef/ef0/`).

What the buttons do (S, and the manual): **New** hosts a game and opens the
*Players dialog* (below) while it waits ("Waiting for players to connect.");
**Join** connects to the selected game, or to the address typed under Options
(over TCP/IP); **Options** opens the options panel; **Cancel** leaves. When
joining, the Messages box reads "Attempting connection to 'Game host' at
<address>"; failures say "Failed to initialize network driver." (no driver),
"Failed to initialize connection.", "Connection to game host '<name>' failed.",
"Could not create network socket.", "Unable to connect.". Whether Join and
Options grey out with no game selected is U (next step: the enable calls in
the screen function at `0x492740`).

**Options panels (R, S).** `NETTCP` (TCP/IP, 465 by 395 at (89, 80)) holds three
buttons, **OK**, **Default**, **Cancel**, and three entry fields drawn by the
game: `TCP port number :` (a field about 36 pixels wide, 18 spaces of the panel
font), `Broadcast address :` and `Host IP address :` (78 pixels, 39 spaces
each), each with a paragraph explaining it: the port is needed only after an
error that the port is already in use; the broadcast address only if the
correct one cannot be found; the host IP address is "of the most help" to
connect directly to the player hosting. A line reads `This computer's IP
address: %s`. The title is `<protocol> Network connection options`. `NETIPX2`
(IPX/SPX, 465 by 275) has the same three buttons and one field, the 20 digit
network address (`address :` with a 61 space field), and the line `This
computer's network address: %s`. The prompts for each field are "Enter an
unused TCP port number:", "Enter broadcast address:", "Enter Host (caller) IP
address:" and "Enter player's 20 digit network address:", with "Invalid
network address" on a bad entry. Default restores the defaults (a normal
button; what they are is U). The settings live in files the game writes next to the
program (`NET.DAT`, `IP.CFG`; not retail data).

**Callsign.** The callsign is chosen from a **Choose your
callsign.** list (`CALLSIGN`: fourteen rows, a rocker, OK and Cancel; the
retail names, Acid, Bat, Boomer and so on, and the last row `>> enter your
own <<`, R) or typed in an **Enter your callsign:** prompt (`EDITSIGN`); which of
the two a screen offers when is U. The connection screens will not proceed without one ("Please enter your callsign
before connecting." on the modem screen, S). The saved callsign is kept in
`NET.DAT`.

**The Players dialogs (R, S).** After **New** the host sees `NETNEW`, a panel
619 by 375 at (10, 100) with a list of eight rows, 549 wide, and three buttons:
**Start** (default), **Reject**, **Cancel**. A joiner sees `NETJOIN`: the same
list with only **Cancel**. The list is the players connected so far; how the
columns are laid out is drawn by the game (U; next step: the function at
`0x493160`). When the host starts a mission with the sides unsound the game says "Players
cannot all be on the same side.", "Players still choosing sides." or "Players
needed on other side." (S, I that this is the moment). Other lines: "Lost connection with <name>", "Lost
connection with <name>.  Disconnecting all computers.", "Connection process
interrupted by user.", "Successful connection!", "Unable to establish
connection.", "The game host has refused your connection.".

**The message prompt (R, S).** Before flight a player presses `~` and a panel
620 by 124 at (10, 200) asks "Enter message to send to all players" with one
entry field (up to 40 characters, I from the DLG value; the edit control's
typing font is the ten pixel monospaced `WHEELFNT`, S) and **OK** and
**Cancel**. Only SEND TO ALL exists before flight (the manual).

**MODEM CONNECTION (R, S, M).** Background `MODEM3` with a baked bolted panel
481 by 384 holding two white outline boxes (R). The upper box is a table of
**eight rows**: a check box at the left of each (the first row ticked on the
screenshot), a **Player name** column and a **Phone number** column (headings
at (186, 116) and (366, 116), M); the player picks a stored entry. The lower
box holds **Callsign:** and an entry field, **Modem port:** with the value
`Autodetect` (a pick of `COM 1` to `COM 8`, S) and the advice "If you are
having difficulties connecting, be sure that Windows has installed and
identified your modem correctly by checking your control panel settings."
Three buttons stand in a column at the lower right, 35 pixels apart and 118
wide: **Cancel**, **Answer**, **Call** (the default, blue, lowest). Related
dialogs: `MODEMCOM` (the **AT command strings:** panel with Initialization,
Dial, Listen for caller and Hangup prefixes and suffixes, OK and Cancel),
`MODEMSTS` (**Connection status**) and the pick lists `MODLIST` (modem
database, ten rows) and `COMLIST` (COM port, ten rows). The saved list is
written to `MODEM.DAT` by the game.

**DIRECT SERIAL CONNECTION (R, S).** Background `SERIAL3` with a baked panel
361 by 349, buttons **Call**, **Answer**, **Cancel**, 85 wide in a row, a
**Connection status** text, and fields for the callsign, the COM port and the
baud rate (110 to 115200, or Custom), with advice about 57600 and no flow
control. Two players only.

**The connected-state status window (R, S).** Once connected, the menus that
follow (Choose Activity, Mission Map, Aircraft Select, Load Ordnance and so
on) show a black recessed window with four bolts in the upper right of the
screen, 333 by 80 at (305, 2) (`MPSTATUS`), written in the nine pixel `MPFONT`.
It lists every player with a status word (Disconnected, Verifying, Choose
Activity, Single Mission, Quick Mission, Airbase, Airbase Options, Mission
Brief, Mission Debrief, Mission Map, Aircraft Select, Load Ordnance, Choose
Side, Chose Side, Choose Airbase Side, Chose Airbase Side, Initializing,
Waiting!, Replay, In flight, Ending Mission) in two columns (a per-player flag
picks the column, I: the side; the airbase side statuses print Blue or Red), and
below a rule it keeps the last chat lines, nine pixels apart: five
lines for two to four players, four for five or six, three for seven or eight
(S: the table at `0x4f7750`). Up to six received lines are kept, each at most
120 characters. This is the manual's "message window in the upper right"; the
survey that first took it for the in-flight pane was wrong. Its colours are
U (they come from the menu's palette).

**In flight (S).** A sent message is formatted `%s: "%s"` with `YOU TO ALL`,
`YOU TO FRIENDLIES`, `YOU TO ENEMIES`, `YOU TO WING` or `YOU TO TARGET` as the
name on the sender's own line and the sender's callsign on the others' (S,
`0x413640`); a typed message carries the default sound, the 2.5 kHz beep
`^BEEP2.5K`.
Where the pane sits in the cockpit, how many lines it holds and its colours are
U (next step: the in-flight message draw, reached from `0x4133c0`'s caller and
the receive path at `0x46d225`). John chose T.O.R.E's in-flight chat window
(top left, coloured by sender and receiver) himself, so retail's pane is a
reference only. `Show player scores?` in the in-flight Multi menu shows one of
three headings, `PLAYERS RANKED BY KILLS`, `KILL RATIO` or `TOTAL DAMAGE`, one per
kill tally mode; its layout is U (next step: the reader of the pointer table at
file offset `0x100cb8`).

The in-flight `Multi` menu (R, `FMENUD.MNU`) holds `Reduce bullet accuracy?`
(30% or 60%), `Reduce bullet damage?`, `Reduce missile accuracy?`, `Reduce
missile damage?`, `Reduce engine thrust?`, `Reduce radar look-down?`, `Allow find
nearest objects cheat?`, `Show weapon camera?` and `Show player scores?`.

### CHAT.TXT

A loose text file in the install root (591 bytes, 12 lines, CRLF, then a Ctrl-Z
and two more line ends), not in any archive; the 1.0 disc carries it
compressed inside `SETUP.ESA`. Each line is `receiver\text\sound`, case
insensitive. Lines 1 to 4 are F1 to F4 (to all), 5 to 8 F5 to F8 (friendlies),
9 to 12 F9 to F12 (target). The shipped lines (R):

```
send to all\Hurry up, I don't have all day\^shwtime.5k
send to all\I don't like this\^dntlike.5k
send to all\The worm has turned\^worm.5k
send to all\Splash one bandit!\^splbndt.5k
send to friendlies\Get this guy off me\^offme.5k
send to friendlies\I'm going after him\^igoaf.5k
send to friendlies\I'm taking damage\^imdmge2.5k
send to friendlies\Who's side are you on?\^whoside.5k
send to target\I've got a lock on you!\&rwrlock.5k
send to target\Eat hot lead\^hotlead.5k
send to target\Missile inbound! Break!\^missbrk.5k
send to target\Eject! Eject! Eject!\^ejectx3.5k
```

How the game reads it (S, `0x413140`): each line is cut at 159 characters. The
text before the first backslash is compared with the five keywords (`SEND TO
ALL`, `FRIENDLIES`, `ENEMIES`, `WING`, `TARGET`, case insensitive); a match
fixes the line's receiver and is removed, a line without a matching prefix has
none (it uses the receiver currently picked). The part after the last backslash
is the sound if it is at most 12 characters and ends in `.5K` or `.11K`; it is
upper-cased. **The text is kept to 50 characters** and the sound name to 40. A
line without a sound field is accepted (text only); a line whose last field is
not a sound is kept whole as text. The shipped file's sounds all exist in
`FA_2.LIB` (the `^` and `&` are part of the names). Pressing an F key loads that
line as the message and its sound as the message's sound, sets the receiver if
the line has one, and sends it. The default sound for a typed message is
`^BEEP2.5K`.

**In T.O.R.E** (EF1): the import keeps the file as `TORE_CHAT_V1` and
`tore_formats::chat::parse` returns its lines with the receiver, text and
sound under the limits above (sound accepted at 12 characters or fewer; the
40 is the retail buffer). Details and agent decisions: the [menu
format notes](../formats/menu.md#what-the-import-keeps-for-these-screens-ef1-2026-10-01).

**Who hears the sound.** The sound's name travels in the message itself (the
packet holds the text, a zero byte and the sound name, S, `0x413640`), and the
receive path reads it as a second string (S, `0x48d3c0`), so **receivers hear
it**. Whether the sender hears the sound too is not established: the send loop
treats the sender like any other addressed player and only labels the line `YOU
TO ...` (I, plausible both). Next step: trace `0x48d470`, which the receive
path calls after storing the strings.

### The host's mission-setting dialogs

Small panels opened from the host's multiplayer mission setup; the titles are
the dialogs' own (R). Each single-list dialog shows every choice at once, so
the row count is the number of choices (I): **Set the time limit.** (7 rows),
**Set the number of kills to end a scenario.** (7), **Set how kills end a
scenario.** (4), **Set the number of times to revive.** (7), **Set the time
delay before a revive.** (6, which fits the manual's 0 to 5 minutes), **Set the
distance away after a revive.** (5), **Set the weather conditions.** (6),
**Enter the pilot's name.** (one entry field, 25 characters), **Choose the
nationality of this object.** (15 rows with a rocker), **Set which screens the
player can access.** (four check boxes: Briefing Paper, Briefing Map, Select
Plane and Arm Plane screens), two nationality check box dialogs whose words the
game fills in (`MC_NAT`, 520 by 235, and `MC_NATF`, 620 by 404), and `MC_DLG`,
a larger picture dialog with two sliders and a rocker. What each row holds is
U (the values are in the program, not the dialogs); next step: read the
list fillers called from `0x4880d0`'s callers.

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
| Who is credited with a kill | The shooter whose recorded hit destroyed the plane, else the last shooter whose hit damaged it before it was lost another way (ejection, crash); nobody for a plane lost with no shooter. The score and the kill limit use the debrief's rule ([debrief spec](debrief.md#kills)) | agent decision, 2026-10-09 |
| `CHAT.TXT` | Up to 12 lines, F1 to F12; text 50 characters, sound name 12 | p. 46; S |

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
- The mission builder's multiplayer parameters (manual p. 214). Belongs to M4.
- How revival picks a position "just outside the battle zone", and whether the
  scoring screen is part of the debrief.
- What the X in the lock box and the names under callsigns look like on screen.
  Next step: retail screenshots.
- Connection screens (EF0), each with its next step:
  - What greys out when: Join and Options with no game selected, Start with
    sides unsound, Disconnect until connected. Next step: the enable calls in
    `0x492740` (network), `0x493160` (host) and the Multi menu's setup.
  - The text font in the callsign field and in a `NEWNET` list row once filled
    (list rows are `SMLFONT` from the code; the callsign field may be
    `WHEELFNT`). Next step: a screenshot with a callsign typed and a game
    listed, or the inline field routine behind `.button`.
  - The Players dialog's columns and the Games list's row text. Next step:
    `0x493160` and the list filler.
  - When the Messages heading's Send Message button shows, and its position.
  - The `EDITL/M/R` field in a `NETCEDT` prompt: its width (the DLG value 40 is
    probably the longest entry, but a 40 character field does not fit `NETEDT`).
  - The meaning of the 16 bit value after a text record's position (9, 10, 3) and
    of the header bytes `ff 00 00 00`.
  - Whether `MODEMSTS.DLG` (360 by 190) or `MODEMSTS.PIC` (361 by 192) is drawn.
  - The in-flight message pane's placement, line count and colours, and the Show
    Player Scores layout (see above).
  - Whether the sender hears a `CHAT.TXT` sound.
  - Whether `PANEL.PIC` tiles under a panel larger than 640 by 480 (none is).
  - What the setup thunk `_MultiPreload` does.

## Source notes

Fighters Anthology manual, chapter 2 "Multi-Player Games" (print layout dated
1999-05-24, pp. 29 to 49), read from the local text extraction in
`.local/missile-update/manual.txt`. The debrief INCOMPLETE rule comes from the
debrief format notes. No retail multiplayer session has been run.

The retail connection screens section comes from the installed game's data
(`FA_1.LIB`, `FA_2.LIB`, `CHAT.TXT`), from static reading of `FA.EXE` 1.02F and
from John's two retail screenshots of 2026-10-01 (NETWORK CONNECTION with the
TCP/IP panel, MODEM CONNECTION), registered on the background picture before
measuring. The lead's survey of the same day (`.local/mp-notes/stage-ef/`) is
where the pieces were first found; the
[menu format notes](../formats/menu.md#multiplayer-connection-screens-dlg-records-panels-and-widget-pieces-ef0-2026-10-01)
hold the byte layouts and the corrections to the survey.
