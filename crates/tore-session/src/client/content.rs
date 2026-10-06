//! Stage L on a player's game: its own words about what its import cannot
//! play, and the line about gaps a lobby shows (docs/ARCHITECTURE.md,
//! "Compatibility" and "The words").
//!
//! The host's words, which every player reads about a player, are in
//! `host::content`; the creator's refusal of a choice in a gap uses the same
//! function as the host's ([`gap_refusal`]). Here are the second-person lines
//! a player reads about its own game, and the line a lobby prints when a
//! player's items differ from the host's. A player's Fighters Anthology build
//! is not shown anywhere (John, 2026-10-06: the build audit found no
//! difference a player sees); the wire still carries it.

use crate::host::content::{
    GameContent, ItemKey, Resources, files_phrase, gap_label, label_of, list, mission_items,
    not_everyone, shortfall, world_key, world_kind,
};
use crate::wire::messages::{ContentGaps, Gap, Importer, ItemKind, LobbyPlayer, LobbyState};
use std::collections::{BTreeMap, BTreeSet};
use tore_world::mission::MissionSpec;

/// What to do about an item the import lacks.
const REIMPORT: &str = "Re-import Fighters Anthology (Pref, Re-import) to add it.";

/// The numbers of a version's release part: `0.1.3` of `0.1.3-4-gabc`.
fn version_numbers(version: &str) -> Option<Vec<u64>> {
    let release = version.split('-').next()?;
    release.split('.').map(|part| part.parse().ok()).collect()
}

/// Whether an import made by `importer` was made by an earlier T.O.R.E than
/// this build (`this`, a version string). An unknown importer is earlier:
/// only a pack made before stage L has none. Versions that do not read as
/// numbers are not earlier (agent decision: then nothing is said).
pub fn made_earlier(importer: Option<&Importer>, this: &str) -> bool {
    let Some(importer) = importer else {
        return true;
    };
    match (version_numbers(&importer.version), version_numbers(this)) {
        (Some(theirs), Some(ours)) => theirs < ours,
        _ => false,
    }
}

/// "Your import was made by an earlier T.O.R.E (0.1.3)." when the import is
/// older than this build, else `None`.
pub fn earlier_words(importer: Option<&Importer>, this: &str) -> Option<String> {
    made_earlier(importer, this).then(|| match importer {
        Some(importer) => format!(
            "Your import was made by an earlier T.O.R.E ({}).",
            importer.version
        ),
        None => "Your import was made by an earlier T.O.R.E.".to_owned(),
    })
}

/// The label of an item for the player's own words: its own import's name
/// for it, else the key.
fn own_label(kind: ItemKind, key: &str, resources: &Resources) -> String {
    label_of(kind, key, resources).unwrap_or_else(|| key.to_owned())
}

/// Why the player's import cannot play `spec`, in the second person, from
/// the names its exact check found different (`differences`, not empty):
///
/// - a mission item the import lacks: "Your game has no Su-27, which this
///   mission flies." and what to do, naming an earlier importer;
/// - an item it has that reads a differing file: "Your Su-27's data differs
///   from the host's.";
/// - otherwise the shared data: "Your game's shared flight data differs from
///   the host's (3 files, such as CRATER.SH). Re-import Fighters Anthology
///   with this version of T.O.R.E."
pub fn refusal(
    own: &GameContent,
    resources: &Resources,
    spec: &MissionSpec,
    differences: &[String],
    this_version: &str,
) -> String {
    let items = mission_items(spec);
    let digests = own.digests();
    let found = shortfall(&items, &|key| digests.get(key).copied(), &BTreeMap::new());
    if let Some((kind, keys)) = found.lacks {
        let labels: Vec<String> = keys
            .iter()
            .map(|key| own_label(kind, key, resources))
            .collect();
        let uses = match kind {
            ItemKind::Aircraft => "which this mission flies",
            ItemKind::Theater => "where this mission flies",
            ItemKind::Weapon => "which this flight carries",
            ItemKind::Shared => "which every mission reads",
        };
        let what = match labels.as_slice() {
            [one] => one.clone(),
            [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
            [] => String::new(),
        };
        let mut text = format!("Your game has no {what}, {uses}.");
        if let Some(earlier) = earlier_words(own.importer().as_ref(), this_version) {
            text.push(' ');
            text.push_str(&earlier);
        }
        text.push(' ');
        text.push_str(REIMPORT);
        return text;
    }
    // The mission's own items that read a differing file.
    let wanted: BTreeSet<&ItemKey> = items.iter().collect();
    let mut holders: Vec<ItemKey> = Vec::new();
    for name in differences {
        for item in own.content.holding(name) {
            let kind = crate::host::content::item_kind(item.kind);
            let key = (kind, crate::host::content::wire_key(kind, &item.key));
            if kind != ItemKind::Shared && wanted.contains(&key) && !holders.contains(&key) {
                holders.push(key);
            }
        }
    }
    if !holders.is_empty() {
        let labels: Vec<String> = holders
            .iter()
            .map(|(kind, key)| own_label(*kind, key, resources))
            .collect();
        return match labels.as_slice() {
            [one] => format!("Your {one}'s data differs from the host's."),
            many => format!("Your data for the {} differs from the host's.", list(many)),
        };
    }
    format!(
        "Your game's shared flight data differs from the host's ({}). Re-import Fighters \
         Anthology with this version of T.O.R.E.",
        files_phrase(differences)
    )
}

/// Whether the player's own content has the item of the wire's `kind` and
/// `key`.
pub fn has(own: &GameContent, kind: ItemKind, key: &str) -> bool {
    own.content
        .get(world_kind(kind), &world_key(kind, key))
        .is_some()
}

/// The gap of an item, if it is in one.
pub fn gap<'a>(gaps: &'a ContentGaps, kind: ItemKind, key: &str) -> Option<&'a Gap> {
    gaps.gaps
        .iter()
        .find(|gap| gap.kind == kind && gap.key == key)
}

/// The refusal of a choice in a gap, the host's words ("Not everyone can fly
/// the Su-27: Hawk's game has no Su-27."), from the gaps and the lobby;
/// `None` when the item is in no gap. The shared item restricts nothing.
pub fn gap_refusal(
    gaps: &ContentGaps,
    lobby: Option<&LobbyState>,
    kind: ItemKind,
    key: &str,
) -> Option<String> {
    if kind == ItemKind::Shared {
        return None;
    }
    let gap = gap(gaps, kind, key)?;
    let name = |id: u8| {
        lobby
            .and_then(|lobby| lobby.player(id))
            .map_or_else(|| format!("player {id}"), |p| p.callsign.clone())
    };
    let house = lobby
        .and_then(|lobby| lobby.host.and_then(|id| lobby.player(id)))
        .map(|p| p.callsign.clone());
    Some(not_everyone(gap, &gap_label(gap), &name, house.as_deref()))
}

/// The Messages line when a player's items differ from the host's, once, as
/// the lobby learns them: "Hawk's game differs from the host's: no Su-27, a
/// different AIM-9X."; for the player itself (`you`), "Your game differs
/// ...". `None` when no gap names the player. The player's Fighters
/// Anthology build is not part of it (agent decision with John's 2026-10-06
/// removal: the retail builds read the same to a player).
pub fn differs_line(player: &LobbyPlayer, you: bool, gaps: &ContentGaps) -> Option<String> {
    let mut lacks = Vec::new();
    let mut differs = Vec::new();
    for gap in &gaps.gaps {
        if let Some(named) = gap.players.iter().find(|p| p.id == player.id) {
            let label = gap_label(gap);
            if named.differs {
                differs.push(format!("a different {label}"));
            } else {
                lacks.push(format!("no {label}"));
            }
        }
    }
    if lacks.is_empty() && differs.is_empty() {
        return None;
    }
    let whose = if you {
        "Your".to_owned()
    } else {
        format!("{}'s", player.callsign)
    };
    lacks.extend(differs);
    Some(format!(
        "{whose} game differs from the host's: {}.",
        list(&lacks)
    ))
}

/// The hint for a player selected in the lobby: "Hawk: on Linux."
pub fn hint_line(player: &LobbyPlayer) -> String {
    let system = match player.platform {
        crate::wire::Platform::Unknown => "an unknown system".to_owned(),
        platform => platform.name().to_owned(),
    };
    format!("{}: on {system}.", player.callsign)
}
