//! The score board of a networked flight (slice F2-S; docs/ARCHITECTURE.md,
//! "Scoring"): **K** opens and closes it, retail's Show Player Scores, open
//! to every player (the guide's gap-fill). It shows the newest scores the
//! host sent ([`tore_session::Client::scores`]):
//!
//! - one of retail's three headings, PLAYERS RANKED BY KILLS, KILL RATIO or
//!   TOTAL DAMAGE;
//! - the players in the host's order, each with its rank, callsign, side,
//!   kills, losses, damage (in aircraft) and kill ratio, coloured by side as
//!   the chat window colours them (the viewer's side green, the other red,
//!   no side yet grey) and the viewer's own row marked;
//! - in PvP by sides, each side's kills and losses;
//! - the kill limit, the time left (counted down between messages) and,
//!   once the mission is decided, the winner.
//!
//! It is drawn over the middle of the flight view in the HUD's font, over a
//! translucent dark band as the chat window is (*agent decision*: retail's
//! layout of the board is unknown). The board closes with each new flight.
use crate::flight_canvas::{FlightCanvas, HUD_SCALE};
use crate::net::play::NetFlight;
use crate::net::session::NetSession;
use crate::widgets::tone;
use tore_formats::font::Font;
use tore_session::client::scores::{clock, heading, limit_text, ratio, side_name, winner_text};
use tore_session::settings::Fight;
use tore_session::wire::messages::Scores;
use tore_sim::ai::launch::Side;

/// The board's width, in 640 by 480 layer units (the HUD's own).
const WIDTH: f64 = 360.;
/// Where each column starts, layer units from the board's left edge: rank,
/// pilot, side, kills, losses, damage, ratio.
const COLUMNS: [f64; 7] = [0., 18., 132., 196., 236., 280., 324.];
/// The board's top edge, layer units.
const TOP: f64 = 96.;
/// The translucent band behind the text.
const BACKING: f64 = 0.65;
/// The heading's colour, and the marked row's.
const HEADING: [u8; 3] = [255, 255, 255];
const YOU: [u8; 3] = [255, 226, 120];

/// One line of the board: its cells, each at its column (a single cell
/// spans the board), and its colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub cells: Vec<String>,
    pub colour: [u8; 3],
}

impl Line {
    fn text(text: impl Into<String>, colour: [u8; 3]) -> Self {
        Self {
            cells: vec![text.into()],
            colour,
        }
    }
}

/// K: opens or closes the board of a networked flight. `false` when there
/// is none (single player), so the caller says so.
pub fn toggle(flight: &mut Option<NetFlight>) -> bool {
    match flight {
        Some(flight) => {
            flight.score_board = !flight.score_board;
            true
        }
        None => false,
    }
}

/// The board's lines for `scores` (or none yet), as `you` (the viewer's
/// lobby id) sees them, with `seconds_left` of the time limit.
pub fn lines(scores: Option<&Scores>, you: Option<u8>, seconds_left: Option<u32>) -> Vec<Line> {
    let Some(scores) = scores else {
        return vec![
            Line::text("SCORES", HEADING),
            Line::text("Waiting for the scores", tone::SYSTEM),
        ];
    };
    let viewer = scores
        .players
        .iter()
        .find(|p| Some(p.id) == you)
        .and_then(|p| p.side);
    let colour = |side: Option<Side>| match (side, viewer) {
        (None, _) => tone::SYSTEM,
        (Some(side), Some(viewer)) if side == viewer => tone::OWN_SIDE,
        (Some(_), Some(_)) => tone::ENEMY,
        (Some(Side::Friendly), None) => tone::OWN_SIDE,
        (Some(Side::Enemy), None) => tone::ENEMY,
    };
    let mut lines = vec![
        Line::text(heading(scores.tally), HEADING),
        Line {
            cells: ["", "PILOT", "SIDE", "KILLS", "LOSSES", "DAMAGE", "RATIO"]
                .map(str::to_owned)
                .to_vec(),
            colour: tone::SYSTEM,
        },
    ];
    for (n, player) in scores.players.iter().enumerate() {
        lines.push(Line {
            cells: vec![
                format!("{}", n + 1),
                player.callsign.clone(),
                player
                    .side
                    .map_or("-".to_owned(), |s| side_name(s).to_uppercase()),
                player.kills.to_string(),
                player.losses.to_string(),
                format!("{:.2}", f64::from(player.damage) / 1000.),
                format!("{:.2}", ratio(player.kills, player.losses)),
            ],
            colour: if Some(player.id) == you {
                YOU
            } else {
                colour(player.side)
            },
        });
    }
    if scores.players.is_empty() {
        lines.push(Line::text("No players", tone::SYSTEM));
    }
    if scores.fight == Fight::Sides {
        for (side, tally) in [Side::Friendly, Side::Enemy].into_iter().zip(scores.sides) {
            lines.push(Line {
                cells: vec![
                    String::new(),
                    format!("{} SIDE", side_name(side).to_uppercase()),
                    String::new(),
                    tally.kills.to_string(),
                    tally.losses.to_string(),
                    format!("{:.2}", f64::from(tally.damage) / 1000.),
                    format!("{:.2}", ratio(tally.kills, tally.losses)),
                ],
                colour: colour(Some(side)),
            });
        }
    }
    let mut footer = Vec::new();
    if let Some(limit) = limit_text(scores) {
        footer.push(limit);
    }
    footer.push(match seconds_left {
        Some(left) => format!("Time left {}", clock(left)),
        None => "No time limit".to_owned(),
    });
    lines.push(Line::text(footer.join("    "), tone::SYSTEM));
    if let Some(winner) = winner_text(scores) {
        lines.push(Line::text(winner, HEADING));
    }
    lines
}

/// Draws the board when it is open in a networked flight.
pub(crate) fn draw(
    flight: Option<&NetFlight>,
    session: Option<&NetSession>,
    canvas: &mut FlightCanvas,
    font: &Font,
) {
    let (Some(flight), Some(session)) = (flight, session) else {
        return;
    };
    if !flight.score_board {
        return;
    }
    let client = &session.client;
    let you = client.lobby().map(|lobby| lobby.you);
    draw_lines(
        canvas,
        font,
        &lines(client.scores(), you, client.seconds_left()),
    );
}

/// Draws `lines` centred across the view from [`TOP`], over a band.
pub fn draw_lines(canvas: &mut FlightCanvas, font: &Font, lines: &[Line]) {
    let [w, h] = canvas.size.map(f64::from);
    let layer = (w / 640.).min(h / 480.);
    let scale = layer * HUD_SCALE;
    let line_height = (font.height + 2) as f64 * scale;
    let left = (w - WIDTH * layer) / 2.;
    let top = TOP * layer;
    let pad = 4. * layer;
    darken(
        canvas,
        (
            left - pad,
            top - pad,
            left + WIDTH * layer + pad,
            top + lines.len() as f64 * line_height + pad,
        ),
    );
    for (row, line) in lines.iter().enumerate() {
        let y = top + row as f64 * line_height;
        for (cell, column) in line.cells.iter().zip(COLUMNS) {
            if !cell.is_empty() {
                canvas.text(font, cell, [left + column * layer, y], scale, line.colour);
            }
        }
    }
}

/// Darkens the band (left, top, right, bottom), screen pixels.
fn darken(canvas: &mut FlightCanvas, (left, top, right, bottom): (f64, f64, f64, f64)) {
    let [w, h] = canvas.size.map(|n| n as i32);
    let (x0, x1) = (
        (left.floor() as i32).clamp(0, w),
        (right.ceil() as i32).clamp(0, w),
    );
    let (y0, y1) = (
        (top.floor() as i32).clamp(0, h),
        (bottom.ceil() as i32).clamp(0, h),
    );
    for y in y0..y1 {
        for x in x0..x1 {
            canvas.blend(x, y, [0, 0, 0], BACKING);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_session::settings::{KillOwner, ScoreTally};
    use tore_session::wire::messages::{PlayerScore, SideScore, Winner};

    fn font() -> Font {
        Font {
            height: 8,
            glyphs: (0..256)
                .map(|_| tore_formats::font::Glyph {
                    advance: 6,
                    pixels: vec![(0, 0), (1, 0), (0, 1), (1, 1), (2, 2), (3, 3)],
                })
                .collect(),
        }
    }

    fn player(id: u8, callsign: &str, side: Option<Side>, kills: u32, losses: u32) -> PlayerScore {
        PlayerScore {
            id,
            callsign: callsign.into(),
            side,
            kills,
            losses,
            damage: kills * 1000,
        }
    }

    fn scores() -> Scores {
        Scores {
            tally: ScoreTally::Kills,
            fight: Fight::Sides,
            seconds_left: Some(600),
            kill_limit: 5,
            kill_owner: KillOwner::Side,
            players: vec![
                player(1, "Hawk", Some(Side::Enemy), 4, 1),
                player(0, "Viper", Some(Side::Friendly), 2, 0),
                player(2, "Ghost", None, 0, 0),
            ],
            sides: [
                SideScore {
                    kills: 2,
                    losses: 0,
                    damage: 2000,
                },
                SideScore {
                    kills: 4,
                    losses: 1,
                    damage: 4000,
                },
            ],
            winner: Winner::NoneYet,
        }
    }

    #[test]
    fn the_board_ranks_the_players_with_their_sides_and_the_limits() {
        let scores = scores();
        let lines = lines(Some(&scores), Some(0), Some(545));
        let texts: Vec<Vec<&str>> = lines
            .iter()
            .map(|l| l.cells.iter().map(String::as_str).collect())
            .collect();
        assert_eq!(texts[0], ["PLAYERS RANKED BY KILLS"]);
        assert_eq!(texts[1][1], "PILOT");
        assert_eq!(texts[2], ["1", "Hawk", "ENEMY", "4", "1", "4.00", "4.00"]);
        assert_eq!(
            texts[3],
            ["2", "Viper", "FRIENDLY", "2", "0", "2.00", "2.00"]
        );
        assert_eq!(texts[4][2], "-");
        assert_eq!(texts[5][1], "FRIENDLY SIDE");
        assert_eq!(texts[6][1], "ENEMY SIDE");
        assert_eq!(texts[7], ["First side to 5 kills    Time left 9:05"]);
        assert_eq!(lines.len(), 8, "no winner yet");
        // Viper's view: its own row marked, the enemy red, no side grey.
        assert_eq!(lines[2].colour, tone::ENEMY);
        assert_eq!(lines[3].colour, YOU);
        assert_eq!(lines[4].colour, tone::SYSTEM);
        assert_eq!(lines[5].colour, tone::OWN_SIDE);
        // Hawk's view: its own side is the green one.
        let theirs = super::lines(Some(&scores), Some(1), None);
        assert_eq!(theirs[3].colour, tone::ENEMY);
        assert_eq!(theirs[6].colour, tone::OWN_SIDE);
        assert_eq!(theirs[7].cells, ["First side to 5 kills    No time limit"]);
    }

    #[test]
    fn the_board_names_the_winner_and_a_free_for_all_lists_no_sides() {
        let mut scores = scores();
        scores.fight = Fight::FreeForAll;
        scores.tally = ScoreTally::Ratio;
        scores.winner = Winner::Player(1);
        let lines = lines(Some(&scores), None, Some(0));
        assert_eq!(lines[0].cells, ["PLAYERS RANKED BY KILL RATIO"]);
        assert!(
            lines
                .iter()
                .all(|l| !l.cells.iter().any(|c| c.ends_with(" SIDE")))
        );
        assert_eq!(lines.last().unwrap().cells, ["Hawk wins"]);
        // Before the first message.
        let waiting = super::lines(None, Some(0), None);
        assert_eq!(waiting[1].cells, ["Waiting for the scores"]);
    }

    #[test]
    fn a_headless_render_draws_the_board_in_the_middle_in_its_colours() {
        let mut canvas = FlightCanvas::default();
        canvas.size = [1280, 960];
        canvas.pixels = vec![0; 1280 * 960 * 4];
        let font = font();
        let scores = scores();
        draw_lines(
            &mut canvas,
            &font,
            &lines(Some(&scores), Some(0), Some(545)),
        );
        let mut colours = Vec::new();
        let (mut left, mut right, mut top) = (1280, 0, 960);
        for (i, p) in canvas.pixels.chunks_exact(4).enumerate() {
            if p[3] == 0 {
                continue;
            }
            let (x, y) = (i % 1280, i / 1280);
            left = left.min(x);
            right = right.max(x);
            top = top.min(y);
            if p[3] > 200 && p[..3] != [0, 0, 0] && !colours.contains(&[p[0], p[1], p[2]]) {
                colours.push([p[0], p[1], p[2]]);
            }
        }
        for want in [HEADING, YOU, tone::ENEMY, tone::OWN_SIDE, tone::SYSTEM] {
            assert!(colours.contains(&want), "{want:?} in {colours:?}");
        }
        // Centred: 360 layer units wide at twice the 640 by 480 layer.
        assert!((1280 / 2 - 370..=1280 / 2 - 360).contains(&left), "{left}");
        assert!(
            (1280 / 2 + 360..=1280 / 2 + 370).contains(&right),
            "{right}"
        );
        assert_eq!(top, (TOP * 2. - 8.) as usize);
        // The band is translucent: the view shows through.
        let at = ((top + 1) * 1280 + left + 1) * 4;
        assert!(canvas.pixels[at + 3] > 100 && canvas.pixels[at + 3] < 255);
    }
}
