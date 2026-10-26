//! World-owned prediction jobs. Pending work and published observation ages
//! are simulation state, so frame readers cannot spend or change the budget.
use super::{
    energy::{Body, DT, Observation},
    energy_estimate::{Answer, Queue, Request, Trial},
};
use crate::attitude::{Vector, dot, unit};

pub const ENVELOPE_BIT: u64 = 1 << 63;
pub const MISSILE_BIT: u64 = 1 << 62;

/// Probe both sides of the data-driven loft transition. A farther lofted
/// intercept can succeed after an unlofted shot has already become unreachable.
/// Coarse doubling alone can miss that disconnected successful interval.
pub fn envelope_probe_scales(body: Body) -> &'static [f64] {
    if body.loft_height > 0. {
        &[
            0.0625, 0.125, 0.25, 0.5, 0.6, 0.600001, 0.625, 0.65, 0.675, 0.7, 0.75, 1., 1.5, 2., 4.,
        ]
    } else {
        &[0.0625, 0.125, 0.25, 0.5, 0.75, 1., 1.5, 2., 4.]
    }
}

fn norm(v: Vector) -> f64 {
    dot(v, v).sqrt()
}
fn sub(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] - b[i])
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Published {
    pub at: u64,
    pub question: Trial,
    pub answer: Answer,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Client {
    pub id: u64,
    pub target: u32,
    pub touched: u64,
    pub submitted: u64,
    pub question: Trial,
    pub pending: bool,
    pub waiting_since: u64,
    pub result: Option<Published>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Envelope {
    pub maximum: f64,
    pub favorable: Option<(f64, f64)>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RangeSearch {
    pub id: u64,
    pub target: u32,
    pub touched: u64,
    pub submitted: u64,
    pub waiting_since: u64,
    pub template: Trial,
    pub bearing: Vector,
    pub cap: f64,
    pub distance: f64,
    pub low: f64,
    pub high: f64,
    pub phase: u8,
    pub iteration: u8,
    pub run: Option<(f64, f64)>,
    pub best: Option<(f64, f64)>,
    pub result: Option<Envelope>,
    /// The last completed envelope of an earlier geometry, shown while a
    /// restarted search runs so a manoeuvring launcher keeps a range cue.
    pub previous: Option<Envelope>,
    pub done: bool,
}
impl RangeSearch {
    pub fn new(id: u64, target: u32, tick: u64, template: Trial) -> Self {
        let b = template.body;
        let life = f64::from(b.life.saturating_sub(template.start_age)) * DT;
        let impulse = b.boost.thrust * f64::from(b.boost.ticks) * DT
            + b.sustain.thrust * f64::from(b.sustain.ticks) * DT;
        let cap = (norm(template.flight.velocity)
            + impulse / b.dry_mass()
            + norm(template.target.velocity))
            * life
            + 0.5 * super::energy::G * life * life
            + template.radius;
        Self {
            id,
            target,
            touched: tick,
            submitted: tick,
            waiting_since: tick,
            template,
            bearing: unit([
                template.target.position[0] - template.flight.position[0],
                0.,
                template.target.position[2] - template.flight.position[2],
            ]),
            cap,
            distance: 100f64
                .max((template.target.position[1] - template.flight.position[1]).abs() + 0.01)
                .min(cap),
            low: 0.,
            high: cap,
            phase: 0,
            iteration: 0,
            run: None,
            best: None,
            result: None,
            previous: None,
            done: false,
        }
    }
    fn trial(&self) -> Trial {
        let mut trial = self.template;
        let vertical = trial.target.position[1] - trial.flight.position[1];
        let horizontal = (self.distance * self.distance - vertical * vertical)
            .max(0.)
            .sqrt();
        trial.target.position[0] = trial.flight.position[0] + self.bearing[0] * horizontal;
        trial.target.position[2] = trial.flight.position[2] + self.bearing[2] * horizontal;
        trial
    }
    fn accept(&mut self, answer: Answer) {
        match self.phase {
            0 => {
                if answer.hit {
                    self.low = self.distance;
                    self.high = self.cap;
                } else if self.distance > self.low {
                    self.high = self.high.min(self.distance);
                }
                let scales = envelope_probe_scales(self.template.body);
                let anchor = (self.template.body.loft_range / 0.6).max(1600.);
                if usize::from(self.iteration) < scales.len() {
                    self.distance = (anchor * scales[usize::from(self.iteration)])
                        .max(
                            (self.template.target.position[1] - self.template.flight.position[1])
                                .abs()
                                + 0.01,
                        )
                        .min(self.cap);
                    self.iteration += 1;
                } else if answer.hit && self.distance < self.cap {
                    self.distance = (self.distance * 2.).min(self.cap);
                } else if self.low == 0. {
                    self.result = Some(Envelope::default());
                    self.done = true;
                } else {
                    self.phase = 1;
                    self.iteration = 0;
                    self.distance = (self.low + self.high) * 0.5;
                }
            }
            1 => {
                if answer.hit {
                    self.low = self.distance;
                } else {
                    self.high = self.distance;
                }
                self.iteration += 1;
                if self.iteration >= 10 {
                    self.result = Some(Envelope {
                        maximum: self.low,
                        favorable: None,
                    });
                    self.phase = 2;
                    self.iteration = 0;
                    self.distance = self.low * 0.1;
                } else {
                    self.distance = (self.low + self.high) * 0.5;
                }
            }
            _ => {
                let relative = (self.distance / self.low.max(1.)).clamp(0., 1.);
                let life = f64::from(self.template.body.life) * DT;
                let score = 95.
                    * (1. - 0.75 * relative * relative)
                    * (1. - 0.6 * answer.seconds / life).clamp(0., 1.);
                if answer.hit && score >= 70. {
                    self.run = Some(self.run.map_or((self.distance, self.distance), |(lo, _)| {
                        (lo, self.distance)
                    }));
                    let run = self.run.unwrap();
                    if run.1 > run.0 && self.best.is_none_or(|best| run.1 - run.0 > best.1 - best.0)
                    {
                        self.best = Some(run);
                    }
                } else {
                    self.run = None;
                }
                self.iteration += 1;
                if self.iteration == 17 {
                    self.done = true;
                    self.result = Some(Envelope {
                        maximum: self.low,
                        favorable: self.best,
                    });
                } else {
                    self.distance = self.low * (0.1 + 0.9 * f64::from(self.iteration) / 16.);
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Service {
    pub queue: Queue,
    pub clients: Vec<Client>,
    pub ranges: Vec<RangeSearch>,
    /// Admission rotates across waiting point and range clients, never actor order.
    pub admission_cursor: usize,
}
impl Service {
    pub fn request(&mut self, id: u64, target: u32, tick: u64, question: Trial, refresh: u64) {
        // Same-aircraft rails can share a qualified recent answer. They already
        // use the same geometric compatibility tolerance; duplicating identical
        // flyouts on every rail delays fresh answers in a turning fight.
        let reused = (id & MISSILE_BIT == 0 && question.flight.age == 0)
            .then(|| {
                self.clients
                    .iter()
                    .filter(|c| c.id != id && c.id >> 16 == id >> 16 && c.target == target)
                    .filter_map(|c| c.result)
                    .filter(|r| {
                        (tick.saturating_sub(r.at) < refresh || r.question == question)
                            && compatible(r.question, question)
                    })
                    .max_by_key(|r| r.at)
            })
            .flatten();
        if let Some(result) = reused {
            self.queue.remove(id);
            let replacement = Client {
                id,
                target,
                touched: tick,
                submitted: result.at,
                question,
                pending: false,
                waiting_since: tick,
                result: Some(result),
            };
            if let Some(client) = self.clients.iter_mut().find(|c| c.id == id) {
                *client = replacement;
            } else {
                self.clients.push(replacement);
            }
            return;
        }
        if let Some(client) = self.clients.iter_mut().find(|c| c.id == id) {
            client.touched = tick;
            if client.target != target || client.question.body != question.body {
                client.target = target;
                client.pending = false;
                client.result = None;
                self.queue.remove(id);
            }
            if client.pending && !self.queue.requests.iter().any(|r| r.id == id) {
                client.question = question;
                client.submitted = tick;
            }
            // The world terrain is fixed. Identical physical inputs need no
            // new flyout merely because a refresh interval passed.
            if client.result.is_some_and(|r| r.question == question) {
                return;
            }
            // A published answer that no longer matches the question cannot be
            // read (see `answer`), so waiting for the refresh interval only
            // leaves the cue and the AI gate dark. Short shots go stale within
            // a few ticks of launcher motion.
            let stale = client
                .result
                .is_some_and(|r| !compatible(r.question, question));
            if !client.pending
                && (client.result.is_none()
                    || stale
                    || tick.saturating_sub(client.submitted) >= refresh)
            {
                client.question = question;
                client.submitted = tick;
                client.pending = true;
                client.waiting_since = tick;
            }
            return;
        }
        self.clients.push(Client {
            id,
            target,
            touched: tick,
            submitted: tick,
            question,
            pending: true,
            waiting_since: tick,
            result: None,
        });
    }
    pub fn request_range(&mut self, id: u64, target: u32, tick: u64, question: Trial) {
        let id = id | ENVELOPE_BIT;
        if let Some(range) = self.ranges.iter_mut().find(|r| r.id == id) {
            range.touched = tick;
            // A range does not depend on the present distance. Keep the
            // normalized geometry while a long cold search is still running.
            let same_model =
                range.target == target && envelope_model_matches(range.template, question);
            // Finish a cold search before resampling moving geometry. Otherwise
            // a sustained turn can restart it forever without one publication.
            if same_model
                && (range.result.is_none() || envelope_compatible(range.template, question))
            {
                return;
            }
            let previous = same_model
                .then_some(range.result.or(range.previous))
                .flatten();
            self.queue.remove(id);
            *range = RangeSearch::new(id, target, tick, question);
            range.previous = previous;
            return;
        }
        self.ranges
            .push(RangeSearch::new(id, target, tick, question));
    }
    /// Waiting requests are retained even when all active slots are occupied.
    /// Each admitted flyout finishes before its slot is reused. A persistent
    /// round-robin cursor gives every waiting client a turn under saturation.
    fn admit(&mut self) {
        let count = self.clients.len() + self.ranges.len();
        if count == 0 {
            return;
        }
        let start = self.admission_cursor % count;
        for offset in 0..count {
            if self.queue.requests.len() >= super::energy_estimate::MAX_REQUESTS {
                break;
            }
            let index = (start + offset) % count;
            let request = if index < self.clients.len() {
                let c = &self.clients[index];
                c.pending.then_some(Request {
                    id: c.id,
                    observed_tick: c.submitted,
                    trial: c.question,
                })
            } else {
                let r = &self.ranges[index - self.clients.len()];
                (!r.done).then(|| Request {
                    id: r.id,
                    observed_tick: r.submitted,
                    trial: r.trial(),
                })
            };
            if let Some(request) = request
                && !self.queue.requests.iter().any(|r| r.id == request.id)
            {
                self.queue.submit(request);
            }
            self.admission_cursor = (index + 1) % count;
        }
    }
    pub fn answer(&self, id: u64, target: u32, current: Trial) -> Option<Published> {
        let client = self
            .clients
            .iter()
            .find(|c| c.id == id && c.target == target)?;
        let mut result = client.result?;
        if !compatible(result.question, current) {
            return None;
        }
        result.answer.seconds = (result.answer.seconds
            - f64::from(
                current
                    .flight
                    .age
                    .saturating_sub(result.question.flight.age),
            ) * DT)
            .max(0.);
        Some(result)
    }
    pub fn envelope(&self, id: u64, target: u32, current: Trial) -> Option<Envelope> {
        let range = self
            .ranges
            .iter()
            .find(|r| r.id == (id | ENVELOPE_BIT) && r.target == target)?;
        if !envelope_model_matches(range.template, current) {
            return None;
        }
        if envelope_compatible(range.template, current) {
            range.result.or(range.previous)
        } else {
            range.previous
        }
    }
    pub fn advance(&mut self, tick: u64, ground: &impl Fn(f64, f64) -> f64) -> usize {
        self.advance_over(tick, ground, &|_, _| false)
    }
    pub fn advance_over(
        &mut self,
        tick: u64,
        ground: &impl Fn(f64, f64) -> f64,
        clear: &impl Fn(Vector, Vector) -> bool,
    ) -> usize {
        self.admit();
        let used = self.queue.advance_over(ground, clear);
        for client in &mut self.clients {
            if client.pending
                && let Some((at, answer)) = self.queue.answer(client.id)
            {
                client.pending = false;
                client.result = Some(Published {
                    at,
                    question: client.question,
                    answer,
                });
                self.queue.remove(client.id);
            }
        }
        for range in &mut self.ranges {
            if !range.done
                && let Some((_, answer)) = self.queue.answer(range.id)
            {
                range.accept(answer);
                range.waiting_since = tick;
                self.queue.remove(range.id);
            }
        }
        for client in &self.clients {
            if tick.saturating_sub(client.touched) > 240 {
                self.queue.remove(client.id);
            }
        }
        for range in &self.ranges {
            if tick.saturating_sub(range.touched) > 240 {
                self.queue.remove(range.id);
            }
        }
        self.clients
            .retain(|c| tick.saturating_sub(c.touched) <= 240);
        self.ranges
            .retain(|c| tick.saturating_sub(c.touched) <= 240);
        used
    }
}

/// A published answer is explicitly an observation-snapshot estimate, not a
/// proof about future manoeuvres. Reject material changes instead of spending
/// unbounded synchronous work in a frame reader.
fn compatible(a: Trial, b: Trial) -> bool {
    let ra = sub(a.target.position, a.flight.position);
    let rb = sub(b.target.position, b.flight.position);
    let elapsed = f64::from(b.flight.age.saturating_sub(a.flight.age)) * DT;
    let projected: Vector = std::array::from_fn(|i| {
        ra[i]
            + 0.5
                * (a.target.velocity[i] + b.target.velocity[i]
                    - a.flight.velocity[i]
                    - b.flight.velocity[i])
                * elapsed
    });
    let position: Vector = std::array::from_fn(|i| {
        a.flight.position[i] + 0.5 * (a.flight.velocity[i] + b.flight.velocity[i]) * elapsed
    });
    let tolerance = 30f64.max(norm(ra) * 0.02);
    a.body == b.body
        && a.radius == b.radius
        && a.arming_age == b.arming_age
        && a.wind == b.wind
        && b.flight.age >= a.flight.age
        && b.flight.age - a.flight.age <= if a.body.cruise_drag { 3600 } else { 600 }
        && a.body.motor(a.flight.age).map(|(motor, _)| motor)
            == b.body.motor(b.flight.age).map(|(motor, _)| motor)
        && (a.body.mass_at(a.flight.age) - b.body.mass_at(b.flight.age)).abs() <= a.body.mass * 0.02
        && dot(a.flight.axis, b.flight.axis) >= 1f64.to_radians().cos()
        && norm(sub(projected, rb)) <= tolerance
        && norm(sub(position, b.flight.position)) <= tolerance
        && (position[1] - b.flight.position[1]).abs() <= 30.
        && norm(sub(a.flight.velocity, b.flight.velocity)) <= 10.
        && norm(sub(a.target.velocity, b.target.velocity)) <= 10.
}

fn envelope_model_matches(a: Trial, b: Trial) -> bool {
    a.body == b.body && a.radius == b.radius && a.arming_age == b.arming_age && a.wind == b.wind
}

fn envelope_compatible(a: Trial, b: Trial) -> bool {
    // The launch axis only shapes the first second of a range-scale flight;
    // a 1 degree tolerance restarted every search during gentle turns.
    envelope_model_matches(a, b)
        && norm(sub(a.flight.position, b.flight.position)) <= 1000.
        && dot(a.flight.axis, b.flight.axis) >= 5f64.to_radians().cos()
        && (norm(a.flight.velocity) - norm(b.flight.velocity)).abs() <= 10.
        && dot(unit(a.flight.velocity), unit(b.flight.velocity)) >= 5f64.to_radians().cos()
        && norm(sub(a.target.velocity, b.target.velocity)) <= 25.
        && (a.flight.position[1] - b.flight.position[1]).abs() <= 150.
        && (a.target.position[1] - b.target.position[1]).abs() <= 150.
        && dot(
            unit([
                a.target.position[0] - a.flight.position[0],
                0.,
                a.target.position[2] - a.flight.position[2],
            ]),
            unit([
                b.target.position[0] - b.flight.position[0],
                0.,
                b.target.position[2] - b.flight.position[2],
            ]),
        ) >= 3f64.to_radians().cos()
}

pub fn launch_question(
    w: &tore_formats::weapons::Weapon,
    launcher: super::super::live::Launcher,
    target: super::seeker::Observation,
    radius_ft: f64,
    wind: Vector,
) -> Trial {
    use super::energy::FT;
    let motion = super::Motion::launch_rules(
        w,
        launcher.velocity,
        launcher.position[1],
        super::Rules::Energy,
    );
    let energy = motion
        .energy
        .expect("a powered guided weapon has energy motion");
    let mut flight = energy.flight;
    flight.position = launcher.position.map(|v| v * FT);
    flight.axis = energy.body.launch_axis(launcher.basis.forward);
    let mut trial = Trial::new(
        energy.body,
        flight,
        Observation {
            position: target.position.map(|v| v * FT),
            velocity: target.velocity.map(|v| v * FT),
        },
        radius_ft * FT,
        u32::from(w.damage.fuze_arm_t) * 30,
    );
    trial.wind = wind.map(|v| v * FT);
    trial
}

crate::checkpoint_struct!(Published {
    at,
    question,
    answer
});
crate::checkpoint_struct!(Client {
    id,
    target,
    touched,
    submitted,
    question,
    pending,
    waiting_since,
    result
});
crate::checkpoint_struct!(Envelope { maximum, favorable });
crate::checkpoint_struct!(RangeSearch {
    id,
    target,
    touched,
    submitted,
    waiting_since,
    template,
    bearing,
    cap,
    distance,
    low,
    high,
    phase,
    iteration,
    run,
    best,
    result,
    previous,
    done
});
crate::checkpoint_struct!(Service {
    queue,
    clients,
    ranges,
    admission_cursor
});

#[cfg(test)]
mod tests {
    use super::super::energy::{Flight, Observation};
    use super::*;
    fn question() -> Trial {
        Trial::new(
            super::super::energy::tests::body(),
            Flight::launch([0., 5000., 0.], [0., 0., 300.], [0., 0., 1.]),
            Observation {
                position: [0., 5000., 15000.],
                velocity: [0., 0., -250.],
            },
            30.,
            0,
        )
    }
    #[test]
    fn the_range_search_finds_a_disconnected_success_band_after_loft_starts() {
        let mut q = question();
        q.body.loft_height = 1000.;
        q.body.loft_range = 6000.;
        let mut search = RangeSearch::new(1, 2, 0, q);
        for _ in 0..100 {
            if search.done {
                break;
            }
            let hit = (1000. ..=5000.).contains(&search.distance)
                || (6100. ..=6500.).contains(&search.distance);
            search.accept(Answer {
                hit,
                seconds: 10.,
                closest: if hit { 0. } else { 100. },
                impact_speed: 300.,
            });
        }
        assert!(search.done);
        let maximum = search.result.unwrap().maximum;
        assert!((6499. ..=6500.).contains(&maximum), "{maximum}");
    }
    #[test]
    fn published_jobs_survive_checkpoint_and_readers_do_not_advance_them() {
        let q = question();
        let expected = q.finish(&|_, _| 0.);
        let mut service = Service::default();
        service.request(1, 2, 0, q, 30);
        service.advance(1, &|_, _| 0.);
        let mut copy =
            crate::checkpoint::round_trip(&service, &crate::checkpoint::Models::default()).unwrap();
        for tick in 2..200 {
            assert_eq!(
                service.advance(tick, &|_, _| 0.),
                copy.advance(tick, &|_, _| 0.)
            );
        }
        assert_eq!(service, copy);
        for _ in 0..100 {
            assert_eq!(service.answer(1, 2, q).unwrap().answer, expected);
        }
        assert_eq!(service, copy);
        service.request(1, 2, 400, q, 30);
        assert!(
            service.queue.requests.is_empty(),
            "unchanged input reuses its completed result"
        );
        let mut turned = q;
        turned.flight.axis = [1., 0., 0.];
        assert!(service.answer(1, 2, turned).is_none());
        let mut far = q;
        far.target.position[2] += 1000.;
        assert!(service.answer(1, 2, far).is_none());
        assert!(service.answer(1, 3, q).is_none());
    }
    /// A short shot's launch question changes faster than the refresh
    /// interval. The published answer must be renewed as soon as it no
    /// longer matches, or the cue and the AI launch gate stay dark.
    #[test]
    fn saturation_serves_every_client_even_when_early_clients_refresh() {
        let mut service = Service::default();
        let count = super::super::energy_estimate::MAX_REQUESTS + 64;
        let mut seen = vec![false; count];
        for tick in 0..4000 {
            for (id, seen) in seen.iter_mut().enumerate() {
                let mut q = question();
                q.body.life = 30;
                q.target.position[2] += tick as f64;
                service.request(id as u64, id as u32 + 2, tick, q, 1);
                *seen |= service.answer(id as u64, id as u32 + 2, q).is_some();
            }
            service.advance(tick, &|_, _| 0.);
            if seen.iter().all(|v| *v) {
                break;
            }
        }
        assert!(
            seen.iter().all(|v| *v),
            "starved {}",
            seen.iter().filter(|v| !**v).count()
        );
        assert_eq!(service.clients.len(), count);
    }
    #[test]
    fn neighboring_rails_reuse_only_a_recent_compatible_answer_for_the_same_target() {
        let mut q = question();
        q.body.life = 1;
        let mut service = Service::default();
        service.request(1, 2, 0, q, 30);
        service.advance(0, &|_, _| 0.);
        let mut nearby = q;
        nearby.flight.position[0] += 2.;
        service.request(2, 2, 1, nearby, 30);
        assert!(service.answer(2, 2, nearby).is_some());
        assert!(service.queue.requests.is_empty());
        service.request(3, 3, 1, nearby, 30);
        assert!(service.answer(3, 3, nearby).is_none());
        service.request(4, 2, 40, nearby, 30);
        assert!(
            service.answer(4, 2, nearby).is_none(),
            "old geometry is not a fresh neighboring-rail answer"
        );
    }
    #[test]
    fn a_moving_short_range_launch_question_stays_answered() {
        let mut q = question();
        q.target.position[2] = 3000.;
        let mut service = Service::default();
        let mut answered = 0;
        for tick in 0..480u64 {
            let t = tick as f64 * DT;
            let mut current = q;
            for i in 0..3 {
                current.flight.position[i] += q.flight.velocity[i] * t;
                current.target.position[i] += q.target.velocity[i] * t;
            }
            service.request(1, 2, tick, current, 30);
            service.advance(tick, &|_, _| 0.);
            answered += usize::from(service.answer(1, 2, current).is_some());
        }
        assert!(answered >= 432, "answered {answered} of 480 ticks");
    }
    #[test]
    fn a_continuous_fast_turn_cannot_restart_a_cold_search_forever() {
        let mut service = Service::default();
        let mut shown = 0;
        for tick in 0..480u64 {
            let heading = (45f64 * tick as f64 * DT).to_radians();
            let mut q = question();
            q.flight.axis = [heading.sin(), 0., heading.cos()];
            q.flight.velocity = q.flight.axis.map(|v| v * 300.);
            service.request_range(1, 2, tick, q);
            assert!(
                service.advance(tick, &|_, _| 0.) <= super::super::energy_estimate::STEP_BUDGET
            );
            shown += usize::from(service.envelope(1, 2, q).is_some());
        }
        assert!(shown >= 240, "range shown {shown} of 480 ticks");
        let mut other = question();
        other.wind[0] = 30.;
        service.request_range(1, 3, 480, other);
        assert!(
            service.envelope(1, 3, other).is_none(),
            "a different target/model must not inherit the old cue"
        );
    }
    #[test]
    fn a_turning_launcher_keeps_its_range_envelope() {
        let mut service = Service::default();
        let mut shown = 0;
        for tick in 0..1200u64 {
            let heading = (3f64 * tick as f64 * DT).to_radians();
            let mut q = question();
            q.flight.axis = [heading.sin(), 0., heading.cos()];
            q.flight.velocity = q.flight.axis.map(|v| v * 300.);
            service.request_range(1, 2, tick, q);
            service.advance(tick, &|_, _| 0.);
            shown += usize::from(service.envelope(1, 2, q).is_some());
        }
        assert!(shown >= 600, "envelope shown {shown} of 1200 ticks");
    }
    #[test]
    fn a_stable_inflight_estimate_ages_with_observed_relative_motion() {
        let mut q = question();
        q.flight.age = 1200;
        q.start_age = 1200;
        let mut current = q;
        current.flight.age += 300;
        for i in 0..3 {
            current.flight.position[i] += q.flight.velocity[i] * 2.5;
            current.target.position[i] += q.target.velocity[i] * 2.5;
        }
        assert!(compatible(q, current));
        current.flight.position[0] += 1000.;
        assert!(!compatible(q, current));
        current = q;
        current.flight.age = 1801;
        assert!(!compatible(q, current));
    }
    #[test]
    fn changing_surface_range_keeps_the_target_above_the_same_ground() {
        let mut q = question();
        q.target.position = [0., 10., 10000.];
        let mut search = RangeSearch::new(1, 2, 0, q);
        search.distance = 20000.;
        let far = search.trial();
        assert_eq!(far.target.position[1], 10.);
        assert!((norm(sub(far.target.position, far.flight.position)) - 20000.).abs() < 1e-8);
    }
    #[test]
    fn range_publishes_maximum_before_the_optional_band_finishes() {
        let mut search = RangeSearch::new(1, 2, 0, question());
        search.phase = 1;
        search.iteration = 9;
        search.low = 1000.;
        search.high = 2000.;
        search.distance = 1500.;
        search.accept(Answer {
            hit: true,
            seconds: 3.,
            closest: 0.,
            impact_speed: 700.,
        });
        assert_eq!(search.result.unwrap().maximum, 1500.);
        assert!(!search.done);
        for _ in 0..17 {
            search.accept(Answer {
                hit: true,
                seconds: 3.,
                closest: 0.,
                impact_speed: 700.,
            });
        }
        assert!(search.done && search.result.unwrap().favorable.is_some());
    }
}
