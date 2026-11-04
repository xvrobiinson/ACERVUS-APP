//! "Update available": the newest Windows version, asked of the Acervus site
//! (the PWA's server, `/download/windows/info`, which reads the GitHub
//! release): its number, the installer's SHA-256 and what's new in plain
//! words (the release's `whats-new.json`, written in this repo). The
//! installer comes from the same site and runs only once its SHA-256 matches
//! the published one.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use super::auth::{self, Endpoints};

/// A newer version than this app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    /// The installer's SHA-256 (hex); none when the release didn't say.
    pub sha256: Option<String>,
    /// What's new, in English and in Spanish (empty when not written).
    pub notes_en: Vec<String>,
    pub notes_es: Vec<String>,
}

#[derive(Deserialize)]
struct Info {
    available: bool,
    version: Option<String>,
    sha256: Option<String>,
    notes: Option<Notes>,
}

#[derive(Deserialize)]
struct Notes {
    en: Vec<String>,
    es: Vec<String>,
}

/// "1.2.3" as numbers; None when it isn't a plain version.
fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let mut parts = v.trim().trim_start_matches('v').split('.').map(|p| p.parse::<u64>().ok());
    let found = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(found)
}

/// Whether `latest` is a newer version than `current`.
pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse(latest), parse(current)), (Some(l), Some(c)) if l > c)
}

/// The newest release when it is newer than this app (None: up to date, or
/// no installer to get right now).
pub fn check(ep: &Endpoints) -> Result<Option<Release>, String> {
    let mut res = auth::agent().get(&format!("{}/download/windows/info", ep.origin)).call().map_err(|e| e.to_string())?;
    if res.status() != 200 {
        return Err(format!("HTTP {}", res.status()));
    }
    let info: Info = res.body_mut().read_json().map_err(|e| e.to_string())?;
    Ok(newer(info, env!("CARGO_PKG_VERSION")))
}

fn newer(info: Info, current: &str) -> Option<Release> {
    let version = info.version.filter(|v| info.available && is_newer(v, current))?;
    let (notes_en, notes_es) = info.notes.map_or_else(Default::default, |n| (n.en, n.es));
    Some(Release { version, sha256: info.sha256.map(|s| s.to_lowercase()), notes_en, notes_es })
}

/// What went wrong with a download.
#[derive(Debug, PartialEq, Eq)]
pub enum DownloadError {
    /// The release published no SHA-256, so nothing can be checked.
    NoChecksum,
    /// The file differs from the one published; it was deleted.
    Mismatch,
    Failed(String),
}

/// Downloads the installer into `dir` and checks it against the release's
/// SHA-256; the checked file's path. `progress(done, total)` follows it.
pub fn download(ep: &Endpoints, release: &Release, dir: &Path, progress: impl Fn(u64, Option<u64>)) -> Result<PathBuf, DownloadError> {
    let want = release.sha256.clone().ok_or(DownloadError::NoChecksum)?;
    let failed = |e: &dyn std::fmt::Display| DownloadError::Failed(e.to_string());
    let mut res =
        auth::agent_waiting(Duration::from_secs(15 * 60)).get(&format!("{}/download/windows", ep.origin)).call().map_err(|e| failed(&e))?;
    if res.status() != 200 {
        return Err(failed(&format!("HTTP {}", res.status())));
    }
    let total = res.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
    std::fs::create_dir_all(dir).map_err(|e| failed(&e))?;
    let path = dir.join(format!("AcervusFinance-Setup-{}.exe", release.version));
    let mut file = std::fs::File::create(&path).map_err(|e| failed(&e))?;
    let mut body = res.body_mut().with_config().limit(200 * 1024 * 1024).reader();
    let (mut hash, mut done, mut buf) = (Sha256::new(), 0u64, vec![0u8; 64 * 1024]);
    let copied = loop {
        match body.read(&mut buf) {
            Ok(0) => break Ok(()),
            Ok(n) => {
                hash.update(&buf[..n]);
                if let Err(e) = file.write_all(&buf[..n]) {
                    break Err(failed(&e));
                }
                done += n as u64;
                progress(done, total);
            }
            Err(e) => break Err(failed(&e)),
        }
    };
    drop(file);
    let got = format!("{:x}", hash.finalize());
    let result = copied.and_then(|()| if got == want { Ok(path.clone()) } else { Err(DownloadError::Mismatch) });
    if result.is_err() {
        let _ = std::fs::remove_file(&path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_higher_plain_version_is_newer() {
        assert!(is_newer("0.13.0", "0.12.2"));
        assert!(is_newer("v0.12.10", "0.12.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.12.2", "0.12.2"));
        assert!(!is_newer("0.12.1", "0.12.2"));
        assert!(!is_newer("0.13.0-beta", "0.12.2"));
        assert!(!is_newer("garbage", "0.12.2"));
    }

    fn info(available: bool, version: &str) -> Info {
        Info {
            available,
            version: Some(version.into()),
            sha256: Some("ABC".into()),
            notes: Some(Notes { en: vec!["New".into()], es: vec!["Nuevo".into()] }),
        }
    }

    #[test]
    fn a_newer_release_with_an_installer_is_offered() {
        let r = newer(info(true, "0.13.0"), "0.12.2").unwrap();
        assert_eq!(r.version, "0.13.0");
        assert_eq!(r.sha256.as_deref(), Some("abc"));
        assert_eq!((r.notes_en, r.notes_es), (vec!["New".to_string()], vec!["Nuevo".to_string()]));
        assert_eq!(newer(info(true, "0.12.2"), "0.12.2"), None, "up to date");
        assert_eq!(newer(info(false, "0.13.0"), "0.12.2"), None, "no installer to get yet");
    }

    /// Each version comes with its plain-words notes (published with the
    /// release; the "Update available" window shows them).
    #[test]
    fn whats_new_is_written_for_this_version() {
        let notes: serde_json::Value = serde_json::from_str(include_str!("../../whats-new.json")).unwrap();
        let minor: Vec<&str> = env!("CARGO_PKG_VERSION").split('.').take(2).collect();
        assert_eq!(notes["version"], minor.join("."), "update whats-new.json for this version");
        for lang in ["en", "es"] {
            let lines = notes[lang].as_array().unwrap();
            assert!(!lines.is_empty() && lines.len() <= 20, "{lang}");
            assert!(lines.iter().all(|l| l.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 300)), "{lang}");
        }
        assert_eq!(notes["en"].as_array().unwrap().len(), notes["es"].as_array().unwrap().len());
    }
}
