//! No hard-coded content in the simulation (`AC-12.4`): rule R2 of GDD 12's
//! `NoHardcodedContentLint`, the content-id check. The simulation finds aircraft by handle and
//! kind, never by id or name, so adding, renaming or removing content never needs a code change.
//!
//! Outside comments and `#[cfg(test)]` items, the sources of the simulation crates must not
//! contain
//! - a string literal holding something shaped like a content id (`pack:kind/name`), or
//! - the name of any content entry (the part of its id after the `/`) as a word, in any case:
//!   `KESTREL`, `kestrel_aero` and `"Kestrel"` all name `core:ac/kestrel`.
//!
//! The names come from the content packs themselves, so new content is covered without editing
//! this file. Integration tests (`tests/`), unit-test modules and comments may name content.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::util::workspace_root;

/// The crates that make up the simulation (GDD 61.7).
const SIM_CRATES: &[&str] = &[
    "simmath",
    "flight",
    "fcs",
    "flight-test",
    "sim-core",
    "sim-wasm",
];

/// Names shorter than this are not searched for: they would match ordinary words.
const MIN_NAME_LEN: usize = 3;

pub fn check() -> Result<(), String> {
    let root = workspace_root();
    let content = content_ids(&root.join("content"))?;
    if content.is_empty() {
        return Err("check-content-ids: no content ids found under content/".into());
    }
    let mut problems = Vec::new();
    let mut files = 0;
    for krate in SIM_CRATES {
        let mut sources = Vec::new();
        rust_files(&root.join("crates").join(krate).join("src"), &mut sources)?;
        let texts = sources
            .iter()
            .map(|p| fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display())))
            .collect::<Result<Vec<_>, _>>()?;
        // Test modules kept in their own files (`#[cfg(test)] mod tests;`) are tests too.
        let test_dirs: Vec<PathBuf> = sources
            .iter()
            .zip(&texts)
            .flat_map(|(path, text)| {
                let children = child_module_dir(path);
                lex(text)
                    .test_modules
                    .into_iter()
                    .map(move |m| children.join(m))
            })
            .collect();
        for (path, text) in sources.iter().zip(&texts) {
            let module = path.with_extension("");
            if test_dirs
                .iter()
                .any(|t| module.starts_with(t) || path.starts_with(t))
            {
                continue;
            }
            files += 1;
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .display()
                .to_string();
            for (line, what) in findings(text, &content) {
                problems.push(format!("{rel}:{line}: {what}"));
            }
        }
    }
    if problems.is_empty() {
        println!(
            "check-content-ids: {files} files in {} crates name none of {} content entries",
            SIM_CRATES.len(),
            content.len()
        );
        Ok(())
    } else {
        Err(format!(
            "check-content-ids: the simulation names content (AC-12.4). Find content by handle \
             and kind (Registry::aircraft_handles and friends), or move the code into a test.\n{}",
            problems.join("\n")
        ))
    }
}

/// Every `id` of the JSON entities under `dir` (schemas excluded), as `(id, name)`.
fn content_ids(dir: &Path) -> Result<Vec<(String, String)>, String> {
    let mut files = Vec::new();
    json_files(dir, &mut files)?;
    let mut out = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let Ok(json) = serde_json::from_str::<Value>(&text) else {
            continue; // the baker reports broken JSON; this lint only needs the ids
        };
        if let Some(id) = json.get("id").and_then(Value::as_str)
            && let Some(name) = id_name(id)
        {
            out.push((id.to_string(), name.to_string()));
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn json_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "schemas") {
                json_files(&path, out)?;
            }
        } else if path.extension().is_some_and(|e| e == "json") {
            out.push(path);
        }
    }
    Ok(())
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .map(|e| e.map(|e| e.path()).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// Where the files of `path`'s child modules live: next to it for `lib.rs`, `main.rs` and
/// `mod.rs`, otherwise in a directory named after it.
fn child_module_dir(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or(Path::new(""));
    match path.file_stem().and_then(|s| s.to_str()) {
        Some("lib" | "main" | "mod") | None => parent.to_path_buf(),
        Some(stem) => parent.join(stem),
    }
}

/// The name part of `pack:kind/name`, if `s` has that shape.
fn id_name(s: &str) -> Option<&str> {
    let (pack, rest) = s.split_once(':')?;
    let (kind, name) = rest.split_once('/')?;
    let word = |w: &str, extra: &str| {
        w.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && w.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || extra.contains(c))
    };
    (word(pack, "_-") && word(kind, "_") && !name.is_empty() && {
        name.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-.".contains(c))
    })
    .then_some(name)
}

/// `(line, what)` for every line with a violation in one source file, in line order.
fn findings(source: &str, content: &[(String, String)]) -> Vec<(usize, String)> {
    let lexed = lex(source);
    let mut by_line: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for lit in &lexed.literals {
        for token in lit
            .text
            .split(|c: char| !(c.is_ascii_alphanumeric() || "_-.:/".contains(c)))
        {
            if id_name(token).is_some() {
                by_line
                    .entry(lit.line)
                    .or_default()
                    .push(format!("content id \"{token}\" in a string"));
            }
        }
    }
    // Several entries may share a name (an aircraft and its aero tables): search each name once.
    let mut names: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, name) in content {
        if name.len() >= MIN_NAME_LEN {
            names.entry(name).or_default().push(id);
        }
    }
    let code: Vec<char> = lexed.code.chars().collect();
    let lower: Vec<char> = code.iter().map(char::to_ascii_lowercase).collect();
    let boundary = |c: Option<char>| c.is_none_or(|c| !c.is_ascii_alphanumeric());
    for (name, ids) in &names {
        let needle: Vec<char> = name.chars().collect();
        for at in 0..(lower.len() + 1).saturating_sub(needle.len()) {
            if lower[at..at + needle.len()] == needle[..]
                && boundary(at.checked_sub(1).map(|i| lower[i]))
                && boundary(lower.get(at + needle.len()).copied())
            {
                let line = 1 + code[..at].iter().filter(|&&c| c == '\n').count();
                let word: String = code[at..at + needle.len()].iter().collect();
                let what = format!("\"{word}\" names {}", ids.join(", "));
                let entry = by_line.entry(line).or_default();
                if !entry.contains(&what) {
                    entry.push(what);
                }
            }
        }
    }
    by_line
        .into_iter()
        .map(|(line, what)| (line, what.join("; ")))
        .collect()
}

/// A string literal and the line it starts on.
#[derive(Debug, PartialEq)]
struct Literal {
    line: usize,
    text: String,
}

/// A source file with comments and `#[cfg(test)]` items blanked (newlines kept, so line numbers
/// hold), and the string literals that remain.
struct Lexed {
    code: String,
    literals: Vec<Literal>,
    /// Modules declared as `#[cfg(test)] mod name;`, whose files are tests.
    test_modules: Vec<String>,
}

/// Splits Rust source into code and string literals, blanking comments, then blanks
/// `#[cfg(test)]` items.
fn lex(source: &str) -> Lexed {
    let s: Vec<char> = source.chars().collect();
    let spans = scan(&s);
    // `code` keeps literals; `skeleton` blanks their contents too, so braces in strings do not
    // count when finding the end of a test item.
    let mut code = s.clone();
    let mut skeleton = s.clone();
    for &(from, to) in &spans.comments {
        blank(&mut code, from, to);
        blank(&mut skeleton, from, to);
    }
    for &(from, to) in spans.strings.iter().chain(&spans.chars) {
        blank(&mut skeleton, from, to);
    }
    let (tests, test_modules) = test_items(&skeleton);
    for &(from, to) in &tests {
        blank(&mut code, from, to);
    }
    let in_test = |at: usize| tests.iter().any(|&(f, t)| at >= f && at < t);
    let literals = spans
        .strings
        .iter()
        .filter(|&&(from, _)| !in_test(from))
        .map(|&(from, to)| Literal {
            line: 1 + s[..from].iter().filter(|&&c| c == '\n').count(),
            text: s[from..to].iter().collect(),
        })
        .collect();
    Lexed {
        code: code.into_iter().collect(),
        literals,
        test_modules,
    }
}

/// Replaces `v[from..to]` with spaces, keeping newlines.
fn blank(v: &mut [char], from: usize, to: usize) {
    for c in &mut v[from..to] {
        if *c != '\n' {
            *c = ' ';
        }
    }
}

/// Where the comments and literals are, as `[start, end)` spans; for literals, their contents.
struct Spans {
    comments: Vec<(usize, usize)>,
    strings: Vec<(usize, usize)>,
    chars: Vec<(usize, usize)>,
}

/// A small hand lexer: line, block (nested) and doc comments; plain, byte and raw strings; char
/// literals versus lifetimes.
fn scan(s: &[char]) -> Spans {
    let mut out = Spans {
        comments: Vec::new(),
        strings: Vec::new(),
        chars: Vec::new(),
    };
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        let next = s.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            let end = s[i..]
                .iter()
                .position(|&c| c == '\n')
                .map_or(s.len(), |p| i + p);
            out.comments.push((i, end));
            i = end;
        } else if c == '/' && next == Some('*') {
            let (mut depth, mut j) = (1, i + 2);
            while j < s.len() && depth > 0 {
                if s[j] == '/' && s.get(j + 1) == Some(&'*') {
                    depth += 1;
                    j += 2;
                } else if s[j] == '*' && s.get(j + 1) == Some(&'/') {
                    depth -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            let j = j.min(s.len());
            out.comments.push((i, j));
            i = j;
        } else if c == 'r'
            && (i == 0 || !ident(s[i - 1]) || (s[i - 1] == 'b' && (i < 2 || !ident(s[i - 2]))))
            && matches!(next, Some('"' | '#'))
        {
            let hashes = s[i + 1..].iter().take_while(|&&c| c == '#').count();
            let open = i + 1 + hashes;
            if s.get(open) != Some(&'"') {
                i += 1; // `r#ident`, a raw identifier
                continue;
            }
            let closes = |j: usize| {
                s[j] == '"'
                    && s[j + 1..]
                        .iter()
                        .take(hashes)
                        .filter(|&&c| c == '#')
                        .count()
                        == hashes
            };
            let j = (open + 1..s.len()).find(|&j| closes(j)).unwrap_or(s.len());
            out.strings.push((open + 1, j));
            i = (j + 1 + hashes).min(s.len());
        } else if c == '"' {
            let mut j = i + 1;
            while j < s.len() && s[j] != '"' {
                j += if s[j] == '\\' { 2 } else { 1 };
            }
            let j = j.min(s.len());
            out.strings.push((i + 1, j));
            i = j + 1;
        } else if c == '\'' {
            // `'\n'` and `'x'` are chars; `'a` and `'static` are lifetimes.
            let end = if next == Some('\\') {
                s[i + 2..]
                    .iter()
                    .position(|&c| c == '\'')
                    .map(|p| i + 2 + p)
            } else if s.get(i + 2) == Some(&'\'') {
                Some(i + 2)
            } else {
                None
            };
            if let Some(end) = end {
                out.chars.push((i + 1, end));
                i = end + 1;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    out
}

/// The `#[cfg(test)]` items of a skeleton (comments and literals blanked), each from the
/// attribute to its closing brace or semicolon, and the names of those that are modules in their
/// own files.
fn test_items(skeleton: &[char]) -> (Vec<(usize, usize)>, Vec<String>) {
    let attr: Vec<char> = "#[cfg(test)]".chars().collect();
    let mut items = Vec::new();
    let mut modules = Vec::new();
    let mut i = 0;
    while i + attr.len() <= skeleton.len() {
        if skeleton[i..i + attr.len()] != attr[..] {
            i += 1;
            continue;
        }
        let mut j = i + attr.len();
        let mut depth = 0usize;
        while j < skeleton.len() {
            match skeleton[j] {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                ';' if depth == 0 => break,
                _ => {}
            }
            j += 1;
        }
        let end = (j + 1).min(skeleton.len());
        let item: String = skeleton[i + attr.len()..end].iter().collect();
        if let Some(name) = out_of_line_module(&item) {
            modules.push(name);
        }
        items.push((i, end));
        i = end;
    }
    (items, modules)
}

/// `name` if `item` is `mod name;` (optionally `pub` or `pub(...)`), a module in its own file.
fn out_of_line_module(item: &str) -> Option<String> {
    let words: Vec<&str> = item
        .split(|c: char| c.is_whitespace() || c == ';')
        .filter(|w| !w.is_empty())
        .collect();
    let words = match words.first() {
        Some(w) if w.starts_with("pub") => &words[1..],
        _ => &words[..],
    };
    match words {
        ["mod", name] if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') => {
            Some((*name).to_string())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kestrel() -> Vec<(String, String)> {
        vec![
            ("core:ac/kestrel".into(), "kestrel".into()),
            ("core:eng/f129".into(), "f129".into()),
        ]
    }

    fn lines(src: &str) -> Vec<usize> {
        findings(src, &kestrel())
            .into_iter()
            .map(|(l, _)| l)
            .collect()
    }

    #[test]
    fn content_ids_have_the_pack_kind_name_shape() {
        assert_eq!(id_name("core:ac/kestrel"), Some("kestrel"));
        assert_eq!(id_name("my_mod:eng/f-129.b"), Some("f-129.b"));
        for not in [
            "https://example.org",
            "a:b",
            "core:/x",
            ":ac/x",
            "Core:ac/x",
            "core:ac/",
        ] {
            assert_eq!(id_name(not), None, "{not}");
        }
    }

    #[test]
    fn finds_an_id_in_a_string_and_a_name_in_code_in_any_case() {
        assert_eq!(
            lines("fn a() {\n    let h = find(\"core:ac/kestrel\");\n}\n"),
            [2]
        );
        assert_eq!(lines("pub const KESTREL: u32 = 1;"), [1]);
        assert_eq!(lines("let x = kestrel_aero();"), [1]);
        assert_eq!(lines("let n = \"F129 engine\";"), [1]);
        assert_eq!(lines("let s = r#\"a \"core:eng/f129\" b\"#;"), [1]);
    }

    #[test]
    fn a_name_inside_a_longer_word_is_not_a_name() {
        assert_eq!(lines("let kestrels = 1; let xf1290 = 2;"), [0usize; 0]);
    }

    #[test]
    fn comments_and_test_items_may_name_content() {
        let src = "\
/// The Kestrel's tables. /* nested /* \"core:ac/kestrel\" */ still a comment */
// KESTREL
fn live() {}
#[cfg(test)]
mod tests {
    const S: &str = \"} core:ac/kestrel {\";
    fn kestrel() {}
}
#[cfg(test)]
use kestrel::Thing;
fn after() { let k = KESTREL; }
";
        assert_eq!(lines(src), [11], "only the code after the test items");
    }

    #[test]
    fn test_modules_in_their_own_files_are_found() {
        let src = "#[cfg(test)]\nmod tests;\n#[cfg(test)]\npub(crate) mod fixtures;\nmod live;\n";
        assert_eq!(lex(src).test_modules, ["tests", "fixtures"]);
        assert_eq!(
            child_module_dir(Path::new("src/aircraft.rs")),
            Path::new("src/aircraft")
        );
        assert_eq!(child_module_dir(Path::new("src/lib.rs")), Path::new("src"));
    }

    #[test]
    fn lifetimes_are_not_char_literals() {
        // A lifetime must not swallow the code after it as if it were a literal.
        assert_eq!(
            lines("fn f<'a>(x: &'a str) -> char { let k = KESTREL; '\\'' }"),
            [1]
        );
        assert_eq!(lines("let c = '\"'; let k = \"core:ac/kestrel\";"), [1]);
    }
}
