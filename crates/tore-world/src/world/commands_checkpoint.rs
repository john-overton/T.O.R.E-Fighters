//! The coders of the mission's commands and what they carry: the settings,
//! a revival's spawn and its loadout (stage K slice K0).
//!
//! The host's journal records each tick's mission commands as given, and a
//! standby replays them (docs/ARCHITECTURE.md, "The journal: one door into
//! the world"). Every field and variant is named with no catch-all, so a
//! variant added to [`MissionCommand`], or a field added to [`Spawn`] or a
//! loadout, fails to compile here until the journal codes it.

use super::{MissionCommand, Settings};
use crate::mission::{LoadoutSpec, StationLoad};
use crate::world::revive::Spawn;
use tore_sim::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

tore_sim::checkpoint_struct!(Settings { cheats });

tore_sim::checkpoint_struct!(StationLoad {
    weapon,
    count,
    quantity,
});

tore_sim::checkpoint_struct!(LoadoutSpec {
    fuel_lbs,
    cheat,
    stations,
});

tore_sim::checkpoint_struct!(Spawn {
    position,
    heading_rad,
    speed_fps,
    loadout,
});

/// A command by its variant's number and its fields, with no baseline.
impl Checkpoint for MissionCommand {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            MissionCommand::Settings(settings) => {
                s.writer().write_varint(0);
                settings.save(s, None)
            }
            MissionCommand::Take { seat, plane } => {
                s.writer().write_varint(1);
                seat.save(s, None)?;
                plane.save(s, None)
            }
            MissionCommand::GiveBack { seat } => {
                s.writer().write_varint(2);
                seat.save(s, None)
            }
            MissionCommand::Abandon { seat } => {
                s.writer().write_varint(3);
                seat.save(s, None)
            }
            MissionCommand::Revive { seat, spawn } => {
                s.writer().write_varint(4);
                seat.save(s, None)?;
                spawn.save(s, None)
            }
        }
    }

    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(match l.reader().read_varint()? {
            0 => MissionCommand::Settings(Checkpoint::load(l, None)?),
            1 => MissionCommand::Take {
                seat: Checkpoint::load(l, None)?,
                plane: Checkpoint::load(l, None)?,
            },
            2 => MissionCommand::GiveBack {
                seat: Checkpoint::load(l, None)?,
            },
            3 => MissionCommand::Abandon {
                seat: Checkpoint::load(l, None)?,
            },
            4 => MissionCommand::Revive {
                seat: Checkpoint::load(l, None)?,
                spawn: Checkpoint::load(l, None)?,
            },
            other => return invalid(format!("a mission command has no variant {other}")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seats::{PlaneId, SeatId};
    use tore_sim::checkpoint::{Models, from_bytes, round_trip, to_bytes};

    fn spawn() -> Spawn {
        Spawn {
            position: [1.5e5, -2.25e4, 9_000.125],
            heading_rad: -2.5,
            speed_fps: 640.,
            loadout: LoadoutSpec {
                fuel_lbs: 10_860.,
                cheat: true,
                stations: vec![
                    StationLoad {
                        weapon: "AIM9X.JT".into(),
                        count: 2,
                        quantity: 2,
                    },
                    StationLoad {
                        weapon: String::new(),
                        count: 0,
                        quantity: 0,
                    },
                    StationLoad {
                        weapon: "M61A1.JT".into(),
                        count: 1,
                        quantity: 578,
                    },
                ],
            },
        }
    }

    /// One command of every variant.
    fn every_command() -> Vec<MissionCommand> {
        let cheats = tore_sim::cheats::Cheats {
            unlimited_ammo: true,
            ..Default::default()
        };
        vec![
            MissionCommand::Settings(Settings::default()),
            MissionCommand::Settings(Settings { cheats }),
            MissionCommand::Take {
                seat: SeatId(3),
                plane: PlaneId(1_000),
            },
            MissionCommand::GiveBack { seat: SeatId(0) },
            MissionCommand::Abandon { seat: SeatId(254) },
            MissionCommand::Revive {
                seat: SeatId(7),
                spawn: Box::new(spawn()),
            },
        ]
    }

    #[test]
    fn every_mission_command_round_trips() {
        let models = Models::default();
        for command in every_command() {
            assert_eq!(round_trip(&command, &models).unwrap(), command);
        }
        assert_eq!(round_trip(&spawn(), &models).unwrap(), spawn());
    }

    #[test]
    fn damaged_command_bytes_are_refused_without_a_panic() {
        let models = Models::default();
        for command in every_command() {
            let coded = to_bytes(&command, &models).unwrap();
            for cut in 0..coded.body.len() {
                let mut shorter = coded.clone();
                shorter.body.truncate(cut);
                assert!(from_bytes::<MissionCommand>(&shorter, &models).is_err());
            }
            for bit in 0..coded.body.len() * 8 {
                let mut flipped = coded.clone();
                flipped.body[bit / 8] ^= 1 << (bit % 8);
                let _ = from_bytes::<MissionCommand>(&flipped, &models);
            }
        }
        let mut s = Saver::new();
        s.writer().write_varint(5);
        let body = s.finish_section();
        let mut l = Loader::new(&body, &[], &models);
        assert!(MissionCommand::load(&mut l, None).is_err());
    }
}
