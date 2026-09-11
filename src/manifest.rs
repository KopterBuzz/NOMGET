use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub const DEFAULT_MANIFEST_URL: &str =
    "https://kopterbuzz.github.io/NOMNOM/manifest/manifest.json";

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

pub struct ManifestClient {
    source: String,
}

impl ManifestClient {
    pub fn new(source: Option<&str>) -> Self {
        Self {
            source: source.unwrap_or(DEFAULT_MANIFEST_URL).to_string(),
        }
    }

    pub fn fetch(&self) -> Result<Manifest> {
        if self.source.starts_with("http://") || self.source.starts_with("https://") {
            log::info!("Fetching manifest from URL: {}", self.source);
            let client = reqwest::blocking::Client::builder()
                .user_agent("NOMGET/0.1.0")
                .build()?;

            let response = client
                .get(&self.source)
                .send()
                .with_context(|| format!("Failed to fetch manifest from {}", self.source))?;

            if !response.status().is_success() {
                anyhow::bail!(
                    "Failed to fetch manifest: HTTP {} from {}",
                    response.status(),
                    self.source
                );
            }

            let text = response.text()?;
            let raw: ManifestRaw = serde_json::from_str(&text)
                .context("Failed to parse manifest JSON from remote")?;
            let mods = match raw {
                ManifestRaw::List(list) => list.into_iter().map(|m| (m.id.clone(), m)).collect(),
                ManifestRaw::Map(map) => map,
            };
            Ok(Manifest { mods })
        } else {
            let path = Path::new(&self.source);
            log::info!("Reading manifest from local file: {}", path.display());
            let text = fs::read_to_string(path)
                .with_context(|| format!("Failed to read local manifest: {}", path.display()))?;
            let raw: ManifestRaw = serde_json::from_str(&text)
                .context("Failed to parse local manifest JSON")?;
            let mods = match raw {
                ManifestRaw::List(list) => list.into_iter().map(|m| (m.id.clone(), m)).collect(),
                ManifestRaw::Map(map) => map,
            };
            Ok(Manifest { mods })
        }
    }
}

impl Manifest {
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
