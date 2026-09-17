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
