//! Errors that point at the offending value: file, JSON pointer, message and a fix.

use core::fmt;

/// One problem found while loading or validating authoring data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// File, relative to the pack directory.
    pub file: String,
    /// RFC 6901 JSON pointer to the value; empty for the whole file.
    pub pointer: String,
    /// What is wrong.
    pub message: String,
    /// What to do about it.
    pub hint: String,
}

impl Diagnostic {
    /// A diagnostic for `pointer` in `file`.
    pub fn new(
        file: &str,
        pointer: impl Into<String>,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            file: file.to_owned(),
            pointer: pointer.into(),
            message: message.into(),
            hint: hint.into(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let at = if self.pointer.is_empty() {
            "(whole file)"
        } else {
            &self.pointer
        };
        write!(
            f,
            "error: {}: {at}: {}\n  fix: {}",
            self.file, self.message, self.hint
        )
    }
}

/// Joins path segments into a JSON pointer, escaping `~` and `/`.
#[must_use]
pub fn pointer(segments: &[&str]) -> String {
    use core::fmt::Write;
    segments.iter().fold(String::new(), |mut out, s| {
        let _ = write!(out, "/{}", s.replace('~', "~0").replace('/', "~1"));
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointers_escape_their_separators() {
        assert_eq!(pointer(&[]), "");
        assert_eq!(pointer(&["mass", "empty_kg"]), "/mass/empty_kg");
        assert_eq!(pointer(&["a/b", "c~d"]), "/a~1b/c~0d");
    }

    #[test]
    fn display_names_file_pointer_and_fix() {
        let d = Diagnostic::new("aircraft/x.json", "/id", "bad id", "use core:ac/x");
        let text = d.to_string();
        assert!(text.contains("aircraft/x.json") && text.contains("/id"));
        assert!(text.contains("bad id") && text.contains("fix: use core:ac/x"));
        assert!(
            Diagnostic::new("f", "", "m", "h")
                .to_string()
                .contains("(whole file)")
        );
    }
}
