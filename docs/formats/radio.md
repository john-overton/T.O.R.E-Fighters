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

The reviewed table spans 0x4fef10..0x4ff9e0 in 1.02F and holds 329 pairs
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
| 0x4ff8f8..0x4ff990 | Landing signal officer corrections, landing grades, welcome back/home |
| 0x4ff998..0x4ff9e0 | AWACS report lines and a second weapon release group |

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

## Radio chatter source notes

Research mode, 2026-09-23. FA.EXE 1.02F, SHA-256
e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c, image base
0x400000. Static review of `.local/weapons-research/native/fa-disassembly.txt`,
`symbols.json` and direct llvm-objdump ranges. Nothing was executed. The
behaviour spec is [radio chatter](../spec/radio-chatter.md). Queue layout and routing
already recorded in `docs/formats/native-strip.md` (NE-00.1k/NE-00.1n) are not
repeated here; link to them.

### Build shift

1.0 disc build (SHA-256 c7d2c1cc...ba9b): every pair table address in this note
is the 1.02F address minus 0x4608 (`radio_shift` in
`crates/tore-formats/src/executable.rs`). Code addresses below were reviewed in
1.02F only; their 1.0 locations were not established.

### Say record (event record as seen by the say procedures)

Event record from `MSGSend` 0x4180a0 (existing layout, native-strip NE-00.1k):
+0 flags, +1 bound-100 roll, +2 sender, +4 recipient, +6 quarter-second
deadline, +8 mask, +0xa subtype, +0xb length, +0xd payload.

- Flag 1: suppress speech. Set by the caller, or by the poster for subtypes
  0x11..0x15, 0x17, 0x1c..0x1e, 0x22, 0x24 when preference dword 0x4eb6f8 lacks
  bit 0x100000 and the caller flags lack 2. Bit 0x100000 is the "Radio traffic
  OK" state: FlightKey 0x4159dc XORs it and prints 0x4ee3a8 "Radio traffic OK"
  or 0x4ee398 "Radio silence".
- Flag 2: exempt from the radio-silence gate (all 0x24 wrapper sends use it).
- Flag 4: forwarded/self echo (existing note).
- Delay argument (arg 3) is quarter seconds: 0, 2, 8 and 0x14 are used.
- Speech mask is 0x8000. `MessagesToPlayer` 0x414523 polls the player with
  mask 0x8000. `MSGReceive` 0x4185a0 voices a record through `SAYMsg` only when
  flag 1 is clear and the recipient is the player (sender not the player, or
  flag 4), or the recipient is 0x8003 + local controller.
- `MSGSend` voices a player-sent record once at send time (label YOU) when mode
  0x520a50 == 0x10, the player object is live kind 4 and flag 1 is clear.

Recipient codes expanded by `MSGSend` through `_WNGPart` 0x45e710 (returns 0 no
wing, 1 position 0, 2 member; 10 wings x 10 slots at 0x5469a0, counts at
0x546980), acting leader 0x45e630 (first slot with instance flag 1) and
`_WNGWingmen` 0x45e6e0 (slots 1..n-1):

| Code | Expansion |
| --- | --- |
| 0x8000 | No wing: sender. Else acting leader. |
| 0x8001 | Only for part 1 (leader): slots 1..n-1 except sender. |
| 0x8002 | No wing: sender. Else acting leader plus slots 1..n-1 except sender. |

### Say dispatch

`SAYMsg` 0x48d350 dispatches callback request 6 of the **sender** (plane:
`_PLANESayProc` 0x48d780; generic objects: `_OBJSayProc` 0x48e8d0, which only
handles subtype 0x24), or `_SAYDefaultSayProc` 0x48d3c0 for zero/special senders.
Both buffers: text 0x552ff0, stems 0x553050 (comma separated). Output 0x48d470:

- Sender nationality (object +9 & 0x7f) 20 or 21 first runs formatter 0x490480
  (mask 0x8000 records only). Nationality names table 0x4fb2b8 indexed by that
  byte: 20 North Vietnamese, 21 South Vietnamese.
- Label: sender == player: receiver == player and PLANE type flags +0xba bit 4
  (multi-crew) gives "RIO" (0x4ffbdc) unless current type byte 0x50d276 bit
  0x40 (class word bit 0x4000, non-fighter class) gives "CO-PILOT" (0x4ffbd0);
  otherwise "YOU" (0x4ffbcc). Other senders 0x48d6e0: instance name pointer +0xa;
  else kind 4 in a wing: "%s %s" (0x4ebff0) with colour table 0x5023a8 (Red,
  Blue, Green, Black, White, Orange, Purple, Yellow) and position table
  0x502424 (one..twelve); else NextString(type names, 1).
- Display line `label + ": '" + text + "'"` via 0x405f50.
- Stems played by 0x48d610 (existing note), gain word 0x5718ec * 255 / 100.
- Busy deadline 0x552fdc = clock seconds + scaled_delay(3).

### PLANESayProc subtypes (jump table 0x48e0bc, subtype - 5, 32 entries)

Selector 0x48e150(pair base ECX, record EDX, count): count <= 1 uses the base;
otherwise index = record +1 roll mod count (a fresh RNG draw when EDX is null).
Chance helper 0x48e140(record, n) = roll < n. 0x4561a0(n) = fresh draw of
bound 100 < n.

| Sub | Meaning | Pairs / rule | Producer(s) |
| --- | --- | --- | --- |
| 5 | Break | 0x4ff170 + 8*i by sign of heading/pitch words | FlightKey 0x4157cd, CTDo_wm_break |
| 6 | Approach | 0x4ff198 + 8*i | FlightKey, CTDo_wm_approach |
| 7 | Horizontal spacing | 0x4ff1c0 (<1000) / 0x4ff1c8 | GRPSetSpacingH, WNGFormationMove |
| 8 | Vertical spacing | 0x4ff1d0 + 8*i | GRPSetSpacingV |
| 9 | Formation | 0x4ff1e8 + 8*byte | GRPSetType |
| 10 | Control | 0x4ff1f8 + 8*byte (1 loose, 2 medium, 3 tight) | GRPSetControl |
| 11 | Target order | dword 0: 0x4ff218; bit 0x40000000: 0x4ff220 x2; 0x1fffffff: 0x4ff230; else 0x4ff230 + 0x4ff048 + noun 0x48e190 + 0x4ff050 + clock 0x48e350 | FlightKey, WNGSendWM, GRPSendWM |
| 12 | Class order | "Attack" + category noun | FlightKey 0x4158d0 |
| 13 | Evade variant | 0x4ff238 base | no producer located |
| 14 | Waypoint | 0x48e5f0; global cooldown word 0x55304c, 5 s scaled | WPSetupCurrent 0x499412 (0x8001, delay 2) |
| 15 | New leader | 0x4ff288 | WNGAdd 0x45e618, GRPRemove 0x45f348 (delay 0x14, flag 1 when self-sent) |
| 16 | Bug out | 0x4ff240 | no producer located |
| 17 | Contact | 0x48e740(target, 0, receiver, roll, 1, advise) ; advise = WNG part 2 and control byte 0x50cf7b >= 2, or 0x50cf5e bit 2 | GRPSetStateTarget 0x45fa3d, WNGFormationMove 0x45ed0d |
| 18 | Launch | see below | PROJAdd 0x4c109f |
| 19 | Hit | payload +0xf nonzero: 0x4ff290 x5; else cooldown 0x5530cc, 0x4ff2b8 x8, +4 s | PROJDamageProc 0x4c1a57 |
| 20 | Kill | see below | PROJDamageProc 0x4c1a57 |
| 21 | Engage | target non-plane: 0x4ff3d8 x1; else x9 | PLANEEventProc 0x49efb0 (0x8000, delay 8) |
| 22 | SAM/AAM launch | launcher kind 4: 0x4ff4c8; else 0x4ff4c0 | PLANEEventProc 0x49e244 (0x8002, delay 2) |
| 23 | I'm hit | attacker kind 4: 0x4ff420 x5; class byte +0xe bit 8 (AAA): 0x4ff448 x4; else 0x4ff468 x2 | PLANEEventProc 0x49e8ea (0x8002, delay 0) |
| 24 | Death | type flags 0x50d322 bit 8 clear and bit 0x10 set: 0x4ff490 x6; else x3 | PLANEEventProc 0x49e73e (0x8002, delay 0) |
| 25 | Radar missile inbound | 0x4ff478; cooldown 0x4fef0c +6 s | PLANEEventProc 0x49e181 (self, delay 2) |
| 26 | IR missile inbound | 0x4ff480; cooldown 0x4fef08 +6 s | same |
| 27 | Other missile inbound | 0x4ff488 | same |
| 28 | Range | 0x48e430 miles, clock unless +0xf == 12, 0x4ff4d0 if miles <= 2 | no producer located |
| 29 | Position, turning | 0x4ff038 + 8*i, clock, 0x4ff4d8 + 0x4ff4e0/0x4ff4e8 when angle word +0x13 in 0xe38..0x71c0 | no producer located |
| 30 | Position, heading away | 0x4ff038 + 8*i, clock, 0x4ff4f0 | PLANECommentProc 0x48f64c |
| 31 | Mission accomplished | sets 0x552fe0; 0x4ff4f8 unless byte 0x5528bc | PLANECommentProc 0x48ecff (0x8001, delay 8) |
| 32 | Mission failure | sets 0x552fc8; 0x4ff500 x5 unless 0x5528bc | PLANECommentProc 0x48ed42 |
| 33 | Almost home | sets 0x552fcc; 0x4ff528 | PLANECommentProc 0x48edaf |
| 34 | Friendly fire | 0x4ff538 x8 | PROJDamageProc 0x4c1b2e (victim to player, delay 8) |
| 35 | Protect me reply | 0x4ff530 | PLANEEventProc 0x49ee80 (0x8000, delay 8) |
| 36 | Literal | payload text + stems (two C strings) | 0x48e950 wrapper, MSGSendChatter |

Launch (18): payload +0xf names the weapon type (RMAccess 0x4a6ae0 mode 0x8000).
Order: `_stricmp(name, "AIM-54")` (0x4ffbe8) == 0 gives 0x4ff258 (Fox three).
Projectile flags +0xa6 bit 0x10, or bit 0x400 with +0x6d == 0: cooldown word
0x552fd8, 0x4ff260, +4 s. Else bit 1 clear (unguided): cooldown 0x552fd4,
0x4ff268 appended, +4 s. Then roll < 50 and target kind 4: seeker byte +0xb4 3
gives 0x4ff248, 2 gives 0x4ff250. Else 0x4ff270 x3. JT census: bit 1 set on all
guided missiles; 0x10 on MK82/FAB/CBU/RBK/MK20 families; 0x400 on GBU-10/28,
Paveway, AS-14, AS-30, AT-12.

PROJAdd sender gates: shooter owned by the local controller, shooter nonzero,
target nonzero or current type +0xa6 (0x50d30e) bit 0x10. Flags = 1 when the
last PROJAdd argument is zero. ServicePlayer 0x417027 passes 1 through PROJFire;
PROJServiceWeapon 0x4c4e34 passes 1 only on its state-4 path (0x4c4e08).

Kill (20): payload +0x11 victim, +0xd projectile ID. Victim kind 4: PLANE flag
bit 8 (rotorcraft, V-22, blimp) or 0x4561a0(40) gives 0x4ff300 x12. Else buffer
"^AC" + RMChangeType(NextString(names, 2), "") (0x4a6870); base name "F22"
(0x4ffbe0) gives the x12 set; stem cleared when longer than 8; then pair
0x4ff3d0 plus text NextString(names, 1) and the buffer stem. Victim not kind 4:
cooldown word 0x552fe4 checked, 0x4ff380 x10, cooldown +4 s set only when the
projectile's type flags have bit 0x10.

Hit/kill producer 0x4c19a8..0x4c1a6e: owner +0xe2 nonzero and not the victim,
hit record +0x21 zero (meaning unknown); kind-4 shooter, victim alive and
unguided store: per-shooter word +0x270 cooldown, +8 s. Subtype 0x14 when
victim word 0x50ce8e < 1 else 0x13. Flags 1 when shooter and victim share the
nationality high bit. Friendly fire 0x4c1a75..0x4c1b3d: victim alive, owner is
the player, victim kind 4, global word 0x58f1d4 +6 s, same side, distance
<= 0xce4000.

I'm hit producer 0x49e86a..0x49e8ea: owner on the other side or none; projectile
flags bit 0x80 applies per-aircraft word 0x50d0ee, +8 s.

Missile warning producer 0x49e0f4..0x49e181: hold word 0x50cf81; human
(0x50ce90 bit 0x80) and PLANE flags bit 4 send 0x19/0x1a/0x1b by missile seeker
byte +0xb4 (3, 2, other) to itself. SAM/AAM producer 0x49e199..0x49e244: gate
0x49f7e0, kind-9 store 0x452f80, opposite nationality high bit.

Contact producer 0x45f998..0x45faa8: new target differs from word 0x50cfa2,
clock >= word 0x50cfa4, `_GRPPart` 0x45f440 position <= 1, state byte 0x1f
(new or current 0x50cf63), target +0xe3 not in 1..0x11 or 0x16..0x1e, not human
(0x50ce90 bit 0x80); sets 0x50cfa2 and 0x50cfa4 = clock + scaled 15. Engage
reply path 0x49ef05 sets 0x50cfa4 = clock + scaled 20 and 0x50cfa2 = target.

### Composition helpers

- 0x48e190 noun(target, class word, count, named): named text = type names
  first string (+ "s" 0x4f0a20 when count > 1) when `named` and name nonempty;
  `_strnicmp` 6 against "mig-17"/"mig-19"/"mig-21" gives stems ^MIG17(S),
  ^MIG19(S), ^MIG21(S). Category from class word +0xd: 0xc000 bandit, 0x2000 ship,
  0x1000 SAM, 0x800 AAA, 0x400 tank, 0x200 vehicle, 0x100 structure, 0x80
  missile, else target; base 0x4ff060 singular, 0x4ff0a8 plural.
- 0x48e350 clock(your, clock, elev, flag): optional 0x4ff0f0; stem
  "^CLCK%02dD" (0x4ffc50) when flag and elev 0, else "^CLOCK%02d" (0x4ffc44);
  text number word + " o'clock" (0x4ffc38); elev +1/-1 appends 0x4ff048 then
  0x4ff030 "high" / 0x4ff020 "low".
- 0x48e430 miles(n): n < 1 becomes 1; n <= 10, 20, 30: text "%d" + " mile(s)",
  stem "^MILE%02d" (0x4ffc5c); else digits via 0x48e4e0 then 0x4ff0f8/0x4ff100.
- 0x48e4e0 number(n, falling): text "%d"; n <= 12 stem numberSay[n] (0x4fef10
  + 8n + 4) plus "D" (0x4ffc68) when falling; else decimal digits from the
  ten-thousands down, leading zeros skipped, "D" on the last digit when falling.
- 0x48e5f0 waypoint: 0x4ff108/0x4ff110 selected by roll (first letter case set
  by argument), 0x4ff118 (empty text, ^WAYPNT), waypointSay 0x4fef70 + 8*index,
  0x4ff120, bearing degrees = angle / 0xb6 with falling, 0x4ff128 + 8*i
  (i from rounded thousands difference, 0x3e800 units of 1/256 ft), 0x4ff140,
  altitude / 0x3e800 with falling.
- 0x48e740 contact: miles via 0x411de0 (rounded, 0x17bc00 = 1 nm in 1/256 ft);
  group size `0x45e790` = target wing members alive within 0x271000 and heading
  within 0x1ffe, plus one; size/noun only when arg 5 and miles <= 15; naming
  when distance <= 0xa50000 * 0x4b4720(...) / 100; 0x4ff148, 0x4ff150/0x4ff158/
  numberSay, 0x4ff168, 0x4ff050, clock with "your", 0x4ff050 + miles, 0x4ff160.

Unlisted pair slots (empty or no stem) used by the helpers: 0x4ff028 "" (level),
0x4ff048 " ", 0x4ff050 ", ", 0x4ff058 is the sample-handle word, 0x4ff118 ""
with ^WAYPNT, 0x4ff4d8 ", turning " (no stem), 0x4ff578 " Plane Re-Armed and
Re-Fueled" (no stem), 0x4ff9a8 "Bandit, visual range" (no stem),
0x4feff8..0x4ff008 Orange/Purple/Yellow (no stem).

### Other SAY entry points

- `@SAYLowFuelMessage@8` 0x48eb20(crew flag CL, recipient DX): levels in
  current instance flags +0x16f (0x50cfef): 0x40000 out, 0x20000 fumes, 0x8000
  bingo, 0x10000 joker; announced bits 0x400000/0x200000/0x80000/0x100000;
  announcing marks 0x780000/0x380000/0x180000/0x100000. Pairs 0x4ff5b0/0x4ff5b8,
  0x4ff5c0/0x4ff5c8, 0x4ff5d0, 0x4ff5d8. Callers: ServicePlayer 0x4166ab (crew
  flag 1, to self, only PLANE flag bit 4), PLANEEventProc 0x49e07a (crew 0, to
  0x8000, AI only, skipped when 0x411910 is true). Levels from `_PLANECheckFuel`
  0x49fb70, ORed in at 0x45259a: total fuel <= 0 out; no home base (0x4bed70)
  none; endurance at best throttle (0..110 step 10) <= 0 out, < 240 s fumes,
  < home time + 300 s bingo, < home time + 600 s joker.
- `@SAYSuppRadarMessage@12` 0x48ea10: pairs 0x4ff580..0x4ff5a8 (stems &SQACK2,
  &SQACK1, ^BEEP2), forces bit 0x100000 on during the send, player to player.
  Callers FlightKey 0x414b57/0x414b6c/0x414de7/0x414dfc, CPComputeRCS 0x440cdc.
- `@SAYRearmMessage@8` 0x48e920: 0x4ff578 text only. ServicePlayer 0x4175f0,
  0x417616.
- 0x48e950 wrapper: subtype 0x24, flags 2, mask 0x8000, delay 0; sets 0x552fdc.
- `_SAYAwacsReport@0` 0x4901c0: no call or absolute pointer found in the image
  (byte scan for E8/E9 rel32 and dword). Selects the nearest live kind 2/4
  same-side object with type flags +0xa6 bit 1 whose radar (hardpoint kind 3,
  range dword +0x17) reaches the player; none prints 0x4ffc6c. Then nearest
  opposite-side kind-4 object passing detection 0x4c2860; none: 0x4ff998 /
  0x4ff9a0 by fresh 50%; count of enemies within 0x271000 of it; nearest
  < 0x3a9800 gives 0x4ff9a8 (count 1) / 0x4ff9b0; else 0x48e740 with arg 5 zero.
  Sent from the AWACS to the player through 0x48e950.
- `@SAYTranslate@4` 0x490f30: for current nationality 20/21 rewrites direct
  sample names (ejection, damage, CATGUY, Kill callers) to # names.
- `_SAYFortStatus` 0x4911f0 / `_SAYFortAircraft` 0x491130: text screens
  ("Shift-F%d:    %d   %s", "Airbase Aircraft Inventory", "%s is %d%% destroyed.").
- `_MSGSendChatter@24` 0x418880: multiplayer text, subtype 0x24, flags 2.

### Prefixes

- `^`: ordinary radio voice (618 entries in local FA_2.LIB).
- `#`: Vietnamese-nationality voice set (85 entries). Formatter 0x490480, jump
  table 0x490e44, first replaces every ^ with #, then for subtypes 5, 6, 11..14,
  17..21, 23, 24, 28..30, 32, 34, 36 writes replacement stems from 0x4ffc90..
  0x4ffdd0 (for example #BREAK, #APPR, #DISENG, #CLRMY6, #ATTACKB/#ATTACKV,
  #BUGOUT, #PROCTO, #FOXONE/#FOXTWO/#FOXTHR, #BOMBAW1/#BOMBAW2, #BULLS1,
  #IMPACT, #MULTHIT, #OHYEAH, #OHYES, #BEAUT1, #ENGAGE, #ISEEEM, #TALLYHO,
  #IMHIT1, #IMDMGE1/#IMDMGE2, #IMAAA, #AARRRGH, #YAAAAAH, #EJECT, #AARRGH2,
  #P_POS, #G_POS, #CONTACT, #MISSFAL, #WTCHOUT, ^RADIOBP). Per-branch choice
  not fully read. Also 0x4ff9e8.. holds # stem pointers for literal messages.
- `&`: sound effect played through the same stem list (&SQACK1, &SQACK2).
- Composed stems outside the pair tables: ^NUM00D..^NUM12D, ^CLOCK01..12,
  ^CLCK01D..12D, ^MILE01..10/20/30, ^MIG17(S)/^MIG19(S)/^MIG21(S), ^AC<name>
  (78 present), ^WAYPNT. All listed composed forms are present.

### Archive census (local FA_2.LIB, listed with tore-extract --list)

All 330 pair stems in 0x4fef10..0x4ff9e0 plus ^WAYPNT are present as .5K
entries except `^FIRGUN` (0x4ff268, "I'm using my gun"). Unreferenced by any
say routine in this build: 0x4ff2f8 ^OBJDEST, 0x4ff360..0x4ff378 ^SPLMIG1/2,
0x4ff9b8..0x4ff9e0 second Fox/bombs group (no direct reference found).
Comment-system consumers (other pass): 0x4ff5e0 at 0x48f053, 0x4ff658 at
0x48f193, 0x4ff688 at 0x48f23e, 0x4ff690 at 0x48f35e, 0x4ff840 at 0x48f985.

### Corrections to existing docs

- ai.md B46 says the target reply is "Engaging". Only non-aircraft targets get
  that fixed line; aircraft targets and attack-on-contact pick 1 of 9.
- ai.md B47 "A launch by an aircraft on the same side sends a radio message but
  no maneuver": the SAM/AAM radio send at 0x49e1d9..0x49e244 requires the
  opposite nationality high bit. Recheck that sentence.
- ejection.md "object classes 0x14/0x15": these are nationality codes 20/21,
  North and South Vietnamese (table 0x4fb2b8 indexed by object +9 & 0x7f).

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
