//! Discovery of bundled native libraries (`libkraken.so`, `libwwtools.so`).
//!
//! PenguKit never installs system packages for its codecs. Bounded natives
//! are shipped inside the distribution (tar.gz, AppImage, Flatpak, Homebrew)
//! and located here in a deterministic order:
//!
//! 1. `$PENGU_NATIVE_DIR` (user override)
//! 2. next to the running executable
//! 3. `$XDG_DATA_HOME/pengu/natives` (or `~/.local/share/pengu/natives`)
//! 4. `/app/lib/pengu` (Flatpak layout)

use crate::error::{PenguError, Result};
use std::path::{Path, PathBuf};

const NATIVE_ENV: &str = "PENGU_NATIVE_DIR";

/// Names of the bundled natives on Linux x86_64.
pub const LIB_KRAKEN: &str = "libkraken.so";
pub const LIB_WWTOOLS: &str = "libwwtools.so";

fn is_native(dir: &Path, name: &str) -> Option<PathBuf> {
    let p = dir.join(name);
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

/// Locate a bundled native library by filename.
pub fn find_native(name: &str) -> Result<PathBuf> {
    // 1. explicit override
    if let Some(dir) = std::env::var_os(NATIVE_ENV) {
        if let Some(p) = is_native(Path::new(&dir), name) {
            return Ok(p);
        }
    }

    // 2. next to the executable (also covers AppImage mounts)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if let Some(p) = is_native(dir, name) {
                return Ok(p);
            }
        }
    }

    // 3. per-user share dir
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    if let Some(base) = data_home {
        for candidate in [base.join("pengu/natives"), base.join("PenguKit/natives")] {
            if let Some(p) = is_native(&candidate, name) {
                return Ok(p);
            }
        }
    }

    // 4. Flatpak layout
    if let Some(p) = is_native(Path::new("/app/lib/pengu"), name) {
        return Ok(p);
    }

    Err(PenguError::Native(format!(
        "{name} not found (looked in $PENGU_NATIVE_DIR, beside the executable, ~/.local/share/pengu/natives, /app/lib/pengu)"
    )))
}

/// Report which bundled natives are present (used by `pengu self-check`).
pub fn check_natives() -> Vec<(&'static str, std::result::Result<PathBuf, PenguError>)> {
    [LIB_KRAKEN, LIB_WWTOOLS]
        .into_iter()
        .map(|name| (name, find_native(name)))
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Serializes tests that read/write the process-global `PENGU_NATIVE_DIR`
    /// env var so they never race in parallel.
    pub(crate) static PENGU_NATIVE_TEST_LOCK: std::sync::Mutex<()> =
        std::sync::Mutex::new(());

    #[test]
    fn override_dir_is_honored() {
        let _guard = PENGU_NATIVE_TEST_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join("pengu-native-test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(LIB_KRAKEN), b"fake").unwrap();
        let prev = std::env::var_os(NATIVE_ENV);
        std::env::set_var(NATIVE_ENV, &dir);
        let found = find_native(LIB_KRAKEN).unwrap();
        assert!(found.is_file());
        match prev {
            Some(v) => std::env::set_var(NATIVE_ENV, v),
            None => std::env::remove_var(NATIVE_ENV),
        }
    }
}