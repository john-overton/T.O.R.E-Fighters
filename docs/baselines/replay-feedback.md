# Dedicated replay recording and feedback validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation validation on Linux, 2026-10-22 (America/Chicago). Build:
0.2.0, protocol 67, replay format 4, `replay-feedback` worktree based on
`b008a1b8` plus this change. The imported profile identifies Fighters
Anthology 1.02F. This is a host-feature test, not a retail comparison.

The server guide's 12-aircraft Ukraine mission was flown by one local client.
Both runs recorded at 120 Hz, through the player's departure until console
quit. The ordinary replay viewer opened both files without reader problems.
Sizes are measured files, not a compression guarantee:

| Feedback | Duration | Frames | Replay bytes | Companion log bytes | Flags |
| --- | ---: | ---: | ---: | ---: | ---: |
| off | 28.275 s | 3,393 | 461,455 | 3,001 | 0 |
| on | 25.100 s | 3,012 | 393,862 | 2,348 | 1 |

These short examples average about 0.9 MiB of replay data per minute. Longer
missions, aircraft count, weapons, smoke and damage change the rate. Flagged
files are unlimited by retention. The existing format bounds a file to 1 GiB;
an I/O or format-limit failure is logged, and feedback is disabled for that
mission rather than continuing to advertise a working recorder.

`net-replay-feedback-on` submitted one report and cancelled another. The
server and automatically converted local replay exported identical flag data.
The submit tick was later than the opening tick, confirming that the form did
not pause multiplayer. A limit of 0 retained the flagged server recording.
The viewer selected the opening tick and displayed the complete report.
`net-replay-feedback-off` showed the baseline Escape menu without the button.

The battery uses isolated profiles and hidden workspaces. Its driver waits for
the normal asynchronous local conversion before stopping the menu process.
An earlier fixed delay ended that process before conversion finished; the
capture itself contained the correct report. The initial run also exposed the
empty-cockpit render snapshot reset, now covered by a no-player recording and
mission-rotation regression test.

Related passing scenarios: `net-server-check`, `net-server-chat`,
`net-convert-capture`, `replay-menu-pages`, `replay-keys-bookmarks`,
`replay-view-model-f18` and `replay-view-model-rafale`.

Evidence stays local, with retail-derived captures uncommitted:
`.local/next-round/replay-feedback/battery/20261022-193216-complete/` in the
main checkout. The session report includes the capture paths and full checks.
Physical controller hardware and Windows/macOS runtime execution were not
validated; controller input was exercised synthetically. The server has no
client cockpit, heard radio or locally regenerated contrail tracks.

See [behavior](../spec/replay-feedback.md),
[event layout](../formats/replay-feedback.md) and
[operator settings](../DEDICATED-SERVER.md#recording-and-feedback).
