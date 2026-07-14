//! Data fetching & caching (CLAUDE.md §4–§5).
//!
//! Startup: fetch the tiny `version.json`, compare per-file hashes with what
//! we last stored, fetch only changed files. `version.json` may not exist
//! upstream yet — on 404, fall back to conditional GETs (ETag /
//! If-None-Match) on the data files and warn once. Offline with a complete
//! cache → use the cache and warn.
//!
//! With a complete cache, the network is only touched once per
//! `refresh_interval_secs` window (`Config::refresh_interval_secs`, default
//! 15 min) — repeated launches within that window reuse the cache with zero
//! requests. Every check (success, 404, unexpected status, or a connection
//! error) resets the window, so a rate-limited host doesn't get hammered by
//! quick repeated launches either.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::{Config, EMBEDDINGS_FILE, QA_DATA_FILE, TOKENIZER_TAG, VERSION_FILE};

/// `version.json` manifest (§5). All fields optional so a future upstream
/// schema bump degrades to "refetch" rather than a parse failure.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub generated_at: Option<String>,
    #[serde(default)]
    pub schema_version: Option<u32>,
    #[serde(default)]
    pub tokenizer: Option<String>,
    /// Per-key model name (`"small"`/`"large"`, matching `embeddings.json`'s
    /// top-level keys — §7.1).
    #[serde(default)]
    pub embedding_model: Option<HashMap<String, String>>,
    #[serde(default)]
    pub files: HashMap<String, ManifestFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFile {
    pub hash: String,
    #[serde(default)]
    pub size: Option<u64>,
}

/// Local bookkeeping (`data/meta.json`): the manifest hash and/or ETag each
/// cached file was fetched with. The manifest hash is treated as an opaque
/// string — files are refetched when it changes, so we never depend on the
/// upstream hash algorithm.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Meta {
    #[serde(default)]
    files: HashMap<String, FileMeta>,
    /// Unix timestamp of the last time we checked (not necessarily changed)
    /// `version.json`, regardless of the outcome. `None` (fresh/old cache)
    /// always allows an immediate check.
    #[serde(default)]
    last_checked_unix: Option<u64>,
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Pure decision so it's unit-testable without a network/clock dependency:
/// skip the version check entirely if the cache is complete and was checked
/// more recently than `refresh_interval_secs` ago. `refresh_interval_secs ==
/// 0` always checks (never skips) — the pre-refresh-window behavior.
fn should_skip_check(last_checked_unix: Option<u64>, now: u64, refresh_interval_secs: u64) -> bool {
    match last_checked_unix {
        Some(last) if refresh_interval_secs > 0 => now.saturating_sub(last) < refresh_interval_secs,
        _ => false,
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct FileMeta {
    #[serde(default)]
    manifest_hash: Option<String>,
    #[serde(default)]
    etag: Option<String>,
}

/// Outcome of the startup fetch.
pub struct DataFiles {
    pub qa_path: PathBuf,
    pub embeddings_path: PathBuf,
    /// Tokenizer tag the BM25 index must carry (manifest `tokenizer`, or the
    /// locked default until version.json ships).
    pub tokenizer_tag: String,
    pub warnings: Vec<String>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(out, "{b:02x}");
    }
    out
}

fn meta_path(cfg: &Config) -> PathBuf {
    cfg.data_dir().join("meta.json")
}

fn load_meta(cfg: &Config) -> Meta {
    std::fs::read(meta_path(cfg))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_meta(cfg: &Config, meta: &Meta) -> Result<()> {
    std::fs::create_dir_all(cfg.data_dir())?;
    let bytes = serde_json::to_vec_pretty(meta)?;
    write_atomic(&meta_path(cfg), &bytes)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("renaming into {}", path.display()))?;
    Ok(())
}

fn cache_complete(cfg: &Config) -> bool {
    cfg.qa_data_path().exists() && cfg.embeddings_path().exists()
}

/// Ensure the data files exist locally and are current, per the §5 startup
/// flow. `progress` receives human-readable phase updates for the spinner.
pub async fn ensure_data(
    cfg: &Config,
    progress: &(dyn Fn(&str) + Send + Sync),
) -> Result<DataFiles> {
    let mut warnings = Vec::new();
    let mut tokenizer_tag = TOKENIZER_TAG.to_string();

    if cfg.offline {
        if !cache_complete(cfg) {
            bail!(
                "offline mode requested but the data cache is incomplete ({})",
                cfg.data_dir().display()
            );
        }
        warnings.push("offline mode: using cached data without checking for updates".to_string());
        return Ok(DataFiles {
            qa_path: cfg.qa_data_path(),
            embeddings_path: cfg.embeddings_path(),
            tokenizer_tag,
            warnings,
        });
    }

    let mut meta = load_meta(cfg);

    if cache_complete(cfg)
        && should_skip_check(
            meta.last_checked_unix,
            unix_now(),
            cfg.refresh_interval_secs,
        )
    {
        return Ok(DataFiles {
            qa_path: cfg.qa_data_path(),
            embeddings_path: cfg.embeddings_path(),
            tokenizer_tag,
            warnings,
        });
    }

    let client = reqwest::Client::builder()
        .user_agent(concat!("physq/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .context("failed to build HTTP client")?;

    progress(&format!("Fetching {VERSION_FILE}…"));
    let version_url = cfg.file_url(VERSION_FILE);

    match client.get(&version_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let bytes = resp.bytes().await.context("reading version.json body")?;
            match serde_json::from_slice::<Manifest>(&bytes) {
                Ok(manifest) => {
                    if let Some(tag) = &manifest.tokenizer {
                        tokenizer_tag = tag.clone();
                    }
                    fetch_by_manifest(cfg, &client, &manifest, &mut meta, progress, &mut warnings)
                        .await?;
                    write_atomic(&cfg.data_dir().join(VERSION_FILE), &bytes)?;
                }
                Err(e) => {
                    warnings.push(format!(
                        "version.json exists upstream but could not be parsed ({e}); falling back to conditional fetches"
                    ));
                    fetch_by_etag(cfg, &client, &mut meta, progress, &mut warnings).await?;
                }
            }
        }
        Ok(resp) if resp.status() == reqwest::StatusCode::NOT_FOUND => {
            // Expected until version.json lands upstream (§5). Warn once.
            warnings.push(
                "version.json not found upstream yet; using conditional (ETag) fetches instead"
                    .to_string(),
            );
            fetch_by_etag(cfg, &client, &mut meta, progress, &mut warnings).await?;
        }
        Ok(resp) => {
            let status = resp.status();
            if cache_complete(cfg) {
                warnings.push(format!(
                    "data host returned HTTP {status} for version.json; using cached data"
                ));
            } else {
                bail!(
                    "data host returned HTTP {status} for {version_url} and no local cache exists"
                );
            }
        }
        Err(e) => {
            if cache_complete(cfg) {
                warnings.push(format!("offline? ({e}); using cached data"));
            } else {
                return Err(anyhow::Error::new(e).context(format!(
                    "cannot reach the data host ({version_url}) and no local cache exists"
                )));
            }
        }
    }

    // Record that a check happened regardless of outcome (including errors)
    // so a rate-limited or unreachable host can't be hammered by repeated
    // quick launches either — the window backs off the same as a success.
    meta.last_checked_unix = Some(unix_now());
    save_meta(cfg, &meta)?;
    if !cache_complete(cfg) {
        bail!(
            "data files are missing after fetch — expected {} and {}",
            cfg.qa_data_path().display(),
            cfg.embeddings_path().display()
        );
    }
    Ok(DataFiles {
        qa_path: cfg.qa_data_path(),
        embeddings_path: cfg.embeddings_path(),
        tokenizer_tag,
        warnings,
    })
}

/// §5 path: compare manifest hashes to what each cached file was stored
/// with; fetch only changed/missing files.
async fn fetch_by_manifest(
    cfg: &Config,
    client: &reqwest::Client,
    manifest: &Manifest,
    meta: &mut Meta,
    progress: &(dyn Fn(&str) + Send + Sync),
    warnings: &mut Vec<String>,
) -> Result<()> {
    for name in [QA_DATA_FILE, EMBEDDINGS_FILE] {
        let local = cfg.data_dir().join(name);
        let Some(mf) = manifest.files.get(name) else {
            // Manifest exists but doesn't list this file: fall back to a
            // conditional fetch for it so we never serve nothing.
            warnings.push(format!(
                "version.json does not list {name}; fetching it conditionally"
            ));
            fetch_one_conditional(cfg, client, name, meta, progress).await?;
            continue;
        };
        let stored = meta.files.get(name).and_then(|m| m.manifest_hash.clone());
        if local.exists() && stored.as_deref() == Some(mf.hash.as_str()) {
            continue; // unchanged
        }
        progress(&format!("Downloading {name}…"));
        let url = cfg.file_url(name);
        let resp = client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("fetching {url}"))?
            .error_for_status()
            .with_context(|| format!("fetching {url}"))?;
        let etag = header_string(resp.headers(), reqwest::header::ETAG);
        let bytes = resp
            .bytes()
            .await
            .with_context(|| format!("reading {name}"))?;
        write_atomic(&local, &bytes)?;
        meta.files.insert(
            name.to_string(),
            FileMeta {
                manifest_hash: Some(mf.hash.clone()),
                etag,
            },
        );
    }
    Ok(())
}

/// Pre-version.json fallback: conditional GET per data file.
async fn fetch_by_etag(
    cfg: &Config,
    client: &reqwest::Client,
    meta: &mut Meta,
    progress: &(dyn Fn(&str) + Send + Sync),
    warnings: &mut Vec<String>,
) -> Result<()> {
    for name in [QA_DATA_FILE, EMBEDDINGS_FILE] {
        if let Err(e) = fetch_one_conditional(cfg, client, name, meta, progress).await {
            let local = cfg.data_dir().join(name);
            if local.exists() {
                warnings.push(format!(
                    "could not refresh {name} ({e:#}); using cached copy"
                ));
            } else {
                return Err(e);
            }
        }
    }
    Ok(())
}

async fn fetch_one_conditional(
    cfg: &Config,
    client: &reqwest::Client,
    name: &str,
    meta: &mut Meta,
    progress: &(dyn Fn(&str) + Send + Sync),
) -> Result<()> {
    let local = cfg.data_dir().join(name);
    let url = cfg.file_url(name);
    progress(&format!("Checking {name}…"));

    let mut req = client.get(&url);
    if local.exists()
        && let Some(etag) = meta.files.get(name).and_then(|m| m.etag.clone())
    {
        req = req.header(reqwest::header::IF_NONE_MATCH, etag);
    }
    let resp = req
        .send()
        .await
        .with_context(|| format!("fetching {url}"))?;
    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(());
    }
    let resp = resp
        .error_for_status()
        .with_context(|| format!("fetching {url}"))?;
    progress(&format!("Downloading {name}…"));
    let etag = header_string(resp.headers(), reqwest::header::ETAG);
    let bytes = resp
        .bytes()
        .await
        .with_context(|| format!("reading {name}"))?;
    write_atomic(&local, &bytes)?;
    meta.files.insert(
        name.to_string(),
        FileMeta {
            manifest_hash: None,
            etag,
        },
    );
    Ok(())
}

fn header_string(
    headers: &reqwest::header::HeaderMap,
    key: reqwest::header::HeaderName,
) -> Option<String> {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn manifest_parses_the_claude_md_example() {
        let json = r#"{
            "generated_at": "2026-06-30T12:00:00Z",
            "schema_version": 3,
            "tokenizer": "lindera-ipadic",
            "embedding_model": { "small": "multilingual-e5-small", "large": "multilingual-e5-large" },
            "files": {
                "q_and_a_data.json": { "hash": "aa", "size": 584110 },
                "embeddings.json":   { "hash": "bb", "size": 5720000 }
            }
        }"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.tokenizer.as_deref(), Some("lindera-ipadic"));
        assert_eq!(
            m.embedding_model.as_ref().unwrap()["large"],
            "multilingual-e5-large"
        );
        assert_eq!(m.files["q_and_a_data.json"].hash, "aa");
        assert_eq!(m.files["embeddings.json"].size, Some(5720000));
    }

    #[test]
    fn skip_check_within_window() {
        // checked 100s ago, window is 900s → skip
        assert!(should_skip_check(Some(1_000), 1_100, 900));
    }

    #[test]
    fn skip_check_expired_window() {
        // checked 1000s ago, window is 900s → don't skip
        assert!(!should_skip_check(Some(1_000), 2_000, 900));
    }

    #[test]
    fn skip_check_never_checked_before() {
        assert!(!should_skip_check(None, 1_000, 900));
    }

    #[test]
    fn skip_check_zero_interval_always_checks() {
        // refresh_interval_secs == 0 means "check every launch" (old behavior),
        // even if we just checked a moment ago.
        assert!(!should_skip_check(Some(1_000), 1_000, 0));
    }

    #[test]
    fn skip_check_boundary_is_inclusive_of_expiry() {
        // exactly at the window edge: age == interval, no longer "within" it
        assert!(!should_skip_check(Some(1_000), 1_900, 900));
    }

    #[test]
    fn manifest_tolerates_unknown_and_missing_fields() {
        let m: Manifest = serde_json::from_str(r#"{"files":{}, "future_field": 1}"#).unwrap();
        assert!(m.tokenizer.is_none());
    }
}
