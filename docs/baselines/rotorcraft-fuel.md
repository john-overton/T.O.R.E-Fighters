# Rotorcraft fuel burn

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Measured evidence for [the rotorcraft fuel rule](../FLIGHT-MODEL.md#rotorcraft-fuel)
(slice HF2, 2026-10-13, John's requests 14, 15 and 16 on the morning list). Taken on
branch `nr-hf2` off multiplayer d03d74ba, Linux (Ryzen 9 7900X), release build of the
example, debug build of the app. Retail comparison is unavailable: retail never made a
rotorcraft fuel-limited. The record evidence and the reasoning for the Mi-24 are in
[the aircraft records](../formats/aircraft.md#rotorcraft-fuel-records).

## How the numbers were taken

```sh
cargo run --release --locked -p tore-sim --example powered_probe -- PATH/FA_2.LIB/AH64.PT PATH/FA_2.LIB/MI24.PT PATH/FA_2.LIB/CH47.PT PATH/FA_2.LIB/V22.PT
```

Each aircraft's `== fuel` block trims a hover and level flights at 60, 100 and 130 knots
at 1,000 ft at the PT's gross weight (empty plus full internal fuel) and prints the
flow the model burns and the tank over it. The PT tank is the record's.

## Flow and endurance

| | AH-64 | Mi-24 | CH-47 | V-22 |
| --- | --- | --- | --- | --- |
| Tank, lb (PT) | 2,000 | 3,307 | 3,307 | 2,000 |
| Record flow, lb/s (no longer read) | 1 | 3 | 1 | 1 |
| Tank at the record's flow | 33 min | 18 min | 55 min | 33 min |
| Rated power, hp (the rotor model's) | 3,668 | 3,315 | 3,001 | 5,122 |
| Full flow at rated power, lb/s | 0.611 | 0.553 | 0.500 | 0.854 |
| Hover: share of rated power, flow lb/s, endurance | 70 %, 0.470, 71 min | 72 %, 0.431, 128 min | 63 %, 0.354, 156 min | 52 %, 0.536, 62 min |
| 60 kt level | 41 %, 0.329, 101 min | 36 %, 0.276, 200 min | 34 %, 0.242, 228 min | 25 %, 0.353, 94 min |
| 100 kt level | 47 %, 0.360, 93 min | 36 %, 0.278, 199 min | 42 %, 0.275, 201 min | 24 %, 0.348, 96 min |
| 130 kt level | 70 %, 0.470, 71 min | 49 %, 0.334, 165 min | 60 %, 0.343, 160 min | 30 %, 0.390, 85 min |
| Planning flow (the AI's and the crew voice's), lb/s | 0.467 | 0.429 | 0.344 | 0.534 |

Against real aircraft (public figures, not re-derived here): the Mi-24 carries 1,500 to
1,720 kg internally (3,307 lb is 1,500 kg) and flies about two hours; the AH-64D's
combat endurance is 2.5 hours at about 1,000 to 1,200 lb an hour (0.3 lb/s). The model's
Mi-24 is inside two hours at a hover and three hours at 100 knots; the AH-64 burns more
than the real one (its PT tank is 2,000 lb and its rated power comes from the PT
thrust).

## Headless flight

`tore-app --headless-flight 7200 --aircraft mi24 --maneuver hover` (a minute of hands-off
hover at 5,000 ft): fuel 3,307 to 3,280.4 lb, 0.44 lb/s (before: 180 lb in the minute,
3 lb/s). The `flight-powered-hover-<aircraft>` scenarios now assert 0.1 to 0.9 lb/s in
their 15 seconds for the four rotorcraft.

## Tests and battery

- Unit tests: `flight::powered::fuel` (flow by power, hover weight),
  `flight::powered::fuel_tests` (a hover burns the rotor's draw and not the record's
  flow, hover against cruise against idle, the Mi-24 about two hours, every rotorcraft
  over 45 minutes at a hover, planning at a steady hover flow, the jets unchanged),
  `ai::route::tests` (the five minute reserve; home in range),
  `ai_wings::tests::a_helicopter_has_a_fuel_schedule_at_its_own_cruise_and_goes_home_on_bingo`
  (caution above and bingo below the time home plus 300 s).
- Scenarios: `ai-fight-{ch47,v22,av8,yak141,mi24,ah64}-friendly-wings` (rotorcraft plan
  at least an hour, 100 minutes, two hours and 50 minutes at the start),
  `ai-fight-heli-fuel-bingo-ah64` (now 250 lb), `flight-powered-*` (27, with the fuel
  band in the hover).
- `python3 tools/battery.py --changed d03d74ba --budget 900 --jobs 2 --with-windows no`:
  163 of 163 passed.
- Single-player guard against `guard-base-a4dadf76` (the base d03d74ba matches): SAME 51,
  DIFFERENT 1. The one difference is `text/roster-7200.txt`: the 16 rotorcraft rows
  (V-22, AH-64, Mi-24, CH-47 at four skills) move in the third decimal of `peak_roll` or
  `peak_turn` (AH-64 68.503 to 68.516, V-22 29.760 to 29.759, Mi-24 67.767 to 67.764) and
  still PASS. That is the intended change: in the 7,200-tick roster run the rotorcraft
  now burn a fraction of the fuel and so stay heavier. No fixed-wing line, recording
  or tape moved.
