use std::path::{Component, Path, PathBuf};

pub(super) fn parse_new_path_marker(raw: &str) -> Option<PathBuf> {
    let marker = raw.strip_prefix("+++ ")?;
    let path = parse_diff_path_token(marker)?;
    if path.is_dev_null() {
        return None;
    }
    let path = path.without_side_prefix("b/");
    confine_to_relative_path(&path)
}

pub(super) fn is_dev_null_new_path_marker(raw: &str) -> bool {
    raw.strip_prefix("+++ ")
        .and_then(parse_diff_path_token)
        .is_some_and(|path| path.is_dev_null())
}

pub(super) fn parse_git_old_path(raw: &str) -> Option<PathBuf> {
    let marker = raw.strip_prefix("diff --git ")?;
    let path = if marker.starts_with('"') {
        parse_diff_path_token(marker)?
    } else {
        let old_path = marker.strip_prefix("a/")?.split_once(" b/")?.0;
        parse_diff_path_token(&format!("a/{old_path}"))?
    };
    let path = path.without_side_prefix("a/");
    confine_to_relative_path(&path)
}

pub(super) fn parse_rename_from_path(raw: &str) -> Option<PathBuf> {
    parse_rename_path_marker(raw, "rename from ")
}

pub(super) fn parse_rename_to_path(raw: &str) -> Option<PathBuf> {
    parse_rename_path_marker(raw, "rename to ")
}

fn parse_rename_path_marker(raw: &str, prefix: &str) -> Option<PathBuf> {
    let path = raw.strip_prefix(prefix).and_then(parse_diff_path_token)?;
    confine_to_relative_path(&path)
}

/// Whether `raw` is syntactically a `+++ <path>` new-path marker, regardless
/// of whether the path survives confinement. Boundary detection must treat a
/// confinement-rejected (or `/dev/null`) marker as a file-section boundary:
/// otherwise, in a plain diff with no `diff --git` separators, the rejected
/// marker line and its hunk are consumed as payload of the previous file,
/// mis-attributing attacker-controlled lines to an in-workspace path
/// (#2099 review).
pub(super) fn is_new_path_marker(raw: &str) -> bool {
    let Some(marker) = raw.strip_prefix("+++ ") else {
        return false;
    };
    let trimmed = marker.trim_end_matches('\r');
    let quoted = trimmed.starts_with('"');
    match parse_diff_path_token(marker) {
        // An unquoted path containing whitespace is implausible as a diff
        // path (git C-quotes such paths): without this gate, hunk payload
        // lines like `--- token` / `+++ token with spaces` could be misread
        // as a file-section boundary. Mirrors the plausibility contract of
        // parse_old_path_marker. Quoted paths may legitimately contain
        // spaces.
        Some(path) => quoted || path.is_plausible(),
        None => false,
    }
}

/// Lexically confine a parsed diff path to the workspace: keep only `Normal`
/// components and reject the whole path when it contains a parent-directory,
/// root, or prefix component. A crafted diff such as
/// `+++ b/../../../etc/passwd` would otherwise reach `root.join(path)`
/// unconfined and produce a `SourceLocation` that escapes the workspace
/// (#2099). Rejection returns `None`, which the parser treats like
/// `/dev/null`: the file is never registered, so no probe, output record, or
/// snapshot lookup can reference the escaping path. Byte-native raw forms
/// confine through the platform's own path components, so traversal
/// detection is unchanged (#3609).
fn confine_to_relative_path(path: &DecodedPath) -> Option<PathBuf> {
    let mut confined = PathBuf::new();
    for component in path.as_path().components() {
        match component {
            Component::Normal(part) => confined.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if confined.as_os_str().is_empty() {
        None
    } else {
        Some(confined)
    }
}

/// Detect whether `raw` is a syntactically valid `--- <path>` old-path marker.
///
/// Returns `true` for any plausible old-path marker, including paths that
/// would be rejected by `confine_to_relative_path`. This function is a
/// **boundary detector** for the parser — it must recognize `---` lines as
/// file-section boundaries even when the path escapes the workspace, so the
/// parser can clear the current file and prevent payload mis-attribution
/// (#2099).
///
/// The old path is **not** confined here because it is currently discarded
/// (the parser does not build a `SourceLocation` from it). Any future feature
/// that consumes the old path (rename-aware probe, copy detection) MUST call
/// [`confine_to_relative_path`] on the extracted path before joining it to
/// the workspace root. Use [`parse_old_path_for_confinement`] for that
/// purpose (#2402).
pub(super) fn parse_old_path_marker(raw: &str) -> bool {
    let Some(marker) = raw.strip_prefix("--- ") else {
        return false;
    };
    let Some(path) = parse_diff_path_token(marker) else {
        return false;
    };
    if path.is_dev_null() {
        return true;
    }
    path.without_side_prefix("a/").is_plausible()
}

/// Extract and confine the old path from a `--- <path>` marker, symmetrically
/// with [`parse_new_path_marker`]. Returns `None` when the path escapes the
/// workspace (traversal, absolute, prefix). Future features that consume the
/// old path should use this instead of raw string extraction (#2402).
#[allow(
    dead_code,
    reason = "reserved for future rename/copy-detection features that consume the old path (#2402)"
)]
pub(super) fn parse_old_path_for_confinement(raw: &str) -> Option<PathBuf> {
    let marker = raw.strip_prefix("--- ")?;
    let path = parse_diff_path_token(marker)?;
    if path.is_dev_null() {
        return None;
    }
    let path = path.without_side_prefix("a/");
    confine_to_relative_path(&path)
}

/// A decoded diff path token, held in the representation that preserves the
/// file's identity.
///
/// The common case is valid UTF-8 text. When the decoded bytes are not valid
/// UTF-8, Unix keeps them byte-native through the OS path type, so a
/// valid-UTF-8 name can never decode onto the same path as an invalid-byte
/// name — decoded identity is injective (#3609). Windows paths are WTF-16
/// and cannot carry raw bytes; there the invalid bytes are rendered as
/// literal octal-residue text instead (a disclosed lossy form, see
/// [`decode_path_bytes`]).
#[derive(Debug, PartialEq, Eq)]
enum DecodedPath {
    Text(String),
    #[cfg(unix)]
    Raw(std::ffi::OsString),
}

impl DecodedPath {
    /// Borrow the value as a native path for lexical confinement and
    /// plausibility checks. Component semantics stay the platform's own: on
    /// Unix a raw form separates on `/` at the byte level; on Windows only
    /// the text form exists and `Path` confinement is unchanged.
    fn as_path(&self) -> &Path {
        match self {
            DecodedPath::Text(text) => Path::new(text),
            #[cfg(unix)]
            DecodedPath::Raw(raw) => Path::new(raw),
        }
    }

    /// `/dev/null` is all-ASCII, so only the text form can equal it; a raw
    /// form always carries a byte that is not valid UTF-8.
    fn is_dev_null(&self) -> bool {
        match self {
            DecodedPath::Text(text) => text == "/dev/null",
            #[cfg(unix)]
            DecodedPath::Raw(_) => false,
        }
    }

    /// Strip git's `a/` or `b/` side prefix when present, byte-exact on both
    /// representations. The ASCII prefixes cannot occur inside a multi-byte
    /// sequence, so the byte-level strip on a raw form is unambiguous.
    fn without_side_prefix(self, prefix: &str) -> DecodedPath {
        match self {
            DecodedPath::Text(text) => match text.strip_prefix(prefix) {
                Some(rest) => DecodedPath::Text(rest.to_string()),
                None => DecodedPath::Text(text),
            },
            #[cfg(unix)]
            DecodedPath::Raw(raw) => {
                use std::os::unix::ffi::OsStrExt;
                match raw.as_bytes().strip_prefix(prefix.as_bytes()) {
                    Some(rest) => {
                        DecodedPath::Raw(std::ffi::OsStr::from_bytes(rest).to_os_string())
                    }
                    None => DecodedPath::Raw(raw),
                }
            }
        }
    }

    /// Whether the decoded token is plausible as an unquoted diff path:
    /// non-empty and free of whitespace (git C-quotes paths containing
    /// whitespace, so an unquoted token with spaces is likely hunk payload).
    fn is_plausible(&self) -> bool {
        match self {
            DecodedPath::Text(text) => !text.is_empty() && !text.chars().any(char::is_whitespace),
            // A raw form is not valid UTF-8, so it cannot be inspected
            // per-character; whitespace is detected at the byte level.
            // Unicode whitespace encodes without ASCII bytes, so such a name
            // counts as plausible here — erring toward recognizing the
            // `--- `/`+++ ` line as a file-section boundary, which is the
            // fail-safe direction for payload mis-attribution (#2099).
            #[cfg(unix)]
            DecodedPath::Raw(raw) => {
                use std::os::unix::ffi::OsStrExt;
                let bytes = raw.as_bytes();
                !bytes.is_empty() && !bytes.iter().any(|byte| byte.is_ascii_whitespace())
            }
        }
    }
}

fn parse_diff_path_token(raw: &str) -> Option<DecodedPath> {
    let raw = raw.trim_end_matches('\r');
    if let Some(quoted) = raw.strip_prefix('"') {
        return parse_c_quoted_path(quoted);
    }

    let token = raw.split_once('\t').map_or(raw, |(path, _metadata)| path);
    let token = token.trim_end();
    (!token.is_empty()).then(|| DecodedPath::Text(token.to_string()))
}

fn parse_c_quoted_path(raw: &str) -> Option<DecodedPath> {
    // Decode at the byte level: git's octal escapes carry raw bytes, so
    // mapping each escaped byte to one Unicode scalar would turn a valid
    // UTF-8 name like `caf\303\251.rs` into `cafÃ©.rs` and lose workspace
    // identity (#3601 review). Valid UTF-8 sequences reconstruct their
    // true name; bytes that are not valid UTF-8 stay native in the path
    // type on Unix, so a valid name cannot decode onto an invalid-byte
    // name's identity (#3609).
    let mut bytes = Vec::new();
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(decode_path_bytes(bytes)),
            '\\' => parse_c_escape(&mut chars, &mut bytes),
            _ => {
                let mut buf = [0u8; 4];
                bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            }
        }
    }

    None
}

/// Reconstruct a diff path from decoded bytes. Valid UTF-8 passes through as
/// text. On Unix, bytes that are not valid UTF-8 stay raw in the OS path
/// type: the decoded identity is injective, so the file named with raw byte
/// 0xFF and the file literally named `pricing_\377.rs` decode to distinct
/// paths and cannot merge in the parser's path-keyed map (#3609). Windows
/// paths are WTF-16 and cannot carry raw bytes; there, invalid bytes render
/// as literal octal escapes (`\377`) — distinct per byte and never the
/// U+FFFD replacement character. That text form is a disclosed Windows
/// limitation: a valid-UTF-8 name that literally contains the escape text
/// can still mimic an invalid-byte name on that platform.
fn decode_path_bytes(bytes: Vec<u8>) -> DecodedPath {
    match String::from_utf8(bytes) {
        Ok(text) => DecodedPath::Text(text),
        #[cfg(unix)]
        Err(err) => {
            use std::os::unix::ffi::OsStringExt;
            DecodedPath::Raw(std::ffi::OsString::from_vec(err.into_bytes()))
        }
        #[cfg(not(unix))]
        Err(err) => DecodedPath::Text(octal_residue_text(err.into_bytes())),
    }
}

/// Windows rendering for decoded bytes that are not valid UTF-8: every
/// invalid byte becomes its literal octal escape (`\377`), so distinct
/// bytes stay distinct and no U+FFFD replacement character is produced.
/// The escape text itself remains forgeable by a valid name, which is why
/// Unix decodes through the path type instead (#3609).
#[cfg(not(unix))]
fn octal_residue_text(bytes: Vec<u8>) -> String {
    let mut out = String::with_capacity(bytes.len());
    let mut rest: &[u8] = &bytes;
    while !rest.is_empty() {
        match std::str::from_utf8(rest) {
            Ok(valid) => {
                out.push_str(valid);
                break;
            }
            Err(error) => {
                let valid_up_to = error.valid_up_to();
                if let Ok(valid) = std::str::from_utf8(&rest[..valid_up_to]) {
                    out.push_str(valid);
                }
                let invalid_len = error.error_len().unwrap_or(rest.len() - valid_up_to);
                for byte in &rest[valid_up_to..valid_up_to + invalid_len] {
                    push_octal_residue(&mut out, *byte);
                }
                rest = &rest[valid_up_to + invalid_len..];
            }
        }
    }
    out
}

#[cfg(not(unix))]
fn push_octal_residue(out: &mut String, byte: u8) {
    out.push('\\');
    out.push((b'0' + byte / 64) as char);
    out.push((b'0' + (byte / 8) % 8) as char);
    out.push((b'0' + byte % 8) as char);
}

fn parse_c_escape<I>(chars: &mut std::iter::Peekable<I>, bytes: &mut Vec<u8>)
where
    I: Iterator<Item = char>,
{
    let Some(ch) = chars.next() else {
        bytes.push(b'\\');
        return;
    };

    match ch {
        'n' => bytes.push(b'\n'),
        'r' => bytes.push(b'\r'),
        't' => bytes.push(b'\t'),
        '\\' => bytes.push(b'\\'),
        '"' => bytes.push(b'"'),
        '0'..='7' => bytes.push(parse_octal_escape(ch, chars)),
        other => {
            let mut buf = [0u8; 4];
            bytes.extend_from_slice(other.encode_utf8(&mut buf).as_bytes());
        }
    }
}

fn parse_octal_escape<I>(first: char, chars: &mut std::iter::Peekable<I>) -> u8
where
    I: Iterator<Item = char>,
{
    let mut value = first.to_digit(8).unwrap_or(0);

    for _ in 0..2 {
        let Some(next) = chars.peek().copied() else {
            break;
        };
        let Some(digit) = next.to_digit(8) else {
            break;
        };
        let _ = chars.next();
        value = value.saturating_mul(8).saturating_add(digit);
    }

    // git emits three octal digits per raw byte, so the value fits u8;
    // clamp out-of-range forms instead of widening to a Unicode scalar.
    u8::try_from(value).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_quoted_paths_reconstruct_utf8_names_and_keep_invalid_bytes_distinct() {
        #[cfg(unix)]
        use std::os::unix::ffi::OsStrExt;

        // Valid UTF-8 sequences decode at the byte level, restoring the
        // on-disk name (#3601 review): \303\251 is é, not Ã©.
        assert_eq!(
            parse_diff_path_token("\"caf\\303\\251.rs\""),
            Some(DecodedPath::Text("café.rs".to_string()))
        );
        // Bytes that are not valid UTF-8 stay native in the path type on
        // Unix (#3609); Windows keeps the disclosed octal-residue text.
        #[cfg(unix)]
        assert_eq!(
            parse_diff_path_token("\"pricing_\\377.rs\""),
            Some(DecodedPath::Raw(
                std::ffi::OsStr::from_bytes(b"pricing_\xff.rs").to_os_string()
            ))
        );
        #[cfg(not(unix))]
        assert_eq!(
            parse_diff_path_token("\"pricing_\\377.rs\""),
            Some(DecodedPath::Text("pricing_\\377.rs".to_string()))
        );
        #[cfg(unix)]
        assert_eq!(
            parse_diff_path_token("\"pricing_\\376.rs\""),
            Some(DecodedPath::Raw(
                std::ffi::OsStr::from_bytes(b"pricing_\xfe.rs").to_os_string()
            ))
        );
        #[cfg(not(unix))]
        assert_eq!(
            parse_diff_path_token("\"pricing_\\376.rs\""),
            Some(DecodedPath::Text("pricing_\\376.rs".to_string()))
        );
        // Escaped backslash and metacharacters decode as before.
        assert_eq!(
            parse_diff_path_token("\"a\\\\b\\tc.rs\""),
            Some(DecodedPath::Text("a\\b\tc.rs".to_string()))
        );
        // No closing quote stays malformed.
        assert_eq!(parse_diff_path_token("\"unclosed.rs"), None);
    }

    #[test]
    fn quoted_utf8_names_keep_their_workspace_identity() {
        // \303\251 is é: the decoded path must equal the on-disk name's path
        // byte-for-byte, on every platform.
        let decoded = parse_new_path_marker("+++ \"b/caf\\303\\251.rs\"");
        assert_eq!(decoded, Some(PathBuf::from("café.rs")));
    }

    #[cfg(unix)]
    #[test]
    fn decoded_raw_bytes_stay_native_so_a_valid_name_cannot_mimic_them() {
        use std::os::unix::ffi::OsStrExt;

        // #3609: the invalid-byte name and the valid-UTF-8 name that
        // literally spells the old octal-residue text (`pricing_\377.rs`)
        // are two distinct files. Decoding through the path type keeps
        // their identities injective: the raw byte stays a raw byte and
        // never collapses onto the residue text, so the two paths cannot
        // merge in the parser's path-keyed map.
        let raw_byte = parse_new_path_marker("+++ \"b/pricing_\\377.rs\"");
        let mimic = parse_new_path_marker("+++ \"b/pricing_\\\\377.rs\"");

        assert_eq!(
            raw_byte.as_deref().map(|path| path.as_os_str().as_bytes()),
            Some(b"pricing_\xff.rs".as_slice()),
            "the decoded path must carry the raw byte, not octal-residue text"
        );
        assert_eq!(
            mimic.as_deref().map(|path| path.as_os_str().as_bytes()),
            Some(b"pricing_\\377.rs".as_slice()),
            "the literal mimic keeps its own distinct text identity"
        );
        assert_ne!(
            raw_byte, mimic,
            "a valid-UTF-8 name must not decode onto an invalid-byte name's path (#3609)"
        );
    }

    #[cfg(unix)]
    #[test]
    fn raw_byte_paths_confine_like_text_paths() {
        // Confinement applies to byte-native forms unchanged: a parent
        // directory inside a quoted raw-byte token is still rejected.
        assert_eq!(parse_new_path_marker("+++ \"b/../pricing_\\377.rs\""), None);
    }

    #[test]
    fn parse_new_path_marker_strips_b_prefix() {
        let path = parse_new_path_marker("+++ b/src/lib.rs");
        assert_eq!(path, Some(PathBuf::from("src/lib.rs")));
    }

    #[test]
    fn parse_new_path_marker_accepts_no_prefix() {
        let path = parse_new_path_marker("+++ src/lib.rs");
        assert_eq!(path, Some(PathBuf::from("src/lib.rs")));
    }

    #[test]
    fn parse_new_path_marker_returns_none_for_dev_null() {
        assert_eq!(parse_new_path_marker("+++ /dev/null"), None);
    }

    #[test]
    fn parse_git_old_path_confines_the_section_identity() {
        assert_eq!(
            parse_git_old_path("diff --git a/src/old.rs b/src/old.rs"),
            Some(PathBuf::from("src/old.rs"))
        );
        assert_eq!(
            parse_git_old_path("diff --git a/../../../etc/passwd b/../../../etc/passwd"),
            None
        );
    }

    #[test]
    fn parse_new_path_marker_rejects_traversal() {
        assert_eq!(parse_new_path_marker("+++ b/../escape.rs"), None);
        assert_eq!(parse_new_path_marker("+++ ../../../etc/passwd"), None);
    }

    #[test]
    fn parse_new_path_marker_strips_curdir() {
        // A path like `./src/lib.rs` normalizes to `src/lib.rs`.
        let path = parse_new_path_marker("+++ b/./src/lib.rs");
        assert_eq!(path, Some(PathBuf::from("src/lib.rs")));
    }

    #[test]
    fn parse_new_path_marker_parses_c_quoted_path() {
        let path = parse_new_path_marker("+++ \"b/src/my file.rs\"");
        assert_eq!(path, Some(PathBuf::from("src/my file.rs")));
    }

    #[test]
    fn parse_new_path_marker_parses_octal_escape() {
        // \040 is the C-quoted form of a space.
        let path = parse_new_path_marker("+++ \"b/src/my\\040file.rs\"");
        assert_eq!(path, Some(PathBuf::from("src/my file.rs")));
    }

    #[test]
    fn is_new_path_marker_recognizes_valid_markers() {
        assert!(is_new_path_marker("+++ b/src/lib.rs"));
        assert!(is_new_path_marker("+++ /dev/null"));
        assert!(is_new_path_marker("+++ \"b/src/file with space.rs\""));
    }

    #[test]
    fn is_new_path_marker_rejects_non_markers() {
        assert!(!is_new_path_marker("--- a/src/lib.rs"));
        assert!(!is_new_path_marker("++ payload with spaces"));
        assert!(!is_new_path_marker("regular line"));
    }

    #[test]
    fn parse_old_path_marker_recognizes_valid_markers() {
        assert!(parse_old_path_marker("--- a/src/lib.rs"));
        assert!(parse_old_path_marker("--- /dev/null"));
    }

    #[test]
    fn parse_old_path_marker_rejects_non_markers() {
        assert!(!parse_old_path_marker("+++ b/src/lib.rs"));
        assert!(!parse_old_path_marker("regular line"));
    }
}
