//! Independent HIND.SH source endpoint and rigid-gear witnesses.
use super::*;
const LEFT: [usize; 2] = [0x5639, 0x5658];
const RIGHT: [usize; 2] = [0x5677, 0x5696];
const NOSE: [usize; 2] = [0x56b5, 0x56d4];
pub(super) struct Sources {
    gear: Vec<Face>,
}
impl Sources {
    pub(super) fn load(bytes: &[u8]) -> AppResult<Self> {
        let shape = tore_formats::shape::Shape::with_state(bytes, &[(0x7196, 1)].into())?;
        let gear: Vec<_> = shape
            .faces
            .into_iter()
            .filter(|f| {
                LEFT.contains(&f.address) || RIGHT.contains(&f.address) || NOSE.contains(&f.address)
            })
            .collect();
        if gear.len() != 6 {
            return Err("Mi24 probe lacks six reviewed source gear faces".into());
        }
        Ok(Self { gear })
    }
}
pub(super) fn check(
    control: Control,
    value: f64,
    pose: &[Face],
    scale: f32,
    source: &Sources,
    metric: &mut Metrics,
) {
    if !matches!(control, Control::Gear) {
        return;
    }
    let after = keyed(pose);
    let mut left = f32::NEG_INFINITY;
    let mut right = f32::INFINITY;
    for original in &source.gear {
        let actual = after.get(&(original.address, 0));
        if value == 0. {
            metric.reviewed_wheel_failed |= actual.is_some();
            continue;
        }
        let Some(actual) = actual else {
            metric.reviewed_wheel_failed = true;
            continue;
        };
        if actual.positions.len() != original.positions.len() {
            metric.reviewed_wheel_failed = true;
            continue;
        }
        let nose = NOSE.contains(&original.address);
        for (before, point) in original.positions.iter().zip(&actual.positions) {
            if before[2] == -17. && (!nose || before[1] == 43.) {
                metric.max_reviewed_anchor_gap = metric
                    .max_reviewed_anchor_gap
                    .max(distance(*before, *point) * scale);
            }
            if value == 1. {
                metric.reviewed_neutral_mismatch |= before != point;
            }
            if LEFT.contains(&original.address) {
                left = left.max(point[0] * scale);
            }
            if RIGHT.contains(&original.address) {
                right = right.min(point[0] * scale);
            }
            if value < 1e-5 {
                metric.reviewed_wheel_failed |= if nose {
                    point[0].abs() > 1e-3
                        || !(29.999..=43.001).contains(&point[1])
                        || !(-17.001..=-8.999).contains(&point[2])
                } else {
                    !(6.7..=7.1).contains(&point[0].abs())
                        || !(-23.001..=-12.999).contains(&point[1])
                        || !(-17.001..=-2.999).contains(&point[2])
                };
            }
        }
        for i in 0..original.positions.len() {
            for j in i + 1..original.positions.len() {
                metric.reviewed_wheel_rigidity_error = metric.reviewed_wheel_rigidity_error.max(
                    (distance(original.positions[i], original.positions[j])
                        - distance(actual.positions[i], actual.positions[j]))
                    .abs()
                        * scale,
                );
            }
        }
    }
    if value > 0. {
        metric.reviewed_min_wheel_gap = Some(right - left);
        metric.reviewed_wheel_failed |= right - left < 4.
            || right < 2.
            || left > -2.
            || metric.reviewed_wheel_rigidity_error > EPSILON;
        let selected: BTreeMap<_, _> = after
            .into_iter()
            .filter(|(key, _)| {
                LEFT.contains(&key.0) || RIGHT.contains(&key.0) || NOSE.contains(&key.0)
            })
            .collect();
        if let Some(gap) = shared_vertex_gaps(&keyed(&source.gear), &selected, scale).first() {
            metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(gap.gap);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_and_stretched_whole_gear_cards_fail() {
        let gear = [
            (LEFT[0], -7.),
            (LEFT[1], -7.),
            (RIGHT[0], 7.),
            (RIGHT[1], 7.),
            (NOSE[0], 0.),
            (NOSE[1], 0.),
        ]
        .map(|(address, x)| {
            super::super::tests::face(
                address,
                vec![
                    [x, 43., -17.],
                    [x, 38., -17.],
                    [x, 38., -26.],
                    [x, 43., -26.],
                ],
            )
        })
        .to_vec();
        let source = Sources { gear: gear.clone() };
        let mut metric = Metrics::default();
        check(Control::Gear, 1., &gear, 1., &source, &mut metric);
        assert!(!metric.reviewed_wheel_failed);
        let mut metric = Metrics::default();
        check(Control::Gear, 0.5, &gear[..5], 1., &source, &mut metric);
        assert!(metric.reviewed_wheel_failed);
        let mut moved = gear;
        moved[0].positions[2][2] -= 1.;
        let mut metric = Metrics::default();
        check(Control::Gear, 0.5, &moved, 1., &source, &mut metric);
        assert!(metric.reviewed_wheel_failed);
    }
}
