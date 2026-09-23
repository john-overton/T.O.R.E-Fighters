# Radio metadata

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, updated 2026-09-23. Local FA.EXE SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`.
The reviewed i386 PE image has base 0x400000. Its .data section contains
8-byte pairs of absolute pointers to NUL-terminated phrase and sample stem.
The bounded reader selects only reviewed entries, never executes code, checks
all pointers within file-backed section data, limits text to 127 ASCII bytes
and stems to eight safe characters beginning with ^. Unknown layouts fail closed.
The caller treats import failure as optional radio unavailability.

The reviewed table spans 0x4fef10..0x4ff8f0 in 1.02F and holds 329 pairs
naming 299 distinct stems. `tore_formats::radio::STEMS` lists one address per
stem; where a stem appears in several pairs, the standalone phrase is kept over
a sentence fragment (for example `^RDYCAT` "Ready on the cat" rather than
", ready on the cat"). On 2026-09-23 all 329 pairs were compared between the
1.0 disc build and 1.02F: every pair matches at the single 0x4608 shift.

| Table VA range | Contents |
| --- | --- |
| 0x4fef10..0x4ff168 | Numbers, military letters, colors, contact and waypoint report fragments |
| 0x4ff170..0x4ff288 | Player wing orders, weapon release calls, "You're the Wingleader now" |
| 0x4ff290..0x4ff3d0 | Hit and kill confirmation variants |
| 0x4ff3d8..0x4ff470 | Engagement acknowledgements, tally, hit and damage calls |
| 0x4ff478..0x4ff4c8 | Missile warnings, death cries, ejection, SAM/AAM launch |
| 0x4ff4d0..0x4ff570 | Launch report fragments, mission result lines, friendly-fire complaints |
| 0x4ff5a8..0x4ff688 | Radar, fuel, G strain, feet wet/dry, approaching target |
| 0x4ff690..0x4ff838 | Offensive and defensive coaching |
| 0x4ff840..0x4ff8f0 | Takeoff, catapult, landing and wind reports |

Only `^FIRGUN` has no matching recording in the local FA_2.LIB. The archive
also holds 703 speech recordings in all: 618 with the `^` prefix and 85 with
`#`. Recordings without a pair include callsigns, aircraft and weapon names,
clock positions and compass directions. The importer now keeps every `^`/`#`
`.5K` recording with a stem of up to eight safe characters, about 3 MB in all.
Importing a recording assigns it no meaning; only a reviewed consumer selects
it. Cache marker `TORE_SPEECH_V1` makes older caches reimport.

Say procedure 0x48d780 selects the wing pairs by event and parameters. Playback
0x48d610..0x48d6d2 appends the extension at 0x4ee2c4 when none is present.
The local archive census finds matching .5K entries in FA_2.LIB. This is table
and resource evidence, not filename-based inference of dialogue meaning.
B46 sender/reply behavior and unresolved branches remain in
[the AI specification](../spec/ai.md#b46-wing-command-receiver-contract).
FMENUD.MNU is imported by the existing bounded menu reader. Inspection of its
menu tree found no wing-order submenu. Host shortcuts are independent, documented in [input](../INPUT.md#player-wing-orders).

Cache entries TORE_RADIO_<stem> hold one ASCII phrase; the key names the reviewed
stem. Only listed stems have phrase text. Samples retain original resource names and
are parsed by the existing bounded PCM reader. No retail metadata or audio is
embedded into the application. Old caches keep text-only command operation until
reimport; audio initialization reports which optional airport voices are missing. Unknown report phrases are never mapped to plausible recordings.

The two airport mappings are in the same executable pointer-pair table as the
reviewed wing phrases. The local user-owned `FA_2.LIB` contains both exact
resources. `^CLRLAND.5K` is 4,307 decoded bytes with SHA-256
`4fd166d3538d360562dbe2ead54ba3868972b44711b2358acba919db881735bf`.
`^WELHOME.5K` is 4,032 decoded bytes with SHA-256
`80c55b6973d2c4e8e6bea345c54eacc60a47049b437e788c107aaa6cf8eab3b7`.
The parent review also found the airport consumer: APCommentProc loads pair
0x4ff8c8 at 0x48fbc8 and passes it to the phrase/stem buffer helper at 0x48fbd4.
Later it passes group 0x4ff978 with count 2 to 0x48e150, which selects one of
its adjacent eight-byte pairs, including 0x4ff980, then calls the same helper.
This establishes airport use of these recordings, not the complete original
state, eligibility or timing conditions. Source: the same hash-reviewed EXE and
its local `.local/weapons-research/native/fa-disassembly.txt`.

This establishes the phrase/sample pairs. It does not establish that retail
exposes TORE's authored select, repeat or cancel commands. TORE uses the
clearance recording for a successful player landing request and its repeat, and
the welcome recording after its deterministic landing-completion event and when
that latest reply is repeated. Those
event bindings are fitted. Selection, cancellation, rejection and runway
invalidation remain text only because no matching retail event recording has
been established.

Ejection clips also enter the existing serial speech queue from discrete escape
and cockpit warning transitions. Their reviewed filenames, source call sites
and unresolved speaker assignments are in [ejection source notes](ejection.md).
They do not require speculative phrase-to-speaker mappings.

## Cockpit voice source notes

Static research, 2026-09-23. FA.EXE 1.02F, SHA-256
e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c. Nothing run.
Behaviour spec: [cockpit voice](../spec/cockpit-voice.md). Fixed8 distances are feet * 256;
angles are 16-bit binary (0x4000 = 90 deg). `scaled(n)` = 0x48d5e0, n seconds
shifted left by `_timeCompression` 0x5528f8. `_currentTime` 0x5528e0 is seconds;
`_currentT` 0x5528c8 is quarter seconds.

### Symbols and globals

- `_PLANECommentProc` 0x48ec40 (symbols.json decimal 4779072; the task's
  0x48f880 is a typo). Returned by `_PLANEProc` kind 7 (0x49fb46); `Service`
  0x462e70 calls `_CallUtilProc(7)` at 0x46302e on every object update.
- `_APCommentProc` 0x48f6a0 (airport object). `@SAYTranslate@4` 0x490f30.
  `#` rewrite routine 0x490480 (unnamed). SAY output 0x48d470 (unnamed).
- `?COMMENT_TYPE@@3W4__unnamed@@A` 0x552fd0: no code reference found in the
  comment routines; unresolved and not needed.
- Comment buffers: text 0x552ff0, stem list 0x553050 (comma separated, `.5K`
  appended when no dot, 0x48d610). 0x48d410 clears both; 0x48d420(pair) appends
  pair {text, stem}, stems joined with 0x4f0a48.
- 0x48e150(ecx=pair array, edx=msg or 0, push n): picks pair index Rand(n), or
  (msg byte +1) % n when edx is a message record (that byte is set from a
  Rand(100) draw in MSGSend).
- 0x48e950(cx=sender, dx=recipient): submit comment; MSGSend(sender, recipient,
  delay 0, flags 2, 0x8000, type 0x24, payload text\0stem\0); sets speech
  deadline 0x552fdc = now + scaled(3). SAY output 0x48d470 also sets 0x552fdc
  = now + scaled(3) for every displayed message.
- Player fatal deadline 0x52253d (0x7fff none; 0x411330 keeps the earliest).
- Latches: 0x552fe0 mission accomplished said, 0x552fc8 failure said,
  0x552fcc `_home`, 0x552fe8 almost-home poll (raw +4 s).
- Per-aircraft (curThing 0x50ce80 base): +0x19b G fixed8 (0x50d01b); +0x265
  byte prev situation (0x50d0e5); +0x266 prev integer G (0x50d0e6); +0x267
  crossing count (0x50d0e7); +0x268 count minute (0x50d0e8); +0x26a last spoken
  truncated NM (0x50d0ea); +0x26c next coaching time (0x50d0ec); +0x26e hit-call
  8 s gate (0x50d0ee); +0x270 victim gun-hit gate; +0x16f flags (0x50cfef):
  0x8000000 feet wet, fuel 0x40000/0x20000/0x8000/0x10000 with said-latches
  0x400000/0x200000/0x80000/0x100000; +0xe3 state (0x50cf63); +0xee target
  (0x50cf6e); +0x11c selected station (0x50cf9c).
- curThingType 0x50d268: +0xe (0x50d276) = high byte of class word +0xd
  (0x80 fighter class 0x8000, 0x40 class 0x4000); +0xba (0x50d322) PLANE flags.
- PLANE flags bit 0x4 = multi-crew (comment, fuel, missile-warning and label
  gates). Bit 0x10 = seat (ejection). 55 retail PT records set bit 0x4.

### _PLANECommentProc 0x48ec40 control flow

1. Exit unless type class byte & 0xc0 (0x48ec4b). Exit if 0x552fdc > now, or
   0x52253d - 10 <= now.
2. Human (controller +0x10 bit 0x80): unless WNGPart==2 and 0x45f030(curId),
   mission result: `_MISSIONSucceededForThisPlayer` > 0 and not latched ->
   MSGSend(curId, 0x8001, delay 8, 0, 0x8000, 0x1f); < 0 -> type 0x20. Every
   4 s while > 0 and not `_home`: `_AlmostHome` 0x481b80 (home airport distance
   < 0xa50000 = 42,240 ft and +0x15 altitude < 0x4e2000 = 20,000 ft) and not
   `_OnTheGround` -> type 0x21.
3. `_gamePrefs` 0x4eb6f8 bit 0x100000 clear (Radio silence) -> call
   `_PLANESetFeetWet` 0x4a0510 and exit.
4. Exit unless +0xe3 >= 0x1f. Human with type flag 0x4 -> self, bp=0, speaker
   id = player. Human without it -> exit. AI: WNGPart(curId,&wing,&pos)==2,
   pos==1, leader = 0x45e630(wing) human, leader type flag 0x4 clear, leader
   +0xee == own +0xee, Dist <= 0x3a9800 (15,000 ft) -> bp=1, speaker target =
   leader. PushCurObj(speaker target).
5. Situation byte bl (0 if target 0 or state <= 0x1f): start 8. Aircraft target
   (kind 4): Dist >= 0x19c8000 (105,600 ft) -> bl=0; target +0xe3 in 1..0x1e ->
   bl=0; target +0xe3 == 0 -> bl=0xc. bit 1: AngleOffNose(target) <= 0x3ffc.
   bit 2: |AnglesOffNose(target, us) yaw| <= 0x3ffc. Aircraft target and class
   byte bit 0x40 -> clear bit 8. Distance NM: FeetToNM truncating (divisor
   0x17bc00 = 6076 ft fixed8; rounding variant adds 0xbde00).
6. Self only: G integer = +0x19b sar 8. If (prevG >= -1) != (G >= -1): minute =
   now / 60, reset count on new minute, ++count; 18 -> `^EASEUP` (timer +
   scaled 5); >= 20 -> 7-set 0x4ff5e8 BARF1..7, count 0, timer + scaled 15.
   Else if (G >= 5 or G <= -3) and -3 < prevG < 5 and Percent(30) -> 7-set
   0x4ff620 GRUNT1..4 BREATH2..4, timer + scaled 3.
7. bl != prev situation -> timer = 0. timer > now -> exit. timer = now +
   scaled(4 + Rand(4)), + scaled(2) if target and Dist > 0x1f4000 (8,000 ft).
8. Branches: bl 0 feet (PLANESetFeetWet then compare 0x8000000 old/new; 3-sets
   0x4ff658 / 0x4ff670, +scaled 10; else +scaled(5+Rand(5))). Surface target
   needs bit 1: prev NM > 10 and NM <= 10 -> 0x4ff688 APPTRGT; NM changed and
   >= 1 -> MSGSend type 0x1c payload {rounded NM, clock}. Aircraft: bit 4 -> none;
   0xb head-on; 9 offensive; 0xa defensive; 8 neutral, as in the spec table.
   Pair arrays: 0x4ff690 (self 2), 0x4ff6a0 (wingman 2), 0x4ff6b0 (2), 0x4ff6c0
   (6), 0x4ff6f0 SWCMISS, 0x4ff6f8 DONTOVR, 0x4ff700 (2, wingman), 0x4ff710 (6),
   0x4ff740 (4), 0x4ff760 (7 self), 0x4ff798 (10 wingman), 0x4ff7e8 SLOWDWN,
   0x4ff7f0 (5), 0x4ff818 (4), 0x4ff838 VERTICL. Position calls MSGSend type 0x1d
   (head-on, offensive, defensive fallback) and 0x1e (neutral) with 6-byte
   payload {bp, clock, turn}.
   Thresholds: 0x271000 10,000 ft; 0x138800 5,000 ft; 0x4e2000 20,000 ft;
   0x4b000 1,200 ft; closure (speed +0x34 difference, fps fixed8) >= 0x9200;
   0x71c 10 deg; 0x71c0 160 deg; 0x1554 30 deg; |G| <= 0x300; corner speed
   `_COCornerSpeed` + 0x6e fps; target skill +0xe2 >= 3 for kinds 2/4; target
   pitch +0x1f in 0x2aa8..0x5550. "Switch to missiles" needs station
   `_HARDPtrs(+0x11c)` with type +0xa6 bit 1 clear (gun) and `_CTEval_ir` or
   `_CTEval_radar` nonzero.
9. Store prev NM and situation, submit via 0x48e950(curId, speaker target).
   Always store prevG (except the step 1 class exit), PopCurObj.

### SAY output and labels, 0x48d470

- Sender +2, recipient +4, type +0xa, broadcast word +8. Specials 0x8003..0x800b
  and 0 are system senders (no label).
- Sender nationality (+9 & 0x7f) 0x14 or 0x15 -> 0x490480 `#` rewrite.
- Label: sender == `_playerId` 0x520a1c: recipient == player and player type
  +0xba bit 0x4 -> `RIO` (0x4ffbdc) unless class byte 0x50d276 bit 0x40 ->
  `CO-PILOT` (0x4ffbd0); else `YOU` (0x4ffbcc). Other senders: 0x48d6e0 name
  (+0xa custom name, else wing name/number via 0x4ebff0, else type name).
  Format `LABEL: 'text'` via 0x4ffbc8/0x4ffbc4 to `_HUDMessage` 0x405f50.
- Speech: 0x48d610 splits stems on commas, `SoundOn` priority 0x32, volume
  word 0x5718ec * 255 / 100.
- MSGSend routing (0x4181b9..): recipient 0x8002 (and 0x8000) resolves to the
  sender's wing leader, or to the sender when not in a wing; 0x8001 branch not
  traced.

### Nationality ids 0x14 / 0x15

+9 & 0x7f indexes the name table 0x4fb2b8 (0 American, 1 British, 2 Chinese, 3
French, 4 German, 5 Belgian, 6 Jordanian, 7 Israeli, 8 Japanese, 9 North Korean,
10 Russian, 11 South Korean, 12 Syrian, 13 Arab Egyptian, 14 Islamic Egyptian,
15 Estonian, 16 Latvian, 17 Lithuanian, 18 Polish, 19 Belorussian, 20 North
Vietnamese, 21 South Vietnamese, 22 Ukrainian, 23 Iraqi, 24 Iranian, 25 Kuwaiti,
...). Users: 0x42956f, 0x427816, 0x4c1c20. `nationality2` (0x48274f) skips the
+1 increment of `nationality` but still applies the map remap 0x483d50; retail
`map tviet.T2` missions write enemy `nationality2 142` -> 0x80 | 20. So 0x14/0x15
are North and South Vietnamese, not object classes or crew seats.

### SAYTranslate 0x490f30 (direct sounds)

If curThing +9 & 0x7f is 0x14 or 0x15: `^EJECT.5K`, `^EJECTNG.5K`,
`^IMOUTTA.5K`, `^OUTTA.5K`, `^EJECTX3.5K` -> `#EJECT.5K` (0x4ffddc);
`^AARRRGH.5K`, `^AARRGH2.5K`, `^OHSH.5K`, `^YAAAAAH.5K` -> 0x4ffbb8[Rand(3)] =
`#AARRRGH.5K`, `#AARRGH2.5K`, `#YAAAAAH.5K`. Otherwise unchanged. Callers:
0x410cdf (eject countdown), 0x414d08 (player eject, list 0x4ee388), 0x4426df
(collision kill, `^AARRRGH`), 0x473cf2 (`_Kill` 0x473c10 for the player: Rand(4)
table 0x473d9c -> AARRRGH, AARRRGH, OHSH, YAAAAAH; `SingleSound` volume 0xff).

### # rewrite 0x490480 (radio messages)

Only when message word +8 == 0x8000. Replaces every `^` in 0x553050 with `#`,
then switches on type (table 0x490e44, types 5..0x24): 5 `#BREAK`, 6 `#APPR`,
0xb..0xd `#DISENG`/`#CLRMY6`/`#ATTACKB`/`#ATTACKV`/`#BUGOUT`, 0xe `#PROCTO`,
0x11 `#CONTACT`, 0x12 weapon calls (`#FOXONE`/`#FOXTWO`/`#FOXTHR` with an
`AIM-54` test, `#BOMBAW1`/`#BOMBAW2`), 0x13/0x14 hit/kill (`#BULLS1`, `#IMPACT`,
`#MULTHIT`, `#OHYEAH`, `#OHYES`, `#BEAUT1`), 0x15 `#ENGAGE`/`#ISEEEM`/
`#TALLYHO`, 0x17 hit (`#IMHIT1`, `#IMDMGE2`, `#IMAAA`, `#IMDMGE1`), 0x18 death
(`#AARRRGH`, `#YAAAAAH`, `#EJECT`), 0x1c `#G_POS`, 0x1d/0x1e `#P_POS`, 0x20
`#MISSFAL`, 0x22 `#WTCHOUT`, 0x24 comment rule below; other types keep the
`#`-prefixed buffer.
Comment rule 0x490d1c: tokenise buffer (0x490ed0). If any token equals an entry
of the NUL-terminated keep list 0x4ff9e8 (#OUTGAS #BINGO #JOKER #EASEUP #RADIOBP
#SWCMISS #DONTOVR #SLOWDOWN #AIRBORN #GDLUCK #GDHUNT #CLRLAND #WELBACK #CNTTONE
#VERTICL) keep it. Else scan pair table 0x4ffa28 {from, to}, terminated by
{0,0} at 0x4ffbb0: token == from -> buffer = to; token == to -> keep. Else
buffer = `^RADIOBP` (0x4ffc90). Note `#SLOWDOWN` in the keep list never matches
the `SLOWDWN` stem, so "Slow down" falls to the beep.
Pairs: CONTACT>CONTBAN, OUTFUEL>OUTGAS, WEFUMES>IMFUMES, APPTRGT>G_POS,
ATUS/ATYOU/CLOSING>OFFBEAM, GETGUY>CLOSING, YAHOO2/GOTNOW2/REELING/YAHOO3/
BURN1/FINISH/DOHIM>GOTNOW1, LOCKHIM>CNTTONE, ONTAIL>COMARND, BREAK1/BREAK2>BREAK,
BANDIT6/ONOUR6/PLTSHT1/PLTSHT2/USOUT/NOTGOOD>BANDIT6, DNTLIKE>GUYGOOD,
BURN2/LOSTHIM>BRGARND, RDYROLL/TAKOFF2/LAUNCH/RDYCAT>TAKOFF1, WELHOME>WELBACK,
BARF1..7>BARF, GRUNT1..4>GRUNT, BREATH1/BREATH2>BREATH1, BREATH3/BREATH4>BREATH2.
FA_2.LIB `#` recordings present: 85 names (AARRGH2..YAAAAAH), including
`#BARF`, `#GRUNT`, `#BREATH1`, `#BREATH2`, `#G_POS`, `#P_POS`, `#MISSFAL`.

### Other crew senders

- `@SAYLowFuelMessage@8` 0x48eb20(cl=plural, dx=recipient): first unlatched of
  0x40000 (OUTGAS / OUTFUEL, latch 0x780000), 0x20000 (WEFUMES / IMFUMES, 0x380000),
  0x8000 (BINGO, 0x180000), 0x10000 (JOKER, 0x100000). Callers: `_ServicePlayer`
  0x4166ab (cl=1, recipient curId, only with type flag 0x4); AI 0x49e07a (cl=0,
  recipient 0x8000, not human, airborne). Fuel flags set by `_PLANECheckFuel`
  0x49fb70 from `_FMUpdatePlaneFields` 0x452589 (see ai.md B48).
- `_PLANEEventProc` event 0x400 (missile launch) 0x49e0f4: hold gate +0x101
  (0x50cf81); human with type flag 0x4: missile type +0xb4 == 2 -> type 0x1a
  (ATOLFLR), 3 -> 0x19 (APEXCHF), else 0x1b (MISSBRK); MSGSend(curId, curId,
  delay 2, 0, 0x8000, type). PLANESayProc gates 0x19/0x1a with globals 0x4fef0c /
  0x4fef08 = now + 6 (raw).
- `@SAYSuppRadarMessage@12` 0x48ea10: callers 0x414b57/0x414b6c/0x414de7/
  0x414dfc/0x440cdc; no link -> 0x4ff5a8 (`^BEEP2`), link on/broken pairs
  0x4ff580..0x4ff5a4 use `&SQACK2`/`&SQACK1`. Sender = recipient = player. Sets
  `_gamePrefs` 0x100000 around the submit and restores it.
- Radio silence: Alt+S handler 0x4159dc toggles 0x100000; set = "Radio traffic
  OK" 0x4ee3a8, clear = "Radio silence" 0x4ee398. MSGSend 0x41810c: clear and
  flags without bit 2 -> types 0x11..0x15, 0x17, 0x1c..0x1e, 0x22, 0x24 get
  flag 1, which skips delivery to the local player (0x4184e5).
- PLANESayProc (0x48d780, other agent) type table 0x48e0bc; relevant cases:
  0x16 launch (0x4ff4c8 AAM / 0x4ff4c0 SAM), 0x17 hit (sets 0x4ff420 x5,
  0x4ff448 x4 for type +0xe bit 8, 0x4ff468 x2), 0x18 death (0x4ff490 x6 or x3),
  0x19..0x1b warnings, 0x1c range (`^INRANGE` when miles <= 2), 0x1d/0x1e
  position, 0x1f 0x4ff4f8 (not `_fortMission`), 0x20 0x4ff500 x5, 0x21 0x4ff528,
  0x22 0x4ff538 x8, 0x23 0x4ff530, 0x24 comment payload.
- Hit/kill sender 0x4c1a57: shooter -> 0x8002, type 0x13 (target alive) or
  0x14 (hp < 1). Friendly fire 0x4c1b2e: victim -> player shooter, type 0x22,
  within 0xce4000 (52,800 ft), gate 0x58f1d4.

### _APCommentProc 0x48f6a0 (airport; tower and LSO)

Picks the best human aircraft of the same side using this airport (+0x231),
per-airport retry word +0x127, flags +0x12e, last actor +0x129, state +0x12b,
miles +0x12c. Dispatch on actor state via byte map 0x4900c8 and table 0x4900a0:
states 1 and 6 clear for takeoff (0x4ff840/0x4ff850, needs `_APStripFree`), 7
ready on the cat, 8 stand by, 9 cat one or cat failure (actor +0x16f bit 0x4000),
0x12 airborne / rotate (carrier, climb < -10) / good luck or waypoint call
(> 10), 0x14 and 0x15 approach (clear to land, or clear the deck when hit
points 0x5224ca <= 3/4 of 0x5224cc on a carrier; wind with a 10% gust; call the
ball within 90 deg and 30 deg heading; distance countdown; gear; hook; LSO
corrections within 0x1388; "Steady" 20%), 0x16 landing grade 0x4ff960 +
8*0x50d0f3, 0x1b welcome back within 10,000 ft. Callsign prefix 0x4900f0.
