# Flight loading screen

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. **Opinionated**, requested by John on 2026-10-16:
flight preparation shows the main T.O.R.E squadron patch, an aircraft photograph,
a named stage and a progress bar while the window continues receiving events.
This is host behaviour, not a claim about the retail game's loading screens.

## Presentation

The two backgrounds are John's Harrier sunset and Thunderbirds canopy-to-canopy
photos. He supplied the full-size originals on 2026-10-16. The build bundles
them as 2560 by 1707 baseline JPEGs. One random photo is chosen per load and
retained across its worker stages, including automatic Quick Mission launch
and the multiplayer connection screen that continues into mission preparation.
The loading worker decodes a JPEG with `zune-jpeg` before the window samples
it. The window thread never decodes. Photos cover the whole window, with a
dark gradient for readable text. The CPU cover-fit samples the decoded photo
into the logical canvas, and the GPU stretches that canvas to the window, as
before. If a decode fails, the screen logs one line and keeps a plain dark
background under the same patch, stage and progress bar. The connecting screen,
which can appear before a flight loader exists, starts that decode on a short
background thread, still not on the window thread. The patch is 220 pixels
across on the 640 by 480 menu canvas. The progress bar is 480 by 16 logical
pixels. Text uses the bundled Noto Sans menu font. Image provenance is in
[website images](../WEBSITE.md#images); asset format and sizes are in the
[bundled asset notes](../../crates/tore-app/assets/README.md).

The stage names are Preparing loadout, Building the mission, Loading aircraft, Loading terrain
and scenery, Preparing graphics, and Starting flight. Replay preparation is
labeled Loading the replay. Loadout selection has a fitted 25 ms allowance. CPU stage reference weights
are measured milliseconds from the multiplayer base: mission 2442, aircraft art 11,
scenery 164. Graphics reserves **fitted** 500 ms, an agent decision until GPU
measurement is available. Other loading modes reuse these reference weights
as fitted estimates pending separate measurements. The resulting boundaries are approximately 0, 0.8, 78.5,
78.9, 84.1 and 100 percent. The label explicitly says Estimated. Elapsed time
advances within a stage asymptotically toward its next boundary; only worker
completion reaches 100 percent. Progress never resets within a load; duplicate or late stage notifications
cannot move it backward. Elapsed seconds remain visible. There is no moving
activity strip or moving bar highlight, requested by John on 2026-10-17.
[Measurement and limits](../baselines/loading-screen.md) give the source of the
weights. Checking game content, Connecting to the host and Waiting for the host use a
stationary bar and elapsed time without a percentage because their durations
are externally determined. The same photo continues into mission preparation.

## Scheduling and cancellation

Quick Mission selection loads only the aircraft needed for the loadout page.
It does not build a throwaway terrain, combat world, scenery or graphics set.
Automatic launch continues the same progress and photograph into the single
mission build. Custom loadout editing finishes selection before user editing;
Fly then starts its own load. Selection no longer builds terrain, scenery or
graphics, so those stages run only in the mission build.
A multiplayer join keeps one progress from content check through mission
preparation. A player who leaves a flight and takes a plane again continues
that rebuild's progress into the presentation that follows. Waiting after the
mission stage has started does not rewind the bar.

Quick Mission preparation, restarts, replay Watch opening, multiplayer client mission
builds, and multiplayer presentation preparation run on workers. GPU scenery
creation, aircraft textures and cockpit uploads use cloned thread-safe wgpu handles; the window and surface stay on
the event thread. The existing network keepalive mechanism remains enabled.
Session events wait in their existing order behind a background mission build.
Default headless session clients keep synchronous builds for virtual-time runs.
Host and join content hashing waits are polled, never joined on the event thread.
Multiplayer re-entry also builds on a worker.

Escape while multiplayer content or a mission is building cancels the join and returns to
the existing connection screen, when the live world is still on the event thread.
A theater change or restart that has moved the world into the worker runs to
completion, so cancellation cannot discard that world. The worker can finish
but cannot install its result after cancellation. The window can be closed or resized
while preparation runs. Simulation does not advance during single-player loading. Recording initialization runs with the flight loader. Finalization,
graphics retirement and a load still running finish off the event thread; shutdown waits for them after closing the window.

## Validation and remaining coverage

CPU previews: `--snapshot FILE.ppm --snapshot-state loading` and
`--snapshot-state loading-thunderbirds`. Neither requires media or a GPU.
A window size may be appended, as in `loading-1920x1080` or
`loading-thunderbirds-1920x1080`. The file is still the logical canvas for
that window, the same buffer the GPU stretches, not a separate full-window
render.
The in-memory multiplayer test holds the worker while the caller continues
servicing transport for seven seconds of virtual time, then requires flight.
The local-only ignored loading profile test measures the caller's heartbeat
while building a real mission from a private profile copy.

The implementation is not yet a complete no-stall guarantee: remaining per-frame geometry and driver work need display-host measurements.
Graphics destruction runs off-thread, but shared-device driver contention still
requires a display-capable host check. A headless heartbeat test is not a
measurement of compositor responsiveness.
