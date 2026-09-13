use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
struct Fixture {
    root: PathBuf,
    source: PathBuf,
    out: PathBuf,
}
impl Fixture {
    fn new(entries: Vec<(&str, u8, Vec<u8>)>) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "tore-extract-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let source = root.join("disc with spaces");
        let out = root.join("output");
        fs::create_dir_all(&source).unwrap();
        let mut data = b"EALIB".to_vec();
        data.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        let mut offset = 7 + (entries.len() + 1) * 18;
        for (name, flag, bytes) in &entries {
            let mut raw = [0; 13];
            raw[..name.len()].copy_from_slice(name.as_bytes());
            data.extend_from_slice(&raw);
            data.push(*flag);
            data.extend_from_slice(&(offset as u32).to_le_bytes());
            offset += bytes.len();
        }
        data.extend_from_slice(&[0; 14]);
        data.extend_from_slice(&(offset as u32).to_le_bytes());
        for (_, _, bytes) in entries {
            data.extend_from_slice(&bytes);
        }
        // An arbitrary filename proves that discovery uses signature, not FA_* names.
        fs::write(source.join("OTHER.DAT"), data).unwrap();
        Self { root, source, out }
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_tore-extract"))
            .arg("--source")
            .arg(&self.source)
            .arg("--out")
            .arg(&self.out)
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn extracts_stored_and_compressed_resources_and_preserves_changes() {
    let mut compressed = 13u32.to_le_bytes().to_vec();
    compressed.extend_from_slice(&[0, 4, 0x82, 0x24, 0x25, 0x8f, 0x80, 0x7f]);
    let f = Fixture::new(vec![
        ("TEXT.TXT", 0, b"original".to_vec()),
        ("PACKED.BIN", 4, compressed),
    ]);
    let result = f.run(&[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let file = f.out.join("OTHER.DAT/TEXT.TXT");
    assert_eq!(fs::read(&file).unwrap(), b"original");
    assert_eq!(
        fs::read(f.out.join("OTHER.DAT/PACKED.BIN")).unwrap(),
        b"AIAIAIAIAIAIA"
    );
    assert!(f.run(&[]).status.success());
    assert!(
        fs::read_to_string(f.out.join("extraction-report.json"))
            .unwrap()
            .contains("unchanged")
    );
    fs::write(&file, b"user edit").unwrap();
    assert!(!f.run(&[]).status.success());
    assert_eq!(fs::read(&file).unwrap(), b"user edit");
    assert!(f.run(&["--overwrite"]).status.success());
    assert_eq!(fs::read(file).unwrap(), b"original");
}
#[test]
fn filtered_dry_run_writes_nothing_and_missing_matches_fail() {
    let f = Fixture::new(vec![
        ("ART.PIC", 0, b"synthetic".to_vec()),
        ("TEXT.TXT", 0, b"text".to_vec()),
    ]);
    assert!(f.run(&["--include", "*.pic", "--dry-run"]).status.success());
    assert!(!f.out.exists());
    assert!(!f.run(&["--include", "*.sh", "--list"]).status.success());
    assert!(!f.out.exists());
    assert!(f.run(&["--include", "*.pic"]).status.success());
    assert!(f.out.join("OTHER.DAT/ART.PIC").exists());
    assert!(!f.out.join("OTHER.DAT/TEXT.TXT").exists());
}
#[test]
fn rejects_traversal_reserved_names_and_oversized_resources() {
    for name in ["../BAD", "CON.TXT"] {
        let f = Fixture::new(vec![(name, 0, b"synthetic".to_vec())]);
        assert!(!f.run(&[]).status.success());
        assert!(!f.root.join("BAD").exists());
    }
    let mut bytes = (2 * 1024 * 1024u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&[0, 6]);
    let f = Fixture::new(vec![("BIG.BIN", 4, bytes)]);
    assert!(!f.run(&["--max-entry-mib", "1"]).status.success());
    assert!(!f.out.join("OTHER.DAT/BIG.BIN").exists());
}
#[cfg(unix)]
#[test]
fn refuses_output_symlink_escape() {
    let f = Fixture::new(vec![("TEXT.TXT", 0, b"synthetic".to_vec())]);
    fs::create_dir_all(&f.out).unwrap();
    let elsewhere = f.root.join("elsewhere");
    fs::create_dir(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, f.out.join("OTHER.DAT")).unwrap();
    assert!(!f.run(&[]).status.success());
    assert!(!elsewhere.join("TEXT.TXT").exists());
}
