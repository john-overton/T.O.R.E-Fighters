# Documentation realignment — 2026-09-15

A single pass to make the repository's documentation say what the project
actually is: a rebuild targeting **1:1 gameplay parity by expression of feature**,
not a reconstruction of the original program's code.

## Why this was needed

Earlier today John said "replicate only native functionality". He meant the
game's player-visible behaviour. The agent read "native" as the repository's
provenance label — byte-faithful reconstruction of the original executable's
control flow — and built eighteen plan revisions on that reading. Documentation
across the repo had hardened that misreading into acceptance gates: features
could not be considered done until their native code path was recovered and
connected, and authored behaviour was treated as a debt to be repaid.

This pass corrects the documentation so the misreading cannot happen again. **No
code, tests or build configuration were changed.**

## The strategy being applied

Stated in [AGENTS.md](../AGENTS.md) and recorded in
[the parity plan](parity-plan.md):

1. Parity is measured by expression of feature — what a player experiences.
2. "Native" is a provenance label only, never a requirement or a gate.
3. Ground, terrain and object contact is reclassified as **opinionated**.
4. Research mode produces prose specs under `docs/spec/`; implementation mode
   reads a spec and builds the behaviour idiomatically.
5. `spec-derived` is the default provenance for gameplay code. Fitted and
   opinionated components need no replacement before acceptance.

### The D27 label — resolved 2026-09-16 as D30

The task named this strategy "D27 in docs/native-environment-systems-plan.md".
That decision ID was already taken. The repository's actual D27 reads:

> | D27 / 2026-09-15 | Implementation choice: NE-00.1p scheduler/clock ownership | Reuse existing clock translations, extend only reviewed shift widths and preserve load-before-due and merge tie ordering. The fixed 120 Hz host bridge remains authored; no live scheduler replacement |

The decision log in that plan runs D01–D29 and contains no entry recording parity
by expression of feature, contact as opinionated, or specs under `docs/spec/`.
`docs/spec/` did not exist. The strategy above was therefore taken from the task
instruction itself, not read out of the repository.

The strategy was first written up as D27, which briefly gave two decisions one
ID. **On 2026-09-16 it was renumbered D30** — the next free ID — which leaves the
scheduler decision and every existing link untouched. References to "D27" below
are historical: read them as D30.

## 1. Inventory

Every tracked Markdown file, read in full before classification. 105 files at the
start of the pass; 108 after this one.

`docs/native-environment-pm.md` is **not** in this table: it is excluded through
`.git/info/exclude` and is Jeeves control state, not repository documentation. It
is discussed under unresolved conflicts below.

### Root and top-level docs

| Path | Purpose | Classification | Conflicted with D27 |
| --- | --- | --- | --- |
| `AGENTS.md` | Authoritative agent rules | agent rules | **Yes** — made native provenance the work target and tracked "runtime connected" as a completion column |
| `CLAUDE.md` | Pointer plus reporting-style preference | agent rules | No |
| `README.md` | Project front page | other | **Yes** — described the project through native-recovery progress; had accreted 20 dated paragraphs |
| `MODS.md` | GPL/modding policy: what counts as a mod, who owns it, retail-asset rules | other (policy) | No |
| `THIRD_PARTY_NOTICES.md` | Attribution for the blast-derived DCL decoder | other (legal) | No |
| `docs/ROADMAP.md` | Milestones M0–M6, principles, the 1:1 definition | roadmap or plan | **Yes** — current-priority block pointed at native recovery; the AI VM question left open |
| `docs/ARCHITECTURE.md` | Module-by-module map of what each crate and file owns | other (design notes) | Framing only — plus a stale workspace table listing one crate of six |
| `docs/DEVELOPMENT.md` | Setup, toolchain, everyday checks, CLI flag and diagnostic reference | agent rules (handbook) | **Yes** — "runtime connection" named as a completion milestone |
| `docs/EXTRACTION.md` | How to run the extractor: profiles, filters, reports, caps | format reference | Minor — research-mode statements needing scope |
| `docs/INPUT.md` | Contract for the controller/input layer | spec | Minor — authored effects called "provisional" |
| `docs/FLIGHT-CONTROLS.md` | Player-facing binding table, HUD/look/mirror behaviour | spec | **Yes** — "not accepted native flight/system parity" |
| `docs/FLIGHT-MODEL.md` | Flight kernel contract, adapters, telemetry, model ownership | spec | **Yes** — listed what is "still required to claim native parity" |
| `docs/behavior-provenance.md` | Provenance categories and acceptance rules | agent rules | **Yes** — the root of the misreading. A four-step native ladder was the acceptance model |
| `docs/aircraft-import.md` | Per-aircraft playbook, gates A–F, coverage table | roadmap or plan (playbook) | **Yes** — "exact whole-tick native parity is open"; no vocabulary for spec-derived |
| `docs/REFERENCES.md` | Where the user's media and reference checkouts live | format reference | No |
| `docs/CHEAT-CODES.md` | Four original FA cheat-key sequences | baseline evidence | No |
| `docs/progress.md` | Dated append-only substep log plus milestone checklist | roadmap or plan + revision log | **Yes** — "replace fitted adapter only after trajectory acceptance" |
| `docs/flight-response-plan.md` | Ordered work plan and gates for G/roll/rudder/departure response | roadmap or plan | **Yes** — native runtime ownership and contact as an activation gate |
| `docs/weather-plan.md` | Nine-step weather sequence with dated checkpoints | roadmap or plan + research archive | **Yes** — "Replace authored distance fog with the native recovered visibility and horizon rules" |
| `docs/ordnance-plan.md` | Load Ordnance screen description, research, build steps | roadmap or plan + spec | **Yes** — screen behaviour gated on handler recovery |
| `docs/quick-mission-plan.md` | Creator build plan: field groups, imports, UI, launch wiring | roadmap or plan | **Yes** — options held "unavailable pending recovery" |
| `docs/menu-parity-matrix.md` | Status ledger of creator/ordnance controls | research archive (tracker) | **Yes** — "**Mapped** means static source evidence exists"; measures parity by source recovery |
| `docs/parity-plan.md` | **New.** One-page parity status and next feature | roadmap or plan | n/a |
| `docs/spec/README.md` | **New.** What a behaviour spec is and how to write one | spec | n/a |
| `docs/doc-realignment-2026-09-15.md` | **New.** This report | other | n/a |

### `docs/formats/` — recovered format and code facts

All 13 are research material to be preserved. A conflict here means the file
stated a **policy**, not that its facts are wrong.

| Path | Purpose | Classification | Conflicted with D27 |
| --- | --- | --- | --- |
| `formats/aircraft.md` | F18/RAFALE PT, SH, cockpit and instrument facts plus adapter status | research archive | **Yes** — a "Next parity gates" list |
| `formats/coverage.md` | Per-format import/decode status matrix | research archive | No |
| `formats/menu.md` | Menu resource inventory, EALIB/DCL/PIC/DLG/MNU/PCM rules, button layout | format reference | No |
| `formats/music.md` | Recorded-PCM decision, MUS opcode grammar, playlists, cue mapping | format reference | Minor — a player-audible cue gated on non-fitted contact |
| `formats/native-flight.md` | Static decode of FA flight code, pass-by-pass ledger | research archive | **Yes** — "Remaining implementation/acceptance" column |
| `formats/native-land-contact.md` | Ground-contact contracts: queries, cache, cell arithmetic, offsets | research archive | **Yes** — "It must not be replaced with the adapter's eight-foot clearance" |
| `formats/native-strip.md` | STRIP/runway ledger: callbacks, placement, scheduler, events, speech | research archive | **Yes** — "No runway is runtime-connected"; "Live contact stays gated" |
| `formats/objects-and-shapes.md` | SH shape/object structure, LOD, encodings, investigation workflow | format reference | Minor — "Recover native branch conditions/order before claiming software visibility parity" |
| `formats/ordnance-menu.md` | Load Ordnance source contract: dispatch, geometry, eligibility, fuel rules | research archive | Minor — one acceptance clause |
| `formats/quick-mission.md` | Creator option tables, theater/nationality maps, DLG geometry, defaults | research archive | **Yes** — "Wire verified data into state and rendering before claiming screen parity" |
| `formats/theater.md` | T2/BIT2 layout, tmap placement, MM fields, LAY palettes, sky inventory | format reference | No |
| `formats/weapons.md` | Weapons scope contract, exporter audit, ordered W0–W5 plan with gates | roadmap or plan | **Yes** — roughly half the file is scheduling and acceptance policy |
| `formats/weather.md` | FA clock, LAY weather records, visibility numbers, turbulence, vapor | research archive | Minor — wording only |

### `docs/baselines/` — measured evidence

69 files. All preserved unchanged apart from a scope header. "Yes" marks
research-mode gate language that a reader could mistake for a product gate.

| Path | Purpose | Classification | Conflicted |
| --- | --- | --- | --- |
| `all-theaters.md` | All-16-theater extraction counts, disc inventory, font follow-up | baseline evidence | No |
| `audio.md` | Recorded-PCM extraction and NORMAL music slice acceptance | baseline evidence | No |
| `banked-pull-aoa.md` | Banked-pull AoA fix, fitted nose-target formula, probe numbers | baseline evidence | **Yes** |
| `cockpit-controls.md` | Full-canvas cockpit/HUD/FMENUD menu checkpoint, Metal checks | baseline evidence | No |
| `cockpit-slide.md` | Sliding flat cockpit and zoom presentation, user-requested | baseline evidence | No |
| `combat-components.md` | Combat exporter counts, native weapon arithmetic components | baseline evidence | **Yes** |
| `creator-ordnance.md` | Quick Mission Creator and Load Ordnance delivered behaviour | baseline evidence | No |
| `directional-cockpit.md` | Aircraft-fixed cockpit/HUD plane pass, captures, frame timing | baseline evidence | No |
| `environment.md` | macOS M3 dev host, toolchain, first workspace check results | baseline evidence | No |
| `f18-animations.md` | F/A-18D exterior moving-part rig, source evidence vs fitted motion | baseline evidence | No |
| `f18-free-flight.md` | First F/A-18D free-flight slice, extraction hashes, commands | baseline evidence | No |
| `flight-performance.md` | Frame-time diagnosis and fixes, plus later appended perf samples | baseline evidence | No |
| `flight-response.md` | Adapter producer/response/departure pass, G/spin/loop measurements | baseline evidence | **Yes** |
| `flight-response-sky.md` | Momentum, vertical flight, sky-pole and cockpit fixes, loop probe | baseline evidence | No |
| `hud-bank-scale.md` | User-requested graphical HUD bank arc | baseline evidence | No |
| `input.md` | Controller input acceptance plus rumble/editor/combat follow-ups | baseline evidence | No |
| `instrument-layouts.md` | Large/small instrument window coordinates and toggle | baseline evidence | No |
| `linux-setup.md` | Linux host setup, SIGSEGV shutdown fix, GPU-selection follow-up | baseline evidence | No |
| `live-fire.md` | Two-aircraft live-fire range, fidelity limits, HUD appendices | baseline evidence | No |
| `look-around.md` | Cockpit head-look and exterior orbit behaviour, key evidence | baseline evidence | Research-mode, scoped |
| `main-menu.md` | Main-menu import/render result, media census, decoder comparison | baseline evidence | No |
| `manual-weapons.md` | Manual weapons: readiness, damage classes, tapes, validation | baseline evidence | Research-mode, scoped |
| `menu-behavior-mapping.md` | Static creator/ordnance behaviour mapping results | research archive | No |
| `menu-contract-pass.md` | `--native-menus` research tooling, menu-tree reads, addresses | research archive | No |
| `menu-options-geometry.md` | Creator dispatch/theater/geometry tables, dialog parse results | research archive | No |
| `mirrors.md` | Live rear-view mirrors, fitted optics, presentation measurements | baseline evidence | No |
| `native-departure-stage.md` | Joined native departure/tumble/spin diagnostic stage | research archive | **Yes** |
| `native-flight-diagnostic.md` | 21-case joined native flight service diagnostic | research archive | **Yes** |
| `native-flight.md` | Five-pass static native-flight research log | research archive | **Yes** |
| `native-land-angles.md` | NE-00.1c contact angle arithmetic, 332,800 probe cases | research archive | **Yes** |
| `native-land-foundation.md` | NE-00.1a land-contact predicates, STRIP/T2/RUNWAY hashes | research archive | No |
| `native-land-geometry.md` | NE-00.1b vertical land geometry, sqrt table, 166,400 cases | research archive | **Yes** |
| `native-live-flight.md` | Restricted `--native-flight-tables` airborne mode and acceptance | baseline evidence | **Yes** |
| `native-movement-control.md` | Native normal-control, movement/contact component tests | research archive | **Yes** |
| `native-strip.md` | STRIP source regions, RUNWAY.SH box metadata, PIC extraction | research archive | **Yes** |
| `native-strip-accounting.md` | NE-00.1q trailing events and death-statistics ledger | research archive | No |
| `native-strip-clock.md` | NE-00.1p scheduler/frame-clock ownership, clock helper tests | research archive | No |
| `native-strip-commands.md` | NE-00.1l initial-command/default-event regions | research archive | No |
| `native-strip-definition.md` | NE-01.1a STRIP.OT definition reader | research archive | No |
| `native-strip-events.md` | NE-00.1k static-object event-service and enqueue ledger | research archive | No |
| `native-strip-hit.md` | NE-00.1o collision hit dispatch and death marking | research archive | No |
| `native-strip-lifecycle.md` | NE-00.1e candidate lifetime, template blob, list tests | research archive | **Yes** |
| `native-strip-movement.md` | NE-00.1m stationary movement dispatch, approach angle | research archive | No |
| `native-strip-observer.md` | NE-00.1n queue routing and speech observer ledger | research archive | No |
| `native-strip-ownership.md` | NE-00.1h airport and callback ownership ledger | research archive | No |
| `native-strip-placement.md` | NE-00.1f placement conversion and nationality remap | research archive | No |
| `native-strip-record.md` | NE-01.1b isolated first UKR.MM STRIP record decode | research archive | **Yes** |
| `native-strip-removal.md` | NE-00.1r removal caller and notification exclusions | research archive | No |
| `native-strip-service.md` | NE-00.1g service dispatcher, kind-0 delay, RNG dependency | research archive | **Yes** |
| `native-strip-slots.md` | NE-00.1i airport slot/attachment producers | research archive | **Yes** |
| `native-strip-speech.md` | NE-00.1j current-object switches and speech delay arithmetic | research archive | No |
| `native-tumble.md` | Initial native tumble component translation, diagnostic | research archive | **Yes** |
| `ordnance-research.md` | Load Ordnance planning: screenshot, HARDLoad addresses | research archive | No |
| `quick-mission-research.md` | Creator planning evidence: dialog art, EXE offsets, limits | research archive | No |
| `rafale-animations.md` | Rafale cockpit-selection fix and moving-part rig | baseline evidence | Research-mode, scoped |
| `rafale-quick-mission.md` | Rafale C import identity, Quick Mission screen, validation | baseline evidence | No |
| `responsive-flight-ui.md` | Responsive flight overlay/instrument composition, capture sizes | baseline evidence | No |
| `shape-reference-review.md` | Review of the user-supplied Plurry SH documentation | research archive | Research-mode, scoped |
| `shared-flight-model.md` | Shared `tore-sim` hybrid model acceptance, 26 scenarios | baseline evidence | **Yes** |
| `ukraine-viewer.md` | First terrain viewer slice: T2/MM parse, camera, counts | baseline evidence | No |
| `weapons-research.md` | Weapons extraction and static audit, catalog counts | research archive | No |
| `weapons-systems.md` | ECM, subsystem damage, player damage, controller chords | baseline evidence | **Yes** |
| `weather-cameras.md` | Per-camera weather, sun-whiteout cheat, paired perf | baseline evidence | No |
| `weather-foundation.md` | The full weather implementation log, eight appended checkpoints | baseline evidence | No |
| `weather.md` | First weather implementation evidence, superseded | baseline evidence | No |
| `weather-research.md` | Static weather clock/turbulence evidence, withdrawn claim | research archive | No |
| `weather-review.md` | Eleven-commit audit, corrected defects, SH-effects inspection | other (code audit) | **Yes** (retail-comparison gate) |
| `weather-smoothing.md` | User-requested smooth shading, fitted celestial scale factor | baseline evidence | No |
| `wind-turbulence-vapor.md` | Wind defaults, turbulence gate, vapor attachment acceptance | baseline evidence | No |

### Filenames that do not describe their contents

Found while reading; not fixed in this pass, because renaming breaks inbound
links and the constraint was to preserve evidence files as they are.

- `baselines/environment.md` — reads as game environment; is the macOS dev-host
  and toolchain baseline.
- `baselines/native-flight.md` — a research log sharing a filename with the
  actual contract file `formats/native-flight.md`.
- `baselines/weather-foundation.md` — not a foundation slice; the entire weather
  implementation history, 514 lines.
- `baselines/weather.md` — reads as the weather baseline; is the superseded first
  pass. The current record is `weather-foundation.md`.
- `baselines/live-fire.md` — last two sections are unrelated HUD/render changes.
- `baselines/flight-performance.md` and `baselines/input.md` — both carry
  appended notes belonging to later, unrelated passes.

## 2. Files rewritten

| File | What changed |
| --- | --- |
| `AGENTS.md` | Rewritten from scratch. Was 53 paragraph-length bullets accreted over the project's life, mixing rules with status. Now 168 lines in the required order: project intent, the two modes of work, terms, the spec granularity test, provenance categories, standing constraints, documentation rules, development checks. Under the 200-line limit. |
| `CLAUDE.md` | Reduced to a one-line pointer. |
| `README.md` | Rewritten. Was 202 lines, roughly 20 of them dated feature paragraphs appended one pass at a time, describing the project through native-recovery progress. Now leads with the project intent as AGENTS.md states it, points at the roadmap, the parity plan and `docs/spec/`, and consolidates the feature paragraphs into one "What works today" section. Every command and flag in it was checked against the flags actually parsed in `crates/`. |
| `docs/behavior-provenance.md` | Rewritten. This file was the root of the misreading. |
| `docs/ROADMAP.md` | Milestones M0–M6 unchanged in intent. Preamble and the "What 1:1 means" section rewritten. |
| `docs/FLIGHT-MODEL.md` | 8 targeted edits. |
| `docs/FLIGHT-CONTROLS.md` | 2 targeted edits. |
| `docs/DEVELOPMENT.md` | 2 targeted edits. Every command left byte-identical. |
| `docs/aircraft-import.md` | 5 targeted edits. |
| `docs/ARCHITECTURE.md` | 5 edits, including a factual fix: the workspace table listed one crate of six. |
| `docs/INPUT.md` | 1 edit. |
| `docs/EXTRACTION.md` | 3 edits. |

## 3. Files created

| File | Why |
| --- | --- |
| `docs/parity-plan.md` | The second of the two planning documents. One page: the D27 decision, spec status, what is built, what is next, and what was frozen. |
| `docs/spec/README.md` | `docs/spec/` did not exist. Defines what a behaviour spec is, the granularity test, and the shape of a spec file. No specs are written yet. |
| `docs/doc-realignment-2026-09-15.md` | This report. |

## 4. Files moved

Seven planning documents became frozen archives under `docs/research/`. Each got a
one-line header stating it is frozen as of 2026-09-15 and superseded by D27, and
each kept its revision record intact. Their internal relative links were deepened
one level and all inbound links across the repo were repointed.

| From | To | Why |
| --- | --- | --- |
| `docs/native-environment-systems-plan.md` | `docs/research/` | Named in the task. 717 lines, v21, decision log D01–D29, the plan the misreading was built into. |
| `docs/progress.md` | `docs/research/` | An append-only per-substep log — 334 lines added and 22 deleted across the last 25 commits that touched it. Exactly the revision log the new rules forbid. Its evidence links are its value. |
| `docs/weather-plan.md` | `docs/research/` | Half research archive: address lists, LAY inventories, six dated checkpoints. Half sequencing now covered by the roadmap and parity plan. |
| `docs/flight-response-plan.md` | `docs/research/` | Sequencing now covered; kept for the turbulence routine evidence and the authored rumble mapping. |
| `docs/ordnance-plan.md` | `docs/research/` | Kept for the Load Ordnance screen description and `HARDLoad` research. Its screen-specification table is the best raw material in the repo for a first behaviour spec. |
| `docs/quick-mission-plan.md` | `docs/research/` | Kept for the creator field research and resource inventory. |
| `docs/menu-parity-matrix.md` | `docs/research/` | Measured parity by source recovery: "**Mapped** means static source evidence exists". That is the wrong axis under D27. |

## 5. Files deleted

**None.** The constraint was to preserve all research and evidence: move or
freeze, never delete. Nothing was removed from the repository in this pass.

## 6. Scope headers added

Contradictions inside `docs/formats/` and `docs/baselines/` are almost all
correct statements about **research mode** that read as product gates. The
constraint was not to rewrite those files beyond adding scope headers, so every
file in both directories got one, immediately under its title.

- **13 files in `docs/formats/`** — "Research notes — research mode. … Requirements, gates and remaining work described here are research-mode scope; they are not acceptance gates for gameplay."
- **69 files in `docs/baselines/`** — "Measured evidence — research mode. … Provenance labels and any remaining gates named here are research-mode scope; they are not acceptance gates for gameplay."

Both headers point at `AGENTS.md` for the parity definition and at `docs/spec/`
for player-visible behaviour. No other content in either directory was touched.

## 7. Contradictions fixed, before and after

### `AGENTS.md`

Before:

> Track native work as separate **source established**, **translated/tested**, **runtime connected**, and **retail compared** steps. Diagnostic helpers, imported assets, fitted substitutes and passing replay tests do not close native runtime/parity gates.

After:

> **Native** — a code path reconstructed from the original executable's control flow. This is a **provenance label only**. It is never a requirement, an acceptance gate, or a reason to block or revert working behaviour.

Before:

> Current implementation focus is native FA features. Do not invent flight laws or effects to fill gaps in native research, or add fitted gameplay behavior merely because it seems plausible. Recover the contract or document it as unknown.

After (split across the two modes, so the no-guessing rule survives where it is correct):

> Research mode … Do not guess. A missing fact is recorded as unknown with the next research step, never filled in with something plausible.
>
> Implementation mode … If the spec is missing a number you need, that is a research task. Say so, choose a documented value, and label it `fitted` or `opinionated`.

Before:

> Current scheduled order (2026-09-15): after the restricted F18/Rafale native airborne connection, the next environment/systems continuation is governed by [the living native environment/systems plan] …

After: removed from the rules file entirely. Sequencing now lives in
`docs/parity-plan.md`; AGENTS.md carries rules, not status.

Also added, verbatim as instructed:

> When John says "match the original", "do what the game does", or "native functionality", he means **original behaviour**, not native provenance.
>
> If an instruction is ambiguous between the two, assume original behaviour and say so in your report.

### `CLAUDE.md`

Before:

> Follow `/AGENTS.md`
>
> Summarize in plain English for smart product manager: short sentences, no jargon or acronyms without a quick definition, and lead with what it means for me, not how it works. Do not patronize.

After:

> ``Follow [AGENTS.md](AGENTS.md).``

**Nothing was lost:** the reporting preference moved into the AGENTS.md standing
constraints as "Summarize in plain English for a smart product manager: short
sentences, no unexplained jargon, lead with what it means rather than how it
works."

### `docs/behavior-provenance.md`

The sharpest sentence in the repository blocking the new strategy. Before:

> - Keep native tasks and fitted/user-directed tasks separately labeled. A fitted substitute cannot close a native checklist item.

After:

> - Research-mode labels and implementation status are tracked separately. A fitted substitute does not close a *research* item; it can perfectly well close a gameplay one.

Before (the `Fitted` category defined by deficit, and required to document its own debt):

> | Fitted | An implementation approximation we authored because a native contract is missing or because of host integration | Exact rule/constants, reason, known difference or uncertainty, runtime scope, and the native work it cannot close |

After:

> | Fitted | An implementation approximation authored because a behaviour is not yet specified, or because the host needs something the original did not have | Exact rule/constants, reason, known difference or uncertainty, and runtime scope |

Before (the four-step ladder that was the acceptance model, step 3 named "Runtime connected", step 4 requiring retail comparison):

> ## Native implementation steps
>
> Track these independently; a later step must not be implied by an earlier one:
>
> 1. **Source established:** … 2. **Translated and tested:** … 3. **Runtime connected:** … 4. **Retail compared:** …

After:

> ## Research-mode recovery steps
>
> These steps apply **in research mode only** … They are **not** completion columns for a gameplay feature — a gameplay feature is complete when it matches its spec, whatever its provenance.
>
> 1. **Source established:** … 2. **Translated and tested:** … 3. **Specified:** write the player-visible behaviour and its numbers into `spec/`. This is where research ends and implementation begins.
>
> Two former columns are retired. **"Runtime connected"** described wiring a translated code path into the running game; under parity by expression of feature, implementation works from the spec instead, so it is no longer tracked for gameplay.

Before:

> - Current work focuses on native feature recovery. Do not add new fitted flight laws or gameplay effects to fill unknown behavior.

After:

> - Work from specs. When a spec is missing a number, that is a research task: say so, choose a documented value, and label the component `fitted` or `opinionated`. Do not invent a *feature* Fighters Anthology does not have.

Contact reclassified, in the examples table. Before, contact appeared only inside
a row ending:

> … explicit turbulence bypass and adapted clock/device/fuel producers, no contact/lifecycle or retail acceptance

After, contact has its own row:

> | Ground, terrain and object contact | **Opinionated** | Reclassified 2026-09-15. Contact behaviour is authored to match what a player experiences on a runway and deck; it is no longer waiting on a recovered native producer |

`spec-derived` added as the first category and named the default:

> | **Spec-derived** (default) | Implemented from a behaviour spec in `spec/` that describes what the player experiences | The spec file, the numbers tested against, and anything the spec left unknown |

### `docs/ROADMAP.md`

Before:

> Original terrain and environment systems will be recovered from retail assets and native behavior

After:

> Original terrain and environment systems are rebuilt from retail assets and from the game's observed behaviour

Before:

> Native/fitted/runtime/retail evidence remains separate under [behavior provenance](behavior-provenance.md). Retail flight comparison is unavailable and does not block source-backed progress.

After:

> Where a behaviour came from is recorded per component under [behavior provenance](behavior-provenance.md); no provenance label is an acceptance gate. Retail comparison is unavailable and does not block progress.

The 1:1 definition gained an explicit statement of the parity axis:

> Parity is measured **by expression of feature**: the player must experience what they experience in Fighters Anthology. It is not a recreation of the original program's code, control flow or internal structure.

and a fourth out-of-scope line:

> - The original program's control flow, call ordering, caches and RNG ordering

Before:

> Open decision (see bottom): whether the retail AI VM is reimplemented from the recovered bytecode or the behaviors are recreated from observation. This is decided in M0 and shapes M1e.

After:

> AI behavior is recreated from a behaviour spec, like every other feature; the retail AI bytecode VM is not reimplemented. This follows from parity by expression of feature and shapes M1e.

**This is an agent decision derived from D27, not John's.** It is listed under
unresolved conflicts below.

### `docs/FLIGHT-MODEL.md`

Before:

> To claim native parity still requires original trajectory comparison, terrain/object collision geometry and cache producers, carrier/arresting dynamics, remaining damage/equipment state, exact integer update scheduling and RNG consumption order.

After:

> Behavior a player would still find missing includes carrier and arresting-gear dynamics and the remaining damage/equipment state. Terrain and object contact is authored rather than recovered, and exact integer update scheduling and RNG consumption order are original implementation details rather than parity targets.

Before:

> This boundary is intentional until source airfield/object collision mapping is ported.

After:

> That boundary is a deliberate design choice (opinionated), not a hold waiting on recovered source airfield/object collision mapping; ground, terrain and object contact was reclassified opinionated on 2026-09-15.

Before:

> Terrain contact stops the research flight and environmental turbulence is disabled. Native query producers, engine/device/fuel/damage lifecycles, setup refresh cadence, event execution and scheduler/RNG parity remain open.

After:

> Terrain contact stops this restricted research path and environmental turbulence is disabled inside it; ordinary free flight has working authored ground contact. Native query producers, engine/device/fuel/damage lifecycles, setup refresh cadence and event execution remain open research items; scheduler and RNG ordering are original implementation details, not parity targets.

Before:

> It retains explicitly authored clock/device/fuel boundaries and stops at unsupported contact.

After:

> Inside that research path only, the clock, device and fuel boundaries are explicitly authored and the flight stops at unsupported contact; ordinary free flight has working authored ground contact.

Before:

> … airborne acceptance does not close the remaining lifecycle/contact gates.

After:

> … Those labels describe origin; none of them is an acceptance gate. Lifecycle and contact work inside the research path continues.

### `docs/FLIGHT-CONTROLS.md`

Before:

> This is still a development flight adapter, not accepted native flight/system parity.

After:

> This is still a development flight adapter; [behaviour provenance](behavior-provenance.md) records which flight and system components are spec-derived, native, fitted or opinionated.

Before:

> Reaching terrain contact pauses with an explicit unsupported-contact message; restart resets the native state.

After (the fact kept, the scope made explicit):

> Reaching terrain contact pauses with an explicit unsupported-contact message; restart resets the native state. That limit belongs to this research option alone: ordinary free flight has working authored ground contact.

### `docs/DEVELOPMENT.md`

Before:

> Apply [behavior provenance](behavior-provenance.md) when interpreting results: native source, translation tests, runtime connection and retail comparison are separate milestones.

After:

> Apply [behavior provenance](behavior-provenance.md) when interpreting results. In research mode, identifying the source, testing the translation and writing the spec are separate steps. "Runtime connection" is retired as a completion column, and retail comparison is unavailable.

Before:

> Contact stops the run; environmental turbulence is unavailable.

After:

> Two limits belong to this restricted research path only: contact stops the run, and environmental turbulence is unavailable here.

### `docs/aircraft-import.md`

Before:

> Keep [native, fitted and user-directed provenance](behavior-provenance.md) separate.

After:

> Keep [spec-derived, native, fitted and opinionated provenance](behavior-provenance.md) separate.

Before:

> Fitted response/coupling remains; exact whole-tick native parity is open.

After:

> Response and coupling components are fitted and acceptable as shipped; whole-tick native reconstruction is research, not an acceptance bar.

Before:

> Restricted airborne native coupling is tested; contact/lifecycle producers are next under the environment/systems plan.

After:

> Restricted airborne native coupling is tested. Ground, terrain and object contact is opinionated authored behaviour, not a pending native producer; lifecycle work is sequenced in [the parity plan](parity-plan.md).

Before:

> aircraft calibration and configuration remain independently owned and explicitly source-based or fitted.

After:

> aircraft calibration and configuration remain independently owned, each component carrying its own provenance label (spec-derived, native, fitted or opinionated).

Before:

> Use the terms **source reviewed**, **extraction supported**, **headless flight supported**, **rendered flight supported**, **systems partially supported**, and **retail validated for named cases** independently.

After:

> Use the terms **source reviewed**, **extraction supported**, **headless flight supported**, **rendered flight supported** and **systems partially supported** independently. Add **retail validated for named cases** only where that evidence exists; retail comparison is currently unavailable. These describe support, not origin: a spec-derived, fitted or opinionated component can be fully supported.

### `docs/ARCHITECTURE.md`

A factual fix. Before:

> | Workspace | `crates/tore-app` | Application entry point and desktop shell |

After:

> | Workspace | `crates/tore-app`, `tore-formats`, `tore-extract`, `tore-sim`, `tore-input`, `tore-input-native` | Desktop shell and entry point, plus the format, extraction, simulation and input crates |

Before:

> The M0 environment supports the first M1a menu slice, partial M1b all-theater renderer and partial M1c Hornet free flight.

After:

> The M0 environment supports the M1a menu slice, the M1b renderer across all 16 theaters, and M1c free flight in two aircraft (F/A-18D and Rafale C) plus a development weapons range.

Before:

> this is not a complete native combat tick.

After:

> the service reproduces combat behaviour rather than reconstructing the original executable's combat tick.

Before:

> This is combat-service determinism, not native scheduler or full application replay parity.

After:

> This is combat-service determinism: it reproduces combat state, not a full application replay.

### `docs/INPUT.md`

Before:

> These are provisional tactile designs, not recovered native effects or directional flight-stick forces.

After:

> These are opinionated tactile designs, chosen by the implementation rather than recovered from the original. They are not directional flight-stick forces.

### `docs/EXTRACTION.md`

Before:

> STRIP callback/box metadata is partly recovered; full initialization, shape and contact closure remain unresolved.

After:

> STRIP callback/box metadata is partly recovered; in research mode, full initialization, shape and contact closure remain unresolved. Ground, terrain and object contact in the game is opinionated authored behaviour and does not wait on that research.

Before:

> Dynamic aircraft producers are identified rather than replaced with guessed lists.

After:

> In research mode, dynamic aircraft producers are identified rather than replaced with guessed lists.

Before:

> NE-01/02 in the [living environment/systems plan] define the next full discovery/catalog/import pass

After:

> NE-01/02 in the [frozen environment/systems plan] describe a full discovery/catalog/import pass … That archive is kept for its recovered facts; sequencing lives in [the parity plan](parity-plan.md).

### Patterns searched for that do not exist in this repository

The task named several phrases to hunt. Searched with whitespace normalised, so
hard-wrapped sentences were matched across line breaks. Results:

- **"native source recovery is the implementation specification"** — one hit, in
  `native-environment-systems-plan.md`, now frozen. Left intact inside the
  archive; the freeze header supersedes it.
- **"reference knowledge alone does not establish native behavior"** — one hit,
  same frozen file, as "Reference naming patterns, screenshots and real-world
  aircraft knowledge alone do not establish native gameplay behavior." This one
  is **correct in research mode** and is preserved as a research-mode rule in the
  new AGENTS.md.
- **"retain the live stop"** — in the frozen plan's decision log (D10) and, as
  "The live stop … remain[s] unchanged", in `baselines/native-land-geometry.md`,
  which now carries a research-mode scope header.
- **"do not replace with guessed physics"** — no literal match. The nearest is
  "Unrecovered native coupling remains listed rather than filled with assumed
  physics" in the now-frozen flight-response plan.
- **"replicate only native functionality"** — **no occurrence anywhere in the
  repository.** The phrase was spoken, not written; the documentation encoded its
  consequences instead.
- **"runtime connected" as a completion column** — retired in
  `behavior-provenance.md`, `AGENTS.md` and `DEVELOPMENT.md`. Remaining
  occurrences are inside frozen archives or inside `formats/` and `baselines/`
  files, all of which now carry research-mode scope headers.

## 8. Conflicts not resolved here — these need John

Listed rather than decided, because each is a real scope question.

**1. The D27 label collides with an existing decision.** Detailed at the top of
this report. D27 is already "NE-00.1p scheduler/clock ownership" in the frozen
plan, whose log runs to D29. The parity strategy is labelled D27 throughout the
new documents because that is the name it was given. Recommendation: renumber the
strategy **D30**, which leaves every existing link and ID intact. One
search-and-replace.

**2. The Jeeves PM control file is now broken.** `docs/native-environment-pm.md`
is excluded through `.git/info/exclude` and says of itself "It is active control
state only; NEVER stage/commit." It was left untouched. But its eleven checklist
items each carry an `awk` verifier that reads
`docs/native-environment-systems-plan.md` by path, and that file moved to
`docs/research/`. Every verifier will now fail to find its file. Separately, its
NE-00 through NE-10 checklist tracks the work D27 supersedes. Someone needs to
decide whether to repoint the verifiers or retire the checklist. This is control
state, not documentation, so the choice is John's.

**3. The AI VM decision was settled by implication, not by John.** M0 carried an
open decision: reimplement the retail AI bytecode VM, or recreate the behaviours.
Parity by expression of feature answers it — behaviours, from a spec. The roadmap
was updated to say so and the open-decisions table marks it settled. If that
inference is wrong, revert those two edits. Nothing else depends on it; no AI
work is scheduled or authorized either way.

**4. What "contact is opinionated" means in practice.** The reclassification is
recorded everywhere. Two readings are possible and the documents do not
distinguish them: either the ground contact that already exists is now simply
accepted as shipped, or contact is now an authored feature to be deliberately
designed and improved. The parity plan's next-steps list assumes the second
("make takeoff, landing, taxi and deck behaviour feel right"). Separately, the
restricted `--native-flight-tables` research path really does still stop at
unsupported contact in the code. That behaviour was documented accurately and
scoped to that one option — it was not changed, because this pass changed no
code. Whether that research path should keep the stop, or be retired, is open.

**5. Sixteen per-predicate baseline files now violate a rule they predate.**
`baselines/native-strip-*.md` is one baseline per recovered routine, NE-00.1e
through NE-00.1r. The new AGENTS.md rule says no per-predicate baseline files.
They were preserved unchanged with scope headers, because the constraint was
never to delete evidence. Consolidating them into one frozen document would
satisfy the rule but would rewrite evidence files and break inbound links.
Recommendation: leave them; the rule is forward-looking.

**6. `docs/formats/weapons.md` is roughly half a plan.** It carries an ordered
W0–W5 implementation sequence with a "Gate:" paragraph per step, inside the
formats directory that is supposed to hold recovered facts. It was given a
research-mode scope header rather than being split, because the constraint was
not to rewrite `docs/formats/` content. Splitting the plan half into
`docs/research/` is the tidier answer if you want it.

**7. `docs/INPUT.md` may already be a spec.** It reads as a complete,
player-facing contract for an explicitly authored subsystem — closer to what
`docs/spec/` is for than to a guide. Moving it would break inbound links from the
README and the baselines, so it stayed put.

**8. Twenty evidence files still call the frozen plan "the living plan" in link
text.** Those links live in `docs/baselines/` and `docs/formats/` and now point
at an archive whose first line says it is frozen, so a reader who follows one is
corrected immediately. Fixing the link text means editing twenty evidence files,
which the "scope headers only" constraint ruled out. Cosmetic, listed for
completeness.

**9. One sentence in `docs/ARCHITECTURE.md` could not be judged from the docs
alone.** L74 refers to "native departure/contact limits" in the model
configuration. Those may be genuine PT source fields rather than provenance
language. Confirming means reading code, which was out of scope for this pass.

## 9. Checks run

Documentation-only pass; no Rust source, test or build file was touched.

| Check | Result |
| --- | --- |
| Relative link and heading-anchor check across all tracked Markdown | 108 files, 0 problems |
| `git diff --check` (whitespace errors) | clean |
| `cargo fmt --all -- --check` | pass — confirms no source file was modified |
| `python3 -m unittest discover -s tools -p 'test_*.py'` | 26 tests, pass |
| `python3 tools/check_assets.py` | pass, 267 files |

The link check was written for this pass and verified against the repository
before any edits: the documentation had zero broken relative links at the start,
and has zero now. Seven file moves repointed 67 inbound links.

Clippy, the workspace test suite and the executable asset guards were not re-run:
no code changed, and they require a build. GPU, Windows and macOS checks were not
run.

---

# Follow-up — 2026-09-16

John reviewed the pass, approved it, deleted the Jeeves PM control file and asked
for the open items to be worked through. All nine are closed. Still documentation
only; no code, test or build change.

**1. D27 → D30.** Renumbered to the next free decision ID, so the frozen plan's
D27 (NE-00.1p scheduler/clock ownership) keeps its name and every existing link
resolves. Applied to the seven freeze headers and to `docs/parity-plan.md`, which
now records the renumber and why.

**2. The Jeeves PM control file is gone.** John deleted it — his words: he is
rebuilding it. `docs/native-environment-pm.md` was removed along with its
`.git/info/exclude` entry, so the path is no longer specially ignored. Nothing
tracked referenced it except the AGENTS.md reporting rule, which now reads:
"Report to Jeeves at milestones. The PM control file was retired on 2026-09-16
and is being rebuilt; until it exists, report milestones in the session." The
broken `awk` verifiers died with the file; there is nothing left to repoint.

**3. The AI VM decision stands as settled.** Behaviours are recreated from a
spec; the retail bytecode VM is not reimplemented. It was an inference from D30
and is now confirmed. No AI work is scheduled or authorized either way.

**4. "Contact is opinionated" means both readings.** The parity plan now says so
explicitly: the ground contact that exists today is accepted as shipped, *and*
contact is a feature to design deliberately rather than a gap waiting on
recovery. The restricted `--native-flight-tables` research path keeps its
unsupported-contact stop — it is a research diagnostic, it is documented as a
limit of that one option, and removing it would be a code change.

**5. The per-predicate baselines stay.** The sixteen `baselines/native-strip-*`
files predate the rule that now forbids them, and deleting or merging evidence
was never on the table. The AGENTS.md rule is now explicitly forward-looking:
"This governs new baselines; the existing `baselines/native-strip-*` set predates
the rule and is kept as evidence."

**6. `docs/formats/weapons.md` split.** Its "Implementation status — 2026-09-14"
log and the ordered W0–W5 plan — 180 lines, and the half of the file carrying
acceptance-gate language — moved to
[`docs/research/weapons-plan.md`](research/weapons-plan.md), frozen like the
others. The formats file keeps the recovered facts and now carries a pointer;
its title changed from "FA research and implementation plan" to "FA research".
The one inbound anchor in use,
`weapons.md#development-live-fire-adapter-subsequent-implementation`, was not in
the moved block and still resolves.

**7. `docs/INPUT.md` stays a guide.** `docs/spec/` holds specifications of
*Fighters Anthology's* behaviour — that is the parity target. The input layer is
deliberately ours, not the original's, so a spec is the wrong home for it. The
spec README now states the rule: an opinionated subsystem is documented in its
own guide and its components labelled `opinionated`.

**8. The stale "living plan" link text is fixed.** Twenty files in
`docs/baselines/` and `docs/formats/` called the frozen archive "the living
plan"; they now say "the frozen plan", plus one sentence in
`formats/native-strip.md`. Link targets were already correct and did not change.

**9. `docs/ARCHITECTURE.md` L74 checked against the code.**
`crates/tore-sim/src/models/config.rs:53` documents the field group as
"Recovered departure, landing, device drag, velocity bounds and flags", and the
model reads `n.departure.warning_delay`, `stall_delay` and `severity`. These are
genuine recovered source values, so the sentence was accurate — it is now worded
"recovered departure/contact limits" rather than "native", which says the same
thing without leaning on the overloaded word.

## Follow-up checks

| Check | Result |
| --- | --- |
| Relative link and heading-anchor check | 109 files, 0 problems |
| `git diff --check` | clean |
| `cargo fmt --all -- --check` | pass — no source file touched |
| `python3 -m unittest discover -s tools -p 'test_*.py'` | 26 tests, pass |
| `python3 tools/check_assets.py` | pass, 268 files |
