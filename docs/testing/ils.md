# ILS alignment and glide angle

Findings from the 2026-09-29 check requested by John, who noticed that the ILS was
not quite aligned for altitude at the airports and that the approach felt shallow.
The command is `tore-app --validate-ils` (add `--theater '~CODE'` to include one
developer layout); the battery runs it as `menus-validate-ils*` and the landing
scenarios of the flight lane read the ILS along a flown approach (`ils_probe:`).

## What was measured

For all 578 runway ends of the 16 base theaters (and 28 to 68 more in each of four
sampled `~` layouts): the ILS datum height at the threshold against the runway
support plane (the plane the wheels touch), the terrain there, the plane at the
touchdown zone, the runway's pitch and bank, and where the ideal path crosses the
threshold. Then an aircraft was placed on the ideal path at six ranges from 28,000
to 300 ft, and the bars were read with 10 to 100 ft displacements each way.

## Result

- **The datum height was already right everywhere.** The datum (the authored airport
  ground) equals the runway plane at the threshold and at the touchdown zone to the
  foot at every airport of every base theater: worst offset 0.0 ft, every runway flat
  (pitch and bank 0). The terrain at the threshold matches to the foot too, except
  KURILE airport 3 (terrain 71 ft above the runway at the near end) and NSK airport
  6 (65 ft above), where the terrain around the airport square is not level with it.
  These are scenery facts; the ideal path is 19 and 12 ft below that terrain at the
  threshold and nothing else is above the path, apart from four Ukraine far ends
  (Donets'k, Kharkiv, L'viv, Ivano-Frankivs'k) whose 5 nm corridor runs into hills
  1,900 to 2,000 ft above the path at 13,000 to 28,000 ft out, and Amiens (2 ft at
  5,500 ft). The survey prints them as `ils-terrain:` lines and does not fail.
- **The path was wrong in two ways, both fixed.**
  1. The glide error used the height of the aircraft's origin, not its wheels, and
     the origin sits 8 to 14 ft above the wheels (F/A-18: 8.5). Following the bars
     to zero put the wheels that far under the runway plane: at 800 ft out the bar
     read +0.6 degrees on a wheel-perfect path, and a pilot who centred it touched
     down short of the threshold. The wheel height is now used
     (`airport::Aircraft::ground_clearance_ft`).
  2. The path was aimed at the threshold itself, at ground level: zero height over
     the threshold, touchdown at the threshold. Real approaches and the manual
     ("The aircraft should touch down approximately a quarter down the length of the
     runway", p. 68) aim past it. The aim point is now the touchdown zone, 1,000 ft
     past the threshold on the runway plane, so the path crosses the threshold
     **52.4 ft** up at every airport (the same as the aim point's height offset of
     0 plus 1,000 ft times tan 3 degrees). The landing probe already aimed there.
- **Signs and centring are correct.** High reads positive, and the HUD draws
  `y = 240 + normalized * 30`, so the dots go to the bottom of the HUD when too
  high and to the top when too low, as the manual says (p. 66). Right of the
  centre line reads positive and the bar draws at `x = 320 - normalized * 42`, so
  the dots drift left when you are too far right. The two bars cross at the
  HUD centre (320, 240) on the path. The survey checks every runway end at six
  ranges; unit tests cover it (`the_glide_path_aims_at_the_touchdown_zone_and_reads_zero_on_the_path`).
- **Flown check.** The scripted `land` pilot follows the same path: over 676 landing
  scenarios it stays within 0.97 degrees of the glide bar during the approach (0.31
  degrees for the Hornet) and within 0.02 degrees of the localizer in calm air.

## The one shared path

`crates/tore-sim/src/airport.rs`: `GLIDE_SLOPE_DEGREES` (3.0), `AIM_PAST_THRESHOLD_FT`
(1,000), `glide_path_height_ft(before_threshold_ft)` (the wheels' height above the
runway surface on the path, zero at the aim point, 52.4 ft at the threshold) and
`Runway::aim_point(end)`. The player's ILS uses them; the AI landing controller
should aim at the same point.

## The angle: brief note

John decided the ILS stays at 3 degrees (2026-09-29). The evidence gathered, for the
record:

- The retail glide angle is not recovered. The manual gives none. The AI's landing
  path from the executable is 6 degrees with gates 35,200, 17,600 and 8,800 ft out
  (heights 3,700, 1,850 and 925 ft at 6 degrees, 1,845, 922 and 461 at 3).
- At the ILS entry (5 nm) a 3 degree path is at 1,592 ft, 4.5 degrees 2,391 ft and 6
  degrees 3,193 ft. The manual says the ILS starts within 5 nm below 2,000 ft on p. 67
  and below 4,000 ft on p. 87; the 2,000 ft figure suits 3 degrees, the 4,000 ft one
  (John's rule) admits 6.
- Sink rate (feet per minute) at 130, 145 and 160 kt: 3 degrees 690, 770, 850;
  4.5 degrees 1,040, 1,160, 1,280; 6 degrees 1,380, 1,540, 1,700.
- The manual's landing walk-through (p. 68) says "about 2 nm out ... at about 1,000
  feet", 4.7 degrees, and "about 2,000 feet up when 10 nm out" (1.9 degrees); the two
  disagree, so they are approximations, not a slope. The first sits between 3 and 6.
- No `--ils-glide` switch was added: the angle is one constant, and a 2 to 8 degree
  switch was not asked for once John chose 3.
