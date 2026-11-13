//! The header JSON (ADR-54): UTF-8, keys sorted, no whitespace, integers only, so that rebuilding
//! from the same inputs gives the same bytes. A small writer and a small parser; no serde here.
//!
//! ```text
//! {"cell":[33,7],"datapack":"geo-2026.12","layers":["height","landcover"],"licences":["copernicus_dem"],
//!  "max_level":12,"min_level":6,"scheme":"geodetic_quadtree","sim_level":12,
//!  "sources":[{"files":[{"bytes":1,"name":"…","sha256":"…"}],"product":"glo30","tile_list_sha256":"…"}],
//!  "tile_posts":129}
//! ```

use std::collections::BTreeMap;
use std::fmt::Write as _;

use geo::gpk::GpkError;

/// One source file of an archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    /// The file name.
    pub name: String,
    /// Its SHA-256, lower-case hex.
    pub sha256: String,
    /// Its size, bytes.
    pub bytes: u64,
}

/// One product's sources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceSet {
    /// `glo30`, `glo90` or `synthetic`.
    pub product: String,
    /// The SHA-256 of the tile list the files were chosen from (empty for synthetic data).
    pub tile_list_sha256: String,
    /// The files, sorted by name.
    pub files: Vec<SourceFile>,
}

/// The header JSON's fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderJson {
    /// The datapack id (`geo-YYYY.MM`).
    pub datapack: String,
    /// The cell (z5 x, y), or `None` for the base pack.
    pub cell: Option<(u32, u32)>,
    /// The coarsest level.
    pub min_level: u8,
    /// The finest level.
    pub max_level: u8,
    /// The descriptor ids of the archive's sources.
    pub licences: Vec<String>,
    /// The sources.
    pub sources: Vec<SourceSet>,
}

/// The tiling scheme's name. // DATA: ADR-54
pub const SCHEME: &str = "geodetic_quadtree";
/// Posts per tile side. // DATA: ADR-54
pub const TILE_POSTS: u64 = 129;
/// The sim's level. // DATA: ADR-54
pub const SIM_LEVEL: u64 = 12;
/// The layers, in layer order. // DATA: ADR-54
pub const LAYERS: [&str; 2] = ["height", "landcover"];

/// A JSON value (the subset the header uses).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Json {
    /// `null`.
    Null,
    /// A non-negative integer.
    Int(u64),
    /// A string.
    Str(String),
    /// An array.
    Arr(Vec<Json>),
    /// An object; keys sorted by `BTreeMap`.
    Obj(BTreeMap<String, Json>),
}

impl Json {
    fn str(s: &str) -> Self {
        Self::Str(s.to_string())
    }

    /// The compact text, keys sorted.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        self.write(&mut s);
        s
    }

    fn write(&self, out: &mut String) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Int(n) => {
                let _ = write!(out, "{n}");
            }
            Self::Str(s) => write_str(out, s),
            Self::Arr(a) => {
                out.push('[');
                for (k, v) in a.iter().enumerate() {
                    if k > 0 {
                        out.push(',');
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Self::Obj(m) => {
                out.push('{');
                for (k, (key, v)) in m.iter().enumerate() {
                    if k > 0 {
                        out.push(',');
                    }
                    write_str(out, key);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A recursive-descent parser for [`Json`].
struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\n' | b'\r' | b'\t') {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> Result<(), GpkError> {
        self.ws();
        if self.b.get(self.i) == Some(&c) {
            self.i += 1;
            Ok(())
        } else {
            Err(GpkError::BadJson)
        }
    }

    fn value(&mut self) -> Result<Json, GpkError> {
        self.ws();
        match self.b.get(self.i) {
            Some(b'n') if self.b[self.i..].starts_with(b"null") => {
                self.i += 4;
                Ok(Json::Null)
            }
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b'[') => {
                self.i += 1;
                let mut a = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(Json::Arr(a));
                }
                loop {
                    a.push(self.value()?);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Json::Arr(a));
                        }
                        _ => return Err(GpkError::BadJson),
                    }
                }
            }
            Some(b'{') => {
                self.i += 1;
                let mut m = BTreeMap::new();
                self.ws();
                if self.b.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(Json::Obj(m));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.eat(b':')?;
                    let v = self.value()?;
                    m.insert(k, v);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Json::Obj(m));
                        }
                        _ => return Err(GpkError::BadJson),
                    }
                }
            }
            Some(c) if c.is_ascii_digit() => {
                let start = self.i;
                while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
                    self.i += 1;
                }
                let t =
                    core::str::from_utf8(&self.b[start..self.i]).map_err(|_| GpkError::BadJson)?;
                t.parse().map(Json::Int).map_err(|_| GpkError::BadJson)
            }
            _ => Err(GpkError::BadJson),
        }
    }

    fn string(&mut self) -> Result<String, GpkError> {
        if self.b.get(self.i) != Some(&b'"') {
            return Err(GpkError::BadJson);
        }
        self.i += 1;
        let mut out = Vec::new();
        while let Some(&c) = self.b.get(self.i) {
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(out).map_err(|_| GpkError::BadJson),
                b'\\' => {
                    let e = *self.b.get(self.i).ok_or(GpkError::BadJson)?;
                    self.i += 1;
                    match e {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'u' => {
                            let h = self.b.get(self.i..self.i + 4).ok_or(GpkError::BadJson)?;
                            let t = core::str::from_utf8(h).map_err(|_| GpkError::BadJson)?;
                            let cp = u32::from_str_radix(t, 16).map_err(|_| GpkError::BadJson)?;
                            let ch = char::from_u32(cp).ok_or(GpkError::BadJson)?;
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                            self.i += 4;
                        }
                        _ => return Err(GpkError::BadJson),
                    }
                }
                _ => out.push(c),
            }
        }
        Err(GpkError::BadJson)
    }
}

/// Parses `text`, which must hold exactly one value.
///
/// # Errors
/// [`GpkError::BadJson`].
pub fn parse(text: &str) -> Result<Json, GpkError> {
    let mut p = Parser {
        b: text.as_bytes(),
        i: 0,
    };
    let v = p.value()?;
    p.ws();
    if p.i == p.b.len() {
        Ok(v)
    } else {
        Err(GpkError::BadJson)
    }
}

fn obj(pairs: Vec<(&str, Json)>) -> Json {
    Json::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn get<'a>(m: &'a BTreeMap<String, Json>, k: &str) -> Result<&'a Json, GpkError> {
    m.get(k).ok_or(GpkError::BadJson)
}

fn as_int(j: &Json) -> Result<u64, GpkError> {
    if let Json::Int(n) = j {
        Ok(*n)
    } else {
        Err(GpkError::BadJson)
    }
}

fn as_str(j: &Json) -> Result<String, GpkError> {
    if let Json::Str(s) = j {
        Ok(s.clone())
    } else {
        Err(GpkError::BadJson)
    }
}

fn as_arr(j: &Json) -> Result<&[Json], GpkError> {
    if let Json::Arr(a) = j {
        Ok(a)
    } else {
        Err(GpkError::BadJson)
    }
}

fn as_obj(j: &Json) -> Result<&BTreeMap<String, Json>, GpkError> {
    if let Json::Obj(m) = j {
        Ok(m)
    } else {
        Err(GpkError::BadJson)
    }
}

fn level(j: &Json) -> Result<u8, GpkError> {
    u8::try_from(as_int(j)?).map_err(|_| GpkError::BadJson)
}

impl HeaderJson {
    /// The JSON value.
    #[must_use]
    pub fn to_json(&self) -> Json {
        let sources = self
            .sources
            .iter()
            .map(|s| {
                let files = s
                    .files
                    .iter()
                    .map(|f| {
                        obj(vec![
                            ("bytes", Json::Int(f.bytes)),
                            ("name", Json::str(&f.name)),
                            ("sha256", Json::str(&f.sha256)),
                        ])
                    })
                    .collect();
                obj(vec![
                    ("files", Json::Arr(files)),
                    ("product", Json::str(&s.product)),
                    ("tile_list_sha256", Json::str(&s.tile_list_sha256)),
                ])
            })
            .collect();
        obj(vec![
            (
                "cell",
                self.cell.map_or(Json::Null, |(x, y)| {
                    Json::Arr(vec![Json::Int(u64::from(x)), Json::Int(u64::from(y))])
                }),
            ),
            ("datapack", Json::str(&self.datapack)),
            (
                "layers",
                Json::Arr(LAYERS.iter().map(|l| Json::str(l)).collect()),
            ),
            (
                "licences",
                Json::Arr(self.licences.iter().map(|l| Json::str(l)).collect()),
            ),
            ("max_level", Json::Int(u64::from(self.max_level))),
            ("min_level", Json::Int(u64::from(self.min_level))),
            ("scheme", Json::str(SCHEME)),
            ("sim_level", Json::Int(SIM_LEVEL)),
            ("sources", Json::Arr(sources)),
            ("tile_posts", Json::Int(TILE_POSTS)),
        ])
    }

    /// The compact text.
    #[must_use]
    pub fn to_text(&self) -> String {
        self.to_json().to_text()
    }

    /// Parses and checks the header JSON (scheme, posts, layers).
    ///
    /// # Errors
    /// [`GpkError::BadJson`] for anything the writer would not write.
    pub fn parse(text: &str) -> Result<Self, GpkError> {
        let root = parse(text)?;
        let m = as_obj(&root)?;
        if as_str(get(m, "scheme")?)? != SCHEME || as_int(get(m, "tile_posts")?)? != TILE_POSTS {
            return Err(GpkError::BadJson);
        }
        let layers: Result<Vec<String>, _> =
            as_arr(get(m, "layers")?)?.iter().map(as_str).collect();
        if layers? != LAYERS {
            return Err(GpkError::BadJson);
        }
        let cell = match get(m, "cell")? {
            Json::Null => None,
            j => {
                let a = as_arr(j)?;
                if a.len() != 2 {
                    return Err(GpkError::BadJson);
                }
                let x = u32::try_from(as_int(&a[0])?).map_err(|_| GpkError::BadJson)?;
                let y = u32::try_from(as_int(&a[1])?).map_err(|_| GpkError::BadJson)?;
                Some((x, y))
            }
        };
        let mut sources = Vec::new();
        for s in as_arr(get(m, "sources")?)? {
            let s = as_obj(s)?;
            let mut files = Vec::new();
            for f in as_arr(get(s, "files")?)? {
                let f = as_obj(f)?;
                files.push(SourceFile {
                    name: as_str(get(f, "name")?)?,
                    sha256: as_str(get(f, "sha256")?)?,
                    bytes: as_int(get(f, "bytes")?)?,
                });
            }
            sources.push(SourceSet {
                product: as_str(get(s, "product")?)?,
                tile_list_sha256: as_str(get(s, "tile_list_sha256")?)?,
                files,
            });
        }
        Ok(Self {
            datapack: as_str(get(m, "datapack")?)?,
            cell,
            min_level: level(get(m, "min_level")?)?,
            max_level: level(get(m, "max_level")?)?,
            licences: as_arr(get(m, "licences")?)?
                .iter()
                .map(as_str)
                .collect::<Result<_, _>>()?,
            sources,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> HeaderJson {
        HeaderJson {
            datapack: "geo-2026.12".into(),
            cell: Some((33, 7)),
            min_level: 6,
            max_level: 12,
            licences: vec!["copernicus_dem".into()],
            sources: vec![SourceSet {
                product: "glo30".into(),
                tile_list_sha256: "ab".repeat(32),
                files: vec![SourceFile {
                    name: "Copernicus_DSM_COG_10_N46_00_E007_00_DEM.tif".into(),
                    sha256: "cd".repeat(32),
                    bytes: 41_776_750,
                }],
            }],
        }
    }

    #[test]
    fn the_header_round_trips_with_sorted_keys() {
        let h = header();
        let t = h.to_text();
        assert!(t.starts_with("{\"cell\":[33,7],\"datapack\":\"geo-2026.12\",\"layers\":[\"height\",\"landcover\"],"), "{t}");
        assert!(t.ends_with(",\"tile_posts\":129}"));
        assert!(!t.contains(' '));
        assert_eq!(HeaderJson::parse(&t), Ok(h));
        let base = HeaderJson {
            cell: None,
            ..header()
        };
        assert_eq!(HeaderJson::parse(&base.to_text()), Ok(base));
    }

    #[test]
    fn bad_json_is_refused() {
        assert_eq!(HeaderJson::parse("{}"), Err(GpkError::BadJson));
        assert_eq!(parse("[1,2"), Err(GpkError::BadJson));
        assert_eq!(parse("\"a\\u0041\""), Ok(Json::Str("aA".into())));
        let wrong = header().to_text().replace("geodetic_quadtree", "mercator");
        assert_eq!(HeaderJson::parse(&wrong), Err(GpkError::BadJson));
    }
}
