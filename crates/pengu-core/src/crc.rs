//! CRC-64 and REDengine 4 path hashing.

/// Bit-reversed representation of the CRC-64/ECMA-182 polynomial
/// `0x42F0E1EBA9EA3693`. REDengine path hashes use the reflected form.
const POLY_CRC64_REFLECTED: u64 = 0xC96C5795D7870F42;

/// Table-driven reflected CRC-64 over `data` with a custom init and final XOR.
///
/// The reflected polynomial is fixed (ECMA-182 reflected). Different CRC-64
/// variants differ only in init/xorout — and REDengine's path hash uses
/// `init = 0`, `xorout = 0`, the same as WolvenKit's `RedHash.Crc64`.
pub fn crc64_with(init: u64, xorout: u64, data: &[u8]) -> u64 {
    let mut table = [0u64; 256];
    for i in 0..256u64 {
        let mut crc = i;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ POLY_CRC64_REFLECTED
            } else {
                crc >> 1
            };
        }
        table[i as usize] = crc;
    }

    let mut crc: u64 = init;
    for &b in data {
        crc = (crc >> 8) ^ table[((crc ^ b as u64) & 0xFF) as usize];
    }
    crc ^ xorout
}

/// CRC-64 with reflected bits, initial value 0, no final XOR — the variant
/// REDengine uses for path hashing (matches WolvenKit's `Crc64`).
pub fn crc64(data: &[u8]) -> u64 {
    crc64_with(0, 0, data)
}

/// CRC-64 computed over a string, one byte per character (ASCII paths).
pub fn crc64_str(s: &str) -> u64 {
    crc64(s.as_bytes())
}

/// Normalize a resource path the way REDengine does before hashing:
/// leading/trailing slashes stripped, backslashes to forward slashes,
/// and the whole path lowercased.
pub fn normalize_path(s: &str) -> String {
    let mut p = s.replace('\\', "/");
    while p.starts_with('/') {
        p.remove(0);
    }
    while p.ends_with('/') {
        p.pop();
    }
    p.to_ascii_lowercase()
}

/// Compute the REDengine 4 path hash for a resource path.
///
/// Callers should pre-normalize with [`normalize_path`] if the input may
/// contain leading slashes, backslashes, or mixed case. The raw form hashes
/// the (ASCII) bytes verbatim, matching how the game's file table is built.
///
/// NOTE: the authoritative check against a real `.archive` file table happens
/// in Phase 1 (`pengu-archive`), which will lock in the confirmed behavior.
pub fn red4_path_hash(text: &str) -> u64 {
    crc64_str(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc64_empty_is_zero() {
        assert_eq!(crc64(b""), 0);
    }

    #[test]
    fn crc64_xz_catalog_vector() {
        // Independent check of the reflected poly/bit order against the
        // CRC-64/XZ catalog entry (same reflected poly, init/xor = all-ones):
        // CRC-64/XZ of "123456789" == 0x995DC9BBDF1939FA.
        const XZ_INIT_XOR: u64 = 0xFFFF_FFFF_FFFF_FFFF;
        assert_eq!(
            crc64_with(XZ_INIT_XOR, XZ_INIT_XOR, b"123456789"),
            0x995DC9BBDF1939FA
        );
        // REDengine = same poly, init 0, no xorout.
        // Cross-checked against an independent Python reflected-CRC oracle.
        assert_eq!(crc64(b"123456789"), 0x2B9C7EE4E2780C8A);
    }

    #[test]
    fn normalize_slashes_and_case() {
        assert_eq!(normalize_path("/Foo/Bar\\Baz/"), "foo/bar/baz");
    }

    #[test]
    fn normalize_strips_leading_trailing() {
        assert_eq!(normalize_path("//a/b//"), "a/b");
    }
}