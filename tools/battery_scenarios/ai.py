"""Lane: AI fights, wings, damage and missiles (headless probes)."""
from battery import Scenario


def scenarios() -> list[Scenario]:
    out = []
    for f, e in [(1, 1), (2, 2), (3, 3), (5, 5), (8, 8), (10, 10), (15, 15)]:
        out.append(
            Scenario(
                name=f"ai-fight-{f}v{e}-default",
                lane="ai",
                args=[
                    "--ai-probe-ticks", "7200", "--probe-fight", f"{f}:{e}", "--separation", "5",
                    "--probe-attack", "600:10", "--no-audio",
                ],
                timeout=900,
                expect=[r"AI probe totals:"],
            )
        )
    return out
