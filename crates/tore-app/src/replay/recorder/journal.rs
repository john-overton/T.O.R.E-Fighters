//! Messages as recorded events: the AI message journal (attack reports,
//! leaders' free selection, wing requests and launch warnings) and the
//! communication journal (radio calls, crew remarks, tower lines, AI text
//! lines, player orders and the music's inputs), each with its trigger,
//! rolls and every recipient's outcome. Both journals are write-only; this
//! only reads what was drained. Mapping choices are agent decisions
//! (2026-09-26); see docs/REPLAYS.md ("Communication journal").

use super::{Recorder, distance, who};
use crate::comms::{
    self,
    journal::{self as talk, Answered, Audience, Cause, Source, TowerEvent},
};
use crate::replay::trees;
use tore_replay::{
    Event, Frame, Value,
    vocab::{self, field, kind, outcome},
};
use tore_sim::ai::{
    mission::ObservedAttack,
    thought::{DropReason, ExpiryReason, JournalBatch, JournalEntry, Message, Outcome},
    threat::SeekerClass,
    wing::{AppliedSetting, ReceiverOutcome, RejectReason, SpacingAxis, TargetOrder, WingRequest},
};

/// Attack evidence is kept this long without news (240 ticks).
const ATTACK_KEPT_S: f64 = 2.;

/// `attack on Friendly 1-3 by Enemy 2-1 (shot 12), bearing 132`.
fn attack_text(attack: &ObservedAttack, who: &dyn Fn(u32) -> String) -> String {
    let mut text = format!("attack on {}", who(attack.report.defended_id));
    match attack.report.attacker_id {
        Some(id) => text.push_str(&format!(" by {}", who(id))),
        None => text.push_str(" by an unidentified attacker"),
    }
    if let Some(shot) = attack.event_id {
        text.push_str(&format!(" (shot {shot})"));
    }
    if let Some(bearing) = attack.bearing_world_deg {
        text.push_str(&format!(", bearing {bearing:.0}"));
    }
    text
}

fn reject_reason(reason: RejectReason) -> String {
    match reason {
        RejectReason::Dummy => "a training target takes no orders".into(),
        RejectReason::IneligibleState(state) => {
            format!("its state ({state}) does not allow a maneuver")
        }
        RejectReason::BuggedOut => "it bugged out and no longer answers".into(),
        RejectReason::Landed => "it has landed".into(),
        RejectReason::OnAirfield => "taking off, landing or on the ground".into(),
    }
}

/// A wing command's outcome at one recipient.
fn receiver(outcome: ReceiverOutcome) -> (&'static str, String) {
    match outcome {
        ReceiverOutcome::Applied(setting) => (
            outcome::APPLIED,
            match setting {
                AppliedSetting::Spacing { feet, .. } => format!("spacing set to {feet} ft"),
                AppliedSetting::FormationSelection {
                    cleared_active_command,
                } => if cleared_active_command {
                    "formation set; its running maneuver ended"
                } else {
                    "formation set"
                }
                .into(),
                AppliedSetting::WingControl => "wing control set".into(),
                AppliedSetting::TargetOrder { .. } => "target order set".into(),
            },
        ),
        ReceiverOutcome::Rejected(reason) => (outcome::REJECTED, reject_reason(reason)),
        ReceiverOutcome::AppliedNoMotion => {
            (outcome::APPLIED, "accepted without a maneuver".into())
        }
        ReceiverOutcome::MotionInstalled(m) => (
            outcome::APPLIED,
            format!("maneuver heading {}, pitch {}", m.heading_deg, m.pitch_deg),
        ),
    }
}

fn drop_reason(reason: DropReason) -> String {
    match reason {
        DropReason::Airfield { phase } => {
            format!("ignored while {}", trees::airfield_label(phase))
        }
        DropReason::TrainingTarget => "training targets ignore warnings".into(),
        DropReason::NotWarned => "the delay rule does not warn this kind of target".into(),
    }
}

/// One AI message outcome as an outcome name and a reason.
fn ai_outcome(o: Outcome) -> (&'static str, String) {
    match o {
        Outcome::Queued => (outcome::QUEUED, String::new()),
        Outcome::Delivered => (outcome::DELIVERED, String::new()),
        Outcome::Ignored(reason) => (outcome::IGNORED, trees::ignore_reason(reason)),
        Outcome::Expired(ExpiryReason::Age { ticks }) => (
            outcome::EXPIRED,
            format!("no news for {:.1} s", ticks as f64 / 120.),
        ),
        Outcome::Expired(ExpiryReason::AttackerGone) => {
            (outcome::EXPIRED, "the attacker is gone".into())
        }
        Outcome::Order(o) => receiver(o),
        Outcome::WarningDue { due_tick, .. } => {
            (outcome::QUEUED, format!("due at mission tick {due_tick}"))
        }
        Outcome::WarningReceived { outcome: o, .. } => {
            (outcome::DELIVERED, trees::warning_reaction(o.reaction))
        }
        Outcome::WarningDropped(reason) => (outcome::DROPPED, drop_reason(reason)),
    }
}

fn request_text(request: &WingRequest, who: &dyn Fn(u32) -> String) -> String {
    match request {
        WingRequest::Break {
            heading_offset_deg,
            pitch_deg,
        } => format!("break {heading_offset_deg:+} deg, pitch {pitch_deg}"),
        WingRequest::Approach {
            heading_deg,
            pitch_deg,
            speed,
        } => {
            if speed.0 == 0. {
                format!("approach heading {heading_deg}, pitch {pitch_deg}, corner speed")
            } else {
                format!(
                    "approach heading {heading_deg}, pitch {pitch_deg}, {:.0} kt",
                    trees::knots(speed.0)
                )
            }
        }
        WingRequest::Spacing { axis, feet } => format!(
            "{} spacing {feet} ft",
            match axis {
                SpacingAxis::Horizontal => "horizontal",
                SpacingAxis::Vertical => "vertical",
            }
        ),
        WingRequest::FormationSelection(f) => format!("formation {}", f.name()),
        WingRequest::WingControl(c) => format!("wing control {}", c.name()),
        WingRequest::TargetAssignment(order) => match order {
            TargetOrder::HoldFire => "hold fire".into(),
            TargetOrder::FreeSelection => "free selection".into(),
            TargetOrder::ClassPolicy(policy) => format!("attack {policy:?}").to_lowercase(),
            TargetOrder::ConcreteTarget(target) => format!("attack {}", who(target.0)),
        },
        WingRequest::Land(order) => format!("land ({:?})", order.reason).to_lowercase(),
    }
}

fn devices(class: SeekerClass, count: u8) -> String {
    match class {
        SeekerClass::Radar => format!("chaff x{count}"),
        SeekerClass::Infrared => format!("flares x{count}"),
    }
}

impl Recorder {
    /// The AI message journal drained for this tick.
    pub(super) fn ai_journal(
        &mut self,
        batch: &JournalBatch,
        frame: &Frame,
        events: &mut Vec<Event>,
    ) {
        if batch.dropped > 0 {
            events.push(Event::new(kind::SYSTEM_NOTE).with_text(format!(
                "the AI message journal overflowed; {} entries were lost",
                batch.dropped
            )));
        }
        for entry in &batch.entries {
            match &entry.message {
                Message::AttackEvidence(attack) => self.attack(entry, attack, events),
                Message::FreeSelection { trigger } => self.free_selection(entry, trigger, events),
                Message::WingRequest(request) => self.wing_request(entry, request, events),
                // An escort's changed selection shows in its thought tree.
                Message::EscortPriority { .. } => {}
                Message::MissileWarning(report) => self.warning(entry, report, frame, events),
            }
        }
    }

    /// Attack evidence: the report when it is queued, each recipient's
    /// delivery, and reports that went nowhere.
    fn attack(&mut self, entry: &JournalEntry, attack: &ObservedAttack, events: &mut Vec<Event>) {
        let infos = &self.infos;
        let named = |id: u32| who(infos, id);
        let text = attack_text(attack, &named);
        let key = (
            attack.report.attacker_id,
            attack.report.defended_id,
            attack.event_id,
        );
        let about = attack
            .report
            .attacker_id
            .unwrap_or(attack.report.defended_id);
        let queued = !entry.receipts.is_empty()
            && entry
                .receipts
                .iter()
                .all(|r| matches!(r.outcome, Outcome::Queued));
        if queued {
            let message = self.why.message();
            if self.why.attacks.len() >= 4096 {
                self.why.attacks.clear();
            }
            self.why.attacks.insert(key, message);
            let recipients: Vec<u32> = entry.receipts.iter().map(|r| r.actor).collect();
            let mut event = Event::new(kind::COMMS_REPORT)
                .with(field::MESSAGE, message)
                .with(field::RECIPIENTS, recipients)
                .with(field::ORDER, "attack report")
                .with(field::ABOUT, Value::Id(about))
                .with(field::KEPT_S, ATTACK_KEPT_S)
                .with(field::OUTCOME, outcome::QUEUED)
                .with(field::TRIGGER, "an attack it saw")
                .with_text(text);
            if let Some(sender) = entry.sender {
                event = event.with_subject(sender);
            }
            events.push(event);
            return;
        }
        for receipt in &entry.receipts {
            let (result, reason) = ai_outcome(receipt.outcome);
            let known = self.why.attacks.get(&key).copied();
            let mut event = match known {
                Some(message) => Event::new(kind::COMMS_DELIVERY)
                    .with_subject(receipt.actor)
                    .with(field::MESSAGE, message),
                // A report that went nowhere, or one queued before the
                // recording began.
                None => {
                    let message = self.why.message();
                    let mut report = Event::new(kind::COMMS_REPORT)
                        .with(field::MESSAGE, message)
                        .with(field::RECIPIENTS, vec![receipt.actor])
                        .with(field::ORDER, "attack report")
                        .with(field::ABOUT, Value::Id(about))
                        .with(field::KEPT_S, ATTACK_KEPT_S);
                    if let Some(sender) = entry.sender {
                        report = report.with_subject(sender);
                    }
                    report
                }
            };
            if known.is_some()
                && let Some(sender) = entry.sender
            {
                event = event.with_object(sender);
            }
            event = event.with(field::OUTCOME, result).with_text(text.as_str());
            if !reason.is_empty() {
                event = event.with(field::REASON, reason);
            }
            events.push(event);
            if matches!(receipt.outcome, Outcome::Delivered) {
                self.why
                    .hear(receipt.actor, format!("a report of an {text}"));
            }
        }
    }

    /// A neutral leader released itself to free target selection.
    fn free_selection(
        &mut self,
        entry: &JournalEntry,
        trigger: &ObservedAttack,
        events: &mut Vec<Event>,
    ) {
        let infos = &self.infos;
        let named = |id: u32| who(infos, id);
        let text = attack_text(trigger, &named);
        for receipt in &entry.receipts {
            let (result, reason) = ai_outcome(receipt.outcome);
            let message = self.why.message();
            let mut event = Event::new(kind::COMMS_ORDER)
                .with(field::MESSAGE, message)
                .with(field::RECIPIENTS, vec![receipt.actor])
                .with(field::ORDER, "free selection")
                .with(field::TRIGGER, format!("the leader saw an {text}"))
                .with(field::OUTCOME, result)
                .with_text("released itself to free target selection");
            if let Some(sender) = entry.sender {
                event = event.with_subject(sender);
            }
            if !reason.is_empty() {
                event = event.with(field::REASON, reason);
            }
            events.push(event);
            self.why.hear(
                receipt.actor,
                format!("releasing itself to free selection after an {text}"),
            );
        }
    }

    /// A wing command and each recipient's outcome.
    fn wing_request(
        &mut self,
        entry: &JournalEntry,
        request: &WingRequest,
        events: &mut Vec<Event>,
    ) {
        if entry.receipts.is_empty() {
            // Nobody in the wing to receive it: a leader alone.
            return;
        }
        let infos = &self.infos;
        let named = |id: u32| who(infos, id);
        let order = request_text(request, &named);
        let from = match entry.sender {
            None => "the mission".to_owned(),
            Some(sender) => named(sender),
        };
        let trigger = match entry.sender {
            None => "the mission".to_owned(),
            Some(0) => "your order".to_owned(),
            Some(sender) => format!("an order from {}", named(sender)),
        };
        let message = self.why.message();
        let recipients: Vec<u32> = entry.receipts.iter().map(|r| r.actor).collect();
        let mut event = Event::new(kind::COMMS_REQUEST)
            .with(field::MESSAGE, message)
            .with(field::RECIPIENTS, recipients)
            .with(field::ORDER, order.as_str())
            .with(field::TRIGGER, trigger);
        if let Some(sender) = entry.sender {
            event = event.with_subject(sender);
        }
        if let [one] = entry.receipts.as_slice() {
            let (result, reason) = ai_outcome(one.outcome);
            event = event.with(field::OUTCOME, result);
            if !reason.is_empty() {
                event = event.with(field::REASON, reason);
            }
            events.push(event);
        } else {
            events.push(event.with(field::OUTCOME, outcome::ANSWERED));
            for receipt in &entry.receipts {
                let (result, reason) = ai_outcome(receipt.outcome);
                let mut delivery = Event::new(kind::COMMS_DELIVERY)
                    .with_subject(receipt.actor)
                    .with(field::MESSAGE, message)
                    .with(field::ORDER, order.as_str())
                    .with(field::OUTCOME, result);
                if let Some(sender) = entry.sender {
                    delivery = delivery.with_object(sender);
                }
                if !reason.is_empty() {
                    delivery = delivery.with(field::REASON, reason);
                }
                events.push(delivery);
            }
        }
        // Orders that can explain a decision change; spacing and wing
        // control move the slot, not the decision.
        let decisive = !matches!(
            request,
            WingRequest::Spacing { .. } | WingRequest::WingControl(_)
        );
        for receipt in &entry.receipts {
            if decisive
                && !matches!(
                    receipt.outcome,
                    Outcome::Order(ReceiverOutcome::Rejected(_))
                )
            {
                self.why
                    .hear(receipt.actor, format!("\"{order}\" from {from}"));
            }
        }
    }

    /// A launch warning's arrival or its drop, as a defensive reaction.
    fn warning(
        &mut self,
        entry: &JournalEntry,
        report: &tore_sim::ai::controller::ThreatReport,
        frame: &Frame,
        events: &mut Vec<Event>,
    ) {
        let infos = &self.infos;
        let named = |id: u32| who(infos, id);
        let seeker = trees::seeker_label(report.seeker);
        for receipt in &entry.receipts {
            let (reaction, reason) = match receipt.outcome {
                Outcome::WarningReceived {
                    outcome: o,
                    launch_call,
                } => {
                    let mut reaction = trees::warning_reaction(o.reaction);
                    if let Some(d) = o.devices {
                        reaction.push_str(&format!(", {} scheduled", devices(d.class, d.count)));
                    }
                    let mut reason = format!(
                        "launch warning: {seeker} missile from {}",
                        named(report.launcher_id)
                    );
                    if o.approach_abandoned {
                        reason.push_str("; it abandoned its approach");
                    }
                    if launch_call {
                        reason.push_str("; it made a launch call");
                    }
                    (reaction, reason)
                }
                Outcome::WarningDropped(r) => (
                    "ignored".to_owned(),
                    format!("launch warning dropped: {}", drop_reason(r)),
                ),
                _ => continue,
            };
            let delay_s = entry.tick.saturating_sub(report.launch_tick) as f64 / 120.;
            let mut event = Event::new(kind::AI_DEFENSE)
                .with_subject(receipt.actor)
                .with_object(report.launcher_id)
                .with(field::THREAT, Value::Id(report.missile_id))
                .with(field::REACTION, reaction)
                .with(field::REASON, reason)
                .with(
                    field::LAUNCH_RANGE_FT,
                    trees::round(report.distance_at_launch_ft, 0),
                )
                .with(field::DELAY_S, trees::round(delay_s, 2));
            let missile = frame.projectiles.iter().find(|p| p.id == report.missile_id);
            let aircraft = frame.aircraft.iter().find(|a| a.id == receipt.actor);
            if let (Some(m), Some(a)) = (missile, aircraft) {
                event = event.with(
                    field::RANGE_FT,
                    trees::round(distance(m.position, a.position), 0),
                );
            }
            events.push(event);
            self.why
                .hear(receipt.actor, format!("a {seeker} launch warning"));
        }
    }

    /// Drains the channel's communication journal for this tick: every
    /// radio call, crew remark, tower line, AI text line and player order,
    /// with its trigger, rolls and outcome. Entries the journal's bound
    /// threw away since the last drain are noted.
    pub fn drain_comms(&mut self, comms: &mut comms::Comms) {
        let lost = comms.journal().lost();
        if lost > self.why.comms_lost {
            self.note(Event::new(kind::SYSTEM_NOTE).with_text(format!(
                "the communication journal was full; {} entries were lost",
                lost - self.why.comms_lost
            )));
            self.why.comms_lost = lost;
        }
        self.comms(comms.take_journal());
    }

    /// Journal entries drained elsewhere, such as the situation music's.
    pub fn comms(&mut self, entries: Vec<talk::Entry>) {
        for entry in &entries {
            for event in self.comms_events(entry) {
                self.note(event);
            }
        }
    }

    fn comms_events(&mut self, entry: &talk::Entry) -> Vec<Event> {
        let source = entry.source();
        let (event_kind, producer) = match source {
            Source::Radio => (kind::COMMS_RADIO, vocab::source::RADIO),
            Source::Reply => (kind::COMMS_RADIO, vocab::source::REPLY),
            Source::Chatter => (kind::COMMS_RADIO, vocab::source::CHATTER),
            Source::Crew => (kind::COMMS_CREW, vocab::source::CREW),
            Source::Tower => (kind::COMMS_TOWER, vocab::source::TOWER),
            Source::Hud => (kind::COMMS_HUD, vocab::source::HUD),
            Source::Order => (kind::COMMS_ORDER, vocab::source::ORDER),
            Source::Music => return vec![music_event(entry)],
        };
        // The HUD records each message it shows, so an AI text line handed
        // to the HUD is not listed twice.
        if source == Source::Hud && matches!(entry.outcome, talk::Outcome::Delivered { .. }) {
            return Vec::new();
        }
        // The two host tower entries a replay acts on get fixed names.
        let trigger = match &entry.origin.cause {
            Cause::TowerRequest => vocab::trigger::PLAYER_REQUEST.to_owned(),
            Cause::Tower(TowerEvent::ClearanceCancelled) => {
                vocab::trigger::CLEARANCE_CANCELLED.to_owned()
            }
            cause => cause.to_string(),
        };
        let mut event = Event::new(event_kind)
            .with(field::SPEAKER, entry.label.as_str())
            .with(field::SOURCE, producer)
            .with(field::OUTCOME, entry.outcome.name())
            .with(field::TRIGGER, trigger)
            .with_text(entry.text.as_str());
        if let Some(speaker) = entry.origin.speaker {
            event = event.with_subject(speaker);
        }
        if !entry.stems.is_empty() {
            event = event.with(field::STEMS, entry.stems.join(" "));
        }
        if let Some(route) = entry.route {
            event = event.with(
                field::ROUTE,
                match route {
                    comms::Route::Radio => vocab::route::RADIO,
                    comms::Route::Airport => vocab::route::TOWER,
                    comms::Route::Direct => vocab::route::DIRECT,
                },
            );
        }
        if let Some(call_kind) = entry.kind {
            event = event.with(
                field::KIND,
                match call_kind {
                    comms::Kind::Chatter => "chatter",
                    comms::Kind::Important => "important",
                },
            );
        }
        if entry.origin.audience != Audience::Unknown {
            event = event.with(field::AUDIENCE, entry.origin.audience.to_string());
        }
        if !entry.origin.rolls.is_empty() {
            let rolls: Vec<String> = entry.origin.rolls.iter().map(ToString::to_string).collect();
            event = event.with(field::ROLLS, rolls.join("; "));
        }
        let message = entry.call.map(|call| call as i64);
        if let Some(message) = message {
            event = event.with(field::MESSAGE, message);
        }
        if entry.heard() {
            event = event.with(field::HEARD, true);
        }
        match &entry.outcome {
            talk::Outcome::Delivered { waited } => {
                event = event.with(field::WAIT_S, trees::round(*waited, 2));
            }
            talk::Outcome::Queued { due, expires } => {
                event = event.with(field::DUE_S, trees::round(due - entry.at, 2));
                if let Some(expires) = expires {
                    event = event.with("expires_s", trees::round(expires - entry.at, 2));
                }
            }
            talk::Outcome::Interrupted(_) => event = event.with(field::HEARD, true),
            talk::Outcome::Unheard(_)
            | talk::Outcome::Dropped(_)
            | talk::Outcome::Suppressed(_)
            | talk::Outcome::Replaced(_)
            | talk::Outcome::Expired(_)
            | talk::Outcome::Cancelled(_)
            | talk::Outcome::Refused(_) => event = event.with(field::HEARD, false),
            talk::Outcome::Answered { .. } | talk::Outcome::Silent | talk::Outcome::Noted => {}
        }
        if let Some(reason) = entry.outcome.reason() {
            event = event.with(field::REASON, reason.to_string());
        }
        if let Cause::Order { order, target, .. } = &entry.origin.cause {
            event = event.with(field::ORDER, format!("{order:?}"));
            if let Some(target) = target {
                event = event.with_object(*target);
            }
        }
        let talk::Outcome::Answered { answers, reply } = &entry.outcome else {
            return vec![event];
        };
        // An order's answers: one delivery per addressed wingman.
        let message = message.unwrap_or_else(|| self.why.message());
        let recipients: Vec<u32> = answers.iter().map(|a| a.recipient).collect();
        let mut out = vec![
            event
                .with(field::MESSAGE, message)
                .with(field::RECIPIENTS, recipients)
                .with("reply", reply.to_string()),
        ];
        for answer in answers {
            let mut reason = match answer.result {
                Answered::Receiver(r) => receiver(r).1,
                Answered::CannotSeeTarget => "its sensors cannot see the target".into(),
                Answered::BuggedOut => "it bugged out and no longer answers".into(),
                Answered::Human => "flown by a human".into(),
                Answered::AlreadyLanded => "already landed".into(),
                Answered::OnAirfield => "taking off, landing or on the ground".into(),
                Answered::NoBase => "no base to return to".into(),
            };
            for (what, side) in &answer.side {
                let (result, why) = receiver(*side);
                reason.push_str(&format!("; {what}: {result}"));
                if !why.is_empty() {
                    reason.push_str(&format!(" ({why})"));
                }
            }
            let mut delivery = Event::new(kind::COMMS_DELIVERY)
                .with_subject(answer.recipient)
                .with(field::MESSAGE, message)
                .with(field::OUTCOME, answer.result.name())
                .with("member", i64::from(answer.member));
            if let Some(speaker) = entry.origin.speaker {
                delivery = delivery.with_object(speaker);
            }
            if !reason.is_empty() {
                delivery = delivery.with(field::REASON, reason);
            }
            out.push(delivery);
        }
        out
    }
}

/// The music's inputs changed: the score they ask for, why, and every
/// input, so a replay can hand the music the same inputs.
fn music_event(entry: &talk::Entry) -> Event {
    let mut event = Event::new(kind::AUDIO_MUSIC)
        .with(field::OUTCOME, entry.outcome.name())
        .with(field::REASON, entry.origin.cause.to_string());
    if let Cause::Music(music) = &entry.origin.cause {
        let name = |rank: crate::audio::situation::Rank| format!("{rank:?}").to_lowercase();
        event = event
            .with(field::FROM, music.from.map_or_else(|| "-".to_owned(), name))
            .with(field::TO, name(music.to));
        for (input, on) in vocab::music::ALL
            .into_iter()
            .zip(music_inputs(&music.inputs))
        {
            event = event.with(input, on);
        }
    }
    event
}

/// The music's inputs in the order of [`vocab::music::ALL`].
fn music_inputs(inputs: &crate::audio::situation::Inputs) -> [bool; 8] {
    let crate::audio::situation::Inputs {
        succeeded,
        ejected,
        launching,
        air_target,
        hit_recently,
        danger,
        home,
        deck,
    } = *inputs;
    [
        succeeded,
        ejected,
        launching,
        air_target,
        hit_recently,
        danger,
        home,
        deck,
    ]
}

#[cfg(test)]
#[path = "journal_tests.rs"]
mod tests;
