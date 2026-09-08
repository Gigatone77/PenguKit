//! Little-endian binary reader over a byte slice.
//!
//! REDengine 4 files are little-endian throughout. This cursor-based reader
//! provides bounded, checked reads so parsers can fail with precise
//! `UnexpectedEof` errors instead of panicking.

use crate::error::{PenguError, Result};

/// A cursor over `&[u8]` providing checked little-endian reads.
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Current read position in bytes.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Remaining bytes.
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    /// Total length of the underlying data.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn need(&self, n: usize) -> Result<()> {
        if self.remaining() < n {
            return Err(PenguError::UnexpectedEof {
                needed: n,
                available: self.remaining(),
            });
        }
        Ok(())
    }

    /// Seek to an absolute position.
    pub fn seek(&mut self, pos: usize) -> Result<()> {
        if pos > self.data.len() {
            return Err(PenguError::UnexpectedEof {
                needed: pos,
                available: self.data.len(),
            });
        }
        self.pos = pos;
        Ok(())
    }

    /// Skip forward `n` bytes.
    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.need(n)?;
        self.pos += n;
        Ok(())
    }

    /// Peak next `n` bytes without advancing.
    pub fn peek(&self, n: usize) -> Result<&'a [u8]> {
        self.need(n)?;
        Ok(&self.data[self.pos..self.pos + n])
    }

    /// Read `n` raw bytes, advancing the cursor.
    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let out = self.peek(n)?;
        self.pos += n;
        Ok(out)
    }

    /// Read exactly `n` bytes into an owned buffer.
    pub fn read_vec(&mut self, n: usize) -> Result<Vec<u8>> {
        Ok(self.read_bytes(n)?.to_vec())
    }

    /// Read the rest of the slice.
    pub fn read_to_end(&mut self) -> &'a [u8] {
        let out = &self.data[self.pos..];
        self.pos = self.data.len();
        out
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        Ok(self.read_bytes(1)?[0])
    }

    pub fn read_i8(&mut self) -> Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn read_i16(&mut self) -> Result<i16> {
        Ok(self.read_u16()? as i16)
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_i32(&mut self) -> Result<i32> {
        Ok(self.read_u32()? as i32)
    }

    pub fn read_u64(&mut self) -> Result<u64> {
        let b = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    pub fn read_i64(&mut self) -> Result<i64> {
        Ok(self.read_u64()? as i64)
    }

    pub fn read_f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.read_u32()?))
    }

    pub fn read_f64(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.read_u64()?))
    }

    /// Read a null-terminated C string (ASCII/Latin-1 pass-through).
    pub fn read_cstring(&mut self) -> Result<&'a str> {
        let start = self.pos;
        let end = self
            .data
            .iter()
            .enumerate()
            .skip(start)
            .find(|(_, &b)| b == 0)
            .map(|(i, _)| i)
            .ok_or_else(|| PenguError::UnexpectedEof {
                needed: 1,
                available: self.remaining(),
            })?;
        let bytes = &self.data[start..end];
        self.pos = end + 1;
        Ok(std::str::from_utf8(bytes)?)
    }

    /// Read a length-prefixed string: `u16` length + bytes.
    pub fn read_len_string16(&mut self) -> Result<String> {
        let len = self.read_u16()? as usize;
        let bytes = self.read_bytes(len)?;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }

    /// Read a length-prefixed string: `u32` length + bytes.
    pub fn read_len_string32(&mut self) -> Result<String> {
        let len = self.read_u32()? as usize;
        let bytes = self.read_bytes(len)?;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian() {
        let mut r = Reader::new(&[0x4D, 0x00, 0x78, 0x56, 0x34, 0x12, 0x00, 0x00]);
        assert_eq!(r.read_u16().unwrap(), 0x004D);
        assert_eq!(r.read_u32().unwrap(), 0x12345678);
        assert_eq!(r.position(), 6);
    }

    #[test]
    fn eof_is_reported() {
        let mut r = Reader::new(&[1, 2, 3]);
        assert!(matches!(
            r.read_u32(),
            Err(PenguError::UnexpectedEof { .. })
        ));
    }

    #[test]
    fn cstring_stops_at_nul() {
        let mut r = Reader::new(b"hello\x00world\x00");
        assert_eq!(r.read_cstring().unwrap(), "hello");
        assert_eq!(r.read_cstring().unwrap(), "world");
    }

    #[test]
    fn seek_and_skip() {
        let mut r = Reader::new(&[0u8; 16]);
        r.seek(4).unwrap();
        r.skip(6).unwrap();
        assert_eq!(r.position(), 10);
    }
}