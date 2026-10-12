//! The `.cpack` v1 container (GDD 62.2).
//!
//! ```text
//! 0    80 B   header   magic "FGCP", version, flags, entry count, section offsets, total length,
//!                      checksum (FNV-1a 64 of the whole file with the checksum field zeroed),
//!                      content hash (SHA-256 of every byte after the header)
//! 80   16 B   x n      table of contents, sorted by id; the index is the handle
//!                      u32 id_off, u32 id_len (into the strings), u32 kind, u32 record_off
//! ...         strings  UTF-8, ids and locale keys, no separators
//! ...         padding  zeros up to a multiple of 8
//! ...  144 B  x n      aircraft records, in table order
//! ```
//!
//! The encoding is canonical: one set of entries has exactly one byte string, and the reader
//! refuses anything else (offsets that do not follow from the counts, non-zero padding or
//! reserved bytes, unsorted ids). That is what lets the pack's hash stand for its meaning.
//!
//! The checksum catches damage; the content hash is the pack's identity (it goes into the state
//! hash and the multiplayer handshake). This crate has no hash library, so the writer takes the
//! SHA-256 function from its caller and the reader trusts the stored value, which the checksum
//! covers. The baker's tests recompute it.

use core::fmt;

use crate::aircraft::{AircraftDef, RECORD_BYTES};

/// File magic.
pub const MAGIC: [u8; 4] = *b"FGCP";
/// The format version this crate reads and writes.
pub const VERSION: u32 = 1;
/// Header size, bytes.
pub const HEADER_BYTES: usize = 80;
/// Table-of-contents entry size, bytes.
pub const TOC_ENTRY_BYTES: usize = 16;
/// `kind` of an aircraft entry.
pub const KIND_AIRCRAFT: u32 = 1;

/// Header field offsets.
pub mod header {
    /// `[u8; 4]` magic.
    pub const MAGIC: usize = 0;
    /// `u32` format version.
    pub const VERSION: usize = 4;
    /// `u32` flags, zero in version 1.
    pub const FLAGS: usize = 8;
    /// `u32` number of entries.
    pub const ENTRY_COUNT: usize = 12;
    /// `u32` offset of the table of contents.
    pub const TOC_OFF: usize = 16;
    /// `u32` offset of the string section.
    pub const STRINGS_OFF: usize = 20;
    /// `u32` length of the string section.
    pub const STRINGS_LEN: usize = 24;
    /// `u32` offset of the record section.
    pub const RECORDS_OFF: usize = 28;
    /// `u32` length of the record section.
    pub const RECORDS_LEN: usize = 32;
    /// `u32` total file length.
    pub const TOTAL_LEN: usize = 36;
    /// `u64` checksum.
    pub const CHECKSUM: usize = 40;
    /// `[u8; 32]` content hash.
    pub const CONTENT_HASH: usize = 48;
}

const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

/// Why a pack could not be read or written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CpackError {
    /// Shorter than its header or than the header says.
    Truncated,
    /// Not a `.cpack`.
    BadMagic,
    /// A version this build does not read.
    UnsupportedVersion(u32),
    /// The stored checksum does not match the bytes.
    Checksum {
        /// Value in the header.
        stored: u64,
        /// Value computed from the file.
        computed: u64,
    },
    /// A section offset, length or padding byte is not what the counts require.
    Layout(&'static str),
    /// The string section is not valid UTF-8.
    Utf8,
    /// Ids are not in strictly ascending order.
    UnsortedToc,
    /// A table-of-contents entry has a kind this version does not know.
    UnknownKind(u32),
    /// A record's content is invalid.
    Record(&'static str),
    /// Two entries share an id (writer).
    DuplicateId(String),
    /// A section does not fit in 32 bits (writer).
    TooLarge,
}

impl fmt::Display for CpackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => write!(f, "the pack is truncated"),
            Self::BadMagic => write!(f, "not a content pack (bad magic)"),
            Self::UnsupportedVersion(v) => {
                write!(
                    f,
                    "pack format version {v} is not supported (expected {VERSION})"
                )
            }
            Self::Checksum { stored, computed } => write!(
                f,
                "pack checksum mismatch (stored {stored:016x}, computed {computed:016x}); the file is damaged"
            ),
            Self::Layout(what) => write!(f, "malformed pack layout: {what}"),
            Self::Utf8 => write!(f, "the pack's string table is not valid UTF-8"),
            Self::UnsortedToc => write!(f, "pack entries are not sorted by id"),
            Self::UnknownKind(k) => write!(f, "unknown entry kind {k}"),
            Self::Record(what) => write!(f, "invalid record: {what}"),
            Self::DuplicateId(id) => write!(f, "duplicate id `{id}`"),
            Self::TooLarge => write!(f, "the pack is too large for the format"),
        }
    }
}

impl std::error::Error for CpackError {}

/// What a pack holds, in handle order.
#[derive(Debug)]
pub(crate) struct Parsed {
    pub ids: Vec<String>,
    pub aircraft: Vec<AircraftDef>,
    pub content_hash: [u8; 32],
}

/// FNV-1a 64 of `data` with the checksum field read as zero.
#[must_use]
pub fn checksum(data: &[u8]) -> u64 {
    let end = header::CHECKSUM + 8;
    let mut h = FNV_OFFSET;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(FNV_PRIME);
        }
    };
    feed(&data[..header::CHECKSUM.min(data.len())]);
    feed(&[0; 8]);
    if data.len() > end {
        feed(&data[end..]);
    }
    h
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

const fn align8(n: u64) -> u64 {
    (n + 7) & !7
}

/// Where the sections of a validated header sit.
struct Sections {
    entries: usize,
    toc_off: usize,
    strings_off: usize,
    strings_len: usize,
    records_off: usize,
}

/// Checks the header against the file: magic, version, length, checksum, and that the section
/// offsets are exactly the ones the counts require.
fn read_sections(data: &[u8]) -> Result<Sections, CpackError> {
    use header as h;
    if data.len() < HEADER_BYTES {
        return Err(CpackError::Truncated);
    }
    if data[h::MAGIC..h::MAGIC + 4] != MAGIC {
        return Err(CpackError::BadMagic);
    }
    let version = u32_at(data, h::VERSION);
    if version != VERSION {
        return Err(CpackError::UnsupportedVersion(version));
    }
    if u64::from(u32_at(data, h::TOTAL_LEN)) != data.len() as u64 {
        return Err(CpackError::Truncated);
    }
    let stored = u64::from_le_bytes(data[h::CHECKSUM..h::CHECKSUM + 8].try_into().unwrap());
    let computed = checksum(data);
    if stored != computed {
        return Err(CpackError::Checksum { stored, computed });
    }

    if u32_at(data, h::FLAGS) != 0 {
        return Err(CpackError::Layout("flags are not zero"));
    }
    let n = u64::from(u32_at(data, h::ENTRY_COUNT));
    let toc_off = u64::from(u32_at(data, h::TOC_OFF));
    let strings_off = u64::from(u32_at(data, h::STRINGS_OFF));
    let strings_len = u64::from(u32_at(data, h::STRINGS_LEN));
    let records_off = u64::from(u32_at(data, h::RECORDS_OFF));
    let records_len = u64::from(u32_at(data, h::RECORDS_LEN));
    if toc_off != HEADER_BYTES as u64 {
        return Err(CpackError::Layout(
            "the table of contents does not follow the header",
        ));
    }
    if strings_off != toc_off + n * TOC_ENTRY_BYTES as u64 {
        return Err(CpackError::Layout(
            "the strings do not follow the table of contents",
        ));
    }
    if records_off != align8(strings_off + strings_len) {
        return Err(CpackError::Layout(
            "the records are not at the aligned end of the strings",
        ));
    }
    if records_len != n * RECORD_BYTES as u64 {
        return Err(CpackError::Layout(
            "the record section does not hold one record per entry",
        ));
    }
    if records_off + records_len != data.len() as u64 {
        return Err(CpackError::Layout(
            "sections do not add up to the file length",
        ));
    }
    // Everything is inside the file now, so the narrowing casts cannot lose anything.
    Ok(Sections {
        entries: n as usize,
        toc_off: toc_off as usize,
        strings_off: strings_off as usize,
        strings_len: strings_len as usize,
        records_off: records_off as usize,
    })
}

/// Reads and fully validates a pack.
pub(crate) fn parse(data: &[u8]) -> Result<Parsed, CpackError> {
    let Sections {
        entries: n,
        toc_off,
        strings_off,
        strings_len,
        records_off,
    } = read_sections(data)?;

    if data[strings_off + strings_len..records_off]
        .iter()
        .any(|&b| b != 0)
    {
        return Err(CpackError::Layout("padding is not zero"));
    }
    let strings = core::str::from_utf8(&data[strings_off..strings_off + strings_len])
        .map_err(|_| CpackError::Utf8)?;

    let mut ids: Vec<String> = Vec::with_capacity(n);
    let mut aircraft = Vec::with_capacity(n);
    for i in 0..n {
        let e = toc_off + i * TOC_ENTRY_BYTES;
        let (id_off, id_len) = (u32_at(data, e) as usize, u32_at(data, e + 4) as usize);
        let kind = u32_at(data, e + 8);
        let record_off = u32_at(data, e + 12) as usize;
        if kind != KIND_AIRCRAFT {
            return Err(CpackError::UnknownKind(kind));
        }
        if record_off != i * RECORD_BYTES {
            return Err(CpackError::Layout("records are not in table order"));
        }
        let id = id_off
            .checked_add(id_len)
            .and_then(|end| strings.get(id_off..end))
            .filter(|s| !s.is_empty())
            .ok_or(CpackError::Layout(
                "an id is outside the string table or empty",
            ))?;
        if ids.last().is_some_and(|prev| prev.as_str() >= id) {
            return Err(CpackError::UnsortedToc);
        }
        let rec = &data[records_off + record_off..records_off + record_off + RECORD_BYTES];
        aircraft.push(AircraftDef::decode(rec, strings)?);
        ids.push(id.to_owned());
    }

    let mut content_hash = [0_u8; 32];
    content_hash.copy_from_slice(&data[header::CONTENT_HASH..header::CONTENT_HASH + 32]);
    Ok(Parsed {
        ids,
        aircraft,
        content_hash,
    })
}

/// Interns strings, sharing identical ones.
#[derive(Default)]
struct Strings {
    bytes: Vec<u8>,
    seen: std::collections::BTreeMap<String, (u32, u32)>,
}

impl Strings {
    fn intern(&mut self, s: &str) -> Result<(u32, u32), CpackError> {
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

/// Packs `entries` (in any order) into a `.cpack`. `sha256` hashes the bytes after the header;
/// its result is stored as the pack's content hash.
///
/// # Errors
/// [`CpackError::DuplicateId`] for repeated ids, [`CpackError::Record`] for an empty id, and
/// [`CpackError::TooLarge`] if a section would not fit in 32 bits.
pub fn write(
    entries: &[(String, AircraftDef)],
    sha256: impl FnOnce(&[u8]) -> [u8; 32],
) -> Result<Vec<u8>, CpackError> {
    let mut order: Vec<&(String, AircraftDef)> = entries.iter().collect();
    order.sort_by(|a, b| a.0.cmp(&b.0));
    if let Some(w) = order.windows(2).find(|w| w[0].0 == w[1].0) {
        return Err(CpackError::DuplicateId(w[0].0.clone()));
    }
    if order.iter().any(|e| e.0.is_empty()) {
        return Err(CpackError::Record("an id is empty"));
    }

    let mut strings = Strings::default();
    let mut toc = Vec::with_capacity(order.len() * TOC_ENTRY_BYTES);
    let mut records = Vec::with_capacity(order.len() * RECORD_BYTES);
    // Ids first (they come out sorted), then the locale keys.
    let id_at = order
        .iter()
        .map(|(id, _)| strings.intern(id))
        .collect::<Result<Vec<_>, _>>()?;
    for (i, (_, def)) in order.iter().enumerate() {
        let name_key = strings.intern(&def.name_key)?;
        let mut rec = [0_u8; RECORD_BYTES];
        def.encode(name_key, &mut rec);
        records.extend_from_slice(&rec);

        let record_off = u32::try_from(i * RECORD_BYTES).map_err(|_| CpackError::TooLarge)?;
        for v in [id_at[i].0, id_at[i].1, KIND_AIRCRAFT, record_off] {
            toc.extend_from_slice(&v.to_le_bytes());
        }
    }

    let strings_off = HEADER_BYTES + toc.len();
    let records_off = align8((strings_off + strings.bytes.len()) as u64) as usize;
    let total = records_off + records.len();
    let to_u32 = |n: usize| u32::try_from(n).map_err(|_| CpackError::TooLarge);

    let mut out = vec![0_u8; HEADER_BYTES];
    out[header::MAGIC..header::MAGIC + 4].copy_from_slice(&MAGIC);
    for (at, v) in [
        (header::VERSION, VERSION),
        (header::FLAGS, 0),
        (header::ENTRY_COUNT, to_u32(order.len())?),
        (header::TOC_OFF, to_u32(HEADER_BYTES)?),
        (header::STRINGS_OFF, to_u32(strings_off)?),
        (header::STRINGS_LEN, to_u32(strings.bytes.len())?),
        (header::RECORDS_OFF, to_u32(records_off)?),
        (header::RECORDS_LEN, to_u32(records.len())?),
        (header::TOTAL_LEN, to_u32(total)?),
    ] {
        out[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&toc);
    out.extend_from_slice(&strings.bytes);
    out.resize(records_off, 0);
    out.extend_from_slice(&records);

    let hash = sha256(&out[HEADER_BYTES..]);
    out[header::CONTENT_HASH..header::CONTENT_HASH + 32].copy_from_slice(&hash);
    let sum = checksum(&out);
    out[header::CHECKSUM..header::CHECKSUM + 8].copy_from_slice(&sum.to_le_bytes());
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::aircraft::tests::sample;
    use crate::aircraft::{Category, Era};

    /// A stand-in for SHA-256 with the same shape: deterministic and sensitive to every byte.
    pub(crate) fn fake_sha(bytes: &[u8]) -> [u8; 32] {
        let mut out = [0_u8; 32];
        for (i, chunk) in out.chunks_mut(8).enumerate() {
            let mut h = FNV_OFFSET ^ (i as u64);
            for &b in bytes {
                h ^= u64::from(b);
                h = h.wrapping_mul(FNV_PRIME);
            }
            chunk.copy_from_slice(&h.to_le_bytes());
        }
        out
    }

    pub(crate) fn entries() -> Vec<(String, AircraftDef)> {
        let mut second = sample();
        second.name_key = "ac.sparrow.name".into();
        second.category = Category::Uav;
        second.era = Era::G5;
        // Deliberately out of order, with a shared locale key.
        vec![
            ("core:ac/zulu".into(), sample()),
            ("core:ac/alpha".into(), second),
            ("core:ac/mike".into(), sample()),
        ]
    }

    pub(crate) fn pack() -> Vec<u8> {
        write(&entries(), fake_sha).unwrap()
    }

    /// Recomputes the checksum after a test edits the bytes, so the edit reaches the layout
    /// checks instead of stopping at the checksum.
    pub(crate) fn reseal(mut b: Vec<u8>) -> Vec<u8> {
        let sum = checksum(&b);
        b[header::CHECKSUM..header::CHECKSUM + 8].copy_from_slice(&sum.to_le_bytes());
        b
    }

    #[test]
    fn a_pack_round_trips_in_id_order() {
        let p = parse(&pack()).unwrap();
        assert_eq!(p.ids, ["core:ac/alpha", "core:ac/mike", "core:ac/zulu"]);
        assert_eq!(p.aircraft[0].name_key, "ac.sparrow.name");
        assert_eq!(p.aircraft[0].category, Category::Uav);
        assert_eq!(p.aircraft[1], sample());
        assert_eq!(p.aircraft[2], sample());
        assert_eq!(p.content_hash, fake_sha(&pack()[HEADER_BYTES..]));
    }

    #[test]
    fn the_output_is_byte_stable_and_independent_of_input_order() {
        let mut reversed = entries();
        reversed.reverse();
        assert_eq!(pack(), write(&reversed, fake_sha).unwrap());
        assert_eq!(pack(), pack());
    }

    #[test]
    fn identical_strings_are_stored_once() {
        let b = pack();
        let strings_len = u32_at(&b, header::STRINGS_LEN) as usize;
        let ids = "core:ac/alphacore:ac/mikecore:ac/zulu".len();
        let keys = "ac.sparrow.name".len() + "ac.kestrel.name".len();
        assert_eq!(strings_len, ids + keys);
    }

    #[test]
    fn the_layout_is_what_the_comment_says() {
        let b = pack();
        assert_eq!(&b[..4], b"FGCP");
        assert_eq!(u32_at(&b, header::VERSION), 1);
        assert_eq!(u32_at(&b, header::ENTRY_COUNT), 3);
        assert_eq!(u32_at(&b, header::TOC_OFF), 80);
        assert_eq!(u32_at(&b, header::STRINGS_OFF), 80 + 3 * 16);
        assert_eq!(u32_at(&b, header::RECORDS_OFF) % 8, 0);
        assert_eq!(u32_at(&b, header::RECORDS_LEN), 3 * 144);
        assert_eq!(u32_at(&b, header::TOTAL_LEN) as usize, b.len());
    }

    #[test]
    fn an_empty_pack_is_valid() {
        let b = write(&[], fake_sha).unwrap();
        let p = parse(&b).unwrap();
        assert!(p.ids.is_empty() && p.aircraft.is_empty());
        assert_eq!(b.len(), HEADER_BYTES);
    }

    #[test]
    fn flipping_any_single_byte_is_detected() {
        let good = pack();
        for i in 0..good.len() {
            let mut bad = good.clone();
            bad[i] ^= 0x01;
            assert!(
                parse(&bad).is_err(),
                "a flipped bit at byte {i} went unnoticed"
            );
        }
    }

    #[test]
    fn truncation_at_every_length_is_detected() {
        let good = pack();
        for len in 0..good.len() {
            assert!(parse(&good[..len]).is_err(), "length {len} was accepted");
        }
        let mut longer = good;
        longer.push(0);
        assert!(parse(&longer).is_err(), "trailing bytes were accepted");
    }

    #[test]
    fn bad_headers_are_named() {
        let good = pack();
        let mut b = good.clone();
        b[0] = b'X';
        assert_eq!(parse(&b).unwrap_err(), CpackError::BadMagic);
        let mut b = good.clone();
        b[header::VERSION] = 2;
        assert_eq!(parse(&b).unwrap_err(), CpackError::UnsupportedVersion(2));
        let mut b = good.clone();
        b[header::CONTENT_HASH] ^= 1;
        assert!(matches!(parse(&b), Err(CpackError::Checksum { .. })));
    }

    #[test]
    fn a_structurally_wrong_but_correctly_checksummed_pack_is_refused() {
        let good = pack();
        let toc = HEADER_BYTES;
        let strings_off = u32_at(&good, header::STRINGS_OFF) as usize;
        let strings_len = u32_at(&good, header::STRINGS_LEN) as usize;
        let records_off = u32_at(&good, header::RECORDS_OFF) as usize;

        let edit = |f: &dyn Fn(&mut Vec<u8>)| {
            let mut b = good.clone();
            f(&mut b);
            reseal(b)
        };
        let set32 =
            |b: &mut Vec<u8>, at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());

        // Swap the first two ids' string references: ids are no longer ascending.
        let unsorted = edit(&|b| {
            let (a, c) = (toc, toc + TOC_ENTRY_BYTES);
            for k in 0..8 {
                b.swap(a + k, c + k);
            }
        });
        assert_eq!(parse(&unsorted).unwrap_err(), CpackError::UnsortedToc);

        let wrong_kind = edit(&|b| set32(b, toc + 8, 7));
        assert_eq!(parse(&wrong_kind).unwrap_err(), CpackError::UnknownKind(7));

        let flags = edit(&|b| set32(b, header::FLAGS, 1));
        assert!(matches!(parse(&flags), Err(CpackError::Layout(_))));

        let shifted_record = edit(&|b| set32(b, toc + 12, 144 * 2));
        assert!(matches!(parse(&shifted_record), Err(CpackError::Layout(_))));

        let id_out_of_range = edit(&|b| set32(b, toc + 4, 10_000));
        assert!(matches!(
            parse(&id_out_of_range),
            Err(CpackError::Layout(_))
        ));

        let empty_id = edit(&|b| set32(b, toc + 4, 0));
        assert!(matches!(parse(&empty_id), Err(CpackError::Layout(_))));

        let bad_utf8 = edit(&|b| b[strings_off + 3] = 0xFF);
        assert_eq!(parse(&bad_utf8).unwrap_err(), CpackError::Utf8);

        let bad_pad = edit(&|b| {
            if records_off > strings_off + strings_len {
                b[strings_off + strings_len] = 1;
            }
        });
        if records_off > strings_off + strings_len {
            assert!(matches!(parse(&bad_pad), Err(CpackError::Layout(_))));
        }

        let wrong_records_off = edit(&|b| set32(b, header::RECORDS_OFF, records_off as u32 + 8));
        assert!(matches!(
            parse(&wrong_records_off),
            Err(CpackError::Layout(_))
        ));

        let bad_record = edit(&|b| b[records_off + crate::aircraft::field::CATEGORY] = 0);
        assert!(matches!(parse(&bad_record), Err(CpackError::Record(_))));
    }

    #[test]
    fn the_writer_refuses_duplicates_and_empty_ids() {
        let mut dup = entries();
        dup.push(("core:ac/mike".into(), sample()));
        assert_eq!(
            write(&dup, fake_sha).unwrap_err(),
            CpackError::DuplicateId("core:ac/mike".into())
        );
        let empty = vec![(String::new(), sample())];
        assert!(write(&empty, fake_sha).is_err());
    }
}
