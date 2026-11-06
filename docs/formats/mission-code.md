# Mission code modules (MC)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Research notes, research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Requirements, gates and remaining
> work described here are research-mode scope; they are not acceptance gates
> for gameplay. Parity is measured by expression of feature, see
> [AGENTS.md](../../AGENTS.md). Player-visible behaviour is specified in
> [docs/spec/](../spec/).

Research mode, 2026-11-06. Static reading only: no module was loaded or run.
The player-visible result is specified in
[mission scripts](../spec/mission-code.md); this page holds the evidence.

Build identity:

| File | SHA-256 |
| --- | --- |
| `FA.EXE` 1.02F | `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c` |
| `FA.SMS` (the game's own symbol map, 3,829 names) | `e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0` |
| `FA_2.LIB` (installed) | `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198` |

The 21 `.MC` entries inside disc 1 `SETUP.ESA:FA_2.LIB` differ from the
installed ones only in the four link-timestamp bytes at file offset 136, as
[the build baseline](../baselines/fa-builds-2026-10-06.md) already recorded.
The CODE section hashes below are therefore stable across both copies.

## Summary

- An MC module is a small mission script compiled to i386 code. Each one is a
  single event handler that the game calls once per object-service pass and
  whenever it asks "has this mission succeeded?".
- The 21 shipped modules do only a handful of things: timed or conditional radio
  messages, starting hit points, a protected ship, scripted ship losses, removing
  a rescued pilot, a forced catapult failure, landing-based success, and one
  timed target-designation sequence.
- 18 modules are referenced by 31 mission files; 3 (`FOO`, `TRAIN01`, `UKR01`)
  are referenced by none and have no player effect.
- Every observed behaviour can be stated as data: a trigger (mission time, first
  damage, success first reported, route progress, first frame) and an action
  (radio message, hit point change, kill, remove, target flag, success rule).
  Only `K16` needs a richer phased-objective rule.
- Modules are installed only when the local computer index is zero, so they do
  not run in a networked game.

## Inventory

All modules are 4,608 bytes except `K16` (8,704). "CODE" is the CODE section
length the game reads. "Missions" lists every `.M` file in `FA_2.LIB` whose
`code` line names the module; loose `.M` files in the install folder have no
`code` line.

| Module | CODE bytes | CODE SHA-256 | Missions |
| --- | --- | --- | --- |
| `CATFAIL` | 302 | `d19d7b1e65468bb9c3b33e45cebdcffbe10f4a28a4b821e21b1be5e04c5cc118` | `UKR14` |
| `EXTRA01` | 146 | `95343a165444f7a78c67e45d8920e20e8aaf8b9dc0e2b600a033a2a49b96570a` | `EXTRA01` |
| `FOO` | 216 | `c39bd7dac060c8ff4af29485addbf339830361b0faa1e5c9d21ff8ee51ae6794` | none |
| `K16` | 3,966 | `f069996c9c816a274181f3cba1d48524b91faa67d4859c90501d61cadf02942f` | `~K16` |
| `K17` | 490 | `3e7208ed263c37117e6bf6245a1479ee915fce5dc85d65a103bc4f98fb17f571` | `~K17` |
| `TRAIN01` | 608 | `446e458a1bb79d824e9c591a80c37d2caabeec74ec45e9166497df31f0c3800a` | none (`TRAIN01.M` has `;code train01`, commented out) |
| `U01` | 558 | `34db343474e062c736a8d5e0cb5123318424f7b5144b8e98a98c379ffaa77009` | `~U01` |
| `U07` | 398 | `11822147b51346e6846bdaa2c942641fb260ed57dc44ef2e396b28ea1fa54b9d` | `~U07` |
| `U08` | 146 | `7026c704222be6c24445d1cef3bdbd760603e12e54746be3d2f2a0672b8a3ae0` | `~U08` |
| `U11` | 1,102 | `1e7daff14d4049ef2c4681901d2f0c021f496bbb7c2b6130ce6b149069bca895` | `~U11` |
| `U12` | 248 | `511bc8121bf613f505850cb84c9a3a13c096deb3d231a3e1f5b4ae24f5b834ae` | `~U12` |
| `U15` | 582 | `351f430520734aae0df376a4d2c29ce5c251a9ea0183b262f3ae8d0cc7047b69` | `~U15`, `UKR18` |
| `U22` | 296 | `9f840bd31b2d4656943ddcf2b958a21f3664fb22e6fc0b68d9b5dba964071917` | `~U22`, `UKR30` |
| `U23` | 488 | `0a3f0c2bfbd137a8b986de65d7df444df6653dbbd7c4bb376266f94cf4775b11` | `~U23`, `UKR16` |
| `U24` | 232 | `0b2f3c071468978e62ae8f17bb70ce34d309c5f2fe9873a7955ca98fd9a545ce` | `~U24`, `UKR33` |
| `U25` | 456 | `7fb2437861ed974bae204d4f82ddd6757e491998a6854e1ebd0d026306f53371` | `~U25`, `UKR24` |
| `U29` | 456 | `23ca77629ad2a21382e2a107957cd3737f62e06ba1810f375592107dcb2fde76` | `~U29`, `UKR19` |
| `U34` | 568 | `a9e7f2dc29db81a0bdea64af8eaa5b1df500629bfaf65c503443a09305b8f284` | `~U34`, `UKR47` |
| `UKR01` | 468 | `d67c52ad704624f0b0e578196801b7d85eed5619f24602b851282f3f7ab04c89` | none (`UKR01.M` names `ukr02`) |
| `UKR02` | 608 | `ebd72f9be88255c02c634d93afcd55ae8cb5c503d19f5346ba889a16cf9b8d30` | `UKR01` to `UKR07` |
| `VIET03` | 608 | `0e27ddfa60ac856c828674412b9b99878481c591eb30ed0980b73cfc2c701344` | `VIET03` |

Which campaign uses which mission: `UKRAINE.CAM` lists `~U01.M` to `~U50.M`,
`KURILE.CAM` lists `~K01.M` to `~K35.M`. The `UKR*` files are the USNF '97
training and single missions (their `.MT` section 1 says "Training Mission" or
the mission is a single-mission copy of a `~U` mission with the same aliases),
`EXTRA01` is an ATF Gold single mission and `VIET03` a USNF '97 single mission.
So the campaigns use 14 modules (12 Ukraine, 2 Kurile) and the single and
training missions use 9, 7 of them shared with the Ukraine campaign.

`UKR02`, `VIET03` and `TRAIN01` are one program with different aliases;
`EXTRA01` and `U08` are one program with different aliases. The CAM files are
themselves PL modules (exports `_UkraineQuit`, `_UkraineMedals` and others);
they are outside this page.

## Container and linking

- Same PL container as DLG, MNU, HUD and LAY: `MZ` stub, `PL\0\0` signature,
  i386, sections `CODE`, `.idata`, `.reloc`, `$$DOSX`; no data section (state
  bytes and inline strings live in CODE). Image base 0, entry point 0, so the
  handler is the first byte of CODE. [`module.rs`](../../crates/tore-formats/src/module.rs)
  already reads this CODE section as inert data.
- Imports name the library `main.dll`. `FA.EXE`'s own export table holds only
  `T_HorizonProc` and `WRFogLayerUpdate`, so the MC names are not resolved
  through it. The game's loader (`_LoadDLL@16` 0x41eb60, fixups 0x41ef10)
  rewrites every relocated reference to an import thunk into the address
  returned by `_SMAddress@4` (0x46a4e0) for that name, which looks the name up
  in `FA.SMS`. A missing name ends the game through `_ErrorExit`. Data imports
  (`_currentTime`, `_co`) therefore read engine globals directly.
- Calls are made with a push-return-address-then-`ret` idiom; strings sit inline
  behind short jumps. Disassembly has to split those islands out (see
  [reproduction](#reproduction)).

## Host interface

### Installation

`MISSIONTextProc` 0x481f30 reads the `code` keyword into `missionDLLName`
0x4fb238 and changes its type to `.MC`. `_MISSIONInit2` at 0x480aa0 stores
`RMAccess(name, 0x8000)` into `_eventFilterProc` 0x4f6fb8 only when the name is
non-empty and `_thisComputer` 0x4eb608 is zero. `_InitChain` 0x462600 clears it.
This refines [native-strip](native-strip.md#mission-interceptor-selection),
which named the same gate.

### Calling convention

`handler(u16 *event, void *argument, u8 *result)`, cdecl (the caller pops
12 bytes). The handler reads the event code from the first word and returns
its answer in AL.

| Event | Caller | When | Argument / result |
| --- | --- | --- | --- |
| 0 | `_ServiceObjects` 0x462b35 | Once at the end of every object-service pass, after all objects moved | pointer to a zero dword / result byte ignored |
| 0x20 | `_CallMissionProc@8` 0x481940 with selector 1 | Success query, see below | both null; AL is the success value (signed) |
| object event bits 0x80, 0x100, 0x400, 0x800, 0x1000, 0x2000, 0x4000, 0x8000 | `_CallEventProc@8` 0x4639c0, usually through `_MaybeCallEventProc@8` 0x463980 | During one object's service, when that object's flag byte +1 bit 0 is set and its event mask word +0x58 has the bit (0x8000 message received, 0x4000 hit payload, 0x80 command buffer empty; others not identified) | AL non-zero means "handled" and the byte at `result` replaces the default; zero falls through to `_CallUtilProc(3, ...)` |

No shipped module handles object events: every one returns 0 for codes other
than 0 and 0x20 (`FOO` instead runs its tick body for them; it is unreferenced).

The success query (event 0x20) replaces `_MISSIONSuccess@0` 0x481a70 when a
module is installed; without one the default runs. It is asked:

- every 4 seconds of mission time by `_MISSIONCheckSuccess@0` 0x486860 from
  `FlyingLoop` (only when `_thisComputer` is zero), which stores
  `_missionSucceeded`;
- once more at mission end in `usnfmain` (0x4048ee, 0x40491c), where success is
  `value > 0`;
- by `_UkraineQuit` 0x481201 (quit confirmation), `_ChooseScoreInit` 0x441c71 and
  the map screen 0x428453.

A module may return a negative value; that and zero both mean "not succeeded".
The query is the only success hook; there is no separate failure event.

### Engine services imported

| Import | FA address | Use in the modules |
| --- | --- | --- |
| `@OBJAlias@8` | 0x4914c0 | Mission alias (negative `alias` in the `.M` object) to object id; the second argument is always 0 |
| `@OBJGet@4` | 0x491240 | Object id to instance pointer |
| `@Alive@4` | 0x4747c0 | Instance is alive |
| `_PushCurObj@4`, `_PopCurObj@0` | 0x4629e0, 0x462a20 | Make an object current; its record is then visible as `_co`/`_cp`/`_cn` (all 0x50ce80) |
| `_RemoveCurObj@0` | 0x4627b0 | Remove the current object from the world |
| `_Kill@0` | 0x473c10 | Destroy the current object |
| `_GRAPHICAddExp@28` | 0x4432d0 | Explosion effect; the modules pass type 0x22 (34, `watlrg`, large water blast, see [explosions](explosions.md)) at the current object |
| `_MSGSendChatter@24` | 0x418880 | Literal radio message, see below |
| `_MISSIONSuccess@0` | 0x481a70 | The default success rule |
| `_Dist@8` | 0x4c66cc | Octagonal distance: largest axis plus a quarter of each other axis |
| `_OnTheGround@0` | 0x411910 | Current object within 1 ft (0x100 units) of the height `_T_Info` returns |
| `_currentTime` | 0x5528e0 | Mission clock in whole seconds (`_TIMEUpdate` 0x486bd0 stores `currentTicks >> 8`; time of day is start plus this, wrapping at 86,400) |
| `_curId`, `_playerId`, `_thisComputer` | 0x4f6fbc, 0x520a1c, 0x4eb608 | Current object id, player object id, local computer index |

`_MSGSendChatter@24(0x8003, sender, 1, 1, text, samples)` builds subtype 0x24
("Literal") through `_MSGSend@32` with flag 2 (not muted by radio silence) and
recipient 0x8003, which `MSGReceive` voices for the local player (see
[radio notes](radio.md#say-record-event-record-as-seen-by-the-say-procedures)).
Every shipped call passes the player's own alias as sender; the speaker name is
part of the text ("E-2C:", "AWACS:", "J-STARS:"). `samples` is a comma list of
voice stems played in order (`^RADIOBP.5K` is the radio beep) or null for a
text-only line. All 11 referenced stems exist in `FA_2.LIB`.

### Object record fields touched

Offsets are into the object record (`_co` copy or `OBJGet` pointer).

| Offset | Meaning | Evidence |
| --- | --- | --- |
| +0x1 dword | The `.M` object `flags`; bit 0x80 marks a mission destroy target | [native-strip](native-strip.md) mission reader stores `flags` at +1; [quick mission](quick-mission.md) 0x80 targets |
| +0xe word | Hit points | Removal at zero in native-strip hit dispatch |
| +0x11, +0x15, +0x19 | Position x, y, z in 1/256 ft | [AI notes](ai.md) |
| +0xe3 byte | Activity state: 9 catapult stroke, 0x22 to 0x26 ejection (`EJECTAdd` sets 0x22, `EJECTMoveProc` 0x23, 0x25, 0x26) | `PLANEMoveProc` 0x49f86d; ejection writers |
| +0xe4, +0xe8 | Route start and current waypoint pointers; waypoints are 0x44 (68) bytes | `_WPChange` writes +0xe8 |
| +0x16f dword bit 0x4000 | Catapult failure | `PLANEMoveProc` 0x49f8c9: with the bit set, off the stroke and on the ground, speed +0x34 is forced to 0x4900 (73 ft/s) |

## Per-module behaviour

Aliases are the `.M` `alias` numbers. Times are mission-clock seconds and fire
on the first service pass at or after the value. Each message and action fires
once (private one-shot flags). Confidence is high unless stated.

### `U01` (Eviction Notice, `~U01`)

- First pass: remembers the hit points of alias -2, the IL-96 airliner named
  Yeltsin (nationality 12).
- The first pass on which its hit points are below that value: emergency call
  from the airliner ("Aeroflot flight 261", voice stem `^aeroflt`).
- Success query: passes the default rule through; the first time it reports
  success, schedules a thank-you call ("IL-96: thanks for the help", beep) for
  30 s later.

### `U07` (Nightprowler, `~U07`)

- Alias -33, the Kirov cruiser: hit points latched on the first pass and
  restored to that value on every later pass in which they are lower. Non-lethal
  damage never sticks. Whether a single hit that empties its hit points still
  sinks it is not established (removal happens inside hit dispatch, before the
  module's pass).
- 180 s: E-2C call, bogeys high inbound from the northeast (stems `bogeyhi`,
  `fromthe`, `ntheast`).
- Briefing: destroy aircraft near the cruiser; the next mission attacks it.

### `U08` (Fog of War, `~U08`) and `EXTRA01` (Carrier Strike, `EXTRA01`)

- First pass: halves (integer shift, rounding down) the hit points of alias -34
  (`U08`, the Kirov) or alias -1 (`EXTRA01`, the carrier Clemenceau).
- Both briefings say the ship was damaged in an earlier strike.

### `U11` (Mop Up, `~U11`)

Scripted submarine kills of three ships that are not targets (`flags` 0x417,
no 0x80):

| Time | Alias | Ship | Message |
| --- | --- | --- | --- |
| 180 s | -41 | Krivak | Krivak destroyed by friendly sub |
| 300 s | -46 | Sovremennyy | Sovremennyy destroyed by friendly sub |
| 360 s | -45 | Krivak | Krivak destroyed by friendly sub |

Each kill destroys the ship, adds a large water explosion at it and sends the
message with a radio beep. Quirk: the checks run in order and the code returns
early when the scheduled ship is already dead, so if the player sinks the first
Krivak before 180 s, neither later kill happens; if the Sovremennyy is dead by
300 s, the 360 s kill does not happen.

### `U12` (Top Cover), `U22` (BARCAP), `U24` (MiG Hunt), `U29` (Eyes in the Sky)

Timed calls only, success unchanged:

| Module | Time | Speaker and content | Sound |
| --- | --- | --- | --- |
| `U12` | 120 s | AWACS: bogeys to the northwest, obtain visual ID | beep |
| `U22` | 120 s | E-2C: contact, bogeys low inbound from the north | `contct2`, `bogeylo`, `fromthe`, `north` |
| `U24` | 420 s | Knight Hawks (the strike): commencing bombing run | beep |
| `U29` | 120 s | AWACS: contact, bogeys high inbound from the southeast | `contct2`, `bogeyhi`, `fromthe`, `stheast` |
| `U29` | 240 s | AWACS: Su-34s protected by MiG-29s, engage | beep |

### `U23` (Down and Dirty, `~U23`, `UKR16`)

- 180 s: FAC, the FEBA is 10 miles west of waypoint Alpha (beep).
- 240 s: FAC, attack the advancing tanks and APCs (beep), followed at once by a
  text-only second line saying they are crossing the highway.

### `U15` (HAVCAP, `~U15`, `UKR18`)

Only while alias -7 (the E-3 AWACS) is alive:

- 120 s: AWACS call, bogeys high inbound out of the northeast (voiced).
- When the E-3's current waypoint becomes index 2 (the third route entry, the
  last leg in `~U15`): "Go on home, we'll take it from here" (stem `gohome`),
  labelled F-14, the relief flight.
- `UKR18.M` contains no route blocks, so the E-3 has no route there and the
  second call is not expected to play (medium confidence: depends on the
  pointers of a route-less object being equal).

### `U25` (Convoy Escort, `~U25`, `UKR24`)

- 180 s: USAF F-15, bingo fuel, take over, going home (beep).
- Success query: as `U01`; 30 s after success is first reported, convoy call
  thanking the player for the top cover.

### `U34` (Stealthy Ivan, `~U34`, `UKR47`)

- First pass: remembers the hit points of aliases -3 and -4, two Sacramento
  supply ships.
- The first pass on which either is below its starting value: Sacramento call,
  under fire, please assist (stem `plsasst`). Once only, for both ships.

### `K16` (Rough Tread, `~K16`)

A timed target-designation sequence over eight T-72 tanks (nationality 138,
`flags` 0x493, all initially targets). Alpha tanks sit near (234,000, 252,000)
and Bravo tanks near (400,000, 306,000) in `.M` feet, about 29 nm apart.

| Time | Action | Calls (J-STARS) |
| --- | --- | --- |
| first pass | Clear the target bit (0x80) on all eight | none |
| 10 s | none | Scramble to Alpha and await instructions (beep) |
| 150 s | Pair -8, -10 at Alpha become targets | Destroy designated tanks at Alpha (beep); text: when finished, egress to Bravo |
| 500 s | Alpha pair stops being targets; pair -40, -41 at Bravo become targets | Destroy at Bravo; text: then await instructions at Alpha |
| 850 s | Bravo pair stops; pair -9, -73 at Alpha | Destroy at Alpha; text: then await instructions at Bravo |
| 1200 s | Alpha pair stops; pair -74, -39 at Bravo | Destroy at Bravo; text: then head home |
| 1400 s | Last pair stops; success if nothing failed | none |

Failure, latched permanently:

- a designated pair still has a live tank when its window closes;
- a tank is dead when its pair's window opens, or is found dead by any success
  query before its window opens (each query checks every pair whose window has
  not opened yet).

The module never calls the default rule: success is exactly "every pair killed
inside its window and none early", reported from 1400 s. Other objects do not
matter, although the briefing says wasted ordnance on other targets fails the
mission. The briefing also states the time-window rule, which the code confirms.

### `K17` (RESCAP, `~K17`)

- Watches alias -56, the SH-60 rescue helicopter, whose route is: start,
  Approach, Find Pilot, Rescue Pilot (300 ft), Rescue Pilot (300 ft over the
  downed pilot, 34 ft/s), Return To Group, end.
- Current waypoint index 3: SH-60, picking up the pilot (beep).
- Current waypoint index 4: SH-60, the pilot is on board (beep), and alias -57
  (the `PLTDWN.NT` downed pilot object) is removed from the world.
- Success unchanged.

### `CATFAIL` (Unwelcome Surprise, `UKR14`)

- Alias -7 is the player's F-14 on the Eisenhower deck.
- Whenever the player is in catapult state 9, the module sets the catapult
  failure bit 0x4000. The game then holds the aircraft at 73 ft/s while it is on
  the deck after the stroke; [AI notes](ai.md#airfield-takeoff-and-landing-sequences)
  record the same bit as a 1 percent roll on any human catapult launch, with a
  grind sound. The `UKR14` debrief text ("catapults do not often fail") confirms
  the reading as a failed catapult shot.
- Success replaces the default: true once the failure has happened and the
  player object's state is 0x22 or 0x26 (ejection). The failure debrief names
  SHIFT-E. Anything else fails.

### `UKR02` (training `UKR01` to `UKR07`) and `VIET03` (Running on Empty)

Landing success. Aliases: player -7 and carrier Eisenhower -1 (`UKR02`), player
A-7 -2 and carrier Kitty Hawk -1 (`VIET03`).

- Airborne latch: the first pass on which the player is not on the ground.
- Touchdown latch: afterwards, the first pass on which the player is alive and
  on the ground.
- Success replaces the default: true from the first query after touchdown,
  permanently. The code compares the player's position with the carrier's
  against 0xbb800 (3,000 ft), but passes the player's position as both
  arguments to `_Dist@8`, so the distance is always zero. As shipped, any live
  touchdown counts, including a touch-and-go.
- `UKR01` starts on the deck; `UKR02` to `UKR07` and `VIET03` start airborne.

### Unreferenced: `FOO`, `TRAIN01`, `UKR01`

- `FOO`: a test; at 10 s sends a message saying the time is now at least 10
  seconds. It labels the clock unit, which agrees with `_TIMEUpdate`.
- `TRAIN01`: the landing program with aliases -4 (player F-29) and -2 (an
  airstrip); `TRAIN01.M` comments its `code` line out.
- `UKR01`: an earlier landing program that latches the player's start position
  and requires landing within 3,000 ft of it; it passes the record base instead
  of the position field to `_Dist@8`. No mission names it.

## Classification

| Class | Modules | Count |
| --- | --- | --- |
| Generic triggers and actions (time, first damage, success first reported, route index, first pass; message, hit point scale, protect, kill with explosion, remove) | `EXTRA01`, `K17`, `U01`, `U07`, `U08`, `U11`, `U12`, `U15`, `U22`, `U23`, `U24`, `U25`, `U29`, `U34` | 14 |
| Success-rule replacement expressible as data (touchdown after takeoff; forced catapult failure then ejection) | `CATFAIL`, `UKR02`, `VIET03` | 3 |
| Bespoke phased objective (timed target windows with early and late failure) | `K16` | 1 |
| No player effect (unreferenced) | `FOO`, `TRAIN01`, `UKR01` | 3 |

`K16` can also be written as data with a "phase" primitive (time, target set,
messages, fail if a set survives or dies early), but no other module needs it.

Recommendation for the M3 mission loader: never load MC code. Recognise a
module by its `code` name and CODE SHA-256 from the table above and map it to a
T.O.R.E mission-script description built from the [spec](../spec/mission-code.md).
Message text and voice stems can be read at import time from the user's own
module bytes at the inline string positions (bounded data reads, the same way
`module.rs` treats CODE as data), so no retail text enters the repository. An
unknown or user-made module name loads the mission with the default success
rule and a warning, which keeps native-strip's "non-empty code stays inert"
rule for anything unrecognised.

## Unknown and next steps

- **Literal message presentation.** How subtype 0x24 text is drawn (line,
  colour, duration) is not recovered here; trace `SAYMsg` for subtype 0x24 next
  to the existing [radio notes](radio.md).
- **Kills by script.** Who is credited for `U11`'s `_Kill` in the debrief, and
  whether the kills count as friendly losses or enemy losses for scoring: read
  `_Kill` 0x473c10 and the debrief counters.
- **Protected cruiser.** Whether a single hit that empties the Kirov's hit
  points in `U07` sinks it: follow the native-strip hit dispatch removal for a
  ship type.
- **Deck touchdown.** That `_OnTheGround` is true on a carrier deck is inferred
  from the missions being winnable; confirm in `_T_Info` 0x4abab0.
- **Catapult failure visuals.** The 73 ft/s hold is static evidence; what the
  player sees on the bow (sink, roll off) needs the carrier deck-edge rules.
- **Route index meaning.** "Current waypoint" is read as the waypoint being flown
  to, from `_WPChange`; confirm against the waypoint-reached handler.
- **Multiplayer host.** Whether a host has computer index zero (and so would
  run a module) is not established; campaign missions are single player.
- **Default success rule.** `_MISSIONSuccess@0` itself (which pass-through
  modules keep) is outside this page; it includes a 300 s gate at 0x481b5a.

## Reproduction

All work in a scratch directory outside the repository:

1. `tore-extract --source gameassets/fighters-anthology --include '*.MC' --include '*.M' --include '*.MT'`, plus `*.CAM` from `FA_2.LIB`.
2. Copy each module with the `PL` signature changed to `PE` so `objdump` reads
   it; `objdump -x` lists sections and named imports.
3. Disassemble CODE with `objdump -d -M intel`, restarting after each short jump
   whose skipped bytes are printable text (the inline strings).
4. Parse `FA.SMS` with the reader in `tools/extract_native_flight.py`
   (`symbols()`) to name FA addresses; disassemble `FA.EXE` `.text` for the
   host side.
5. Resolve aliases by reading the `obj` blocks and `waypoint2 ... w_for` route
   blocks of each `.M` file, and compare with the `.MT` briefing and debrief
   sections.
