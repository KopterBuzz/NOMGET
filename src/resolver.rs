use crate::manifest::{Artifact, Manifest, Mod};
use anyhow::{bail, Result};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ResolvedItem {
    pub mod_id: String,
    pub mod_info: Mod,
    pub artifact: Artifact,
}

#[derive(Debug, Default)]
pub struct ResolutionPlan {
    pub items: Vec<ResolvedItem>,
}

pub struct DependencyResolver<'a> {
    manifest: &'a Manifest,
}

impl<'a> DependencyResolver<'a> {
    pub fn new(manifest: &'a Manifest) -> Self {
        Self { manifest }
    }

    pub fn resolve(
        &self,
        root_mod_id: &str,
        requested_version: Option<&str>,
    ) -> Result<ResolutionPlan> {
        let mut ordered_items: Vec<ResolvedItem> = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut in_progress: HashSet<String> = HashSet::new();

        self.resolve_recursive(
            root_mod_id,
            requested_version,
            &mut ordered_items,
            &mut visited,
            &mut in_progress,
        )?;

        // Verify incompatibilities across planned items
        for item in &ordered_items {
            for incomp in &item.artifact.incompatibilities {
                if let Some(conflict) = ordered_items.iter().find(|other| other.mod_id == incomp.id) {
                    log::warn!(
                        "Warning: Mod '{}' lists incompatibility with '{}' (version {:?}), conflict detected with '{}'",
                        item.mod_id,
                        incomp.id,
                        incomp.version,
                        conflict.mod_id
                    );
                }
            }
        }

        Ok(ResolutionPlan { items: ordered_items })
    }

    fn resolve_recursive(
        &self,
        mod_id: &str,
        requested_version: Option<&str>,
        plan: &mut Vec<ResolvedItem>,
        visited: &mut HashSet<String>,
        in_progress: &mut HashSet<String>,
    ) -> Result<()> {
        if in_progress.contains(mod_id) {
            bail!("Circular dependency detected involving mod: {}", mod_id);
        }

        if visited.contains(mod_id) {
            return Ok(());
        }

        let mod_info = self
            .manifest
            .get_mod(mod_id)
            .ok_or_else(|| anyhow::anyhow!("Mod '{}' not found in manifest", mod_id))?
            .clone();

        let artifact = self
            .manifest
            .get_artifact(mod_id, requested_version)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No matching artifact found for mod '{}' (requested version: {:?})",
                    mod_id,
                    requested_version
                )
            })?;

        in_progress.insert(mod_id.to_string());

        // Resolve dependencies first
        for dep in &artifact.dependencies {
            log::info!("Resolving dependency for '{}': '{}' (min version: {:?})", mod_id, dep.id, dep.version);
            self.resolve_recursive(
                &dep.id,
                dep.version.as_deref(),
                plan,
                visited,
                in_progress,
            )?;
        }

        // Resolve parent mod if this is an extension/add-on
        if let Some(ext) = &artifact.extends {
            log::info!("Resolving parent extension for '{}': '{}'", mod_id, ext.id);
            self.resolve_recursive(
                &ext.id,
                ext.version.as_deref(),
                plan,
                visited,
                in_progress,
            )?;
        }

        in_progress.remove(mod_id);
        visited.insert(mod_id.to_string());

        plan.push(ResolvedItem {
            mod_id: mod_info.id.clone(),
            mod_info,
            artifact,
        });

        Ok(())
    }
}
