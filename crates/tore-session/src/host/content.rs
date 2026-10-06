//! Stage L on the host: each player's content, the gaps (the items not every
//! human can use) and the host's words about them (docs/ARCHITECTURE.md,
//! "Compatibility"; the wire is docs/formats/net-protocol.md, "Compatibility
//! (stage L)").
//!
//! A game's content is [`GameContent`]: its items (`tore_world::content`)
//! and where its import came from (`tore_import::source`). A joining game
//! sends its items' digests in a Content message; the host keeps them as a
//! [`PlayerContent`] and compares every counted player's with its own. An
//! item is in a **gap** when the host lacks it, or some counted player lacks
//! it or has it with another digest than the host's. Every connected player
//! whose Content has arrived counts, in the lobby, flying, watching or
//! unable (John, 2026-10-05).
//!
//! The words a player reads about a gap are made here, in one place, from a
//! gap and the players' callsigns: the host's refusal of a mission or a
//! loadout that uses an item in a gap ([`not_everyone`]), the third-person
//! reason a player cannot fly the mission ([`unable_words`]), and the log
//! lines ([`ContentLog`]). A player's game words the same refusals from the
//! Content gaps message with the same functions (`client::content`).

use crate::wire::messages::{
    self, Build as WireBuild, Content as WireContent, ContentGaps, ContentItem, Gap, GapPlayer,
    Importer as WireImporter, ItemKind, content_limits,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex, Weak};
use tore_import::source::{self, Importer, Source};
use tore_world::content::{Content, Kind, SHARED_KEY};
use tore_world::mission::MissionSpec;
use tore_world::resources::ResourceSource;

/// An import's resources, as the host and the client hold them.
pub type Resources = BTreeMap<String, Vec<u8>>;

/// An item's kind and its key as the wire codes it (the shared item's key is
/// empty).
pub type ItemKey = (ItemKind, String);

/// The most bytes of a Content gaps message: the transport's limit for one
/// reliable message.
pub const MESSAGE_LIMIT: usize = tore_net::MAX_MESSAGE_LEN;

/// The most items a log line names before "and N more".
const LOG_ITEMS: usize = 12;

/// The most content log entries kept for a caller that does not read them.
const LOG_LIMIT: usize = 256;

/// The label of the shared item.
pub const SHARED_LABEL: &str = "shared flight data";

/// A game's content and where its import came from: what a host compares
/// players with, and what a joining game sends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameContent {
    /// The items and the names each reads.
    pub content: Content,
    /// The Fighters Anthology build and the T.O.R.E that made the import.
    pub source: Source,
}

/// Contents already computed, by the resources they were computed from.
/// Holding a weak reference keeps `Arc::get_mut` from changing those
/// resources behind the cache's back, so an entry whose resources are alive
/// is always right.
static COMPUTED: Mutex<Vec<(Weak<Resources>, Arc<GameContent>)>> = Mutex::new(Vec::new());

impl GameContent {
    /// The content of `resources`, with the source the pack's own entry
    /// gives (unknown for a pack made before stage L: a caller that knows
    /// the data folder uses [`GameContent::read`]).
    pub fn of(resources: &Resources) -> Self {
        let source = resources
            .get(source::RESOURCE)
            .and_then(|bytes| Source::parse(bytes))
            .unwrap_or(Source::UNKNOWN);
        Self::with_source(resources, source)
    }

    /// The content of the import in `data_dir` whose resources are
    /// `resources`, with its source read as `Source::read` reads it (the
    /// pack's entry, else the import report's build).
    pub fn read(data_dir: &Path, resources: &Resources) -> Self {
        Self::with_source(resources, Source::read(data_dir, resources))
    }

    /// The content of `resources` with `source`.
    pub fn with_source(resources: &Resources, source: Source) -> Self {
        Self {
            content: Content::of(resources),
            source,
        }
    }

    /// [`GameContent::of`] for `resources`, computed once for each set of
    /// resources while they live: a host and the clients a test or a
    /// hosting game gives the same resources share one computation (agent
    /// decision; the design computes the content once for each import).
    pub fn shared(resources: &Arc<Resources>) -> Arc<Self> {
        let mut computed = COMPUTED.lock().unwrap_or_else(|e| e.into_inner());
        computed.retain(|(weak, _)| weak.strong_count() > 0);
        if let Some((_, content)) = computed
            .iter()
            .find(|(weak, _)| weak.upgrade().is_some_and(|r| Arc::ptr_eq(&r, resources)))
        {
            return Arc::clone(content);
        }
        let content = Arc::new(Self::of(resources));
        computed.push((Arc::downgrade(resources), Arc::clone(&content)));
        content
    }

    /// The import's Fighters Anthology build, as the wire codes it.
    pub fn build(&self) -> WireBuild {
        wire_build(self.source.build)
    }

    /// The T.O.R.E that made the import, as the wire carries it: each string
    /// cut to the wire's 64 bytes (agent decision; a real version and commit
    /// are far shorter).
    pub fn importer(&self) -> Option<WireImporter> {
        self.source.importer.as_ref().map(|importer| WireImporter {
            version: clip(&importer.version, content_limits::IMPORTER_BYTES),
            commit: clip(&importer.commit, content_limits::IMPORTER_BYTES),
        })
    }

    /// Every item's digest, by the wire's kind and key. An item whose key
    /// the wire cannot carry (over 32 bytes, or not printable ASCII) is left
    /// out (agent decision: none of the real keys is).
    pub fn digests(&self) -> BTreeMap<ItemKey, u64> {
        self.content
            .items()
            .iter()
            .map(|item| {
                let kind = item_kind(item.kind);
                ((kind, wire_key(kind, &item.key)), item.digest)
            })
            .filter(|((kind, key), _)| key_fits(*kind, key))
            .take(content_limits::ITEMS)
            .collect()
    }

    /// The Content message a joining game sends, or `None` for an import
    /// with no item at all (the wire wants at least one; such a game can
    /// fly nothing, so it sends none and the host never counts it).
    pub fn message(&self) -> Option<WireContent> {
        let items: Vec<ContentItem> = self
            .digests()
            .into_iter()
            .map(|((kind, key), digest)| ContentItem { kind, key, digest })
            .collect();
        (!items.is_empty()).then(|| WireContent {
            build: self.build(),
            importer: self.importer(),
            items,
        })
    }

    /// The labels of every item the import has, for the words.
    pub fn labels(&self, resources: &Resources) -> Labels {
        Labels(
            self.digests()
                .into_keys()
                .filter_map(|(kind, key)| {
                    let label = label_of(kind, &key, resources)?;
                    Some(((kind, key), label))
                })
                .collect(),
        )
    }
}

/// The wire's kind of an item kind.
pub fn item_kind(kind: Kind) -> ItemKind {
    match kind {
        Kind::Aircraft => ItemKind::Aircraft,
        Kind::Theater => ItemKind::Theater,
        Kind::Weapon => ItemKind::Weapon,
        Kind::Shared => ItemKind::Shared,
    }
}

/// The item kind of a wire kind.
pub fn world_kind(kind: ItemKind) -> Kind {
    match kind {
        ItemKind::Aircraft => Kind::Aircraft,
        ItemKind::Theater => Kind::Theater,
        ItemKind::Weapon => Kind::Weapon,
        ItemKind::Shared => Kind::Shared,
    }
}

/// The key the wire carries for an item: the shared item's is empty.
pub fn wire_key(kind: ItemKind, key: &str) -> String {
    if kind == ItemKind::Shared {
        String::new()
    } else {
        key.to_owned()
    }
}

/// The key `tore_world::content` gives an item of the wire.
pub fn world_key(kind: ItemKind, key: &str) -> String {
    if kind == ItemKind::Shared {
        SHARED_KEY.to_owned()
    } else {
        key.to_owned()
    }
}

/// Whether the wire can carry `key` for an item of `kind`.
fn key_fits(kind: ItemKind, key: &str) -> bool {
    (kind == ItemKind::Shared) == key.is_empty()
        && key.len() <= content_limits::KEY_BYTES
        && key.bytes().all(|b| (b' '..=b'~').contains(&b))
}

/// The wire's build of a source's.
pub fn wire_build(build: Option<source::Build>) -> WireBuild {
    match build {
        None => WireBuild::Unknown,
        Some(source::Build::Disc10) => WireBuild::V10,
        Some(source::Build::V102F) => WireBuild::V102F,
    }
}

/// The version a player says for a wire build: `1.0` or `1.02F`; `None` when
/// unknown.
pub fn build_version(build: WireBuild) -> Option<&'static str> {
    match build {
        WireBuild::Unknown => None,
        WireBuild::V10 => Some(source::Build::Disc10.version()),
        WireBuild::V102F => Some(source::Build::V102F.version()),
    }
}

/// The source a Content message describes, for the log's words.
pub fn source_of(build: WireBuild, importer: Option<&WireImporter>) -> Source {
    Source {
        build: match build {
            WireBuild::Unknown => None,
            WireBuild::V10 => Some(source::Build::Disc10),
            WireBuild::V102F => Some(source::Build::V102F),
        },
        importer: importer.map(|importer| Importer {
            version: importer.version.clone(),
            commit: importer.commit.clone(),
        }),
    }
}

/// `text` cut to at most `bytes` bytes on a character boundary.
pub fn clip(text: &str, bytes: usize) -> String {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// The name a player reads for an item, from `resources`: the aircraft's
/// label, the theater's grid name, the weapon record's name, or
/// [`SHARED_LABEL`]; at most 64 bytes. `None` when the import cannot name it.
pub fn label_of(kind: ItemKind, key: &str, resources: &Resources) -> Option<String> {
    let label = match kind {
        ItemKind::Aircraft => tore_formats::aircraft::AircraftId::parse(key)
            .ok()?
            .label()
            .to_owned(),
        ItemKind::Theater => resources.theater_label(key).ok().flatten()?,
        ItemKind::Weapon => {
            let bytes = resources.get(key)?;
            tore_formats::weapons::Weapon::parse(key, bytes).ok()?.name
        }
        ItemKind::Shared => SHARED_LABEL.to_owned(),
    };
    let label = clip(label.trim(), content_limits::LABEL_BYTES);
    (!label.is_empty()).then_some(label)
}

/// The labels of an import's items.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Labels(BTreeMap<ItemKey, String>);

impl Labels {
    /// The item's label, if the import names it.
    pub fn get(&self, kind: ItemKind, key: &str) -> Option<&str> {
        self.0.get(&(kind, key.to_owned())).map(String::as_str)
    }

    /// The item's label, or its key when the import does not name it.
    pub fn or_key(&self, kind: ItemKind, key: &str) -> String {
        self.get(kind, key).map_or_else(
            || {
                if kind == ItemKind::Shared {
                    SHARED_LABEL.to_owned()
                } else {
                    key.to_owned()
                }
            },
            str::to_owned,
        )
    }
}

/// A player's content as its Content message said, kept by the host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerContent {
    pub build: WireBuild,
    pub importer: Option<WireImporter>,
    /// Each item's digest, by kind and key.
    pub items: BTreeMap<ItemKey, u64>,
}

impl From<WireContent> for PlayerContent {
    fn from(content: WireContent) -> Self {
        Self {
            build: content.build,
            importer: content.importer,
            items: content
                .items
                .into_iter()
                .map(|item| ((item.kind, item.key), item.digest))
                .collect(),
        }
    }
}

/// A counted player: its lobby id, callsign and content.
#[derive(Clone, Copy, Debug)]
pub struct Counted<'a> {
    pub id: u8,
    pub callsign: &'a str,
    pub content: &'a PlayerContent,
}

/// The gap of the item `key`, if not every counted player and the host can
/// use it: the host lacks it, or a player lacks it or has another digest.
/// When the host lacks it, the players named are those that lack it too (a
/// player that has it has nothing to compare with). `players` are in lobby
/// id order.
pub fn gap_of(
    key: &ItemKey,
    host: &BTreeMap<ItemKey, u64>,
    labels: &Labels,
    players: &[Counted<'_>],
) -> Option<Gap> {
    let ours = host.get(key);
    let named: Vec<GapPlayer> = players
        .iter()
        .filter_map(|player| match (player.content.items.get(key), ours) {
            (None, _) => Some(GapPlayer {
                id: player.id,
                differs: false,
            }),
            (Some(theirs), Some(ours)) if theirs != ours => Some(GapPlayer {
                id: player.id,
                differs: true,
            }),
            _ => None,
        })
        .collect();
    let host_lacks = ours.is_none();
    if !host_lacks && named.is_empty() {
        return None;
    }
    Some(Gap {
        kind: key.0,
        key: key.1.clone(),
        label: if host_lacks {
            String::new()
        } else {
            labels.get(key.0, &key.1).unwrap_or_default().to_owned()
        },
        host_lacks,
        players: named,
    })
}

/// Every gap among the host's items and the counted players', in Content's
/// order (kind, then key). An item only the host has and every player has
/// with its digest is in none; an item no one has is in none.
pub fn gaps(host: &BTreeMap<ItemKey, u64>, labels: &Labels, players: &[Counted<'_>]) -> Vec<Gap> {
    let mut keys: BTreeSet<&ItemKey> = host.keys().collect();
    for player in players {
        keys.extend(player.content.items.keys());
    }
    keys.into_iter()
        .filter_map(|key| gap_of(key, host, labels, players))
        .collect()
}

/// The Content gaps message for `gaps`, kept inside one message: when the
/// whole list would pass [`MESSAGE_LIMIT`] (or the wire's 1,024 gaps), the
/// shared item, the aircraft and the theaters are kept first and weapons
/// last, the list cut where it fits. Returns the message and how many gaps
/// were left out of it (agent decision: the host's own refusals still see
/// every gap; a gap left out is only not dimmed in a player's creator).
pub fn message(content: &GameContent, gaps: &[Gap]) -> (ContentGaps, usize) {
    let header = |gaps: Vec<Gap>| ContentGaps {
        host_build: content.build(),
        host_importer: content.importer(),
        gaps,
    };
    let fits = |gaps: &[Gap]| {
        gaps.len() <= content_limits::GAPS
            && messages::Message::ContentGaps(Box::new(header(gaps.to_vec())))
                .encode()
                .is_ok_and(|body| body.len() <= MESSAGE_LIMIT)
    };
    if fits(gaps) {
        return (header(gaps.to_vec()), 0);
    }
    // Kept first: the shared item, then aircraft, theaters and weapons.
    let rank = |kind: ItemKind| match kind {
        ItemKind::Shared => 0,
        ItemKind::Aircraft => 1,
        ItemKind::Theater => 2,
        ItemKind::Weapon => 3,
    };
    let mut ranked: Vec<&Gap> = gaps.iter().collect();
    ranked.sort_by_key(|gap| rank(gap.kind));
    let take = |n: usize| -> Vec<Gap> {
        let mut kept: Vec<Gap> = ranked[..n].iter().map(|gap| (*gap).clone()).collect();
        kept.sort_by(|a, b| (a.kind, &a.key).cmp(&(b.kind, &b.key)));
        kept
    };
    // The longest prefix that fits, by halving.
    let (mut low, mut high) = (0, ranked.len().min(content_limits::GAPS));
    while low < high {
        let mid = (low + high).div_ceil(2);
        if fits(&take(mid)) {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    (header(take(low)), gaps.len() - low)
}

// ----- The words ------------------------------------------------------------

/// Up to three names, then "and N more": "Hawk", "Hawk and Viper", "Hawk,
/// Viper and Ace", "Hawk, Viper, Ace and 2 more".
pub fn list(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        [a, b, c] => format!("{a}, {b} and {c}"),
        [a, b, c, rest @ ..] => format!("{a}, {b}, {c} and {} more", rest.len()),
    }
}

/// Who a gap's words name for the host: the house's callsign in a game a
/// player hosts, else "the server".
pub fn host_name(house: Option<&str>) -> String {
    house.map_or_else(|| "the server".to_owned(), str::to_owned)
}

/// Whose game lacks or has differently the item `label` in `gap`: "Hawk's
/// game has no Su-27", "the games of Hawk and Viper have no Su-27", "Ace's
/// Su-27 differs from the host's", "the server has no Su-27", joined by
/// "; ". `name` gives a player's callsign by lobby id; `house` is the
/// hosting player's callsign, `None` on a dedicated server.
pub fn whose(gap: &Gap, label: &str, name: &dyn Fn(u8) -> String, house: Option<&str>) -> String {
    let mut clauses = Vec::new();
    if gap.host_lacks {
        clauses.push(match house {
            Some(house) => format!("{house}'s game has no {label}"),
            None => format!("the server has no {label}"),
        });
    }
    let group = |differs: bool| -> Vec<String> {
        gap.players
            .iter()
            .filter(|p| p.differs == differs)
            .map(|p| name(p.id))
            .collect()
    };
    match group(false).as_slice() {
        [] => {}
        [one] => clauses.push(format!("{one}'s game has no {label}")),
        many => clauses.push(format!("the games of {} have no {label}", list(many))),
    }
    match group(true).as_slice() {
        [] => {}
        [one] => clauses.push(format!("{one}'s {label} differs from the host's")),
        many => clauses.push(format!(
            "the {label} of {} differs from the host's",
            list(many)
        )),
    }
    clauses.join("; ")
}

/// The line that refuses a choice in a gap, the same on the host (its
/// Refused) and in the King's creator: "Not everyone can fly the Su-27:
/// Hawk's game has no Su-27.", "Not everyone has Vietnam: ...", "Not
/// everyone has the AGM-65G: ...".
pub fn not_everyone(
    gap: &Gap,
    label: &str,
    name: &dyn Fn(u8) -> String,
    house: Option<&str>,
) -> String {
    let head = match gap.kind {
        ItemKind::Aircraft => format!("Not everyone can fly the {label}"),
        ItemKind::Theater => format!("Not everyone has {label}"),
        ItemKind::Weapon => format!("Not everyone has the {label}"),
        ItemKind::Shared => "Not everyone has the same shared flight data".to_owned(),
    };
    format!("{head}: {}.", whose(gap, label, name, house))
}

/// The label a gap's words use: the gap's own (the host's name for it), or
/// its key when the host lacks it.
pub fn gap_label(gap: &Gap) -> String {
    if !gap.label.is_empty() {
        gap.label.clone()
    } else if gap.kind == ItemKind::Shared {
        SHARED_LABEL.to_owned()
    } else {
        gap.key.clone()
    }
}

/// The items a mission uses, in Content's order: the aircraft of every wing
/// that flies, its theater, the weapons of its loadouts (the `.JT` records
/// only; a tank is part of its aircraft's item).
pub fn mission_items(spec: &MissionSpec) -> Vec<ItemKey> {
    let mut items = BTreeSet::new();
    for wing in spec.wings.iter().filter(|wing| wing.count > 0) {
        items.insert((ItemKind::Aircraft, wing.aircraft.selection_key().to_owned()));
    }
    items.insert((ItemKind::Theater, spec.theater.clone()));
    for load in spec.loadout.iter().chain(spec.plane_loadouts.values()) {
        for station in &load.stations {
            if station.weapon.ends_with(".JT") {
                items.insert((ItemKind::Weapon, station.weapon.clone()));
            }
        }
    }
    items.into_iter().collect()
}

/// What follows "has no X, " for an item of `kind` a mission uses.
fn uses(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Aircraft => "which this mission flies",
        ItemKind::Theater => "where this mission flies",
        ItemKind::Weapon => "which this flight carries",
        ItemKind::Shared => "which every mission reads",
    }
}

/// Labels joined with "or": "Su-27", "Su-27 or MiG-29".
fn or_list(labels: &[String]) -> String {
    match labels {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    }
}

/// "3 files, such as CRATER.SH" or "1 file, CRATER.SH".
pub fn files_phrase(names: &[String]) -> String {
    match names {
        [] => "no file".to_owned(),
        [one] => format!("1 file, {one}"),
        [first, ..] => format!("{} files, such as {first}", names.len()),
    }
}

/// What a mission's items come to for one game against the host's:
/// the items it lacks, grouped by the first kind that has any, and the
/// items it has differently.
pub struct Shortfall {
    /// The kind of the lacking items, and them, by key.
    pub lacks: Option<(ItemKind, Vec<String>)>,
    /// The items it has with another digest than the host's.
    pub differs: Vec<ItemKey>,
}

/// A mission's `items` against what a game `has` (an item's digest, `None`
/// when it lacks it) and the host's `digests`.
pub fn shortfall(
    items: &[ItemKey],
    has: &dyn Fn(&ItemKey) -> Option<u64>,
    host: &BTreeMap<ItemKey, u64>,
) -> Shortfall {
    let mut lacks: Option<(ItemKind, Vec<String>)> = None;
    let mut differs = Vec::new();
    for key in items {
        match (has(key), host.get(key)) {
            (None, _) => match &mut lacks {
                Some((kind, keys)) if *kind == key.0 => keys.push(key.1.clone()),
                Some(_) => {}
                None => lacks = Some((key.0, vec![key.1.clone()])),
            },
            (Some(theirs), Some(ours)) if theirs != *ours => differs.push(key.clone()),
            _ => {}
        }
    }
    Shortfall { lacks, differs }
}

/// Why a player cannot fly the mission, in the third person, for every
/// player to read: "Hawk's game has no Su-27, which this mission flies.",
/// "Hawk's Su-27 differs from the host's.", "Hawk's game data differs from
/// the host's in 3 files, such as CRATER.SH.", or "Hawk's game cannot build
/// this mission." `player` is the content its game sent (`None` if none
/// came); `names` the resources its exact check found different.
pub fn unable_words(
    callsign: &str,
    player: Option<&PlayerContent>,
    host: &BTreeMap<ItemKey, u64>,
    labels: &Labels,
    spec: &MissionSpec,
    names: &[String],
) -> String {
    if let Some(player) = player {
        let items = mission_items(spec);
        let found = shortfall(&items, &|key| player.items.get(key).copied(), host);
        if let Some((kind, keys)) = found.lacks {
            let labels: Vec<String> = keys.iter().map(|key| labels.or_key(kind, key)).collect();
            return format!(
                "{callsign}'s game has no {}, {}.",
                or_list(&labels),
                uses(kind)
            );
        }
        if !found.differs.is_empty() {
            let shown: Vec<String> = found
                .differs
                .iter()
                .map(|(kind, key)| labels.or_key(*kind, key))
                .collect();
            let verb = if shown.len() == 1 {
                "differs"
            } else {
                "differ"
            };
            return format!("{callsign}'s {} {verb} from the host's.", list(&shown));
        }
    }
    if names.is_empty() {
        format!("{callsign}'s game cannot build this mission.")
    } else {
        format!(
            "{callsign}'s game data differs from the host's in {}.",
            files_phrase(names)
        )
    }
}

// ----- The report ---------------------------------------------------------------

/// The lines that describe a game's content for an operator, the same in
/// `tore-server --check` and `tore-bot --content-report`, so two imports can
/// be compared line by line: the source, the counts, then one line per item
/// with its kind, key and digest ("  aircraft F18.PT 1a2b3c4d5e6f7a8b").
pub fn report_lines(content: &GameContent) -> Vec<String> {
    let items = content.digests();
    let count = |kind: ItemKind| items.keys().filter(|(k, _)| *k == kind).count();
    let plural =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let mut lines = vec![
        format!("Content: {}", content.source.describe()),
        format!(
            "Content items: {}, {}, {}, {}",
            plural(count(ItemKind::Aircraft), "aircraft", "aircraft"),
            plural(count(ItemKind::Theater), "theater", "theaters"),
            plural(count(ItemKind::Weapon), "weapon", "weapons"),
            if count(ItemKind::Shared) > 0 {
                "the shared data"
            } else {
                "no shared data"
            }
        ),
    ];
    for ((kind, key), digest) in &items {
        lines.push(format!("  {} {digest:016x}", item_text(*kind, key)));
    }
    lines
}

// ----- The log --------------------------------------------------------------

/// What a content log line is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentLogKind {
    /// A player's Content arrived: its build, its importer and how its items
    /// differ from the host's (the `content` line).
    Player,
    /// The gaps changed (the `gaps` line).
    Gaps,
}

/// A line for the console and the log about content, with the host tick
/// (docs/DEDICATED-SERVER.md, "The log"). The host keeps these apart from
/// [`crate::HostLog`] (`Host::poll_content_log`), so a caller that does not
/// read them changes nothing (agent decision; at most 256 are kept).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentLog {
    pub tick: u64,
    pub kind: ContentLogKind,
    /// The line: "content Hawk: Fighters Anthology 1.0, imported by T.O.R.E
    /// 0.1.4 (48d62dac); the same items as the host", "gaps: aircraft
    /// SU27.PT (Hawk lacks it)", "gaps: none".
    pub text: String,
}

/// Up to [`LOG_ITEMS`] items as "aircraft SU27.PT, weapon AIM9X.JT", then
/// "and N more".
fn items_text(items: &[ItemKey]) -> String {
    let named: Vec<String> = items
        .iter()
        .take(LOG_ITEMS)
        .map(|(kind, key)| item_text(*kind, key))
        .collect();
    let more = items.len().saturating_sub(LOG_ITEMS);
    if more > 0 {
        format!("{} and {more} more", named.join(", "))
    } else {
        named.join(", ")
    }
}

/// "aircraft SU27.PT", "shared data".
fn item_text(kind: ItemKind, key: &str) -> String {
    match kind {
        ItemKind::Shared => "shared data".to_owned(),
        kind => format!("{} {key}", world_kind(kind).name()),
    }
}

/// The `content` line for a player's Content against the host's items.
pub fn player_line(
    callsign: &str,
    player: &PlayerContent,
    host: &BTreeMap<ItemKey, u64>,
) -> String {
    let source = source_of(player.build, player.importer.as_ref()).describe();
    let lacks: Vec<ItemKey> = host
        .keys()
        .filter(|key| !player.items.contains_key(*key))
        .cloned()
        .collect();
    let differs: Vec<ItemKey> = player
        .items
        .iter()
        .filter(|(key, digest)| host.get(*key).is_some_and(|ours| ours != *digest))
        .map(|(key, _)| key.clone())
        .collect();
    let extra: Vec<ItemKey> = player
        .items
        .keys()
        .filter(|key| !host.contains_key(*key))
        .cloned()
        .collect();
    let mut parts = vec![format!("content {callsign}: {source}")];
    if lacks.is_empty() && differs.is_empty() && extra.is_empty() {
        parts.push("the same items as the host".to_owned());
    }
    if !lacks.is_empty() {
        parts.push(format!("lacks {}", items_text(&lacks)));
    }
    if !differs.is_empty() {
        parts.push(format!("differs in {}", items_text(&differs)));
    }
    if !extra.is_empty() {
        parts.push(format!("has what the host lacks: {}", items_text(&extra)));
    }
    parts.join("; ")
}

/// The `gaps` line: "gaps: none", or each gap with who cannot use it,
/// "gaps: aircraft SU27.PT (Hawk lacks it); weapon AIM9X.JT (Hawk and Viper
/// differ)", and how many the message left out.
pub fn gaps_line(gaps: &[Gap], name: &dyn Fn(u8) -> String, left_out: usize) -> String {
    if gaps.is_empty() {
        return "gaps: none".to_owned();
    }
    let mut shown: Vec<String> = gaps
        .iter()
        .take(LOG_ITEMS)
        .map(|gap| {
            let mut who = Vec::new();
            if gap.host_lacks {
                who.push("the host lacks it".to_owned());
            }
            for (differs, one, many) in
                [(false, "lacks it", "lack it"), (true, "differs", "differ")]
            {
                let names: Vec<String> = gap
                    .players
                    .iter()
                    .filter(|p| p.differs == differs)
                    .map(|p| name(p.id))
                    .collect();
                if !names.is_empty() {
                    let verb = if names.len() == 1 { one } else { many };
                    who.push(format!("{} {verb}", list(&names)));
                }
            }
            format!("{} ({})", item_text(gap.kind, &gap.key), who.join("; "))
        })
        .collect();
    if gaps.len() > LOG_ITEMS {
        shown.push(format!("and {} more", gaps.len() - LOG_ITEMS));
    }
    let mut text = format!("gaps: {}", shown.join("; "));
    if left_out > 0 {
        text.push_str(&format!(
            " ({left_out} left out of the players' message, which has a size limit)"
        ));
    }
    text
}

/// A bounded queue of content log lines.
#[derive(Debug, Default)]
pub struct ContentLogs(std::collections::VecDeque<ContentLog>);

impl ContentLogs {
    /// Adds a line, dropping the oldest past [`LOG_LIMIT`].
    pub fn push(&mut self, entry: ContentLog) {
        if self.0.len() >= LOG_LIMIT {
            self.0.pop_front();
        }
        self.0.push_back(entry);
    }

    /// The oldest line.
    pub fn pop(&mut self) -> Option<ContentLog> {
        self.0.pop_front()
    }
}

// ----- The host's part --------------------------------------------------------

/// The host's stage L state: its own content and labels, the gaps last
/// found and the message that carries them, and the log lines.
pub(super) struct Compat {
    own: Arc<GameContent>,
    digests: BTreeMap<ItemKey, u64>,
    labels: Labels,
    /// Every gap now, and the message players get (which may leave some out
    /// to fit).
    gaps: Vec<Gap>,
    message: ContentGaps,
    /// A player's content arrived or a player left: the gaps are found again
    /// before the next send.
    dirty: bool,
    logs: ContentLogs,
}

impl Compat {
    pub(super) fn new(own: Arc<GameContent>, resources: &Resources) -> Self {
        let message = ContentGaps {
            host_build: own.build(),
            host_importer: own.importer(),
            gaps: Vec::new(),
        };
        Self {
            digests: own.digests(),
            labels: own.labels(resources),
            own,
            gaps: Vec::new(),
            message,
            dirty: false,
            logs: ContentLogs::default(),
        }
    }

    /// The gaps are to be found again.
    pub(super) fn touch(&mut self) {
        self.dirty = true;
    }
}

impl super::Host {
    /// The host's own content.
    pub fn content(&self) -> &GameContent {
        &self.compat.own
    }

    /// Every item not every counted player and the host can use, as last
    /// found (the host's refusals find them afresh).
    pub fn gaps(&self) -> &[Gap] {
        &self.compat.gaps
    }

    /// The next content log line, oldest first ([`ContentLog`]).
    pub fn poll_content_log(&mut self) -> Option<ContentLog> {
        self.compat.logs.pop()
    }

    /// The players who count for the gaps: every connection whose Content
    /// has arrived and that is not closing, in lobby id order.
    fn counted(&self) -> Vec<Counted<'_>> {
        let mut counted: Vec<Counted<'_>> = self
            .peers
            .values()
            .filter(|peer| !matches!(peer.stage, super::Stage::Closing { .. }))
            .filter_map(|peer| {
                Some(Counted {
                    id: peer.lobby.id,
                    callsign: &peer.callsign,
                    content: peer.content.as_ref()?,
                })
            })
            .collect();
        counted.sort_by_key(|player| player.id);
        counted
    }

    /// The callsign of the player with lobby id `id`, or "player N".
    fn callsign_of(&self, id: u8) -> String {
        self.peers
            .values()
            .find(|peer| peer.lobby.id == id)
            .map_or_else(|| format!("player {id}"), |peer| peer.callsign.clone())
    }

    /// The house's callsign in a game a player hosts.
    fn house_callsign(&self) -> Option<String> {
        self.peers
            .values()
            .find(|peer| peer.house)
            .map(|peer| peer.callsign.clone())
    }

    /// A player's Content: kept (a second one replaces the first), logged
    /// against the host's items, and the gaps found again.
    pub(super) fn content_arrived(
        &mut self,
        connection: tore_net::ConnectionId,
        content: WireContent,
    ) {
        let tick = self.world.tick();
        let content = PlayerContent::from(content);
        let Some(peer) = self.peers.get_mut(&connection) else {
            return;
        };
        let text = player_line(&peer.callsign, &content, &self.compat.digests);
        peer.content = Some(content);
        // The newcomer gets the gaps even if they do not change.
        peer.gaps_sent = false;
        self.compat.logs.push(ContentLog {
            tick,
            kind: ContentLogKind::Player,
            text,
        });
        self.compat.touch();
        // Its build shows in the lobby.
        self.lobby_dirty = true;
    }

    /// Finds the gaps again when something changed, logs them when they
    /// differ, and sends every counted player that has not had the newest
    /// gaps the Content gaps message.
    pub(super) fn send_gaps(&mut self) {
        if std::mem::take(&mut self.compat.dirty) {
            let found = {
                let counted = self.counted();
                gaps(&self.compat.digests, &self.compat.labels, &counted)
            };
            if found != self.compat.gaps {
                let (message, left_out) = message(&self.compat.own, &found);
                let text = gaps_line(&found, &|id| self.callsign_of(id), left_out);
                self.compat.gaps = found;
                self.compat.message = message;
                for peer in self.peers.values_mut() {
                    peer.gaps_sent = false;
                }
                let tick = self.world.tick();
                self.compat.logs.push(ContentLog {
                    tick,
                    kind: ContentLogKind::Gaps,
                    text,
                });
            }
        }
        let to: Vec<tore_net::ConnectionId> = self
            .peers
            .iter()
            .filter(|(_, peer)| {
                !peer.gaps_sent
                    && peer.content.is_some()
                    && !matches!(peer.stage, super::Stage::Closing { .. })
            })
            .map(|(id, _)| *id)
            .collect();
        if to.is_empty() {
            return;
        }
        let message = messages::Message::ContentGaps(Box::new(self.compat.message.clone()));
        for id in to {
            self.send(id, &message);
            if let Some(peer) = self.peers.get_mut(&id) {
                peer.gaps_sent = true;
            }
        }
    }

    /// The refusal of a choice that uses `items`, when one is in a gap now:
    /// the first such item's line ([`not_everyone`]). The shared item
    /// restricts nothing, since no choice avoids it.
    fn choice_refusal(&self, items: &[ItemKey]) -> Option<String> {
        let counted = self.counted();
        let house = self.house_callsign();
        items
            .iter()
            .filter(|key| key.0 != ItemKind::Shared)
            .find_map(|key| gap_of(key, &self.compat.digests, &self.compat.labels, &counted))
            .map(|gap| {
                not_everyone(
                    &gap,
                    &gap_label(&gap),
                    &|id| self.callsign_of(id),
                    house.as_deref(),
                )
            })
    }

    /// Why the King's mission `spec` is refused, when an aircraft or its
    /// theater is in a gap.
    pub(super) fn mission_gap_refusal(&self, spec: &MissionSpec) -> Option<String> {
        self.choice_refusal(&mission_items(spec))
    }

    /// Why a loadout is refused, when one of its weapons is in a gap.
    pub(super) fn loadout_gap_refusal(
        &self,
        load: &tore_world::mission::LoadoutSpec,
    ) -> Option<String> {
        let weapons: Vec<ItemKey> = load
            .stations
            .iter()
            .filter(|station| station.weapon.ends_with(".JT"))
            .map(|station| (ItemKind::Weapon, station.weapon.clone()))
            .collect();
        self.choice_refusal(&weapons)
    }

    /// The third-person reason a player's import cannot play the mission
    /// it refused ([`unable_words`]): the flight's mission with the
    /// loadouts when only the flight failed, else the lobby's.
    pub(super) fn unable_text(
        &self,
        connection: tore_net::ConnectionId,
        names: &[String],
        flight: bool,
    ) -> String {
        let Some(peer) = self.peers.get(&connection) else {
            return String::new();
        };
        let flown = flight
            .then(|| MissionSpec::from_text(&self.spec_text).ok())
            .flatten();
        unable_words(
            &peer.callsign,
            peer.content.as_ref(),
            &self.compat.digests,
            &self.compat.labels,
            flown.as_ref().unwrap_or(&self.spec),
            names,
        )
    }
}
