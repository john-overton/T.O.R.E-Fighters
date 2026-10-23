# Replay feedback and dedicated recording

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Opinionated addition requested by John on 2026-10-22. This is a host feature,
not a claim about Fighters Anthology. The addendum restricts feedback to
dedicated servers whose operator enables it.

- `feedback off` and `record-replays off` are the defaults. Enabling feedback
  without recording is a startup error. These are server-wide settings.
- Only a flying player on an enabled dedicated server sees **Feedback...** in
  Escape's first tab. Single player, player-hosted games and replay playback
  have no submission entry. The menu and form do not pause multiplayer.
- Opening the form captures the displayed simulation tick and UTC wall time.
  Text is UTF-8, at most 1,000 Unicode scalar values and 4,000 bytes. Empty or
  whitespace-only submissions are refused. Cancel writes nothing.
- Keyboard editing supports Left, Right, Home, End, Backspace, Delete and
  Enter for a newline. Tab selects the field, Submit and Cancel. Escape
  cancels. The controller's menu directions and accept/back navigate the
  on-screen keyboard and the buttons. Clipboard paste is not supported by
  the existing application and is not added here.
- The host accepts feedback only from a currently seated player while enabled.
  It supplies the callsign, seat and exact aircraft identity, clamps future
  flag ticks to its current tick, records its submit tick, and sends the
  accepted event to the sender for the local capture's replay conversion.
  The existing chat rate limiter also bounds submissions. Client wall times
  are recorded as supplied; they are not evidence of synchronized clocks.
- A replay with any `player.feedback` event is flagged. Pink timeline markers
  identify the opening tick. The viewer's Feedback list shows the flags;
  selecting a row seeks to that tick and shows the text. Up/Down choose a row,
  Enter seeks, PageUp/PageDown scroll the text, and Escape closes the list.
- With recording enabled, every authoritative host tick goes to an ordinary
  replay. The server records aircraft, weapons, damage, moving surface units,
  debris, pilots and effects. There is no dedicated-server cockpit display
  or heard radio track. Its companion `.log` holds mission combat and server
  events. The ordinary viewer opens the recording.
- Recording must not pause flight while files are written or synced. Chunks
  remain 120 frames; syncs remain every 3,600 written frames and at finish.
- At most 120 accepted frames may await a completed chunk write, excluding
  the host's existing one-frame event staging. A chunk that is syncing still
  counts against this limit until its sync completes. This preserves the
  previous crash exposure without adding more unwritten frames.
  If a handoff would exceed the budget, the host never waits: it logs a
  recording failure immediately, disables feedback for this mission, and
  stops recording. Accepted frames are written and synced to a recoverable
  `.partial` file without a completion footer. No frames are silently skipped
  in a complete recording. Retention leaves this partial and its log alone.
- Mission end, rotation, Stop, host handover and graceful shutdown wait for
  accepted frames to finish writing. These boundaries may wait for disk
  completion. They include the last staged frame and normal completion
  footer unless recording already failed. A new mission can record again.
- Periodic and console `status` lines append `, worst N.N ms` after all
  existing fields. This is the slowest tick since the preceding status call;
  an interval with no ticks reports zero. The existing mean and load remain.
- `replays-kept` defaults to 10 and counts complete unflagged server recordings
  only. After finishing a mission, oldest eligible recordings and their logs
  are removed first. All flagged recordings survive, even with a limit of 0.
  Incomplete, damaged, unreadable and non-server files are left alone.
  The operator is responsible for those files and for flagged recordings.
- `replays-folder` defaults to `replays/` inside the imported data folder,
  beside `logs/`. An explicit relative path is relative to the configuration
  file. Local player recording and retention settings are unchanged.

The 120-frame shared budget and stopping the recording on saturation are
opinionated agent decisions, 2026-10-23. A slow chunk write or sync can end a
recording even when disk throughput averaged over the mission would suffice.
This deliberately favors uninterrupted flight and the existing crash-loss
bound. The host's pending event frame is unchanged.

The 1,000-character limit, controller keyboard, startup rejection, timestamp
units, prefix-scoped retention and conservative handling of damaged files are
agent decisions. Format details have one home in
[the feedback event layout](../formats/replay-feedback.md).
