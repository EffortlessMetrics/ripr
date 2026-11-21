//! Static test-inventory resolution for test-valued `covered_by` claims
//! (issue #3528).
//!
//! A `covered_by = ["cargo test -p <pkg> <filter>"]` entry in a policy ledger
//! is a claim that a named test exists and exercises the suppressed surface.
//! When the test is renamed or deleted the entry becomes a false-confidence
//! receipt. This module enumerates the workspace's actual tests with a
//! bounded static scan (`#[test]`-family attributes under workspace member
//! `src/` and `tests/` trees) so a gate can resolve each cited filter without
//! invoking Cargo: the compiled-binary enumeration path is unreliable on
//! Windows (stale/invalid cached test artifacts under AV interference) and
//! far too heavy for a policy gate.
//!
//! Naivety bounds (accepted, line-based like the repo's other static policy
//! scans): string literals and comments that look like module or test items
//! can add phantom inventory names (lenient direction), and exotic module
//! declarations (`#[path]`, several `mod` opens on one line, brace placement
//! other than `mod <ident> {`) can distort module paths. The attribute set
//! covers `#[test]`, `#[tokio::test]`, `#[rstest]`, `#[test_case(...)]`, and
//! `#[test_matrix(...)]` forms seen in this workspace.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::{collect_files, read_text_lossy};

/// Flags that take a separate value token in a `cargo test` invocation.
const CARGO_TEST_VALUE_FLAGS: &[&str] = &[
    "--features",
    "--target",
    "--profile",
    "--manifest-path",
    "--test",
    "--bin",
    "--bench",
    "--example",
];

/// Flags that stand alone in a `cargo test` invocation.
const CARGO_TEST_BARE_FLAGS: &[&str] = &[
    "--locked",
    "--offline",
    "--release",
    "--all-features",
    "--no-default-features",
    "--no-fail-fast",
    "--all-targets",
    "--lib",
    "--bins",
    "--tests",
    "--doc",
    "--benches",
    "--examples",
    "--workspace",
];

/// A `cargo test ...` command reduced to what a static inventory can resolve:
/// the selected package (`-p` / `--package`, if any) and the positional
/// filter tokens. Everything after `--` targets the test binary and is
/// ignored, mirroring `cargo test <filters> -- <binary args>`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CargoTestSelection {
    pub(crate) package: Option<String>,
    pub(crate) filters: Vec<String>,
}

/// Parse a test-valued `covered_by` command. Unknown flags fail closed so a
/// ledger entry cannot smuggle an unmodeled argument past the resolver.
pub(crate) fn parse_cargo_test_command(command: &str) -> Result<CargoTestSelection, String> {
    let mut words = command.split_whitespace();
    if words.next() != Some("cargo") || words.next() != Some("test") {
        return Err("command is not a `cargo test` invocation".to_string());
    }
    let mut package: Option<String> = None;
    let mut filters = Vec::new();
    let mut pending_value_flag: Option<&str> = None;
    for word in words {
        if let Some(flag) = pending_value_flag.take() {
            if flag == "-p" || flag == "--package" {
                package = Some(word.to_string());
            }
            continue;
        }
        if word == "--" {
            break;
        }
        if let Some(inline) = word
            .strip_prefix("--")
            .and_then(|rest| rest.split_once('='))
        {
            let (name, value) = inline;
            match name {
                "package" => package = Some(value.to_string()),
                "features" | "target" | "profile" | "manifest-path" | "test" | "bin" | "bench"
                | "example" => {}
                other => return Err(format!("unsupported `cargo test` flag `--{other}`")),
            }
            continue;
        }
        if let Some(short) = word.strip_prefix("-p=") {
            package = Some(short.to_string());
            continue;
        }
        if word == "-p" {
            pending_value_flag = Some("-p");
            continue;
        }
        if word.starts_with("--") {
            if CARGO_TEST_BARE_FLAGS.contains(&word) {
                continue;
            }
            if CARGO_TEST_VALUE_FLAGS.contains(&word) {
                pending_value_flag = Some(word);
                continue;
            }
            return Err(format!("unsupported `cargo test` flag `{word}`"));
        }
        if word.starts_with('-') {
            return Err(format!("unsupported `cargo test` flag `{word}`"));
        }
        filters.push(word.to_string());
    }
    if pending_value_flag.is_some() {
        return Err("`cargo test` command ends with a flag that requires a value".to_string());
    }
    Ok(CargoTestSelection { package, filters })
}

/// Fully-qualified test names grouped by workspace package, plus the package
/// scope a bare `cargo test` (no `-p`) selects.
pub(crate) struct TestInventory {
    packages: BTreeMap<String, Vec<String>>,
    default_scope: Vec<String>,
}

impl TestInventory {
    /// Scan the workspace rooted at `root` (the repository root in
    /// production) and enumerate every statically visible test name per
    /// package. Fails closed when the workspace shape cannot be read.
    pub(crate) fn scan_workspace(root: &Path) -> Result<TestInventory, String> {
        let cargo_toml = read_text_lossy(&root.join("Cargo.toml"))
            .map_err(|error| format!("read workspace Cargo.toml: {error}"))?;
        let members = read_string_array(&cargo_toml, "members")?;
        if members.is_empty() {
            return Err(
                "workspace Cargo.toml declares no `members`; cannot enumerate tests".to_string(),
            );
        }
        let default_members = read_string_array(&cargo_toml, "default-members")?;

        let mut packages = BTreeMap::new();
        for member in &members {
            for dir in resolve_member_dirs(root, member)? {
                let name = read_package_name(&dir)?;
                let names = scan_package_tests(&dir)?;
                if packages.insert(name.clone(), names).is_some() {
                    return Err(format!(
                        "workspace declares package `{name}` more than once via members"
                    ));
                }
            }
        }
        let scope_members = if default_members.is_empty() {
            members
        } else {
            default_members
        };
        let mut default_scope = Vec::new();
        for member in &scope_members {
            for dir in resolve_member_dirs(root, member)? {
                default_scope.push(read_package_name(&dir)?);
            }
        }
        Ok(TestInventory {
            packages,
            default_scope,
        })
    }

    /// A test-only constructor so resolution tests can pin inventories
    /// without building a workspace fixture on disk.
    #[cfg(test)]
    pub(crate) fn from_parts(
        packages: BTreeMap<String, Vec<String>>,
        default_scope: Vec<String>,
    ) -> TestInventory {
        TestInventory {
            packages,
            default_scope,
        }
    }

    /// True when the static scan found no tests at all. That is a scanner or
    /// checkout failure, not evidence that ledger claims are stale, so the
    /// gate refuses to validate against an empty denominator.
    pub(crate) fn is_empty(&self) -> bool {
        self.packages.values().all(|names| names.is_empty())
    }

    /// Resolve every filter of a test-valued command against the selected
    /// scope. Each filter token is a claim that the filter alone selects the
    /// covered test, so every token must match at least one enumerated name.
    pub(crate) fn resolve(&self, selection: &CargoTestSelection) -> Result<(), String> {
        let scope: Vec<&Vec<String>> = match &selection.package {
            Some(package) => match self.packages.get(package) {
                Some(names) => vec![names],
                None => {
                    let known = self.packages.keys().cloned().collect::<Vec<_>>().join(", ");
                    return Err(format!(
                        "workspace has no package `{package}` (packages: {known})"
                    ));
                }
            },
            None => self
                .default_scope
                .iter()
                .filter_map(|name| self.packages.get(name))
                .collect(),
        };
        let scope_label = match &selection.package {
            Some(package) => format!("package `{package}`"),
            None => format!(
                "workspace default scope ({})",
                self.default_scope.join(", ")
            ),
        };
        for filter in &selection.filters {
            let matched = scope
                .iter()
                .any(|names| names.iter().any(|name| name.contains(filter.as_str())));
            if !matched {
                return Err(format!(
                    "filter `{filter}` matches no statically enumerated test in {scope_label}"
                ));
            }
        }
        Ok(())
    }
}

/// Expand one `members` entry to member directories. Supports exact paths and
/// the `dir/*` glob shape; anything else fails closed.
fn resolve_member_dirs(root: &Path, member: &str) -> Result<Vec<PathBuf>, String> {
    let normalized = member.replace('\\', "/");
    if let Some(parent) = normalized.strip_suffix("/*") {
        let parent_dir = root.join(parent);
        let mut dirs = Vec::new();
        let entries = std::fs::read_dir(&parent_dir)
            .map_err(|error| format!("read workspace member glob `{member}`: {error}"))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("Cargo.toml").is_file() {
                dirs.push(path);
            }
        }
        dirs.sort();
        if dirs.is_empty() {
            return Err(format!("workspace member glob `{member}` matched no crate"));
        }
        return Ok(dirs);
    }
    if normalized.contains('*') {
        return Err(format!(
            "workspace member `{member}` uses an unsupported glob; only `dir/*` is supported"
        ));
    }
    let dir = root.join(&normalized);
    if !dir.join("Cargo.toml").is_file() {
        return Err(format!(
            "workspace member `{member}` has no Cargo.toml at {}",
            dir.join("Cargo.toml").display()
        ));
    }
    Ok(vec![dir])
}

/// Read the `[package] name` field of a member manifest.
fn read_package_name(dir: &Path) -> Result<String, String> {
    let manifest = read_text_lossy(&dir.join("Cargo.toml"))
        .map_err(|error| format!("read member Cargo.toml at {}: {error}", dir.display()))?;
    let mut section = String::new();
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = trimmed[1..trimmed.len() - 1].trim().to_string();
            continue;
        }
        if section != "package" {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=')
            && key.trim() == "name"
        {
            let name = value.trim().trim_matches('"').to_string();
            if name.is_empty() {
                break;
            }
            return Ok(name);
        }
    }
    Err(format!(
        "member Cargo.toml at {} declares no [package] name",
        dir.display()
    ))
}

/// Read a single-line `key = ["a", "b"]` array from a Cargo.toml body.
fn read_string_array(text: &str, key: &str) -> Result<Vec<String>, String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        let Some((name, value)) = trimmed.split_once('=') else {
            continue;
        };
        if name.trim() != key {
            continue;
        }
        let value = value.trim();
        if !(value.starts_with('[') && value.ends_with(']')) {
            return Err(format!(
                "`{key}` is not a single-line array; keep the workspace manifest one line per array so static policy scans stay bounded"
            ));
        }
        let items = value[1..value.len() - 1]
            .split(',')
            .map(|item| item.trim().trim_matches('"').to_string())
            .filter(|item| !item.is_empty())
            .collect();
        return Ok(items);
    }
    Ok(Vec::new())
}

/// Scan one package's `src/` and `tests/` trees for test function names.
fn scan_package_tests(dir: &Path) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for target in ["src", "tests"] {
        let target_dir = dir.join(target);
        if !target_dir.is_dir() {
            continue;
        }
        for file in collect_files(&target_dir)? {
            if file.extension().and_then(|value| value.to_str()) != Some("rs") {
                continue;
            }
            let relative = file.strip_prefix(&target_dir).map_err(|error| {
                format!(
                    "{} is not under {}: {error}",
                    file.display(),
                    target_dir.display()
                )
            })?;
            let Some(prefix) = module_prefix(relative, target) else {
                continue;
            };
            let text = read_text_lossy(&file)?;
            scan_file(&text, &prefix, &mut names);
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

/// Map a file path (relative to `src/` or `tests/`) to its Rust module path.
///
/// - `src/` files follow the module tree: `a/b.rs` is `a::b`, and `lib.rs`,
///   `main.rs`, and `mod.rs` contribute no segment.
/// - `tests/` files are integration-test crates: the first path component is
///   the test binary name and is not part of any test name, so `foo.rs` and
///   `foo/main.rs` both contribute no segment while `foo/bar.rs` (reached via
///   `mod`) contributes `bar`.
fn module_prefix(relative: &Path, target: &str) -> Option<Vec<String>> {
    let mut components: Vec<String> = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    let file = components.pop()?;
    let stem = file.strip_suffix(".rs")?;
    let stem_segment = if stem == "mod" || stem == "lib" || stem == "main" {
        None
    } else {
        Some(stem.to_string())
    };
    if target == "tests" {
        // Drop the test-binary component; it never appears in test names.
        if components.is_empty() {
            return Some(Vec::new());
        }
        components.remove(0);
    }
    components.extend(stem_segment);
    Some(components)
}

/// Largest number of lines an attribute may be ahead of its `fn` item.
const MAX_FN_LOOKAHEAD: usize = 6;

/// Collect fully-qualified test names from one source file.
fn scan_file(text: &str, prefix: &[String], names: &mut Vec<String>) {
    let lines: Vec<&str> = text.lines().collect();
    let mut modules: Vec<(String, usize)> = Vec::new();
    let mut depth = 0usize;
    for (index, line) in lines.iter().enumerate() {
        let stripped = strip_line_comment(line);
        if let Some(module) = inline_module_open(stripped) {
            modules.push((module, depth));
        }
        for ch in stripped.chars() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    while modules
                        .last()
                        .is_some_and(|(_, open_depth)| *open_depth == depth)
                    {
                        modules.pop();
                    }
                }
                _ => {}
            }
        }
        if let Some(function) = test_attribute_function(&lines, index) {
            let mut name = prefix.to_vec();
            name.extend(modules.iter().map(|(module, _)| module.clone()));
            name.push(function);
            names.push(name.join("::"));
        }
    }
}

/// Return the test function name if the line at `index` carries a
/// `#[test]`-family attribute, looking ahead past other attributes, blanks,
/// and comments for the `fn` item.
fn test_attribute_function(lines: &[&str], index: usize) -> Option<String> {
    if !is_test_attribute(lines[index]) {
        return None;
    }
    for offset in 0..=MAX_FN_LOOKAHEAD {
        let line = lines.get(index + offset)?.trim();
        if offset > 0 {
            if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
                continue;
            }
            if let Some(name) = function_name(line) {
                return Some(name);
            }
            return None;
        }
        if let Some(name) = function_name(line) {
            return Some(name);
        }
    }
    None
}

/// Recognize a `#[test]`-family attribute at the start of a line.
fn is_test_attribute(line: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix("#[") else {
        return false;
    };
    let path = rest.split(['(', ')', ']']).next().unwrap_or("").trim();
    path == "test"
        || path == "rstest"
        || path == "test_case"
        || path == "test_case::test_case"
        || path == "test_matrix"
        || path.ends_with("::test")
}

/// Extract the name of the first `fn` item in a line.
fn function_name(line: &str) -> Option<String> {
    let mut words = line.split(|ch: char| !(ch.is_alphanumeric() || ch == '_'));
    while let Some(word) = words.next() {
        if word == "fn"
            && let Some(name) = words.next()
            && name
                .chars()
                .next()
                .is_some_and(|ch| ch.is_alphabetic() || ch == '_')
        {
            return Some(name.to_string());
        }
    }
    None
}

/// Detect an inline `mod <ident> {` open on one (comment-stripped) line.
/// `mod <ident>;` declarations have no brace and are handled by the file-path
/// mapping instead.
fn inline_module_open(line: &str) -> Option<String> {
    if !line.contains('{') {
        return None;
    }
    let mut words = line.split(|ch: char| !(ch.is_alphanumeric() || ch == '_'));
    while let Some(word) = words.next() {
        if word == "mod"
            && let Some(name) = words.next()
            && !name.is_empty()
            && name
                .chars()
                .next()
                .is_some_and(|ch| ch.is_alphabetic() || ch == '_')
        {
            return Some(name.to_string());
        }
    }
    None
}

/// Strip a `//` line comment that is not inside a string literal, so brace
/// counting and module detection do not trip over commented-out code or URLs.
fn strip_line_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '/' if !in_string && line[idx + 1..].starts_with('/') => {
                return &line[..idx];
            }
            _ => {}
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::{
        TestInventory, function_name, inline_module_open, is_test_attribute, module_prefix,
        parse_cargo_test_command, scan_file, strip_line_comment,
    };
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    fn write(path: &Path, text: &str) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| format!("mkdir failed: {error}"))?;
        }
        std::fs::write(path, text).map_err(|error| format!("write failed: {error}"))
    }

    fn temp_root(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ripr-xtask-test-inventory-{label}-{}",
            std::process::id()
        ));
        let created = std::fs::create_dir_all(&dir);
        assert!(created.is_ok(), "failed to create temp dir: {created:?}");
        dir
    }

    #[test]
    fn parses_package_filters_and_flags() -> Result<(), String> {
        let selection =
            parse_cargo_test_command("cargo test -p xtask --locked --offline some_module::tests")
                .map_err(|error| format!("parse failed: {error}"))?;
        if selection.package.as_deref() != Some("xtask") {
            return Err("package was not parsed".to_string());
        }
        if selection.filters != vec!["some_module::tests".to_string()] {
            return Err(format!("filters were not parsed: {:?}", selection.filters));
        }
        let bare = parse_cargo_test_command("cargo test -- --list --format terse")
            .map_err(|error| format!("parse failed: {error}"))?;
        if !bare.filters.is_empty() || bare.package.is_some() {
            return Err("binary args after `--` leaked into filters".to_string());
        }
        let inline = parse_cargo_test_command("cargo test --package=beta one")
            .map_err(|error| format!("parse failed: {error}"))?;
        if inline.package.as_deref() != Some("beta") {
            return Err("inline --package= was not parsed".to_string());
        }
        Ok(())
    }

    #[test]
    fn rejects_unknown_and_dangling_flags() -> Result<(), String> {
        let unknown = parse_cargo_test_command("cargo test --frobnicate x");
        let dangling = parse_cargo_test_command("cargo test -p");
        let not_test = parse_cargo_test_command("cargo check -p xtask");
        if unknown.is_err() && dangling.is_err() && not_test.is_err() {
            Ok(())
        } else {
            Err("parser accepted an unsupported command shape".to_string())
        }
    }

    #[test]
    fn maps_src_and_tests_paths_to_module_prefixes() -> Result<(), String> {
        let src = module_prefix(Path::new("a/b.rs"), "src").ok_or("src mapping failed")?;
        if src != vec!["a".to_string(), "b".to_string()] {
            return Err(format!("src module path wrong: {src:?}"));
        }
        let lib = module_prefix(Path::new("lib.rs"), "src").ok_or("lib mapping failed")?;
        if !lib.is_empty() {
            return Err(format!("lib.rs must map to no prefix: {lib:?}"));
        }
        let tests_file =
            module_prefix(Path::new("causal_delta_fixture.rs"), "tests").ok_or("tests mapping")?;
        if !tests_file.is_empty() {
            return Err(format!(
                "integration test file name must not be a module segment: {tests_file:?}"
            ));
        }
        let tests_binary =
            module_prefix(Path::new("it/main.rs"), "tests").ok_or("tests mapping")?;
        if !tests_binary.is_empty() {
            return Err(format!(
                "binary main.rs must map to no prefix: {tests_binary:?}"
            ));
        }
        let tests_nested =
            module_prefix(Path::new("it/support.rs"), "tests").ok_or("tests mapping")?;
        if tests_nested != vec!["support".to_string()] {
            return Err(format!("nested tests module path wrong: {tests_nested:?}"));
        }
        Ok(())
    }

    #[test]
    fn scan_resolves_inline_modules_and_lookahead_attributes() -> Result<(), String> {
        let text = "#[cfg(test)]\nmod tests {\n    #[test]\n    #[should_panic]\n    fn inner_case() {}\n\n    #[test] fn same_line_case() {}\n}\n";
        let mut names = Vec::new();
        scan_file(text, &["outer".to_string()], &mut names);
        let expected = vec![
            "outer::tests::inner_case".to_string(),
            "outer::tests::same_line_case".to_string(),
        ];
        if names != expected {
            return Err(format!("scan produced {names:?}, want {expected:?}"));
        }
        Ok(())
    }

    #[test]
    fn scan_ignores_comments_strings_and_external_mods() -> Result<(), String> {
        let text = "// mod fake { // #[test] fn ghost() {}\nlet url = \"https://example.net/a//b\";\nmod external;\n#[test]\nfn real_case() {}\n";
        let mut names = Vec::new();
        scan_file(text, &[], &mut names);
        if names != vec!["real_case".to_string()] {
            return Err(format!("scan picked up non-code items: {names:?}"));
        }
        Ok(())
    }

    #[test]
    fn attribute_and_helper_predicates_stay_bounded() -> Result<(), String> {
        if !is_test_attribute("#[tokio::test]") || !is_test_attribute("#[test_case(1, 2)]") {
            return Err("test attribute was not recognized".to_string());
        }
        if is_test_attribute("#[cfg(test)] mod tests {") {
            return Err("cfg(test) was treated as a test attribute".to_string());
        }
        if function_name("    async fn handles_it<T>() {").as_deref() != Some("handles_it") {
            return Err("function name extraction failed".to_string());
        }
        if inline_module_open("mod tests;").is_some() {
            return Err("external mod declaration looked inline".to_string());
        }
        if strip_line_comment("let a = 1; // trailing { brace").contains("//") {
            return Err("line comment was not stripped".to_string());
        }
        if strip_line_comment("let url = \"https://x\";") != "let url = \"https://x\";" {
            return Err("in-string slashes were treated as a comment".to_string());
        }
        Ok(())
    }

    #[test]
    fn workspace_scan_resolves_real_and_flags_missing() -> Result<(), String> {
        let root = temp_root("workspace");
        write(
            &root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/alpha\", \"crates/beta\"]\ndefault-members = [\"crates/alpha\"]\n",
        )?;
        write(
            &root.join("crates/alpha/Cargo.toml"),
            "[package]\nname = \"alpha\"\n",
        )?;
        write(
            &root.join("crates/alpha/src/lib.rs"),
            "#[test]\nfn unit_case() {}\n",
        )?;
        write(
            &root.join("crates/alpha/tests/it.rs"),
            "#[test]\nfn integration_case() {}\n",
        )?;
        write(
            &root.join("crates/beta/Cargo.toml"),
            "[package]\nname = \"beta\"\n",
        )?;
        write(
            &root.join("crates/beta/src/lib.rs"),
            "#[test]\nfn beta_case() {}\n",
        )?;

        let inventory = TestInventory::scan_workspace(&root)?;
        if inventory.is_empty() {
            return Err("scan of a populated workspace reported an empty inventory".to_string());
        }
        let present = parse_cargo_test_command("cargo test -p alpha unit_case")?;
        inventory
            .resolve(&present)
            .map_err(|error| format!("existing test did not resolve: {error}"))?;
        let integration = parse_cargo_test_command("cargo test -p alpha integration_case")?;
        inventory
            .resolve(&integration)
            .map_err(|error| format!("integration test did not resolve: {error}"))?;
        let missing = parse_cargo_test_command("cargo test -p alpha renamed_away_case")?;
        if inventory.resolve(&missing).is_ok() {
            return Err("missing test resolved".to_string());
        }
        let wrong_package = parse_cargo_test_command("cargo test -p alpha beta_case")?;
        if inventory.resolve(&wrong_package).is_ok() {
            return Err("test from another package resolved inside -p alpha".to_string());
        }
        let default_scope = parse_cargo_test_command("cargo test unit_case")?;
        inventory
            .resolve(&default_scope)
            .map_err(|error| format!("default-members scope did not resolve: {error}"))?;
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn workspace_scan_reports_empty_inventory_distinctly() -> Result<(), String> {
        let root = temp_root("empty");
        write(
            &root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/quiet\"]\n",
        )?;
        write(
            &root.join("crates/quiet/Cargo.toml"),
            "[package]\nname = \"quiet\"\n",
        )?;
        write(&root.join("crates/quiet/src/lib.rs"), "pub fn a() {}\n")?;
        let inventory = TestInventory::scan_workspace(&root)?;
        if !inventory.is_empty() {
            return Err("inventory of a test-free workspace was not empty".to_string());
        }
        let any = parse_cargo_test_command("cargo test -p quiet anything")?;
        if inventory.resolve(&any).is_ok() {
            return Err("empty inventory resolved a claim".to_string());
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn from_parts_inventory_drives_resolution_without_disk() -> Result<(), String> {
        let mut packages = BTreeMap::new();
        packages.insert("alpha".to_string(), vec!["tests::known_case".to_string()]);
        let inventory = TestInventory::from_parts(packages, vec!["alpha".to_string()]);
        let good = parse_cargo_test_command("cargo test -p alpha tests::known_case")?;
        inventory
            .resolve(&good)
            .map_err(|error| format!("known case failed to resolve: {error}"))?;
        let stale = parse_cargo_test_command("cargo test -p alpha stale_case")?;
        let error = match inventory.resolve(&stale) {
            Ok(()) => return Err("stale case must not resolve".to_string()),
            Err(error) => error,
        };
        if !error.contains("stale_case") || !error.contains("package `alpha`") {
            return Err(format!("resolution error was not diagnosable: {error}"));
        }
        Ok(())
    }
}
