use std::path::{Path, PathBuf};
use tower_lsp_server::ls_types::Uri;

pub(super) fn file_uri_for_path(path: &Path) -> Result<Uri, String> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let encoded = encode_uri_path(&normalized);
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

pub(super) fn file_uris_match(left: &Uri, right: &Uri) -> bool {
    if left == right {
        return true;
    }
    let Some(left_path) = normalized_file_uri_path(left) else {
        return false;
    };
    let Some(right_path) = normalized_file_uri_path(right) else {
        return false;
    };
    if is_windows_drive_path(&left_path) && is_windows_drive_path(&right_path) {
        return left_path.eq_ignore_ascii_case(&right_path);
    }
    left_path == right_path
}

fn normalized_file_uri_path(uri: &Uri) -> Option<String> {
    let raw = uri.as_str();
    let path = raw.strip_prefix("file://")?;
    let decoded = percent_decode_uri_path(path)?;
    let path = if is_windows_drive_uri_path(&decoded) {
        decoded[1..].to_string()
    } else {
        decoded
    };
    Some(path.replace('\\', "/"))
}

fn is_windows_drive_uri_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3 && bytes[0] == b'/' && bytes[2] == b':' && bytes[1].is_ascii_alphabetic()
}

fn is_windows_drive_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

fn percent_decode_uri_path(path: &str) -> Option<String> {
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

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub(super) fn encode_uri_path(path: &str) -> String {
    let mut encoded = String::new();
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uri_for_path_percent_encodes_spaces_and_unicode() -> Result<(), String> {
        let uri = file_uri_for_path(Path::new("/tmp/ripr fixtures/über.rs"))?;

        assert_eq!(uri.as_str(), "file:///tmp/ripr%20fixtures/%C3%BCber.rs");
        Ok(())
    }

    #[test]
    fn path_from_file_uri_decodes_percent_escaped_paths() -> Result<(), String> {
        let uri: Uri = "file:///tmp/ripr%20fixtures/%C3%BCber.rs"
            .parse()
            .map_err(|err| format!("test URI should parse: {err}"))?;

        assert_eq!(
            path_from_file_uri(&uri),
            Some(PathBuf::from("/tmp/ripr fixtures/über.rs")),
        );
        Ok(())
    }

    #[test]
    fn file_uris_match_normalizes_windows_drive_case() -> Result<(), String> {
        let lower: Uri = "file:///c:/work/ripr/src/lib.rs"
            .parse()
            .map_err(|err| format!("test URI should parse: {err}"))?;
        let upper: Uri = "file:///C:/work/ripr/src/lib.rs"
            .parse()
            .map_err(|err| format!("test URI should parse: {err}"))?;

        assert!(file_uris_match(&lower, &upper));
        Ok(())
    }

    #[test]
    fn invalid_utf8_percent_escape_does_not_match() -> Result<(), String> {
        let invalid: Uri = "file:///tmp/%FF.rs".parse().map_err(|err| {
            format!("URI parser should accept structurally valid percent escapes: {err}")
        })?;
        let valid: Uri = "file:///tmp/%C3%BC.rs"
            .parse()
            .map_err(|err| format!("test URI should parse: {err}"))?;

        assert!(!file_uris_match(&invalid, &valid));
        assert_eq!(path_from_file_uri(&invalid), None);
        Ok(())
    }
}
