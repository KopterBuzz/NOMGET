# NOMGET Agent Guide

## 1. Executive Overview

**NOMGET** is a high-performance terminal application and library written in Rust for the combat flight simulator *Nuclear Option* (developed by Shockfront Studios).

NOMGET serves as the package manager and runtime orchestrator for Nuclear Option mods, interfacing directly with the community manifest registry [NOMNOM](https://github.com/KopterBuzz/NOMNOM). It automates the entire lifecycle of modding for both client game installations and dedicated servers on both Windows and Linux:
- Manifest querying, filtering, and keyword searching.
- Local Steam library auto-discovery across multi-library configurations.
- Targeting and verifying arbitrary game or dedicated server installations.
- Automated installation and runtime configuration of **BepInEx 5**.
- Deep dependency chain resolution with topological ordering and cycle detection.
- Cryptographically verified artifact download, archive extraction, add-on extension placement, and metadata tracking.
- Native game/server launching with pre-configured BepInEx runtime environments.

NOMGET is a community project and is not officially affiliated with Shockfront Studios.

---

## 2. Repository Layout

```
NOMGET/
├── agents/                       # Agent documentation & architectural specifications
│   ├── README.md                 # Primary system manual & executive guide (this file)
│   ├── architecture.md           # Component architecture, data flow & internal design
│   ├── workflows.md              # Operational runbooks, build guides & testing loops
│   └── rules.md                  # Strict invariants, safety rules & coding standards
├── src/
│   ├── lib.rs                    # Public Rust library API entrypoint
│   ├── main.rs                   # Terminal CLI application (clap CLI parser)
│   ├── manifest.rs               # NOMNOM manifest models, fetching, search & filtering
│   ├── target.rs                 # Steam library discovery & target directory inspection
│   ├── bepinex.rs                # BepInEx 5 installer, extraction & platform configuration
│   ├── resolver.rs               # Topological dependency chain resolver & DAG validator
│   ├── installer.rs              # Mod artifact installer, hash verification & metadata tracking
│   └── logger.rs                 # Multi-level verbosity logger configuration
├── Cargo.toml                    # Package manifest & minimal dependency definitions
├── Cargo.lock                    # Deterministic dependency lockfile
├── README.md                     # User-facing README and quick start
└── .gitignore                    # Git ignore rules (targets /dist and /target)
```

---

## 3. Core Capabilities & Feature Matrix

| Feature | Description | Modules |
| :--- | :--- | :--- |
| **Manifest Querying** | Query the live NOMNOM manifest over HTTPS or local files; search by keyword, tag, or author; inspect mod metadata and artifact release trees. | `manifest.rs` |
| **Steam Discovery** | Read Steam configurations (`libraryfolders.vdf`) via Windows Registry or Linux standard XDG/Flatpak/Snap paths to automatically locate game/server installations. | `target.rs` |
| **Target Management** | Validate and inspect arbitrary installation directories; distinguish between dedicated servers and client games across PE (.exe) and ELF (.x86_64) binaries. | `target.rs` |
| **BepInEx 5 Deployment** | Download, extract, and configure BepInEx 5 for Windows (x64) and Linux native (x64). Automatically patch `run_bepinex.sh` and generate `BepInEx.cfg`. | `bepinex.rs` |
| **Dependency Resolution** | Traverse mod dependency trees and parent extension hierarchies; compute topological install orders; detect circular dependencies and report incompatibilities. | `resolver.rs` |
| **Artifact Packaging** | Download mod release archives, verify SHA256 checksums, unpack plugins or add-ons (`addons/<id>`), and maintain persistent `meta.json` records. | `installer.rs` |
| **Target Launcher** | Launch the target application with all necessary runtime environment variables (`LD_PRELOAD`, `LD_LIBRARY_PATH`, doorstop settings) pre-configured. | `target.rs` |
| **Logging & Verbosity** | Fine-grained log level control (`-v`, `-vv`, `-vvv`, `-q`) with colored terminal output. | `logger.rs` |

---

## 4. CLI Command Reference

### Global Options
- `-v, --verbose`: Increase verbosity (`-v` = info, `-vv` = debug, `-vvv` = trace).
- `-q, --quiet`: Quiet mode (errors only).
- `-t, --target <PATH>`: Explicit path to the Nuclear Option or Dedicated Server directory.
- `-m, --manifest <URL|PATH>`: Custom manifest URL (HTTP/HTTPS) or local JSON file path.

### Subcommands

#### `manifest`
```sh
# List all mods in the manifest
nomget manifest list

# Filter for server-compatible mods only
nomget manifest list --server-only

# Search mods matching keyword in name, id, description, or tags
nomget manifest search <keyword>

# Display comprehensive details and release artifacts for a specific mod
nomget manifest info <mod_id>
```

#### `target`
```sh
# Locate the default Nuclear Option installation from local Steam libraries
nomget target find

# Inspect the target directory (detected or specified via --target)
nomget target info [--target <path>]

# Launch the targeted game or server with BepInEx configured
nomget target launch [--target <path>] [-- <extra_args>...]
```

#### `bepinex`
```sh
# Install and configure BepInEx 5 for the targeted installation
nomget bepinex install [--target <path>]
```

#### `mod`
```sh
# Install a mod, resolving and installing all upstream dependencies first
nomget mod install <mod_id> [--version <version>] [--target <path>] [--ignore-hash]

# Update an installed mod to the latest release
nomget mod update <mod_id> [--target <path>]

# List all currently installed mods and versions in the target directory
nomget mod list-installed [--target <path>]
```

---

## 5. Rust Library API Usage

NOMGET can be incorporated into external Rust applications as a library crate:

```rust
use nomget::manifest::Manifest;
use nomget::target::TargetInstallation;
use nomget::bepinex::install_bepinex;
use nomget::installer::ModInstaller;
use anyhow::Result;

fn main() -> Result<()> {
    // 1. Fetch remote or local manifest
    let manifest = Manifest::fetch(None)?;

    // 2. Identify target installation
    let target = TargetInstallation::from_path("/home/steam/NuclearOptionServer")?;

    // 3. Ensure BepInEx 5 is provisioned
    if !target.is_bepinex_installed() {
        install_bepinex(&target)?;
    }

    // 4. Resolve dependencies and install mod
    let installer = ModInstaller::new(&target, &manifest);
    let installed_mods = installer.install("NOBlackBox", None)?;

    for mod_name in installed_mods {
        println!("Installed: {}", mod_name);
    }

    Ok(())
}
```
