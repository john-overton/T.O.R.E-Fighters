//! Bounded prediction integration. Frame readers only consume published work.
use super::*;
use missiles::{energy::FT, energy_estimate::Trial, prediction};

fn key(owner: u32, station: usize) -> u64 {
    (u64::from(owner) << 16) | (station.min(65535) as u64)
}

fn raw_observation(
    own: &Ownship,
    launcher: Launcher,
    targets: &[Target],
) -> Option<seeker::Observation> {
    if let Some(point) = own.guided_sight.and_then(|s| s.aim) {
        let delta = missiles::sub(point, launcher.position);
        return Some(seeker::Observation {
            id: u32::MAX,
            position: point,
            velocity: own
                .guided_sight
                .filter(|s| s.kind == Guidance::Command)
                .and_then(|_| own.cockpit.target_object())
                .and_then(|id| targets.iter().find(|t| t.id == id))
                .map_or([0.; 3], |t| t.velocity),
            quality: 1.,
            range: missiles::length(delta),
            off_axis: 0.,
        });
    }
    if own.launch_mode == LaunchMode::Boresight {
        return own.bore_observation;
    }
    own.mounted.observation.or_else(|| {
        let id = own.designated()?;
        let o = own.sensors.observation(id)?;
        let delta = missiles::sub(o.position, launcher.position);
        Some(seeker::Observation {
            id,
            position: o.position,
            velocity: o.velocity,
            quality: 1.,
            range: missiles::length(delta),
            off_axis: dot(unit(delta), launcher.basis.forward)
                .clamp(-1., 1.)
                .acos(),
        })
    })
}
fn prediction_radius(w: &Weapon) -> f64 {
    if super::super::guided::kind(w).is_some() {
        super::super::guided::HIT_FT
    } else {
        f64::from(w.damage.fuze_radius.max(0)) + AIRCRAFT_RADIUS_FT
    }
}
fn powered(w: &Weapon) -> bool {
    w.movement.fuel_t > w.movement.ignite_t
}
impl State {
    /// Motion of a designated command target, already owned by the strike cockpit.
    pub fn guided_point_velocity(&self, owner: u32, kind: Guidance) -> Vector {
        if kind != Guidance::Command {
            return [0.; 3];
        }
        self.strike_cockpit(owner)
            .and_then(|c| c.target_object())
            .and_then(|id| self.targets.iter().find(|t| t.id == id))
            .map_or([0.; 3], |t| t.velocity)
    }

    /// Optional model-based timing for a threat already perceived by the caller.
    /// Pending and defeated missiles have no answer. This does not disclose new threats.
    pub fn missile_time_to_impact(&self, projectile: u32, target: u32) -> Option<f64> {
        let p = self
            .projectiles
            .iter()
            .find(|p| p.id == projectile && p.target == Some(target))?;
        let flight = p.guidance.as_ref()?;
        flight.solution.map(|s| s.seconds)
    }

    pub fn request_energy_range(
        &mut self,
        owner: u32,
        station: usize,
        w: &Weapon,
        launcher: Launcher,
        observed: seeker::Observation,
    ) {
        if self.weapon_rules != Rules::Energy || !powered(w) {
            return;
        }
        let question = prediction::launch_question(
            w,
            launcher,
            observed,
            prediction_radius(w),
            self.devices.wind,
        );
        self.energy_predictions.request_range(
            key(owner, station),
            observed.id,
            self.tick,
            question,
        );
    }
    /// Admission starvation means a requested point has waited over ten
    /// simulated seconds without an active slot. Completion latency is separate.
    pub fn energy_prediction_admission_stats(&self) -> (u64, usize, u64) {
        let service = &self.energy_predictions;
        let waits = service
            .clients
            .iter()
            .filter(|c| c.pending && !service.queue.requests.iter().any(|r| r.id == c.id))
            .map(|c| self.tick.saturating_sub(c.waiting_since));
        (
            service.queue.refused,
            waits.clone().filter(|wait| *wait > 1200).count(),
            waits.max().unwrap_or(0),
        )
    }
    pub fn energy_prediction_counts(&self) -> (usize, usize, usize) {
        (
            self.energy_predictions.queue.requests.len(),
            self.energy_predictions.clients.len(),
            self.energy_predictions
                .ranges
                .iter()
                .filter(|r| r.result.is_some())
                .count(),
        )
    }
    pub(super) fn energy_guided_output(
        &self,
        owner: u32,
        station: usize,
        w: &Weapon,
        launcher: Launcher,
        mut sight: super::super::guided::GuidedSight,
    ) -> super::super::guided::GuidedSight {
        if self.weapon_rules != Rules::Energy || !powered(w) {
            return sight;
        }
        sight.energy_intercept = Some(false);
        sight.reach = None;
        let Some(point) = sight.aim else {
            sight.in_reach = false;
            return sight;
        };
        let observed = seeker::Observation {
            id: u32::MAX,
            position: point,
            velocity: self.guided_point_velocity(owner, sight.kind),
            quality: 1.,
            range: missiles::length(missiles::sub(point, launcher.position)),
            off_axis: 0.,
        };
        let question = prediction::launch_question(
            w,
            launcher,
            observed,
            prediction_radius(w),
            self.devices.wind,
        );
        let hit = self
            .energy_predictions
            .answer(key(owner, station), u32::MAX, question)
            .is_some_and(|r| r.answer.hit);
        sight.energy_intercept = Some(hit);
        sight.in_reach &= hit;
        sight.reach = self
            .energy_predictions
            .envelope(key(owner, station), u32::MAX, question)
            .map(|e| super::super::guided::Reach {
                minimum_ft: f64::from(w.seeker.zones[1].minimum_range),
                maximum_ft: e.maximum / FT,
            });
        sight
    }
    pub fn request_energy_solution(
        &mut self,
        owner: u32,
        station: usize,
        w: &Weapon,
        launcher: Launcher,
        observed: seeker::Observation,
    ) {
        if self.weapon_rules != Rules::Energy || !powered(w) {
            return;
        }
        let radius = prediction_radius(w);
        let question =
            prediction::launch_question(w, launcher, observed, radius, self.devices.wind);
        self.energy_predictions
            .request(key(owner, station), observed.id, self.tick, question, 30);
    }
    pub fn energy_solution(
        &self,
        owner: u32,
        station: usize,
        w: &Weapon,
        launcher: Launcher,
        observed: seeker::Observation,
    ) -> Option<missiles::Solution> {
        if !powered(w) {
            return None;
        }
        let radius = prediction_radius(w);
        let question =
            prediction::launch_question(w, launcher, observed, radius, self.devices.wind);
        let result = self
            .energy_predictions
            .answer(key(owner, station), observed.id, question)?;
        if !result.answer.hit {
            return None;
        }
        Some(missiles::Solution {
            seconds: result.answer.seconds,
            point: std::array::from_fn(|i| {
                observed.position[i] + observed.velocity[i] * result.answer.seconds
            }),
        })
    }
    pub(super) fn advance_energy_predictions(
        &mut self,
        rows: &[OwnRow],
        ground: &impl Fn(f64, f64) -> f64,
    ) {
        let cpu_start = if self.measure_prediction_cost {
            self.prediction_cpu_clock.and_then(|clock| clock())
        } else {
            None
        };
        let started = self.measure_prediction_cost.then(std::time::Instant::now);
        for row in rows {
            let own = &self.ownships[row.index];
            if !own.armed {
                continue;
            }
            let Some(station) = own.config.stations.get(own.selected) else {
                continue;
            };
            let w = &station.weapon;
            if !powered(w)
                || (missiles::Profile::for_weapon(w).is_none()
                    && super::super::guided::kind(w).is_none())
            {
                continue;
            }
            let Some(observed) = raw_observation(own, row.launcher, &self.targets) else {
                continue;
            };
            let radius = prediction_radius(w);
            let question =
                prediction::launch_question(w, row.launcher, observed, radius, self.devices.wind);
            let id = key(own.aircraft, own.selected);
            self.energy_predictions
                .request(id, observed.id, self.tick, question, 30);
            self.energy_predictions
                .request_range(id, observed.id, self.tick, question);
        }
        for projectile in &self.projectiles {
            let Some(energy) = projectile.motion.and_then(|m| m.energy) else {
                continue;
            };
            let Some(target) = energy.target.filter(|_| {
                energy.observed && energy.decoy.pursuit.is_none() && !energy.flight.lost
            }) else {
                continue;
            };
            let id = projectile
                .target
                .or_else(|| self.guided.get(&projectile.id).map(|_| u32::MAX));
            let Some(id) = id else {
                continue;
            };
            let w = self.weapon(projectile);
            let mut question = Trial::new(
                energy.body,
                energy.flight,
                target,
                (prediction_radius(w)) * FT,
                u32::from(w.damage.fuze_arm_t) * 30,
            );
            question.wind = energy.wind;
            self.energy_predictions.request(
                prediction::MISSILE_BIT | u64::from(projectile.id),
                id,
                self.tick,
                question,
                15,
            );
        }
        let ceiling = self.ground_ceiling.as_deref();
        self.energy_predictions.advance_over(
            self.tick,
            &|x, z| ground(x / FT, z / FT) * FT,
            &|a, b| ceiling.is_some_and(|c| c.clear_above(a.map(|v| v / FT), b.map(|v| v / FT))),
        );
        for row in rows {
            let own = &mut self.ownships[row.index];
            own.energy_hit = None;
            own.range_estimate = None;
            if !own.armed {
                continue;
            }
            let Some(w) = own.config.stations.get(own.selected).map(|s| &s.weapon) else {
                continue;
            };
            if !powered(w)
                || (missiles::Profile::for_weapon(w).is_none()
                    && super::super::guided::kind(w).is_none())
            {
                continue;
            }
            let Some(observed) = raw_observation(own, row.launcher, &self.targets) else {
                continue;
            };
            let question = prediction::launch_question(
                w,
                row.launcher,
                observed,
                prediction_radius(w),
                self.devices.wind,
            );
            let id = key(own.aircraft, own.selected);
            own.energy_hit = self
                .energy_predictions
                .answer(id, observed.id, question)
                .filter(|r| r.answer.hit)
                .map(|_| observed.id);
            if let Some(sight) = &mut own.guided_sight {
                let hit = own.energy_hit == Some(observed.id);
                sight.energy_intercept = Some(hit);
                sight.in_reach &= hit;
                sight.reach = self
                    .energy_predictions
                    .envelope(id, observed.id, question)
                    .map(|e| super::super::guided::Reach {
                        minimum_ft: f64::from(w.seeker.zones[1].minimum_range),
                        maximum_ft: e.maximum / FT,
                    });
            }
            if let Some(envelope) = self.energy_predictions.envelope(id, observed.id, question) {
                own.range_estimate = Some(RangeEstimate {
                    station: own.selected,
                    target: observed.id,
                    mode: own.launch_mode,
                    maximum: envelope.maximum / FT,
                    favorable: envelope
                        .favorable
                        .map(|(minimum, maximum)| missiles::FiringBand {
                            minimum: minimum / FT,
                            maximum: maximum / FT,
                        }),
                });
            }
        }
        for projectile in &mut self.projectiles {
            let Some(energy) = projectile.motion.and_then(|m| m.energy) else {
                continue;
            };
            let guided = self.guided.get_mut(&projectile.id);
            let id = projectile
                .target
                .or_else(|| guided.as_ref().map(|_| u32::MAX));
            let result = energy.target.zip(id).and_then(|(target, id)| {
                if !energy.observed || energy.decoy.pursuit.is_some() || energy.flight.lost {
                    return None;
                }
                let client = self
                    .energy_predictions
                    .clients
                    .iter()
                    .find(|c| c.id == (prediction::MISSILE_BIT | u64::from(projectile.id)))?;
                let mut question = client.question;
                question.flight = energy.flight;
                question.target = target;
                question.wind = energy.wind;
                let result = self
                    .energy_predictions
                    .answer(client.id, id, question)
                    .filter(|r| r.answer.hit)?;
                Some(missiles::Solution {
                    seconds: result.answer.seconds,
                    point: std::array::from_fn(|i| {
                        (target.position[i] + target.velocity[i] * result.answer.seconds) / FT
                    }),
                })
            });
            if let Some(flight) = &mut projectile.guidance {
                flight.solution = result;
            }
            if let Some(flight) = guided {
                flight.seconds = result.map(|r| r.seconds);
            }
        }
        if let Some(started) = started {
            self.prediction_cost_ns = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
            self.prediction_cpu_cost_ns = cpu_start.and_then(|from| {
                self.prediction_cpu_clock
                    .and_then(|clock| clock())
                    .and_then(|to| to.checked_sub(from))
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn weapon(source: &str, signature: u8) -> Weapon {
        let mut w = super::super::tests::fixture(true)
            .own()
            .configuration()
            .stations[0]
            .weapon
            .clone();
        w.source = source.into();
        w.flags = 0x1204f;
        w.seeker.signature = signature;
        w.seeker.all_aspect = 1;
        w.seeker.chaff_flare_chance = 50;
        w.damage.fuze_arm_t = 0;
        w.damage.fuze_radius = 100;
        w
    }
    fn projectile(w: &Weapon, target: &Target) -> Projectile {
        let position = [0., target.position[1], 0.];
        let velocity = [0., 0., 300. / FT];
        let observation = seeker::Observation {
            id: target.id,
            position: target.position,
            velocity: target.velocity,
            quality: 1.,
            off_axis: 0.,
            range: missiles::length(missiles::sub(target.position, position)),
        };
        let profile = missiles::Profile::for_weapon(w).unwrap();
        let mut guidance =
            Flight::from_supported_launch(profile, LaunchMode::Cued, observation, position);
        guidance.enabled = true;
        guidance.seeker.acquired = true;
        guidance.seeker.observation = Some(observation);
        guidance.seeker.status = Status::Locked;
        let mut motion = Motion::launch_rules(w, velocity, position[1], Rules::Energy);
        motion.prepare(position, [0., 0., 1.], 0, [0.; 3]);
        Projectile {
            id: 7,
            owner: 0,
            weapon: Some(w.clone()),
            guidance: Some(guidance),
            motion: Some(motion),
            guidance_ticks: Some(profile.guidance_ticks),
            age: 0,
            incoming: None,
            station: 0,
            position,
            previous: position,
            direction: [0., 0., 1.],
            speed_f8: (300. / FT * 256.) as i32,
            launched_t: 0,
            target: Some(target.id),
            fall: FallState::default(),
            gun_round: None,
            tracer: false,
        }
    }
    #[test]
    fn live_decoy_odds_respond_to_heat_aspect_and_imaging() {
        let mut target = super::super::tests::target(100, [0., 10000., 1000.], 10000, 0x80);
        target.signature.infrared = 100.;
        let mut scene = missiles::decoys::Scene::default();
        let release = super::super::super::countermeasures::Release {
            position: target.position,
            velocity: [0.; 3],
            basis: Basis::new(0., 0., 0.),
        };
        scene.release(
            missiles::decoys::Device::from_release(
                1,
                target.id,
                missiles::decoys::Kind::Flare,
                release,
                0,
            ),
            0,
        );
        for _ in 0..12 {
            scene.step([0.; 3], &|_, _| 0.);
        }
        let threshold = |target: &Target, source: &str| {
            let w = weapon(source, 2);
            let mut p = projectile(&w, target);
            energy_decoy(&mut p, &w, target, &scene, 123, &|_, _| false)
                .unwrap()
                .threshold
        };
        target.heat = Heat::Engine {
            on: true,
            throttle: 1.,
            afterburner: false,
        };
        let tail = threshold(&target, "AIM9M.JT");
        target.basis = Basis::new(std::f64::consts::FRAC_PI_2, 0., 0.);
        let beam = threshold(&target, "AIM9M.JT");
        target.heat = Heat::Engine {
            on: false,
            throttle: 0.,
            afterburner: false,
        };
        let cold = threshold(&target, "AIM9M.JT");
        assert!(tail < beam && beam < cold, "{tail}/{beam}/{cold}");
        assert!(threshold(&target, "AIM9X.JT") < tail);
        let w = weapon("AIM9M.JT", 2);
        let profile = missiles::Profile::for_weapon(&w).unwrap();
        let view = seeker::View {
            position: [0., 10000., 0.],
            basis: Basis::new(0., 0., 0.),
            cap: None,
            obscured: &|_, _| false,
        };
        assert!(
            seeker::observe_energy(&w, profile, &view, &target).is_some_and(|o| o.quality >= 0.25),
            "nearby cold target stays measurable"
        );
    }
    #[test]
    fn a_terminal_notch_cannot_be_bypassed_by_fresh_launcher_coordinates() {
        for source in ["AIM120.JT", "AIM7.JT"] {
            let mut w = weapon(source, 3);
            w.seeker.zones[0].maximum_range = 100_000;
            if source == "AIM7.JT" {
                w.flags |= 0x200;
            }
            let mut target =
                super::super::tests::target(100, [0., 1000. / FT, 5000. / FT], 10000, 0x80);
            target.velocity = [100. / FT, 0., 0.];
            let mut p = projectile(&w, &target);
            let remembered = [0., 1000. / FT, 4900. / FT];
            p.guidance.as_mut().unwrap().last_intercept = Some(remembered);
            p.motion
                .as_mut()
                .unwrap()
                .energy
                .as_mut()
                .unwrap()
                .tracking
                .notched = true;
            let support = ActorSupport {
                owner: 0,
                observation: Some(seeker::Observation {
                    id: target.id,
                    position: target.position,
                    velocity: target.velocity,
                    quality: 1.,
                    off_axis: 0.,
                    range: 5000. / FT,
                }),
                supported: true,
                radar_position: p.position,
                radar_emitting: true,
            };
            guide_owned(
                &mut p,
                &w,
                std::slice::from_ref(&target),
                std::iter::empty(),
                Some(&support),
                &|_, _| false,
                &|_, _| 0.,
            );
            let observation = p.motion.unwrap().energy.unwrap().target.unwrap();
            assert_eq!(observation.position, remembered.map(|v| v * FT), "{source}");
            assert_eq!(observation.velocity, [0.; 3], "{source}");
        }
    }
    #[test]
    fn an_energy_sam_does_not_go_blind_inside_its_launch_minimum() {
        let mut w = weapon("FIM92.JT", 2);
        w.seeker.zones[0].minimum_range = 7500;
        w.seeker.zones[0].maximum_range = 100_000;
        let target = super::super::tests::target(100, [0., 1000., 1000.], 10000, 0x80);
        let profile = missiles::Profile::for_weapon(&w).unwrap();
        let view = seeker::View {
            position: [0., 1000., 0.],
            basis: Basis::new(0., 0., 0.),
            cap: None,
            obscured: &|_, _| false,
        };
        assert!(seeker::observe_energy(&w, profile, &view, &target).is_some());
        assert!(seeker::observe(&w, profile, &view, &target).is_none());
        assert_eq!(w.seeker.zones[0].minimum_range, 7500);
    }
    /// The SA-2's decoded 3,000 ft zone floor is a look-up limit from the
    /// site. Measured from the climbing missile it hid every target within
    /// 3,000 ft of the missile's own altitude, so no moving target was hit.
    #[test]
    fn an_energy_sam_keeps_its_target_near_the_target_altitude() {
        let mut w = weapon("FIM92.JT", 2);
        w.seeker.zones[0].minimum_altitude = 3000;
        w.seeker.zones[0].maximum_range = 100_000;
        let target = super::super::tests::target(100, [0., 9000., 3000.], 10000, 0x80);
        let profile = missiles::Profile::for_weapon(&w).unwrap();
        let view = seeker::View {
            position: [0., 8000., 0.],
            basis: Basis::new(0., (1000f64).atan2(3000.), 0.),
            cap: None,
            obscured: &|_, _| false,
        };
        assert!(seeker::observe_energy(&w, profile, &view, &target).is_some());
        assert!(seeker::observe(&w, profile, &view, &target).is_none());
    }
    #[test]
    fn live_contact_tick_matches_the_shared_point_estimate() {
        let mut state = super::super::tests::fixture(true);
        state.weapon_rules = Rules::Energy;
        state.energy_predictions_enabled = false;
        state.own_mut().side = Side(1);
        let mut target =
            super::super::tests::target(100, [0., 1000. / FT, 5000. / FT], 10000, 0x80);
        target.side = Side(2);
        target.radius = AIRCRAFT_RADIUS_FT;
        let w = weapon("AIM120.JT", 3);
        let p = projectile(&w, &target);
        let energy = p.motion.unwrap().energy.unwrap();
        let expected = Trial::new(
            energy.body,
            energy.flight,
            missiles::energy::Observation {
                position: target.position.map(|v| v * FT),
                velocity: [0.; 3],
            },
            (f64::from(w.damage.fuze_radius) + AIRCRAFT_RADIUS_FT) * FT,
            0,
        )
        .finish(&|_, _| 0.);
        assert!(expected.hit, "{expected:?}");
        state.set_actor_supports([ActorSupport {
            owner: 0,
            observation: Some(seeker::Observation {
                id: target.id,
                position: target.position,
                velocity: target.velocity,
                quality: 1.,
                off_axis: 0.,
                range: 5000. / FT,
            }),
            supported: true,
            radar_position: p.position,
            radar_emitting: true,
        }]);
        state.targets = vec![target];
        state.projectiles = vec![p];
        let mut contact = None;
        for tick in 1..=energy.body.life {
            if state
                .step_surface(&[], |_, _| 0., |_, _| false)
                .iter()
                .any(|e| matches!(e, Event::Hit(100)))
            {
                contact = Some(tick);
                break;
            }
        }
        assert_eq!(contact, Some((expected.seconds * 120.).round() as u32));
    }
    #[test]
    fn prediction_and_live_preserve_the_existing_arming_grace() {
        let mut state = super::super::tests::fixture(true);
        state.weapon_rules = Rules::Energy;
        state.energy_predictions_enabled = false;
        state.own_mut().side = Side(1);
        let mut target = super::super::tests::target(100, [0., 1000. / FT, 50. / FT], 10000, 0x80);
        target.side = Side(2);
        target.radius = AIRCRAFT_RADIUS_FT;
        let mut w = weapon("AIM120.JT", 3);
        w.damage.fuze_arm_t = 1;
        let p = projectile(&w, &target);
        let energy = p.motion.unwrap().energy.unwrap();
        let mut q = Trial::new(
            energy.body,
            energy.flight,
            missiles::energy::Observation {
                position: target.position.map(|v| v * FT),
                velocity: [0.; 3],
            },
            prediction_radius(&w) * FT,
            30,
        );
        state.targets = vec![target];
        state.projectiles = vec![p];
        for _ in 0..30 {
            q.step(&|_, _| 0.);
            assert!(!q.answer.is_some_and(|a| a.hit));
            assert!(
                !state
                    .step_surface(&[], |_, _| 0., |_, _| false)
                    .iter()
                    .any(|e| matches!(e, Event::Hit(100)))
            );
        }
        assert!(
            q.closest < AIRCRAFT_RADIUS_FT * FT,
            "the unarmed body actually passed the aircraft"
        );
    }
    #[test]
    fn a_late_physical_decoy_does_not_disarm_the_real_aircraft_fuze() {
        let mut state = super::super::tests::fixture(true);
        state.weapon_rules = Rules::Energy;
        state.own_mut().side = Side(1);
        let mut target = super::super::tests::target(100, [0., 1000. / FT, 50. / FT], 10000, 0x80);
        target.side = Side(2);
        target.radius = AIRCRAFT_RADIUS_FT;
        let w = weapon("AIM120.JT", 3);
        let mut p = projectile(&w, &target);
        p.target = None;
        p.motion
            .as_mut()
            .unwrap()
            .energy
            .as_mut()
            .unwrap()
            .decoy
            .pursuit = Some(missiles::decoys::Pursuit {
            device: missiles::decoys::Device::release(
                1,
                100,
                missiles::decoys::Kind::Chaff,
                1,
                [0., 1000., 60.],
                [0.; 3],
            ),
            passed: false,
        });
        state.targets = vec![target];
        state.projectiles = vec![p];
        let mut hit = false;
        for _ in 0..30 {
            hit |= state
                .step_surface(&[], |_, _| 0., |_, _| false)
                .iter()
                .any(|event| matches!(event, Event::Hit(100)));
        }
        assert!(hit, "the aircraft is encountered before the nearby decoy");
    }
    #[test]
    fn a_missile_that_passed_its_decoy_cannot_fuze_on_a_later_aircraft() {
        let mut state = super::super::tests::fixture(true);
        state.weapon_rules = Rules::Energy;
        state.own_mut().side = Side(1);
        let mut target =
            super::super::tests::target(100, [0., 1000. / FT, 1000. / FT], 10000, 0x80);
        target.side = Side(2);
        target.radius = AIRCRAFT_RADIUS_FT;
        let w = weapon("AIM120.JT", 3);
        let mut p = projectile(&w, &target);
        p.target = None;
        p.motion
            .as_mut()
            .unwrap()
            .energy
            .as_mut()
            .unwrap()
            .decoy
            .pursuit = Some(missiles::decoys::Pursuit {
            device: missiles::decoys::Device::release(
                1,
                100,
                missiles::decoys::Kind::Chaff,
                1,
                [0., 1000., 10.],
                [0.; 3],
            ),
            passed: false,
        });
        state.targets = vec![target];
        state.projectiles = vec![p];
        let mut hit = false;
        for _ in 0..600 {
            hit |= state
                .step_surface(&[], |_, _| 0., |_, _| false)
                .iter()
                .any(|event| matches!(event, Event::Hit(100)));
        }
        assert!(!hit, "passing the decoy permanently disables the fuze");
    }
}
