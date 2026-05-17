//! Identifier sanitization for filesystem paths.
//!
//! Defense against directory traversal: the sensing-server has several REST
//! endpoints that take a user-controlled identifier and use it directly to
//! build a filesystem path:
//!
//!   * `recording.rs`     — `{session_name}.csi.jsonl` under `RECORDINGS_DIR`
//!   * `model_manager.rs` — `{model_id}.rvf` under `models_dir()`
//!   * `training_api.rs`  — `{dataset_id}.csi.jsonl` under `RECORDINGS_DIR`
//!
//! Without validation, an attacker can pass `../../etc/passwd` or similar to
//! read, write, or delete arbitrary files the server process can access. See
//! issue #615 for the full exploit catalogue.
//!
//! [`safe_id`] returns the input only when it is safe to embed in a
//! `format!()` that builds a path under a fixed parent directory.

use std::fmt;

/// Maximum length for a safe identifier. 64 is generous for human-typed
/// session names while keeping the resulting filename well under
/// most filesystem limits.
pub const MAX_ID_LEN: usize = 64;

/// Error returned by [`safe_id`] when the input is not safe to embed in a
/// filesystem path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSafetyError {
    /// Empty string is never a valid identifier.
    Empty,
    /// Identifier exceeds `MAX_ID_LEN` bytes.
    TooLong { len: usize, max: usize },
    /// Identifier contains a character not in the allowed set
    /// `[A-Za-z0-9._-]` (and the leading character is not `.`).
    /// Path separators, null bytes, parent-directory references, and any
    /// non-printable or non-ASCII characters all hit this.
    InvalidChar { ch: char, position: usize },
    /// Identifier is `"."` or `".."`, or any leading `.` that would
    /// otherwise be interpreted as a hidden file / parent reference.
    LeadingDot,
}

impl fmt::Display for PathSafetyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathSafetyError::Empty => write!(f, "identifier is empty"),
            PathSafetyError::TooLong { len, max } => {
                write!(f, "identifier is {len} bytes (max {max})")
            }
            PathSafetyError::InvalidChar { ch, position } => write!(
                f,
                "identifier contains invalid character {ch:?} at position {position} \
                 (only A-Z, a-z, 0-9, '.', '_', '-' are allowed)"
            ),
            PathSafetyError::LeadingDot => write!(
                f,
                "identifier may not start with '.' (would be a hidden file \
                 or parent-directory reference)"
            ),
        }
    }
}

impl std::error::Error for PathSafetyError {}

/// Return `Ok(input)` if the string is safe to embed in a filesystem path
/// built under a fixed parent directory; otherwise return a structured error.
///
/// The allowed character set is `[A-Za-z0-9._-]`. The first character must
/// not be `.` (rules out `..`, `.`, and hidden-file shenanigans).
///
/// Examples:
/// ```ignore
/// assert!(safe_id("my-session_42").is_ok());
/// assert!(safe_id("session.v2").is_ok());
/// assert!(safe_id("../../etc/passwd").is_err());
/// assert!(safe_id("foo/bar").is_err());
/// assert!(safe_id("..").is_err());
/// assert!(safe_id(".env").is_err());
/// assert!(safe_id("").is_err());
/// ```
pub fn safe_id(input: &str) -> Result<&str, PathSafetyError> {
    if input.is_empty() {
        return Err(PathSafetyError::Empty);
    }
    if input.len() > MAX_ID_LEN {
        return Err(PathSafetyError::TooLong {
            len: input.len(),
            max: MAX_ID_LEN,
        });
    }
    // Reject leading '.' to block `.`, `..`, `.env`, etc.
    if input.starts_with('.') {
        return Err(PathSafetyError::LeadingDot);
    }
    for (position, ch) in input.chars().enumerate() {
        let ok = ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-';
        if !ok {
            return Err(PathSafetyError::InvalidChar { ch, position });
        }
    }
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_simple_alphanumeric() {
        assert!(safe_id("foo").is_ok());
        assert!(safe_id("MyModel123").is_ok());
        assert!(safe_id("session-2026-05-17_v2").is_ok());
        assert!(safe_id("a.b.c").is_ok());
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(safe_id(""), Err(PathSafetyError::Empty));
    }

    #[test]
    fn rejects_path_separators() {
        assert!(matches!(
            safe_id("foo/bar"),
            Err(PathSafetyError::InvalidChar { ch: '/', .. })
        ));
        assert!(matches!(
            safe_id("foo\\bar"),
            Err(PathSafetyError::InvalidChar { ch: '\\', .. })
        ));
    }

    #[test]
    fn rejects_parent_directory_traversal() {
        assert_eq!(safe_id("."), Err(PathSafetyError::LeadingDot));
        assert_eq!(safe_id(".."), Err(PathSafetyError::LeadingDot));
        assert_eq!(safe_id(".env"), Err(PathSafetyError::LeadingDot));
        // The classic attack vector — even after rejecting leading-dot,
        // the InvalidChar guard catches the embedded slash.
        assert!(matches!(
            safe_id("../../etc/passwd"),
            Err(PathSafetyError::LeadingDot)
        ));
    }

    #[test]
    fn rejects_null_byte() {
        assert!(matches!(
            safe_id("foo\0bar"),
            Err(PathSafetyError::InvalidChar { ch: '\0', .. })
        ));
    }

    #[test]
    fn rejects_whitespace_and_specials() {
        assert!(matches!(
            safe_id("foo bar"),
            Err(PathSafetyError::InvalidChar { ch: ' ', .. })
        ));
        assert!(matches!(
            safe_id("foo;rm -rf /"),
            Err(PathSafetyError::InvalidChar { .. })
        ));
        assert!(matches!(
            safe_id("foo$bar"),
            Err(PathSafetyError::InvalidChar { ch: '$', .. })
        ));
    }

    #[test]
    fn rejects_non_ascii() {
        // Reject unicode that could normalise to path separators in
        // weird filesystems, or just look like ASCII.
        assert!(matches!(
            safe_id("café"),
            Err(PathSafetyError::InvalidChar { .. })
        ));
        // Fullwidth slash (U+FF0F) — visually similar to '/'.
        assert!(matches!(
            safe_id("foo\u{FF0F}bar"),
            Err(PathSafetyError::InvalidChar { .. })
        ));
    }

    #[test]
    fn rejects_too_long() {
        let too_long = "a".repeat(MAX_ID_LEN + 1);
        assert_eq!(
            safe_id(&too_long),
            Err(PathSafetyError::TooLong {
                len: MAX_ID_LEN + 1,
                max: MAX_ID_LEN
            })
        );
    }

    #[test]
    fn boundary_max_len() {
        let at_max = "a".repeat(MAX_ID_LEN);
        assert!(safe_id(&at_max).is_ok());
    }
}
