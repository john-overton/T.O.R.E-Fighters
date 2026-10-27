#!/usr/bin/env python3
"""Select one-anchor game fits jointly. See docs/formats/missile-energy-data.md."""
import argparse
import csv
import itertools
import json
from pathlib import Path

CELLS = [f"r{height}{aspect}" for height in ("500", "5", "15", "35") for aspect in "hbt"]
TAILS = ["r5t", "r15t", "r35t"]
FAMILIES = [("AIM7.JT", "AIM7E.JT"), ("R27ER.JT", "AA10.JT"), ("R27ET.JT", "R27T.JT")]
MODERN = ["AIM9M.JT", "AIM9X.JT", "AA11.JT"]
EARLY = ["AIM9B.JT", "AA2.JT", "AA8.JT"]


def beats(newer, older, cells):
    """Require a positive, strictly ordered envelope with 2 m search margin."""
    return all(older[key] > 0 and newer[key] > older[key] + 0.002 for key in cells)


def select(rows):
    by_store = {}
    for row in rows:
        # Retain the established finite deck escape regression for modern IR.
        if row["source"] in MODERN and row["r500t"] >= 11.99:
            continue
        by_store.setdefault(row["source"], []).append(row)
    best = {name: min(options, key=lambda row: row["score"])
            for name, options in by_store.items()}
    margins = {}
    for newer, older in FAMILIES:
        pairs = [(a["score"] + b["score"], a, b)
                 for a, b in itertools.product(by_store[newer], by_store[older])
                 if beats(a, b, CELLS)]
        if not pairs:
            raise ValueError(f"No feasible family fit: {newer}/{older}")
        _, best[newer], best[older] = min(pairs, key=lambda pair: pair[0])
        margins[f"{newer}/{older}"] = min(
            best[newer][key] / best[older][key] - 1 for key in CELLS)

    options = []
    for modern in itertools.product(*(by_store[name] for name in MODERN)):
        early = []
        for name in EARLY:
            eligible = [row for row in by_store[name]
                        if all(beats(newer, row, TAILS) for newer in modern)]
            if not eligible:
                break
            early.append(min(eligible, key=lambda row: row["score"]))
        if len(early) == len(EARLY):
            options.append((sum(row["score"] for row in (*modern, *early)), modern, early))
    if not options:
        raise ValueError("No feasible short-IR joint fit")
    _, modern, early = min(options, key=lambda option: option[0])
    for name, row in zip(MODERN + EARLY, [*modern, *early]):
        best[name] = row
    for newer, older in itertools.product(MODERN, EARLY):
        margins[f"{newer}/{older}"] = min(
            best[newer][key] / best[older][key] - 1 for key in TAILS)
    # Fictional R73 class shares the exact physical fit.
    best["AA11B.JT"] = next(
        row for row in by_store["AA11B.JT"]
        if abs(row["drag"] - best["AA11.JT"]["drag"]) < 1e-10
        and abs(row["fraction"] - best["AA11.JT"]["fraction"]) < 1e-8)
    return {"fits": best, "margins": margins}


def main():
    parser = argparse.ArgumentParser(
        description="Select family-constrained game fits from --fit-class CSV/log. "
                    "Writes evidence JSON, never runtime data or source.")
    parser.add_argument("candidates", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    lines = args.candidates.read_text().splitlines()
    header = next(line for line in lines if line.startswith("source,class,"))
    rows = list(csv.DictReader([header] + [line for line in lines if ".JT," in line]))
    for row in rows:
        for key in row.keys() - {"source", "class"}:
            row[key] = float(row[key])
    result = select(rows)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(f"Selected {len(result['fits'])} fits; family margins: {result['margins']}")


if __name__ == "__main__":
    main()
