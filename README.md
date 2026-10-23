# NOMGET

A powerful terminal application and library written in Rust for managing Nuclear Option mods, querying the [NOMNOM](https://github.com/KopterBuzz/NOMNOM) manifest, configuring BepInEx 5, and resolving complex mod dependency chains.

## DISCLAIMER

This Project is an EXPERIMENT that was entirely AI generated using a single prompt.

I am not planning to maintain it or address any issue requests or pull requests at the present time. 

You may copy it, fork it etc and do what you want with it.

## Features

- **Query NOMNOM Manifest**:
  - List all catalogued mods with client/server compatibility tags.
  - Case-insensitive search across mod IDs, display names, descriptions, authors, and tags.
  - Detailed mod view showing versions, release categories, dependencies, parent extensions, and SHA256 hashes.
- **Local Manifest Caching**:
  - Caches the manifest and its `version.json` marker under `%USERPROFILE%\.nomget` on Windows and `~/.nomget` on Linux.
  - Runs offline-friendly: the manifest is only redownloaded when the remote version marker is newer than the cached copy.
- **Steam & Target Discovery**:
  - Automatically detect Nuclear Option or Nuclear Option Dedicated Server from local Steam installations on Windows and Linux.
  - Target arbitrary game or dedicated server installations via `--target <PATH>`.
- **BepInEx 5 Automated Installer**:
  - Installs and configures BepInEx 5 for both Windows (x64) and native Linux (x64) dedicated server or client.
  - Automatically generates `run_bepinex.sh` with correct executable names and permissions on Linux.
  - Automatically sets up default `BepInEx.cfg` with `HideManagerGameObject = true`.
- **Mod Management & Dependency Chain Resolution**:
  - Downloads and installs mods directly from release artifacts.
  - Resolves complete dependency chains with topological ordering (dependencies installed before dependents).
  - Handles add-on mods extending parent mods (`addons/<id>` subdirectories).
  - SHA256 integrity verification for every downloaded artifact.
  - Tracks installed mods and versions via `meta.json`.
  - Supports checking and updating installed mods to their latest versions.
- **Cross-Platform**:
  - Compatible with Linux and Windows.
- **Configurable Logging**:
  - Multi-level verbosity (`-v`, `-vv`, `-vvv`, `-q`).

---

## CLI Usage

### Manifest Queries

```sh
# List all mods
nomget manifest list

# List server-compatible mods only
nomget manifest list --server-only

# Search mods by keyword
nomget manifest search blackbox

# View detailed mod information
nomget manifest info NOBlackBox
```

### Target & Installation

```sh
# Detect default local Steam installation
nomget target find

# Inspect target installation details
nomget target info --target /path/to/NuclearOptionServer

# Install BepInEx 5 to the target
nomget bepinex install --target /path/to/NuclearOptionServer

# Install mods and all their dependencies (space- or comma-separated)
nomget mod install NOBlackBox NOHitreg --target /path/to/NuclearOptionServer
nomget mod install "NOBlackBox, NOHitreg" --target /path/to/NuclearOptionServer

# List installed mods in the target
nomget mod list-installed --target /path/to/NuclearOptionServer

# Update installed mods to the latest version
nomget mod update NOBlackBox NOHitreg --target /path/to/NuclearOptionServer

# Launch the game or dedicated server with BepInEx
nomget target launch --target /path/to/NuclearOptionServer -- -batchmode -nographics
```

---

## Library API (Rust)

`nomget` can also be used directly as a Rust crate:

```rust
use nomget::manifest::ManifestClient;
use nomget::target::TargetInstallation;
use nomget::bepinex::install_bepinex;
use nomget::installer::ModInstaller;

// 1. Fetch manifest
let client = ManifestClient::new(None);
let manifest = client.fetch()?;

// 2. Target server or game
let target = TargetInstallation::from_path("/path/to/NuclearOptionServer")?;

// 3. Install BepInEx 5
install_bepinex(&target)?;

// 4. Resolve and install mod dependencies
let installer = ModInstaller::new(&target, &manifest);
let installed = installer.install("NOBlackBox", None)?;
println!("Installed: {:?}", installed);
```
