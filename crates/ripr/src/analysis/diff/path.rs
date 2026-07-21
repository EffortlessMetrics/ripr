use std::path::{Component, Path, PathBuf};

pub(super) fn parse_new_path_marker(raw: &str) -> Option<PathBuf> {
    let marker = raw.strip_prefix("+++ ")?;
    let path = parse_diff_path_token(marker)?;
    if path == "/dev/null" {
        return None;
    }
    let path = path.strip_prefix("b/").unwrap_or(&path);
    confine_to_relative_path(path)
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
        Some(path) => quoted || is_plausible_unquoted_diff_path(&path),
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
/// snapshot lookup can reference the escaping path.
fn confine_to_relative_path(path: &str) -> Option<PathBuf> {
    let mut confined = PathBuf::new();
    for component in Path::new(path).components() {
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

pub(super) fn parse_old_path_marker(raw: &str) -> bool {
    let Some(marker) = raw.strip_prefix("--- ") else {
        return false;
    };
    let Some(path) = parse_diff_path_token(marker) else {
        return false;
    };
    if path == "/dev/null" {
        return true;
    }
    let path = path.strip_prefix("a/").unwrap_or(&path);
    is_plausible_unquoted_diff_path(path)
}

fn is_plausible_unquoted_diff_path(path: &str) -> bool {
    !path.is_empty() && !path.chars().any(char::is_whitespace)
}

fn parse_diff_path_token(raw: &str) -> Option<String> {
    let raw = raw.trim_end_matches('\r');
    if let Some(quoted) = raw.strip_prefix('"') {
        return parse_c_quoted_path(quoted);
    }

    let token = raw.split_once('\t').map_or(raw, |(path, _metadata)| path);
    Some(token.trim_end().to_string()).filter(|path| !path.is_empty())
}

fn parse_c_quoted_path(raw: &str) -> Option<String> {
    let mut path = String::new();
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(path),
            '\\' => path.push(parse_c_escape(&mut chars)),
            _ => path.push(ch),
        }
    }

    None
}

fn parse_c_escape<I>(chars: &mut std::iter::Peekable<I>) -> char
where
    I: Iterator<Item = char>,
{
    let Some(ch) = chars.next() else {
        return '\\';
    };

    match ch {
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        '\\' => '\\',
        '"' => '"',
        '0'..='7' => parse_octal_escape(ch, chars),
        _ => ch,
    }
}

fn parse_octal_escape<I>(first: char, chars: &mut std::iter::Peekable<I>) -> char
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

    char::from_u32(value).unwrap_or('\u{FFFD}')
}
