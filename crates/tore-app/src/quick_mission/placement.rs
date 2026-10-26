//! Asynchronous placement audit for the creator's distance choices.
use super::*;
use std::sync::mpsc;
use tore_world::mission_layout::availability::{self, Availability};

pub const CHECKING: &str = "Checking distances and target placement...";
pub const TOO_FAR: &str = "This distance does not fit the selected theater, target and start.";

#[derive(Default)]
pub struct Check {
    key: String,
    pending: Option<mpsc::Receiver<Result<Availability, String>>>,
    ready: Option<Result<Availability, String>>,
}

impl Check {
    fn poll(&mut self) {
        if let Some(receiver) = &self.pending {
            match receiver.try_recv() {
                Ok(result) => {
                    self.ready = Some(result);
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.ready = Some(Err(
                        "Placement check did not finish. Change the target to retry.".into(),
                    ));
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
    }
    fn request(
        &mut self,
        resources: Arc<BTreeMap<String, Vec<u8>>>,
        spec: MissionSpec,
    ) -> Option<Result<Availability, String>> {
        // One geometry build tests all eleven separations. Keep the current
        // distance in the key because its front-line clearance is checked too.
        let key = spec.to_text();
        self.poll();
        if self.key == key {
            return self.ready.clone();
        }
        if self.pending.is_some() {
            return None;
        }
        let (tx, rx) = mpsc::channel();
        self.key = key;
        self.ready = None;
        match std::thread::Builder::new()
            .name("tore-placement-check".into())
            .spawn(move || {
                let _ = tx.send(
                    availability::audit(resources.as_ref(), &spec).map_err(|e| e.to_string()),
                );
            }) {
            Ok(_) => self.pending = Some(rx),
            Err(e) => self.ready = Some(Err(format!("Cannot check placement: {e}"))),
        }
        self.ready.clone()
    }
}

impl QuickMission {
    pub(super) fn reserve_placement_seed(&mut self) {
        if self.lays_out_surface() && !self.seed_pending {
            if self.view.is_none() {
                self.surface_seed = super::fresh_seed();
            }
            self.seed_pending = true;
        }
    }
    /// CPU snapshot/probe entry point. Live menus only poll the worker.
    pub fn wait_placement(&mut self) {
        if self.resources.is_none() {
            return;
        }
        self.reserve_placement_seed();
        for _ in 0..2 {
            if self.placement_check().is_some() {
                return;
            }
            let mut state = self.placement.borrow_mut();
            if let Some(rx) = state.pending.take() {
                state.ready = Some(
                    rx.recv()
                        .unwrap_or_else(|_| Err("Placement check did not finish.".into())),
                );
            }
        }
    }
    pub(super) fn placement_check(&self) -> Option<Result<Availability, String>> {
        let resources = self.resources.as_ref()?;
        let spec = self.mission_spec().ok()?;
        self.placement.borrow_mut().request(resources.clone(), spec)
    }
    pub(super) fn separation_problem(&self, index: usize) -> Option<&'static str> {
        self.resources.as_ref()?;
        self.mission_spec().ok()?;
        match self.placement_check() {
            Some(Ok(result)) if result.separations.get(index).copied().unwrap_or(false) => None,
            Some(Ok(_)) => Some(TOO_FAR),
            Some(Err(_)) => Some("Placement could not be checked. Change the target or theater."),
            None => Some(CHECKING),
        }
    }
    pub(super) fn placement_notice(&self) -> Option<String> {
        self.resources.as_ref()?;
        match self.placement_check() {
            Some(Ok(a)) if !a.limitations.is_empty() => Some(a.limitations.join(" ")),
            Some(Err(e)) => Some(e),
            None => Some(CHECKING.into()),
            _ => None,
        }
    }
}
