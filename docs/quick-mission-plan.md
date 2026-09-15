# Quick Mission Creator and Load Ordnance implementation plan

Requested 2026-09-14: schedule the full creator setup, faithful to Fighters
Anthology, with all recovered options represented and unsupported systems left
as explicit placeholders. First playable target: airborne BARCAP setup.
This document plans the work; it does not claim implementation or mission parity.
Scope expanded 2026-09-14: implement the Load Ordnance screen and connect supported
loadouts to the existing weapons service. It is no longer a deferred screen stub.
See the [ordnance review and implementation steps](ordnance-plan.md).

## Review findings

- `crates/tore-app/src/quick_mission.rs` owns the fitted briefing and two selectors.
  Its state stores aircraft/theater indices, not a mission configuration.
- `main.rs` handles aircraft/theater changes immediately and routes OK through
  `Action::FreeFlight`. Restart uses that action too. Launch resets flight,
  combat, input, interpolation and the clock; preserve those reset guarantees.
- `aircraft.rs::Airframe::start` chooses a terrain-safe airborne position at
  max(5,000 feet, terrain + 2,000 feet). The two reviewed player aircraft are
  F18.PT and RAFALE.PT. All 16 theater profiles already exist.
- Combat is a separate explicit manual range. Its target fixture and default
  stores are not a quick-mission generator or BARCAP objective implementation.
- Retail resources include QUIKMISS.DLG, QUICKB3–25.DLG, QUICK14.DLG,
  QM_MENU.MNU and QUICK.MT. Embedded dialog lists can be stale; the photo,
  executable tables and handler references must determine the active FA values.

See [research evidence](baselines/quick-mission-research.md).

## 1. Recover the active creator contract

Trace the hash-identified FA.EXE creator setup and selection handlers, using
FA.SMS only with explicit build/address verification. Record every control's
label, geometry, value order, default, bounds, dependencies and callback outcome.
Distinguish data read from assets, verified native behavior and fitted behavior.

Recover these complete field groups:

| Group | Required setup |
| --- | --- |
| Friendly forces | Nationality; three wings, each with count, skill and aircraft |
| Enemy forces | Nationality; three wings, each with count, skill and aircraft |
| Location and conditions | Theater, altitude/start condition, weather/time choices |
| Encounter | Advantage/neutral/disadvantage, separation distance |
| Armament | Standard/custom load; guns-only/guns-and-missiles restriction |
| Ground forces | Target type, AAA strength and SAM strength, including none |
| Menu and navigation | Help, Aircraft filters, OK, Cancel; next-screen flow |

Verify where airborne/runway/carrier starts and BARCAP/other mission types belong
in this edition. The supplied creator photo does not expose a mission-type field;
BARCAP appears in executable strings near other flight tasks. Do not invent a
retail creator dropdown from that evidence. If these choices belong downstream,
preserve that flow; identify any temporary development selector as authored.

The Aircraft top menu contains era/filter strings; it is not established as a
shortcut to the Wing 1 selector. Recover its actual tree and filtering rules.
Recover available aircraft as catalog entries separately from flyable identities.

**Gate:** every displayed option has a source/status entry; unresolved options
remain explicitly unresolved, rather than guessed from unused DLG strings.

## 2. Import bounded creator data

Extend `tore-formats` with narrow inert readers for the reviewed creator records
and menu tree. Reuse PE/PL bounds checks; never run imported handlers. Validate
relocations, strings, counts, indices and malformed/cyclic references. Recover
runtime-populated lists through a reviewed data contract, not generic string scans.

Keep CLI/app resource resolution shared, update cache requirements and import
provenance, and update `docs/formats/menu.md` and `coverage.md` as readers land.
Import original dialog/button/font resources, retaining QUIKMIS3's palette.
Synthetic fixtures cover malformed input; retail comparison artifacts stay local.

## 3. Add typed mission setup and capability validation

Introduce a renderer-independent setup value with stable IDs, three wings per
side and typed encounter/environment/armament settings. Keep it separate from
each aircraft model's immutable typed configuration and mutable flight state.

Maintain a draft while editing and construct a validated launch request on OK.
Each option is either operational, setup-only, or unavailable pending recovery.
Setup-only selections remain visible and retained, but cannot silently change
into a different playable mission. When a requested launch is unsupported, show
the specific missing capability and let the user revise the setup. Placeholders
must not spawn fake units or report objectives as completed.

Keep catalog availability separate from supported player flight. Aircraft with
no validated model can appear as clearly marked setup placeholders. Preserve
F18/RAFALE identity and separate model configuration; do not alias variants.
Retain setup across flight return/retry; decide disk persistence only after
recovering the retail behavior. Restart reuses the accepted launch request.

## 4. Complete the original setup UI

Replace hard-coded ghost text with the recovered fields, selectable dialogs and
draft values. Preserve the two-column briefing, original art/fonts/button pieces,
640×480 menu composition and measured glyph-based hit regions. Keep long labels
and all catalog entries reachable. Restore source sentences and punctuation;
current free-flight wording is development scaffolding.

Implement mouse/keyboard selection, scroll/rocker behavior where verified,
commit/cancel, focus restoration and nested Escape. Require matching press and
release, silence hover/focus, and play sounds only on actual activation/toggles.
Show concise availability feedback at the affected control or attempted launch,
without adding implementation terminology to the normal briefing.

Load Ordnance is included as a working screen: retail catalog/station cards,
category/page controls, fuel and weight, supported store editing and Fly.
Recover standard/custom branching and Select Plane return behavior before wiring
the creator transition. Unsupported store execution and Airbase systems retain
explicit placeholders; standard PT slots do not prove arbitrary loadout support.

## 5. Wire the supported airborne slice

Route creator OK through a validated mission launch, keeping direct CLI free
flight and the manual range available. Reuse aircraft/theater loading, input
reset, cockpit refresh, audio transition and fixed 120 Hz simulation plumbing.

- Wire the reviewed player identity and selected theater first.
- Add selected airborne altitude after confirming units/reference and defining
  explicit terrain-clearance validation. Report any required correction instead
  of silently displaying an altitude different from the launched state.
- Wire clear conditions supported by the current environment. Other weather/time
  choices remain setup placeholders until their renderer/physics effects agree.
- Represent BARCAP intent where source evidence places it. An airborne patrol
  shell may launch, but has no claimed enemy AI, objective/scoring or success
  behavior. Explicitly identify that limitation before launch.
- Connect supported weapon restrictions/default stores only after verifying the
  creator's standard-load contract for each aircraft. Do not enable the manual
  range fixture as a substitute mission or silently arm clean free flight.
- Pass accepted ordnance, ammunition and fuel into combat and the aircraft model;
  reset/retry must retain the accepted load instead of restoring PT defaults.
- Retain placeholders for additional wings/enemies, skill, relative placement,
  ground targets/defenses, runway/carrier starts and other mission behaviors.
  Apply nationality to simulation only when allegiance semantics are verified.

Read aircraft, theater and flight-controls specifications before their respective
changes. Do not infer runway validity from arbitrary terrain heights. No combat
AI is added by this creator pass; existing manual-acceptance gates remain.

## 6. Acceptance and documentation

- Check every field and every recovered option against its source matrix;
  verify defaults, filtering, zero-count wings and cancel/return behavior.
- Test launch validation, unsupported combinations, identity separation, exact
  accepted altitude, repeatable setup/restart and input release/pause isolation.
- Capture the base screen and every dialog/placeholder with deterministic setup;
  compare against retail captures. Missing native popup captures remain an open
  visual acceptance item. Inspect wide/tall windows for letterboxing/hit alignment.
- Run formatting, warnings-denied Clippy, workspace tests/build with `--locked`,
  Python tests and source/binary asset guards from `DEVELOPMENT.md`.
- Run creator and viewer window smokes, plus both aircraft launch/return/restart
  checks. Run both `--validate-flight` suites when launch/state integration changes.
  Record Linux evidence and any unavailable Windows/macOS runtime checks honestly.
- Update README/setup instructions, progress, format coverage and baseline
  evidence to describe exactly what works and what remains a placeholder.

**Delivery order:** creator/ordnance source matrix → bounded imports → typed
mission/loadout setup → both screens → armed airborne launch wiring → acceptance.
Full creator setup does not close M1d's
combat loop, M1e AI, or native mission-generation parity.

## Mapping checkpoint — 2026-09-14

See the [defaults and input contract](formats/quick-mission.md) and
[validation evidence](baselines/menu-behavior-mapping.md). Verified source rules
are recorded separately from unresolved behavior. Implementation and original-game
acceptance gates above remain open.
