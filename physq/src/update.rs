//! `physq update`: self-update against GitHub Releases.
//!
//! Releases are tagged `physq-v<semver>` (CLAUDE.md-adjacent convention, kept
//! distinct from any future non-physq tags in the same repo). A tag's semver
//! may carry a prerelease component (`0.1.2-rc1`) for release-candidate
//! builds. Channel selection is purely a filter over that parsed version:
//! `stable` keeps only tags with no prerelease component, `beta` keeps all of
//! them. Picking the *maximum* semver-ordered version within that filtered
//! set is what makes the whole thing behave correctly around the tricky
//! case the owner flagged: SemVer defines `X.Y.Z-rcN < X.Y.Z`, so going from
//! a beta back to `update` (stable) never silently "downgrades" you to an
//! older release — it correctly resolves to the release version of the same
//! line once it ships, and refuses (unless `--force`) if the currently
//! running version is genuinely ahead of every stable tag published so far.
//!
//! Each release additionally publishes small, unarchived per-target binaries
//! (`physq-bin-<target-triple>[.exe]`, CI-side) alongside the human-facing
//! `.tar.gz`/`.zip` archives, plus a `checksums.txt` covering just those raw
//! binaries — `update` downloads the matching raw binary directly (no
//! tar/zip decoding needed in the client) and verifies its SHA-256 before
//! ever handing it to `self_replace`.
//!
//! Intel Mac (x86_64-apple-darwin) is the one platform whose binary is
//! *not* self-contained: it dynamically links a bundled Microsoft ONNX
//! Runtime dylib (ort/ONNX Runtime is in its final x86_64 macOS release,
//! 1.23.2 — see `.github/workflows/physq-release.yml`), which the release
//! workflow ships next to the binary as `libonnxruntime.1.23.2.dylib`.
//! `update` downloads that companion asset too and places it beside the
//! executable; without it the Intel binary would not even start (dyld
//! resolves the `@loader_path` dylib at process load).

use std::io::Write;

use anyhow::{Context, Result, bail};
use semver::Version;
use serde::Deserialize;

use crate::data::sha256_hex;

const GITHUB_API_RELEASES: &str =
    "https://api.github.com/repos/legrs/physics_notes/releases?per_page=100";
const TAG_PREFIX: &str = "physq-v";
const CHECKSUMS_ASSET: &str = "checksums.txt";
/// Release-side name of the Microsoft ONNX Runtime dylib bundled with the
/// Intel Mac binary. The version in the filename must match
/// `physq/scripts/install_ort_intel.sh`'s `VERSION` (and the release
/// workflow's copy of it) — update.rs just mirrors whatever they ship.
const INTEL_ORT_DYLIB_ASSET: &str = "libonnxruntime.1.23.2.dylib";

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// Everything needed to actually perform an update, plus enough context
/// (`current`/`target`) for the caller to decide whether to proceed.
pub struct UpdatePlan {
    pub current: Version,
    pub target: Version,
    pub tag: String,
    asset_url: String,
    checksums_url: Option<String>,
    /// Intel Mac only: the ONNX Runtime dylib that must sit next to the
    /// binary for it to launch. `None` elsewhere.
    companion_asset_url: Option<String>,
}

fn http_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("physq/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .context("failed to build HTTP client")
}

/// The raw-binary asset name this exact build should look for, matching the
/// naming the release workflow uses. Recognized = the target triples physq
/// actually ships prebuilt binaries for. Linux needs glibc >= 2.38 (see
/// physq/README.md). Intel Mac also has a companion dylib asset — see
/// `companion_asset_name()`.
fn asset_name_for_this_platform() -> Result<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Ok("physq-bin-aarch64-apple-darwin"),
        ("macos", "x86_64") => Ok("physq-bin-x86_64-apple-darwin"),
        ("linux", "x86_64") => Ok("physq-bin-x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Ok("physq-bin-aarch64-unknown-linux-gnu"),
        ("windows", "x86_64") => Ok("physq-bin-x86_64-pc-windows-msvc.exe"),
        ("windows", "aarch64") => Ok("physq-bin-aarch64-pc-windows-msvc.exe"),
        (os, arch) => bail!(
            "no prebuilt physq binary for {os}/{arch}; build from source instead (see README)"
        ),
    }
}

/// Intel Mac binaries dynamically link a bundled ONNX Runtime dylib, so an
/// update must fetch that companion asset as well. Every other platform
/// returns `None` (nothing changes for them).
fn companion_asset_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "x86_64") => Some(INTEL_ORT_DYLIB_ASSET),
        _ => None,
    }
}

fn parse_tag_version(tag: &str) -> Option<Version> {
    Version::parse(tag.strip_prefix(TAG_PREFIX)?).ok()
}

/// Pick the highest-SemVer release that (a) isn't a draft, (b) matches the
/// requested channel (stable = no prerelease component, beta = any), and
/// (c) actually ships a binary for `asset_name`. Pure and independent of the
/// network call so the selection logic (including the draft/asset filters)
/// is unit-testable without a live server.
fn pick_best<'a>(
    releases: &'a [Release],
    beta: bool,
    asset_name: &str,
) -> Option<(Version, &'a Release)> {
    let mut best: Option<(Version, &Release)> = None;
    for r in releases {
        if r.draft {
            continue;
        }
        let Some(v) = parse_tag_version(&r.tag_name) else {
            continue; // not a physq-v* tag (or unparseable) — ignore
        };
        if !beta && !v.pre.is_empty() {
            continue; // stable channel: skip release candidates
        }
        if !r.assets.iter().any(|a| a.name == asset_name) {
            continue; // no binary for this platform on this release
        }
        if best.as_ref().is_none_or(|(bv, _)| v > *bv) {
            best = Some((v, r));
        }
    }
    best
}

/// Fetch the release list and resolve the best available version for
/// `beta`'s channel (stable = release tags only, beta = release candidates
/// too). Does not compare against the running version — that's the
/// caller's call, since "would this be a downgrade" is a UI decision, not a
/// resolution one.
pub fn resolve(beta: bool) -> Result<UpdatePlan> {
    let current = Version::parse(env!("CARGO_PKG_VERSION"))
        .expect("CARGO_PKG_VERSION is set from Cargo.toml and must be valid semver");
    let client = http_client()?;
    let asset_name = asset_name_for_this_platform()?;

    let releases: Vec<Release> = client
        .get(GITHUB_API_RELEASES)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .context("fetching release list from GitHub")?
        .error_for_status()
        .context("fetching release list from GitHub")?
        .bytes()
        .context("reading GitHub releases response")
        .and_then(|b| {
            serde_json::from_slice(&b).context("parsing GitHub releases response as JSON")
        })?;

    let Some((target, release)) = pick_best(&releases, beta, asset_name) else {
        let channel = if beta { "beta" } else { "stable" };
        bail!("no published physq release found for {channel} on this platform ({asset_name})");
    };

    let asset_url = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .map(|a| a.browser_download_url.clone())
        .expect("checked above");
    let checksums_url = release
        .assets
        .iter()
        .find(|a| a.name == CHECKSUMS_ASSET)
        .map(|a| a.browser_download_url.clone());
    let companion_asset_url = companion_asset_name().and_then(|n| {
        release
            .assets
            .iter()
            .find(|a| a.name == n)
            .map(|a| a.browser_download_url.clone())
    });

    Ok(UpdatePlan {
        current,
        target,
        tag: release.tag_name.clone(),
        asset_url,
        checksums_url,
        companion_asset_url,
    })
}

/// Fetch `checksums.txt` and return name → SHA-256 hex. The file covers the
/// raw `physq-bin-*` binaries plus the Intel Mac ONNX Runtime dylib.
fn fetch_checksums(
    client: &reqwest::blocking::Client,
    url: &str,
) -> Result<std::collections::HashMap<String, String>> {
    let text = client
        .get(url)
        .send()
        .context("downloading checksums.txt")?
        .error_for_status()
        .context("downloading checksums.txt")?
        .text()
        .context("reading checksums.txt")?;
    Ok(text
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let hash = parts.next()?;
            let name = parts.next()?.trim_start_matches('*').to_string();
            Some((name, hash.to_string()))
        })
        .collect())
}

/// Download an asset and verify its SHA-256 against the release's
/// `checksums.txt`.
fn download_verified(
    client: &reqwest::blocking::Client,
    url: &str,
    name: &str,
    checksums: &std::collections::HashMap<String, String>,
) -> Result<Vec<u8>> {
    let bytes = client
        .get(url)
        .send()
        .with_context(|| format!("downloading {name}"))?
        .error_for_status()
        .with_context(|| format!("downloading {name}"))?
        .bytes()
        .with_context(|| format!("reading {name}"))?;
    let expected = checksums
        .get(name)
        .with_context(|| format!("checksums.txt has no entry for {name}"))?;
    let actual = sha256_hex(&bytes);
    if actual != *expected {
        bail!(
            "downloaded {name} failed checksum verification (expected {expected}, got {actual}) — try again"
        );
    }
    Ok(bytes.to_vec())
}

/// Download the resolved binary (and, on Intel Mac, the bundled ONNX
/// Runtime dylib), verify each against the release's `checksums.txt`, and
/// replace the running executable via `self_replace`. `progress` receives
/// short phase labels for the caller's spinner.
pub fn apply(plan: &UpdatePlan, progress: &dyn Fn(&str)) -> Result<()> {
    let client = http_client()?;
    let asset_name = asset_name_for_this_platform()?;

    progress(&format!("Downloading physq {}…", plan.target));
    let checksums = match &plan.checksums_url {
        Some(url) => Some(fetch_checksums(&client, url)?),
        None => {
            progress("(no checksums.txt on this release; skipping verification)");
            None
        }
    };
    let bytes = match &checksums {
        Some(map) => download_verified(&client, &plan.asset_url, asset_name, map)?,
        None => client
            .get(&plan.asset_url)
            .send()
            .context("downloading the new physq binary")?
            .error_for_status()
            .context("downloading the new physq binary")?
            .bytes()
            .context("reading the downloaded binary")?
            .to_vec(),
    };

    progress("Installing…");
    let dir = tempfile::Builder::new()
        .prefix("physq-update-")
        .tempdir()
        .context("creating a temp dir for the downloaded binary")?;
    let new_exe_path = dir
        .path()
        .join(if cfg!(windows) { "physq.exe" } else { "physq" });
    {
        let mut f = std::fs::File::create(&new_exe_path)
            .with_context(|| format!("writing {}", new_exe_path.display()))?;
        f.write_all(&bytes)
            .with_context(|| format!("writing {}", new_exe_path.display()))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&new_exe_path, std::fs::Permissions::from_mode(0o755))
            .with_context(|| format!("setting {} executable", new_exe_path.display()))?;
    }

    // On Intel Mac, install the companion ONNX Runtime dylib into the same
    // directory as the executable before replacing anything, so the
    // replaced binary can actually launch. Ignored everywhere else.
    if let Some(url) = &plan.companion_asset_url {
        let Some(name) = companion_asset_name() else {
            unreachable!("companion url implies a companion asset name");
        };
        let checksums = checksums.as_ref().with_context(|| {
            format!(
                "Intel Mac update needs checksums.txt to verify {name}, but the release has none"
            )
        })?;
        let dylib = download_verified(&client, url, name, checksums)?;
        let exe_dir = std::env::current_exe()
            .context("locating the running executable")?
            .parent()
            .context("running executable has no parent dir")?
            .to_path_buf();
        let dest = exe_dir.join(name);
        std::fs::write(&dest, dylib).with_context(|| format!("writing {}", dest.display()))?;
    }

    self_replace::self_replace(&new_exe_path).context("replacing the running executable")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stable_and_prerelease_tags() {
        assert_eq!(
            parse_tag_version("physq-v0.1.1").unwrap(),
            Version::new(0, 1, 1)
        );
        let rc = parse_tag_version("physq-v0.1.1-rc1").unwrap();
        assert_eq!(rc, Version::parse("0.1.1-rc1").unwrap());
    }

    #[test]
    fn rejects_tags_outside_the_physq_namespace() {
        assert!(parse_tag_version("v0.1.1").is_none());
        assert!(parse_tag_version("physics-notes-v1.0.0").is_none());
        assert!(parse_tag_version("physq-vnotasemver").is_none());
    }

    /// The exact case the owner asked about: a stable release must outrank
    /// its own release-candidate, and a newer minor line must outrank an
    /// older stable release — both directions matter for the downgrade
    /// guard in `cli.rs` to behave correctly.
    #[test]
    fn semver_ordering_handles_the_beta_to_stable_case() {
        let rc = Version::parse("0.1.1-rc1").unwrap();
        let stable_same = Version::parse("0.1.1").unwrap();
        let stable_older = Version::parse("0.1.0").unwrap();
        let stable_newer_line = Version::parse("0.2.0").unwrap();

        assert!(stable_same > rc, "a release must outrank its own rc");
        assert!(rc > stable_older, "an rc must outrank an older stable line");
        assert!(
            stable_newer_line > rc,
            "a newer stable line must outrank an older rc"
        );
    }

    fn release(tag: &str, draft: bool, asset_names: &[&str]) -> Release {
        Release {
            tag_name: tag.to_string(),
            draft,
            assets: asset_names
                .iter()
                .map(|n| Asset {
                    name: n.to_string(),
                    browser_download_url: format!("https://example.invalid/{n}"),
                })
                .collect(),
        }
    }

    const ASSET: &str = "physq-bin-aarch64-apple-darwin";

    #[test]
    fn pick_best_ignores_drafts_and_foreign_tags_and_missing_assets() {
        let releases = vec![
            release("physq-v0.2.0", true, &[ASSET]), // draft: must be ignored
            release("physics-notes-v9.9.9", false, &[ASSET]), // not our namespace
            release("physq-v0.1.2", false, &["checksums.txt"]), // no matching binary
            release("physq-v0.1.1", false, &[ASSET, "checksums.txt"]),
        ];
        let (v, r) = pick_best(&releases, false, ASSET).expect("should find a candidate");
        assert_eq!(v, Version::new(0, 1, 1));
        assert_eq!(r.tag_name, "physq-v0.1.1");
    }

    #[test]
    fn stable_channel_skips_release_candidates() {
        let releases = vec![
            release("physq-v0.1.2-rc1", false, &[ASSET]),
            release("physq-v0.1.1", false, &[ASSET]),
        ];
        let (v, _) = pick_best(&releases, false, ASSET).unwrap();
        assert_eq!(v, Version::new(0, 1, 1), "stable must skip the newer rc");

        let (v, _) = pick_best(&releases, true, ASSET).unwrap();
        assert_eq!(
            v,
            Version::parse("0.1.2-rc1").unwrap(),
            "beta includes the rc and it's newer"
        );
    }

    #[test]
    fn pick_best_returns_none_when_nothing_matches() {
        let releases = vec![release("physq-v0.1.1", false, &["checksums.txt"])];
        assert!(pick_best(&releases, true, ASSET).is_none());
        assert!(pick_best(&[], true, ASSET).is_none());
    }
}
