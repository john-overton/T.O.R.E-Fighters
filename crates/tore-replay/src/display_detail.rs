//! Display-only facts sampled from the live cockpit, without simulation types.
use crate::{Event, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct TargetSample {
    pub position: [f64; 3],
    pub velocity: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImpactSample {
    pub point: [f64; 3],
    pub range_ft: f64,
    pub spent: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RadarContact {
    pub position: [f64; 3],
    /// 0 dot, 1 cross, 2 square.
    pub symbol: u8,
    pub selected: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SightDetail {
    pub impacts: [Option<ImpactSample>; 3],
    pub gimbal_limit: bool,
    pub mark: Option<[f64; 3]>,
    pub radar: Vec<RadarContact>,
}

pub(super) fn put_point(mut event: Event, prefix: &str, point: [f64; 3]) -> Event {
    for (axis, value) in ["x", "y", "z"].into_iter().zip(point) {
        event = event.with(&format!("{prefix}_{axis}"), value);
    }
    event
}

pub(super) fn point(event: &Event, prefix: &str) -> Option<[f64; 3]> {
    let p = [
        event.num(&format!("{prefix}_x"))?,
        event.num(&format!("{prefix}_y"))?,
        event.num(&format!("{prefix}_z"))?,
    ];
    p.iter().all(|v| v.is_finite()).then_some(p)
}

impl TargetSample {
    pub(super) fn write(&self, event: Event) -> Event {
        put_point(
            put_point(event, "target", self.position),
            "velocity",
            self.velocity,
        )
    }
    pub(super) fn parse(event: &Event) -> Option<Self> {
        Some(Self {
            position: point(event, "target")?,
            velocity: point(event, "velocity")?,
        })
    }
}

impl SightDetail {
    pub(super) fn write(&self, mut event: Event) -> Event {
        event = event
            .with("display_24hz", true)
            .with("gimbal_limit", self.gimbal_limit);
        for (i, impact) in self.impacts.iter().enumerate() {
            if let Some(impact) = impact {
                event = put_point(event, &format!("impact{i}"), impact.point)
                    .with(&format!("impact{i}_range"), impact.range_ft)
                    .with(&format!("impact{i}_spent"), impact.spent);
            }
        }
        if let Some(mark) = self.mark {
            event = put_point(event, "mark", mark);
        }
        // Four u32 words per contact: f32 world coordinates and symbol/selection.
        // Radar glyphs only need plan-view accuracy. Pippers retain f64 positions.
        let dots: Vec<u32> = self
            .radar
            .iter()
            .take(128)
            .flat_map(|dot| {
                [
                    (dot.position[0] as f32).to_bits(),
                    (dot.position[1] as f32).to_bits(),
                    (dot.position[2] as f32).to_bits(),
                    u32::from(dot.symbol) | (u32::from(dot.selected) << 8),
                ]
            })
            .collect();
        event.with("radar_dots", Value::Ids(dots))
    }
    pub(super) fn parse(event: &Event) -> Option<Self> {
        if event.flag("display_24hz") != Some(true) {
            return None;
        }
        let radar = match event.get("radar_dots") {
            Some(Value::Ids(words)) => words
                .chunks_exact(4)
                .take(128)
                .filter_map(|w| {
                    let position = [w[0], w[1], w[2]].map(|v| f64::from(f32::from_bits(v)));
                    (position.iter().all(|v| v.is_finite()) && w[3] & 0xff <= 2).then_some(
                        RadarContact {
                            position,
                            symbol: (w[3] & 0xff) as u8,
                            selected: w[3] & 256 != 0,
                        },
                    )
                })
                .collect(),
            _ => Vec::new(),
        };
        Some(Self {
            impacts: std::array::from_fn(|i| {
                Some(ImpactSample {
                    point: point(event, &format!("impact{i}"))?,
                    range_ft: event
                        .num(&format!("impact{i}_range"))
                        .filter(|r| r.is_finite() && *r >= 0.)?,
                    spent: event.flag(&format!("impact{i}_spent")).unwrap_or(false),
                })
            }),
            gimbal_limit: event.flag("gimbal_limit").unwrap_or(false),
            mark: point(event, "mark"),
            radar,
        })
    }
}
