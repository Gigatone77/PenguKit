//! FFI to the bundled Kraken (RAD) compression native library.
//!
//! `libkraken.so` exposes the RAD simple API:
//! `int Kraken_Decompress(const u8* src, size_t src_len, u8* dst, size_t dst_len)`
//! returning the decompressed byte count (or < 0 on error).

use crate::error::{PenguError, Result};
use crate::natives;
use std::sync::OnceLock;

type KrakenDecompressFn =
    unsafe extern "C" fn(*const u8, usize, *mut u8, usize) -> i32;

/// A loaded handle to `libkraken.so`. Loaded lazily and kept alive for the
/// process lifetime so decompression calls never re-dlopen.
pub struct Kraken {
    _lib: libloading::Library,
    decompress: KrakenDecompressFn,
}

fn load() -> Result<&'static Kraken> {
    static KRAKEN: OnceLock<std::result::Result<Kraken, String>> = OnceLock::new();
    let k = KRAKEN.get_or_init(|| Kraken::init().map_err(|e| e.to_string()));
    k.as_ref()
        .map_err(|e| PenguError::Native(e.clone()))
}

impl Kraken {
    fn init() -> Result<Self> {
        let path = natives::find_native(natives::LIB_KRAKEN)?;
        let lib = unsafe { libloading::Library::new(&path) }
            .map_err(|e| PenguError::Native(format!("{path:?}: {e}")))?;
        let decompress = unsafe {
            let sym: libloading::Symbol<KrakenDecompressFn> =
                lib.get(b"Kraken_Decompress")
                    .map_err(|e| PenguError::Native(format!("Kraken_Decompress: {e}")))?;
            *sym
        };
        Ok(Kraken {
            _lib: lib,
            decompress,
        })
    }

    /// Decompress a Kraken block into `dst` (which must be exactly sized).
    /// Returns the number of bytes written.
    pub fn decompress_into(&self, src: &[u8], dst: &mut [u8]) -> Result<usize> {
        let ret = unsafe { (self.decompress)(src.as_ptr(), src.len(), dst.as_mut_ptr(), dst.len()) };
        if ret < 0 {
            return Err(PenguError::Format(format!("Kraken decompress failed: {ret}")));
        }
        Ok(ret as usize)
    }
}

/// Convenience: decompress a whole Kraken block into an exactly-sized buffer.
pub fn decompress(src: &[u8], dst: &mut [u8]) -> Result<usize> {
    load()?.decompress_into(src, dst)
}

/// Whether the Kraken native library is available.
pub fn available() -> bool {
    load().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_and_load_native() {
        // Serialize against the env-var mutation in natives tests.
        let _lock = crate::natives::tests::PENGU_NATIVE_TEST_LOCK.lock().unwrap();
        // The native may be unreachable in a bare `cargo test` (no
        // $PENGU_NATIVE_DIR); that is expected. Real loading is covered by
        // the pengu-archive integration tests with the env var set.
        if !available() {
            eprintln!("skipping: no libkraken.so discoverable");
            return;
        }
        assert!(available());
    }
}