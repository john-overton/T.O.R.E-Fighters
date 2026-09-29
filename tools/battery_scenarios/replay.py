"""Lane: mission recording, replay playback, comms and audio start-up."""
from battery import Scenario


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="replay-record-verify",
            lane="replay",
            args=["--ai-probe-ticks", "1200", "--separation", "2", "--record-mission", "{work}/a.tore-replay", "--verify-render", "--no-audio"],
            expect=[r"AI probe totals:"],
            outputs=["a.tore-replay"],
        ),
    ]
