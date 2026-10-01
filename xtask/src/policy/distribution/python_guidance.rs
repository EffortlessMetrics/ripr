const WORKFLOW_REQUIRED_MARKERS: &[(&str, &str)] = &[
    (
        "native-version job output",
        "native_version: ${{ steps.candidate.outputs.native_version }}",
    ),
    (
        "Python-version job output",
        "python_version: ${{ steps.python_version.outputs.python_version }}",
    ),
    ("native version environment", "RIPR_NATIVE_VERSION"),
    ("Python version environment", "RIPR_PYTHON_VERSION"),
    (
        "release-candidate mapping control",
        "\"0.11.0-rc.1\": \"0.11.0rc1\"",
    ),
    (
        "PEP 440 wheel filename",
        "ripr_rs-${RIPR_PYTHON_VERSION}-py3-none-linux_x86_64.whl",
    ),
    (
        "PEP 440 metadata assertion",
        "metadata[\"Version\"] == expected_python_version",
    ),
    (
        "PEP 440 installer requirement",
        "ripr-rs==${RIPR_PYTHON_VERSION}",
    ),
    (
        "native binary version assertion",
        "ripr ${RIPR_NATIVE_VERSION} (${CANDIDATE_SHA,,})",
    ),
    (
        "pinned PEP 440 implementation",
        "packaging==${PACKAGING_VERSION}",
    ),
    (
        "missing executable negative control",
        "removed .data/scripts/ripr",
    ),
    (
        "stale RECORD negative control",
        "mutated .data/scripts/ripr without updating RECORD",
    ),
    ("RECORD digest rejection", "RECORD digest mismatch"),
    (
        "native-version receipt field",
        "\"native_version\": expected_native_version",
    ),
    (
        "Python-version receipt field",
        "\"python_version\": expected_python_version",
    ),
];

const WORKFLOW_FORBIDDEN_MARKERS: &[(&str, &str)] = &[
    (
        "native SemVer used as a Python installer requirement",
        "ripr-rs==${RIPR_NATIVE_VERSION}",
    ),
    (
        "native SemVer used in the wheel filename",
        "ripr_rs-${RIPR_NATIVE_VERSION}",
    ),
    (
        "native SemVer used as wheel metadata",
        "metadata[\"Version\"] == expected_native_version",
    ),
    ("collapsed native/Python version variable", "RIPR_VERSION"),
];

pub(super) fn validate_package_readme_commands(path: &str, text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let command = normalized_command_line(line);
        if command.is_empty() {
            continue;
        }
        let tokens = command_tokens(command);
        if tokens.is_empty() {
            continue;
        }
        if let Some(distribution) = selected_unrelated_distribution(&tokens) {
            violations.push(format!(
                "{path}:{} package README selects unrelated PyPI distribution `{distribution}` in `{command}`; use `ripr-rs` and explicit `--from ripr-rs` for one-shot uv execution",
                index + 1
            ));
        }
    }
    violations
}

pub(super) fn validate_qualification_workflow(path: &str, text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    for (label, marker) in WORKFLOW_REQUIRED_MARKERS {
        if !text.contains(marker) {
            violations.push(format!(
                "{path}: Python wheel qualification is missing {label}: `{marker}`"
            ));
        }
    }
    for (label, marker) in WORKFLOW_FORBIDDEN_MARKERS {
        if text.contains(marker) {
            violations.push(format!(
                "{path}: Python wheel qualification contains {label}: `{marker}`"
            ));
        }
    }
    violations
}

fn normalized_command_line(line: &str) -> &str {
    let trimmed = line.trim();
    let without_prompt = trimmed
        .strip_prefix("$ ")
        .or_else(|| trimmed.strip_prefix("> "))
        .unwrap_or(trimmed);
    if without_prompt.starts_with("```") {
        ""
    } else {
        without_prompt.trim()
    }
}

fn command_tokens(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .map(normalize_token)
        .filter(|token| !token.is_empty())
        .collect()
}

fn normalize_token(token: &str) -> String {
    token
        .trim_matches(|character: char| {
            matches!(
                character,
                '`' | '\'' | '"' | ',' | ';' | '(' | ')' | '{' | '}'
            )
        })
        .to_ascii_lowercase()
}

fn selected_unrelated_distribution(tokens: &[String]) -> Option<&str> {
    let first = tokens.first()?.as_str();
    if first == "pip" && token_is(tokens, 1, "install") {
        return first_unrelated_spec(tokens, 2);
    }
    if is_python_launcher(first)
        && token_is(tokens, 1, "-m")
        && token_is(tokens, 2, "pip")
        && token_is(tokens, 3, "install")
    {
        return first_unrelated_spec(tokens, 4);
    }
    if first == "pipx" && token_is(tokens, 1, "install") {
        return first_unrelated_spec(tokens, 2);
    }
    if first == "uv" && token_is(tokens, 1, "tool") && token_is(tokens, 2, "install") {
        return first_unrelated_spec(tokens, 3);
    }
    if first == "uv" && token_is(tokens, 1, "tool") && token_is(tokens, 2, "run") {
        return selected_uv_run_distribution(tokens, 3);
    }
    if first == "uvx" {
        return selected_uv_run_distribution(tokens, 1);
    }
    None
}

fn first_unrelated_spec(tokens: &[String], start: usize) -> Option<&str> {
    tokens
        .iter()
        .skip(start)
        .map(String::as_str)
        .find(|token| is_unrelated_ripr_spec(token))
}

fn selected_uv_run_distribution(tokens: &[String], start: usize) -> Option<&str> {
    let mut index = start;
    while let Some(token) = tokens.get(index).map(String::as_str) {
        if token == "--from" {
            return tokens
                .get(index + 1)
                .map(String::as_str)
                .filter(|value| is_unrelated_ripr_spec(value));
        }
        if let Some(value) = token.strip_prefix("--from=") {
            return is_unrelated_ripr_spec(value).then_some(value);
        }
        if option_takes_value(token, &UV_OPTIONS_WITH_VALUE) {
            index += 2;
            continue;
        }
        if token.starts_with('-') {
            index += 1;
            continue;
        }
        return is_unrelated_ripr_spec(token).then_some(token);
    }
    None
}

fn option_takes_value(token: &str, options_with_value: &[&str]) -> bool {
    !token.contains('=') && options_with_value.contains(&token)
}

fn token_is(tokens: &[String], index: usize, expected: &str) -> bool {
    tokens.get(index).is_some_and(|token| token == expected)
}

fn is_python_launcher(token: &str) -> bool {
    token == "python"
        || token == "python3"
        || token.strip_prefix("python3.").is_some_and(|suffix| {
            !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn is_unrelated_ripr_spec(token: &str) -> bool {
    let Some(remainder) = token.strip_prefix("ripr") else {
        return false;
    };
    matches!(
        remainder.chars().next(),
        None | Some('[' | '=' | '<' | '>' | '!' | '~' | '@')
    )
}

const UV_OPTIONS_WITH_VALUE: [&str; 13] = [
    "-p",
    "--python",
    "--python-preference",
    "--index",
    "--default-index",
    "--index-url",
    "--extra-index-url",
    "--find-links",
    "--with",
    "--with-editable",
    "--with-requirements",
    "--resolution",
    "--prerelease",
];
