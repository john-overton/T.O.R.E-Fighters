# World edge

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Requested behavior

Implementation mode. John requested this on 2026-09-29. It is `opinionated`: the
retail game has no world edge that this project has traced. The specs and the
manual say only that the flight map draws theater edges dark. The distances are
agent decisions.

The ocean stays; the terrain height beyond the map is the edge value as before.
The **edge distance** is the distance in nautical miles (6,076.11549 ft) from the
aircraft to the nearest point of the map rectangle, `0..(columns - 1) * 8,192` by
`0..(rows - 1) * 8,192` ft. It is zero anywhere inside the map, and past a corner
it is the straight distance to that corner. This is an agent decision.

| Distance | Player | AI aircraft |
| --- | --- | --- |
| Under 100 nm | Nothing. | Nothing. |
| 100 nm and over | The cockpit message `You have left the theater: turn back now`, repeated every ten seconds while beyond 100 nm. No radio call: no existing channel fits. | Nothing. |
| 105 nm and over | The aircraft is lost through the ordinary destroyed path, cause `out of bounds` (message `Aircraft lost: out of bounds`). | Destroyed and despawned with no warning. |

An AI aircraft lost this way is a lost aircraft (a crash) for the debrief and the
objectives, and it is **not a kill for anyone**: the kill ledger removes it from
credit (`Ledger::lose_without_credit`) so an earlier shooter's damage does not
turn into a kill. It does not crash the ledger, which stays consistent. Overspeed and belly wear
losses use the same no-credit call (`AiWings::lose_uncredited`), and a lost player is never
credited to the last aircraft that hit it.
The AI probe's activity line names the cause.

The 105 nm loss stays fatal for a player with the Invulnerable cheat on, so a cheating player cannot fly off forever
(John, 2026-09-29), unlike the [overspeed](overspeed.md) loss, which Invulnerable skips.

The check runs once per simulation tick on the aircraft's position. Long AI
scenarios that used to leave the map now end cleanly with the aircraft lost.

## Tests

`edge_distance_nm` unit test in `terrain.rs`; the ledger test in `ledger.rs`; the
AI test `lose_out_of_bounds` in `ai_wings.rs`. Battery: `flight-edge-*` fly 200
seconds out over each edge of every theater at 20,000 ft and must stay flying
(about 30 nm out), and `flight-edge-lost-*` fly on for 1,250 seconds and must be
lost between 104.5 and 112 nm past the map with cause `out of bounds`.

## Numbers (agent decisions, 2026-09-29)

| Number | Value |
| --- | --- |
| Warning distance | 100 nm |
| Loss distance | 105 nm |
| Warning repeat | 10 s |
