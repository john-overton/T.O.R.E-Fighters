# Night fire and explosion lighting

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-13. Worktree `T.O.R.E-Fighters-night-lighting`,
branch `night-effects-lighting`, based on multiplayer commit `82316757`.
The lighting is fitted presentation for John's request, not measured retail
parity. The constants and behavior live in the
[night glow spec](../spec/explosions.md#night-glow).

## Scope and regression coverage

Fire light grows with area, with stronger local illumination than the previous
size curve. Fading fires keep their source height and dim to zero. A visible
fireball keeps lighting surfaces after its initial flash without counting that
flash twice. The close glow preserves flame artwork by suppressing the point
core when the sprite is resolved. The simulation, networking, replay format
and flight adapters are unchanged.

The synthetic `night_effect_lights_follow_fire_size_fade_and_fireball_lifetime`
test checks a 90-foot fire's intensity, half-opacity and extinction, a fixed
source height, and an explosion at one second, 1.5 seconds and extinction.
It also checks that a dud supplies no sustained light. The `night_glow` tests
cover size scaling, range, light limits and distant visibility.

## Reproduction

From the worktree, after `cargo build --locked -p tore-app`:

```sh
TORE_PREVIEW_THEATER=EGY TORE_CITY_FILL=0 TORE_DATA_DIR=.local/dev-profile target/debug/tore-app --impact-preview .local/night-final night-range
python3 tools/battery.py --scenario render-night-effect-lights --profile .local/dev-profile --out .local/night-battery --windows 1
```

The preview renders offscreen without opening a window. The battery uses the
agent wrapper to reserve its GPU run. The 24-, 90- and 160-foot comparison
images each put the fire on the left and a single resting, burning flare on
the right at the same camera distance. The range images show 250-, 500- and
2,000-pound explosions with increasing fire sizes behind them. The one-second
image is after the tracked initial flashes have ended.

Captures use user-owned imported media and remain ignored under `.local/`.
No retail derivatives belong in Git. These are prepared renderer fixtures,
not screenshots from a live multiplayer session. Windows, macOS and retail
comparison were not run in this Linux validation pass.

## Validation result

Passed on Linux: formatting, workspace Clippy, 467 Python tests, source asset
and documentation checks, the app build, five night-glow unit tests, the
sustained-effect-light regression, and the production GPU flare/surface-light
test. The new `render-night-effect-lights` battery scenario passed and wrote
15 PNGs; the close 90-foot comparison and one-second explosion row were
visually reviewed. Selected final captures are in `.local/night-final/`.

The broad workspace test run was stopped when John requested focused testing.
Its app suite had already passed: 1,845 tests, with 34 explicitly ignored.
The full workspace test/build checklist is therefore not claimed complete.
The updated app was launched from this worktree with `.local/dev-profile` and
moved to workspace 1 for John's review. John approved the lighting and
authorized committing and merging it into multiplayer on 2026-10-13.
