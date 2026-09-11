use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use nomget::bepinex::install_bepinex;
use nomget::installer::ModInstaller;
use nomget::logger::init_logger;
use nomget::manifest::Manifest;
use nomget::target::TargetInstallation;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "nomget",
    author = "KopterBuzz",
    version = "0.1.0",
    about = "NOMGET: Nuclear Option Mod Manager CLI and Package Manager",
    long_about = "NOMGET is a terminal tool to query the NOMNOM Manifest, manage BepInEx 5, and resolve/install Nuclear Option mods."
)]
struct Cli {
    #[arg(short, long, global = true, action = clap::ArgAction::Count, help = "Increase logging verbosity (-v, -vv, -vvv)")]
    verbose: u8,

    #[arg(short, long, global = true, help = "Quiet mode (errors only)")]
    quiet: bool,

    #[arg(short, long, global = true, help = "Path to Nuclear Option or Dedicated Server directory")]
    target: Option<PathBuf>,

    #[arg(short, long, global = true, help = "Custom manifest URL or local path")]
    manifest: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Query the NOMNOM manifest")]
    Manifest {
        #[command(subcommand)]
        sub: ManifestCommands,
    },

    #[command(about = "Target installation discovery and management")]
    Target {
        #[command(subcommand)]
        sub: TargetCommands,
    },

    #[command(about = "Install and configure BepInEx 5")]
    Bepinex {
        #[command(subcommand)]
        sub: BepinexCommands,
    },

    #[command(about = "Install, update, and manage mods")]
    Mod {
        #[command(subcommand)]
        sub: ModCommands,
    },
}

#[derive(Subcommand)]
enum ManifestCommands {
    #[command(about = "List mods from the manifest")]
    List {
        #[arg(short, long, help = "Filter for server-compatible mods only")]
        server_only: bool,
    },
    #[command(about = "Search mods by keyword in id, name, description, or tags")]
    Search {
        #[arg(help = "Keyword to search for")]
        query: String,
    },
    #[command(about = "Display detailed information about a specific mod")]
    Info {
        #[arg(help = "Mod ID or display name")]
        mod_id: String,
    },
}

#[derive(Subcommand)]
enum TargetCommands {
    #[command(about = "Find default Nuclear Option installation from local Steam library")]
    Find,
    #[command(about = "Show information about the targeted installation")]
    Info,
    #[command(about = "Launch the target game or dedicated server with BepInEx")]
    Launch {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, help = "Extra arguments passed to the executable")]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
enum BepinexCommands {
    #[command(about = "Install BepInEx 5 to the targeted installation")]
    Install,
}

#[derive(Subcommand)]
enum ModCommands {
    #[command(about = "Download and install a mod and resolve all dependencies")]
    Install(ModInstallArgs),

    #[command(about = "Update an installed mod to the latest version")]
    Update {
        #[arg(help = "Mod ID to update")]
        mod_id: String,
    },

    #[command(about = "List installed mods in the target installation")]
    ListInstalled,
}

#[derive(Args)]
struct ModInstallArgs {
    #[arg(help = "Mod ID to install")]
    mod_id: String,

    #[arg(short, long, help = "Specific version to install (defaults to latest release)")]
    version: Option<String>,

    #[arg(long, help = "Ignore SHA256 checksum mismatch")]
    ignore_hash: bool,
}

fn resolve_target(cli_target: Option<&PathBuf>) -> Result<TargetInstallation> {
    if let Some(path) = cli_target {
        TargetInstallation::from_path(path)
    } else {
        log::info!("No target specified, searching local Steam library...");
        TargetInstallation::find_default().context(
            "Could not find default installation. Please provide --target <PATH> to specify the game or server folder.",
        )
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logger(cli.verbose, cli.quiet);

    let manifest_source = cli.manifest.as_deref();

    match cli.command {
        Commands::Manifest { sub } => match sub {
            ManifestCommands::List { server_only } => {
                let manifest = Manifest::fetch(manifest_source)?;
                let mut mods = manifest.list_all();
                if server_only {
                    mods.retain(|m| {
                        m.is_client_or_server
                            .as_deref()
                            .map(|s| s.eq_ignore_ascii_case("Server") || s.eq_ignore_ascii_case("Both"))
                            .unwrap_or(false)
                    });
                }
                println!("Found {} mod(s) in manifest:\n", mods.len());
                for m in mods {
                    let side = m.is_client_or_server.as_deref().unwrap_or("Client");
                    println!("{:<30} [{}] {}", m.id, side, m.display_name);
                }
            }
            ManifestCommands::Search { query } => {
                let manifest = Manifest::fetch(manifest_source)?;
                let results = manifest.search(&query);
                println!("Search results for '{}' ({} found):\n", query, results.len());
                for m in results {
                    let side = m.is_client_or_server.as_deref().unwrap_or("Client");
                    println!("{:<30} [{}] {}", m.id, side, m.display_name);
                    println!("  Description: {}", m.description);
                    if !m.tags.is_empty() {
                        println!("  Tags: {}", m.tags.join(", "));
                    }
                    println!();
                }
            }
            ManifestCommands::Info { mod_id } => {
                let manifest = Manifest::fetch(manifest_source)?;
                let m = manifest
                    .get_mod(&mod_id)
                    .ok_or_else(|| anyhow::anyhow!("Mod '{}' not found in manifest", mod_id))?;

                println!("ID:           {}", m.id);
                println!("Display Name: {}", m.display_name);
                println!("Description:  {}", m.description);
                println!("Authors:      {}", m.authors.join(", "));
                println!("Target:       {}", m.is_client_or_server.as_deref().unwrap_or("Client"));
                if !m.tags.is_empty() {
                    println!("Tags:         {}", m.tags.join(", "));
                }
                println!("\nArtifacts ({}):", m.artifacts.len());
                for art in &m.artifacts {
                    println!(
                        "  - Version: {} (Game: {}, Category: {})",
                        art.version,
                        art.game_version.as_deref().unwrap_or("any"),
                        art.category.as_deref().unwrap_or("release")
                    );
                    println!("    URL:  {}", art.download_url);
                    if let Some(hash) = &art.hash {
                        println!("    Hash: {}", hash);
                    }
                    if !art.dependencies.is_empty() {
                        println!(
                            "    Dependencies: {}",
                            art.dependencies
                                .iter()
                                .map(|d| format!("{} (min {})", d.id, d.version.as_deref().unwrap_or("any")))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                    if let Some(ext) = &art.extends {
                        println!("    Extends: {} ({:?})", ext.id, ext.version);
                    }
                }
            }
        },

        Commands::Target { sub } => match sub {
            TargetCommands::Find => {
                let target = TargetInstallation::find_default()?;
                println!("Found default installation:");
                println!("  Root:       {}", target.root_path.display());
                println!("  Type:       {}", target.kind);
                println!("  Executable: {}", target.executable_name);
                println!("  BepInEx:    {}", if target.is_bepinex_installed() { "Installed" } else { "Not installed" });
            }
            TargetCommands::Info => {
                let target = resolve_target(cli.target.as_ref())?;
                println!("Target Installation Details:");
                println!("  Root:       {}", target.root_path.display());
                println!("  Type:       {}", target.kind);
                println!("  Executable: {}", target.executable_name);
                println!("  Native:     {}", target.is_linux_native);
                println!("  BepInEx:    {}", if target.is_bepinex_installed() { "Installed" } else { "Not installed" });
                println!("  Plugins:    {}", target.plugins_dir().display());
            }
            TargetCommands::Launch { args } => {
                let target = resolve_target(cli.target.as_ref())?;
                println!("Launching {}...", target.kind);
                let mut child = target.launch(&args)?;
                child.wait()?;
            }
        },

        Commands::Bepinex { sub } => match sub {
            BepinexCommands::Install => {
                let target = resolve_target(cli.target.as_ref())?;
                println!("Installing BepInEx 5 to {}...", target.root_path.display());
                install_bepinex(&target)?;
                println!("BepInEx 5 installed successfully!");
            }
        },

        Commands::Mod { sub } => match sub {
            ModCommands::Install(args) => {
                let target = resolve_target(cli.target.as_ref())?;
                let manifest = Manifest::fetch(manifest_source)?;
                let installer = ModInstaller::new(&target, &manifest)
                    .with_ignore_hash(args.ignore_hash);

                println!("Resolving dependencies and installing '{}'...", args.mod_id);
                let installed = installer.install(&args.mod_id, args.version.as_deref())?;

                if installed.is_empty() {
                    println!("Mod '{}' is already up to date.", args.mod_id);
                } else {
                    println!("\nSuccessfully installed:");
                    for item in installed {
                        println!("  - {}", item);
                    }
                }
            }
            ModCommands::Update { mod_id } => {
                let target = resolve_target(cli.target.as_ref())?;
                let manifest = Manifest::fetch(manifest_source)?;
                let installer = ModInstaller::new(&target, &manifest);

                if installer.update(&mod_id)? {
                    println!("Mod '{}' updated successfully!", mod_id);
                } else {
                    println!("Mod '{}' is already up to date.", mod_id);
                }
            }
            ModCommands::ListInstalled => {
                let target = resolve_target(cli.target.as_ref())?;
                let manifest = Manifest::fetch(manifest_source)?;
                let installer = ModInstaller::new(&target, &manifest);

                let installed = installer.list_installed()?;
                println!("Installed mods in {} ({}):\n", target.root_path.display(), installed.len());
                for meta in installed {
                    println!("{:<30} v{}", meta.id, meta.artifact.version);
                }
            }
        },
    }

    Ok(())
}
