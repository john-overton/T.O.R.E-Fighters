# Quick Mission Creator planning evidence — 2026-09-14

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


Read-only code/media review plus selective local extraction; no runtime behavior
changed. Reference photo inspected: `gameassets/reference-photos/quick-mission-creator-screen.jpg`.
It shows three wings on each side, nationalities, theater, altitude, conditions,
situation, distance, load, weapon restriction, ground target and AAA/SAM fields.

## Reproduction

```sh
python3 tools/extract_assets.py --include '*QUI*' --include '*.DLG' --include '*.MNU' --exclude-archive 'disc1/LHX/*' --exclude-archive 'disc1/WB/*' --out .local/quick-mission-plan
```

Result: 108 resources, zero errors; archive/resource hashes and provenance in
ignored `.local/quick-mission-plan/extraction-report.json`. Initial broad scan
encountered five unrelated demo/archive-format errors; the command above excludes
those demo directories and completed successfully. Extractor built with `--locked`.

FA.EXE SHA-256:
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`.
The offsets below are raw file offsets, not virtual addresses.

## Findings and limits

- FA_1.LIB supplies QUIKMIS3/QUIKMISS art; FA_2.LIB supplies QUIKMISS.DLG,
  QUICKB3–25.DLG, QUICK14.DLG, QM_MENU.MNU and QUICK/QUICKMP.MT.
- QUICKB8 embeds test theater names. QUICKB7 embeds altitude choices differing
  from executable strings. QUICKB5/15/18/21 omit the ace skill shown in the photo
  and executable. These lists cannot be treated as the active retail options.
- EXE strings at `0xed908–0xed923` contain novice through ace;
  `0xedae0–0xedaf4` contain 5,000/10,000/20,000/40,000;
  `0xedb00–0xedb2e` contain combined weather/time labels;
  `0xedb38–0xedb4c` contain encounter advantage labels;
  `0xedb60–0xedb89` contain 1/2/5/10/20/50-mile distances.
  These are research candidates, not verified complete lists or default ordering.
- Multiple target lists occur from `0xedbf0` onward. Their theater/edition
  dispatch remains untraced; do not merge them into one universal dropdown.
- QM_MENU contains Aircraft, Fly all and era ranges. Native filter callbacks,
  full tree and default/check states remain unverified.
- BARCAP occurs at `0xeeb1c` near other flight tasks. That establishes a string,
  not a Quick Mission mission-type control or executable patrol implementation.
- Current app OK returns `Action::FreeFlight`; the UI has only aircraft/theater
  selectors. `Airframe::start` supplies the airborne position; manual combat's
  range fixture is independent. No BARCAP mission objective was found in this path.

No native game code was executed. No creator handler disassembly, complete option
mapping, new UI captures or runtime acceptance was performed in this planning pass.
See the [implementation plan](../research/quick-mission-plan.md) for remaining gates.
