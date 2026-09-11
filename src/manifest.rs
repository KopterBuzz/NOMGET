use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_MANIFEST_URL: &str =
    "https://kopterbuzz.github.io/NOMNOM/manifest/manifest.json";

const DEFAULT_VERSION_URL: &str =
    "https://kopterbuzz.github.io/NOMNOM/manifest/version.json";

/// Cached manifest filename inside the per-user cache directory.
const MANIFEST_CACHE_FILE: &str = "manifest.json";
/// Cached version-marker filename inside the per-user cache directory.
const VERSION_CACHE_FILE: &str = "version.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Manifest {
    pub mods: HashMap<String, Mod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum ManifestRaw {
    List(Vec<Mod>),
    Map(HashMap<String, Mod>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mod {
    pub id: String,
    pub display_name: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub urls: Vec<ModUrl>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub is_client_or_server: Option<String>,
    #[serde(default)]
    pub github_owner: Option<String>,
    #[serde(default)]
    pub github_repo_name: Option<String>,
    #[serde(default)]
    pub auto_update_artifacts: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub image_hash: Option<String>,
    #[serde(default)]
    pub download_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModUrl {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub file_name: String,
    pub version: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(rename = "type", default)]
    pub artifact_type: Option<String>,
    #[serde(default)]
    pub game_version: Option<String>,
    pub download_url: String,
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub extends: Option<Extends>,
    #[serde(default)]
    pub incompatibilities: Vec<Incompatibility>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub id: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Extends {
    pub id: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Incompatibility {
    pub id: String,
    #[serde(default)]
    pub version: Option<String>,
}

impl Manifest {
    /// Fetch a manifest from the given source, or the default NOMNOM
    /// manifest when `source` is `None`.
    ///
    /// The default NOMNOM manifest is cached locally (see
    /// [`Manifest::fetch_cached`]); explicit URLs or file paths bypass
    /// the cache and are fetched exactly as given.
    pub fn fetch(source: Option<&str>) -> Result<Self> {
        match source {
            Some(src) if src != DEFAULT_MANIFEST_URL => Self::fetch_legacy(src),
            _ => Self::fetch_cached(),
        }
    }

    fn from_text(text: &str, origin: &str) -> Result<Self> {
        let raw: ManifestRaw = serde_json::from_str(text)
            .with_context(|| format!("Failed to parse manifest JSON from {}", origin))?;
        let mods = match raw {
            ManifestRaw::List(list) => list.into_iter().map(|m| (m.id.clone(), m)).collect(),
            ManifestRaw::Map(map) => map,
        };
        Ok(Manifest { mods })
    }

    /// Fetch an explicitly supplied manifest source (custom URL or local
    /// file) without caching.
    fn fetch_legacy(src: &str) -> Result<Self> {
        if src.starts_with("http://") || src.starts_with("https://") {
            log::info!("Fetching manifest from URL: {}", src);
            let text = http_get(src)?;
            Self::from_text(&text, "remote manifest")
        } else {
            let path = Path::new(src);
            log::info!("Reading manifest from local file: {}", path.display());
            let text = fs::read_to_string(path)
                .with_context(|| format!("Failed to read local manifest: {}", path.display()))?;
            Self::from_text(&text, "local manifest")
        }
    }

    /// Fetch the default NOMNOM manifest with local caching.
    ///
    /// The manifest and its companion `version.json` are cached under
    /// `%USERPROFILE%\.nomget` on Windows and `~/.nomget` on Linux.
    /// Each run fetches the remote `version.json` and compares it
    /// with the cached one: the manifest is only redownloaded when the
    /// remote version is newer, keeping the cached `version.json` in
    /// sync. If the remote is unreachable, the cached copy is used.
    fn fetch_cached() -> Result<Self> {
        let cache = cache_dir()?;
        let manifest_path = cache.join(MANIFEST_CACHE_FILE);
        let version_path = cache.join(VERSION_CACHE_FILE);
        let cached_manifest_exists = manifest_path.is_file();

        let cached_version = read_cached_version(&version_path);
        let remote_version_text = match http_get(DEFAULT_VERSION_URL) {
            Ok(text) => Some(text),
            Err(e) => {
                log::warn!(
                    "Failed to fetch remote version check ({}); {}",
                    e,
                    if cached_manifest_exists {
                        "using cached manifest"
                    } else {
                        "proceeding without a version check"
                    }
                );
                None
            }
        };
        let remote_version = remote_version_text.as_deref().map(parse_version_string);

        let use_cache = match (&remote_version, &cached_version) {
            (Some(remote), Some(cached)) => match remote.cmp(cached) {
                Ordering::Greater => {
                    log::info!(
                        "Remote manifest version {} is newer than cached {}, redownloading",
                        remote,
                        cached
                    );
                    false
                }
                Ordering::Less => {
                    log::warn!(
                        "Remote manifest version {} is older than cached {}, keeping local copy",
                        remote,
                        cached
                    );
                    cached_manifest_exists
                }
                Ordering::Equal => {
                    log::info!(
                        "Cached manifest is up to date (version {}), using local copy",
                        remote
                    );
                    cached_manifest_exists
                }
            },
            (None, Some(_)) => cached_manifest_exists,
            _ => {
                log::info!(
                    "No cached manifest found, downloading from {}",
                    DEFAULT_MANIFEST_URL
                );
                false
            }
        };

        if use_cache {
            let text = fs::read_to_string(&manifest_path).with_context(|| {
                format!(
                    "Failed to read cached manifest at {}",
                    manifest_path.display()
                )
            })?;
            return Self::from_text(&text, &format!("cache {}", manifest_path.display()));
        }

        log::info!("Fetching manifest from URL: {}", DEFAULT_MANIFEST_URL);
        let manifest_text = http_get(DEFAULT_MANIFEST_URL)
            .with_context(|| format!("Failed to fetch manifest from {}", DEFAULT_MANIFEST_URL))?;
        let manifest = Self::from_text(&manifest_text, "remote manifest")?;

        // Best-effort cache refresh: a failed cache write must not fail
        // the command, so only the freshness check is authoritative.
        if let Err(e) = write_cached_manifest(
            &cache,
            &manifest_path,
            &version_path,
            &manifest_text,
            remote_version_text,
        ) {
            log::warn!("Failed to update local manifest cache: {}", e);
        }

        Ok(manifest)
    }
    pub fn list_all(&self) -> Vec<&Mod> {
        let mut list: Vec<&Mod> = self.mods.values().collect();
        list.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
        list
    }

    pub fn search(&self, keyword: &str) -> Vec<&Mod> {
        let kw = keyword.trim().to_lowercase();
        let mut results: Vec<&Mod> = self
            .mods
            .values()
            .filter(|m| {
                m.id.to_lowercase().contains(&kw)
                    || m.display_name.to_lowercase().contains(&kw)
                    || m.description.to_lowercase().contains(&kw)
                    || m.tags.iter().any(|t| t.to_lowercase().contains(&kw))
                    || m.authors.iter().any(|a| a.to_lowercase().contains(&kw))
            })
            .collect();
        results.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
        results
    }

    pub fn get_mod(&self, id: &str) -> Option<&Mod> {
        self.mods.get(id).or_else(|| {
            self.mods
                .values()
                .find(|m| m.id.eq_ignore_ascii_case(id) || m.display_name.eq_ignore_ascii_case(id))
        })
    }

    pub fn get_artifact(&self, mod_id: &str, requested_version: Option<&str>) -> Option<Artifact> {
        let m = self.get_mod(mod_id)?;
        if m.artifacts.is_empty() {
            return None;
        }

        if let Some(v) = requested_version {
            m.artifacts
                .iter()
                .find(|a| a.version == v)
                .cloned()
        } else {
            // Default to latest release, or first artifact if none marked release
            m.artifacts
                .iter()
                .find(|a| a.category.as_deref().map(|c| c.eq_ignore_ascii_case("release")).unwrap_or(false))
                .or_else(|| m.artifacts.first())
                .cloned()
        }
    }
}

/// Resolve the per-user cache directory: `%USERPROFILE%\.nomget` on
/// Windows, `~/.nomget` on Linux and other Unixes.
fn cache_dir() -> Result<PathBuf> {
    #[cfg(windows)]
    let root = std::env::var_os("USERPROFILE").map(PathBuf::from);
    #[cfg(not(windows))]
    let root = std::env::var_os("HOME").map(PathBuf::from);

    let root = root.ok_or_else(|| {
        anyhow::anyhow!(
            "Cannot determine user home directory; set {}",
            if cfg!(windows) { "USERPROFILE" } else { "HOME" }
        )
    })?;
    Ok(root.join(".nomget"))
}

fn http_get(url: &str) -> Result<String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("NOMGET/0.1.0")
        .build()?;
    let response = client
        .get(url)
        .send()
        .with_context(|| format!("Failed to fetch {}", url))?;
    if !response.status().is_success() {
        bail!("Failed to fetch {}: HTTP {}", url, response.status());
    }
    Ok(response.text()?)
}

/// Parse a remote or cached `version.json` payload into a [`Version`].
/// Accepts the JSON string (`"1.1.0.2830"`) served by NOMNOM and bare
/// version text for robustness.
fn parse_version_string(text: &str) -> Version {
    let parsed = serde_json::from_str::<String>(text).unwrap_or_else(|_| text.trim().to_string());
    Version::parse(&parsed)
}

fn read_cached_version(path: &Path) -> Option<Version> {
    let text = fs::read_to_string(path).ok()?;
    Some(parse_version_string(&text))
}

/// Write a freshly downloaded manifest and version marker into the cache.
/// Failures are handled by the caller as non-fatal.
fn write_cached_manifest(
    cache: &Path,
    manifest_path: &Path,
    version_path: &Path,
    manifest_text: &str,
    fresh_version_text: Option<String>,
) -> Result<()> {
    fs::create_dir_all(cache)?;
    if let Some(version) = fresh_version_text {
        write_atomic(version_path, &version)?;
    }
    write_atomic(manifest_path, manifest_text)
}

/// Write a file atomically (temp file + rename). Windows `rename` cannot
/// replace an existing destination, so it is removed first.
fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents)?;
    let _ = fs::remove_file(path);
    fs::rename(&tmp, path)?;
    Ok(())
}

/// A dotted-numeric version (e.g. `1.1.0.2830`) compared component-wise.
/// Non-numeric versions fall back to plain string comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Version {
    raw: String,
    parts: Vec<u64>,
    numeric: bool,
}

impl Version {
    fn parse(raw: &str) -> Self {
        let raw = raw.trim().to_string();
        let mut numeric = true;
        let parts = raw
            .split('.')
            .map(|part| match part.parse::<u64>() {
                Ok(value) => value,
                Err(_) => {
                    numeric = false;
                    0
                }
            })
            .collect();
        Version { raw, parts, numeric }
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        if !self.numeric || !other.numeric {
            return self.raw.cmp(&other.raw);
        }
        let len = self.parts.len().max(other.parts.len());
        for i in 0..len {
            let a = self.parts.get(i).copied().unwrap_or(0);
            let b = other.parts.get(i).copied().unwrap_or(0);
            match a.cmp(&b) {
                Ordering::Equal => continue,
                ordering => return ordering,
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

#[cfg(test)]
mod tests {
    use super::Version;
    use std::cmp::Ordering;

    fn cmp(a: &str, b: &str) -> Ordering {
        Version::parse(a).cmp(&Version::parse(b))
    }

    #[test]
    fn numeric_dotted_versions_compare_component_wise() {
        assert_eq!(cmp("1.1.0.2830", "1.1.0.2831"), Ordering::Less);
        assert_eq!(cmp("1.2.0", "1.1.9.999"), Ordering::Greater);
        assert_eq!(cmp("0.9", "0.10"), Ordering::Less); // not lexicographic
        assert_eq!(cmp("1.0.0", "1.0.0.0"), Ordering::Equal);
    }

    #[test]
    fn missing_components_count_as_zero() {
        assert_eq!(cmp("1.0.5", "1"), Ordering::Greater);
        assert_eq!(cmp("1.1", "1.1.0.0"), Ordering::Equal); // semver normalization
    }

    #[test]
    fn non_numeric_versions_fall_back_to_string_order() {
        // Deterministic (string order), never a panic.
        let _ = cmp("1.1.beta", "1.1.0");
        assert_eq!(cmp("2.0", "2.0"), Ordering::Equal);
    }
}
