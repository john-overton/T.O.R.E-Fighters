"""Lane: ai. Fires on the city fill's knocked-down buildings (nr-fx2,
docs/spec/surface-defenses.md, "Wreck fires").

`surface-city-fires` runs `--city-fires`, which builds Ukraine with its city
fill, knocks every building within a radius of Town 1's centre flat, and reads
what the burning city draws: every flattened building burns (no cap on how
many), a frame draws only the nearest fire sprites and smoke columns however
many burn, and the frame's CPU time stays small. No window and no GPU.
"""
import re

from battery import Scenario, Step

FLATTENED = re.compile(r"^city-fires: flattened (\d+) within", re.M)
OBSERVED = re.compile(r"^city-fires: observed (\d+) burning in", re.M)
FRAME = re.compile(r"^city-fires: (over|near|far): (\d+) sprites (\d+) smoke puffs, ([\d.]+) ms a frame", re.M)
BUDGET = re.compile(r"^city-fires: budget (\d+) sprites, (\d+) columns of (\d+) puffs$", re.M)

# A debug build's frame of the burning city must stay well under this, ms.
FRAME_MS = 20.0


def run_problems(output: str, label: str, at_least: int) -> list[str]:
    problems: list[str] = []
    flat, seen, budget = FLATTENED.search(output), OBSERVED.search(output), BUDGET.search(output)
    if not (flat and seen and budget):
        return [f"{label}: no flattened, observed and budget lines"]
    flattened, burning = int(flat.group(1)), int(seen.group(1))
    sprites, columns, puffs = (int(v) for v in budget.groups())
    if flattened < at_least:
        problems.append(f"{label}: only {flattened} buildings flattened, want at least {at_least}")
    if burning != flattened:
        problems.append(f"{label}: {burning} burn of {flattened} flattened, want every one (no cap)")
    frames = {m.group(1): m for m in FRAME.finditer(output)}
    if set(frames) != {"over", "near", "far"}:
        problems.append(f"{label}: frames {sorted(frames)}, want over, near and far")
    for where, m in frames.items():
        drawn, smoke, ms = int(m.group(2)), int(m.group(3)), float(m.group(4))
        if drawn > sprites or smoke > columns * puffs:
            problems.append(f"{label} {where}: {drawn} sprites and {smoke} puffs past the budget {sprites} and {columns * puffs}")
        if ms > FRAME_MS:
            problems.append(f"{label} {where}: a frame took {ms} ms of CPU, want under {FRAME_MS}")
    over = frames.get("over")
    if over is not None and int(over.group(2)) != min(burning, sprites):
        problems.append(f"{label}: {over.group(2)} sprites over the fires, want {min(burning, sprites)}")
    return problems


def problems(output: str) -> list[str]:
    runs = re.split(r"^city-fires: UKR fill .*$", output, flags=re.M)[1:]
    if len(runs) != 2:
        return [f"{len(runs)} runs finished, want 2"]
    return run_problems(runs[0], "6,000 ft", 300) + run_problems(runs[1], "the whole city", 40000)


def scenarios() -> list[Scenario]:
    return [
        Scenario(
            name="surface-city-fires", lane="ai",
            args=["--city-fires", "UKR", "180224,606208,6000", "--repeat", "50"],
            timeout=300,
            then=[Step(["--city-fires", "UKR", "180224,606208,70000", "--repeat", "20"], timeout=300)],
            expect=[r"^city-fires: budget \d+ sprites"],
            check=problems,
            notes="nr-fx2: every flattened city fill building burns, with no cap; a frame draws the nearest 1,024 "
                  "fire sprites and 32 smoke columns however many burn. Builds the fill on the CPU, no window.",
        ),
    ]
