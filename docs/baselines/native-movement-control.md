# Native normal-control and movement/contact components

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Measured evidence, research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature; see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


2026-09-15. Native source-backed diagnostic translations, using the same reviewed
FA executable/symbol hashes as [native flight research](../formats/native-flight.md).
This is component acceptance, not full-tick or retail trajectory acceptance.
The user confirmed that retail flight comparison is unavailable; implementation
continues using recovered contracts and synthetic expected results.

## Coverage

- Two primary-control tests: G/pitch/AoA coupling, roll units, low-speed scaling,
  release, negative commands, ground inhibition and invalid time. At 100 fps with
  a synthetic 200 fps stall reference, +9G authority reduces to +3G; a 121 deg/sec
  roll limit reduces to 30 whole deg/sec before conversion. Neutral release
  reaches 1G/zero pitch rate/zero roll rate in the stated one-second fixture.
- Four movement/contact tests: level and inverted gravity turn; spin/ground
  gates; display-bank isolation; repeated loops crossing both vertical attitudes
  in both directions; integrated-position touchdown, pre-contact display snapshot
  preservation and atomic invalid-time rejection. Sine fixtures are synthetic.
- The existing `native_departure` command now runs 7,200 force→movement snapshot
  pairs with each aircraft's own PT low-AoA parameters and imported sine/atan
  tables. Identical snapshot inputs replay identically. All eight F18/Rafale
  departure scenarios pass. Commands and source PT identities are in the
  [departure baseline](native-departure-stage.md). Local output:
  `.local/native-departure-stage/movement-probe.txt`.

Snapshot positions are not fed back into the departure scenarios. Primary-control
unit fixtures use explicit synthetic loaded limits. This does not establish that
native loading/damage/control producers or terrain/carrier queries are connected.
Contact tests supply explicit query outcomes; arbitrary theater heights are not
promoted to validated runways. No live simulation, rendering or adapter selection
changes, and no new fitted flight law is introduced.

## Remaining implementation

The subsequent [joined diagnostic](native-flight-diagnostic.md) closes the sampled
loading/damage consumers, auxiliary response, rudder/slip/steering, recovery-tick
skip and temporary rate/contact ordering gaps from this checkpoint. Event
execution, terrain/object/carrier queries and the full flight lifecycle remain
open before live native activation, separately from unavailable retail comparison.

## Validation

Passed formatting, workspace Clippy with warnings denied, workspace tests and
build with `--locked`, all 24 Python tool tests, repository/binary asset guards
and static extraction (107 symbol spans, 3,829 symbols, including the additional
reviewed region slices). Native-data probes pass for both PTs. Logs are under
`.local/native-departure-stage/checks/movement-*.txt`; extraction artifacts are
under `.local/native-movement-control/native/`. No retail derivatives are committed.
No new GPU/audio/controller checks were needed for these diagnostic changes;
Windows/macOS acceptance was not run.
