//! The AC-130's aim-point box on the Front View and Other View pages
//! (docs/spec/ac130-linked-guns.md, opinionated, John 2026-10-09): a 7 by 7
//! square on the aim point, a 3 pixel arrowhead at the page edge when the
//! point is outside the picture, and a 5 pixel diamond where the guns will
//! hit while they cannot put rounds on the box. The layout is projected with
//! the page's own camera when the picture is requested, so the marks stay
//! registered with it.
use super::{Raster, SCREEN};
use crate::aim_box::{self, Cue, Layout, Style};

/// The picture's size in page pixels.
const WIDTH: i32 = SCREEN.2;
const HEIGHT: i32 = SCREEN.3;
/// How far in from the picture's edge the arrowhead's tip sits.
const INSET: f64 = 4.;

/// A page pixel, ignoring any that fall outside the picture.
fn put(r: &mut Raster, x: i32, y: i32, color: [u8; 4]) {
    if (0..WIDTH).contains(&x) && (0..HEIGHT).contains(&y) {
        r.rect(x, y, 1, 1, color);
    }
}

fn line(r: &mut Raster, a: (i32, i32), b: (i32, i32), color: [u8; 4]) {
    let n = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
    for i in 0..=n {
        put(
            r,
            a.0 + (b.0 - a.0) * i / n,
            a.1 + (b.1 - a.1) * i / n,
            color,
        );
    }
}

/// Draw the marks of one page in `color`.
pub fn draw(r: &mut Raster, layout: &Layout, color: [u8; 3]) {
    let color = [color[0], color[1], color[2], 255];
    match layout.boxed {
        Cue::On([x, y]) => {
            let (x, y) = (x.round() as i32, y.round() as i32);
            let half = 3;
            match layout.style {
                Style::Free => {
                    // Corner brackets with two pixel arms.
                    for (sx, sy) in [(-1, -1), (1, -1), (1, 1), (-1, 1)] {
                        let corner = (x + sx * half, y + sy * half);
                        line(r, corner, (corner.0 - sx * 2, corner.1), color);
                        line(r, corner, (corner.0, corner.1 - sy * 2), color);
                    }
                }
                Style::Tracked { .. } | Style::Pinned => {
                    for (a, b) in [
                        ((-half, -half), (half, -half)),
                        ((half, -half), (half, half)),
                        ((half, half), (-half, half)),
                        ((-half, half), (-half, -half)),
                    ] {
                        line(r, (x + a.0, y + a.1), (x + b.0, y + b.1), color);
                    }
                    if layout.style == Style::Pinned {
                        put(r, x, y, color);
                    }
                }
            }
        }
        Cue::Edge(direction) => {
            let tip = aim_box::edge_point([f64::from(WIDTH), f64::from(HEIGHT)], INSET, direction);
            for (a, b) in aim_box::chevron_strokes(direction, 3., 2.) {
                line(
                    r,
                    ((tip[0] + a.0).round() as i32, (tip[1] + a.1).round() as i32),
                    ((tip[0] + b.0).round() as i32, (tip[1] + b.1).round() as i32),
                    color,
                );
            }
        }
    }
    if let Some([x, y]) = layout.diamond {
        let (x, y) = (x.round() as i32, y.round() as i32);
        for (a, b) in aim_box::diamond_strokes(2.) {
            line(
                r,
                (x + a.0 as i32, y + a.1 as i32),
                (x + b.0 as i32, y + b.1 as i32),
                color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(r: &Raster) -> Vec<(i32, i32)> {
        (0..HEIGHT)
            .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
            .filter(|&(x, y)| r.at(x, y)[3] != 0)
            .collect()
    }
    fn layout(style: Style, boxed: Cue, diamond: Option<[f64; 2]>) -> Layout {
        Layout {
            style,
            boxed,
            diamond,
        }
    }

    #[test]
    fn a_tracked_box_is_a_seven_pixel_outline() {
        let mut r = Raster::screen();
        draw(
            &mut r,
            &layout(
                Style::Tracked { friendly: false },
                Cue::On([60., 50.]),
                None,
            ),
            [0, 255, 0],
        );
        let pixels = lit(&r);
        assert_eq!(pixels.len(), 24);
        assert_eq!(pixels.iter().map(|p| p.0).min(), Some(57));
        assert_eq!(pixels.iter().map(|p| p.0).max(), Some(63));
        assert_eq!(r.at(60, 50)[3], 0, "the outline leaves its centre open");
    }

    #[test]
    fn a_pinned_box_adds_a_centre_dot() {
        let mut r = Raster::screen();
        draw(
            &mut r,
            &layout(Style::Pinned, Cue::On([60., 50.]), None),
            [0, 255, 0],
        );
        assert_eq!(lit(&r).len(), 25);
        assert_eq!(r.at(60, 50)[3], 255);
    }

    #[test]
    fn a_free_point_is_four_corner_brackets() {
        let mut r = Raster::screen();
        draw(
            &mut r,
            &layout(Style::Free, Cue::On([60., 50.]), None),
            [0, 255, 0],
        );
        let pixels = lit(&r);
        assert_eq!(pixels.len(), 20);
        assert_eq!(r.at(60, 47)[3], 0, "the middle of each side is open");
        assert_eq!(r.at(57, 47)[3], 255);
    }

    #[test]
    fn an_off_page_box_is_an_arrowhead_inside_the_edge() {
        let mut r = Raster::screen();
        draw(
            &mut r,
            &layout(Style::Free, Cue::Edge([1., 0.]), None),
            [0, 255, 0],
        );
        let pixels = lit(&r);
        assert!(!pixels.is_empty());
        let tip_x = pixels.iter().map(|p| p.0).max().unwrap();
        assert_eq!(tip_x, WIDTH - INSET as i32);
        assert!(
            pixels
                .iter()
                .all(|p| p.0 >= tip_x - 3 && (p.1 - HEIGHT / 2).abs() <= 2)
        );
    }

    #[test]
    fn a_diamond_is_five_pixels_across_and_clipped_to_the_picture() {
        let mut r = Raster::screen();
        draw(
            &mut r,
            &layout(Style::Free, Cue::Edge([-1., 0.]), Some([100., 80.])),
            [0, 255, 0],
        );
        let diamond: Vec<_> = lit(&r).into_iter().filter(|p| p.0 > 90).collect();
        assert_eq!(diamond.iter().map(|p| p.0).min(), Some(98));
        assert_eq!(diamond.iter().map(|p| p.0).max(), Some(102));
        assert_eq!(diamond.len(), 8);
        // Nothing outside the picture, even for a mark at its very corner.
        let mut r = Raster::screen();
        draw(
            &mut r,
            &layout(Style::Free, Cue::On([0., 0.]), Some([137., 113.])),
            [0, 255, 0],
        );
        assert!(
            lit(&r)
                .iter()
                .all(|p| (0..WIDTH).contains(&p.0) && (0..HEIGHT).contains(&p.1))
        );
        // Raster::at reads window pixels: none may land on the bezel.
        assert_eq!(r.at(-1, 0)[3], 0);
        assert_eq!(r.at(WIDTH, 0)[3], 0);
    }
}
