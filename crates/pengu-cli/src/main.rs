use clap::{Args, Parser, Subcommand};
use pengu_core::{crc, natives, PenguError};

#[derive(Parser)]
#[command(
    name = "pengu",
    version,
    about = "PenguKit - WolvenKit for Linux (CP2077 modding toolkit, Rust-native)",
    long_about = concat!(
        "PenguKit: a from-scratch, Rust-native, Windows-independent\n",
        "modding toolkit for Cyberpunk 2077 (REDengine 4).\n\n",
        "Every REDengine file is extractable and repackable losslessly.\n",
        "Full typed tooling exists for the asset pipeline: textures,\n",
        "meshes, TweakDB, audio and text. See 'pengu <command> --help'.\n"
    )
)]
struct Pengu {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show PenguKit version and build info
    Version,
    /// Hash a resource path the way REDengine 4 does
    Hash(HashArgs),
    /// Verify bundled native libraries and environment
    SelfCheck,
    /// Unbundle (extract) a .archive [Phase 1]
    Unbundle(PlannedArgs),
    /// Pack a folder of REDengine files into an .archive [Phase 1]
    Pack(PlannedArgs),
    /// Archive-level operations [Phase 1]
    Archive(PlannedArgs),
    /// Uncook textures/meshes from archives [Phase 3]
    Uncook(PlannedArgs),
    /// Import raw files back into REDengine format [Phase 3/4]
    Import(PlannedArgs),
    /// Export REDengine files to raw formats [Phase 3/4]
    Export(PlannedArgs),
    /// TweakDB YAML <-> .bin [Phase 5]
    Tweak(PlannedArgs),
    /// Wwise WEM <-> OGG [Phase 6]
    Wwise(PlannedArgs),
    /// Dump CR2W file structure [Phase 2]
    Info(PlannedArgs),
    /// Edit PenguKit settings [Phase 8]
    Settings(PlannedArgs),
}

#[derive(Args)]
struct HashArgs {
    /// Resource path to hash (e.g. "base/gameplay/audio/...")
    path: String,
    /// Hash the raw string without normalization
    #[arg(long)]
    raw: bool,
}

#[derive(Args)]
struct PlannedArgs {
    /// Command-line arguments (reserved for when this lands)
    #[arg(allow_hyphen_values = true)]
    args: Vec<String>,
}

fn main() {
    let p = Pengu::parse();
    let code = match run(p) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    };
    std::process::exit(code);
}

fn run(p: Pengu) -> pengu_core::Result<()> {
    match p.command {
        Command::Version => cmd_version(),
        Command::Hash(a) => cmd_hash(a),
        Command::SelfCheck => cmd_self_check(),
        other => cmd_planned(other),
    }
}

fn cmd_version() -> pengu_core::Result<()> {
    println!("PenguKit {}", pengu_core::PKG_VERSION);
    println!("platform: {}-{}", std::env::consts::OS, std::env::consts::ARCH);
    for (name, res) in natives::check_natives() {
        let status = match res {
            Ok(p) => p.display().to_string(),
            Err(_) => "MISSING".to_string(),
        };
        println!("native:   {name}: {status}");
    }
    Ok(())
}

fn cmd_hash(a: HashArgs) -> pengu_core::Result<()> {
    let text = if a.raw {
        a.path.clone()
    } else {
        crc::normalize_path(&a.path)
    };
    println!("input:      {text}");
    println!("crc64:      0x{:016x}", crc::crc64_str(&text));
    println!("red4_hash:  0x{:016x}", crc::red4_path_hash(&text));
    Ok(())
}

fn cmd_self_check() -> pengu_core::Result<()> {
    println!("PenguKit {} self-check", pengu_core::PKG_VERSION);
    println!("--------------------------------------------------");
    println!("platform:   {}-{}", std::env::consts::OS, std::env::consts::ARCH);
    let mut ok = true;
    for (name, res) in natives::check_natives() {
        match res {
            Ok(p) => println!("native:     {name} -> {}", p.display()),
            Err(e) => {
                ok = false;
                println!("native:     {name} -> {e}");
            }
        }
    }
    println!("result:     {}", if ok { "OK" } else { "MISSING NATIVES" });
    if !ok {
        return Err(PenguError::Native(
            "expected bundled natives are absent".into(),
        ));
    }
    Ok(())
}

fn cmd_planned(cmd: Command) -> pengu_core::Result<()> {
    let name = match cmd {
        Command::Unbundle(_) => "unbundle (Phase 1: archive core)",
        Command::Pack(_) => "pack (Phase 1: archive core)",
        Command::Archive(_) => "archive (Phase 1: archive core)",
        Command::Uncook(_) => "uncook (Phase 3: textures)",
        Command::Import(_) => "import (Phase 3/4: textures + meshes)",
        Command::Export(_) => "export (Phase 3/4: textures + meshes)",
        Command::Tweak(_) => "tweak (Phase 5: TweakDB)",
        Command::Wwise(_) => "wwise (Phase 6: audio)",
        Command::Info(_) => "info (Phase 2: CR2W container)",
        Command::Settings(_) => "settings (Phase 8: distribution)",
        _ => unreachable!(),
    };
    eprintln!("not yet implemented: {name}");
    Ok(())
}