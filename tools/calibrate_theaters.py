#!/usr/bin/env python3
"""Fit Tacview map centers and spacing from local layouts and public airport GPS.

Input layouts remain user-owned local media. Airport coordinates come from
https://ourairports.com/data/ (public domain). Reviewed name/airport matches live
in data/theater-georeference.json; this tool never guesses identities by proximity.
Run with --layouts DIR/FA_2.LIB --airports airports.csv. JSON goes to stdout.
"""
import argparse
import csv
import json
import math
from pathlib import Path
import re
import statistics

CELL_FT = 8192.0


def placements(text):
    """Named runway positions, with duplicate names rejected instead of hidden."""
    found = {}
    for block in re.split(r"(?m)^obj\s*$", text)[1:]:
        block = re.split(r"(?m)^\s*\.\s*$", block, maxsplit=1)[0]
        kind = re.search(r"(?m)^\s*type\s+(STRIP\w*\.OT)\s*$", block, re.I)
        name = re.search(r"(?m)^\s*name\s+\x01(.*?)\x01", block)
        position = re.search(r"(?m)^\s*pos\s+(-?\d+)\s+(-?\d+)\s+(-?\d+)\s*$", block)
        if kind and name and position:
            if name[1] in found:
                raise ValueError(f"ambiguous runway name: {name[1]}")
            found[name[1]] = [int(position[1]), int(position[3])]
    return found


def line_fit(xs, ys):
    if len(xs) < 2 or len(xs) != len(ys):
        raise ValueError("at least two paired coordinates are required")
    if not all(math.isfinite(v) for v in [*xs, *ys]):
        raise ValueError("nonfinite coordinate")
    mx, my = statistics.mean(xs), statistics.mean(ys)
    denominator = sum((x - mx) ** 2 for x in xs)
    if denominator <= 0:
        raise ValueError("reference points have no span on this axis")
    slope = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / denominator
    if slope <= 0:
        raise ValueError("references reverse map north or east")
    return my - slope * mx, slope


def fit(points, grid):
    center = [(n - 1) * CELL_FT / 2 for n in grid]
    latitude, lat_per_ft = line_fit([p['north_ft'] - center[1] for p in points], [p['latitude'] for p in points])
    longitude, lon_per_ft = line_fit([p['east_ft'] - center[0] for p in points], [p['longitude'] for p in points])
    errors = []
    for p in points:
        lat = latitude + lat_per_ft * (p['north_ft'] - center[1])
        lon = longitude + lon_per_ft * (p['east_ft'] - center[0])
        # Report local surface discrepancy in km, not simulation flight distance.
        errors.append(math.hypot((lat - p['latitude']) * 111.195, (lon - p['longitude']) * 111.195 * math.cos(math.radians(latitude))))
    return dict(latitude=latitude, longitude=longitude, degrees_per_ft=[lat_per_ft, lon_per_ft],
                references=len(points), rms_km=math.sqrt(statistics.mean(v * v for v in errors)),
                max_km=max(errors), residuals_km=errors)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--layouts', type=Path, required=True)
    parser.add_argument('--airports', type=Path, required=True)
    args = parser.parse_args()
    references = json.loads((Path(__file__).parent / 'data/theater-georeference.json').read_text())
    with args.airports.open(newline='', encoding='utf-8') as source:
        airports = {r['ident']: r for r in csv.DictReader(source)}
    result = {}
    for code, reference in references.items():
        locations = placements((args.layouts / f'{code}.MM').read_text(encoding='cp437'))
        points = []
        for name, ident in reference['airfields'].items():
            east, north = locations[name]
            airport = airports[ident]
            points.append(dict(east_ft=east, north_ft=north, latitude=float(airport['latitude_deg']), longitude=float(airport['longitude_deg'])))
        result[code] = fit(points, reference['grid'])
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
