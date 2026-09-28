# Visual contact and incoming-fire implementation validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-28. Built in the separate
`fix/visual-awareness-under-fire` worktree from
`bcf85fbb826080d61a3ce86ff16662185c6ca2c3`, with the uncommitted implementation
changes. Linux x86_64, Rust 1.91.1 and locked dependencies. The code implements
the [authored behavior contract](../spec/visual-awareness-under-fire.md), not
recovered retail timing. The [earlier investigation](visual-awareness-under-fire.md)
remains evidence of the previous behavior.

## Delivered behavior

The pilot lookout observes side/rear aircraft without weakening radar stealth.
AI leaders may release neutral members on current hostile contacts permitted by
their assignment. Human-led wings still require orders, hold prevents offensive
fire, recall prevents contact-only release, and an automatic release preserves
an individual's accepted target assignment.

Gun trajectory observations, anonymous close passes and actual weapon-hit
events can request defense without an offensive target. The incoming-fire
service owns measured samples and a bounded cue, not a shooter identity. It
shares the existing flight-limited jink/dive safety rules and compares urgency
with missile defense. Gun-only evidence does not dispense devices. The same
lookout now supplies AI passive-missile visual geometry; electronic warning and
player RWR rules remain unchanged.

Headless probes gained exact opponent, geometry, skill, gun and adapter controls,
air/ground threat fixtures and a complete encounter batch. Replay explanations
separate observations, permission and incoming-fire evidence. Investigation also
found that the old headless probe drew AI with the level-flight fixture pose.
The probe now enables real AI attitudes and checks them against simulated body
axes before checking recording reconstruction. Normal live AI already used real
attitudes; straight-flight fixture mode remains available.

## Imported encounter matrix

The accepted batch contains **1,008 recordings**:

- Player: F22.PT, F22N.PT and the separate F/A-XX runtime identity.
- Opponent: all 14 selectable identities, including F18.PT as F/A-18D and
  RAFALE.PT as Rafale C. Runtime identity, not donor asset stem, names files.
- Resolved enemy skill: Novice, Average, Experienced and Ace.
- Initial enemy heading: head-on, side-on and same-heading rear approach.
- Flight adapter: explicitly legacy and researched, for player and AI.

Each run has two enemies and two friendly AI aircraft, starts at 1 NM and about
5,000 feet MSL/450 knots over Ukraine, and lasts 360 ticks, three seconds.
Mission policy is free engagement. The accepted executable's SHA-256 is
`eda14300efab305cc1f730694ea44c5e71799cdcf2199c533cacbc8946e11645`.
The replay header records the base commit, not the uncommitted diff.

| Measurement | Result |
| --- | --- |
| Enemy aircraft with a current visual observation of the player | 2,016 of 2,016 |
| Latest first visual observation | Tick 80, 0.667 seconds |
| Enemy aircraft released to engage under their free-engagement assignment | 2,016 of 2,016 |
| Body-attitude and recording reconstruction verification | 1,008 of 1,008 passed |
| Incomplete recordings in the accepted directory | None |

This is acquisition and engagement-permission acceptance, not a three-second
dogfight score. It does not require every aircraft to have a weapon solution or
fire before the run ends.

## Incoming-fire response

The imported Su-27 victim runs cover all four skills in both flight adapters:
one anonymous hit, one rear air-gun trajectory, one ground-origin gun trajectory
and a matched no-threat control. All **32 recordings** run for 1,200 ticks,
ten seconds, with weapons hold and the fixture at simulation tick 120.

| Fixture | First defensive event in recording | Behavior |
| --- | --- | --- |
| One hit point and weapon-hit event | Tick 121, 1.008 seconds | Jink with no target; clears after exactly two seconds without new evidence |
| Rear gun round | Tick 122, 1.017 seconds | Two observed positions establish an incoming trajectory; a later close pass refreshes the cue |
| Ground-origin gun round | Tick 202, 1.683 seconds | Lookout observes the approaching round; later close-pass evidence refreshes it |
| No threat | None | Remains in formation |

These event ticks are the same across the four skills and two adapters. They
measure response after permitted evidence, not an immediate warning at an
unseen launch. Gun/hit cases have no offensive target and dispense no devices.
Sampled peak bank is **58.1 to 59.9 degrees**, versus **2.2 to 3.1 degrees** in
the no-threat controls. All 32 pass reconstruction and direct body-attitude
checks. The ground fixture uses an imported gun with documented fitted launch
geometry, not an autonomous AAA actor or a recovered surface gun profile.

Synthetic coverage includes:

- All 14 airframes, four skills and both adapters, **112 hit-response cases**:
  a targetless aircraft under weapons hold requests defense, physically banks,
  consumes no devices, stays above the test terrain and returns to duty.
- Paired threatened/unthreatened trajectories in both adapters: the flown gun
  break banks over 10 degrees and increases closest clearance by more than
  25 feet. This demonstrates a physical response without claiming every shot
  can be evaded.
- Exact predicted-time, predicted-distance, proximity and expiry boundaries;
  two-sample visual evidence, sight loss, harmless/receding rounds, terrain
  masking, own ordnance, burst continuity and low-altitude/unsafe-speed guards.
- Front/rear/side/above/below and inverted lookout fixtures; frozen attention,
  range limits, all skills, recall, weapons hold and assigned-target restrictions.
- A controlled ground-supported radar missile plus an anonymous hit: one motion
  response, valid radar countermeasures and no fabricated shooter target. A
  rear passive-missile fixture detects its motion through lookout without using
  its hidden target ID or identifying the launcher. Existing supported, active,
  infrared, passive, failed-receiver and compatibility tests also pass.

## Reproduction and artifacts

Use a new output directory:

```sh
TORE_DATA_DIR=/path/to/dev-profile cargo run --locked -p tore-app -- \
  --ai-probe-ticks 360 --separation 1 --probe-matrix .local/contact-matrix \
  --verify-render --no-audio
```

The individual [probe commands and fixture constants](../DEVELOPMENT.md#visual-contact-and-incoming-fire-probes)
are maintained in the development guide. For a ground-shot replay, select
F-22 versus Su-27, rear geometry, weapons hold, `--probe-threat 120:aaa` and
`--probe-flight-model researched`.

Local evidence is under `.local/ai-awareness-validation/` in this worktree:
`accepted-matrix/`, `accepted-matrix.log`, `accepted-matrix-results.json`,
`accepted-fire/` with its JSONL exports/results, and check logs. The Su-27
ground-fire replay was GPU-captured at tick 360 and visually inspected. Original
art, imported data, screenshots and recordings stay ignored.

## Checks and limits

All required checks passed: formatting, workspace Clippy with warnings denied,
workspace build, **1,818 Rust tests with 8 existing ignored**, **84 Python
tests**, source and both executable asset guards, documentation headers and diff
whitespace. The display smoke test and replay capture passed on this Linux host.
Two additional researched Su-27 ground-shot runs matched all 11 shared exact
state checksums, all compared event families and recorded aircraft states within
format precision. Both passed body-attitude and reconstruction verification.
The final inspected capture is `su27-defense-final.png`.

Normal flight-adapter and weapon compatibility defaults are unchanged. The
three-second matrix is clear-air close-contact coverage; it is not full weather,
cockpit-blind-sector, long-duration balance or retail-parity acceptance.
Terrain safety and flight limits are exercised by fixtures and the existing
controller suite. Surface firing AI, original timing research, complete weather
visibility and human encounter-balance review remain separate work. Windows and
macOS runtime were not run on this Linux host.
