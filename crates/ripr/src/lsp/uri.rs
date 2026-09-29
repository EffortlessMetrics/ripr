use std::path::{Component, Path, PathBuf};
use tower_lsp_server::ls_types::Uri;

mod percent_codec {
    pub(super) fn decode_uri_path(path: &str) -> Option<String> {
        let bytes = path.as_bytes();
        let mut decoded = Vec::with_capacity(bytes.len());
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'%' {
                let high = hex_value(*bytes.get(index + 1)?)?;
                let low = hex_value(*bytes.get(index + 2)?)?;
                decoded.push((high << 4) | low);
                index += 3;
            } else {
                decoded.push(bytes[index]);
                index += 1;
            }
        }
        String::from_utf8(decoded).ok()
    }

    pub(super) fn encode_uri_path(path: &str) -> String {
        let mut encoded = String::new();
        for byte in path.bytes() {
            match byte {
                b'A'..=b'Z'
                | b'a'..=b'z'
                | b'0'..=b'9'
                | b'-'
                | b'.'
                | b'_'
                | b'~'
                | b'/'
                | b':' => encoded.push(byte as char),
                _ => encoded.push_str(&format!("%{byte:02X}")),
            }
        }
        encoded
    }

    fn hex_value(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }
}

mod windows_paths {
    pub(super) fn is_windows_drive_uri_path(path: &str) -> bool {
        let bytes = path.as_bytes();
        bytes.len() >= 3 && bytes[0] == b'/' && bytes[2] == b':' && bytes[1].is_ascii_alphabetic()
    }

    pub(super) fn is_windows_drive_path(path: &str) -> bool {
        let bytes = path.as_bytes();
        bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
    }
}

/// Encode a local path as a `file:` URI. The inverse of
/// [`normalized_file_uri_path`], so every admitted `Ok` round-trips through the
/// shared decoder. A UNC, extended-length (`\\?\...`), or device (`\\.\...`)
/// spelling normalizes to a doubled leading separator, which this local-only
/// decoder rejects; refuse that at emission rather than publishing a URI that
/// would later read as no file at all.
///
/// A `..` segment in a path this process built is collapsed before encoding.
/// Callers join real files that way (`crates/ripr/../../fixtures/...`). The
/// decoder still refuses a client URI that contains `..`. A relative path
/// whose `..` escapes its own prefix cannot be collapsed without a root, so
/// it is refused instead of emitted with a parent segment.
pub(super) fn file_uri_for_path(path: &Path) -> Result<Uri, String> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if normalized.starts_with("//") {
        return Err(format!(
            "refusing to build a local file URI for the network-share path {}",
            path.display()
        ));
    }
    let Some(collapsed) = collapse_parent_segments(&normalized) else {
        return Err(format!(
            "refusing to build a local file URI for a relative path that escapes its prefix {}",
            path.display()
        ));
    };
    let encoded = encode_uri_path(&collapsed);
    let uri = if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    };
    uri.parse()
        .map_err(|err| format!("failed to build LSP file URI for {}: {err}", path.display()))
}

pub(super) fn path_from_file_uri(uri: &Uri) -> Option<PathBuf> {
    normalized_file_uri_path(uri).map(PathBuf::from)
}

/// Prove that a projected path stays inside the selected workspace root.
/// Existing paths are canonicalized so symlink/junction escapes are rejected;
/// missing paths fall back to normalized lexical containment for diagnostics
/// and command payloads that refer to a future file.
///
/// A relative candidate that is really raw URI text is refused outright. When
/// [`normalized_file_uri_path`] rejects a URI, `state::document_path` keeps the
/// wire string as the document's path; that string begins with a scheme, so it
/// is relative and would otherwise join under every root and read as contained.
pub(super) fn path_is_within_root(root: &Path, path: &Path) -> bool {
    if !path.is_absolute() && carries_uri_separator(path) {
        return false;
    }
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let root = canonical_or_normalized(root);
    let candidate = canonical_or_normalized(&candidate);
    paths_equal_or_below(&root, &candidate)
}

/// Whether a relative candidate's first component ends in a URI scheme
/// separator (`file:`). That is the shape of a raw-wire URI fallback, not of a
/// file path: `:` cannot appear in a Windows filename at all, and a
/// scheme-shaped first segment names a URI the decoder refused rather than a
/// file under the root. Unix does permit `:` inside a filename, so this is a
/// deliberate fail-closed lexical guard rather than a portable statement about
/// Unix path syntax.
fn carries_uri_separator(path: &Path) -> bool {
    match path.components().next() {
        Some(Component::Normal(value)) => value.to_string_lossy().ends_with(':'),
        _ => false,
    }
}

pub(super) fn file_uri_is_within_root(root: &Path, uri: &Uri) -> bool {
    path_from_file_uri(uri).is_some_and(|path| path_is_within_root(root, &path))
}

pub(super) fn file_uris_match(left: &Uri, right: &Uri) -> bool {
    let Some(left_path) = normalized_file_uri_path(left) else {
        return false;
    };
    let Some(right_path) = normalized_file_uri_path(right) else {
        return false;
    };
    // Equal wire strings need no special case: once both sides are admitted they
    // decode to the same path, so the comparisons below already report them
    // equal. An earlier revision short-circuited on `left == right` *before*
    // admission, which made a URI this decoder refuses match itself. Identity is
    // therefore a statement about admitted local paths, never about wire strings.
    if windows_paths::is_windows_drive_path(&left_path)
        && windows_paths::is_windows_drive_path(&right_path)
    {
        return left_path.eq_ignore_ascii_case(&right_path);
    }
    left_path == right_path
}

/// Resolve the supported local forms of a file URI (RFC 8089 sections 2-3).
/// An empty authority and `localhost` identify the same local path. Other
/// authorities are unsupported here: never reinterpret a host as a relative
/// path under the workspace, and never perform DNS or network-share discovery.
///
/// This is the single admission authority for the LSP side: workspace-root
/// selection, file identity ([`file_uris_match`]), containment
/// ([`file_uri_is_within_root`]), and the display path kept by
/// `state::document_path` all resolve through it. `None` therefore means "not a
/// local file this server supports", and callers must fail closed rather than
/// re-derive a path from the wire string. [`file_uri_for_path`] is the matching
/// encoder and refuses the paths whose encoding this decoder would reject.
fn normalized_file_uri_path(uri: &Uri) -> Option<String> {
    let (scheme, rest) = uri.as_str().split_once(':')?;
    if !scheme.eq_ignore_ascii_case("file") || rest.contains(['?', '#']) {
        return None;
    }
    let path = if let Some(authority_path) = rest.strip_prefix("//") {
        let (authority, path) = authority_path.split_at(authority_path.find('/')?);
        if !authority.is_empty() && !authority.eq_ignore_ascii_case("localhost") {
            return None;
        }
        path
    } else if rest.starts_with('/') {
        rest
    } else {
        return None;
    };
    // Split URI components before decoding: encoded filename delimiters are
    // literal path data, not a query, fragment, or a second decoding pass.
    let decoded = percent_codec::decode_uri_path(path)?.replace('\\', "/");
    if decoded.contains('\0') || decoded.starts_with("//") {
        // A doubled leading separator is a UNC/network-share spelling on
        // Windows even when the URI authority is empty or localhost. This
        // local-only decoder must not turn an authority bypass into network
        // filesystem access.
        return None;
    }
    // Checked after percent-decoding and backslash normalization, so
    // `%2e%2e` and `..\` are the same parent segment the saved-content
    // read would follow. `foo..bar` and `..hidden` are filenames.
    if has_parent_directory_segment(&decoded) {
        return None;
    }
    if windows_paths::is_windows_drive_uri_path(&decoded) {
        // `/C:relative` must not become a drive-relative filesystem path.
        if decoded.as_bytes().get(3) != Some(&b'/') {
            return None;
        }
        Some(decoded[1..].to_string())
    } else {
        Some(decoded)
    }
}

/// True when a `/`-separated path has a segment that is exactly `..`.
fn has_parent_directory_segment(path: &str) -> bool {
    path.split('/').any(|segment| segment == "..")
}

/// Collapse `.` and `..` so an emitted URI has no parent segment.
/// Absolute `..` at the root is a no-op: a leading slash stays `/`, and a
/// drive prefix stays on that drive. A relative `..` that escapes the path's
/// own prefix is `None`. An empty relative result is the relative root; the
/// encoder roots that at `/`, matching how relative paths are already
/// published.
pub(super) fn collapse_parent_segments(path: &str) -> Option<String> {
    if !has_parent_directory_segment(path) {
        return Some(path.to_string());
    }
    let bytes = path.as_bytes();
    let (mut collapsed, absolute, body) = if let Some(body) = path.strip_prefix('/') {
        ("/".to_string(), true, body)
    } else if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && bytes[2] == b'/'
    {
        (path[..2].to_string(), true, &path[3..])
    } else {
        (String::new(), false, path)
    };
    let mut stack = Vec::new();
    for segment in body.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            if stack.pop().is_none() && !absolute {
                return None;
            }
            continue;
        }
        stack.push(segment);
    }
    if stack.is_empty() {
        if absolute && collapsed.ends_with(':') {
            collapsed.push('/');
        }
        return Some(collapsed);
    }
    if absolute && collapsed.ends_with(':') {
        collapsed.push('/');
    }
    if collapsed.is_empty() || collapsed.ends_with('/') {
        collapsed.push_str(&stack.join("/"));
    } else {
        collapsed.push('/');
        collapsed.push_str(&stack.join("/"));
    }
    Some(collapsed)
}

/// Render a path with forward slashes for LSP display (diagnostic messages,
/// hover text, context packets). Replaces native OS separators so Windows
/// backslash paths display consistently. Previously duplicated as
/// `display_lsp_path` in both `backend.rs` and `diagnostics.rs`.
pub(super) fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Resolve a path to an absolute PathBuf, joining to `root` when relative.
/// Previously duplicated as `absolute_context_path` in `backend.rs` and
/// `absolute_path` in `diagnostics.rs` (byte-identical bodies, different names).
pub(super) fn absolute_join(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

/// Byte cap for report artifacts read by the long-running LSP server.
/// Real repo-exposure artifacts for large repositories can reach tens of
/// megabytes; 256 MiB is far above any legitimate artifact while still
/// failing closed on an unbounded input. The cap is enforced while reading
/// (`take(limit + 1)`), not just from metadata, so a file that grows between
/// check and read cannot bypass it. Defined as the CLI's
/// `bounded_input::MAX_CLI_INPUT_BYTES` so the two surfaces cannot drift
/// (#2921, #4480).
pub(super) const MAX_LSP_ARTIFACT_BYTES: u64 = crate::bounded_input::MAX_CLI_INPUT_BYTES;

/// Outcome of a capped artifact read. Callers must distinguish an absent
/// artifact — a normal state for deferred analysis output, where falling back
/// or returning no data is honest — from a present-but-unusable artifact
/// (oversize, unreadable, or non-UTF-8), which requires a typed degradation
/// instead of a silent `None`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) enum CappedArtifactRead {
    Contents(String),
    Missing,
    Unusable,
}

/// Read a repo-controlled report artifact with a metadata pre-check and a
/// byte cap, failing closed. Follows `read_agent_verify_snapshot`'s shape.
pub(super) fn read_artifact_capped(path: &Path) -> CappedArtifactRead {
    read_artifact_capped_with_limit(path, MAX_LSP_ARTIFACT_BYTES)
}

/// `limit` is a parameter so tests can exercise the cap without materializing
/// 256 MiB; production callers use [`read_artifact_capped`]. Reads at most
/// `limit + 1` bytes, so a file that grows concurrently is rejected rather
/// than read in full.
pub(super) fn read_artifact_capped_with_limit(path: &Path, limit: u64) -> CappedArtifactRead {
    use std::io::Read as _;
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return CappedArtifactRead::Missing;
        }
        Err(_) => return CappedArtifactRead::Unusable,
    };
    let mut contents = String::new();
    match file
        .take(limit.saturating_add(1))
        .read_to_string(&mut contents)
    {
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return CappedArtifactRead::Missing;
        }
        Err(_) => return CappedArtifactRead::Unusable,
    }
    if contents.len() as u64 > limit {
        return CappedArtifactRead::Unusable;
    }
    CappedArtifactRead::Contents(contents)
}

/// Canonicalize the longest existing prefix; only a path with no existing
/// ancestor at all stays lexical.
///
/// The tail walk stops at a `..` component, because `file_name` is `None`
/// there. Retrying on the lexically collapsed path lets a missing-tail
/// candidate such as `src/nested/../app.py` still canonicalize through its
/// existing ancestors. Without the retry it fell straight to the lexical form,
/// which never matches a canonical root on Windows (`\\?\` verbatim prefix)
/// or under a symlinked ancestor, and which let `linked/missing/../x.rs`
/// through a symlinked `linked` directory read as contained.
fn canonical_or_normalized(path: &Path) -> PathBuf {
    let normalized = normalize_path(path);
    canonicalize_with_missing_tail(path)
        .or_else(|| canonicalize_with_missing_tail(&normalized))
        .unwrap_or(normalized)
}

fn canonicalize_with_missing_tail(path: &Path) -> Option<PathBuf> {
    let mut current = path.to_path_buf();
    let mut missing = Vec::new();
    loop {
        if let Ok(mut canonical) = current.canonicalize() {
            for component in missing.iter().rev() {
                canonical.push(component);
            }
            return Some(canonical);
        }

        let component = current.file_name()?.to_os_string();
        missing.push(component);
        if !current.pop() {
            return None;
        }
    }
}

pub(super) fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(value) => normalized.push(value),
            Component::RootDir | Component::Prefix(_) => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn paths_equal_or_below(root: &Path, candidate: &Path) -> bool {
    if cfg!(windows) {
        let root_components = root
            .components()
            .map(|component| component.as_os_str().to_string_lossy().to_ascii_lowercase())
            .collect::<Vec<_>>();
        let candidate_components = candidate
            .components()
            .map(|component| component.as_os_str().to_string_lossy().to_ascii_lowercase())
            .collect::<Vec<_>>();
        candidate_components.len() >= root_components.len()
            && candidate_components[..root_components.len()] == root_components[..]
    } else {
        candidate == root || candidate.starts_with(root)
    }
}

pub(super) fn encode_uri_path(path: &str) -> String {
    percent_codec::encode_uri_path(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_uri(value: &str) -> Result<Uri, String> {
        value
            .parse()
            .map_err(|err| format!("failed to parse test URI {value}: {err}"))
    }

    #[test]
    fn file_uri_for_path_percent_encodes_spaces_and_symbols() -> Result<(), String> {
        let uri = file_uri_for_path(Path::new("/tmp/ripr fixtures/a#b?.rs"))?;

        assert_eq!(uri.as_str(), "file:///tmp/ripr%20fixtures/a%23b%3F.rs");
        assert_eq!(
            path_from_file_uri(&uri).ok_or("expected decoded path")?,
            PathBuf::from("/tmp/ripr fixtures/a#b?.rs")
        );
        Ok(())
    }

    #[test]
    fn file_uri_for_path_percent_encodes_unicode_relative_paths() -> Result<(), String> {
        let uri = file_uri_for_path(Path::new("workspace/ripr/src/cafe_menu.rs"))?;
        assert_eq!(uri.as_str(), "file:///workspace/ripr/src/cafe_menu.rs");

        let uri = file_uri_for_path(Path::new("workspace/ripr/src/café.rs"))?;
        assert_eq!(uri.as_str(), "file:///workspace/ripr/src/caf%C3%A9.rs");
        Ok(())
    }

    #[test]
    fn invalid_percent_encoding_is_not_a_file_path() -> Result<(), String> {
        let uri = parse_uri("file:///tmp/%FF.rs")?;

        assert_eq!(path_from_file_uri(&uri), None);
        assert!(!file_uris_match(&uri, &uri));
        Ok(())
    }

    #[test]
    fn path_from_file_uri_rejects_non_file_scheme() -> Result<(), String> {
        let uri = parse_uri("https://example.test/src.rs")?;

        assert_eq!(path_from_file_uri(&uri), None);
        assert!(!file_uris_match(&uri, &uri));
        Ok(())
    }

    #[test]
    fn file_uris_match_normalizes_percent_encoded_separators() -> Result<(), String> {
        let encoded_separator = parse_uri("file:///workspace/ripr/src%2Flib.rs")?;
        let literal_separator = parse_uri("file:///workspace/ripr/src/lib.rs")?;

        assert!(file_uris_match(&encoded_separator, &literal_separator));
        Ok(())
    }

    #[test]
    fn windows_drive_file_uris_match_case_insensitively() -> Result<(), String> {
        let upper = parse_uri("file:///C:/Work/Ripr/src/lib.rs")?;
        let lower = parse_uri("file:///c:/Work/Ripr/src/lib.rs")?;

        assert!(file_uris_match(&upper, &lower));
        Ok(())
    }

    #[test]
    fn file_uris_match_keeps_non_windows_paths_case_sensitive() -> Result<(), String> {
        let upper = parse_uri("file:///workspace/ripr/src/Lib.rs")?;
        let lower = parse_uri("file:///workspace/ripr/src/lib.rs")?;

        assert!(!file_uris_match(&upper, &lower));
        Ok(())
    }

    #[test]
    fn path_is_within_root_rejects_traversal_and_foreign_absolute_paths() {
        let root = Path::new("/workspace/ripr");
        assert!(path_is_within_root(root, Path::new("src/lib.rs")));
        assert!(path_is_within_root(
            root,
            Path::new("/workspace/ripr/src/lib.rs")
        ));
        assert!(!path_is_within_root(root, Path::new("../outside.rs")));
        assert!(!path_is_within_root(root, Path::new("/workspace/other.rs")));
    }

    #[cfg(windows)]
    #[test]
    fn windows_drive_root_containment_uses_path_components() {
        let separator = std::path::MAIN_SEPARATOR;
        let root = PathBuf::from(format!("C:{separator}"));
        let lower_case_child = PathBuf::from(format!("c:{separator}workspace{separator}src.rs"));
        let workspace = PathBuf::from(format!("C:{separator}workspace"));
        let workspace_sibling =
            PathBuf::from(format!("C:{separator}workspace-sibling{separator}src.rs"));
        assert!(paths_equal_or_below(&root, &lower_case_child));
        let c_child = PathBuf::from(format!(
            "C:{separator}workspace{separator}src{separator}lib.rs"
        ));
        let d_child = PathBuf::from(format!(
            "D:{separator}workspace{separator}src{separator}lib.rs"
        ));
        assert!(paths_equal_or_below(&root, &c_child));
        assert!(paths_equal_or_below(&root, &root));
        assert!(!paths_equal_or_below(&workspace, &workspace_sibling));
        assert!(!paths_equal_or_below(&root, &d_child));
    }

    #[test]
    fn path_is_within_root_rejects_missing_leaf_under_symlink_ancestor() -> Result<(), String> {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-uri-root-{suffix}"));
        let outside = std::env::temp_dir().join(format!("ripr-uri-outside-{suffix}"));
        std::fs::create_dir_all(&root).map_err(|err| err.to_string())?;
        std::fs::create_dir_all(&outside).map_err(|err| err.to_string())?;
        let link = root.join("linked");

        #[cfg(unix)]
        let link_result = std::os::unix::fs::symlink(&outside, &link);
        #[cfg(windows)]
        let link_result = std::os::windows::fs::symlink_dir(&outside, &link);
        if let Err(err) = link_result {
            eprintln!("skipping symlink containment test: {err}");
            let _ = std::fs::remove_dir_all(&root);
            let _ = std::fs::remove_dir_all(&outside);
            return Ok(());
        }

        assert!(!path_is_within_root(&root, Path::new("linked/missing.rs")));
        std::fs::remove_dir_all(&root).map_err(|err| err.to_string())?;
        std::fs::remove_dir_all(&outside).map_err(|err| err.to_string())?;
        Ok(())
    }

    fn symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
        #[cfg(unix)]
        let result = std::os::unix::fs::symlink(target, link);
        #[cfg(windows)]
        let result = std::os::windows::fs::symlink_dir(target, link);
        result
    }

    /// A missing-tail candidate with a `..` segment still canonicalizes through
    /// its existing ancestors, so it matches a root reached through a symlink.
    /// Before the retry it fell back to the uncanonicalized lexical form and was
    /// dropped as outside the workspace; on Windows the same mismatch is the
    /// `\\?\` verbatim prefix on every canonical root.
    #[test]
    fn missing_tail_with_parent_segment_matches_a_canonical_root() -> Result<(), String> {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let real = std::env::temp_dir().join(format!("ripr-uri-dotdot-real-{suffix}"));
        let alias = std::env::temp_dir().join(format!("ripr-uri-dotdot-alias-{suffix}"));
        std::fs::create_dir_all(&real).map_err(|err| err.to_string())?;
        if let Err(err) = symlink_dir(&real, &alias) {
            eprintln!("skipping symlinked-root containment test: {err}");
            let _ = std::fs::remove_dir_all(&real);
            return Ok(());
        }

        let dotted = alias.join("src").join("nested").join("..").join("app.py");
        let within = path_is_within_root(&alias, &dotted);
        let escaped = path_is_within_root(
            &alias,
            &alias.join("src").join("..").join("..").join("outside.py"),
        );
        let _ = std::fs::remove_file(&alias).or_else(|_| std::fs::remove_dir(&alias));
        std::fs::remove_dir_all(&real).map_err(|err| err.to_string())?;
        assert!(
            within,
            "src/nested/../app.py under a symlinked root was dropped"
        );
        assert!(
            !escaped,
            "a lexical escape above the root must stay outside"
        );
        Ok(())
    }

    /// The lexical fallback must not let `..` hide a symlinked ancestor:
    /// `linked/missing/../x.rs` resolves through `linked`, which points outside.
    #[test]
    fn parent_segment_cannot_hide_a_symlink_escape() -> Result<(), String> {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-uri-dotdot-root-{suffix}"));
        let outside = std::env::temp_dir().join(format!("ripr-uri-dotdot-outside-{suffix}"));
        std::fs::create_dir_all(&root).map_err(|err| err.to_string())?;
        std::fs::create_dir_all(&outside).map_err(|err| err.to_string())?;
        if let Err(err) = symlink_dir(&outside, &root.join("linked")) {
            eprintln!("skipping symlink escape test: {err}");
            let _ = std::fs::remove_dir_all(&root);
            let _ = std::fs::remove_dir_all(&outside);
            return Ok(());
        }

        let contained = path_is_within_root(&root, Path::new("linked/missing/../x.rs"));
        std::fs::remove_dir_all(&root).map_err(|err| err.to_string())?;
        std::fs::remove_dir_all(&outside).map_err(|err| err.to_string())?;
        assert!(
            !contained,
            "linked/missing/../x.rs escapes through the symlink"
        );
        Ok(())
    }

    #[test]
    fn file_uri_is_within_root_rejects_non_file_and_foreign_uris() -> Result<(), String> {
        let root = Path::new("/workspace/ripr");
        let inside = parse_uri("file:///workspace/ripr/src/lib.rs")?;
        let outside = parse_uri("file:///workspace/other/src/lib.rs")?;
        let foreign = parse_uri("https://example.test/src/lib.rs")?;
        assert!(file_uri_is_within_root(root, &inside));
        assert!(!file_uri_is_within_root(root, &outside));
        assert!(!file_uri_is_within_root(root, &foreign));
        Ok(())
    }

    #[test]
    fn read_artifact_capped_reads_small_file_and_reports_missing() -> Result<(), String> {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ripr-uri-capped-read-{suffix}"));
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let path = dir.join("artifact.json");
        std::fs::write(&path, "{\"records\":[]}").map_err(|err| err.to_string())?;

        assert_eq!(
            read_artifact_capped(&path),
            CappedArtifactRead::Contents("{\"records\":[]}".to_string())
        );
        assert_eq!(
            read_artifact_capped(&dir.join("absent.json")),
            CappedArtifactRead::Missing
        );

        std::fs::remove_dir_all(&dir).map_err(|err| err.to_string())?;
        Ok(())
    }

    #[test]
    fn read_artifact_capped_with_limit_rejects_oversize_and_non_utf8() -> Result<(), String> {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ripr-uri-capped-limit-{suffix}"));
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let path = dir.join("artifact.json");
        std::fs::write(&path, "abcd").map_err(|err| err.to_string())?;

        // Four bytes is under any real cap but over a 3-byte test limit.
        assert_eq!(
            read_artifact_capped_with_limit(&path, 3),
            CappedArtifactRead::Unusable
        );
        assert_eq!(
            read_artifact_capped_with_limit(&path, 4),
            CappedArtifactRead::Contents("abcd".to_string())
        );

        let binary_path = dir.join("artifact.bin");
        std::fs::write(&binary_path, [0xff, 0xfe, 0xfd]).map_err(|err| err.to_string())?;
        assert_eq!(
            read_artifact_capped_with_limit(&binary_path, 1024),
            CappedArtifactRead::Unusable
        );

        std::fs::remove_dir_all(&dir).map_err(|err| err.to_string())?;
        Ok(())
    }

    #[test]
    fn local_file_uri_forms_share_path_and_identity() -> Result<(), String> {
        let canonical = parse_uri("file:///workspace/ripr/caf%C3%A9.rs")?;
        for value in [
            "file:/workspace/ripr/caf%C3%A9.rs",
            "file:///workspace/ripr/caf%C3%A9.rs",
            "file://localhost/workspace/ripr/caf%C3%A9.rs",
            "FiLe://LOCALHOST/workspace/ripr/caf%C3%A9.rs",
        ] {
            let uri = parse_uri(value)?;
            assert_eq!(
                path_from_file_uri(&uri),
                Some(PathBuf::from("/workspace/ripr/café.rs")),
                "{value}"
            );
            assert!(file_uris_match(&canonical, &uri), "{value}");
            assert!(file_uris_match(&uri, &canonical), "{value}");
        }
        Ok(())
    }

    #[test]
    fn local_file_uri_forms_preserve_windows_drive_paths() -> Result<(), String> {
        let canonical = parse_uri("file:///c:/Work/Ripr/src/lib.rs")?;
        for value in [
            "file:/C:/Work/Ripr/src/lib.rs",
            "file://localhost/C:/Work/Ripr/src/lib.rs",
            "FILE://LOCALHOST/C%3A/Work/Ripr/src/lib.rs",
        ] {
            let uri = parse_uri(value)?;
            assert_eq!(
                path_from_file_uri(&uri),
                Some(PathBuf::from(
                    ["C:", "Work", "Ripr", "src", "lib.rs"].join("/")
                )),
                "{value}"
            );
            assert!(file_uris_match(&canonical, &uri), "{value}");
            assert!(file_uris_match(&uri, &canonical), "{value}");
        }
        Ok(())
    }

    #[test]
    fn file_uri_rejects_nonlocal_authorities_before_containment() -> Result<(), String> {
        let root = std::env::temp_dir().join("ripr-uri-authority-root");
        for value in [
            "file://remote.example/src/lib.rs",
            "file://localhost.example/src/lib.rs",
            "file://127.0.0.1/src/lib.rs",
            "file://[::1]/src/lib.rs",
            "file://user@localhost/src/lib.rs",
            "file://localhost:80/src/lib.rs",
            "file://local%68ost/src/lib.rs",
        ] {
            let uri = parse_uri(value)?;
            assert_eq!(path_from_file_uri(&uri), None, "{value}");
            assert!(!file_uris_match(&uri, &uri), "{value}");
            assert!(!file_uri_is_within_root(&root, &uri), "{value}");
        }
        Ok(())
    }

    #[test]
    fn file_uri_rejects_network_share_paths_with_local_authority() -> Result<(), String> {
        let root = std::env::temp_dir().join("ripr-uri-network-share-root");
        for value in [
            "file:////remote.example/share/lib.rs",
            "file://localhost//remote.example/share/lib.rs",
            "file:///%2Fremote.example/share/lib.rs",
            "file:/%2F/remote.example/share/lib.rs",
        ] {
            let uri = parse_uri(value)?;
            assert_eq!(path_from_file_uri(&uri), None, "{value}");
            assert!(!file_uris_match(&uri, &uri), "{value}");
            assert!(!file_uri_is_within_root(&root, &uri), "{value}");
        }
        Ok(())
    }

    #[test]
    fn file_uri_rejects_parent_directory_segments() -> Result<(), String> {
        let root = std::env::temp_dir().join("ripr-uri-parent-segment-root");
        for value in [
            "file:///a/../../etc/passwd",
            "file:///tmp/../etc/passwd",
            "file://localhost/a/../../etc/passwd",
            "file:/a/../../etc/passwd",
            "file:///a/%2e%2e/etc/passwd",
            "file:///a/%2E%2E/%2e%2e/etc/passwd",
            "file:///a/..%2f..%2fetc/passwd",
            "file:///a/..%5c..%5cetc/passwd",
            "file:///workspace/src/../lib.rs",
            "file:///C:/Windows/../system.ini",
            "file:///C:/Windows/%2e%2e/system.ini",
            "file://localhost/C:/Windows/../system.ini",
        ] {
            let uri = parse_uri(value)?;
            assert_eq!(path_from_file_uri(&uri), None, "{value}");
            assert!(!file_uris_match(&uri, &uri), "{value}");
            assert!(!file_uri_is_within_root(&root, &uri), "{value}");
        }
        // Two dots inside a filename, a dotfile, and a `.` segment are not
        // parent-directory traversal. Removing the exact-segment check and
        // rejecting every `..` substring would fail these.
        for (value, path) in [
            ("file:///workspace/foo..bar.rs", "/workspace/foo..bar.rs"),
            ("file:///workspace/..hidden.rs", "/workspace/..hidden.rs"),
            ("file:///workspace/./lib.rs", "/workspace/./lib.rs"),
            ("file:///workspace/.../lib.rs", "/workspace/.../lib.rs"),
        ] {
            let uri = parse_uri(value)?;
            assert_eq!(
                path_from_file_uri(&uri),
                Some(PathBuf::from(path)),
                "{value}"
            );
        }
        let drive = ["C:", "Windows", "..", "system.ini"].join("/");
        // Built at runtime so the source does not contain a local absolute
        // Windows path token. The backslash spelling is normalized before
        // the parent segment is collapsed.
        let drive_backslash = ["C:", "Windows", "..", "system.ini"].join(r"\");
        let drive_collapsed = ["C:", "system.ini"].join("/");
        for (path, decoded) in [
            ("/a/../../etc/passwd", "/etc/passwd"),
            ("foo/../bar", "/bar"),
            (
                "/work/repo/crates/ripr/../../fixtures/boundary_gap/input/src/lib.rs",
                "/work/repo/fixtures/boundary_gap/input/src/lib.rs",
            ),
            (drive.as_str(), drive_collapsed.as_str()),
            (drive_backslash.as_str(), drive_collapsed.as_str()),
        ] {
            let uri = file_uri_for_path(Path::new(path)).map_err(|err| {
                format!("server-built parent segment must collapse: {path}: {err}")
            })?;
            assert_eq!(
                path_from_file_uri(&uri),
                Some(PathBuf::from(decoded)),
                "{path}"
            );
        }
        for path in ["../outside.rs", "foo/../../outside.rs"] {
            assert!(
                file_uri_for_path(Path::new(path)).is_err(),
                "a relative path that escapes its prefix must not be emitted: {path}"
            );
        }
        let lookalike = file_uri_for_path(Path::new("/workspace/foo..bar.rs"))
            .map_err(|err| format!("lookalike filename must still encode: {err}"))?;
        assert_eq!(
            path_from_file_uri(&lookalike),
            Some(PathBuf::from("/workspace/foo..bar.rs"))
        );
        Ok(())
    }

    #[test]
    fn file_uri_for_path_round_trips_local_paths_and_refuses_network_shares() -> Result<(), String>
    {
        // The no-network-share policy is enforced at emission too: a doubled
        // leading separator has no admitted local `file:` form, so the encoder
        // must refuse it instead of emitting a URI the shared decoder rejects.
        for path in [
            r"\\remote.example\share\src\lib.rs",
            r"//remote.example/share/src/lib.rs",
            r"\\?\UNC\remote.example\share\src\lib.rs",
            r"\\.\UNC\remote.example\share\src\lib.rs",
        ] {
            assert!(
                file_uri_for_path(Path::new(path)).is_err(),
                "network-share path must be refused, not encoded: {path}"
            );
        }
        // Every local path the encoder accepts must still decode through the
        // shared authority, on drive-absolute, rooted, and relative spellings.
        let drive_absolute = ["C:", "workspace", "ripr", "src", "lib.rs"].join("/");
        let drive_absolute_lower = drive_absolute.to_ascii_lowercase();
        for path in [
            "/workspace/ripr/src/lib.rs",
            drive_absolute.as_str(),
            drive_absolute_lower.as_str(),
            "workspace/ripr/src/lib.rs",
            "/workspace/ripr fixtures/a#b?.rs",
        ] {
            let uri = file_uri_for_path(Path::new(path))
                .map_err(|err| format!("expected a local file URI for {path}: {err}"))?;
            assert!(
                path_from_file_uri(&uri).is_some(),
                "encoder emitted a URI the shared decoder rejects: {path} -> {}",
                uri.as_str()
            );
        }
        Ok(())
    }

    #[test]
    fn path_is_within_root_refuses_relative_uri_text_fallbacks() {
        let root = Path::new("/workspace/ripr");
        for fallback in [
            "file://remote.example/workspace/ripr/src/lib.rs",
            "FILE://LOCALHOST/workspace/ripr/src/lib.rs?revision=1",
            "file:/workspace/ripr/src/lib.rs#symbol",
            "file:////remote.example/share/lib.rs",
        ] {
            let path = Path::new(fallback);
            assert!(
                path.is_relative(),
                "expected a relative fallback: {fallback}"
            );
            assert!(
                !path_is_within_root(root, path),
                "rejected-URI fallback must not read as contained: {fallback}"
            );
        }
        // A genuine workspace-relative source path still resolves under the root.
        assert!(path_is_within_root(root, Path::new("src/lib.rs")));
    }

    #[test]
    fn file_uri_rejects_missing_and_drive_relative_paths() -> Result<(), String> {
        for value in [
            "file:",
            "file://",
            "file://localhost",
            "file:src/lib.rs",
            "file:///C:",
            "file:///C:src/lib.rs",
            "file:/C:src/lib.rs",
        ] {
            let uri = parse_uri(value)?;
            assert_eq!(path_from_file_uri(&uri), None, "{value}");
            assert!(!file_uris_match(&uri, &uri), "{value}");
        }
        assert_eq!(
            path_from_file_uri(&parse_uri("file://localhost/")?),
            Some(PathBuf::from("/"))
        );
        assert_eq!(
            path_from_file_uri(&parse_uri("file://localhost/C:/")?),
            Some(PathBuf::from(["C:", ""].join("/")))
        );
        Ok(())
    }

    #[test]
    fn file_uri_rejects_query_fragment_and_nul_paths() -> Result<(), String> {
        for value in [
            "file:///workspace/lib.rs?revision=1",
            "file:///workspace/lib.rs#symbol",
            "file:///workspace/lib.rs?",
            "file:///workspace/lib.rs#",
            "file:///workspace/lib%00.rs",
            "file://localhost/workspace/lib.rs?revision=1",
        ] {
            let uri = parse_uri(value)?;
            assert_eq!(path_from_file_uri(&uri), None, "{value}");
            assert!(!file_uris_match(&uri, &uri), "{value}");
        }
        Ok(())
    }

    #[test]
    fn local_file_uri_preserves_encoded_filename_delimiters() -> Result<(), String> {
        let uri = parse_uri("file://localhost/workspace/a%23b%3F%252F%20caf%C3%A9.rs")?;
        let path = PathBuf::from("/workspace/a#b?%2F café.rs");
        assert_eq!(path_from_file_uri(&uri), Some(path.clone()));
        assert!(file_uris_match(&uri, &file_uri_for_path(&path)?));
        Ok(())
    }

    #[test]
    fn local_authority_containment_uses_the_absolute_path() -> Result<(), String> {
        let root = std::env::temp_dir().join("ripr-uri-local-root");
        let inside = file_uri_for_path(&root.join("src/lib.rs"))?;
        let outside = file_uri_for_path(&root.with_file_name("ripr-uri-other").join("lib.rs"))?;
        let local_inside = parse_uri(&inside.as_str().replacen("file://", "file://localhost", 1))?;
        let local_outside =
            parse_uri(&outside.as_str().replacen("file://", "file://localhost", 1))?;

        assert!(file_uri_is_within_root(&root, &inside));
        assert!(file_uri_is_within_root(&root, &local_inside));
        assert!(!file_uri_is_within_root(&root, &outside));
        assert!(!file_uri_is_within_root(&root, &local_outside));
        Ok(())
    }

    #[test]
    fn workspace_root_selection_uses_local_uri_authority() -> Result<(), String> {
        use crate::lsp::capabilities::{WorkspaceRootResolution, root_from_initialize_params};
        use tower_lsp_server::ls_types::{InitializeParams, WorkspaceFolder};

        let root = std::env::temp_dir().join("ripr-uri-initialize-root");
        let canonical = file_uri_for_path(&root)?;
        for authority in ["", "localhost", "LOCALHOST"] {
            let uri = parse_uri(&canonical.as_str().replacen(
                "file://",
                &format!("file://{authority}"),
                1,
            ))?;
            let params = InitializeParams {
                workspace_folders: Some(vec![WorkspaceFolder {
                    uri,
                    name: "workspace".to_string(),
                }]),
                ..InitializeParams::default()
            };
            assert_eq!(
                root_from_initialize_params(&params),
                WorkspaceRootResolution::Selected(root.clone()),
                "{authority}"
            );
        }
        let params = InitializeParams {
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: parse_uri("file://remote.example/workspace/ripr")?,
                name: "workspace".to_string(),
            }]),
            ..InitializeParams::default()
        };
        assert!(matches!(
            root_from_initialize_params(&params),
            WorkspaceRootResolution::Unavailable(_)
        ));
        Ok(())
    }
}
