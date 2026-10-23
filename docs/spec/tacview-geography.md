# Tacview geographic alignment

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. John requested calibration of both theater centers and
geographic spacing on 2026-10-02. This is an opinionated export feature. Its
calibration is fitted, not recovered original GPS data.

For every base theater, match explicitly reviewed named airfields to public
airport latitude/longitude records. Fit latitude against game north and longitude
against game east by least squares. The fitted map center is the geographic
position of the center of the terrain grid. Agent decision: retain north-up axes
and use one scale per axis; do not bend individual flight paths around landmarks.
The [calibration baseline](../baselines/theater-georeference.md) owns the measured
centers, errors, reference identities and reproduction command.

For position `(x,y,z)` in game feet, relative to map center `(cx,cz)`, export:

- Latitude: center latitude + `(z-cz) * latitude degrees per foot`.
- Longitude: center longitude + `(x-cx) * longitude degrees per foot`.
- Altitude: `y * 0.3048` meters, with no terrain-height substitution.
- Native U/V: `x * 0.3048`, `z * 0.3048`, retaining original game distances.
- Native Heading: the game's yaw in degrees. Render Yaw uses the bearing of the
  heading vector after geographic scaling, including cosine of local latitude.

Use the same transform for aircraft, weapons, decoys and ejected pilots. Variant
layouts use the base theater's calibration. A caller's center override shifts
that center while retaining the known theater's spacing. Unknown theaters retain
the explicit 0 N, 0 E fallback and a physical flat-earth conversion. Reject
nonfinite centers and centers outside latitude +/-90 or longitude +/-180.

The separate geographic and native coordinates follow
[Tacview's ACMI format](https://www.tacview.net/documentation/acmi/en/).
Calibration changes exported geography only, not simulation, replay positions,
weapon range, radar range, or the game world's dimensions.

Known limits: the source maps are not surveyed terrain. Some named placements
are inconsistent with real geography, and their identities remain uncertain.
A linear fit cannot make every airfield or coastline coincide. Two-reference
fits have no independent accuracy check. Future surface-asset work should add
independent landmarks and update the measured errors, not silently claim exact
GPS registration.
