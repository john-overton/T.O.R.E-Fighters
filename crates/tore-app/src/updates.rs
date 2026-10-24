//! Opinionated project release links. No executable downloads or installation.
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use semver::Version;
use serde_json::{Value, json};

pub const FEEDBACK_URL: &str = "https://github.com/john-overton/T.O.R.E-Fighters/issues";
const RELEASE_PREFIX: &str = "https://github.com/john-overton/T.O.R.E-Fighters/releases/tag/";
const API_URL: &str =
    "https://api.github.com/repos/john-overton/T.O.R.E-Fighters/releases?per_page=100";
const DAY: u64 = 24 * 60 * 60;
const CACHE: &str = "update-cache-v1.json";
const SETTING: &str = "updates-v1.conf";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    pub url: String,
}

/// Git describe's distance/hash suffix is build identity, not a prerelease.
/// Preserve a real prerelease before that suffix, such as 1.2.0-rc.1-3-gabc.
fn running_version(text: &str) -> Option<Version> {
    let text = text.trim_start_matches('v');
    let clean = text.strip_suffix("-dirty").unwrap_or(text);
    let base = clean.rsplit_once("-g").and_then(|(left, hash)| {
        let (base, count) = left.rsplit_once('-')?;
        (!hash.is_empty()
            && hash.bytes().all(|b| b.is_ascii_hexdigit())
            && !count.is_empty()
            && count.bytes().all(|b| b.is_ascii_digit()))
        .then_some(base)
    });
    Version::parse(base.unwrap_or(text)).ok()
}

fn release(value: &Value) -> Option<Release> {
    let tag = value.get("tag_name")?.as_str()?;
    // Bound UI text as well as the JSON input. SemVer excludes URL metacharacters.
    if tag.len() > 80 {
        return None;
    }
    let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()?;
    let url = value.get("html_url")?.as_str()?;
    (url == format!("{RELEASE_PREFIX}{tag}")).then(|| Release {
        version,
        url: url.to_owned(),
    })
}

fn parse_releases(text: &str) -> Result<Option<Release>, String> {
    let data: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let rows = data.as_array().ok_or("expected release array")?;
    Ok(rows
        .iter()
        .filter(|row| row.get("draft").and_then(Value::as_bool) == Some(false))
        .filter_map(release)
        .max_by(|a, b| a.version.cmp_precedence(&b.version)))
}

fn newer(release: Option<Release>, running: &str) -> Option<Release> {
    let current = running_version(running)?;
    release.filter(|r| r.version.cmp_precedence(&current).is_gt())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn cached(text: &str, at: u64) -> Option<Option<Release>> {
    let data: Value = serde_json::from_str(text).ok()?;
    if data.get("schema")?.as_u64()? != 1 {
        return None;
    }
    let checked = data.get("checked")?.as_u64()?;
    if at.checked_sub(checked)? >= DAY {
        return None;
    }
    let value = data.get("release")?;
    if value.is_null() {
        Some(None)
    } else {
        release(value).map(Some)
    }
}

fn cache_text(at: u64, result: Option<&Release>) -> String {
    let value = result.map(|r| {
        json!({
            "tag_name": r.url.strip_prefix(RELEASE_PREFIX).unwrap_or_default(),
            "html_url": r.url,
        })
    });
    json!({"schema": 1, "checked": at, "release": value}).to_string()
}

fn fetch_text() -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .max_redirects(0)
        .https_only(true)
        .build()
        .into();
    let text = agent
        .get(API_URL)
        .header("User-Agent", "TORE-Fighters-release-check")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .with_config()
        .limit(2 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| e.to_string())?;
    Ok(text)
}

fn fetch() -> Result<Option<Release>, String> {
    parse_releases(&fetch_text()?)
}

/// Cache failures too: one timed-out or rate-limited request cannot produce a
/// retry storm across restarts. No credentials, profile data or retry loop.
fn check(
    profile: &Path,
    at: u64,
    fetch: impl FnOnce() -> Result<Option<Release>, String>,
) -> Option<Release> {
    let path = profile.join(CACHE);
    if let Ok(text) = crate::preferences::read(&path)
        && let Some(result) = cached(&text, at)
    {
        return result;
    }
    // Record the attempt before networking, including an interrupted process.
    if let Err(error) = crate::preferences::write(&path, &cache_text(at, None)) {
        log::debug!("Update check skipped: cannot cache attempt: {error}");
        return None;
    }
    let result = match fetch() {
        Ok(result) => result,
        Err(error) => {
            log::debug!("Update check unavailable: {error}");
            None
        }
    };
    let _ = crate::preferences::write(&path, &cache_text(at, result.as_ref()));
    result
}

pub struct Updates {
    profile: Option<PathBuf>,
    pub enabled: bool,
    attempted: bool,
    pending: Option<Receiver<Option<Release>>>,
    pub available: Option<Release>,
}
impl Updates {
    pub fn new(profile: Option<PathBuf>) -> Self {
        let enabled =
            profile
                .as_ref()
                .is_some_and(|p| match crate::preferences::read(&p.join(SETTING)) {
                    Ok(text) => text.trim() == "tore-updates 1\ncheck true",
                    Err(e) => e.kind() == std::io::ErrorKind::NotFound,
                });
        Self {
            profile,
            enabled,
            attempted: false,
            pending: None,
            available: None,
        }
    }
    pub fn toggle(&mut self) -> std::io::Result<()> {
        let profile = self.profile.as_ref().ok_or_else(|| {
            std::io::Error::other("Update preferences are disabled in diagnostic runs")
        })?;
        let enabled = !self.enabled;
        crate::preferences::write(
            &profile.join(SETTING),
            &format!("tore-updates 1\ncheck {enabled}\n"),
        )?;
        self.enabled = enabled;
        Ok(())
    }
    /// Called only on Choose Activity. Disk/network work stays on the worker.
    pub fn poll(&mut self) -> bool {
        if !self.enabled {
            return false;
        }
        if !self.attempted {
            self.attempted = true;
            if let Some(profile) = self.profile.clone() {
                let (send, recv) = mpsc::channel();
                if std::thread::Builder::new()
                    .name("release-check".into())
                    .spawn(move || {
                        let _ = send.send(check(&profile, now(), fetch));
                    })
                    .is_ok()
                {
                    self.pending = Some(recv);
                }
            }
        }
        if let Some(recv) = &self.pending {
            match recv.try_recv() {
                Ok(result) => {
                    self.pending = None;
                    let available = newer(result, crate::version::version());
                    let changed = self.available != available;
                    self.available = available;
                    return changed;
                }
                Err(TryRecvError::Disconnected) => self.pending = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        false
    }
}

/// Only our fixed feedback URL or validated repository release URLs reach this
/// function. No shell interpolation. Reap the opener away from the UI thread.
pub fn open_page(url: &str) -> std::io::Result<()> {
    if url != FEEDBACK_URL
        && url
            .strip_prefix(RELEASE_PREFIX)
            .is_none_or(|tag| Version::parse(tag.strip_prefix('v').unwrap_or(tag)).is_err())
    {
        return Err(std::io::Error::other("invalid project URL"));
    }
    let url = url.to_owned();
    std::thread::Builder::new()
        .name("browser-open".into())
        .spawn(move || {
            #[cfg(target_os = "windows")]
            let result = std::process::Command::new("rundll32.exe")
                .arg("url.dll,FileProtocolHandler")
                .arg(&url)
                .status();
            #[cfg(target_os = "macos")]
            let result = std::process::Command::new("open").arg(&url).status();
            #[cfg(not(any(target_os = "windows", target_os = "macos")))]
            let result = std::process::Command::new("xdg-open").arg(&url).status();
            if !matches!(result, Ok(status) if status.success()) {
                log::warn!("Could not open browser for {url}: {result:?}");
            }
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &str = include_str!("updates/synthetic-releases.json");
    fn latest() -> Release {
        parse_releases(FIXTURE).unwrap().unwrap()
    }
    #[test]
    #[ignore = "one live GitHub request, run manually with an evidence path"]
    fn live_api_manual_check() {
        let path =
            std::env::var_os("TORE_UPDATE_CHECK_EVIDENCE").expect("set an ignored evidence path");
        let text = fetch_text().expect("GitHub request");
        std::fs::write(path, &text).unwrap();
        let latest = parse_releases(&text).unwrap().expect("published release");
        println!("Live latest: v{} ({})", latest.version, latest.url);
        println!(
            "Running {}: update={}",
            crate::version::version(),
            newer(Some(latest.clone()), crate::version::version()).is_some()
        );
        assert!(
            newer(Some(latest), "0.0.0").is_some(),
            "test-only lower version detects update"
        );
    }
    #[test]
    fn semantic_versions_include_prereleases_and_normalize_git_describe() {
        for (value, expected) in [
            ("v0.2.0-3-gabc", "0.2.0"),
            ("0.2.1-rc.2-3-gabc-dirty", "0.2.1-rc.2"),
            ("0.2.1-rc.2", "0.2.1-rc.2"),
        ] {
            assert_eq!(
                running_version(value).unwrap(),
                Version::parse(expected).unwrap()
            );
        }
        for (current, expected) in [
            ("0.2.0", true),
            ("0.2.1-rc.1", true),
            ("0.2.1-rc.2", false),
            ("0.2.1", false),
            ("0.3.0", false),
            ("0.2.1-rc.2+build", false),
            ("garbage", false),
        ] {
            assert_eq!(
                newer(Some(latest()), current).is_some(),
                expected,
                "{current}"
            );
        }
        assert!(newer(Some(latest()), "0.2.0-3-gabc").is_some());
    }
    #[test]
    fn recorded_public_releases_include_prereleases_and_equal_is_not_an_update() {
        // Recorded 2026-10-23, retaining only the API fields this feature reads.
        let text = include_str!("updates/recorded-releases.json");
        let latest = parse_releases(text).unwrap().unwrap();
        assert_eq!(latest.version.to_string(), "0.2.0");
        assert!(newer(Some(latest.clone()), "0.2.0").is_none());
        assert!(newer(Some(latest.clone()), "0.2.0-3-gabc").is_none());
        assert!(newer(Some(latest), "0.1.0").is_some());
    }
    #[test]
    fn json_filters_drafts_invalid_versions_and_external_urls() {
        assert_eq!(latest().version.to_string(), "0.2.1-rc.2");
        assert_eq!(parse_releases("[]").unwrap(), None);
        assert!(parse_releases("{}").is_err());
        assert!(parse_releases("[").is_err());
        assert!(open_page("https://example.com").is_err());
    }
    #[test]
    fn cache_expires_at_one_day_and_rejects_future_or_corrupt_records() {
        let text = cache_text(100, Some(&latest()));
        assert_eq!(cached(&text, 100), Some(Some(latest())));
        assert!(cached(&text, 100 + DAY - 1).is_some());
        assert!(cached(&text, 100 + DAY).is_none());
        assert!(cached(&text, 99).is_none());
        assert!(cached("{}", 100).is_none());
        assert_eq!(cached(&cache_text(100, None), 100), Some(None));
    }
    #[test]
    fn one_worker_per_start_and_toggling_back_on_reuses_known_result() {
        let dir = std::env::temp_dir().join(format!(
            "tore-update-worker-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        crate::preferences::write(&dir.join(CACHE), &cache_text(now(), Some(&latest()))).unwrap();
        let mut state = Updates::new(Some(dir.clone()));
        state.poll();
        // Generous: a loaded CI machine can take seconds to start the worker,
        // and the loop ends as soon as it is done.
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        while state.pending.is_some() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
            state.poll();
        }
        assert!(state.attempted);
        assert!(state.pending.is_none());
        let known = state.available.clone();
        state.toggle().unwrap();
        assert!(!state.poll());
        state.toggle().unwrap();
        assert!(!state.poll());
        assert_eq!(state.available, known);
        assert!(state.pending.is_none(), "no second worker after toggling");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn failures_and_results_are_cached_and_off_switch_survives_restart() {
        let dir = std::env::temp_dir().join(format!(
            "tore-update-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mut state = Updates::new(Some(dir.clone()));
        assert!(state.enabled);
        state.toggle().unwrap();
        assert!(!state.poll());
        assert!(!state.attempted);
        assert!(state.pending.is_none());
        assert!(!Updates::new(Some(dir.clone())).enabled);
        assert_eq!(check(&dir, 100, || Err("rate limited".into())), None);
        assert_eq!(check(&dir, 101, || panic!("must use cached failure")), None);
        assert_eq!(
            check(&dir, 100 + DAY, || Ok(Some(latest()))),
            Some(latest())
        );
        assert_eq!(
            check(&dir, 101 + DAY, || panic!("must use cache")),
            Some(latest())
        );
        state.toggle().unwrap();
        assert!(Updates::new(Some(dir.clone())).enabled);
        std::fs::remove_dir_all(dir).unwrap();
        assert!(!Updates::new(None).poll());
    }
}
