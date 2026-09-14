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
fn recorded_music_profile_wav_provenance_and_conflict_protection() {
    let f = Fixture::new(vec![
        ("AIR003.11K", 0, vec![0, 128, 255]),
        ("AIR003.XMI", 0, b"not selected".to_vec()),
        ("&CLICK.11K", 0, vec![128]),
    ]);
    assert!(
        f.run(&["--music", "--wav-previews", "--dry-run"])
            .status
            .success()
    );
    assert!(!f.out.exists());
    assert!(f.run(&["--music", "--wav-previews"]).status.success());
    assert!(!f.out.join("OTHER.DAT/AIR003.XMI").exists());
    assert!(!f.out.join("OTHER.DAT/&CLICK.11K").exists());
    let path = f.out.join("OTHER.DAT/AIR003.11K.wav");
    let bytes = fs::read(&path).unwrap();
    let pcm = tore_formats::pcm::Pcm::parse("preview.wav", &bytes).unwrap();
    assert_eq!(pcm.samples, [0, 128, 255]);
    assert_eq!(pcm.rate, 11025);
    assert!(
        fs::read_to_string(f.out.join("extraction-report.json"))
            .unwrap()
            .contains("preview_output")
    );
    assert!(f.run(&["--music", "--wav-previews"]).status.success());
    fs::write(&path, b"user recording").unwrap();
    assert!(!f.run(&["--music", "--wav-previews"]).status.success());
    assert_eq!(fs::read(&path).unwrap(), b"user recording");
    assert!(
        f.run(&["--music", "--wav-previews", "--overwrite"])
            .status
            .success()
    );
    assert!(!f.run(&["--wav-previews"]).status.success());
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

#[test]
fn theater_profile_preserves_environment_dependencies_and_reports_metadata_errors() {
    let f = Fixture::new(vec![
        (
            "UKR.MM",
            0,
            b"textFormat\nmap ukr.T2\nlayer day2.LAY 0\ntime 12 0\ntmap 4 8 2 3\n".to_vec(),
        ),
        ("SUN.SH", 0, b"synthetic shape".to_vec()),
        ("_MOON.PIC", 0, b"synthetic texture".to_vec()),
        ("_CLOUD1.PIC", 0, b"synthetic cloud".to_vec()),
        ("SKY8.PIC", 0, b"synthetic sky".to_vec()),
        ("UNRELATED", 0, b"other".to_vec()),
    ]);
    assert!(f.run(&["--theater", "ukr"]).status.success());
    for name in ["UKR.MM", "SUN.SH", "_MOON.PIC", "_CLOUD1.PIC", "SKY8.PIC"] {
        assert!(f.out.join("OTHER.DAT").join(name).exists());
    }
    assert!(!f.out.join("OTHER.DAT/UNRELATED").exists());
    let report = fs::read_to_string(f.out.join("extraction-report.json")).unwrap();
    assert!(report.contains("mission-environment"));
    assert!(report.contains("\"wind_raw\":null"));
    assert!(report.contains("[4,8,2,3]"));
    assert!(!f.run(&["--theater", "UNKNOWN"]).status.success());

    let invalid = Fixture::new(vec![("UKR.T2", 0, b"bad terrain".to_vec())]);
    assert!(!invalid.run(&["--theater", "UKR"]).status.success());
    assert!(!invalid.out.join("OTHER.DAT/UKR.T2").exists());
    assert!(
        fs::read_to_string(invalid.out.join("extraction-report.json"))
            .unwrap()
            .contains("\"complete\":false")
    );
    // General raw extraction deliberately does not require decoded terrain validity.
    assert!(invalid.run(&[]).status.success());
}

#[test]
fn all_theaters_include_aliases_and_skip_unrelated_disc_libraries() {
    let f = Fixture::new(vec![
        ("VIET0.PIC", 0, b"texture".to_vec()),
        ("KURIL.PIC", 0, b"map".to_vec()),
        (
            "~BAL0.MM",
            0,
            b"textFormat\nmap bal.T2\ntime 12 0\n".to_vec(),
        ),
        ("IFMFRA.PIC", 0, b"map".to_vec()),
        ("SKY0.PIC", 0, b"sky".to_vec()),
        ("OTHER.PIC", 0, b"unrelated".to_vec()),
    ]);
    fs::write(f.source.join("INSTALL.LIB"), b"not EALIB").unwrap();
    let result = f.run(&["--theater", "all"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stderr).contains("Skipping non-EALIB"));
    for name in [
        "VIET0.PIC",
        "KURIL.PIC",
        "~BAL0.MM",
        "IFMFRA.PIC",
        "SKY0.PIC",
    ] {
        assert!(f.out.join("OTHER.DAT").join(name).exists());
    }
    assert!(!f.out.join("OTHER.DAT/OTHER.PIC").exists());
    assert!(!f.run(&[]).status.success());
    assert!(
        f.run(&["--exclude-archive", "INSTALL.LIB"])
            .status
            .success()
    );
    let one = Fixture::new(vec![
        ("VIET0.PIC", 0, b"texture".to_vec()),
        ("UKR0.PIC", 0, b"other theater".to_vec()),
    ]);
    assert!(one.run(&["--theater", "tviet"]).status.success());
    assert!(one.out.join("OTHER.DAT/VIET0.PIC").exists());
    assert!(!one.out.join("OTHER.DAT/UKR0.PIC").exists());
}
#[test]
fn aircraft_dependency_plan_follows_resources_across_formats() {
    let mut entries = vec![
 ("F18.PT",0,b"[brent's_relocatable_format]\n:shape\nstring \"F18.SH\"\n:store\nstring \"TEST.JT\"\nend".to_vec()),
 ("F18.SH",0,b"_SKIN.PIC\0".to_vec()),("F18.HUD",0,b"~F18H\0".to_vec()),("~F18H.PIC",0,vec![]),("WIN11.FNT",0,vec![]),("HUD11.FNT",0,vec![]),("FMENUD.MNU",0,vec![]),("PANEL.PIC",0,vec![]),("_SKIN.PIC",0,vec![]),("PALETTE.PAL",0,vec![0;768]),
 ("TEST.JT",0,b"[brent's_relocatable_format]\n:sound\nstring \"TEST.5K\"\nend".to_vec()),("TEST.5K",0,vec![128]),("OTHER.PIC",0,vec![])];
    entries.extend(
        tore_formats::aircraft::COMBAT_RESOURCES
            .iter()
            .map(|&n| (n, 0, vec![0])),
    );
    entries.push(("F18.PTS", 0, vec![0]));
    let f = Fixture::new(entries);
    let result = f.run(&["--aircraft", "f18", "--list"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let out = String::from_utf8_lossy(&result.stdout);
    assert!(out.contains("_SKIN.PIC") && out.contains("TEST.5K") && out.contains("~F18H.PIC"));
    assert!(!out.contains(" / OTHER.PIC"));
    assert!(!f.out.exists());
    let filtered = f.run(&["--aircraft", "f18", "--include", "TEST.5K"]);
    assert!(
        filtered.status.success(),
        "{}",
        String::from_utf8_lossy(&filtered.stderr)
    );
    let report = fs::read_to_string(f.out.join("extraction-report.json")).unwrap();
    assert!(report.contains("\"filtered\":true"));
    assert!(report.contains("\"native_parity\":false"));
    assert!(report.contains("\"available\":true,\"included\":false"));
}
