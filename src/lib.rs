pub mod bepinex;
pub mod installer;
pub mod logger;
pub mod manifest;
pub mod resolver;
pub mod target;

pub use bepinex::install_bepinex;
pub use installer::{ModInstaller, ModMeta};
pub use logger::init_logger;
pub use manifest::{Artifact, Manifest, Mod};
pub use resolver::{ResolutionPlan, ResolvedItem};
pub use target::{find_steam_path, TargetInstallation, TargetKind};
