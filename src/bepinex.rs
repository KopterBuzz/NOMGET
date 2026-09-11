use crate::target::TargetInstallation;
use anyhow::{bail, Context, Result};
use std::fs::{self, File};
use std::path::Path;

pub const BEPINEX_LINUX_X64_URL: &str =
    "https://github.com/BepInEx/BepInEx/releases/download/v5.4.23.4/BepInEx_linux_x64_5.4.23.4.zip";
pub const BEPINEX_WIN_X64_URL: &str =
    "https://github.com/BepInEx/BepInEx/releases/download/v5.4.23.4/BepInEx_win_x64_5.4.23.4.zip";

pub fn install_bepinex(target: &TargetInstallation) -> Result<()> {
    let url = if target.is_linux_native {
        BEPINEX_LINUX_X64_URL
    } else {
        BEPINEX_WIN_X64_URL
    };

    log::info!("Installing BepInEx 5 for {} from {}", target.kind, url);
    let client = reqwest::blocking::Client::builder()
        .user_agent("NOMGET/0.1.0")
        .build()?;

    let response = client
        .get(url)
        .send()
        .with_context(|| format!("Failed to download BepInEx from {}", url))?;

    if !response.status().is_success() {
        bail!("Failed to download BepInEx: HTTP {}", response.status());
    }

    let bytes = response.bytes()?;
    log::info!("Downloaded BepInEx archive ({} bytes)", bytes.len());

    // Extract archive into target directory
    extract_zip_to_directory(&bytes, &target.root_path)?;

    // Create plugins and config folders
    fs::create_dir_all(target.plugins_dir())?;
    fs::create_dir_all(target.config_dir())?;

    // Create default BepInEx.cfg
    let bepinex_cfg = target.config_dir().join("BepInEx.cfg");
    if !bepinex_cfg.exists() {
        log::info!("Writing default BepInEx.cfg");
        fs::write(
            &bepinex_cfg,
            "[Chainloader]\n\n## Disables displaying of manager object in scene.\n# Setting type: Boolean\n# Default value: false\nHideManagerGameObject = true\n",
        )?;
    }

    // Configure Linux run_bepinex.sh if native
    if target.is_linux_native {
        let run_sh = target.root_path.join("run_bepinex.sh");
        if run_sh.is_file() {
            log::info!("Configuring run_bepinex.sh with executable: {}", target.executable_name);
            let content = fs::read_to_string(&run_sh)?;
            let modified = content.replace(
                r#"executable_name="""#,
                &format!(r#"executable_name="{}""#, target.executable_name),
            );
            fs::write(&run_sh, modified)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = fs::Permissions::from_mode(0o755);
                let _ = fs::set_permissions(&run_sh, perms.clone());

                let exe_path = target.root_path.join(&target.executable_name);
                if exe_path.exists() {
                    let _ = fs::set_permissions(&exe_path, perms);
                }
            }
        }
    }

    let core_dll = target.bepinex_dir().join("core").join("BepInEx.dll");
    if !core_dll.is_file() {
        bail!(
            "BepInEx installation incomplete: {} not found",
            core_dll.display()
        );
    }

    log::info!("BepInEx 5 installed successfully to {}", target.root_path.display());
    Ok(())
}

pub fn extract_zip_to_directory(bytes: &[u8], destination: &Path) -> Result<()> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader).context("Failed to parse zip archive")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let enclosed_name = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue, // Discard unsafe paths
        };

        let out_path = destination.join(&enclosed_name);

        if file.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(p) = out_path.parent() {
                fs::create_dir_all(p)?;
            }
            let mut outfile = File::create(&out_path)
                .with_context(|| format!("Failed to create file {}", out_path.display()))?;
            std::io::copy(&mut file, &mut outfile)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    let _ = fs::set_permissions(&out_path, fs::Permissions::from_mode(mode));
                }
            }
        }
    }

    Ok(())
}
