//! RDAR archive writer (raw/uncompressed mode).
//!
//! Replicates WolvenKit's `ArchiveWriter` byte-for-byte for the raw case:
//! one uncompressed segment per file, no inline buffers, no custom data,
//! no dependencies. Entry hashes are the REDengine 4 path hash (FNV-1a 64
//! of the sanitized path); the index CRC is CRC-64/XZ over the table bytes.
//!
//! On-disk layout produced here (all little-endian):
//!
//! ```text
//! [0..40)       header (magic, version, indexPosition, indexSize,
//!               debugPosition=0, debugSize=0, filesize)
//! [40..44)      customDataLength u32 = 0
//! [44..172)     0xAC-40-4 bytes of zero padding
//! [172..)       segment data (each file, sorted by hash ascending)
//! <index>       indexPosition:
//!               [tableOffset u32=8][tableSize u32=table.len()+8]
//!               [crc u64][table bytes]
//! <page pad>    zero-padded to the next 4096-byte boundary
//! ```

use std::time::{SystemTime, UNIX_EPOCH};

use pengu_core::crc::crc64_xz;
use pengu_core::error::{PenguError, Result};
use sha1::{Digest, Sha1};

use crate::{ARCHIVE_VERSION, EXTENDED_SIZE, HEADER_MAGIC_RDAR, HEADER_SIZE};

/// One file to pack: its REDengine 4 path hash and full content bytes.
#[derive(Debug, Clone)]
pub struct PackEntry {
    /// FNV-1a 64 of the sanitized resource path (`crc::red4_path_hash`).
    pub hash: u64,
    /// Full, uncompressed file bytes.
    pub data: Vec<u8>,
}

fn filetime_now() -> u64 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Seconds between the UNIX epoch (1970) and the Windows FILETIME epoch
    // (1601), scaled to 100ns units.
    (secs + 11_644_473_600) * 10_000_000
}

struct Buf(Vec<u8>);

impl Buf {
    fn new() -> Self {
        Self(Vec::new())
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn bytes(&mut self, b: &[u8]) {
        self.0.extend_from_slice(b);
    }
}

/// Build a complete `.archive` in memory from the given files.
///
/// Entries are sorted by hash ascending and must have unique hashes
/// (duplicate hashes abort, matching WolvenKit's behavior). All segments
/// are stored raw (`zsize == size`); compression is a later phase.
pub fn pack_archive(entries: Vec<PackEntry>) -> Result<Vec<u8>> {
    let mut files: Vec<PackEntry> = entries;
    files.sort_by_key(|f| f.hash);
    if files.len() > u32::MAX as usize {
        return Err(PenguError::Argument(format!(
            "too many files ({}) for RDAR",
            files.len()
        )));
    }
    for w in files.windows(2) {
        if w[0].hash == w[1].hash {
            return Err(PenguError::Argument(format!(
                "duplicate hash 0x{:016x}",
                w[0].hash
            )));
        }
    }
    let n = files.len() as u32;
    let timestamp = filetime_now();

    // ---- prefix: header (patched later) + customDataLength + padding ----
    let mut out: Vec<u8> = Vec::new();
    out.resize(HEADER_SIZE, 0);
    out.extend_from_slice(&0u32.to_le_bytes()); // customDataLength = 0
    out.resize(EXTENDED_SIZE, 0); // remaining zero padding to 0xAC

    // ---- segment data ----
    let mut segments: Vec<(u64, u32, u32)> = Vec::with_capacity(files.len());
    let mut entries_table = Vec::with_capacity(files.len());
    for f in &files {
        let offset = out.len() as u64;
        out.extend_from_slice(&f.data);
        let size = f.data.len() as u32;
        segments.push((offset, size, size));

        // FileEntry (56 bytes): hash, FILETIME, no inline buffers, single
        // segment, no dependencies, SHA-1 of the full file bytes.
        let sha1_stream = Sha1::digest(&f.data);
        let digest: [u8; 20] = sha1_stream.into();
        let seg_index = (segments.len() - 1) as u32;
        let mut e = Buf::new();
        e.u64(f.hash);
        e.u64(timestamp);
        e.u32(0); // num_inline_buffer_segments
        e.u32(seg_index); // segments_start (inclusive)
        e.u32(seg_index + 1); // segments_end (exclusive, matches basegame)
        e.u32(0); // resource_dependencies_start
        e.u32(0); // resource_dependencies_end
        e.bytes(&digest);
        entries_table.push(e.0);
    }

    // ---- file table (counts + entries + segments) ----
    let mut table = Buf::new();
    table.u32(n);
    table.u32(n);
    table.u32(0); // dependencies
    for e in &entries_table {
        table.bytes(e);
    }
    for (offset, zsize, size) in &segments {
        table.u64(*offset);
        table.u32(*zsize);
        table.u32(*size);
    }
    let table_bytes = &table.0;
    let index_crc = crc64_xz(table_bytes);

    // ---- index section ----
    let index_position = out.len();
    let mut idx = Buf::new();
    idx.u32(8); // file_table_offset -- always 8
    idx.u32(table_bytes.len() as u32 + 8); // file_table_size
    idx.u64(index_crc);
    idx.bytes(table_bytes);
    out.extend_from_slice(&idx.0);
    let index_size = idx.0.len() as u32;

    // ---- pad the file to a 4096-byte boundary (matches CDPR/WolvenKit) ----
    while out.len() % 0x1000 != 0 {
        out.push(0);
    }
    let filesize = out.len() as u64;

    // ---- final header ----
    let mut hdr = Buf::new();
    hdr.u32(HEADER_MAGIC_RDAR);
    hdr.u32(ARCHIVE_VERSION);
    hdr.u64(index_position as u64);
    hdr.u32(index_size);
    hdr.u64(0); // debug_position
    hdr.u32(0); // debug_size
    hdr.u64(filesize);
    out[..HEADER_SIZE].copy_from_slice(&hdr.0);

    Ok(out)
}