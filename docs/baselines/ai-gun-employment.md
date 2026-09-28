# AI gun-employment investigation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-28, on `7589bcaaa5d29d98c8eb25e700bc37e17608878d` in
`fix/visual-awareness-under-fire`. No gameplay changes in this investigation.
The [behavior contract](../spec/ai-gun-employment.md) and subsequent
[implementation results](ai-gun-implementation.md) are separate from this evidence.
This measures the rebuild, not original-game or real-aircraft gun behavior.

## Player-session evidence

The completed recording `2026-09-28_1512_UKR_F22.tore-replay` was read from the
user's normal replay library. SHA-256:
`329535918b16366d4435c49a9f788163082d51719f1aab4ff30d618d0c13a2dd`.
Its header identifies build 7589bca, Ukraine, clear conditions, F-22 player,
two Average Su-27 enemies and the researched player adapter. Duration is
18,234 ticks, 151.95 seconds. Both enemies' missile stations show empty in
the recorded thinking panels, so this is not a missile-preference explanation.

| Aircraft | Two-round group start times | Weapon-service expiry | Later recorded usable gun |
| --- | --- | --- | --- |
| Enemy 1-1 | 11.508 s and 17.508 s | 32.508 s | First at 44.425 s; 498 thinking samples while expired |
| Enemy 1-2 | 11.008 s and 17.008 s | 32.008 s | First at 36.758 s; 744 thinking samples while expired |

Both retain the player as their target until the recording ends. Neither fires
again after 17.5 seconds. They repeatedly return to a current firing solution
with a nonempty, usable GSh-301, including inside 0.5 NM, while the weapon phase
remains `Window expired`. The freeze lasts approximately the final two minutes.

The eight enemy rounds all miss. Initial releases occur at 1.9 to 2.2 NM;
the later pairs at about 0.2 and 0.4 NM. Recorded off-boresight angles reach
32 to 33 degrees on the second wingman's later pair. Closest approaches range
from 109 to 698 feet. This establishes poor outcomes in this encounter, not a
general accuracy measurement. The player fires 86 gun rounds and also has no
credited hits.

## Causal checks

1. `weapon_service::WeaponService::advance` turns a failed 15-second preparation
   window into `State::WindowExpired`. With any target still present, that state
   returns `WindowExpired` forever. A usable station, restored lock or changed
   target ID cannot reopen it. Only `target: None` returns it to search. The
   source describes this as a fitted consequence of an unresolved original rule.
2. Imported AI guns use `StationSpec::projectile_count = 1` and debit one source
   representative group. The app queues `actual_rounds_per_game * projectiles`
   physical rounds. For the reviewed canonical guns, this means two bullets
   per authorization. The physical scheduler correctly spaces those bullets;
   it does not authorize a sustained burst.
3. After a release, the weapon service reports unresolved reload pacing. The
   controller's `Fallback::BurstPacing` creates a new service, which starts
   preparation again. In this recording the pairs are six seconds apart,
   including the gun's one-second tracking delay. The configured gun pacing
   does not drive the continuation. This differs from the shared
   [32-round-per-second physical gun cadence](../spec/damage-smoke.md#individual-cannon-rounds).
4. `AiWings::realise` constructs gun direction from the current observed target
   position minus aircraft position. It does not use a fixed barrel axis or
   lead the target's observed velocity. The round is unguided after release.
   Store-envelope acceptance is therefore not proof of a physically aligned
   gun solution. Increasing burst volume alone would retain this aiming defect.

Improved visual attention can keep a target continuously observed, allowing the
old timeout trap to persist instead of incidentally resetting on contact loss.
That relationship follows from the service's reset condition; it is not a
claim that the earlier detection behavior provided correct gun employment.

## Reproductions and limits

A media-free Rust harness calls the current service with all 14 exact selectable
aircraft identities. Each starts with a target but no station, expires at tick
1,800, then receives valid target/station/lock/path inputs through tick 7,199.
It switches to another valid target at tick 3,600. **All 14 fire zero times**
during these 45 seconds of valid inputs. A one-tick no-target interval at tick
7,200 recovers all 14; their first subsequent release is tick 7,801. This is a
controlled service test, not an imported flight or weapon-performance matrix.

Two unmodified 14,400-tick imported probes use a Su-27 opponent, rear geometry
and 1 NM separation, with F-22 and F/A-18D players. The Su-27 leader reaches
`Window expired` at 20.508 seconds in both. These probes use default mixed stores
and do not reproduce the player's maneuvers or loadout. Their remaining shots
come from friendly AI wings, so their lack of enemy gun releases is supporting
evidence only. Both pass recording reconstruction and direct AI attitude checks.

All exports, logs and the synthetic harness remain ignored under
`.local/ai-gun-review/`. `player-session/summary.txt` and `log.jsonl` carry the
measured timeline; `timeout-results.txt` contains the 14-profile experiment.
No original executable was run. Original expiry recovery, AI burst duty cycle
and AI lead policy remain unmeasured. The proposed correction labels its new
values as agent choices rather than attributing them to the original game.

Validation: formatting, workspace Clippy with warnings denied, build,
1,818 Rust tests (8 existing ignored), 84 Python tests, source and both executable
asset guards, documentation headers and diff whitespace all passed. These tests
establish the current build remains intact; the existing expiry test does not
assert recovery when valid firing inputs return. No runtime or rendering code
was changed, and no new display smoke was needed for this research pass.
