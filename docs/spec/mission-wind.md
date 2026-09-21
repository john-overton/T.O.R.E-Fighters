# Mission wind loading

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-21. Vietnam theater launches must accept their
signed wind headings. Loading a valid mission must not fail because its heading
is negative, regardless of aircraft count, launch altitude or separation.

The [weather format contract](../formats/weather.md#turbulence-generator-and-wind-line-contracts)
owns units, observed signed values, direction conversion and generated defaults.
Mission speed remains feet per second; explicit zero speed remains calm.
Signed heading is preserved through the existing binary-angle conversion, rather
than normalized through a different rounding rule or replaced with generated wind.

The host accepts heading -360 through +360 degrees inclusive and speed 0 through
200 ft/s inclusive. These bounds are a fitted input guard, not a recovered claim
about every value the original executable accepted. Values beyond either bound
fail with an error stating the accepted ranges and the supplied values.
Absent wind retains the existing generated default. The fix changes validation,
not wind forces, aircraft behavior, weather selection or flight adapters.

[Validation](../baselines/mission-wind.md) records the reproduced launch failure
and checks. No retail gameplay comparison is claimed.
