//! Stage L's wire (protocol 10, slice L2): the Content and Content gaps
//! messages and the lobby player's build, at their limits and past them.
//! Every bound is refused by the writer and by the reader. The round trip,
//! fuzz and golden tests cover the samples
//! ([`super::samples::compatibility_messages`]).

use super::bits;
use super::messages::{
    Build, Content, ContentGaps, ContentItem, Gap, GapPlayer, Importer, ItemKind, Message,
    content_limits, kind,
};
use super::{WireError, samples};
use tore_codec::{BitWriter, CodecError};

fn round_trip(message: &Message) {
    let bytes = message.encode().unwrap();
    assert!(bytes.len() <= super::limits::MESSAGE);
    assert_eq!(&Message::decode(message.kind(), &bytes).unwrap(), message);
}

fn content(items: Vec<ContentItem>) -> Message {
    Message::Content(Box::new(Content {
        items,
        ..samples::content()
    }))
}

fn gaps(gaps: Vec<Gap>) -> Message {
    Message::ContentGaps(Box::new(ContentGaps {
        gaps,
        ..samples::content_gaps()
    }))
}

fn item(kind: ItemKind, key: &str) -> ContentItem {
    ContentItem {
        kind,
        key: key.into(),
        digest: 7,
    }
}

fn gap(kind: ItemKind, key: &str, players: &[u8]) -> Gap {
    Gap {
        kind,
        key: key.into(),
        label: "Su-27 Flanker".into(),
        host_lacks: false,
        players: players
            .iter()
            .map(|&id| GapPlayer { id, differs: false })
            .collect(),
    }
}

/// `count` distinct weapon keys of `len` bytes each, in the wire's order.
fn keys(count: usize, len: usize) -> Vec<String> {
    (0..count)
        .map(|i| format!("{i:0>len$}"))
        .collect::<Vec<_>>()
}

// The raw codings, written field by field with none of the writer's checks,
// so the reader's own refusals can be tried.

fn raw_head(w: &mut BitWriter, build: u64, importer: Option<(&str, &str)>) {
    w.write_bits(build, 2).unwrap();
    w.write_bool(importer.is_some());
    if let Some((version, commit)) = importer {
        w.write_str(version).unwrap();
        w.write_str(commit).unwrap();
    }
}

fn raw_content(build: u64, importer: Option<(&str, &str)>, items: &[(u64, &str)]) -> Vec<u8> {
    let mut w = BitWriter::new();
    raw_head(&mut w, build, importer);
    w.write_varint(items.len() as u64);
    for (kind, key) in items {
        w.write_bits(*kind, 2).unwrap();
        w.write_str(key).unwrap();
        w.write_bits(0x5A5A, 64).unwrap();
    }
    bits::finish(w)
}

/// A gap as (kind, key, label, host lacks, player ids).
type RawGap<'a> = (u64, &'a str, &'a str, bool, &'a [u8]);

fn raw_gaps(build: u64, gaps: &[RawGap<'_>]) -> Vec<u8> {
    let mut w = BitWriter::new();
    raw_head(&mut w, build, None);
    w.write_varint(gaps.len() as u64);
    for (kind, key, label, host_lacks, players) in gaps {
        w.write_bits(*kind, 2).unwrap();
        w.write_str(key).unwrap();
        w.write_str(label).unwrap();
        w.write_bool(*host_lacks);
        w.write_varint(players.len() as u64);
        for id in *players {
            w.write_bits(u64::from(*id), 8).unwrap();
            w.write_bool(true);
        }
    }
    bits::finish(w)
}

fn read_content(bytes: &[u8]) -> Result<Message, WireError> {
    Message::decode(kind::CONTENT, bytes)
}

fn read_gaps(bytes: &[u8]) -> Result<Message, WireError> {
    Message::decode(kind::CONTENT_GAPS, bytes)
}

#[test]
fn the_two_kinds_are_37_and_38_and_go_their_ways() {
    assert_eq!((kind::CONTENT, kind::CONTENT_GAPS), (37, 38));
    let [content, _, gaps, _] = &samples::compatibility_messages()[..] else {
        panic!("four samples");
    };
    assert_eq!((content.kind(), gaps.kind()), (37, 38));
    assert!(content.from_player());
    assert!(!gaps.from_player());
    assert!(Message::decode(39, &[]).is_err());
}

#[test]
fn builds_have_their_codes() {
    for (build, code) in [(Build::Unknown, 0), (Build::V10, 1), (Build::V102F, 2)] {
        assert_eq!(build.code(), code);
        assert_eq!(Build::from_code(code), Some(build));
    }
    assert_eq!(Build::from_code(3), None);
    assert_eq!(Build::default(), Build::Unknown);
    for (kind, code) in [
        (ItemKind::Aircraft, 0),
        (ItemKind::Theater, 1),
        (ItemKind::Weapon, 2),
        (ItemKind::Shared, 3),
    ] {
        assert_eq!(kind.code(), code);
        assert_eq!(ItemKind::from_code(code), Some(kind));
    }
    // Sorting items puts them in the wire's order: by kind, then key.
    let mut items = samples::content().items;
    items.reverse();
    items.sort();
    assert_eq!(items, samples::content().items);
}

#[test]
fn content_round_trips_at_its_limits() {
    // The most: 1,024 items with 32-byte keys, and the importer's longest
    // strings. About 42 KB, inside a message.
    let mut items: Vec<ContentItem> = keys(content_limits::ITEMS - 1, content_limits::KEY_BYTES)
        .iter()
        .map(|key| ContentItem {
            kind: ItemKind::Weapon,
            key: key.clone(),
            digest: u64::MAX,
        })
        .collect();
    items.push(item(ItemKind::Shared, ""));
    let most = Message::Content(Box::new(Content {
        build: Build::V10,
        importer: Some(Importer {
            version: "v".repeat(content_limits::IMPORTER_BYTES),
            commit: "c".repeat(content_limits::IMPORTER_BYTES),
        }),
        items,
    }));
    round_trip(&most);
    assert!(most.encode().unwrap().len() < 45_000);
    // The least: one item. Every printable character is a key's.
    round_trip(&content(vec![item(ItemKind::Aircraft, "x")]));
    let printable: String = (b' '..=b'~').map(char::from).collect();
    let mut items: Vec<ContentItem> = printable
        .as_bytes()
        .chunks(content_limits::KEY_BYTES)
        .map(|chunk| item(ItemKind::Theater, std::str::from_utf8(chunk).unwrap()))
        .collect();
    items.sort();
    round_trip(&content(items));
    // Empty importer strings are allowed.
    round_trip(&Message::Content(Box::new(Content {
        importer: Some(Importer::default()),
        ..samples::content()
    })));
}

#[test]
fn content_gaps_round_trip_at_their_limits() {
    // A gap naming 64 players, each lacking it or differing.
    let mut wide = gap(ItemKind::Aircraft, "F18.PT", &[]);
    wide.label = "l".repeat(content_limits::LABEL_BYTES);
    wide.key = "k".repeat(content_limits::KEY_BYTES);
    wide.players = (0..content_limits::GAP_PLAYERS as u8)
        .map(|id| GapPlayer {
            id: id * 3,
            differs: id % 2 == 1,
        })
        .collect();
    round_trip(&gaps(vec![wide]));
    // 1,024 gaps of the usual size, one player each.
    let many: Vec<Gap> = keys(content_limits::GAPS, 12)
        .iter()
        .map(|key| gap(ItemKind::Weapon, key, &[200]))
        .collect();
    round_trip(&gaps(many));
    // None at all, from a host whose import says nothing of itself.
    round_trip(&Message::ContentGaps(Box::default()));
}

#[test]
fn the_largest_content_gaps_do_not_fit_a_message_and_the_writer_says_so() {
    // 1,024 gaps of the longest key and label naming 64 players each is
    // about 175 KB: the writer refuses it as it refuses any body over 64 KB
    // (the host keeps its gaps inside one message; agent decision, L2).
    let mut most: Vec<Gap> = keys(content_limits::GAPS, content_limits::KEY_BYTES)
        .iter()
        .map(|key| gap(ItemKind::Weapon, key, &[]))
        .collect();
    for gap in &mut most {
        gap.label = "l".repeat(content_limits::LABEL_BYTES);
        gap.players = (0..content_limits::GAP_PLAYERS as u8)
            .map(|id| GapPlayer { id, differs: true })
            .collect();
    }
    assert!(matches!(
        gaps(most).encode(),
        Err(WireError::TooMany {
            what: "message bytes",
            ..
        })
    ));
}

#[test]
fn the_writer_refuses_every_bound_of_content() {
    let refuses = |message: Message, expected: WireError| {
        assert_eq!(message.encode(), Err(expected), "{message:?}");
    };
    let too_many = WireError::TooMany {
        what: "content items",
        limit: content_limits::ITEMS,
    };
    let items = keys(content_limits::ITEMS + 1, 8)
        .iter()
        .map(|key| item(ItemKind::Weapon, key))
        .collect();
    refuses(content(items), too_many);
    refuses(content(Vec::new()), WireError::Invalid("content items"));
    refuses(
        content(vec![item(ItemKind::Weapon, &"K".repeat(33))]),
        WireError::TooMany {
            what: "key bytes",
            limit: content_limits::KEY_BYTES,
        },
    );
    for key in ["AIM\t9X", "AIM9X\u{7f}", "AIM9X.JT\n", "\u{e9}"] {
        refuses(
            content(vec![item(ItemKind::Weapon, key)]),
            WireError::Invalid("content key"),
        );
    }
    // The shared item's key is empty, and only its.
    refuses(
        content(vec![item(ItemKind::Shared, "shared")]),
        WireError::Invalid("content key"),
    );
    refuses(
        content(vec![item(ItemKind::Aircraft, "")]),
        WireError::Invalid("content key"),
    );
    // Out of order, a key twice in a kind, and two shared items.
    for items in [
        vec![
            item(ItemKind::Theater, "UKR"),
            item(ItemKind::Aircraft, "F18.PT"),
        ],
        vec![item(ItemKind::Weapon, "B"), item(ItemKind::Weapon, "A")],
        vec![item(ItemKind::Weapon, "A"), item(ItemKind::Weapon, "A")],
        vec![item(ItemKind::Shared, ""), item(ItemKind::Shared, "")],
    ] {
        refuses(content(items), WireError::Invalid("content order"));
    }
    // The same key in two kinds is two items.
    round_trip(&content(vec![
        item(ItemKind::Aircraft, "F18.PT"),
        item(ItemKind::Weapon, "F18.PT"),
    ]));
    for (version, commit, what) in [
        ("v".repeat(65), String::new(), "importer version bytes"),
        (String::new(), "c".repeat(65), "importer commit bytes"),
    ] {
        refuses(
            Message::Content(Box::new(Content {
                importer: Some(Importer { version, commit }),
                ..samples::content()
            })),
            WireError::TooMany {
                what,
                limit: content_limits::IMPORTER_BYTES,
            },
        );
    }
}

#[test]
fn the_writer_refuses_every_bound_of_content_gaps() {
    let refuses = |message: Message, expected: WireError| {
        assert_eq!(message.encode(), Err(expected), "{message:?}");
    };
    let many = keys(content_limits::GAPS + 1, 8)
        .iter()
        .map(|key| gap(ItemKind::Weapon, key, &[1]))
        .collect();
    refuses(
        gaps(many),
        WireError::TooMany {
            what: "gaps",
            limit: content_limits::GAPS,
        },
    );
    let mut long = gap(ItemKind::Aircraft, "SU27.PT", &[1]);
    long.label = "l".repeat(65);
    refuses(
        gaps(vec![long]),
        WireError::TooMany {
            what: "label bytes",
            limit: content_limits::LABEL_BYTES,
        },
    );
    // The host lacks it, yet names it.
    let mut named = gap(ItemKind::Aircraft, "SU27.PT", &[]);
    named.host_lacks = true;
    refuses(gaps(vec![named]), WireError::Invalid("gap label"));
    // Nobody cannot use it, though the host has it.
    refuses(
        gaps(vec![gap(ItemKind::Aircraft, "SU27.PT", &[])]),
        WireError::Invalid("gap players"),
    );
    let ids: Vec<u8> = (0..=64).collect();
    refuses(
        gaps(vec![gap(ItemKind::Aircraft, "SU27.PT", &ids)]),
        WireError::TooMany {
            what: "gap players",
            limit: content_limits::GAP_PLAYERS,
        },
    );
    // Players out of order, or one twice.
    for ids in [[4, 1], [4, 4]] {
        refuses(
            gaps(vec![gap(ItemKind::Aircraft, "SU27.PT", &ids)]),
            WireError::Invalid("gap players"),
        );
    }
    // Gaps out of Content's order, and a bad key.
    refuses(
        gaps(vec![
            gap(ItemKind::Weapon, "AIM9X.JT", &[1]),
            gap(ItemKind::Aircraft, "SU27.PT", &[1]),
        ]),
        WireError::Invalid("content order"),
    );
    refuses(
        gaps(vec![gap(ItemKind::Shared, "x", &[1])]),
        WireError::Invalid("content key"),
    );
    refuses(
        Message::ContentGaps(Box::new(ContentGaps {
            host_importer: Some(Importer {
                version: "v".repeat(65),
                commit: String::new(),
            }),
            ..ContentGaps::default()
        })),
        WireError::TooMany {
            what: "importer version bytes",
            limit: content_limits::IMPORTER_BYTES,
        },
    );
}

#[test]
fn the_reader_refuses_every_bound_of_content() {
    // The raw coding is the writer's: a good body reads.
    let good = raw_content(2, Some(("0.1.4", "48d62dac")), &[(0, "F18.PT"), (3, "")]);
    assert!(read_content(&good).is_ok());
    assert_eq!(
        read_content(&raw_content(3, None, &[(0, "F18.PT")])),
        Err(WireError::Invalid("build"))
    );
    assert_eq!(
        read_content(&raw_content(0, None, &[])),
        Err(WireError::Invalid("content items"))
    );
    let mut w = BitWriter::new();
    raw_head(&mut w, 0, None);
    w.write_varint(1_025);
    assert_eq!(
        read_content(&bits::finish(w)),
        Err(WireError::TooMany {
            what: "content items",
            limit: content_limits::ITEMS,
        })
    );
    // A count the bytes cannot hold.
    let mut w = BitWriter::new();
    raw_head(&mut w, 0, None);
    w.write_varint(1_000);
    assert_eq!(
        read_content(&bits::finish(w)),
        Err(CodecError::UnexpectedEnd.into())
    );
    assert!(matches!(
        read_content(&raw_content(0, None, &[(2, "K".repeat(33).as_str())])),
        Err(WireError::TooMany {
            what: "key bytes",
            ..
        })
    ));
    for items in [
        [(2, "AIM\t9X")],
        [(2, "\u{e9}")],
        [(3, "shared")],
        [(0, "")],
    ] {
        assert_eq!(
            read_content(&raw_content(0, None, &items)),
            Err(WireError::Invalid("content key")),
            "{items:?}"
        );
    }
    for items in [
        [(1, "UKR"), (0, "F18.PT")],
        [(2, "A"), (2, "A")],
        [(3, ""), (3, "")],
    ] {
        assert_eq!(
            read_content(&raw_content(0, None, &items)),
            Err(WireError::Invalid("content order")),
            "{items:?}"
        );
    }
    for (version, commit, what) in [
        ("v".repeat(65), String::new(), "importer version bytes"),
        (String::new(), "c".repeat(65), "importer commit bytes"),
    ] {
        assert_eq!(
            read_content(&raw_content(0, Some((&version, &commit)), &[(0, "F18.PT")])),
            Err(WireError::TooMany {
                what,
                limit: content_limits::IMPORTER_BYTES,
            })
        );
    }
    // Anything after the body.
    let mut long = good.clone();
    long.push(0);
    assert_eq!(read_content(&long), Err(WireError::Trailing));
}

#[test]
fn the_reader_refuses_every_bound_of_content_gaps() {
    let good: [RawGap<'_>; 2] = [
        (0, "SU27.PT", "Su-27 Flanker", false, &[3]),
        (1, "VIET", "", true, &[]),
    ];
    assert!(read_gaps(&raw_gaps(1, &good)).is_ok());
    assert!(read_gaps(&raw_gaps(0, &[])).is_ok());
    assert_eq!(
        read_gaps(&raw_gaps(3, &[])),
        Err(WireError::Invalid("build"))
    );
    let mut w = BitWriter::new();
    raw_head(&mut w, 0, None);
    w.write_varint(1_025);
    assert_eq!(
        read_gaps(&bits::finish(w)),
        Err(WireError::TooMany {
            what: "gaps",
            limit: content_limits::GAPS,
        })
    );
    let mut w = BitWriter::new();
    raw_head(&mut w, 0, None);
    w.write_varint(500);
    assert_eq!(
        read_gaps(&bits::finish(w)),
        Err(CodecError::UnexpectedEnd.into())
    );
    let long_label = "l".repeat(65);
    let ids: Vec<u8> = (0..=64).collect();
    let cases: [(RawGap<'_>, WireError); 7] = [
        (
            (0, "SU27.PT", long_label.as_str(), false, &[1]),
            WireError::TooMany {
                what: "label bytes",
                limit: content_limits::LABEL_BYTES,
            },
        ),
        (
            (0, "SU27.PT", "Su-27", true, &[]),
            WireError::Invalid("gap label"),
        ),
        (
            (0, "SU27.PT", "Su-27", false, &[]),
            WireError::Invalid("gap players"),
        ),
        (
            (0, "SU27.PT", "Su-27", false, ids.as_slice()),
            WireError::TooMany {
                what: "gap players",
                limit: content_limits::GAP_PLAYERS,
            },
        ),
        (
            (0, "SU27.PT", "Su-27", false, &[4, 1]),
            WireError::Invalid("gap players"),
        ),
        (
            (0, "SU27.PT", "Su-27", false, &[4, 4]),
            WireError::Invalid("gap players"),
        ),
        (
            (3, "x", "shared", false, &[1]),
            WireError::Invalid("content key"),
        ),
    ];
    for (gap, error) in cases {
        assert_eq!(read_gaps(&raw_gaps(0, &[gap])), Err(error), "{gap:?}");
    }
    assert_eq!(
        read_gaps(&raw_gaps(0, &[good[1], good[0]])),
        Err(WireError::Invalid("content order"))
    );
}

#[test]
fn every_lobby_build_round_trips_and_code_three_is_invalid() {
    let with = |build: Build| {
        let mut lobby = samples::lobby();
        for player in &mut lobby.players {
            player.build = build;
        }
        Message::Lobby(Box::new(lobby))
    };
    for build in [Build::Unknown, Build::V10, Build::V102F] {
        round_trip(&with(build));
    }
    // The first player's build: the one bit where 1.0 (code 1) and unknown
    // (code 0) differ is its lowest; setting the next one too makes code 3.
    let unknown = with(Build::Unknown).encode().unwrap();
    let disc = with(Build::V10).encode().unwrap();
    assert_eq!(unknown.len(), disc.len());
    let bit = (0..unknown.len() * 8)
        .find(|&b| (unknown[b / 8] ^ disc[b / 8]) & (1 << (b % 8)) != 0)
        .unwrap();
    let mut bad = disc.clone();
    bad[(bit + 1) / 8] |= 1 << ((bit + 1) % 8);
    assert_eq!(
        Message::decode(kind::LOBBY, &bad),
        Err(WireError::Invalid("build"))
    );
}
