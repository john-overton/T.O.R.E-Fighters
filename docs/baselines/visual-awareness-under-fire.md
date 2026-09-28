# Visual awareness and incoming-fire investigation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-28. The F-22/Su-27 complaint reproduces as a shared
engagement and self-defense gap. It is not explained by an F-22-specific visual
signature failure in the tested geometries. No gameplay code was changed.
The [proposed behavior](../spec/visual-awareness-under-fire.md) and
[delivery plan](../ROADMAP.md#visual-contact-and-incoming-fire-repair) are separate
from these measurements. Retail comparison was unavailable and no retail parity
is claimed.

## Build and method

Base: `7f4f79f95cfad05aaaf2f29ff6d007123ceb8e34`, game 0.1.1, Linux x86_64,
Rust 1.91.1, locked dependencies. The source worktree was clean at the start.
Media came from the existing ignored `.local/dev-profile` import.

Three unmodified CLI controls used `--ai-probe-ticks`, `--probe-attack`,
`--record-mission` and `--verify-render`. The standard probe initializes both
sides with the selected aircraft. It cannot select an independent opponent or
a rear attack, so the mixed-aircraft tests used a detached diagnostic worktree
at the same commit. Only the probe harness and a mutable bridge accessor were
extended, not sensors, engagement, defense, weapons or flight-model rules.

Diagnostic changes: select the enemy type in the creator draft, optionally
align enemy heading/velocity with the player for a rear attack, force the
scripted player to choose its gun, and print the enemy's actual observation
timestamps, neutral state, target, activity, defense and hit event every 30
ticks. Separate synthetic cases subtract one hit point or inject an anonymous
incoming tracer segment. None of these overrides is installed in the main
worktree or normal executable.

| Artifact | SHA-256 |
| --- | --- |
| Unmodified debug executable | `3e7c60167ce8df83aa5fed8a7898762e479091a3f43483850150c3c2a5cd3a9a` |
| Final diagnostic executable | `33f3a06945ef51a06bafc558c5c022d8f60b88207f84d73638cf96f2396eedab` |
| Diagnostic harness patch | `d7ab06877a102ef4586d8b9c41dc38f3d5addb90c66423802dea881bb6c24173` |

The diagnostic replay header records the base commit, not the harness patch.
Use the patch identity above when identifying these runs. The anonymous-tracer
fixture was added while the matrix ran; it is inactive in every matrix case.
The final patch includes both stages.

The main matrix is **84 runs**: F-22A, F-22N and F/A-XX as the player, against
all 14 selectable opponents, from the front and rear. Every run lasts 2,400
ticks, 20 seconds, starts at 1 NM and about 5,000 feet MSL/450 knots, with two
Experienced enemies and two Average friendly aircraft. Gun attacks begin at
tick 120 and repeat after the harness cooldown. Ukraine terrain and seed 1
are shared. Rear runs change the enemies' initial heading and velocity, not
their sensors or pilot skill.

The current airborne probe and AI actors use the legacy flight adapter, as
the replay header and source confirm. This is not validation of the default
player's researched adapter. No adapter default was changed.

## Results

| Player | Opponents | Head-on: initial visual observation / recognized gunfire / released | Rear: initial visual observation / recognized gunfire / released |
| --- | --- | --- | --- |
| F-22A | 14 | 14 / 14 / 14 | 0 / 0 / 0 |
| F-22N | 14 | 14 / 14 / 14 | 0 / 0 / 0 |
| F/A-XX | 14 | 14 / 14 / 14 | 0 / 0 / 0 |

All 84 runs actually emitted player gun rounds. None entered Defending or
Evading in response to those gun attacks. Head-on aircraft pursued after
recognizing the shot. Rear aircraft remained unresponsive to the attack.
There were no aircraft-specific exceptions in this matrix.

Exact opponents: F18.PT (F/A-18D), RAFALE.PT (Rafale C), F14.PT (F-14D),
A4E.PT, X31.PT, MIG29.PT, SU27.PT, MIG21.PT, SU25.PT, MIG23.PT, SU35.PT,
F22.PT, F22N.PT and the separate `faxx` identity. The F/A-XX is an opinionated
variant using F-22N source resources, not a substitution for another aircraft.

### Su-27 evidence

- **No fire, head-on:** both Su-27s had current visual and radar observations
  of the F-22 from simulation tick 0. They stayed neutral, with no offensive
  target. The replay explains this as holding formation, self-defense only.
- **Forced gun, head-on:** first recorded round at tick 122, 1.017 seconds,
  range 4,519 feet. Attack reports were recorded that tick, leader release at
  tick 123, and targeting the player at tick 124. The Su-27 pursued rather than
  jinking. This proves acquisition and engagement can work against an F-22.
- **Normal weapon selection, head-on:** the player eventually used its gun;
  the leader released at 2.033 seconds. No missile was fired in this case, so
  it is not a test of Su-27 response to an F-22 missile launch.
- **Forced gun, rear:** 85 rounds in 20 seconds; no current observation of the
  player at startup, no attack report, engagement release or defensive activity.
  A resolved round passed within **28 feet** according to the replay's shot
  summary. No gun hit was credited. This is a near-miss failure, not a claim
  that a real damaging hit was ignored in that particular run.
- **Non-stealth control:** an F/A-18D firing from behind produced 64 rounds,
  a **34-foot** closest pass, and the same absence of attack/defense response.
- **Anonymous damage:** subtracting one Su-27 hit point at tick 120 produced
  `FrameEvent::Hit`, visible in the controller trace. With no offensive target,
  it remained neutral/in formation and never entered evasion during the run.

Two additional anonymous-tracer fixtures inject an approaching segment 1,118
feet from the Su-27, 26.6 degrees below/ahead of its nose, with an initial
predicted closest approach of about 197 feet. Projectile ages are 1 and 60
ticks. Both generate no attack report or defensive activity. The fresh case
isolates the missing shooter association; the older case also encounters the
departure-age cutoff. These are synthetic incoming-fire perception probes,
not simulated AAA launches. No functioning ground gunner or SAM engagement was
run. Ground-source firing AI remains outside this investigation.

### Source explanation

The measured results agree with these current paths:

| Path | Consequence |
| --- | --- |
| `tore-sim::ai::awareness::visual_eligible` and `AiActor::observe` | Pilot visual acquisition is independent of radar signature, but restricted to the existing forward cone. There is no side/rear lookout. |
| `AiMission` neutral leader release and mission selection | Seeing a hostile does not release a neutral wing. Only a perceived attack or accepted order does. This is the documented startup policy, not failed visual detection. |
| `tore-app::ai_wings::report_perceived_attacks` | Gun reports require a tracer no older than 30 ticks, inside the visual cone, plus exactly one independently observed hostile aircraft close to its departure point. Anonymous or ground-origin fire cannot get through that aircraft-identification requirement. |
| `AiActor::update_missile_defense` | The target-independent defense service consumes missile threats, not gun trajectories or anonymous damage. Recognizing a gun attack releases combat but does not itself request a jink. |
| `Controller::choose_maneuver` | The no-target/no-bearing returns precede the Hit/Evade tactic branch. A hit can reach the controller without reaching that evasion branch. |

The proposal addresses the three separate questions: can the pilot see it,
may the leader engage it, and must the aircraft defend itself? Weakening F-22
radar stealth would not repair those gates.

## Reproduction and local evidence

All recordings, JSONL exports, summaries, raw logs, harness source, patch and
matrix analysis remain ignored under `.local/f22-awareness-review/`.
`matrix-results.json` contains one row per matrix/control case;
`run_matrix.py` and `analyze.py` reproduce the local batch and summary.

Unmodified control:

```sh
TORE_DATA_DIR="$PWD/.local/dev-profile" target/debug/tore-app \
  --ai-probe-ticks 7200 --aircraft f22 --separation 2 \
  --probe-attack 120:5 --record-mission .local/f22-control-new.tore-replay \
  --verify-render --no-audio
```

Su-27 rear case, using the local diagnostic build and a new recording path:

```sh
TORE_DATA_DIR="$PWD/.local/dev-profile" TORE_REVIEW_ENEMY=su27 \
  TORE_REVIEW_REAR=1 TORE_REVIEW_GUN=1 \
  .local/f22-awareness-review/probe-target/debug/tore-app \
  --ai-probe-ticks 2400 --aircraft f22 --separation 1 \
  --probe-attack 120:5 --record-mission .local/su27-rear-new.tore-replay \
  --verify-render --no-audio
```

Use [recording exports and captures](../REPLAYS.md) to inspect a run.
`su27-sees-f22-holds.png` was captured from the head-on no-fire recording at
tick 120 and visually inspected. It shows the Su-27 and its actual AI thought
panel: holding formation, no target, self-defense only. Sensor-source evidence
comes from the diagnostic observation trace, not from the picture alone.

## Validation and limits

There are **94 recorded runs**: the 84-case matrix, three original CLI controls,
four additional matrix controls, two tracer fixtures and one repeated Su-27
run. All 94 passed `--verify-render`. The repeated Su-27 head-on gun run matched
all 21 shared state checksums, all compared event families and recorded aircraft
states within format precision.

Required repository checks passed: formatting, workspace Clippy with warnings
denied, build, **1,803 Rust tests with 8 existing ignored**, **84 Python tests**,
source and both executable asset scans, documentation checks and diff whitespace.
The replay GPU capture succeeded. No renderer code changed.

The scripted player does not fight or avoid a head-on collision. Several
head-on runs therefore end in collision, which must not be read as a gun kill
or successful defensive behavior. The matrix establishes recognition and
response gates, not dogfight effectiveness or balanced kill rates. It covers
the Experienced enemy setting, daylight/clear-air perception and these start
geometries, not every skill, distance, weather state or adapter. Missile defense
already exists and passed the repository suite, but this pass did not run a
complete imported SAM/AAA or missile matchup matrix. Those remain acceptance
work in the proposed plan.
