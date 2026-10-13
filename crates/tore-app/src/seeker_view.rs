//! The target camera's seeker view: when the player fires a seeker weapon
//! (an infrared or laser AGM, [`tore_sim::combat::missiles::SEEKER_VIEW`]),
//! the TARGET CAM page shows the weapon's nose view until it lands
//! (docs/spec/target-window.md, "Seeker view"). Opinionated, John 2026-10-12;
//! the pick of weapons, the field of view and the page are the agent's.
//!
//! Presentation only: it reads the picture (`RenderSnapshot`), which a
//! single-player host, a networked client and a replay all have, so it needs
//! nothing of the simulation and nothing on the wire.
use crate::instruments::{Instruments, gunsight::SightView, targeting::SEEKER_ZOOM_STEP};
use tore_sim::{attitude::Vector, combat::missiles};
use tore_world::snapshot::{ProjectilePose, RenderSnapshot};

/// The seeker head looks at its target while the target stays within this
/// angle of the nose (radians): the 45 degrees of the AGM records' zone 1.
const GIMBAL: f64 = std::f64::consts::FRAC_PI_4;
/// The eye stands this far ahead of the weapon's centre, feet, so a nose
/// camera looks out of the body, not through it (fitted).
const NOSE_FT: f64 = 4.;

/// Which of the player's weapons the page follows, and whether the pilot
/// sent it back to the targeting pod.
#[derive(Clone, Debug, Default)]
pub struct SeekerCam {
    /// The weapon being followed.
    followed: Option<u32>,
    /// The newest weapon the pilot sent back to the pod view: it stays off
    /// the page, and only a newer launch brings the seeker view again.
    dismissed: Option<u32>,
    /// The newest weapon already seen launched, so each launch opens the
    /// window once ([`Self::take_launch`]).
    launched: Option<u32>,
    /// A launch the host has not yet answered.
    new_launch: bool,
}

/// One frame's seeker view.
#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    /// The weapon's id in the picture: the camera hides its body.
    pub id: u32,
    /// The weapon's record name.
    pub weapon: String,
    pub view: SightView,
    /// Where its target stands, when the picture knows it.
    pub target: Option<Vector>,
    /// The distance the page reads: weapon to target, else to the aircraft.
    pub range_ft: f64,
}

impl SeekerCam {
    /// This frame's seeker view of the weapons `player` fired, if any is in
    /// flight and the pilot has not sent it back to the pod.
    ///
    /// The weapon followed is the player's newest seeker weapon in the
    /// picture (ids only grow); a weapon that has landed drops out of the
    /// picture and the page returns to what it showed before. A later
    /// weapon without a seeker picture (a bomb, a HARM) does not take the
    /// page.
    pub fn observe(&mut self, picture: &RenderSnapshot, player: u32, at: Vector) -> Option<Shot> {
        let newest = picture
            .projectiles
            .iter()
            .filter(|p| !p.gun && !p.incoming && p.owner == player)
            .filter(|p| missiles::has_seeker_view(&p.weapon))
            .max_by_key(|p| p.id);
        self.followed = newest.map(|p| p.id);
        if let Some(p) = newest
            && self.launched.is_none_or(|seen| p.id > seen)
        {
            self.launched = Some(p.id);
            self.new_launch = true;
        }
        let p = newest.filter(|p| self.dismissed != Some(p.id))?;
        let target = p.target.and_then(|id| position_of(picture, id));
        let view = view(p, target);
        Some(Shot {
            id: p.id,
            weapon: p.weapon.clone(),
            view,
            target,
            range_ft: distance(p.position, target.unwrap_or(at)),
        })
    }

    /// Whether a seeker weapon of the player's has appeared since the last
    /// call. The host opens the target camera window on it
    /// (docs/spec/target-window.md, "Seeker window"). A launch while the page
    /// is dismissed counts: it brings the seeker view back.
    pub fn take_launch(&mut self) -> bool {
        std::mem::take(&mut self.new_launch)
    }

    /// Open the target camera window on a launch and close it again when the
    /// view ends (docs/spec/target-window.md, "Seeker window"). Call it after
    /// [`Self::observe`] each frame.
    pub fn sync_window(&mut self, instruments: &mut Instruments) {
        if self.take_launch() {
            instruments.open_for_seeker();
        }
        if !self.showing() {
            instruments.close_for_seeker();
        }
    }

    /// Whether the seeker view has anything to show: a seeker weapon of the
    /// player's in flight that the pilot has not sent back to the pod.
    pub fn showing(&self) -> bool {
        self.followed.is_some() && self.followed != self.dismissed
    }

    /// The pilot sent the page back to the targeting pod (the clear
    /// designation key): the weapon being followed leaves the page.
    pub fn dismiss(&mut self) {
        if self.followed.is_some() {
            self.dismissed = self.followed;
        }
    }
}

/// The weapon's nose camera: from just ahead of it, looking at its target
/// while that is inside the seeker's cone, else along the nose.
pub fn view(p: &ProjectilePose, target: Option<Vector>) -> SightView {
    let nose = unit(p.direction);
    let line = target
        .map(|at| unit(std::array::from_fn(|i| at[i] - p.position[i])))
        .filter(|line| dot(*line, nose).clamp(-1., 1.).acos() <= GIMBAL)
        .unwrap_or(nose);
    SightView {
        position: std::array::from_fn(|i| p.position[i] + nose[i] * NOSE_FT),
        yaw: line[0].atan2(line[2]) as f32,
        pitch: line[1].atan2(line[0].hypot(line[2])) as f32,
        zoom: crate::gunsight_view::zoom_factor(SEEKER_ZOOM_STEP),
    }
}

/// Where the picture draws aircraft or surface unit `id`.
fn position_of(picture: &RenderSnapshot, id: u32) -> Option<Vector> {
    picture
        .targets
        .iter()
        .find(|t| t.id == id)
        .map(|t| t.position)
        .or_else(|| {
            picture
                .surface
                .iter()
                .find(|unit| unit.id.0 == id)
                .map(|unit| unit.position)
        })
}

fn dot(a: Vector, b: Vector) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn unit(v: Vector) -> Vector {
    let length = dot(v, v).sqrt();
    if length < 1e-9 {
        [0., 0., 1.]
    } else {
        v.map(|n| n / length)
    }
}
fn distance(a: Vector, b: Vector) -> f64 {
    dot(
        std::array::from_fn::<f64, 3, _>(|i| a[i] - b[i]),
        std::array::from_fn::<f64, 3, _>(|i| a[i] - b[i]),
    )
    .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn missile(id: u32, owner: u32, weapon: &str, at: Vector) -> ProjectilePose {
        ProjectilePose {
            id,
            owner,
            weapon: weapon.into(),
            shape: None,
            gun: false,
            tracer: false,
            position: at,
            previous: at,
            direction: [0., 0., 1.],
            target: None,
            incoming: false,
            speed_f8: 1000 * 256,
        }
    }

    fn picture(projectiles: Vec<ProjectilePose>) -> RenderSnapshot {
        RenderSnapshot {
            tick: 0,
            player: Default::default(),
            targets: vec![],
            projectiles,
            effects: vec![],
            marks: vec![],
            debris: vec![],
            pilots: vec![],
            models: vec![],
            surface: vec![],
        }
    }

    const HERE: Vector = [0., 5_000., 0.];

    #[test]
    fn a_launched_maverick_takes_the_page_until_it_lands() {
        let mut cam = SeekerCam::default();
        assert!(cam.observe(&picture(vec![]), 0, HERE).is_none());
        let flying = picture(vec![missile(3, 0, "AGM65G.JT", [0., 4_900., 600.])]);
        let shot = cam.observe(&flying, 0, HERE).unwrap();
        assert_eq!(shot.weapon, "AGM65G.JT");
        // The eye is a few feet ahead of the weapon, looking along its nose.
        assert!((shot.view.position[2] - 604.).abs() < 1e-9);
        assert!(shot.view.yaw.abs() < 1e-6 && shot.view.pitch.abs() < 1e-6);
        assert!((shot.range_ft - (100f64.powi(2) + 600f64.powi(2)).sqrt()).abs() < 1e-6);
        // Landed: it drops out of the picture and the page is free again.
        assert!(cam.observe(&picture(vec![]), 0, HERE).is_none());
    }

    #[test]
    fn only_the_players_own_seeker_weapons_count() {
        let mut cam = SeekerCam::default();
        let mut incoming = missile(5, 0, "AGM65G.JT", [0., 4_900., 600.]);
        incoming.incoming = true;
        let mut gun = missile(6, 0, "AGM65G.JT", [0., 4_900., 600.]);
        gun.gun = true;
        let others = picture(vec![
            missile(1, 7, "AGM65G.JT", [0., 4_900., 600.]),
            incoming,
            gun,
            // No seeker picture: HARM, a laser bomb, a dumb bomb, an AAM.
            missile(2, 0, "AGM88.JT", [0., 4_900., 600.]),
            missile(3, 0, "GBU10.JT", [0., 4_900., 600.]),
            // The JDAMs fly to a point and carry no picture (John 2026-10-12).
            missile(10, 0, "GBU29.JT", [0., 4_900., 600.]),
            missile(11, 0, "GBU29P.JT", [0., 4_900., 600.]),
            missile(12, 0, "GBU30.JT", [0., 4_900., 600.]),
            missile(4, 0, "MK82.JT", [0., 4_900., 600.]),
            missile(8, 0, "AIM9M.JT", [0., 4_900., 600.]),
        ]);
        assert!(cam.observe(&others, 0, HERE).is_none());
        // Every record the list names is one.
        for weapon in missiles::SEEKER_VIEW {
            let one = picture(vec![missile(1, 0, weapon, [0., 4_900., 600.])]);
            assert!(SeekerCam::default().observe(&one, 0, HERE).is_some());
        }
    }

    #[test]
    fn the_at2_takes_the_page_and_a_jdam_after_it_does_not() {
        // John 2026-10-12: the AT-2 (retail's "optical" Swatter) gets the
        // seeker view, the JDAMs do not.
        let mut cam = SeekerCam::default();
        let at2 = missile(3, 0, "AT2.JT", [0., 4_900., 600.]);
        let jdam = missile(8, 0, "GBU30.JT", [0., 4_900., 700.]);
        let shot = cam.observe(&picture(vec![at2, jdam]), 0, HERE).unwrap();
        assert_eq!((shot.id, shot.weapon.as_str()), (3, "AT2.JT"));
        assert!(missiles::has_seeker_view("AT2.JT"));
        assert!(!missiles::has_seeker_view("GBU29.JT"));
    }

    #[test]
    fn each_launch_is_announced_once_and_the_view_reports_when_it_ends() {
        let mut cam = SeekerCam::default();
        assert!(!cam.take_launch() && !cam.showing());
        let one = missile(3, 0, "AGM65G.JT", [0., 4_900., 600.]);
        cam.observe(&picture(vec![one.clone()]), 0, HERE);
        assert!(cam.take_launch() && cam.showing());
        // Seen again in flight: not a new launch.
        cam.observe(&picture(vec![one.clone()]), 0, HERE);
        assert!(!cam.take_launch());
        // A second weapon is a launch; the first landing is not.
        let two = missile(4, 0, "AT2.JT", [0., 4_900., 700.]);
        cam.observe(&picture(vec![one, two.clone()]), 0, HERE);
        assert!(cam.take_launch());
        cam.observe(&picture(vec![two.clone()]), 0, HERE);
        assert!(!cam.take_launch() && cam.showing());
        // Clear sends the page to the pod: not showing until a new launch.
        cam.dismiss();
        cam.observe(&picture(vec![two.clone()]), 0, HERE);
        assert!(!cam.showing() && !cam.take_launch());
        let three = missile(6, 0, "AS30.JT", [0., 4_900., 800.]);
        cam.observe(&picture(vec![two, three.clone()]), 0, HERE);
        assert!(cam.take_launch() && cam.showing());
        // Landed: the view ends.
        cam.observe(&picture(vec![]), 0, HERE);
        assert!(!cam.showing() && !cam.take_launch());
        // Weapons without a picture never announce anything.
        let bomb = missile(9, 0, "MK82.JT", [0., 4_900., 600.]);
        cam.observe(&picture(vec![bomb]), 0, HERE);
        assert!(!cam.take_launch() && !cam.showing());
    }

    #[test]
    fn a_launch_opens_the_window_and_the_landing_closes_it_unless_the_player_kept_it() {
        use crate::instruments::{Layout, TARGET_PAGE};
        let mut windows = Instruments::new(Layout::Large, None);
        windows.pages = vec![7, 5, 9, 8];
        let mut cam = SeekerCam::default();
        let frame = |cam: &mut SeekerCam, windows: &mut Instruments, p: Vec<ProjectilePose>| {
            cam.observe(&picture(p), 0, HERE);
            cam.sync_window(windows);
        };
        frame(&mut cam, &mut windows, vec![]);
        assert_eq!(windows.pages, [7, 5, 9, 8]);
        let one = missile(3, 0, "AT2.JT", [0., 4_900., 600.]);
        frame(&mut cam, &mut windows, vec![one.clone()]);
        assert_eq!(windows.pages, [7, 5, 9, 8, TARGET_PAGE]);
        // Frames while it flies change nothing, and the player's close sticks.
        frame(&mut cam, &mut windows, vec![one.clone()]);
        windows.toggle(TARGET_PAGE);
        frame(&mut cam, &mut windows, vec![one.clone()]);
        assert_eq!(windows.pages, [7, 5, 9, 8]);
        // The next launch opens it again; the landing of both closes it.
        let two = missile(4, 0, "AGM65G.JT", [0., 4_900., 700.]);
        frame(&mut cam, &mut windows, vec![one, two.clone()]);
        assert_eq!(windows.pages.last(), Some(&TARGET_PAGE));
        frame(&mut cam, &mut windows, vec![two.clone()]);
        assert_eq!(windows.pages.last(), Some(&TARGET_PAGE));
        frame(&mut cam, &mut windows, vec![]);
        assert_eq!(windows.pages, [7, 5, 9, 8]);
        // Clear designation keeps it as the player's own.
        let three = missile(6, 0, "AS14.JT", [0., 4_900., 800.]);
        frame(&mut cam, &mut windows, vec![three.clone()]);
        cam.dismiss();
        windows.keep_seeker_window();
        frame(&mut cam, &mut windows, vec![three]);
        frame(&mut cam, &mut windows, vec![]);
        assert_eq!(windows.pages.last(), Some(&TARGET_PAGE));
        // A window the player already had open stays after the view.
        let mut open = Instruments::new(Layout::Large, None);
        let before = open.pages.clone();
        assert!(before.contains(&TARGET_PAGE));
        let mut cam = SeekerCam::default();
        let four = missile(8, 0, "AGM65A.JT", [0., 4_900., 600.]);
        frame(&mut cam, &mut open, vec![four]);
        frame(&mut cam, &mut open, vec![]);
        assert_eq!(open.pages, before);
    }

    #[test]
    fn the_newest_seeker_weapon_wins_and_a_bomb_after_it_does_not() {
        let mut cam = SeekerCam::default();
        let first = missile(3, 0, "AGM65G.JT", [0., 4_900., 600.]);
        let second = missile(5, 0, "AS30.JT", [0., 4_800., 300.]);
        let bomb = missile(9, 0, "MK82.JT", [0., 4_990., 10.]);
        let shot = cam
            .observe(&picture(vec![first.clone(), second, bomb]), 0, HERE)
            .unwrap();
        assert_eq!(shot.weapon, "AS30.JT");
        // When the newest lands the older one is next.
        let shot = cam.observe(&picture(vec![first]), 0, HERE).unwrap();
        assert_eq!(shot.weapon, "AGM65G.JT");
    }

    #[test]
    fn the_clear_key_sends_the_page_back_to_the_pod_until_the_next_launch() {
        let mut cam = SeekerCam::default();
        let one = missile(3, 0, "AGM65G.JT", [0., 4_900., 600.]);
        assert!(cam.observe(&picture(vec![one.clone()]), 0, HERE).is_some());
        cam.dismiss();
        assert!(cam.observe(&picture(vec![one.clone()]), 0, HERE).is_none());
        // Still flying, still off the page.
        assert!(cam.observe(&picture(vec![one.clone()]), 0, HERE).is_none());
        // A new launch brings the seeker view back.
        let two = missile(4, 0, "AGM65G.JT", [0., 4_900., 700.]);
        assert!(cam.observe(&picture(vec![one, two]), 0, HERE).is_some());
        // The key with nothing in flight does nothing.
        let mut idle = SeekerCam::default();
        idle.dismiss();
        assert!(
            idle.observe(&picture(vec![missile(1, 0, "AS14.JT", HERE)]), 0, HERE)
                .is_some()
        );
    }

    #[test]
    fn the_head_looks_at_its_target_inside_the_cone_and_along_the_nose_outside() {
        let p = missile(3, 0, "AGM65G.JT", [0., 1_000., 0.]);
        // 10 degrees right of the nose: the head turns to it.
        let ten = 10f64.to_radians();
        let near = [5_000. * ten.sin(), 1_000., 5_000. * ten.cos()];
        let v = view(&p, Some(near));
        assert!((f64::from(v.yaw) - ten).abs() < 1e-5);
        let (x, y) = v.project(near).unwrap();
        assert!((x - 69.).abs() < 0.5 && (y - 57.).abs() < 0.5, "{x} {y}");
        // 80 degrees off: beyond the seeker's cone, the nose view.
        let wide = 80f64.to_radians();
        let far = [5_000. * wide.sin(), 1_000., 5_000. * wide.cos()];
        assert!(view(&p, Some(far)).yaw.abs() < 1e-6);
        assert!(view(&p, None).yaw.abs() < 1e-6);
    }
}
