//! Shared helpers for reading segment data out of an archive.
//!
//! A segment is either stored raw (`zsize == size`) or wrapped in a Kraken
//! block: `"KARK"` (LE of "KRAK") + u32 sourceSize + compressed payload of
//! `zsize - 8` bytes. `Kraken_Decompress(payload, zsize-8, dst, size)`
//! recovers the uncompressed bytes (verified against the real archive).

use pengu_core::error::{PenguError, Result};
use pengu_core::kraken;
use pengu_core::io::Reader;

use crate::FileSegment;

/// Magic of a Kraken-compressed block, in LE bytes as stored on disk.
const KRAK_MAGIC_LE: [u8; 4] = [0x4b, 0x41, 0x52, 0x4b]; // "KARK"

/// Output slack for `Kraken_Decompress`. The bundled RAD decoder reports
/// exactly `size` bytes written, but may write a few stray bytes past the
/// end of its final block (observed max 7 across all 14,147 compressed
/// segments of `basegame_1_engine.archive`). The archive `size` remains
/// authoritative, so we allocate a padded buffer and truncate.
const KRAKEN_DST_SLACK: usize = 256;

/// Read (and if needed decompress) a segment body.
pub fn read_block(data: &[u8], seg: &FileSegment) -> Result<Vec<u8>> {
    let seg_size = seg.size as usize;
    if seg.zsize == seg.size {
        // stored raw
        if data.len() != seg_size {
            return Err(PenguError::Format(format!(
                "raw segment size mismatch: stored {} expected {seg_size}",
                data.len()
            )));
        }
        return Ok(data.to_vec());
    }

    if data.len() < 8 {
        return Err(PenguError::Format("segment too short for Kraken header".into()));
    }

    let mut r = Reader::new(data);
    let magic = r.read_bytes(4)?;
    if magic != KRAK_MAGIC_LE {
        return Err(PenguError::Format(format!(
            "unexpected compression magic {magic:02x?}"
        )));
    }
    let source_size = r.read_u32()? as usize;
    if source_size != seg_size {
        return Err(PenguError::Format(format!(
            "Kraken block sourceSize {source_size} != segment size {seg_size}"
        )));
    }

    let payload = r.read_to_end();
    // Padded destination: `dst_len` must equal `size` exactly for the lib to
    // report success, while the wrap-around slack absorbs its end-of-block
    // overshoot (a Vec is allocated at `size + slack`, so the strays stay
    // inside the allocation and the allocator metadata stays intact).
    let mut out = vec![0u8; seg_size + KRAKEN_DST_SLACK];
    let got = kraken::decompress(payload, &mut out[..seg_size])?;
    if got != seg_size {
        return Err(PenguError::Format(format!(
            "Kraken produced {got} bytes, expected {seg_size}"
        )));
    }
    out.truncate(seg_size);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_segment_passthrough() {
        let seg = FileSegment {
            offset: 0,
            zsize: 4,
            size: 4,
        };
        let got = read_block(&[1, 2, 3, 4], &seg).unwrap();
        assert_eq!(got, vec![1, 2, 3, 4]);
    }

    #[test]
    fn rejects_unknown_magic() {
        let seg = FileSegment {
            offset: 0,
            zsize: 12,
            size: 4,
        };
        let data = [b'X', b'X', b'X', b'X', 4, 0, 0, 0, 0, 0, 0, 0];
        assert!(read_block(&data, &seg).is_err());
    }

    #[test]
    fn rejects_size_mismatch_in_header() {
        let seg = FileSegment {
            offset: 0,
            zsize: 12,
            size: 99,
        };
        let mut data = vec![0x4b, 0x41, 0x52, 0x4b];
        data.extend_from_slice(&(5u32).to_le_bytes());
        data.extend_from_slice(&[0, 0, 0, 0]);
        assert!(read_block(&data, &seg).is_err());
    }
}