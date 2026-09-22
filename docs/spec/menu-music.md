# Menu music startup

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation specification, 2026-09-22. Menu music uses the existing
[recorded retail playlists and playback rules](../formats/music.md#runtime-scope-and-boundaries).

Music is enabled by default for a profile without saved preferences. A saved
Music On or Off value overrides that default. This is an agent-selected host
preference rule, not evidence of the original game's initial preference.
Starting without an audio device, requesting silent diagnostics, or transferring
sample buffers into the audio player must not change the Music preference.
Missing optional music media still follows the existing explicit silence and
diagnostic behavior; no substitute track is added.

The main-menu M key and Music option continue to toggle the saved preference.
Mute retains playback position. Ordinary menu and Quick Mission playback uses
its existing playlist, gain and transitions. This fix changes initialization
only, not track selection, sample data, mixer timing or explicit saved choices.

[Regression evidence](../baselines/menu-music-startup.md) records the original
fault, introducing commit and validation.
