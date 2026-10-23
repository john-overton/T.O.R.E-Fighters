# Radar source findings

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-16. Build hashes and inspection method are in the
[baseline](../baselines/radar.md); player-facing values live in the
[radar spec](../spec/radar.md).

## Records and consumers

The existing SENSOR schema maps packed SEE offsets: signature +8, capability
flags +9, lookDown +10, Doppler above/below/minimum +11/+12/+13, allAspect +14,
zone 0 +15 and zone 1 +35. The reviewed PROJInFOV at 0x4c2860 selects the zone
and compares relative altitude, range and angle.

Application code now reads both zones and the lookDown coefficient. The shared
sensor component keeps search and track volumes independent, applies the
look-down coefficient through the authored clutter model in the
[component guide](../radar.md), selects RWS or TWS automatically from the display
range against the installed tracking range, and carries a stable target identity
on every plotted contact so the scope label, the projection and the mouse pick
all agree. The earlier cosmetic three-mode label cycle is gone.

## Newly inspected look-down consumer

Static inspection of 0x4c2b50..0x4c2e36, called by PROJInFOV, provides more
specific evidence than the reference checkout's hypothesis of a flat range loss.
The helper calls COSig at 0x478200 and retains its result as effective signature.
At 0x4c2bff it reads SEE +10. The normal branch at 0x4c2c15..0x4c2c27 requires
sensor altitude greater than target altitude and a positive lookDown value.

At 0x4c2c40 it calls Angles (0x411a40). It multiplies the signed angle output
by lookDown, divides by -8190 (45 degrees in the established 65,520-unit turn),
and clamps the result to 0..100. The interpretation as downward sight angle
needs the angle helper's output contract checked, not just its SMS name.

At 0x4c2c92 it calls T_Info (0x4abab0) at the target position. It subtracts
the returned height from target Y, compares the result with 0x138800, and below
that threshold computes (threshold - height) * lookDown / threshold, clamped
to 0..100. The established fixed8 position units make that threshold 5,000 ft.
It takes the larger of the angle and height penalties. Ground-query context is
partly documented in [native flight](native-flight.md).

If target type flag byte +0x0e has bit 0x20, the penalty is divided by three.
Its player-visible category is not yet established. A caller can request the
penalty separately; otherwise 0x4c2d02..0x4c2d15 reduces effective signature by
that percentage. Later the helper returns a comparison distance scaled by
100 / effective signature. Zero signature and an additional low-signature
cutoff return an effectively infinite comparison distance. Distance input units,
cutoff meaning and complete COSig modifiers need bounded follow-up.

The named reduceLookDown global at 0x50ce28 changes the coefficient: object kind
4 uses 110, other kinds double it. Cockpit code at 0x43e1a1..0x43e1f0 sets and
clears this around a PROJInFOV call under several flags. Those flags and object
kinds must be identified before claiming the normal formula covers all cases.

## Doppler and other remaining branches

0x4c2d27..0x4c2dbe gates a Doppler test on SEE flags bit 4, reads the minimum
range byte, selects above/below speed thresholds and compares a projected speed
against ten times the selected threshold. This shows a Doppler path exists;
angle orientation, units and caller use need review before specifying notching.
The nine inspected roster radar records all have these three bytes zero and
capability flags either 0 or 1, not bit 4. Bit 1 is still unresolved and is not
proven to mean TWS or semi-active illumination capability. The three source
Doppler bytes and the capability flag word are preserved verbatim in the imported
profile; the notch the remake applies is authored and takes its widths from the
agent preset, not from these zeroed fields.

Useful bounded follow-ups: CPRadarRange 0x43ddd0, CPNextTarget 0x440e10,
PROJSetTarget 0x4c0870, PROJLock 0x4c2f20 and PROJLockUpdate 0x4c0960.
Names locate evidence; they do not establish behaviour by themselves.

## Aircraft bindings, range ladder and automatic mode

The follow-up audit resolved PT :hards defaultTypeName pointers, then parsed each
referenced SEE record. All twelve imported aircraft have one signature-3 radar.
Bindings match the current AircraftId::radar mapping. Additional signature-2 IR
and signature-1 laser devices stay distinct. Results and raw source hashes are
in ignored `.local/radar-research/roster.json`; reproduction is in the baseline.
The spec owns the aircraft table and nominal sensor statistics.

Bounded PE reads of FA.EXE give the six one-byte range entries at 0x4f3ef8 and
the five string pointers at 0x4f3f00. CPRadarRange at 0x43ddd0..0x43ddfd clamps
the selected index to 0..5. CPResetRWR sets index 1 at 0x43e839. The scope reads
the same range table at 0x43fc8e and 0x43ff9e. The label lookup at
0x43ff6e..0x43ff99 maps mode 0 to RWS, 1 to TWS, 2 to IR, 3 to HARM, 4 to A-G.
These are static data reads, not execution of retail drawing code.

At 0x440d28..0x440d38 the selector requests signature 3 equipment via 0x452d90
and checks the returned station count. At 0x440d48..0x440d77 it converts the
selected range to fixed8 feet and compares it to SEE +0x2b, zone1.maxRange.
Selected range <= that bound stores mode 1, otherwise mode 0. The ordinary
own-radar branch has no additional era, radar name or capability flag test.
At 0x440e4c..0x440e6d CPNextTarget refreshes that mode and returns no target for
mode 0 unless UsingSuppRadar succeeds. This proves the normal target-cycle
restriction; it does not establish the mouse handler, retention of an existing
target, or post-launch guidance when the player increases scope range.

The older USNF manual at ignored
`USNF-ATF/Docs/reference/JANES_US_NAVY_FIGHTERS_djvu.txt:1950` describes automatic
range modes, contact symbols, Y history and IR/HARM selection. It corroborates
the general interaction but its range table differs from FA sensor records.
Do not transplant its numbers or infer complete FA mouse/history behaviour.

## Angle helper follow-up

0x411a40..0x411ae9 computes horizontal magnitude from X/Z and passes vertical
separation plus that magnitude to the angle routine; its fourth argument receives
the elevation result clamped to +/-16380. The look-down consumer passes a null
third argument and a valid fourth output. This supports the downward elevation
interpretation in the spec. Signed delta convention, original integer rounding,
complete COSig modifiers and the special caller branches still limit exact
numeric parity claims. The component deliberately isolates these policies from
profile import, scope controls and contact identity.

## RCS panel and signature coupling

The supplied image is titled RCS and shows a relative-bearing ring, a central
contour and three filled squares. FA manual chapter 4, pages 94-95, provides the
functional interpretation recorded in the [RCS spec](../spec/rcs.md). It is an
EA/Jane's manual mirrored on PDFCoffee, not a matched run of this local build;
its glyph/risk explanation is corroborating documentation, not numeric validation.

Directly inspected CPComputeRCS at 0x43e8c0..0x43ea33: base signature buckets
come from PT/object +0x45 at 0x43e9ae..0x43e9f0; gear bit 0x40 at instance
+0x16f adds 2/4; pitch word +0x1f and bank word +0x21 feed distinct additions.
The zero second-argument path writes display dimensions. The nonzero path adds
2*front_increment + 5*side_increment to the incoming signature, except for an
unresolved type/player branch that uses side_increment without the factor 5.
These are arithmetic observations, not a request to preserve the control flow.

The scope at 0x43ea40 calls the helper with zero. Its emitter/contact collection
calls 0x43e330 through 0x43f0e0; drawing at 0x43efa3..0x43f0b1 consumes contact
IDs and chooses glyphs from category/status fields. Full eligibility remains
unresolved. Do not label every dot an enemy or proof of a lock.

Crucially, COSig at 0x478337..0x47833e calls the same helper with the current
radar signature. This corrects a possible overreading of the ignored reference
notes saying the RCS drawing is not the detection calculation: the drawing is
not a calibrated range boundary, but its attitude/configuration helper also
feeds detection. The spec separates the two outputs explicitly.

## USNF history and IR functional reference

Read ignored `USNF-ATF/Docs/reference/JANES_US_NAVY_FIGHTERS_djvu.txt`, the local
Internet Archive OCR transcript, SHA-256
`cbb832fde3e80005a60acb454f74d4f15b9ac9db94ede1200db0b51025eb1dd7`.
Its [original source](https://archive.org/download/JANES_US_NAVY_FIGHTERS/JANES_US_NAVY_FIGHTERS_djvu.txt)
is identified by the accompanying reference README. The transcript has 197,638
bytes; it is an older-title manual, not the identity of the installed FA build.
No reference engine or other-title aircraft values were imported.

Lines 2066..2073 describe historical mode: Y or its scope button shows past
contact positions as dots. Lines 2041..2044 separately describe the TWS heading
tail and designation bars. The text supplies no interval, capacity, lifetime,
fade, gap or toggle-retention contract. The guide's eight samples at half-second
intervals are an agent choice, not recovered timing.

Lines 2078..2099 describe IR equipment and selection by I or the M button, and
return to radar using R. Lines 3652..3675 describe passive heat sensing and
cloud/fog limitations. These establish useful functional expectations, not the
FA roster's installed sensors or the new model's weather attenuation. Current
FA PT/SEE records supply that roster independently.

Lines 3872..3923 describe semi-active dependence on launcher lock, active radar
fire-and-forget after launch, and IR seeker lock without radar support. No active
activation distance or timing is supplied. Rust snapshots designation
into each projectile at launch (`crates/tore-sim/src/combat/live.rs`), and its
weapon-specific tracking gate distinguishes continued radar dependency. See
[the reviewed weapon evidence](../baselines/manual-weapons.md) for R530 versus
AIM120/MICA; this is not a new audit of every weapon. The component preserves
independent target ownership, and required illumination is now specific to the
missile's assigned target rather than a check of radar power.

Earlier Rust audit: target HP gated movement, radar/visual observation, scope
readout and designation together, while weapon readiness separately rejected
destroyed targets. Those responsibilities are now split: hit points reaching zero
leaves an airborne object observable, and a grounded wreck ends the air-to-air
observation. John's report that retail hides dead contacts is recorded as
user-provided behaviour, not a newly verified executable finding. The
single-track requirement is likewise a product constraint; this pass does not
claim to survey every Jane's title.

## FA manual contact symbols

Research inspection, 2026-09-21: local `.local/missile-update/manual.pdf`,
SHA-256 `1a082378a8e8cd163ed6b398efcc1df80b67c2f104f6b90ac0733c88d58e26c3`.
The page header identifies `1999 EDOC FA Ch4.qxd`, 5/24/99 11:26 AM.
Printed page 97 is PDF page 101. Read its Radar Contacts paragraph and rendered
the complete page with Poppler at 1500 pixels for visual inspection of both scope
figures. Aircraft symbols are filled in the figures. The prose distinguishes
small aircraft squares from single-pixel large surface contacts, names the
selection bars, and describes TWS motion flags relative to own heading.
The resulting [symbol contract](../spec/radar.md#air-and-surface-contact-symbols)
records the behavior and remaining ground-symbol uncertainty. No executable was
run to establish these manual findings.

## A-G page

Research mode, 2026-10-10, slice R2 of the air-to-ground round. Static read of
FA.EXE 1.02F (sha256 `e31560c2a6d6adb4...`, 1,319,424 bytes,
`gameassets/fighters-anthology/FA.EXE`) through the objdump listing
`.local/bezel-research/fa_exe.asm`, which was made from that file. The copy at
`.local/esa-probe/FA.EXE` is a different build (sha256 `c7d2c1cc...`, 1,299,968
bytes) and gave no addresses here. Nothing was executed. Player-facing rules
belong in the air-to-ground spec; this section keeps the evidence.

Units: positions and `_Dist` results are 1/256 ft; `_currentT` counts quarter
seconds and `_currentTicks` 1/256 s ([music notes](music.md)); angles use 65,520
units per turn (8190 is 45 degrees). Player fields are read through `_curThing`
0x50ce80: flag word +0xde at 0x50cf5e (0x100000 A-G, 0x4000 HARM, 0x2000 IR,
0x10 radar jammer), radar emission deadline +0x11a at 0x50cf9a, designated
target +0xee at 0x50cf6e. Object class words are the
[debrief table](debrief.md#outcome): 0xc000 aircraft, 0x2000 ship, 0x1000 SAM,
0x0800 AAA, then tank, vehicle, structure and other.

### Keys and mode changes

| Evidence | Finding |
| --- | --- |
| ASCII key table 0x415dcc into jump table 0x415cec; scan dispatch 0x414833..0x414838 | Shift+R (ASCII `R`) and scan code 0x1302 run the A-G handler 0x414faf. `r` runs the air radar 0x41559e, `m` the HARM seeker 0x4154c7, `i` the IR seeker 0x415381, `t` 0x415655 and `T` 0x41506e the target cycle. The modifier inside 0x1302 is not decoded here; the manual calls the key Ctrl+R (M:6673) |
| Gate 0x414faf..0x414fc4, 0x41505f | A-G needs a signature-3 sensor with a nonzero count (`HARDBestSeeker` 0x452e60 with weapons excluded). Without one: "No working radar found" (0x4ee5f4) |
| A-G on, 0x414fca..0x415016 | Sets 0x100000. Clears IR 0x2000 unless `_gamePrefs` bit 0x2000000 is set (the survey reads that bit as the IR/Laser advanced targeting preference; not re-verified). If the air radar was emitting (0x50cf9a nonzero) calls `CPUpdateRadar` 0x43e810, then zeroes 0x50cf9a: the air radar goes off. Clears HARM 0x4000 |
| A-G off, 0x41501d..0x415027 | Clears 0x100000 only. The air radar stays off |
| Messages 0x41502c..0x415055 | "Air-Ground Radar on" 0x4ee56c, "Air-Ground Radar off" 0x4ee554, then the button sound |
| Air radar `r`, 0x41559e..0x415641 | On: 0x50cf9a = 0x7fff, IR off unless the preference, HARM off, A-G off (`CPUpdateRadar` first when A-G was on). Off: 0x50cf9a = 0. "Air-Air Radar on" 0x4ee620, "Air-Air Radar off" 0x4ee60c. Air and ground radar are therefore exclusive |
| Target | Neither the A-G handler nor the HARM handler writes the designated target 0x50cf6e. The survey's "A-G clears the current air target" should read "A-G turns the air radar off" |
| Emission | A human's radar emits while +0x11a lies in the future (`PROJRadarIsOn` 0x4c2eb0..0x4c2ec9). A-G is tested separately as flag 0x100000 (0x4c31f0..0x4c3207) and never sets +0x11a. `COSig` gives an anti-radiation seeker a signature only for +0x11a or the jammer bit 0x10 (0x478225..0x478242), so in A-G with the jammer off the player is silent to enemy anti-radiation seekers. Whether the RWR reads the same fields is not traced |

### Page, range and refresh

| Evidence | Finding |
| --- | --- |
| Scope update 0x43faf0: selector call 0x43fb04, override 0x43fb09..0x43fb44 | After the ordinary selector 0x440d00, flag 0x100000 forces scope mode 4 and takes the best signature-3 sensor (weapons excluded) as the page sensor. A-G therefore overrides the IR and HARM pages. The label table 0x4f3f00 entry 4 is "A-G" (0x4f3e68); the range ladder is the ordinary one at 0x4f3ef8, 5, 10, 25, 50, 100 and 150 nmi |
| Box 1, 0x43fcb7..0x43fce4 | Half side = min(2 x the selected range, the sensor's zone-0 maximum range). The range is converted as range x 1519 << 11 >> 16 in 256 ft units, which is 2 nmi per range unit. The scope clip box (0x4403c7..0x4403ea) limits what is drawn |
| Box 2, 0x43fd1b..0x43fd6b | Half side = min(box 1, a cap): 0x42c (1,068 x 256 ft = 273,408 ft, 45.0 nmi) in mode 4; 0x2c8 (182,272 ft, 30.0 nmi) in mode 3, HARM; 0x76 (30,208 ft, 4.97 nmi) in the other modes. Ground objects, and aircraft in mode 4, must lie in box 2 (0x43e0a4, 0x43e142) |
| Refresh 0x43feb6..0x43ff0e | The contact list is rebuilt every 0x600 ticks (6 s) in modes 0, 1, 2 and 4, and every 0x100 ticks (1 s) in mode 3. Each listed contact's position is refreshed from the live object at every redraw, every 0x40 ticks (0.25 s) (0x43ff52..0x43ff69, 0x43f280..0x43f2dd) |
| List 0x53bea8..0x53da00, add 0x43e450 | 50 entries of 0x8c bytes. When the list is full, the farthest entry is replaced if the new object is nearer (0x43e4e5..0x43e524) |
| Merge, 0x43f0e0 with divisor 20 (0x43fecf) | In mode 4 a ground object is not added while an already listed ground contact lies within (its distance from the player) / 20 of it (0x43f1e1..0x43f241). Aircraft follow the same rule against aircraft (0x43f170..0x43f1c9). Ground objects closer together than 5 percent of their range show as one dot |

### What the page lists

Eligibility routine 0x43df70, followed for mode 4:

| Evidence | Finding |
| --- | --- |
| Every contact, 0x43df8b..0x43e00e | Not the player; instance active bit +1 & 1; type flag +9 & 0x20 (retail ground units carry 0x821, ships 0x131). With a radar sensor in any mode but A-G, type flag 0x10 is also required (0x43e00f..0x43e02c) |
| Aircraft, class & 0xc000, 0x43e036..0x43e0c2 | Listed in A-G only while airport-attached: state +0xe3 in 1..0x12 (takeoff sequence) or 0x13..0x1e (landing sequence), see [airfield sequences](ai.md#airfield-takeoff-and-landing-sequences); within `maxVisDist` x 256 ft (type +0x3b); inside box 2. An aircraft in free flight (state 0x1f) never shows on A-G, whatever its speed |
| Ships, class 0x2000, 0x43e10b..0x43e118 | Modes 4 and 2 only, inside box 2, no `maxVisDist` test |
| Other classes (SAM, AAA, tank, vehicle, structure, other), 0x43e129..0x43e181 | Modes 4, 3 and 2. In mode 4 only when `maxVisDist` x 256 >= 19,000 ft (0x4a38, so `maxVisDist` >= 75) and within 3 x `maxVisDist` x 256 ft. Retail values: ground units 78 (59,904 ft, 9.9 nmi), the GCI radar 195 (24.6 nmi), SA-2 391 (49.4 nmi); men (59) and the officer (74) never show |
| `maxVisDist` unit | The shifts at 0x43e099..0x43e0a0 and 0x43e16a..0x43e181 compare `maxVisDist` << 16 with a 1/256 ft distance: the unit is 256 ft. This closes the unit question left open in [surface units](surface-units.md) |
| Then 0x43e191..0x43e218 | Projectiles (kind 6) are excluded. `PROJInFOV` 0x4c2860 with the radar's zone 0 (search), with the relative altitude window, range and angle about the nose, then `COLTerrainBlocking` 0x42e4e0 must report a clear line |
| Detection range | Range is compared after scaling by 100 / effective signature (0x4c2dff..0x4c2e30). Effective signature = the target's `sigs[3]` (type +0x3f + 2 x 3, `COSig` 0x47824c) times the weather layers' selector-3 percent (0x47839f, [weather](weather.md#effect-selectors)), less the look-down penalty: a target within 5,000 ft of the terrain beneath it loses (5,000 - height) x `lookDown` / 5,000 percent, so a vehicle on the ground loses the full `lookDown` (0x4c2c7b..0x4c2cd5; F-18, F-22, MiG-29 and Su-27 radars have 0, A-10, AV-8B, F-14 and F-15 30, F-4 and F-104 50). Class high byte 0x20, ships, divides the penalty by three (0x4c2cd9..0x4c2cee): the "+0x0e bit 0x20" category in [the look-down consumer](#newly-inspected-look-down-consumer) is the ship class |
| Air modes, for contrast, 0x43e054..0x43e064 and 0x43e0d6..0x43e0f7 | RWS, TWS and HARM drop aircraft below 189 ft altitude (0xbd00, sea-level altitude, not height above ground) and aircraft in landing states 0x16..0x1e |

The same routine serves the IR page (mode 2): ground objects and ships pass
inside box 2, which is capped at 4.97 nmi in that mode.

### Symbols

The scope prints one character per contact in the window font
(`_HUDSetWinFont` 0x40ab50 selects the `WINxy.FNT` loaded at 0x438e7e..0x438e9f),
centred both ways (`G_Print` flags 5, 0x4986cd..0x4986f0). Choice at
0x440914..0x4409f2:

| Class | Character | Designated adds |
| --- | --- | --- |
| Aircraft (0xc000) | 0x02 | 0x04 |
| SAM or AAA (0x1800) | 0x03 | 0x04 |
| Everything else, ships included | 0x01 | 0x05 |

Decoded with the repository's FNT grammar, `WIN11.FNT` draws 0x01 as a 2 x 2 dot,
0x02 as a 5 x 5 filled square, 0x03 as a 5 x 5 X, 0x04 as two 2-pixel bars 14
pixels apart and 0x05 as bars 10 pixels apart; `WIN00.FNT` has a 1-pixel dot,
3 x 3 square and X, and bars 7 and 5 pixels wide. So the X for SAM and AAA sites
is drawn on every page, not only with remote targeting as the manual implies.
Colour 0x2c; contacts received from a supplementary radar (entry +0x8a) use 0x29
(0x4408db..0x4408ea, 0x44099d..0x4409ac).

Not drawn on the A-G page: TWS direction lines (mode 1 only, 0x440853..0x44090f),
jammer strobes (modes 0 and 1, 0x440564..0x440847), chaff circles (modes 0 and
1, 0x440529..0x44053f) and the TWS target readout (mode 1, 0x440a2c..0x440ac1).
Clutter noise appears in mode 4 only with bit 0x40000000 at 0x50cfef, which
also suppresses every contact (0x44048a..0x440494) and is read as radar display
damage; in modes 0 and 1 also when pitched below -20 degrees (0xf1c8)
(0x440adf..0x440bd7).

### Designation and weapons

| Evidence | Finding |
| --- | --- |
| Cycle gate 0x41506e, 0x415655 | `t` and `T` act when the air radar emits or any of A-G, HARM or IR is on (0x106000) |
| `CPNextTarget` 0x440e10..0x440e4b | Returns nothing unless a cockpit window of type 9 is open; type 9 is read as the radar window (inference) |
| Order 0x440e6e..0x440fce | `t` picks the nearest listed contact farther than the current target: nearest first, wrapping to the nearest. `T` picks the farthest contact nearer than the current target: farthest first, wrapping to the farthest. Candidates are on the scope (screen x not 0x8000), not jammer strobes, alive, with type flag 0x20; this routine has no side test. The pick goes through `SetPlayerTarget` 0x4140a0 |
| Launch, `PROJLock` 0x4c2fbc..0x4c3011 | At release a signature-3 weapon needs the launcher's air radar emitting for an air-target record (flag 0x10000), or its ground radar on for a surface-target record (flag 0x20000; for a human that is the A-G flag, 0x4c31f0..0x4c3207). AGM-84A, AM-39, AS-16, AS-7 and AS-15 (all 0x20000 without 0x10000) need A-G at release |
| Support in flight, 0x4c3054..0x4c30e4 | A supported record (0x200: AS-7, AS-15) needs the launcher's A-G on for the whole flight; for a human launcher the target must also stay inside the missile's zone 1, range included, seen from the launcher. An AI launcher's ground radar is flag +0xde & 0x400, which also refreshes its emission deadline for 10 s (0x4c320f..0x4c324a) |
| Remote ground targeting, 0x43de10..0x43de4c and 0x43e220 | A friendly aircraft with plane flag 0x4000 (J-STARS) above 1,000 ft feeds the A-G and HARM pages; its contacts sit in a box of its zone-0 range, else 303,616 ft (0x4a2); A-G takes no aircraft from it. Out of scope this round (no J-STARS imported) |

### Unknowns

| Unknown | Next step | Fallback |
| --- | --- | --- |
| The manual's 90 kt split | No speed comparison exists on this path (0x43df70, 0x43f0e0, 0x43e450, `PROJInFOV` and its helper 0x4c2b50, whose Doppler test needs SEE flag 0x4 that no aircraft radar sets). Search the remaining scope writers and the RWR for a speed test | The code's rule, class and airport state, as retail data; the manual's 90 kt (a hovering helicopter on A-G) quoted as a conflict, or kept as an opinionated addition |
| Mouse designation on the page | Trace the scope click handler | The plan's click rule |
| Keeping a ground designation that leaves the list | The drop rule at 0x416556..0x4165ac runs only under `_gamePrefs` bit 0x10000000 and uses `Targetable` 0x4141d0, which has no A-G branch (visible, selected weapon can lock, air radar, HARM or IR). Identify the preference bit | The plan's retention rule |
| Modifier of scan code 0x1302 | Decode the key-code builder | Manual: Ctrl+R |

## HARM page

Same build, listing and units as the [A-G page](#a-g-page).

| Evidence | Finding |
| --- | --- |
| Gate 0x4154c7..0x4154df, 0x415560 | `m` needs an anti-radiation (signature 4) weapon with a nonzero count; here weapons count (`HARDBestSeeker` with edx = 1). Without one: "No HARM seeker found" (0x4ee580) |
| On, 0x4154e1..0x41551b | Sets 0x4000, zeroes 0x50cf9a (air radar off, without `CPUpdateRadar`), clears A-G 0x100000, clears IR unless `_gamePrefs` bit 0x2000000. Off clears 0x4000 only. "HARM seeker on" 0x4ee5a8, "HARM seeker off" 0x4ee598 |
| Mode, selector 0x440dd4..0x440e04 | With the air radar and IR off, HARM gives mode 3 and the page sensor is the seeker (JT +0xac) of an anti-radiation weapon aboard. Label "HARM" (0x4f3e6c) |
| Which seeker, `HARDBestSeekers` 0x452e11..0x452e2a | Per signature the routine keeps the candidate with the smallest zone-0 maximum range. With AGM-45 (60,000 ft) and AGM-88 (200,000 ft) both aboard, the page uses the AGM-45 seeker |
| Field of view | Both records' zone 0 is 45 degrees (8190) in heading and pitch about the nose; the page call passes no slew (0x43e1d1..0x43e1eb) |
| Range and refresh | Box 1 = min(2 x range, seeker zone-0 range); box 2 caps ground objects at 30.0 nmi (0x2c8, 0x43fd24). Rebuilt every second (0x43fec5..0x43fef1) |
| Listed objects, 0x43df70 in mode 3 | Aircraft by the air-mode rules (altitude 189 ft and up, not landing states 0x16..0x1e) inside box 1; ships never (0x43e10b..0x43e11f); other ground classes inside box 2 with no `maxVisDist` test; then the seeker's zone 0 with range and the terrain line |
| Emitters only, `COSig` 0x478216..0x478247 | A signature-4 seeker sees 100 only on an object of kind 2 (surface unit) or 4 (aircraft) ([kinds](music.md)) whose radar deadline +0x11a lies in the future or whose jammer bit 0x10 is set; anything else is 0, and a zero signature fails the range test (0x4c2dbe..0x4c2dd3). Weather selector 4 scales it (100 in every retail layer) |
| Symbols | The A-G rule: SAM and AAA as the X (0x03), other ground emitters as the dot (0x01), aircraft as squares. The page has no painting or tracking distinction; that belongs to the RWR |
| Designation | `t` and `T` as on the A-G page |
| HARM launch and flight, `PROJLock` 0x4c2f20 | The anti-radiation records carry none of 0x100, 0x200 or 0x400 (AGM-88 0x2204f, AGM-45 0x2204f). At release the target must be emitting (signature check with range). In flight the lock is rerun every 0.5 s with angles only (0x4c12ce..0x4c135c), so a HARM keeps homing on the object after its radar stops, while the object stays inside the missile's zone-1 cone (45 degrees) |

| Unknown | Next step | Fallback |
| --- | --- | --- |
| How the page treats a designated emitter that stops emitting | It leaves the list at the next 1 s rebuild; whether the designation is dropped is the A-G retention unknown above | The plan's memory rule |
