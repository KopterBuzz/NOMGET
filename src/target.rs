use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Client,
    DedicatedServer,
}

impl std::fmt::Display for TargetKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TargetKind::Client => write!(f, "Nuclear Option Client"),
            TargetKind::DedicatedServer => write!(f, "Nuclear Option Dedicated Server"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TargetInstallation {
    pub root_path: PathBuf,
    pub kind: TargetKind,
    pub executable_name: String,
    pub is_linux_native: bool,
}

impl TargetInstallation {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let root = path.as_ref().canonicalize().unwrap_or_else(|_| path.as_ref().to_path_buf());
        if !root.is_dir() {
            bail!("Target path is not a directory: {}", root.display());
        }

        // Check for dedicated server
        let linux_server_bin = root.join("NuclearOptionServer.x86_64");
        let win_server_bin = root.join("NuclearOptionServer.exe");
        let run_server_sh = root.join("RunServer.sh");

        if linux_server_bin.is_file() || run_server_sh.is_file() {
            return Ok(Self {
                root_path: root,
                kind: TargetKind::DedicatedServer,
                executable_name: "NuclearOptionServer.x86_64".to_string(),
                is_linux_native: true,
            });
        }

        if win_server_bin.is_file() {
            return Ok(Self {
                root_path: root,
                kind: TargetKind::DedicatedServer,
                executable_name: "NuclearOptionServer.exe".to_string(),
                is_linux_native: false,
            });
        }

        // Check for client
        let linux_client_bin = root.join("NuclearOption.x86_64");
        let win_client_bin = root.join("NuclearOption.exe");

        if linux_client_bin.is_file() {
            return Ok(Self {
                root_path: root,
                kind: TargetKind::Client,
                executable_name: "NuclearOption.x86_64".to_string(),
                is_linux_native: true,
            });
        }

        if win_client_bin.is_file() {
            return Ok(Self {
                root_path: root,
                kind: TargetKind::Client,
                executable_name: "NuclearOption.exe".to_string(),
                is_linux_native: false,
            });
        }

        bail!(
            "Directory does not appear to be a Nuclear Option or Dedicated Server installation: {}",
            root.display()
        );
    }

    pub fn find_default() -> Result<Self> {
        let steam_path = find_steam_path().context("Could not locate Steam installation")?;
        log::info!("Discovered Steam path at: {}", steam_path.display());

        let library_folders = find_library_folders(&steam_path);
        log::info!("Discovered {} Steam library folders", library_folders.len());

        let candidates = [
            // Dedicated server candidates
            ("Nuclear Option Dedicated Server", TargetKind::DedicatedServer),
            ("NuclearOptionServer", TargetKind::DedicatedServer),
            // Client candidates
            ("Nuclear Option", TargetKind::Client),
            ("NuclearOption", TargetKind::Client),
        ];

        for lib in &library_folders {
            for (subfolder, _kind) in candidates {
                let candidate_path = lib.join("steamapps").join("common").join(subfolder);
                if candidate_path.is_dir() {
                    if let Ok(target) = Self::from_path(&candidate_path) {
                        return Ok(target);
                    }
                }
            }
        }

        bail!("Could not find Nuclear Option or Dedicated Server in any Steam library");
    }

    pub fn bepinex_dir(&self) -> PathBuf {
        self.root_path.join("BepInEx")
    }

    pub fn plugins_dir(&self) -> PathBuf {
        self.bepinex_dir().join("plugins")
    }

    pub fn disabled_plugins_dir(&self) -> PathBuf {
        self.bepinex_dir().join("disabledPlugins")
    }

    pub fn config_dir(&self) -> PathBuf {
        self.bepinex_dir().join("config")
    }

    pub fn is_bepinex_installed(&self) -> bool {
        let core_dll = self.bepinex_dir().join("core").join("BepInEx.dll");
        if !core_dll.is_file() {
            return false;
        }

        if self.is_linux_native {
            self.root_path.join("run_bepinex.sh").is_file()
        } else {
            self.root_path.join("winhttp.dll").is_file()
                || self.root_path.join("doorstop_config.ini").is_file()
        }
    }

    pub fn launch(&self, extra_args: &[String]) -> Result<std::process::Child> {
        let mut cmd = if self.is_linux_native {
            let script = if self.root_path.join("run_bepinex.sh").is_file() {
                self.root_path.join("run_bepinex.sh")
            } else if self.root_path.join("RunServer.sh").is_file() {
                self.root_path.join("RunServer.sh")
            } else {
                self.root_path.join(&self.executable_name)
            };
            let mut c = std::process::Command::new(script);
            let linux64 = self.root_path.join("linux64");
            let mut ld_path = linux64.to_string_lossy().to_string();
            if let Ok(existing) = std::env::var("LD_LIBRARY_PATH") {
                ld_path = format!("{}:{}", ld_path, existing);
            }
            c.env("LD_LIBRARY_PATH", ld_path);
            c
        } else {
            std::process::Command::new(self.root_path.join(&self.executable_name))
        };

        cmd.current_dir(&self.root_path);
        for arg in extra_args {
            cmd.arg(arg);
        }
        cmd.spawn().context("Failed to launch target application")
    }
}

pub fn find_steam_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        use winreg::enums::*;
        use winreg::RegKey;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(steam_key) = hkcu.open_subkey("Software\\Valve\\Steam") {
            if let Ok(path_str) = steam_key.get_value::<String, _>("SteamPath") {
                let path = PathBuf::from(path_str.replace('/', "\\"));
                if path.is_dir() {
                    return Some(path);
                }
            }
        }
    }

    #[cfg(unix)]
    {
        if let Ok(home) = std::env::var("HOME") {
            let candidates = [
                format!("{}/.local/share/Steam", home),
                format!("{}/.steam/steam", home),
                format!("{}/.var/app/com.valvesoftware.Steam/.local/share/Steam", home),
                format!("{}/snap/steam/common/.local/share/Steam", home),
            ];
            for cand in candidates {
                let p = PathBuf::from(cand);
                if p.is_dir() {
                    return Some(p);
                }
            }
        }
    }

    None
}

pub fn find_library_folders(steam_path: &Path) -> Vec<PathBuf> {
    let mut folders = vec![steam_path.to_path_buf()];

    let vdf_paths = [
        steam_path.join("steamapps").join("libraryfolders.vdf"),
        steam_path.join("config").join("libraryfolders.vdf"),
    ];

    for vdf_path in vdf_paths {
        if let Ok(content) = fs::read_to_string(&vdf_path) {
            let re = regex::Regex::new(r#""path"\s+"([^"]+)""#).unwrap();
            for cap in re.captures_iter(&content) {
                if let Some(m) = cap.get(1) {
                    let folder_str = m.as_str().replace(r#"\\"#, "/");
                    let p = PathBuf::from(folder_str);
                    if p.is_dir() && !folders.contains(&p) {
                        folders.push(p);
                    }
                }
            }
            break;
        }
    }

    folders
}
