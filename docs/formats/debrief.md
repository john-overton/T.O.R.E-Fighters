# Fighters Anthology mission debrief

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Research notes, research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Player-visible behaviour is specified
> in [the debrief spec](../spec/debrief.md).

Static reading of FA.EXE 1.02F with objdump on 2026-09-23, plus five retail
screenshots from John. No retail code was executed. The 1.0 build carries the
same debrief strings; its code was not compared. Code address = file offset −
0x400 + 0x401000; data address = file offset + 0x401a00.

## Resources

| Resource | Archive | Use |
| --- | --- | --- |
| `DEBSCR.PIC`, `DEBSC3.PIC`, `DEBSCU.PIC`, `DEBSCV.PIC` | FA_1.LIB | Backgrounds with the title, clipboard and control panel, each with its own 256-colour palette. DEBSCR is 640 × 540: rows 480 and below are blank padding. The page box is part of every background |
| `BRIEFSCR.DLG` | FA_2.LIB | Controls shared with the briefing: origin (48,363), size 192 × 91; OK action 1 at local (97,47) width 60; Cancel action 2 at (97,14) width 70; rocker at (48,39) |
| `QUICK.MT` | FA_2.LIB | Quick Mission text: section 3 MISSION SUCCESS, section 4 MISSION FAILURE |
| `BODYFONT`, `BOLDFONT`, `HEADFONT`, `PANELFNT`, `PANLFNT2` `.PIC` | FA_1.LIB | Body, bold and header text; PREV/NEXT; page counter. Chosen by name strings and matched by glyph widths to the screenshots |
| `&ROCKUP.11K`, `&ROCKDN.11K` | FA_2.LIB | Rocker sounds named beside the rocker art strings |

The page counter format strings are ` of  %d` and ` of %d`; the screenshots
measure the two-space form.

## Routines

| Address | Role |
| --- | --- |
| 0x4a2a30 | Builds the debrief text from the statistics block at 0x54be38 |
| 0x4a1dd0 | Runs the briefing/debrief screen; post-flight call at 0x404aef through 0x480020 |
| 0x4a1e99, table 0x4a2204 | rand(4): 0 DEBSCR, 1 DEBSC3, 2 DEBSCU, 3 DEBSCV. Not tied to theater or aircraft |
| 0x4a1e3d, list 0x50c990 | Tab stops 403 and 478 (campaign summary uses 422 and 497). Lookup 0x4b8630 does not move past the last stop |
| 0x4a41f0 | Count cell: 0 is `-`, otherwise `%d`; second mode gives ` (%d)` or nothing |
| 0x4a4290 | Percent cell: zero denominator is `-`; numerator clamped to 0..denominator; `num*100/den` as `%d%%` |
| 0x4854e0 | Per-missile event recorder: 0 launch, 1 hit plus damage, 2 jam, 3 spoof. Flag bit +0xde marks a missile resolved once (0x4854f4) |
| 0x4856f0 | Hit block selection by weapon flags +0xa6 (below) |
| 0x485820 | Kill classifier (below) |
| 0x485a40 | Landing score: player grades 0/1/2 add 0/50/100; wingman adds 40 + rand(20) |
| 0x484d90, 0x484ea5 | Objective counts and sentences |
| 0x481a70 | Default success rule, used without a mission script hook |
| 0x404915 | Single-player result: section 3 when success > 0, otherwise section 4 |
| 0x48b4e0 | Rocker drawing. A press on the top half (state 1) draws ROCKER01, ROCKER00 and plays `&ROCKDN.11K`; the bottom half (state 2) draws 03, 04. Release (state 0) draws back toward ROCKER02 and plays `&ROCKUP.11K`. One screen update per frame. Half regions are 18 × 16. A horizontal rocker uses ROCKERH frames |
| 0x48b450 | While pressed, state 1 lowers the linked value by its step (not below 0) and state 2 raises it (below the maximum), so the page turns on press |
| 0x45f090, set at 0x4913aa | Wingman slot 0x520a14: the flight's second slot when the player leads, else the leader |

## Page contents

Single player: the mission text section, then outcome, objectives, elapsed time
and PILOT STATUS, then KILLS, then HIT PERCENTAGES, then ENEMY HIT
PERCENTAGES. The statistics are appended to the section, starting with `.page`
when the section text is 13 bytes or longer. INCOMPLETE is only used in
multiplayer and the airbase mode. The keyword `printmissionoutcome` (default on)
can suppress the outcome block. Cancel (control 2) is always disabled.
Joystick buttons 1 and 2 act as Page Down and Page Up. The screen loops until OK.

- **Hit** uses `Hit\t%s %s`: hits ÷ launches, then ` (N)` with the damage those
  hits did. Hits on a same-side object that is not a target add a hit with 0
  damage (0x4c1916).
- **Failed** = (launches − hits − spoofed − jammed) ÷ launches.
- **Damage**: 100 when dead or ejected, otherwise hit points lost as a
  percentage (0x484ddf–0x484e1e; input words 0x5224ca, 0x522554, 0x5224cc
  undecoded).
- **Landing grade** is the average score `NN%`, `-` without landings. Grade
  names at 0x4ff960: Unsafe, Fair and Good landing. The carrier trap
  (0x41680e–0x41689b) gives Good for wire 2 or 3 inside a tight angle and speed
  window, Fair inside a looser one, otherwise Unsafe; plane flag 0x1000 without
  0x2000 always gives Fair. No OK/Cut/Bolter strings exist.
- **Status**: 1 Alive (object alive and still an aircraft), 2 Ejected (alive but
  no longer an aircraft, or eject flag 0x54e538), 3 Dead.
- **Elapsed time**: minutes and seconds, no hour wrap.

Weapon flags, for the player and wingman: 0x1 with 0x10000 Air-to-Air; 0x1 with
0x20000 Air-to-Ground; otherwise 0x10 Bomb and 0x20080 Gun. Enemy fire on them:
0x80 is Gun from an aircraft, AAA otherwise; anything else is AAM from an
aircraft, SAM otherwise.

Kills count only for the player and the wingman slot. Same side (0x80 of byte
+9) and not flagged as a target is Friendly fire. Otherwise the victim takes the
first match: an aircraft with plane flag +0xba bit 0x08 is Helicopter, then the
class word +0x0d: 0x8000 Fighter, 0x4000 Bomber, 0x2000 Ship, 0x1000 SAM,
0x0800 AAA, 0x0400 Tank, 0x0200 Vehicle, 0x0100 Structure, 0x40 Other.
Imported PTs carry 0x8000 (fighters) or 0x4000 (bombers, transports and
helicopters); static objects carry 0x100 or 0x40. The same counter table
appears in [native strip research](native-strip.md#score-gate-and-separate-death-counters).

## Outcome

Failure if the player has a friendly-fire kill. Otherwise any dead object
flagged 0x20 (protect) or 0x40 is a failure, a 0x40 object without 0x100 caps
the result at 0, and any living object flagged 0x80 (target) caps it at 0. No
objective flags means success. Protect-only missions also need word 0x5528e0
≥ 300 (meaning unknown). Targets are objects flagged 0x80, destroyed when not
alive; friendly objectives are objects flagged 0x20, protected when alive.

The Quick Mission object writer (0x4323c4–0x4323ea) writes flags $97 (target)
in single player and $17 otherwise. The retail `quick.M` in the install has the
player and wingman at $17 and three enemy aircraft at $97, matching the
screenshot's "Destroyed 0 of 3 targets".

After a non-campaign single-player mission the state resets to 0 at 0x404b97
and 0x403a0b, the main menu. It returns to the creator only when launched from
the Mission Creator state 3 or state 0x1f (medium confidence).

## Unknown

1. The damage input words above and the protect-only word 0x5528e0. Next: find
   their writers.
2. Runway landing grading and the carrier window units (0x50ceb4, 0x50cea1 via
   0x4c6614). Next: trace every transition into landing state 0x16.
3. Which quick-mission ground objects receive the target flag; whether a mission
   script hook applies to Quick Missions.
4. Whether one gun projectile is one round.
5. The font index to resource mapping inside the text parser (0x47e26e) and the
   text box origin; the page layout on the clipboard was fitted to screenshots.
