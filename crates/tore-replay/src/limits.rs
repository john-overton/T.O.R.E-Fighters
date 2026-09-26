//! Hard limits. The writer rejects input beyond them with a clear message and
//! writes nothing for that frame; the reader rejects files beyond them.
//! Agent decisions (2026-09-26), sized well above what the simulation can
//! produce: it caps live projectiles at 256, effects at 64 and smoke puffs at
//! 8,192, and Quick Mission flies at most 30 aircraft.

/// Aircraft in one frame.
pub const MAX_AIRCRAFT: usize = 64;
/// Live projectiles in one frame.
pub const MAX_PROJECTILES: usize = 1024;
/// Debris pieces in one frame.
pub const MAX_DEBRIS: usize = 256;
/// Ejected pilots in one frame.
pub const MAX_ESCAPEES: usize = 64;
/// New visual effects in one frame.
pub const MAX_EFFECTS_PER_TICK: usize = 64;
/// New smoke and contrail puffs in one frame.
pub const MAX_PUFFS_PER_TICK: usize = 4096;
/// Surface object hit-point changes in one frame.
pub const MAX_SURFACE_CHANGES_PER_TICK: usize = 4096;
/// Events in one frame.
pub const MAX_EVENTS_PER_TICK: usize = 1024;
/// Fields on one event.
pub const MAX_FIELDS_PER_EVENT: usize = 64;
/// Ids in one `Value::Ids`.
pub const MAX_IDS_PER_VALUE: usize = 1024;
/// Display tree samples in one frame.
pub const MAX_TREES_PER_TICK: usize = 256;
/// Nodes in one display tree sample.
pub const MAX_TREE_NODES: usize = 4096;
/// Tree depth levels: node depths run from 0 to 31.
pub const MAX_TREE_DEPTH: u8 = 32;
/// Distinct strings in the recording's string table. Later new strings are
/// stored inline instead of failing.
pub const MAX_STRINGS: usize = 65_536;
/// Bytes in any one string (UTF-8).
pub const MAX_STRING_BYTES: usize = 1024;
/// Bytes in one chunk body.
pub const MAX_CHUNK_BYTES: usize = 16 << 20;
/// Frames in one chunk (two seconds).
pub const MAX_CHUNK_FRAMES: u32 = 240;
/// Bytes in one recording file.
pub const MAX_FILE_BYTES: u64 = 1 << 30;
/// Bytes in the text header block.
pub const MAX_HEADER_BYTES: usize = 64 << 10;
/// Extra header entries, and footer result entries.
pub const MAX_KEY_VALUES: usize = 256;
/// Registered aircraft, and separately registered weapons.
pub const MAX_REGISTERED: usize = 4096;
/// Events a reader collects from one file.
pub const MAX_EVENTS_TOTAL: usize = 4_000_000;
/// Chunks in one file.
pub const MAX_CHUNKS: usize = 1 << 20;
