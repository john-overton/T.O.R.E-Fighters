# AC-130 gunsight validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Measured evidence, implementation mode.** A record of what was run and what
> it produced for the AC-130 gunsight project (John, 2026-10-09). Retail has no
> AC-130 gunsight, so nothing here is a parity claim. The behaviour is
> specified in [AC-130 linked guns](../spec/ac130-linked-guns.md) and
> [the target window](../spec/target-window.md#ac-130-gunsight).

2026-10-09, Linux x86_64, debug build of branch `ac130-gunsight` (the slices S1
to S8 and this validation pass, on multiplayer `ccbe8f5d`), Rust 1.91.1,
imported profile `.local/mp-variety-profile`. Targeted checks only: the full
battery was not run, by John's rule for this project.

## Gunsight probes (`--gunsight-probe`)

Three scripted runs on flat ground through the live combat state with the
imported AC-130 guns (`crates/tore-app/src/gunsight_probe.rs`). Each is a
battery scenario in the flight lane (`flight-ac130-pin-orbit`,
`flight-ac130-fire-no-target`, `flight-ac130-track-out-of-arc`) and takes about
half a second. All three pass. `--gunsight-dump PATH` writes the orbit's gun
train every 12 ticks as CSV.

| Probe | Setup | Result |
| --- | --- | --- |
| `pin-orbit` | Left orbit at 4,000 ft, 6,000 ft radius, 300 ft/s (25 degrees of bank); all three guns linked; pin the ground at the orbit's centre; fire from 4 s to 14 s, run to 22 s so the slow 105 rounds land | Pin held for the whole orbit; 319 rounds (300 of the 25, 17 of the 40, 2 of the 105; 720 before the fire-rate pass), all 319 landed, all within 54.3 ft of the pin (mean 30.8 ft); no barrel moved faster than 30 degrees a second (biggest step 0.2500 degrees); every linked gun READY on 6,480 of 6,480 gun-ticks from the first round on; never NO LINE OF FIRE |
| `fire-no-target` | Level at 3,000 ft, default view, nothing held; fire for 6 s | Sight stays in free slew at (-90, -25); all three guns fire 180, 10 and 1 rounds in 6 s (1,800, 100 and 10 a minute; 192, 192 and 48 in 6 s, 432 rounds, before the fire-rate pass); a trigger tapped ten times a second fires 60, 10 and 1; 126 ground impacts within 69.2 ft of the aim point 6,420 ft away (mean 38.2 ft). Loads 3,000, 1,000 and 500 |
| `track-out-of-arc` | Ground object 5,000 ft abeam tracked; fly past for 35 s; trigger held for the last 4 s (so the rounds do not destroy the object) | Object ends 154.6 degrees aft of the nose; all three guns read CANNOT BEAR with the barrels at their arc limits (-150, -135, -115 degrees, to under 0.5 degree); the sight still tracks the object and the camera stays on it (worst error 0.02 degree during the pass); 128 rounds fired while CANNOT BEAR (288 before the fire-rate pass) |

The fire-rate pass (John, 2026-10-09; [rates and muzzle velocities](../spec/ac130-linked-guns.md#fire-rates-muzzle-velocities-and-loads))
cut the round counts and left the impact error where it was, with the 105 now
flying at 1,620 ft/s and the 25 at 3,450 ft/s. Retail's generic record fired
the three guns at 1,920, 1,920 and 480 rounds a minute.

The checks and their thresholds (150 ft of the pin, 95 percent READY) are agent
choices. The AC-130's guns leave no crater marks, so impacts are read from the
ground explosions the combat state shows. Round dispersion is the fitted
0.25-degree cone, which is why impacts spread by tens of feet.

## Pose evidence

- **Animation probe sight poses.** `--aircraft ac130 --animation-probe DIR`
  now also writes `ac130-sight-poses.ppm` (a contact sheet, one row a pose),
  `.csv` and one OBJ per pose: all three guns at the default view's
  (-90, -25), then each gun at the four corners of its own arc. 0 required or
  check failures. The sim's own-airframe check (`gunship::clear_airframe`)
  accepts the default view for every gun and rejects all four C_25 corners,
  C_40 at +45 degrees of elevation, and C_105 at its forward upper corner, the
  extremes that would point through the wing or nacelles. Sheet:
  `.local/tmp-s9/ac130-sight/`.
- **GPU captures through `tools/agent-run.sh`** with `--sight-preview`
  (`--flight-view 1 --flight-look 90,-12 --flight-zoom 2`): the left belly at
  the default view, forward, aft, both arc edges and past the arc, showing the
  barrels train and park. Sheet: `.local/tmp-s9/renders/sheet-sight-poses.png`.
  The target cam page in the same captures shows the gunsight page.
- **Orbit dump.** `.local/tmp-s9/orbit.csv`: the barrels reach the pin's
  direction in under half a second of the first tick and then follow it as the
  aircraft orbits.

## Multiplayer

| Check | Result |
| --- | --- |
| `TORE_LOOPBACK_SECONDS=20 cargo test --locked -p tore-session --test loopback` (real UDP sockets, a host and two bots) | pass, 20.6 s |
| `client::sight_tests`: a second player sees the barrels follow the sight at 80 ms clean and at 300 ms with 5 percent loss; host train, gunner readout and watcher barrels agree within 0.03; predicted camera clamps and corrections | 11 gunsight tests in tore-session pass (with the wire, readout and journal tests: golden at protocol 21, round trip, fuzz, standby replay bit for bit) |

A multiplayer battery scenario with a bot flying the AC-130 was not added: the
bot does not fly it. A human check on the three-platform test is open.

## Check list

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean |
| `cargo test --workspace --locked -- --test-threads=6` | all pass (tore-app 1,551; tore-sim 1,467; tore-world 674; tore-session 486 with 26 ignored by design) |
| `python3 -m unittest discover -s tools -p 'test_*.py'` | 326 pass |
| `tools/check_assets.py` (source, `tore-app`, `tore-extract`) and `tools/check_docs.py` | pass |
| `cargo build --locked --workspace --bins`, then `tools/battery.py --changed ccbe8f5d --jobs 4 --with-windows no` | 50 of 50 scenarios pass, 323 s, including the three AC-130 scenarios and the net lane |
| Single-player guard (`.local/mp-baseline/quick.sh` against `mp-95d402a0`) | SAME 36, DIFFERENT 16, MISSING 0: exactly the known kill-credit differences (every shared state checksum matches; only hit, kill and shot-outcome text differs) |

## Not run, and still open

- The full battery (John's rule for this project), and any windowed battery
  scenario.
- A real orbit under the flight model: the pin-orbit probe poses the aircraft
  kinematically on a circle, so the sight is exercised against a steady bank
  but not against the flight model's own wobble.
- Hand flying: key feel (slew rates, the quarter-rate nudge, zoom steps) is
  unreviewed. From renders John approved the LCOS pipper, its range-arc scale,
  diamonds for the other linked guns, the crosshair ticks and the hand-drawn
  GIMBAL LIMIT eyeball (2026-10-09).
- Retail comparison is unavailable.
