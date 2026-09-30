"""The retail short strips, which a Quick Mission ground start never uses.

John decided on 2026-09-30 that short strips (a runway under 2,000 ft, the 22
strips of about 1,074 ft in Cuba, the Falklands, Pakistan, Panama and the
Persian Gulf) are no ground start for the player or any wing. The AI probe's
`--ground-start N` (N counts the theater's airports, 1 up) refuses them, so a
scenario that wants a takeoff picks a long airport with `ground_airport`.

The table is what `TORE_AIRPORT_PROBE=1 tore-app --quick-mission --snapshot
X.ppm --theater CODE` lists as `short_strip=true` (docs/testing/lane-ai.md,
"Short strips"). The developer `~` layouts have none. Every base theater keeps
at least three long airports (the Falklands has the fewest).
"""
from __future__ import annotations

# Base theater -> the airport numbers of its short strips.
SHORT_STRIPS = {
    "APA": {2, 3, 4, 5, 6},
    "CUB": {13, 14, 15, 16, 17, 18},
    "LFA": {2, 3},
    "PGU": {9, 10, 13, 14, 15},
    "SPA": {8, 9, 13, 15},
}


def is_short_strip(theater: str, airport: int) -> bool:
    return airport in SHORT_STRIPS.get(theater, ())


def ground_airport(theater: str, wanted: int) -> int:
    """`wanted` if it is a ground-start airport, else the next airport that is."""
    while is_short_strip(theater, wanted):
        wanted += 1
    return wanted
