use super::*;

fn read_path_allowlist(path: &str) -> Result<BTreeSet<String>, String> {
    let mut allowed = BTreeSet::new();
    let text = read_text_lossy(Path::new(path))?;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        allowed.insert(normalize_slashes(trimmed));
    }
    Ok(allowed)
}

fn read_count_allowlist(path: &str) -> Result<BTreeMap<(String, String), usize>, String> {
    let mut allowed = BTreeMap::new();
    let text = read_text_lossy(Path::new(path))?;
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = trimmed.split('|').collect::<Vec<_>>();
        if parts.len() != 4 {
            return Err(format!(
                "{path}:{} expected path|pattern|max_count|reason",
                line_number + 1
            ));
        }
        let max_count = parts[2]
            .parse::<usize>()
            .map_err(|err| format!("{path}:{} invalid max_count: {err}", line_number + 1))?;
        allowed.insert(
            (normalize_slashes(parts[0]), parts[1].to_string()),
            max_count,
        );
    }
    Ok(allowed)
}

fn read_count_policy_allowlist(path: &str) -> Result<BTreeMap<(String, String), usize>, String> {
    let mut allowed = BTreeMap::new();
    let text = read_text_lossy(Path::new(path))?;
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = trimmed.split('|').collect::<Vec<_>>();
        if parts.len() != 5 {
            return Err(format!(
                "{path}:{} expected path|pattern|max_count|owner|reason",
                line_number + 1
            ));
        }
        if parts[0].trim().is_empty()
            || parts[1].trim().is_empty()
            || parts[3].trim().is_empty()
            || parts[4].trim().is_empty()
        {
            return Err(format!(
                "{path}:{} allowlist entries require path, pattern, owner, and reason",
                line_number + 1
            ));
        }
        let max_count = parts[2]
            .parse::<usize>()
            .map_err(|err| format!("{path}:{} invalid max_count: {err}", line_number + 1))?;
        allowed.insert(
            (normalize_slashes(parts[0]), parts[1].to_string()),
            max_count,
        );
    }
    Ok(allowed)
}

fn read_local_context_allowlist(path: &str) -> Result<Vec<LocalContextAllow>, String> {
    let mut allowed = Vec::new();
    let text = read_text_lossy(Path::new(path))?;
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = trimmed.split('|').collect::<Vec<_>>();
        if parts.len() != 4 {
            return Err(format!(
                "{path}:{} expected path|pattern|max_count|reason",
                line_number + 1
            ));
        }
        if parts[0].trim().is_empty() || parts[1].trim().is_empty() || parts[3].trim().is_empty() {
            return Err(format!(
                "{path}:{} allowlist entries require path, pattern, and reason",
                line_number + 1
            ));
        }
        let max_count = parts[2]
            .parse::<usize>()
            .map_err(|err| format!("{path}:{} invalid max_count: {err}", line_number + 1))?;
        allowed.push(LocalContextAllow {
            path: normalize_slashes(parts[0].trim()),
            pattern: parts[1].trim().to_string(),
            max_count,
            line: line_number + 1,
        });
    }
    Ok(allowed)
}

fn validate_local_context_allowlist(allowlist: &[LocalContextAllow]) -> Vec<String> {
    let mut violations = Vec::new();
    for entry in allowlist {
        if !is_local_context_candidate(&entry.path) {
            violations.push(format!(
                "Path: policy/local_context_allowlist.txt\nProblem: local context allowlist entry targets a file type that is not scanned\nPattern: {}\nCount: 1, allowed: 0\nLines: {}\nWhy this matters: Local context exceptions should stay narrow and reviewable.\nRecommended fixes:\n1. Remove the stale exception.\n2. If the file should be scanned, add its extension to the checker intentionally.",
                entry.pattern, entry.line
            ));
        }
        if forbidden_local_context_allowlist_pattern(&entry.pattern) {
            violations.push(format!(
                "Path: policy/local_context_allowlist.txt\nProblem: local context allowlist tries to permit real machine or session state\nPattern: {}\nCount: 1, allowed: 0\nLines: {}\nWhy this matters: Real machine paths, Codex memory paths, and sandbox paths must be removed, not allowlisted.\nRecommended fixes:\n1. Delete the local context from the committed file.\n2. Keep only generic examples in durable docs.",
                entry.pattern, entry.line
            ));
        }
    }
    violations
}

fn forbidden_local_context_allowlist_pattern(pattern: &str) -> bool {
    let lower = pattern.to_ascii_lowercase();
    if lower.contains(concat!(".", "codex"))
        || lower.contains(concat!("memory", ".md"))
        || lower.contains(concat!("sandbox:", "/mnt", "/data"))
        || lower.contains(concat!("/mnt", "/data"))
        || lower.contains(concat!("contentreference", "[oaicite"))
    {
        return true;
    }
    for token in windows_absolute_path_tokens(pattern) {
        let generic_example = token
            .to_ascii_lowercase()
            .replace('/', "\\")
            .contains(concat!(":\\", "path", "\\to\\"));
        if !generic_example {
            return true;
        }
    }
    !unix_home_path_tokens(pattern).is_empty()
}

fn read_glob_allowlist(path: &str) -> Result<Vec<GlobAllow>, String> {
    let mut allowed = Vec::new();
    let text = read_text_lossy(Path::new(path))?;
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = trimmed.split('|').collect::<Vec<_>>();
        if parts.len() != 4 {
            return Err(format!(
                "{path}:{} expected glob|kind|owner|reason",
                line_number + 1
            ));
        }
        let entry = GlobAllow {
            glob: normalize_slashes(parts[0]),
        };
        if entry.glob.is_empty()
            || parts[1].trim().is_empty()
            || parts[2].trim().is_empty()
            || parts[3].trim().is_empty()
        {
            return Err(format!(
                "{path}:{} allowlist entries require glob, kind, owner, and reason",
                line_number + 1
            ));
        }
        allowed.push(entry);
    }
    Ok(allowed)
}

pub(crate) fn read_file_policy_allowlist(path: &str) -> Result<Vec<GlobAllow>, String> {
    let entries = parse_file_policy_allowlist(path)?;
    Ok(entries
        .into_iter()
        .map(|entry| GlobAllow {
            glob: normalize_slashes(&entry.glob.unwrap_or_default()),
        })
        .collect())
}

fn parse_file_policy_allowlist(path: &str) -> Result<Vec<FilePolicyAllowEntry>, String> {
    let text = read_text_lossy(Path::new(path))?;
    let mut entries = Vec::new();
    let mut current = FilePolicyAllowEntry::default();
    let mut in_entry = false;

    let lines = text.lines().collect::<Vec<_>>();
    let mut idx = 0;
    while idx < lines.len() {
        let line_number = idx + 1;
        let trimmed = lines[idx].trim();
        idx += 1;
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "[[allow]]" {
            if in_entry {
                validate_file_policy_allow_entry(path, &current)?;
                entries.push(current);
            }
            current = FilePolicyAllowEntry {
                line: line_number,
                ..FilePolicyAllowEntry::default()
            };
            in_entry = true;
            continue;
        }
        let Some((key, value)) = parse_toml_key_value(trimmed) else {
            continue;
        };
        if !in_entry {
            continue;
        }
        match key {
            "glob" => current.glob = Some(parse_string_value(value, path, line_number)?),
            "kind" => current.kind = Some(parse_string_value(value, path, line_number)?),
            "owner" => current.owner = Some(parse_string_value(value, path, line_number)?),
            "surface" => current.surface = Some(parse_string_value(value, path, line_number)?),
            "classification" => {
                current.classification = Some(parse_string_value(value, path, line_number)?)
            }
            "reason" => current.reason = Some(parse_string_value(value, path, line_number)?),
            "generated_by" => {
                current.generated_by = Some(parse_string_value(value, path, line_number)?)
            }
            "covered_by" => {
                let value = collect_toml_array_value(path, line_number, value, &lines, &mut idx)?;
                current.covered_by = Some(parse_inline_array(&value)?);
            }
            "expires" | "retired" => {}
            other => {
                return Err(format!(
                    "{path}:{line_number} unsupported non-Rust allowlist field `{other}`"
                ));
            }
        }
    }

    if in_entry {
        validate_file_policy_allow_entry(path, &current)?;
        entries.push(current);
    }
    if entries.is_empty() {
        return Err(format!("{path} has no [[allow]] entries"));
    }
    Ok(entries)
}

fn collect_toml_array_value(
    path: &str,
    line_number: usize,
    first_value: &str,
    lines: &[&str],
    idx: &mut usize,
) -> Result<String, String> {
    let mut value = first_value.trim().to_string();
    if !value.starts_with('[') || value.ends_with(']') {
        return Ok(value);
    }
    while *idx < lines.len() {
        let next = lines[*idx].trim();
        *idx += 1;
        value.push(' ');
        value.push_str(next);
        if next.ends_with(']') {
            return Ok(value);
        }
    }
    Err(format!(
        "{path}:{line_number} unterminated non-Rust allowlist array"
    ))
}

fn validate_file_policy_allow_entry(
    path: &str,
    entry: &FilePolicyAllowEntry,
) -> Result<(), String> {
    let required = [
        ("glob", entry.glob.as_deref()),
        ("kind", entry.kind.as_deref()),
        ("owner", entry.owner.as_deref()),
        ("surface", entry.surface.as_deref()),
        ("classification", entry.classification.as_deref()),
        ("reason", entry.reason.as_deref()),
    ];
    for (field, value) in required {
        if value.unwrap_or_default().trim().is_empty() {
            return Err(format!(
                "{path}:{} non-Rust allowlist entry requires `{field}`",
                entry.line
            ));
        }
    }
    let covered_by = entry.covered_by.as_ref().ok_or_else(|| {
        format!(
            "{path}:{} non-Rust allowlist entry requires `covered_by`",
            entry.line
        )
    })?;
    if covered_by.iter().any(|value| value.trim().is_empty()) {
        return Err(format!(
            "{path}:{} non-Rust allowlist `covered_by` values must be non-empty",
            entry.line
        ));
    }
    Ok(())
}

fn read_workflow_budgets(path: &str) -> Result<BTreeMap<String, WorkflowBudget>, String> {
    let mut budgets = BTreeMap::new();
    let text = read_text_lossy(Path::new(path))?;
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = trimmed.split('|').collect::<Vec<_>>();
        if parts.len() != 3 {
            return Err(format!(
                "{path}:{} expected path|max_non_empty_lines|reason",
                line_number + 1
            ));
        }
        let max_non_empty_lines = parts[1].parse::<usize>().map_err(|err| {
            format!(
                "{path}:{} invalid max_non_empty_lines: {err}",
                line_number + 1
            )
        })?;
        let budget = WorkflowBudget {
            path: normalize_slashes(parts[0]),
            max_non_empty_lines,
            reason: parts[2].trim().to_string(),
        };
        if budget.reason.is_empty() {
            return Err(format!(
                "{path}:{} reason must not be empty",
                line_number + 1
            ));
        }
        budgets.insert(budget.path.clone(), budget);
    }
    Ok(budgets)
}

fn read_path_allowlist_optional(path: &str) -> Result<BTreeSet<String>, String> {
    if Path::new(path).exists() {
        read_path_allowlist(path)
    } else {
        Ok(BTreeSet::new())
    }
}

fn spec_id_from_file_name(file_name: &str) -> Option<String> {

pub(crate) fn collect_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_files_inner(root, root, &mut files)?;
    Ok(files)
}

fn collect_files_inner(root: &Path, path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let normalized = normalize_path(path);
    let relative = path.strip_prefix(root).unwrap_or(path);
    let relative_normalized = normalize_path(relative);
    if should_skip_path(&relative_normalized) {
        return Ok(());
    }
    let metadata =
        fs::metadata(path).map_err(|err| format!("failed to inspect {normalized}: {err}"))?;
    if metadata.is_file() {
        files.push(path.to_path_buf());
        return Ok(());
    }
    if metadata.is_dir() {
        for entry in
            fs::read_dir(path).map_err(|err| format!("failed to read {normalized}: {err}"))?
        {
            let entry = entry.map_err(|err| format!("failed to read {normalized}: {err}"))?;
            collect_files_inner(root, &entry.path(), files)?;
        }
    }
    Ok(())
}

fn tracked_files() -> Result<Vec<String>, String> {
    let output = run_output("git", &["ls-files"])?;
    Ok(output
        .lines()
        .map(normalize_slashes)
        .filter(|path| !path.is_empty())
        .collect())
}

fn should_skip_path(path: &str) -> bool {
    path == ".git"
        || path.starts_with(".git/")
        || path == ".claude"
        || path.starts_with(".claude/")
        || path == "target"
        || path.starts_with("target/")
        || path.ends_with("/target")
        || path.contains("/target/")
        || path == ".ripr/release"
        || path.starts_with(".ripr/release/")
        || path.ends_with("/.vscode-test")
        || path.contains("/.vscode-test/")
        || path.ends_with("/node_modules")
        || path.contains("/node_modules/")
        || path.ends_with("/out")
        || path.contains("/out/")
        || path.ends_with("/dist")
        || path.contains("/dist/")
}

fn is_static_language_candidate(path: &str) -> bool {
    // Skip fixture CHANGELOG files (#2338): these contain bless reasons that
    // may legitimately reference banned words (e.g. "reword 'established'"). They
    // are bookkeeping, not product output or source prose.
    if path.contains("/expected/CHANGELOG.md") || path == "expected/CHANGELOG.md" {
        return false;
    }
    let extensions = [".md", ".rs", ".txt", ".json", ".toml", ".yml", ".yaml"];
    extensions.iter().any(|extension| path.ends_with(extension))
}

pub(crate) fn read_text_lossy(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn guarded_allow_attribute_lints() -> BTreeSet<&'static str> {
    [
        "clippy::unwrap_used",
        "clippy::expect_used",
        "clippy::panic",
        "clippy::todo",
        "clippy::unimplemented",
        "clippy::dbg_macro",
        "unwrap_used",
        "expect_used",
        "panic",
        "todo",
        "unimplemented",
        "dbg_macro",
        "unsafe_code",
        "dead_code",
        "unused_imports",
        "unused_variables",
        "warnings",
    ]
    .into_iter()
    .collect()
}

fn guarded_allow_attributes_in_text(
    text: &str,
    guarded: &BTreeSet<&'static str>,
) -> Vec<(usize, String)> {
    let bytes = text.as_bytes();
    let mut findings = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'#' {
            index += 1;
            continue;
        }

        let line = byte_line_number(text, index);
        let mut cursor = index + 1;
        if cursor < bytes.len() && bytes[cursor] == b'!' {
            cursor += 1;
        }
        cursor = skip_ascii_whitespace(bytes, cursor);
        if cursor >= bytes.len() || bytes[cursor] != b'[' {
            index += 1;
            continue;
        }
        cursor += 1;
        cursor = skip_ascii_whitespace(bytes, cursor);

        let ident_start = cursor;
        while cursor < bytes.len() && (bytes[cursor].is_ascii_alphabetic() || bytes[cursor] == b'_')
        {
            cursor += 1;
        }
        let kind = &text[ident_start..cursor];
        if kind != "allow" && kind != "expect" {
            index += 1;
            continue;
        }
        cursor = skip_ascii_whitespace(bytes, cursor);
        if cursor >= bytes.len() || bytes[cursor] != b'(' {
            index += 1;
            continue;
        }

        let Some((content_start, content_end, next_index)) = attribute_paren_span(bytes, cursor)
        else {
            index += 1;
            continue;
        };
        for lint in attribute_lints(&text[content_start..content_end]) {
            if guarded.contains(lint.as_str()) {
                findings.push((line, format!("{kind}({lint})")));
            }
        }
        index = next_index;
    }
    findings
}

fn attribute_paren_span(bytes: &[u8], open: usize) -> Option<(usize, usize, usize)> {
    let mut depth = 0usize;
    let mut index = open;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some((open + 1, index, index + 1));
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn attribute_lints(content: &str) -> Vec<String> {
    content
        .split(',')
        .filter_map(|part| {
            let lint = part.trim();
            if lint.is_empty() || lint.contains('=') {
                None
            } else {
                Some(lint.to_string())
            }
        })
        .collect()
}

fn attribute_lint_name(attribute: &str) -> Option<&str> {
    let (_, rest) = attribute.split_once('(')?;
    Some(rest.strip_suffix(')').unwrap_or(rest).trim())
}

fn skip_ascii_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn byte_line_number(text: &str, byte_index: usize) -> usize {
    text.as_bytes()[..byte_index]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

fn allow_attribute_line_summary(lines: &[usize]) -> String {
    let mut unique = lines.to_vec();
    unique.sort_unstable();
    unique.dedup();
    unique
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn local_context_findings_for_path(path: &str) -> Result<Vec<LocalContextFinding>, String> {
    let mut findings = Vec::new();
    let Some(file_name) = path.rsplit('/').next() else {
        return Ok(findings);
    };

    if suspicious_runtime_file_names()
        .iter()
        .any(|name| file_name.eq_ignore_ascii_case(name))
    {
        findings.push(LocalContextFinding {
            path: path.to_string(),
            line: None,
            pattern: file_name.to_string(),
            problem: "committed runtime/session artifact filename".to_string(),
        });
    }

    if !is_local_context_candidate(path) {
        return Ok(findings);
    }

    let text = read_text_lossy(Path::new(path))?;
    for (line_index, line) in text.lines().enumerate() {
        for (pattern, problem) in local_context_line_findings(line) {
            findings.push(LocalContextFinding {
                path: path.to_string(),
                line: Some(line_index + 1),
                pattern,
                problem,
            });
        }
    }
    Ok(findings)
}

fn local_context_line_findings(line: &str) -> Vec<(String, String)> {
    let mut findings = BTreeSet::<(String, String)>::new();

    for token in windows_absolute_path_tokens(line) {
        findings.insert((token, "local absolute Windows path".to_string()));
    }
    for token in unix_home_path_tokens(line) {
        findings.insert((token, "local absolute Unix home path".to_string()));
    }

    let lower = line.to_ascii_lowercase();
    for (marker, problem) in local_context_markers() {
        if lower.contains(&marker.to_ascii_lowercase()) {
            findings.insert((marker, problem));
        }
    }

    if contains_recorded_date(line) {
        findings.insert((
            recorded_on_pattern().to_string(),
            "session timestamp language".to_string(),
        ));
    }
    if lower.contains(concat!("working tree", " is dirty before")) {
        findings.insert((
            concat!("working tree", " is dirty before").to_string(),
            "transient local worktree state".to_string(),
        ));
    }
    if lower.contains(concat!("before any", " codex edits")) {
        findings.insert((
            concat!("before any", " Codex edits").to_string(),
            "transient Codex session state".to_string(),
        ));
    }
    if lower.contains(concat!("current local", " state")) {
        findings.insert((
            concat!("current local", " state").to_string(),
            "transient local state language".to_string(),
        ));
    }
    if lower.contains(concat!("current", " branch:")) {
        findings.insert((
            concat!("Current", " branch:").to_string(),
            "transient local branch state".to_string(),
        ));
    }

    for token in file_reference_tokens(line) {
        let problem = if token.starts_with("file_") {
            "opaque uploaded file artifact reference"
        } else {
            "chat transcript file reference"
        };
        findings.insert((token, problem.to_string()));
    }

    findings.into_iter().collect()
}

fn local_context_markers() -> Vec<(String, String)> {
    vec![
        (
            concat!(".", "codex").to_string(),
            "Codex local memory path".to_string(),
        ),
        (
            concat!("MEMORY", ".md").to_string(),
            "Codex memory artifact".to_string(),
        ),
        (
            concat!("sandbox:", "/mnt", "/data").to_string(),
            "sandbox runtime path".to_string(),
        ),
        (
            concat!("/mnt", "/data/").to_string(),
            "sandbox runtime path".to_string(),
        ),
        (
            concat!("contentReference", "[oaicite").to_string(),
            "chat citation artifact".to_string(),
        ),
    ]
}

fn suspicious_runtime_file_names() -> Vec<String> {
    vec![
        concat!("CURRENT", "_STATE.md").to_string(),
        concat!("SESSION", "_STATE.md").to_string(),
        "SCRATCHPAD.md".to_string(),
        concat!("NOTES", "_FROM", "_RUN.md").to_string(),
        concat!("CODEX", "_STATE.md").to_string(),
        concat!("codex", "-", "memory", ".md").to_string(),
        "transcript.md".to_string(),
        "chat.md".to_string(),
    ]
}

fn windows_absolute_path_tokens(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index + 2 < bytes.len() {
        let token_boundary = index == 0 || is_local_context_token_delimiter(bytes[index - 1]);
        if token_boundary
            && bytes[index].is_ascii_alphabetic()
            && bytes[index + 1] == b':'
            && (bytes[index + 2] == b'\\' || bytes[index + 2] == b'/')
        {
            let start = index;
            index += 3;
            while index < bytes.len() && !is_local_context_token_delimiter(bytes[index]) {
                index += 1;
            }
            tokens.push(line[start..index].to_string());
        } else {
            index += 1;
        }
    }
    tokens
}

fn unix_home_path_tokens(line: &str) -> Vec<String> {
    ["/Users/", "/home/"]
        .iter()
        .flat_map(|prefix| absolute_path_tokens_with_prefix(line, prefix))
        .collect()
}

fn absolute_path_tokens_with_prefix(line: &str, prefix: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut search_start = 0;
    while let Some(offset) = line[search_start..].find(prefix) {
        let start = search_start + offset;
        let mut end = start + prefix.len();
        let bytes = line.as_bytes();
        let name_start = end;
        while end < line.len()
            && bytes[end] != b'/'
            && !is_local_context_token_delimiter(bytes[end])
        {
            end += 1;
        }
        if end == name_start || end >= line.len() || bytes[end] != b'/' {
            search_start = end.max(start + prefix.len());
            continue;
        }
        end += 1;
        while end < line.len() && !is_local_context_token_delimiter(bytes[end]) {
            end += 1;
        }
        tokens.push(line[start..end].to_string());
        search_start = end;
    }
    tokens
}

fn is_local_context_token_delimiter(byte: u8) -> bool {
    byte.is_ascii_whitespace()
        || matches!(
            byte,
            b'`' | b'"' | b'\'' | b')' | b']' | b'}' | b'<' | b'>' | b',' | b';'
        )
}

fn contains_recorded_date(line: &str) -> bool {
    let marker = recorded_on_marker();
    let Some(offset) = line.find(marker) else {
        return false;
    };
    let date = &line[offset + marker.len()..];
    date.len() >= 10
        && date.as_bytes()[0..4].iter().all(u8::is_ascii_digit)
        && date.as_bytes()[4] == b'-'
        && date.as_bytes()[5..7].iter().all(u8::is_ascii_digit)
        && date.as_bytes()[7] == b'-'
        && date.as_bytes()[8..10].iter().all(u8::is_ascii_digit)
}

fn recorded_on_marker() -> &'static str {
    concat!("Recorded", " on ")
}

fn recorded_on_pattern() -> &'static str {
    concat!("Recorded", " on <date>")
}

fn file_reference_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"file_") {
            let start = index;
            index += "file_".len();
            let hex_start = index;
            while index < bytes.len() && bytes[index].is_ascii_hexdigit() {
                index += 1;
            }
            if index - hex_start >= 8 {
                tokens.push(line[start..index].to_string());
            }
            continue;
        }
        if bytes[index..].starts_with(b"turn") {
            let start = index;
            index += "turn".len();
            let digit_start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index > digit_start && bytes[index..].starts_with(b"file") {
                index += "file".len();
                let file_digit_start = index;
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    index += 1;
                }
                if index > file_digit_start {
                    tokens.push(line[start..index].to_string());
                    continue;
                }
            }
            index = start + 1;
            continue;
        }
        index += 1;
    }
    tokens
}

fn local_context_line_summary(lines: &[Option<usize>]) -> String {
    let mut concrete = lines.iter().flatten().copied().collect::<Vec<_>>();
    concrete.sort_unstable();
    concrete.dedup();
    if concrete.is_empty() {
        "file name".to_string()
    } else {
        concrete
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn is_local_context_candidate(path: &str) -> bool {
    let extensions = [
        ".md", ".rs", ".txt", ".json", ".toml", ".yml", ".yaml", ".ts", ".tsx",
    ];
    extensions.iter().any(|extension| path.ends_with(extension))
}

fn write_local_context_json(violations: &[String]) -> Result<(), String> {
    let status = if violations.is_empty() {
        "pass"
    } else {
        "fail"
    };
    let mut body = format!(
        "{{\n  \"schema_version\": \"0.1\",\n  \"status\": \"{status}\",\n  \"violation_count\": {},\n  \"violations\": [",
        violations.len()
    );
    for (index, violation) in violations.iter().enumerate() {
        if index > 0 {
            body.push(',');
        }
        body.push_str("\n    \"");
        body.push_str(&json_escape(violation));
        body.push('"');
    }
    if !violations.is_empty() {
        body.push('\n');
    }
    body.push_str("  ]\n}\n");
    write_report("local-context.json", &body)
}

pub(crate) fn normalize_path(path: &Path) -> String {
    normalize_slashes(&path.to_string_lossy())
        .trim_start_matches("./")
        .to_string()
}

fn normalize_slashes(value: &str) -> String {
    value.replace('\\', "/")
}

pub(crate) fn is_file_policy_candidate(path: &str) -> bool {
    let extensions = [
        ".bash", ".c", ".cjs", ".cpp", ".cs", ".go", ".h", ".hpp", ".java", ".js", ".json", ".kt",
        ".lua", ".mjs", ".php", ".pl", ".ps1", ".py", ".rb", ".sh", ".swift", ".toml", ".ts",
        ".tsx", ".yaml", ".yml", ".zsh",
    ];
    extensions.iter().any(|extension| path.ends_with(extension))
}

pub(crate) fn is_non_rust_programming_candidate(path: &str) -> bool {
    let extensions = [
        ".bash", ".c", ".cjs", ".cpp", ".cs", ".go", ".h", ".hpp", ".java", ".js", ".kt", ".lua",
        ".mjs", ".php", ".pl", ".ps1", ".py", ".rb", ".sh", ".swift", ".ts", ".tsx", ".zsh",
    ];
    extensions.iter().any(|extension| path.ends_with(extension))
}

pub(crate) fn non_rust_programming_retention_reason(path: &str) -> Option<&'static str> {
    if path.starts_with("editors/vscode/") && path.ends_with(".ts") {
        return Some(
            "VS Code extension source and tests must run in the VS Code Extension Host TypeScript API.",
        );
    }

    if path.starts_with("fixtures/")
        && (path.ends_with(".ts")
            || path.ends_with(".tsx")
            || path.ends_with(".js")
            || path.ends_with(".jsx")
            || path.ends_with(".py"))
    {
        return Some(
            "Fixture workspaces may contain TypeScript / JavaScript / Python source as analyzed inputs for the Campaign 27 preview adapters (RIPR-SPEC-0027 / RIPR-SPEC-0028).",
        );
    }

    None
}

fn is_generated_candidate(path: &str) -> bool {
    path == "Cargo.lock"
        || path.ends_with("/package-lock.json")
        || path == "package-lock.json"
        || path.starts_with("target/")
        || path.contains("/target/")
        || path.starts_with(".ripr/release/")
        || path.starts_with("dist/")
        || path.contains("/dist/")
        || path.ends_with(".vsix")
        || path.ends_with(".zip")
        || path.ends_with(".tar.gz")
        || path.ends_with(".sha256")
}

fn is_dependency_surface_candidate(path: &str) -> bool {
    let Some(file_name) = path.rsplit('/').next() else {
        return false;
    };
    matches!(
        file_name,
        "Cargo.toml"
            | "Cargo.lock"
            | "package.json"
            | "package-lock.json"
            | "npm-shrinkwrap.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "requirements.txt"
            | "pyproject.toml"
            | "poetry.lock"
            | "Pipfile"
            | "Pipfile.lock"
            | "go.mod"
            | "go.sum"
            | "pom.xml"
            | "build.gradle"
            | "settings.gradle"
            | "gradle.lockfile"
            | "Gemfile"
            | "Gemfile.lock"
    )
}

fn is_process_policy_candidate(path: &str) -> bool {
    path.ends_with(".rs") || path.ends_with(".ts")
}

fn is_network_policy_candidate(path: &str) -> bool {
    path.ends_with(".rs")
        || path.ends_with(".ts")
        || path.ends_with(".py")
        || path.ends_with(".js")
        || path.ends_with(".sh")
        || path.ends_with(".ps1")
        || path.ends_with(".yml")
        || path.ends_with(".yaml")
}

fn process_policy_patterns() -> Vec<String> {
    [
        concat!("use std::process::", "Command"),
        concat!("Command", "::new"),
        concat!("child", "_process"),
        concat!("cp.", "spawn"),
        concat!("cp.", "exec("),
        concat!("cp.", "execFile"),
    ]
    .iter()
    .map(|value| value.to_string())
    .collect()
}

fn network_policy_patterns() -> Vec<String> {
    [
        // Original patterns.
        concat!("https", ".get"),
        concat!("fetch", "("),
        concat!("req", "west"),
        concat!("u", "req"),
        concat!("Tcp", "Stream"),
        concat!("cu", "rl"),
        concat!("w", "get"),
        // Expanded patterns (#2412): cover common Rust/JS networking crates
        // that were previously invisible to the gate. Split with concat! so
        // the gate does not flag its own source (same technique as the
        // original patterns above).
        concat!("hy", "per"),
        concat!("isa", "hc"),
        concat!("atto", "httpc"),
        concat!("min", "req"),
        concat!("tokio::", "net"),
        concat!("std::net::", "Tcp"),
        concat!("to", "nic::"),
        concat!("tungste", "nite"),
        concat!("ssh", "2::"),
        concat!("req", "west::Client"),
    ]
    .iter()
    .map(|value| value.to_string())
    .collect()
}

fn shell_fetch_tool_name() -> &'static str {
    concat!("cu", "rl")
}

pub(crate) fn matches_any_glob(allowlist: &[GlobAllow], path: &str) -> bool {
    allowlist
        .iter()
        .any(|entry| glob_matches(&entry.glob, path))
}

fn glob_matches(pattern: &str, path: &str) -> bool {
    let pattern_parts = pattern.split('/').collect::<Vec<_>>();
    let path_parts = path.split('/').collect::<Vec<_>>();
    glob_parts_match(&pattern_parts, &path_parts)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StaticLanguageMatcher {
