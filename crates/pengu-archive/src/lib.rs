//! RDAR (`.archive`) container format for REDengine 4 — Cyberpunk 2077.
//!
//! Verified byte layout (2026-09-08) against `basegame_1_engine.archive`
//! (v12 header) and the WolvenKit reference source — full notes in
//! `docs/PLAN.md`.
//!
//! On-disk layout (all little-endian):
//!
//! ```text
//! Header (40 bytes):
//!   magic   u32  = "RDAR" (0x52444152)
//!   version u32  = 12
//!   indexPosition u64  (ABSOLUTE file offset of the index)
//!   indexSize    u32
//!   debugPosition u64
//!   debugSize    u32
//!   filesize     u64
//! then u32 customDataLength at offset 40 (0 for game archives)
//!
//! Index (at indexPosition):
//!   tableOffset u32, tableSize u32, crc u64,
//!   nFileEntries u32, nFileSegments u32, nDependencies u32
//!   ... 56-byte FileEntries, then 16-byte FileSegments, then u64 deps
//! ```

use pengu_core::error::{PenguError, Result};
use std::path::{Path, PathBuf};

pub const HEADER_MAGIC_RDAR: u32 = 0x5241_4452; // LE u32 of on-disk bytes "RDAR"
pub const HEADER_SIZE: usize = 40;
pub const EXTENDED_SIZE: usize = 0xAC;
pub const ARCHIVE_VERSION: u32 = 12;

/// The archive header (40 bytes at offset 0).
#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub magic: u32,
    pub version: u32,
    /// Absolute file offset where the index (file table) starts.
    pub index_position: u64,
    /// Total size of the index section.
    pub index_size: u32,
    pub debug_position: u64,
    pub debug_size: u32,
    /// On-disk archive size.
    pub filesize: u64,
}

impl Header {
    pub const fn new() -> Self {
        Self {
            magic: HEADER_MAGIC_RDAR,
            version: ARCHIVE_VERSION,
            index_position: 0,
            index_size: 0,
            debug_position: 0,
            debug_size: 0,
            filesize: 0,
        }
    }

    pub fn read(data: &[u8]) -> Result<Self> {
        if data.len() < HEADER_SIZE {
            return Err(PenguError::UnexpectedEof {
                needed: HEADER_SIZE,
                available: data.len(),
            });
        }
        let mut r = pengu_core::io::Reader::new(data);
        let hdr = Self {
            magic: r.read_u32()?,
            version: r.read_u32()?,
            index_position: r.read_u64()?,
            index_size: r.read_u32()?,
            debug_position: r.read_u64()?,
            debug_size: r.read_u32()?,
            filesize: r.read_u64()?,
        };
        if hdr.magic != HEADER_MAGIC_RDAR {
            return Err(PenguError::Format(format!(
                "not an RDAR archive (magic 0x{:08x})",
                hdr.magic
            )));
        }
        Ok(hdr)
    }
}

/// One file descriptor inside the archive (56 bytes on disk).
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// FNV-1a 64 of the resource path (see pengu_core::crc::red4_path_hash).
    pub hash: u64,
    /// Windows FILETIME timestamp (100ns since 1601). Kept raw.
    pub timestamp: u64,
    pub num_inline_buffer_segments: u32,
    pub segments_start: u32,
    pub segments_end: u32,
    pub resource_dependencies_start: u32,
    pub resource_dependencies_end: u32,
    pub sha1: [u8; 20],
}

/// One decompression segment (16 bytes on disk).
#[derive(Debug, Clone, Copy)]
pub struct FileSegment {
    /// Absolute file offset of this segment's data.
    pub offset: u64,
    /// Compressed size in the archive (`== size` when stored raw).
    pub zsize: u32,
    /// Decompressed size.
    pub size: u32,
}

/// The loaded index (file table) of an archive.
#[derive(Debug, Default)]
pub struct Index {
    pub files: Vec<FileEntry>,
    pub segments: Vec<FileSegment>,
    pub dependencies: Vec<u64>,
    pub crc: u64,
}

/// A fully opened, mmap-backed archive.
pub struct Archive {
    path: PathBuf,
    mmap: memmap2::Mmap,
    pub header: Header,
}

mod common_read;
use common_read::read_block;

pub mod writer;

impl Archive {
    /// Open an archive file read-only. Maps it into memory (large files are
    /// fine — pages are faulted on demand).
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = std::fs::File::open(&path)
            .map_err(|e| PenguError::Io(std::io::Error::new(e.kind(), format!("{path:?}: {e}"))))?;
        // SAFETY: read-only mapping, archive is immutable by convention.
        let mmap = unsafe { memmap2::Mmap::map(&file) }?;
        if mmap.len() < HEADER_SIZE {
            return Err(PenguError::Format("archive too small for header".into()));
        }
        let header = Header::read(&mmap[..HEADER_SIZE])?;
        if header.version != ARCHIVE_VERSION {
            return Err(PenguError::Unsupported(format!(
                "archive version {} (expected {})",
                header.version, ARCHIVE_VERSION
            )));
        }
        Ok(Self {
            path,
            mmap,
            header,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file_name(&self) -> Option<&str> {
        self.path.file_name().and_then(|s| s.to_str())
    }

    /// Read the index (file table). Always parses from the mapped bytes.
    pub fn read_index(&self) -> Result<Index> {
        let idx = usize::try_from(self.header.index_position).map_err(|_| {
            PenguError::Format("index position out of range".to_string())
        })?;
        let end = idx
            .checked_add(self.header.index_size as usize)
            .ok_or_else(|| PenguError::Format("index size overflow".to_string()))?;
        if end > self.mmap.len() {
            return Err(PenguError::UnexpectedEof {
                needed: end,
                available: self.mmap.len(),
            });
        }
        let data = &self.mmap[idx..end];
        let mut r = pengu_core::io::Reader::new(data);
        let _table_offset = r.read_u32()?;
        let _table_size = r.read_u32()?;
        let crc = r.read_u64()?;
        let n_files = r.read_u32()? as usize;
        let n_segments = r.read_u32()? as usize;
        let n_deps = r.read_u32()? as usize;

        let mut files = Vec::with_capacity(n_files);
        for _ in 0..n_files {
            files.push(FileEntry {
                hash: r.read_u64()?,
                timestamp: r.read_u64()?,
                num_inline_buffer_segments: r.read_u32()?,
                segments_start: r.read_u32()?,
                segments_end: r.read_u32()?,
                resource_dependencies_start: r.read_u32()?,
                resource_dependencies_end: r.read_u32()?,
                sha1: r.read_bytes(20)?.try_into().unwrap(),
            });
        }

        let mut segments = Vec::with_capacity(n_segments);
        for _ in 0..n_segments {
            segments.push(FileSegment {
                offset: r.read_u64()?,
                zsize: r.read_u32()?,
                size: r.read_u32()?,
            });
        }

        let mut dependencies = Vec::with_capacity(n_deps);
        for _ in 0..n_deps {
            dependencies.push(r.read_u64()?);
        }

        Ok(Index {
            files,
            segments,
            dependencies,
            crc,
        })
    }

    fn segment_slice(&self, seg: &FileSegment) -> Result<&[u8]> {
        let start = usize::try_from(seg.offset)
            .map_err(|_| PenguError::Format("segment offset out of range".into()))?;
        let end = start + seg.zsize as usize;
        if end > self.mmap.len() {
            return Err(PenguError::UnexpectedEof {
                needed: end,
                available: self.mmap.len(),
            });
        }
        Ok(&self.mmap[start..end])
    }

    /// Decompress one segment into its full (uncompressed) bytes.
    pub fn read_segment(&self, seg: &FileSegment) -> Result<Vec<u8>> {
        read_block(self.segment_slice(seg)?, seg)
    }

    /// Extract a full file by its path hash: concatenates its segments.
    pub fn extract_by_hash(&self, index: &Index, hash: u64) -> Result<Vec<u8>> {
        let entry = index
            .files
            .iter()
            .find(|f| f.hash == hash)
            .ok_or_else(|| PenguError::Argument(format!("no file with hash {hash:#018x}")))?;
        self.extract_entry(index, entry)
    }

    /// Extract a full file by its entry: concatenates its segments.
    pub fn extract_entry(&self, index: &Index, entry: &FileEntry) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        for i in entry.segments_start..entry.segments_end {
            let seg = index.segments[i as usize];
            out.extend_from_slice(&self.read_segment(&seg)?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ground-truth path on this machine. Tests use an env var so the crate
    // builds on any machine; the real corpus lives in ~/Games/Cyberpunk 2077.
    const ENV_ARCHIVE: &str = "PENGU_TEST_ARCHIVE";

    fn test_archive() -> Option<std::path::PathBuf> {
        std::env::var_os(ENV_ARCHIVE).map(PathBuf::from)
    }

    #[test]
    fn header_parse_from_static_bytes() {
        // 40-byte header for basegame_1_engine.archive
        let bytes = [
            0x52, 0x44, 0x41, 0x52, // RDAR
            0x0c, 0x00, 0x00, 0x00, // version 12
            0x00, 0xe0, 0xf4, 0x63, 0x00, 0x00, 0x00, 0x00, // index 0x63f4e000
            0xc4, 0x1c, 0x08, 0x00, // indexSize 0x081cc4
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // debugPos
            0x00, 0x00, 0x00, 0x00, // debugSize
            0x00, 0x00, 0xfd, 0x63, 0x00, 0x00, 0x00, 0x00, // filesize 0x63fd0000
        ];
        let hdr = Header::read(&bytes).unwrap();
        assert_eq!(hdr.version, 12);
        assert_eq!(hdr.index_position, 0x63f4e000);
        assert_eq!(hdr.index_size, 0x081cc4);
        assert_eq!(hdr.filesize, 0x63fd0000);
    }

    #[test]
    fn reject_bad_magic() {
        let mut bytes = vec![0u8; 40];
        bytes[0] = b'N';
        bytes[1] = b'O';
        bytes[2] = b'P';
        bytes[3] = b'E';
        assert!(Header::read(&bytes).is_err());
    }

    #[test]
    #[ignore = "requires PENGU_TEST_ARCHIVE pointing at a real .archive"]
    fn open_and_read_index() {
        let p = test_archive().expect("set PENGU_TEST_ARCHIVE");
        let ar = Archive::open(&p).unwrap();
        assert_eq!(ar.header.filesize, std::fs::metadata(&p).unwrap().len());
        let idx = ar.read_index().unwrap();
        assert!(!idx.files.is_empty());
        assert!(!idx.segments.is_empty());
    }

    #[test]
    #[ignore = "requires PENGU_TEST_ARCHIVE pointing at a real .archive"]
    fn extract_first_file_is_cr2w() {
        let p = test_archive().expect("set PENGU_TEST_ARCHIVE");
        let ar = Archive::open(&p).unwrap();
        let idx = ar.read_index().unwrap();
        let entry = &idx.files[0];
        let mut out = ar.extract_entry(&idx, entry).unwrap();
        assert_eq!(&out[..4], b"CR2W");
        out.clear();
    }

    #[test]
    fn writer_roundtrip_pack_unpack() {
        // Pack a few synthetic "files" (raw segments), reopen the bytes as an
        // archive and verify every hash extracts byte-identically, the segment
        // metadata is raw (zsize==size), and the index CRC re-verifies.
        let payloads = vec![
            (
                pengu_core::crc::red4_path_hash("base\\foo\\bar.mesh"),
                b"CR2Wmesh-bytes".to_vec(),
            ),
            (
                pengu_core::crc::red4_path_hash("cba.txt"),
                b"hello raw world".to_vec(),
            ),
            (0x1234_5678_9abc_def0, b"fixed-hash-file".to_vec()),
        ];
        let bytes = writer::pack_archive(
            payloads
                .iter()
                .map(|(h, d)| writer::PackEntry {
                    hash: *h,
                    data: d.clone(),
                })
                .collect(),
        )
        .unwrap();

        let tmp = std::env::temp_dir().join("pengu-roundtrip-test.archive");
        std::fs::write(&tmp, &bytes).unwrap();
        let ar = Archive::open(&tmp).unwrap();
        let idx = ar.read_index().unwrap();
        let _ = std::fs::remove_file(&tmp);

        assert_eq!(ar.header.filesize, bytes.len() as u64);
        assert_eq!(idx.files.len(), payloads.len());
        assert_eq!(idx.segments.len(), payloads.len());
        assert!(idx.dependencies.is_empty());

        // Verify the index CRC the same way an independent tool would.
        let idxpos = ar.header.index_position as usize;
        let table = &bytes[idxpos + 16..idxpos + ar.header.index_size as usize];
        assert_eq!(idx.crc, pengu_core::crc::crc64_xz(table));

        for (h, d) in &payloads {
            let out = ar.extract_by_hash(&idx, *h).unwrap();
            assert_eq!(&out, d, "hash {h:#018x}");
        }
        let seg = &idx.segments[0];
        assert_eq!(seg.zsize, seg.size, "raw segment expected");
    }

    #[test]
    #[ignore = "requires PENGU_TEST_ARCHIVE pointing at a real .archive"]
    fn golden_corpus_decodes_and_matches() {
        // Deterministic regression anchor: extracts a fixed set of real
        // basegame entries (from tests/corpus/golden.json) and recomputes
        // their SHA-256. A failure means either our reader regressed or the
        // game files changed (content differs from the frozen corpus).
        let corpus_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("corpus")
            .join("golden.json");
        let raw = std::fs::read_to_string(&corpus_path).unwrap();
        let pairs = parse_golden_corpus(&raw);
        assert!(!pairs.is_empty(), "golden corpus parse/sum failed");

        let p = test_archive().expect("set PENGU_TEST_ARCHIVE");
        let ar = Archive::open(&p).unwrap();
        let idx = ar.read_index().unwrap();
        let mut checked = 0;
        use sha2::{Digest, Sha256};
        for (hash, want_sha) in pairs {
            let out = ar.extract_by_hash(&idx, hash).unwrap();
            let mut h = Sha256::new();
            h.update(&out);
            let got: String = h
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            assert_eq!(got, want_sha, "hash {hash:#018x}");
            checked += 1;
        }
        println!("corpus: {checked} entries verified");
    }
}

/// Read `[{archive,path,hash:"0x…",sha256:"…"}…]` back out without a JSON
/// dependency. Returns (u64 hash, 64-char sha256 hex) pairs in file order.
#[allow(dead_code)] // used by the ignored real-archive test
fn parse_golden_corpus(raw: &str) -> Vec<(u64, String)> {
    let mut pairs = Vec::new();
    let mut cur_hash: Option<u64> = None;
    for line in raw.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("\"hash\": ") {
            let hx = rest.trim().strip_prefix("\"0x").map(|s| s.trim_end_matches([',', '"']));
            cur_hash = hx.and_then(|s| u64::from_str_radix(s, 16).ok());
        } else if let Some(rest) = line.strip_prefix("\"sha256\": ") {
            let v = rest.trim().trim_matches(['"', ',']);
            if let Some(h) = cur_hash.take() {
                if v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit()) {
                    pairs.push((h, v.to_string()));
                }
            }
        }
    }
    pairs
}