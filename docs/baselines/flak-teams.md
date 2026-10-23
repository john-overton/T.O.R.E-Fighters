# Flak teams

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, slice nr-flak2, 2026-10-14, on multiplayer 81593406
(protocol 29, unchanged). Contract: [flak teams](../spec/surface-defenses.md#flak-teams)
and [flak](../spec/surface-defenses.md#flak). John's requests of 2026-10-14:
flak "just attack[s] the first plane that comes in range until it dies ...
usually the flight lead", should "attack the group" as a team of guns within a
mile with "double the detection range", opening fire "once 25% of the detected
group is in range of the entire group" or when the group heads at it (the
same day's amendment); its push on aircraft cut by half, damage unchanged;
the eight-shell opening barrage slowed.

## What was run

- Debug build on Linux x86_64 (Ryzen 9 7900X), the profile `surface-data`
  copied (never run in place), throttled (`nice -n 10`, one run at a time).
  Deterministic: the same command gives the same numbers.
- The probe `tore-app --flak-pass TVIET QTSTRG --defenses 3 0 --solo-team
  --from 20 --seconds 200 --rng SEED ...`: North Vietnam's storage template
  with heavy AAA, whose four heavy flak guns (two KS-19, two KS-12, about
  4,000 ft across) form one team; every other armed unit and SAM battery laid
  down. A flight of F/A-18Ds (Friendly Wing 1) flies a straight line north at
  450 kt and 8,000 ft over the team's middle, from 20 nm short, in formation:
  member k sits k/2 times 1,500 ft back and the same out to alternate sides. It
  is put back on its line every tick, so nobody dodges. "AI" flights are the
  player and AI wingmen (an AI aircraft has 116 hit points and takes a burst's
  nominal damage; the player has 232 and rolls it); "seats" flights are every
  member flown from a seat, as players are online (232 each).
- Lone guns: the template's base-layout KS-19 and KS-12 (each the only heavy
  gun within a nautical mile, so a team of one), the same line.
- Before: base 81593406 with only the probe added. After: this slice. Three
  seeds (1, 2, 3) a case unless marked.

## Results

Per case, before and after, means over the seeds. "Shells" is the shells the
team fired at each member, lead first; "lost" the hit points each lost (an AI
member has 116, the player or a seat 232); "destroyed" how many runs each
member died in; "knock" the blast knock per flak jolt in units of a 100-point
warhead's (after the strength's floor of 0.5, so 0.5 before and 0.25 now).

| Case | | Runs | First shell s | Shells per member | Lost per member | Destroyed | Lead lived | Damage to the flight | Knock per jolt |
| --- | --- | ---: | ---: | --- | --- | --- | ---: | ---: | ---: |
| Team of 4, two seats | before | 3 | 103.8 | 40, 12 | 231, 60 | 1, 0 | 2 of 3 | 291 | 0.5 |
| | after | 3 | 103.1 | 27, 27 | 180, 179 | 0, 1 | 3 of 3 | 359 | 0.25 |
| Team of 4, four seats | before | 3 | 103.8 | 39, 1, 4, 7 | 226, 12, 25, 30 | 1, 0, 0, 0 | 2 of 3 | 294 | 0.5 |
| | after | 3 | 103.1 | 15, 15, 12, 12 | 121, 113, 77, 43 | 0, 0, 0, 0 | 3 of 3 | 354 | 0.25 |
| Team of 4, player and an AI wingman | before | 3 | 103.8 | 40, 11 | 231, 76 | 2, 1 | 1 of 3 | 308 | 0.5 |
| | after | 3 | 103.1 | 32, 19 | 206, 116 | 1, 3 | 2 of 3 | 322 | 0.25 |
| Team of 4, player and three AI | before | 3 | 103.8 | 39, 1, 4, 7 | 228, 11, 26, 30 | 1, 0, 0, 0 | 2 of 3 | 295 | 0.5 |
| | after | 3 | 103.1 | 17, 13, 11, 12 | 119, 111, 82, 46 | 0, 2, 1, 0 | 3 of 3 | 358 | 0.25 |
| Team of 4, four (player and AI) at 15,000 ft | before | 1 | 106.3 | 40, 1, 4, 7 | 221, 11, 24, 22 | none | yes | 278 | 0.5 |
| | after | 1 | 105.5 | 15, 12, 12, 11 | 83, 116, 64, 57 | the first wingman | yes | 320 | 0.25 |
| Lone KS-19, two | before | 3 | 114.8 | 11, 3 | 70, 17 | none | 3 of 3 | 87 | 0.5 |
| | after | 3 | 102.7 | 12, 2 | 64, 27 | none | 3 of 3 | 91 | 0.25 |
| Lone KS-19, four | before | 3 | 114.8 | 11, 0, 0, 3 | 70, 0, 0, 18 | none | 3 of 3 | 88 | 0.5 |
| | after | 3 | 102.7 | 10, 2, 2, 0 | 54, 27, 2, 0 | none | 3 of 3 | 82 | 0.25 |
| Lone KS-12, two | before | 3 | 139.8 | 9, 3 | 34, 11 | none | 3 of 3 | 44 | 0.5 |
| | after | 3 | 127.1 | 10, 2 | 34, 10 | none | 3 of 3 | 44 | 0.25 |
| Lone KS-12, four | before | 3 | 139.8 | 9, 1, 0, 2 | 34, 10, 0, 1 | none | 3 of 3 | 45 | 0.5 |
| | after | 3 | 127.1 | 8, 1, 2, 1 | 29, 1, 6, 10 | none | 3 of 3 | 45 | 0.25 |

The trigger cases (seed 1, four, player and AI; two likewise for the skirt):

| Case | Before | After |
| --- | --- | --- |
| Heading in, over the team | KS-19s open at 103.8 s, both on the lead | The team engages at 102 s as the first member comes into a KS-19's reach (heading-in; none yet in the KS-12s' reach, so the 25 percent rule alone would have waited about 25 s more); the two KS-19s open on the lead and the second member, the KS-12s on the third and fourth when the flight reaches them |
| Line 30,000 ft to the side (just inside the cone when the first member reaches a KS-19) | 26 shells from 117.5 s, 19 on the member nearest the team, which dies | The team engages at 117.5 s (heading-in); 13 shells each on the lead and the second member (the team's assignment), nobody dies |
| Line 35,000 ft to the side (skirting: never heading in once in reach, never inside the KS-12s' reach) | 24 shells from 126 s, 19 on the nearest member, which dies (four); 24 on the lead (two) | Holds: no shell at all, the trace showing the group "holding" from first sight to last |

What it means:

- **The lead is no longer the target.** Over a four-gun team a four-ship's
  lead drew 39 of 52 shells and lost almost all its hit points (dying in a
  third of the passes); now the fire is spread, about 15 shells each, and no
  seat died in any pass. Each gun's eight-shell opening barrage goes to a
  different member.
- **The flight as a whole takes more.** Total damage to the flight rose by
  about a fifth (four seats 294 to 354 hit points; two seats 291 to 359): with
  the fire concentrated, many shells fell on a lead already nearly destroyed,
  and the KS-12s' barrages now land on members they can reach rather than
  waiting on the lead. Damage per burst is unchanged, as John asked. With AI
  wingmen (half the hit points, and no damage roll) a wingman now dies in
  some passes where only the lead died before.
- **A lone gun opens at once.** Before, a gun whose target was still out of
  reach dropped its lock every 15 s (the launch gates' window) and prepared
  again, so its first shell came 12 s after the flight entered its reach; a
  team's gun (a lone gun is a team of one) now holds its lock and fires on
  the tick the flight comes into reach. It walks its later shells over the
  flight every 10 s; its opening barrage, one burst, stays on one member.
- **Skirting groups draw no fire**, including the member the KS-19s could
  reach: the ambush behaviour John described.
- **Knock**: every flak jolt is a quarter of a 100-point warhead's (half the
  0.5 floor), measured on every jolt of every run.

Reproduce: the cases above are `--size 4` (team) or `--size 1` (the first
lone gun, the KS-19; `--team 1` the KS-12) with `--flight 2|4`, `--seats`,
`--pass 30000|35000` and `--altitude 15000`. The battery scenario
`surface-flak-team` checks the heading-in, skirting, seats and lone-gun cases
([testing](../testing/lane-ai.md)).

## Follow-up, 2026-10-14 afternoon (slice nr-flak3)

John's answers: keep holding fire at a skirting group; damage spread over the
flight is fine; "in range of the entire team" is the team's **longest reach**;
the opening barrage is **spread over the flight** unless there is a single
target; lone guns opening sooner is kept. Implemented:

- In range of the team: inside the fire zone of at least one live gun, the
  union of the guns' zones (in the storage team, the KS-19s' 44,250 ft and up
  to 25,000 ft, which holds the KS-12s' zone). See
  [flak teams](../spec/surface-defenses.md#flak-teams).
- The opening barrage: each of a gun's eight shells at the next member of
  the engaged groups it can reach (fire zone, line of sight), its own target
  first; one target takes all eight.
- A gun's choice now prefers any member in its reach over its own member
  still outside it (agent: with the longer trigger reach a group grazing the
  edge engaged the team while each gun waited for its own member, which never
  came into reach, and nothing was fired).

Same cases, the same probe and seeds. "Before" is the end of nr-flak2 (the
"after" rows above), "after" this follow-up; the probe gained `--second N`
(Friendly Wing 2 flying on in the same formation) for groups of eight.

| Case | | Shells per member | Lost per member | Destroyed | Lead lived | Damage |
| --- | --- | --- | --- | --- | ---: | ---: |
| Team of 4, two seats | before | 27, 27 | 180, 179 | 0, 1 | 3 of 3 | 359 |
| | after | 28, 26 | 184, 172 | 0, 1 | 3 of 3 | 356 |
| Team of 4, four seats | before | 15, 15, 12, 12 | 121, 113, 77, 43 | none | 3 of 3 | 354 |
| | after | 16, 14, 12, 12 | 112, 98, 77, 67 | none | 3 of 3 | 353 |
| Team of 4, player and an AI wingman | before | 32, 19 | 206, 116 | 1, 3 | 2 of 3 | 322 |
| | after | 32, 17 | 213, 116 | 1, 3 | 2 of 3 | 329 |
| Team of 4, player and three AI | before | 17, 13, 11, 12 | 119, 111, 82, 46 | 0, 2, 1, 0 | 3 of 3 | 358 |
| | after | 19, 13, 10, 10 | 119, 95, 94, 82 | 0, 1, 2, 1 | 3 of 3 | 391 |
| Lone KS-19, two | before | 12, 2 | 64, 27 | none | 3 of 3 | 91 |
| | after | 9, 5 | 50, 41 | none | 3 of 3 | 91 |
| Lone KS-19, four | before | 10, 2, 2, 0 | 54, 27, 2, 0 | none | 3 of 3 | 82 |
| | after | 5, 3, 4, 2 | 25, 34, 19, 7 | none | 3 of 3 | 85 |
| Lone KS-12, two | before | 10, 2 | 34, 10 | none | 3 of 3 | 44 |
| | after | 7, 5 | 25, 20 | none | 3 of 3 | 45 |
| Lone KS-12, four | before | 8, 1, 2, 1 | 29, 1, 6, 10 | none | 3 of 3 | 45 |
| | after | 3, 2, 4, 3 | 11, 6, 17, 14 | none | 3 of 3 | 48 |
| Four at 35,000 ft to the side (seed 1) | before | none | none | none | yes | 0 |
| | after | 11, 7, 6, 1 | 89, 83, 26, 0 | none | yes | 198 |
| Two at 35,000 ft to the side (seed 1) | before | none | none | none | yes | 0 |
| | after | 15, 9 | 102, 83 | none | yes | 185 |
| Eight at 48,300 ft to the side, one in reach (seed 1) | after | none | none | none | yes | 0 |
| Eight at 46,000 ft to the side, two in reach (seed 1) | after | 4 and 4 (members 5 and 7) | none | none | yes | 0 |

Fire starts as before (103.1 s over the team, 102.7 s for the lone KS-19,
127.1 s for the lone KS-12). What changed:

- **The opening barrage is spread.** A lone gun's damage now falls on every
  member (a lone KS-19 over a four-ship: 10, 2, 2, 0 shells became 5, 3, 4, 2)
  and the lead's share drops further. Over the team each gun's barrage goes
  round the flight; total damage is about the same (seats 354 and 353).
- **The longest-reach trigger engages more groups.** A flight of four or
  fewer with one member inside a KS-19's reach is a quarter of the group, so
  the 35,000 ft passes that were held are now fired at, with about 190 hit
  points lost between the members and nobody destroyed. The hold now applies
  to larger groups with under a quarter in reach: eight aircraft grazing the
  edge with one in reach draw nothing (the trace shows "in-range 1/8
  holding"), with two in reach (25 percent) they are engaged.
- **Knock** still a quarter per jolt.

## Follow-up: a minimum of two (nr-flak3, 2026-10-14 evening)

John: "yeah let's add a minimum of 2 aircraft." The 25 percent rule also needs
at least two of the group in range of the team (the whole group when it is a
single aircraft); the heading-in rule is unchanged. Seed 1, the same probe;
"before" is the follow-up above.

| Case | Before | After |
| --- | --- | --- |
| Two-ship 43,500 ft to the side (only the lead ever in reach) | Engaged at 1 of 2 in range; 9 shells at the lead, 40 hit points | Held ("in-range 1/2 holding"); no shell |
| Four-ship 45,500 ft to the side (only one member ever in reach) | Engaged at 1 of 4; no shell fired before it left | Held ("in-range 1/4 holding"); no shell |
| Two-ship 35,000 ft to the side | Engaged at 1 of 2 in range; first shell 125.0 s; 24 shells, 185 hit points | Held while one is in reach, engaged when the second comes in (2 of 2); first shell 128.6 s; 23 shells, 167 hit points |
| Four-ship 35,000 ft to the side | Engaged at 1 of 4; 25 shells, 198 hit points | Held at 1 of 4, engaged at 2 of 4 a moment later; first shell 125.0 s, the same 25 shells and 198 hit points |
| Eight 48,300 ft to the side (one in reach) | Held | Held |
| Eight 46,000 ft to the side (two in reach) | Engaged; 8 shells over members 5 and 7 | The same |
| Four-ship (player and AI) over the team, heading in | 103.1 s, shells 18, 14, 9, 11, 395 hit points | Identical (heading-in) |
| Lone KS-19, two-ship | 102.7 s, 9 and 5 shells, 92 hit points | Identical (heading-in) |

## Not run

- A windowed or audio run; online play by eye. The team rules run on the host
  inside the surface tick and change nothing on the wire; the battery's
  network and replay scenarios stand for them.
