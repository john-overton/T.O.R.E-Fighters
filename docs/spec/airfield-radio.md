# Airport and wingman departure and landing calls

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode after static retail research, 2026-09-23. John requested
this pass, startup takeoff clearance and Quick Mission wingmen queued on the
taxiway. Recording identities and original event gates come from
[airport speech research](../formats/radio.md#airport-speech-review).
No running retail comparison was made.

## Calls the player hears

A ground-started player hears the selected airport clear the runway for takeoff
at startup, once, using the original clear-for-takeoff recording. Selecting that explicit variant
for startup is an agent choice.
When another aircraft occupies the runway, it reports a text hold instead and
issues clearance once free. Becoming airborne produces the original airborne
call, followed by good luck or good hunting after climbing 10 ft above airport
ground. The same calls return on restart. Ground starts never trigger a welcome
home or a landing grade.

Landing clearance and wind use the reviewed tower recordings. A landing request
and Repeat keep their existing meanings. An approaching player with gear down
can receive automatic clearance within 7 nautical miles of a friendly usable
runway when approaching its near end within 45 degrees. This automatic host
binding is fitted; tower selection and explicit permission still apply.
Wind reports actual host wind speed in knots, once per approach. Random retail
gust wording is omitted unless the host has a measured gust to report.

Touchdown announces the latest grade from the same fitted landing grading used
by the debrief, then welcome back or welcome home after slowing on the runway.
Cancelling, changing airport, a disabled runway, death and restart clear stale
pending airport calls. Raising the gear or leaving the approach vicinity resets its approach
calls so another attempt can be reported.

Carrier-only catapult, hook, call-the-ball, detailed landing-officer corrections,
wave-off and emergency-deck calls stay identified but inactive until the host
has carrier operations. They are not evidence of those facilities on a land
runway. No invented recording is assigned to a text-only taxi status.

## Wingmen

Each living member of the player's wing reports holding short, taxiing, taking
off, holding at marshal, approaching, going around, taxiing clear and parked
when those states change. These are opinionated status messages requested by
John. Takeoff clearance, airborne, landing clearance, go-around, touchdown grade and welcome use the
reviewed airport recordings with the aircraft identified in the subtitle.
Applying the human-facing retail calls to AI wingmen is a fitted host extension.
The original has not established separate taxi or marshal voice recordings.

Reports share the existing radio channel. One airfield report is delivered at
most every 3 simulation seconds. Pending status is replaced when that actor's
state advances, expires after 15 seconds and is discarded when the actor dies.
Player clearance takes precedence over routine wing status. Radio silence
suppresses routine wing reports; player clearances remain important calls.
Pause freezes all timing. Restart resets report history. Speech never changes
flight or clearance decisions. These queue limits are fitted agent choices.
