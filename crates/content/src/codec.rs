//! Little-endian record encoding shared by every entry kind.
//!
//! A record is a sequence of fields written in a fixed order by its kind's `encode` and read back
//! in the same order by its `decode`; it is padded with zeros to a multiple of 8 bytes. Strings
//! live in the pack's string table and are referenced by `(offset, length)`; tables are written
//! inline with their sizes. The reader refuses anything the writer could not have produced:
//! overruns, unused bytes, non-zero padding, non-finite numbers, bad flags.

use std::collections::BTreeMap;

use crate::cpack::CpackError;
use crate::registry::ContentId;
use crate::table::{Curve, Grid};

/// Interns strings into the pack's string table, sharing identical ones.
#[derive(Default)]
pub(crate) struct Strings {
    pub bytes: Vec<u8>,
    seen: BTreeMap<String, (u32, u32)>,
}

impl Strings {
    pub fn intern(&mut self, s: &str) -> Result<(u32, u32), CpackError> {
        if let Some(&at) = self.seen.get(s) {
            return Ok(at);
        }
        let at = (
            u32::try_from(self.bytes.len()).map_err(|_| CpackError::TooLarge)?,
            u32::try_from(s.len()).map_err(|_| CpackError::TooLarge)?,
        );
        self.bytes.extend_from_slice(s.as_bytes());
        self.seen.insert(s.to_owned(), at);
        Ok(at)
    }
}

/// Builds one record.
pub(crate) struct Writer<'s> {
    pub buf: Vec<u8>,
    strings: &'s mut Strings,
}

impl<'s> Writer<'s> {
    pub fn new(strings: &'s mut Strings) -> Self {
        Self {
            buf: Vec::new(),
            strings,
        }
    }

    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub fn bool(&mut self, v: bool) {
        self.buf.push(u8::from(v));
    }

    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn f32(&mut self, v: f32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn f32s(&mut self, vs: &[f32]) {
        for &v in vs {
            self.f32(v);
        }
    }

    pub fn handle(&mut self, h: ContentId) {
        self.u32(h.0);
    }

    pub fn str(&mut self, s: &str) -> Result<(), CpackError> {
        let (off, len) = self.strings.intern(s)?;
        self.u32(off);
        self.u32(len);
        Ok(())
    }

    /// A count that must fit a byte.
    pub fn count(&mut self, n: usize) -> Result<(), CpackError> {
        self.u8(u8::try_from(n).map_err(|_| CpackError::TooLarge)?);
        Ok(())
    }

    pub fn curve(&mut self, c: &Curve) -> Result<(), CpackError> {
        c.validate()?;
        self.u16(u16::try_from(c.xs.len()).map_err(|_| CpackError::TooLarge)?);
        self.f32s(&c.xs);
        self.f32s(&c.ys);
        Ok(())
    }

    pub fn grid(&mut self, g: &Grid) -> Result<(), CpackError> {
        g.validate()?;
        self.u16(u16::try_from(g.xs.len()).map_err(|_| CpackError::TooLarge)?);
        self.u16(u16::try_from(g.ys.len()).map_err(|_| CpackError::TooLarge)?);
        self.f32s(&g.xs);
        self.f32s(&g.ys);
        self.f32s(&g.zs);
        Ok(())
    }

    /// The finished record, zero-padded to a multiple of 8 bytes.
    pub fn finish(mut self) -> Vec<u8> {
        let padded = (self.buf.len() + 7) & !7;
        self.buf.resize(padded, 0);
        self.buf
    }
}

/// Reads one record back.
pub(crate) struct Reader<'a> {
    rec: &'a [u8],
    at: usize,
    strings: &'a str,
}

impl<'a> Reader<'a> {
    pub fn new(rec: &'a [u8], strings: &'a str) -> Self {
        Self {
            rec,
            at: 0,
            strings,
        }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], CpackError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&e| e <= self.rec.len())
            .ok_or(CpackError::Record("a record ends before its last field"))?;
        let s = &self.rec[self.at..end];
        self.at = end;
        Ok(s)
    }

    pub fn u8(&mut self) -> Result<u8, CpackError> {
        Ok(self.take(1)?[0])
    }

    pub fn bool(&mut self) -> Result<bool, CpackError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(CpackError::Record("a flag is not 0 or 1")),
        }
    }

    pub fn u16(&mut self) -> Result<u16, CpackError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&mut self) -> Result<u32, CpackError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn f32(&mut self) -> Result<f32, CpackError> {
        let b = self.take(4)?;
        let v = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if v.is_finite() {
            Ok(v)
        } else {
            Err(CpackError::Record("a number is not finite"))
        }
    }

    pub fn vec3(&mut self) -> Result<[f32; 3], CpackError> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }

    fn f32s(&mut self, n: usize) -> Result<Vec<f32>, CpackError> {
        (0..n).map(|_| self.f32()).collect()
    }

    pub fn handle(&mut self) -> Result<ContentId, CpackError> {
        Ok(ContentId(self.u32()?))
    }

    pub fn str(&mut self) -> Result<String, CpackError> {
        let off = self.u32()? as usize;
        let len = self.u32()? as usize;
        off.checked_add(len)
            .and_then(|end| self.strings.get(off..end))
            .map(str::to_owned)
            .ok_or(CpackError::Record("a string is outside the string table"))
    }

    pub fn curve(&mut self) -> Result<Curve, CpackError> {
        let n = usize::from(self.u16()?);
        let c = Curve {
            xs: self.f32s(n)?,
            ys: self.f32s(n)?,
        };
        c.validate()?;
        Ok(c)
    }

    pub fn grid(&mut self) -> Result<Grid, CpackError> {
        let nx = usize::from(self.u16()?);
        let ny = usize::from(self.u16()?);
        let g = Grid {
            xs: self.f32s(nx)?,
            ys: self.f32s(ny)?,
            zs: self.f32s(nx * ny)?,
        };
        g.validate()?;
        Ok(g)
    }

    /// Checks that the record was consumed exactly: only zero padding to the next multiple of 8
    /// may remain, and the record is exactly that long.
    pub fn finish(self) -> Result<(), CpackError> {
        let padded = (self.at + 7) & !7;
        if self.rec.len() != padded {
            return Err(CpackError::Record(
                "a record has bytes its fields do not use",
            ));
        }
        if self.rec[self.at..].iter().any(|&b| b != 0) {
            return Err(CpackError::Record("record padding is not zero"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_round_trip_and_the_record_is_padded() {
        let mut strings = Strings::default();
        let mut w = Writer::new(&mut strings);
        w.u8(7);
        w.bool(true);
        w.u16(513);
        w.f32(1.5);
        w.str("hello").unwrap();
        w.curve(&Curve::from_points(&[(0.0, 1.0), (2.0, 3.0)]))
            .unwrap();
        let rec = w.finish();
        assert_eq!(rec.len() % 8, 0);
        let text = String::from_utf8(strings.bytes).unwrap();
        let mut r = Reader::new(&rec, &text);
        assert_eq!(r.u8().unwrap(), 7);
        assert!(r.bool().unwrap());
        assert_eq!(r.u16().unwrap(), 513);
        assert_eq!(r.f32().unwrap(), 1.5);
        assert_eq!(r.str().unwrap(), "hello");
        assert_eq!(r.curve().unwrap().ys, vec![1.0, 3.0]);
        r.finish().unwrap();
    }

    #[test]
    fn leftovers_overruns_and_bad_values_are_refused() {
        let rec = [1_u8, 0, 0, 0, 0, 0, 0, 0];
        let mut r = Reader::new(&rec, "");
        r.u8().unwrap();
        r.finish().unwrap();
        let mut r = Reader::new(&rec, "");
        assert!(r.u32().is_ok() && r.u32().is_ok());
        assert!(r.u8().is_err(), "overrun");
        // A byte used by nothing but not zero.
        let rec = [1_u8, 0, 0, 9, 0, 0, 0, 0];
        let mut r = Reader::new(&rec, "");
        r.u8().unwrap();
        assert!(r.finish().is_err());
        // Sixteen bytes for a one-byte record.
        let rec = [0_u8; 16];
        let mut r = Reader::new(&rec, "");
        r.u8().unwrap();
        assert!(r.finish().is_err());
        let mut r = Reader::new(&[2], "");
        assert!(r.bool().is_err());
        let nan = f32::NAN.to_le_bytes();
        assert!(Reader::new(&nan, "").f32().is_err());
    }
}
