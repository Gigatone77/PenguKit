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
    /// Unbundle (extract) a .archive
    Unbundle(UnbundleArgs),
    /// Pack a folder of REDengine files into an .archive [Phase 1]
    Pack(PackArgs),
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
struct UnbundleArgs {
    /// The .archive to extract
    archive: std::path::PathBuf,
    /// Output directory (default: <archive name>/ next to the archive)
    #[arg(short, long)]
    output: Option<std::path::PathBuf>,
    /// Only list files, don't extract; print "hash zsize size"
    #[arg(long)]
    list: bool,
}

#[derive(Args)]
struct PackArgs {
    /// Folder whose files become one archive; relative paths set entry hashes.
    input: std::path::PathBuf,
    /// Output .archive (default: <input>.archive next to the input folder)
    #[arg(short, long)]
    output: Option<std::path::PathBuf>,
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
        Command::Unbundle(a) => cmd_unbundle(a),
        Command::Pack(a) => cmd_pack(a),
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
    println!("fnv1a:      0x{:016x}", crc::fnv1a64_str(&text));
    println!("red4_hash:  0x{:016x}", crc::red4_path_hash(&text));
    println!("crc64_xz:   0x{:016x}", crc::crc64_str(&text));
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

fn cmd_unbundle(a: UnbundleArgs) -> pengu_core::Result<()> {
    use std::io::Write;

    let ar = pengu_archive::Archive::open(&a.archive)?;
    let idx = ar.read_index()?;

    if a.list {
        for f in &idx.files {
            let seg = &idx.segments[f.segments_start as usize];
            println!(
                "{:018} {:010} {:010} 0x{:016x}",
                seg.zsize, seg.size, 0, f.hash
            );
        }
        return Ok(());
    }

    let default_dir = a
        .archive
        .file_stem()
        .map(|s| a.archive.with_file_name(s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| a.archive.clone());
    let out_dir = a.output.unwrap_or(default_dir);
    std::fs::create_dir_all(&out_dir)?;

    let mut total = 0u64;
    for f in &idx.files {
        let data = ar.extract_entry(&idx, f)?;
        let stem = format!("{:016x}", f.hash);
        let name = format!("{stem}.bin");
        let p = out_dir.join(&name);
        let mut fh = std::fs::File::create(&p)?;
        fh.write_all(&data)?;
        total += data.len() as u64;
    }

    println!(
        "extracted {} files, {} MiB -> {}",
        idx.files.len(),
        total / (1024 * 1024),
        out_dir.display()
    );
    Ok(())
}

fn cmd_pack(a: PackArgs) -> pengu_core::Result<()> {
    use pengu_archive::writer::{pack_archive, PackEntry};

    if !a.input.is_dir() {
        return Err(PenguError::Argument(format!(
            "input is not a directory: {}",
            a.input.display()
        )));
    }

    let mut entries = Vec::new();
    let mut by_path_count = 0u32;
    for file in walkdir(&a.input)? {
        let rel = std::path::Path::strip_prefix(&file, &a.input)
            .map_err(|e| PenguError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let hash = numeric_hash(&rel_str).unwrap_or_else(|| {
            by_path_count += 1;
            crc::red4_path_hash(&rel_str)
        });
        let data = std::fs::read(&file)?;
        if data.len() > u32::MAX as usize {
            return Err(PenguError::Argument(format!(
                "file too large for RDAR (>4 GiB): {}",
                file.display()
            )));
        }
        entries.push(PackEntry { hash, data });
    }

    let out_path = a
        .output
        .unwrap_or_else(|| std::path::PathBuf::from(format!("{}.archive", a.input.display())));
    let n_files = entries.len();
    let bytes = pack_archive(entries)?;
    std::fs::write(&out_path, &bytes)?;

    println!("packed {n_files} files -> {}", out_path.display());
    if by_path_count > 0 {
        println!(
            "  {by_path_count} hashes derived from paths (FNV-1a of base\\...)",
        );
    }
    println!("  {} (index-padded)", human_size(bytes.len() as u64));
    Ok(())
}

fn human_size(n: u64) -> String {
    let units = ["B", "KiB", "MiB", "GiB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < units.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    format!("{v:.1} {}", units[u])
}

fn walkdir(root: &std::path::Path) -> pengu_core::Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for ent in std::fs::read_dir(&dir)? {
            let ent = ent?;
            if ent.file_type()?.is_dir() {
                stack.push(ent.path());
            } else {
                out.push(ent.path());
            }
        }
    }
    Ok(out)
}

/// Entry hash from a relative path, mirroring WolvenKit's `<digits>.` naming
/// convention plus our own 16-hex extraction format:
/// - `0123456789abcdef.bin` (16 hex chars) -> hex hash
/// - `15624399973311366.bin` (decimal) -> decimal hash
/// - anything else -> no hash in the name.
fn numeric_hash(rel: &str) -> Option<u64> {
    let stem = rel.split('.').next()?;
    if stem.len() == 16 && stem.bytes().all(|b| b.is_ascii_hexdigit()) {
        return u64::from_str_radix(stem, 16).ok();
    }
    stem.parse::<u64>().ok()
}

fn cmd_planned(cmd: Command) -> pengu_core::Result<()> {
    let name = match cmd {
        Command::Unbundle(_) => "unbundle (Phase 1: archive core)",
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