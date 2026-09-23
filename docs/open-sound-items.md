# Open sound items

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Open items from the 2026-09-23 situation audio pass (radio chatter, cockpit
crew voice and situation music), kept for a later pass. The behaviour itself is
specified in [radio chatter](spec/radio-chatter.md),
[cockpit voice](spec/cockpit-voice.md) and [flight music](spec/flight-music.md).

- **Doubled ejection call.** When a wingman ejects, "Punching out!" can play
  twice: once from the [ejection](spec/ejection.md#audio-and-art) sound and
  once from the new death call.
- **Not yet voiced, because TORE has nothing to trigger them.** Waypoint calls
  (no routes), carrier deck and catapult calls (no carrier model), and "You're
  the Wingleader now" and the AWACS report (the original's triggers are
  unknown).
- **Vietnamese voice set.** The second (`#`) set of recordings is imported but
  only matters once Vietnam missions exist; see
  [the second voice set](spec/cockpit-voice.md#the-second-voice-set).
- **Agent choices open to tuning.** Radio traffic is on at the start of a
  session (radio silence off), and the Alt-S setting is not saved between
  sessions. The other approximations, such as contact report thresholds, fuel
  call estimates and takeoff detection, are listed in the "Implementation in
  TORE" sections of radio chatter and cockpit voice, and the "Current TORE
  state" section of flight music.
