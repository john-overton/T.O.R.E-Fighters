# Flak egg, rate and muzzles

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

**Since 2026-10-14** (slice nr-flak2, [flak teams](flak-teams.md)): the
eight-shell opening barrage leaves 0.75 s a shell (it was a quarter second, so
a pass now sees fewer of its shells in the first seconds), a flak burst knocks
an aircraft half as hard, and the guns fight in teams that hold their fire and
spread it over a flight. The figures below are the 2026-10-13 build's: one gun
against one aircraft, before those changes.

Implementation mode, slice nr-flak, 2026-10-13, on multiplayer 5749aaa8
(protocol 28, unchanged). Contract: [flak](../spec/surface-defenses.md#flak),
[muzzles](../spec/surface-defenses.md#muzzles) and [flak
sound](../spec/sound.md#flak-bursts). John's requests of 2026-10-13: the
flak explosion sound should be checked against retail; flak "needs to be
within a sphere around the target of about 300m wide and roughly 100m in
altitude (not really a sphere but an egg)"; its rate of fire cut to a third;
the muzzle flash of the KS-12 and KS-19 came "from the center of the gun
instead of the end of the barrel".

## What was run

- Debug build on Linux x86_64 (Ryzen 9 7900X), the profile `surface-data`
  copied (never run in place). Single machine, deterministic: the same command
  gives the same numbers.
- The accuracy probe `tore-app --surface-trace TVIET QTAAA --over KS12|KS19
  --solo --skill 1 --rng SEED ...`: one gun of North Vietnam's AAA
  emplacement (every other unit laid down), the player flying a straight line
  over it, 90 s (220 s for the helicopter), four seeds a case. `damage` is the
  sum of the hit points the player lost to that one gun in a pass (a
  destroyed aircraft stops counting); the F/A-18D has 232.
- Cases: the F/A-18D at 450 kt at 6,000, 10,000 and 15,000 ft over the gun, at
  8,000 ft with the line 6,000 ft to the side, and the AH-64 at 100 kt at 6,000
  ft. The flak floor is 4,000 ft, so these are all in the fire zone.

## Why flak was "super strong"

Four things, found by reading the code and measuring (before, on 5749aaa8):

1. **Aim.** Each shell flew at the lead point with 0.2 to 0.6 degrees of
   error: at 15,000 ft that is 50 to 160 ft. It always passed within a few
   hundred feet of the target.
2. **Proximity fuze.** The record's 250 ft fuze burst a shell the moment it
   came within 250 ft of any hostile aircraft, so almost every shell went off
   close to the target whatever the aim.
3. **Splash.** Every burst does 35 percent of the record's damage (50 for the
   KS-12, 80 for the KS-19, so up to 17.5 and 28 hit points), falling in a
   straight line to nothing at 750 ft. Everything within 750 ft is hurt, so
   a burst a long way off still did a third of that.
4. **Rate and opening barrage.** One shell every 4.25 s (14 a minute) and an
   eight-shell opening barrage in 1.75 s the first time a gun engages.

Before, every burst reached the aircraft, 10.5 hp on average from a KS-12 and
9.6 from a KS-19. One gun took 72 percent of an F/A-18's hit points in a pass
(KS-12) or 91 percent (KS-19); it destroyed a helicopter in every run.

## What changed

| | Before | After |
| --- | --- | --- |
| Burst point | The lead point plus 0.2 to 0.6 degrees of error by skill and 0.25 degrees of round dispersion | A point drawn uniformly in an egg 300 m wide and 100 m tall around the target as it will be when the shell arrives; no skill error, no dispersion; a jammer doubles it |
| Fuze | Time fuze to the lead point, or the 250 ft proximity fuze | Time fuze to the burst point only; a shell still bursts on striking an aircraft or the ground |
| Splash | 35 percent falling to 0 at 750 ft | 35 percent falling to 0 at 400 ft |
| Rate | 4.0 s pause, 4.25 s a shell, 14.1 a minute | 12.5 s pause, 12.75 s a shell, 4.7 a minute |
| Burst sound | KS-12: type 27 (retail row); KS-19: type 28 (not retail's row) | Both type 27 |
| Land guns' muzzle | 10 ft above the unit's middle | The end of the drawn barrel |

The rate and egg are John's (opinionated, 2026-10-13); the distribution
(uniform in the egg's volume), the no-proximity-fuze rule, the 400 ft splash
and the barrel tips are agent decisions, fitted.

## Results

Per gun, per pass, mean of four seeds. "Shells" counts the shells fired in the
pass (the eight-shell opening barrage is most of them in a short pass).

| Case | Shells before / after | Damage per pass before | After, splash 750 ft (egg and rate only, first build) | After, final (splash 400 ft) |
| --- | ---: | ---: | ---: | ---: |
| KS-12, F/A-18 at 6,000 ft | 16 / 10 | 168 | 83 | 42 |
| KS-12, F/A-18 at 10,000 ft | 16 / 10 | 168 | 84 | 40 |
| KS-12, F/A-18 at 15,000 ft | 12 / 9 | 108 | 68 | 29 |
| KS-12, F/A-18 at 8,000 ft, 6,000 ft to the side | 16 / 10 | 168 | 86 | 41 |
| KS-12, AH-64 at 6,000 ft | 20 / 16 | 206, destroyed in 4 of 4 | 159 | 74 |
| KS-19, F/A-18 at 6,000 ft | 22 / 12 | 211 | 66 | 69 |
| KS-19, F/A-18 at 10,000 ft | 21 / 12 | 211 | 66 | 70 |
| KS-19, F/A-18 at 15,000 ft | 15 / 11 | 232, destroyed in 4 of 4 | 138 | 66 |
| KS-19, F/A-18 at 8,000 ft, 6,000 ft to the side | 21 / 12 | 211 | 69 | 70 |
| KS-19, AH-64 at 6,000 ft | 29 / 23 | 206, destroyed in 4 of 4 | 178 | 168 |

(The "splash 750 ft" column is the first build, before the aim was centred on
where the target will be and the shell's dispersion was removed, so its KS-19
numbers are lower than a same-splash final would give. It is shown to give the
size of the splash change: about a half for the KS-12.) Nothing is destroyed in
the final column: a lone KS-12 now takes about 17 percent of an F/A-18 in a
pass and a lone KS-19 about 30 percent; a helicopter loses about 30 percent to
a KS-12 and most of it to a KS-19.

The whole North Vietnam AAA emplacement (`QTAAA`, every unit firing, the 37 mm
guns included, which are unchanged), the F/A-18D over it at 450 kt for 150 s,
three seeds, no `--solo`:

| Altitude | Before | After |
| --- | --- | --- |
| 8,000 ft | destroyed at 73.3 s in all three | survives with 45, 41 and 62 hit points left |
| 15,000 ft | destroyed at 66.6 s in all three | survives with 31, 32 and 68 hit points left |

So a defended emplacement is still a serious threat (about 80 percent of an
F/A-18 lost in a pass) but a jet that keeps flying through it can live.

Share of bursts that hurt the aircraft at all: 100 percent before, 70 to 75
percent now (the others fall more than 400 ft from it).

### Where the bursts fall

402 bursts of a KS-12 and a KS-19 at the F/A-18 (8,000 and 15,000 ft, eight
seeds each) measured from the player's position at the moment each burst
appears (`flak-burst ... ft east, ... ft north and ... ft up of the player` in
the trace; a jet flying straight at constant speed stands where the gun led
to):

| | Mean | Spread (sd) | Extreme |
| --- | ---: | ---: | ---: |
| East (across the line) | +12 ft | 227 ft | -452 to 449 ft |
| North (along it) | +19 ft | 228 ft | -446 to 475 ft |
| Up | +6 ft | 72 ft | -143 to 153 ft |

A uniform egg 492 ft out and 164 ft up has an sd of 220 ft sideways and 73 ft
up, a mean scaled radius of 0.75: measured 0.765. 97.3 percent of bursts lie
inside the egg and all but one inside 5 percent past its edge (the target's own
tick-by-tick motion). 22 shells of 424 fired at a jet already flying away at
the edge of the gun's reach burst short at their end of life, more than 7,000
ft behind it; they are not in the figures. Before the aim was corrected, the
bursts were spread 150 ft too far along the line, because the shell arrives at
its burst point, not at the egg's centre, at a different time; that is
accounted for now.

## Muzzles

Every land gun's NT hardpoint is `[0, 30, 0]`. The barrel tips come from the
shapes ([muzzles](../spec/surface-defenses.md#muzzles)); renders before and
after for the KS-12, KS-19 and ZSU-23 are in
`.local/next-round/renders/nr-flak/` with a README. All 14 land gun shapes
listed there were wrong (the flash and shell started at the unit's middle).

## Sound

[Flak sound](../spec/sound.md#flak-bursts): retail's row 27 (read again from
FA.EXE) is what the KS-12 played; the KS-19 played type 28's recordings, now
type 27's. Nothing was played aloud.

## Not run

- A windowed or audio run; the render of the muzzle flash is offscreen.
- Online and replay playback of flak by eye. The burst is one simulation event
  with its sound, carried as before; the unit tests and the replay and network
  scenarios in the battery stand for it.
