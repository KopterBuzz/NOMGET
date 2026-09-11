# NOMGET Rules & Engineering Invariants

These rules are non-negotiable standards for any automated agent or human contributor modifying the NOMGET codebase.

---

## 1. Security & Filesystem Invariants

### 1.1. Zip-Slip Defense
- **Rule**: Never trust archive entry paths blindly.
- **Enforcement**: When extracting zip entries, always check `file.enclosed_name()`. If `enclosed_name()` returns `None` (indicating directory traversal like `../../`), the entry must be discarded immediately.
- **Reference**: `bepinex::extract_zip_to_directory`.

### 1.2. Cryptographic Verification
- **Rule**: All downloaded mod artifacts with an advertised `hash` (e.g. `sha256:...`) must be verified against their SHA256 digest before extraction.
- **Enforcement**: Extraction must abort immediately if a hash mismatch occurs, unless the user explicitly passes `--ignore-hash`.

### 1.3. State Isolation & Metadata
- **Rule**: Every installed mod folder must contain a valid `meta.json` recording its mod ID and artifact version.
- **Enforcement**: Mod removal or update must never corrupt adjacent plugin directories or root game assets.

---

## 2. Dependency Minimization (Ponytail Standard)

- **Rule**: Do not introduce external crates for operations that standard library or simple string routines can achieve cleanly.
- **Checklist**:
  - Do NOT reintroduce `walkdir` (use `std::fs::read_dir`).
  - Do NOT reintroduce `regex` (use `str::split`, `strip_prefix`, or standard string search).
  - Do NOT introduce redundant error crates (use `anyhow` consistently).
  - Keep `clap` features trimmed to `["derive"]` only.

---

## 3. Cross-Platform Portability

- **Rule**: Every feature in NOMGET must remain compatible with both Windows and Linux.
- **Enforcement**:
  - Use `Path` and `PathBuf` for all filesystem operations; never concatenate raw slash strings.
  - Windows-specific platform integrations (such as registry lookups via `winreg`) must be strictly guarded behind `#[cfg(windows)]`.
  - Linux-specific platform integrations (such as file permission bits via `std::os::unix::fs::PermissionsExt`) must be strictly guarded behind `#[cfg(unix)]`.

---

## 4. Idempotency & Failure Recovery

- **Rule**: Re-running any operation (such as `bepinex install`, `mod install`, or `target launch`) must be safe and idempotent.
- **Enforcement**:
  - If a mod is already installed at the requested version, NOMGET must detect it via `meta.json` and exit cleanly without redundant re-downloads.
  - If an installation fails mid-flight, temporary files must be cleaned up, and previous working state preserved where possible.
  - Cycle detection in dependency resolution must fail fast *before* downloading or touching the target filesystem.

---

## 5. Logging & Terminal Output

- **Rule**: Follow the established verbosity tiering:
  - `Error`: Fatal failures preventing completion.
  - `Warn`: Non-fatal anomalies (e.g. hash mismatch with `--ignore-hash`, incompatibility flags).
  - `Info`: Standard operational milestones (e.g., manifest fetch, dependency resolution, installation status).
  - `Debug`: Granular details for troubleshooting.
- **Enforcement**: Use `log::info!`, `log::warn!`, and `log::error!` for internal logging; use `println!` strictly for user-facing CLI command outputs and tables.
