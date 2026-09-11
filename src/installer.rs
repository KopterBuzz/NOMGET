use crate::manifest::{Artifact, Manifest};
use crate::target::TargetInstallation;
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ModMeta {
    pub id: String,
    pub artifact: Artifact,
}

pub struct ModInstaller<'a> {
    target: &'a TargetInstallation,
    manifest: &'a Manifest,
    ignore_hash_mismatch: bool,
}

impl<'a> ModInstaller<'a> {
    pub fn new(target: &'a TargetInstallation, manifest: &'a Manifest) -> Self {
        Self {
            target,
            manifest,
            ignore_hash_mismatch: false,
        }
    }

    pub fn with_ignore_hash(mut self, ignore: bool) -> Self {
        self.ignore_hash_mismatch = ignore;
        self
    }

    pub fn install(&self, mod_id: &str, requested_version: Option<&str>) -> Result<Vec<String>> {
        if !self.target.is_bepinex_installed() {
            bail!(
                "BepInEx is not installed in the target installation ({}). Please install BepInEx first.",
                self.target.root_path.display()
            );
        }

        let plan = self.manifest.resolve_dependencies(mod_id, requested_version)?;

        log::info!(
            "Resolved dependency chain with {} item(s): {}",
            plan.items.len(),
            plan.items
                .iter()
                .map(|i| format!("{} ({})", i.mod_id, i.artifact.version))
                .collect::<Vec<_>>()
                .join(" -> ")
        );

        let mut installed = Vec::new();

        for item in &plan.items {
            let target_dir = self.determine_mod_dir(&item.mod_id, &item.artifact);
            if self.is_version_installed(&target_dir, &item.artifact.version) {
                log::info!(
                    "Mod '{}' version '{}' is already installed at {}",
                    item.mod_id,
                    item.artifact.version,
                    target_dir.display()
                );
                continue;
            }

            log::info!(
                "Installing '{}' ({}) from {}",
                item.mod_id,
                item.artifact.version,
                item.artifact.download_url
            );

            self.download_and_install_artifact(&item.mod_id, &item.artifact, &target_dir)?;
            installed.push(format!("{} ({})", item.mod_id, item.artifact.version));
        }

        Ok(installed)
    }

    pub fn update(&self, mod_id: &str) -> Result<bool> {
        let installed = self.list_installed()?;
        let current = installed.iter().find(|m| m.id.eq_ignore_ascii_case(mod_id));

        let latest_artifact = self.manifest.get_artifact(mod_id, None).ok_or_else(|| {
            anyhow::anyhow!("Mod '{}' not found in manifest", mod_id)
        })?;

        if let Some(installed_meta) = current {
            if installed_meta.artifact.version == latest_artifact.version {
                log::info!(
                    "Mod '{}' is already up to date (version {})",
                    mod_id,
                    installed_meta.artifact.version
                );
                return Ok(false);
            }
        }

        log::info!("Updating mod '{}' to version {}", mod_id, latest_artifact.version);
        self.install(mod_id, Some(&latest_artifact.version))?;
        Ok(true)
    }

    pub fn list_installed(&self) -> Result<Vec<ModMeta>> {
        let plugins = self.target.plugins_dir();
        if !plugins.is_dir() {
            return Ok(Vec::new());
        }

        let mut list = Vec::new();
        let mut check_dir = |dir: &Path| {
            let meta_path = dir.join("meta.json");
            if meta_path.is_file() {
                if let Ok(content) = fs::read_to_string(&meta_path) {
                    if let Ok(meta) = serde_json::from_str::<ModMeta>(&content) {
                        list.push(meta);
                    }
                }
            }
        };

        if let Ok(entries) = fs::read_dir(&plugins) {
            for entry in entries.filter_map(|e| e.ok()) {
                let p = entry.path();
                if p.is_dir() {
                    check_dir(&p);
                    let addons_dir = p.join("addons");
                    if let Ok(addon_entries) = fs::read_dir(&addons_dir) {
                        for a_entry in addon_entries.filter_map(|e| e.ok()) {
                            let ap = a_entry.path();
                            if ap.is_dir() {
                                check_dir(&ap);
                            }
                        }
                    }
                }
            }
        }

        Ok(list)
    }

    fn determine_mod_dir(&self, mod_id: &str, artifact: &Artifact) -> PathBuf {
        if let Some(ext) = &artifact.extends {
            self.target
                .plugins_dir()
                .join(&ext.id)
                .join("addons")
                .join(mod_id)
        } else {
            self.target.plugins_dir().join(mod_id)
        }
    }

    fn is_version_installed(&self, mod_dir: &Path, expected_version: &str) -> bool {
        let meta_path = mod_dir.join("meta.json");
        if !meta_path.is_file() {
            return false;
        }

        if let Ok(content) = fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<ModMeta>(&content) {
                return meta.artifact.version == expected_version;
            }
        }
        false
    }

    fn download_and_install_artifact(
        &self,
        mod_id: &str,
        artifact: &Artifact,
        mod_dir: &Path,
    ) -> Result<()> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("NOMGET/0.1.0")
            .build()?;

        let response = client
            .get(&artifact.download_url)
            .send()
            .with_context(|| format!("Failed to download artifact from {}", artifact.download_url))?;

        if !response.status().is_success() {
            bail!(
                "Download failed for {}: HTTP {} from {}",
                mod_id,
                response.status(),
                artifact.download_url
            );
        }

        let bytes = response.bytes()?;

        // Verify hash if present
        if let Some(expected_hash) = &artifact.hash {
            if let Some(hash_hex) = expected_hash.strip_prefix("sha256:") {
                let mut hasher = Sha256::new();
                hasher.update(&bytes);
                let actual_hex = hex::encode(hasher.finalize());

                if !actual_hex.eq_ignore_ascii_case(hash_hex) {
                    if self.ignore_hash_mismatch {
                        log::warn!(
                            "Hash mismatch for {}: expected {}, got {} (ignored due to flag)",
                            mod_id,
                            hash_hex,
                            actual_hex
                        );
                    } else {
                        bail!(
                            "SHA256 checksum mismatch for mod '{}': expected {}, got {}",
                            mod_id,
                            hash_hex,
                            actual_hex
                        );
                    }
                } else {
                    log::info!("SHA256 checksum verified for mod '{}'", mod_id);
                }
            }
        }

        // Clean previous directory if existing
        if mod_dir.exists() {
            let _ = fs::remove_dir_all(mod_dir);
        }
        fs::create_dir_all(mod_dir)?;

        // Check if file is a zip archive
        let cursor = std::io::Cursor::new(&bytes);
        if let Ok(mut archive) = zip::ZipArchive::new(cursor) {
            let has_bepinex_root = (0..archive.len()).any(|i| {
                if let Ok(file) = archive.by_index(i) {
                    let name = file.name().replace('\\', "/");
                    name.starts_with("BepInEx/") || name.starts_with("plugins/")
                } else {
                    false
                }
            });

            let dest = if has_bepinex_root {
                log::info!("Archive contains root BepInEx folder, extracting to game root");
                &self.target.root_path
            } else {
                mod_dir
            };
            crate::bepinex::extract_zip_to_directory(&bytes, dest)?;
        } else {
            let dest_file = mod_dir.join(&artifact.file_name);
            let mut out = File::create(&dest_file)?;
            out.write_all(&bytes)?;
        }

        // Write meta.json
        let meta = ModMeta {
            id: mod_id.to_string(),
            artifact: artifact.clone(),
        };
        let meta_json = serde_json::to_string_pretty(&meta)?;
        fs::write(mod_dir.join("meta.json"), meta_json)?;

        log::info!("Successfully installed '{}' to {}", mod_id, mod_dir.display());
        Ok(())
    }
}
