# Aircraft directional lighting

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. John requested corrected aircraft shading on 2026-09-18,
then warm, smooth shading and geometric shadows for all game objects, including
terrain. The shared [surface lighting and shadows spec](surface-lighting.md)
now defines smooth presentation. Stepped compatibility mode retains the
[imported light-map response](../formats/weather.md#original-per-normal-light-maps-continuation-2026-09-15).
Original brightness calibration and cast-shadow parity remain **unknown**.
