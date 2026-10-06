//! Where an import came from: the Fighters Anthology build it read and the
//! T.O.R.E that made it.
//!
//! The import writes one small text entry, [`RESOURCE`], into the pack. Nothing
//! in single player reads it; the multiplayer lobby shows each player's build
//! and the host's content check names a stale import (docs/ARCHITECTURE.md,
//! "Compatibility"). A pack made before the entry existed has none: the build
//! is then read from the `FA.EXE:` line of `import-report.txt` beside the pack,
//! and the importer is unknown. No re-import is asked for.
//!
//! The entry, one `key value` line each:
//!
//! ```text
//! tore-source 1
//! build 1.02F
//! tore 0.1.4 48d62dac0f5e3c9b7a1d2e4f60718293a4b5c6d7
//! ```

use crate::Resources;
use std::{fs, io::Read, path::Path};

/// The pack entry's name.
pub const RESOURCE: &str = "TORE_SOURCE_V1";

/// The most bytes of the entry that are read. The real entry is about 80.
const ENTRY_LIMIT: usize = 512;

/// How much of `import-report.txt` is read for the fallback. The `FA.EXE:`
/// line is its fourth.
const REPORT_LIMIT: u64 = 8 * 1024;

/// The longest version or commit text accepted.
const FIELD_LIMIT: usize = 128;

/// A reviewed Fighters Anthology build, as `tore_formats::executable`
/// identifies it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Build {
    /// The 1.0 disc release.
    Disc10,
    /// Patch 1.02F.
    V102F,
}

impl Build {
    /// The name `tore_formats::executable::identify` gives the build, which
    /// is what the entry and the report hold.
    pub fn name(self) -> &'static str {
        match self {
            Self::Disc10 => "1.0 (disc)",
            Self::V102F => "1.02F",
        }
    }

    /// The version as a player says it: `1.0` or `1.02F`.
    pub fn version(self) -> &'static str {
        match self {
            Self::Disc10 => "1.0",
            Self::V102F => "1.02F",
        }
    }

    /// The build named by `name` (as in [`Build::name`]).
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Disc10, Self::V102F]
            .into_iter()
            .find(|build| build.name() == name)
    }
}

/// The T.O.R.E that made an import: its version (`0.1.3`, or
/// `0.1.3-4-gabc1234` between tags) and the commit it was built from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Importer {
    pub version: String,
    pub commit: String,
}

impl Importer {
    /// This build of the import code.
    pub fn this_build() -> Self {
        Self {
            version: option_env!("TORE_BUILD_VERSION")
                .unwrap_or(env!("CARGO_PKG_VERSION"))
                .to_owned(),
            commit: env!("TORE_BUILD_COMMIT").to_owned(),
        }
    }

    /// The commit shortened to eight characters, for a line a person reads.
    pub fn short_commit(&self) -> &str {
        self.commit.get(..8).unwrap_or(&self.commit)
    }
}

/// What an import records about where it came from. Either half may be
/// unknown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// The Fighters Anthology build, if known.
    pub build: Option<Build>,
    /// The T.O.R.E that made the import, if known.
    pub importer: Option<Importer>,
}

impl Source {
    /// Nothing known.
    pub const UNKNOWN: Self = Self {
        build: None,
        importer: None,
    };

    /// The source of an import being made now from `build`.
    pub fn of_import(build: Build) -> Self {
        Self {
            build: Some(build),
            importer: Some(Importer::this_build()),
        }
    }

    /// The bytes of the pack entry. A source with no build is written as
    /// `build unknown`.
    pub fn encode(&self) -> Vec<u8> {
        let mut text = String::from("tore-source 1\n");
        text.push_str(&format!(
            "build {}\n",
            self.build.map_or("unknown", Build::name)
        ));
        if let Some(importer) = &self.importer {
            text.push_str(&format!("tore {} {}\n", importer.version, importer.commit));
        }
        text.into_bytes()
    }

    /// Reads a pack entry. `None` when it is not a source entry of this
    /// version, which callers treat as no entry at all. A build the importer
    /// has not reviewed is an unknown build; an importer line that does not
    /// parse is an unknown importer.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > ENTRY_LIMIT {
            return None;
        }
        let text = std::str::from_utf8(bytes).ok()?;
        let mut lines = text.lines();
        if lines.next()? != "tore-source 1" {
            return None;
        }
        let mut source = Self::UNKNOWN;
        for line in lines {
            if let Some(name) = line.strip_prefix("build ") {
                source.build = Build::from_name(name);
            } else if let Some(rest) = line.strip_prefix("tore ") {
                let mut words = rest.split(' ');
                if let (Some(version), Some(commit), None) =
                    (words.next(), words.next(), words.next())
                    && field_ok(version)
                    && field_ok(commit)
                {
                    source.importer = Some(Importer {
                        version: version.to_owned(),
                        commit: commit.to_owned(),
                    });
                }
            }
        }
        Some(source)
    }

    /// The source of the import in `data_dir` whose resources are `resources`:
    /// the pack's entry when it has one, else the build from the `FA.EXE:`
    /// line of the report beside the pack with the importer unknown, else
    /// nothing known.
    pub fn read(data_dir: &Path, resources: &Resources) -> Self {
        if let Some(source) = resources.get(RESOURCE).and_then(|b| Self::parse(b)) {
            return source;
        }
        Self {
            build: build_in_report(data_dir),
            importer: None,
        }
    }

    /// One line for an operator or a log: `Fighters Anthology 1.02F, imported
    /// by T.O.R.E 0.1.4 (48d62dac)`, with "an unknown T.O.R.E" or "an unknown
    /// Fighters Anthology build" where a half is unknown.
    pub fn describe(&self) -> String {
        let build = match self.build {
            Some(build) => format!("Fighters Anthology {}", build.version()),
            None => "an unknown Fighters Anthology build".to_owned(),
        };
        match &self.importer {
            Some(importer) => format!(
                "{build}, imported by T.O.R.E {} ({})",
                importer.version,
                importer.short_commit()
            ),
            None => format!("{build}, imported by an unknown T.O.R.E"),
        }
    }
}

/// A version or commit word: printable ASCII with no space, not empty.
fn field_ok(word: &str) -> bool {
    !word.is_empty() && word.len() <= FIELD_LIMIT && word.bytes().all(|b| b.is_ascii_graphic())
}

/// The build named by the `FA.EXE:` line at the top of `import-report.txt`.
fn build_in_report(data_dir: &Path) -> Option<Build> {
    let mut head = Vec::new();
    fs::File::open(data_dir.join("import-report.txt"))
        .ok()?
        .take(REPORT_LIMIT)
        .read_to_end(&mut head)
        .ok()?;
    let text = String::from_utf8_lossy(&head);
    let line = text
        .lines()
        .find_map(|line| line.strip_prefix("FA.EXE: "))?;
    // `FA.EXE: 1.02F SHA-256 <hash>; ...`
    let (name, _) = line.split_once(" SHA-256 ")?;
    Build::from_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Folder(PathBuf);
    impl Folder {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "tore-source-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn importer() -> Importer {
        Importer {
            version: "0.1.4".into(),
            commit: "48d62dac0f5e3c9b7a1d2e4f60718293a4b5c6d7".into(),
        }
    }

    fn report(line: &str) -> String {
        format!("T.O.R.E-Fighters menu import v1\nOnly selected.\nSource: disc x\n{line}\n")
    }

    #[test]
    fn the_entry_round_trips_for_each_build() {
        for build in [Build::Disc10, Build::V102F] {
            let source = Source {
                build: Some(build),
                importer: Some(importer()),
            };
            assert_eq!(Source::parse(&source.encode()), Some(source));
        }
        let text = Source::of_import(Build::V102F).encode();
        assert!(text.starts_with(b"tore-source 1\nbuild 1.02F\ntore "));
        assert!(text.len() < ENTRY_LIMIT);
        assert!(RESOURCE.len() <= 32, "the pack bounds a name at 32 bytes");
    }

    #[test]
    fn an_import_records_this_build_of_the_importer() {
        let source = Source::of_import(Build::Disc10);
        assert_eq!(source.build, Some(Build::Disc10));
        let importer = source.importer.unwrap();
        assert!(!importer.version.is_empty());
        assert!(!importer.commit.is_empty());
        assert_eq!(importer, Importer::this_build());
    }

    #[test]
    fn a_half_known_entry_reads_as_unknown_for_that_half() {
        let unknown_build = Source::parse(b"tore-source 1\nbuild 2.0\ntore 0.1.4 abc\n").unwrap();
        assert_eq!(unknown_build.build, None);
        assert_eq!(unknown_build.importer.unwrap().version, "0.1.4");
        let no_importer = Source::parse(b"tore-source 1\nbuild 1.0 (disc)\n").unwrap();
        assert_eq!(no_importer.build, Some(Build::Disc10));
        assert_eq!(no_importer.importer, None);
        // An importer line with a missing word, an extra word or a space-less
        // control character is not an importer.
        for line in [
            "tore 0.1.4\n",
            "tore a b c\n",
            "tore  b\n",
            "tore a b\u{1}\n",
        ] {
            let text = format!("tore-source 1\nbuild 1.02F\n{line}");
            assert_eq!(
                Source::parse(text.as_bytes()).unwrap().importer,
                None,
                "{line:?}"
            );
        }
        // Lines it does not know are ignored.
        let extra = Source::parse(b"tore-source 1\nfuture thing\nbuild 1.02F\n").unwrap();
        assert_eq!(extra.build, Some(Build::V102F));
    }

    #[test]
    fn something_that_is_not_the_entry_is_refused() {
        for bytes in [
            &b""[..],
            b"tore-source 2\nbuild 1.02F\n",
            b"PCM1",
            &[0xff, 0xfe, 0, 1],
            &vec![b'a'; ENTRY_LIMIT + 1],
        ] {
            assert_eq!(Source::parse(bytes), None);
        }
    }

    #[test]
    fn the_pack_entry_wins_over_the_report() {
        let folder = Folder::new();
        fs::write(
            folder.0.join("import-report.txt"),
            report("FA.EXE: 1.0 (disc) SHA-256 abc; inert"),
        )
        .unwrap();
        let mut resources = Resources::new();
        let source = Source {
            build: Some(Build::V102F),
            importer: Some(importer()),
        };
        resources.insert(RESOURCE.into(), source.encode());
        assert_eq!(Source::read(&folder.0, &resources), source);
    }

    #[test]
    fn an_old_pack_reads_its_build_from_the_report_and_its_importer_is_unknown() {
        for (line, build) in [
            (
                "FA.EXE: 1.02F SHA-256 e315; inert creator lists",
                Build::V102F,
            ),
            (
                "FA.EXE: 1.0 (disc) SHA-256 c7d2; inert creator lists",
                Build::Disc10,
            ),
        ] {
            let folder = Folder::new();
            fs::write(folder.0.join("import-report.txt"), report(line)).unwrap();
            let source = Source::read(&folder.0, &Resources::new());
            assert_eq!(source.build, Some(build));
            assert_eq!(source.importer, None);
        }
        // An entry that does not parse falls back to the report too.
        let folder = Folder::new();
        fs::write(
            folder.0.join("import-report.txt"),
            report("FA.EXE: 1.02F SHA-256 e315; inert"),
        )
        .unwrap();
        let mut resources = Resources::new();
        resources.insert(RESOURCE.into(), b"garbage".to_vec());
        assert_eq!(
            Source::read(&folder.0, &resources).build,
            Some(Build::V102F)
        );
    }

    #[test]
    fn a_pack_with_neither_reads_as_unknown() {
        let folder = Folder::new();
        assert_eq!(Source::read(&folder.0, &Resources::new()), Source::UNKNOWN);
        // A report with no FA.EXE line, with an unreviewed build, or past the
        // bytes read, tells nothing.
        for text in [
            "T.O.R.E-Fighters menu import v1\n".to_owned(),
            report("FA.EXE: 2.0 SHA-256 abc; inert"),
            report("FA.EXE: 1.02F"),
            format!(
                "{}{}",
                "x\n".repeat(REPORT_LIMIT as usize),
                report("FA.EXE: 1.02F SHA-256 abc")
            ),
        ] {
            fs::write(folder.0.join("import-report.txt"), text).unwrap();
            assert_eq!(Source::read(&folder.0, &Resources::new()), Source::UNKNOWN);
        }
    }

    #[test]
    fn a_line_for_an_operator_says_what_is_known() {
        let full = Source {
            build: Some(Build::V102F),
            importer: Some(importer()),
        };
        assert_eq!(
            full.describe(),
            "Fighters Anthology 1.02F, imported by T.O.R.E 0.1.4 (48d62dac)"
        );
        let old = Source {
            build: Some(Build::Disc10),
            importer: None,
        };
        assert_eq!(
            old.describe(),
            "Fighters Anthology 1.0, imported by an unknown T.O.R.E"
        );
        assert_eq!(
            Source::UNKNOWN.describe(),
            "an unknown Fighters Anthology build, imported by an unknown T.O.R.E"
        );
        let short = Importer {
            version: "0.1.4".into(),
            commit: "unknown".into(),
        };
        assert_eq!(short.short_commit(), "unknown");
        let tiny = Importer {
            version: "0.1.4".into(),
            commit: "abc".into(),
        };
        assert_eq!(tiny.short_commit(), "abc");
    }

    /// Run after a fresh import into a data folder of your own:
    /// `TORE_DATA_DIR=... cargo test -p tore-import -- --ignored source`. The
    /// newest pack carries the entry, it names the build the report names and
    /// this build as the importer.
    #[test]
    #[ignore = "needs a fresh real import (TORE_DATA_DIR); run in the full suite"]
    fn real_data_a_fresh_import_records_its_source() {
        let directory = PathBuf::from(
            std::env::var_os("TORE_DATA_DIR").expect("TORE_DATA_DIR names a fresh import"),
        );
        let mut packs: Vec<PathBuf> = fs::read_dir(&directory)
            .unwrap()
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| crate::pack::pack_generation(path).is_some())
            .collect();
        packs.sort();
        let resources = crate::pack::read_pack(packs.last().expect("a pack")).unwrap();
        let entry = Source::parse(&resources[RESOURCE]).expect("the pack has the source entry");
        let from_report = build_in_report(&directory);
        assert!(entry.build.is_some());
        assert_eq!(entry.build, from_report, "the report names the same build");
        assert_eq!(entry.importer, Some(Importer::this_build()));
        assert_eq!(Source::read(&directory, &resources), entry);
        println!("{}", entry.describe());
    }
}
