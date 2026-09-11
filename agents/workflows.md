# NOMGET Operational Workflows

This document outlines step-by-step operational runbooks for agents, CI/CD runners, and developers managing NOMGET and Nuclear Option modding environments.

---

## Workflow 1: Building & Cross-Compiling NOMGET

### Native Linux Build
```sh
cargo build --release
# Artifact produced at: target/release/nomget
```

### Cross-Compiling for Windows from Linux
Prerequisites:
- `x86_64-w64-mingw32-gcc` installed (`apt-get install -y mingw-w64`).
- Rust target installed: `rustup target add x86_64-pc-windows-gnu`.

```sh
cargo build --release --target x86_64-pc-windows-gnu
# Artifact produced at: target/x86_64-pc-windows-gnu/release/nomget.exe
```

### Packaging Release Assets
```sh
mkdir -p dist
cp target/release/nomget dist/
cp target/x86_64-pc-windows-gnu/release/nomget.exe dist/
cp README.md dist/

# Create Linux tarball
tar -czvf dist/nomget-linux-x86_64.tar.gz -C dist nomget README.md

# Create Windows zip
# On Linux:
zip -j dist/nomget-windows-x86_64.zip dist/nomget.exe dist/README.md
# Or on Windows PowerShell:
Compress-Archive -Path "dist\nomget.exe", "dist\README.md" -DestinationPath "dist\nomget-windows-x86_64.zip" -Force
```

---

## Workflow 2: Setting up a Clean Linux Dedicated Server

To create a clean testing environment for Nuclear Option Dedicated Server:
```sh
# 1. Install 32-bit runtime (required by SteamCMD)
sudo dpkg --add-architecture i386
sudo apt-get update
sudo apt-get install -y lib32gcc-s1

# 2. Download and unpack SteamCMD
mkdir -p ~/steamcmd && cd ~/steamcmd
curl -sqL "https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz" | tar zxvf -

# 3. Download fresh Nuclear Option Dedicated Server (AppID 3930080)
~/steamcmd/steamcmd.sh +force_install_dir ~/NuclearOptionServer +login anonymous +app_update 3930080 validate +quit
```

---

## Workflow 3: Provisioning BepInEx 5

To initialize BepInEx 5 on any target (detected automatically or passed via `--target`):

```sh
# 1. Inspect target status before install
nomget target info --target ~/NuclearOptionServer

# 2. Run BepInEx installation
nomget bepinex install --target ~/NuclearOptionServer

# 3. Verify BepInEx installation status
nomget target info --target ~/NuclearOptionServer
# Output should indicate: "BepInEx: Installed"
```

NOMGET handles:
- Downloading the correct platform binary archive (`BepInEx_linux_x64` or `BepInEx_win_x64`).
- Extracting core files (`BepInEx/core/BepInEx.dll`, `libdoorstop.so` or `winhttp.dll`).
- Patching `run_bepinex.sh` with `executable_name="NuclearOptionServer.x86_64"`.
- Setting executable permissions on scripts and binaries.
- Generating default `BepInEx.cfg` with `HideManagerGameObject = true`.

---

## Workflow 4: Installing a Mod with Dependency Chains

To install a mod such as `NOBlackBox`:

```sh
# 1. Optional: Query manifest for mod info and release metadata
nomget manifest info NOBlackBox

# 2. Install the mod
nomget mod install NOBlackBox --target ~/NuclearOptionServer
```

**Execution Pipeline**:
1. NOMGET queries `manifest.json`.
2. Locates `NOBlackBox` and resolves its latest release artifact (`0.3.8.5`).
3. Discovers dependency: `BepInEx.ConfigurationManager (>= 18.4.1)`.
4. Computes topological resolution plan:
   `BepInEx.ConfigurationManager (18.4.1)` -> `NOBlackBox (0.3.8.5)`
5. Downloads `BepInEx.ConfigurationManager_BepInEx5_v18.4.1.zip`, verifies SHA256 checksum, extracts `ConfigurationManager.dll` into `BepInEx/plugins/`.
6. Downloads `NOBlackBox-0.3.8.5.zip`, verifies SHA256 checksum, extracts `NOBlackBox.dll` into `BepInEx/plugins/NOBlackBox/`.
7. Writes persistent `meta.json` tracking installed versions.

---

## Workflow 5: Inspecting Installed Mods & Updating

```sh
# List all installed mods and their version records
nomget mod list-installed --target ~/NuclearOptionServer

# Update a specific mod to the latest version in the manifest
nomget mod update NOBlackBox --target ~/NuclearOptionServer
```

---

## Workflow 6: Launching Target & Validating Chainloader Execution

To launch the server in headless batch mode:

```sh
# Launch server via NOMGET target runner
nomget target launch --target ~/NuclearOptionServer -- -batchmode -nographics
```

### Verification Criteria
Inspect `BepInEx/LogOutput.log` to ensure all plugins in the chainloader loaded:
```text
[Message:   BepInEx] Chainloader ready
[Message:   BepInEx] Chainloader started
[Info   :   BepInEx] 2 plugins to load
[Info   :   BepInEx] Loading [Configuration Manager 18.4.1]
[Info   :   BepInEx] Loading [NOBlackBox 0.3.8.5]
[Message:   BepInEx] Chainloader startup complete
```

---

## Workflow 7: Troubleshooting Common Issues

| Symptom | Cause | Remedy |
| :--- | :--- | :--- |
| `cannot execute: required file not found` on `steamcmd` | Missing 32-bit GLIBC / GCC libraries on 64-bit Linux. | Run `sudo dpkg --add-architecture i386 && sudo apt update && sudo apt install -y lib32gcc-s1`. |
| `SHA256 checksum mismatch for mod ...` | Upstream author re-tagged a release asset or corrupted download. | Verify URL and checksum. Use `--ignore-hash` if intentional. |
| `Circular dependency detected involving mod ...` | Manifest contains cyclical dependencies (`A -> B -> A`). | Report issue to NOMNOM registry. NOMGET aborts before altering target. |
| `BepInEx is not installed in the target installation` | Mod install attempted before BepInEx was deployed. | Run `nomget bepinex install --target <PATH>` first. |
