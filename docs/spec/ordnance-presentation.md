# Load Ordnance presentation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation specification, 2026-09-16. The supplied retail screenshot guides
presentation; its executable identity is unknown. Recovered card positions and
loadout interactions remain documented in the [ordnance contract](../formats/ordnance-menu.md).
This pass changes presentation, not store compatibility, quantities, fuel steps,
launch validation, flight adapters or combat behaviour.

The original ORD_AIR3 background, thumbnail images, dial, rocker and button
pieces remain runtime imports. Catalog cards retain two columns at x=68/188,
four rows from y=108 at 68-pixel spacing. Station headings retain x=350/469,
three rows from y=121 at 71-pixel spacing. Card names, quantities, location
labels and numeric fields use the approved 10 px Noto Sans Bold atlas from
[Quick Mission](quick-mission-menu.md). This is an agent-selected extension of
the approved font, not evidence of the original font face. Selected weapon names
are yellow and catalog mass/guidance are blue. Names must fit 111 pixels.

The category dial uses DIAL13 for air-to-air and DIAL11 for air-to-surface,
at (148,393), pointing toward the corresponding category lamp. LIGHTON and LIGHTOFF overlay both background lamps at (115,394) and (115,422),
so active and inactive indicators both use the original blue outer rim. These images were visually inspected; the angle choices
and coordinates are fitted to the reference. They keep the entire dial and shadow
inside the white category frame and do not cover either label or indicator.
The existing category hit regions and keyboard shortcuts are retained.

Vehicle weights have comma-separated integer pounds and align to x=467 inside
their black fields. The available-weight value is shifted down two canvas pixels
to y=381 following John's visual correction. Fuel pounds are centered in (487,354,56,14). Fuel percentage
shows the number only in (487,375,27,14), since the background supplies the percent
sign. The page count is centered in (248,388,48,15). Text centering measures the
visible glyph pixels, excluding transparent font padding.

Select Plane and Fly share Quick Mission's button composition and label offsets.
Fly includes the original ACTDFLT striped cap joined to ACTDFT0L/M/R. Select Plane
uses ACTION0L/M/R without an added outer outline. Labels retain the approved
lower position; pointer hit regions include Fly's cap. Button actions are unchanged.

Top menu labels retain MENUFONT. Ordnance and Quick Mission labels are centered
vertically inside y=38..58, using visible glyph bounds. Ordnance Weapons and
Airbase begin at x=103 and x=178; matching hit regions do not overlap. Main-menu
bar labels are centered within their existing interactive rectangles. The exact
original font alignment and dial placement remain unknown; these are fitted rules.
