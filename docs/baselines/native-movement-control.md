# Native normal-control and movement/contact components

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

Normal-control loading/damage producers, auxiliary rate response, full rudder/slip
and ground steering, event dispatch, terrain/object/carrier queries and the whole
flight lifecycle. Departure dispatch must still skip normal controls on a spin
recovery tick. Contact rate updates must propagate through the native temporary
rate add/subtract caller. These gaps prevent live native activation; they are
separate from the unavailable retail comparison.

## Validation

Passed formatting, workspace Clippy with warnings denied, workspace tests and
build with `--locked`, all 24 Python tool tests, repository/binary asset guards
and static extraction (107 symbol spans, 3,829 symbols, including the additional
reviewed region slices). Native-data probes pass for both PTs. Logs are under
`.local/native-departure-stage/checks/movement-*.txt`; extraction artifacts are
under `.local/native-movement-control/native/`. No retail derivatives are committed.
No new GPU/audio/controller checks were needed for these diagnostic changes;
Windows/macOS acceptance was not run.
