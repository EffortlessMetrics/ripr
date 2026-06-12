//! Package-root and workspace (monorepo) discovery for the TypeScript preview adapter.
//!
//! Given a TypeScript test file path and the repo workspace root, this module
//! resolves manifest-backed facts:
//!
//! - `package_root` — nearest ancestor directory containing `package.json`
//! - `workspace_root` — nearest ancestor with `pnpm-workspace.yaml`, OR a
//!   `package.json` with a `"workspaces"` field; falls back to `package_root`
//!   when no monorepo indicator is found.
//! - `framework_hint` — detected from `package.json` dependencies / devDeps
//!   (jest / vitest / bun-types / mocha / @types/node).
//! - `runner_hint` — detected from `scripts.test` and lockfile presence.
//! - `confidence` — reflects the quantity and quality of manifest evidence.
//! - `limitations` — named limitations when required evidence is absent.
//!
//! **Fail-closed rule (non-negotiable):** when no `package.json` is found for
//! a test file, `package_root` stays `None` and a
//! `typescript_package_root_unresolved` limitation is emitted. No value is
//! ever invented from the file extension alone.

use super::*;

// ─── Public types ─────────────────────────────────────────────────────────────

/// Manifest-backed discovery facts for one TypeScript test file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PackageDiscovery {
    /// Nearest ancestor directory that contains a `package.json`, if found.
    pub(crate) package_root: Option<PathBuf>,
    /// Nearest ancestor with monorepo indicators; falls back to `package_root`.
    pub(crate) workspace_root: Option<PathBuf>,
    /// Framework detected from manifest deps/devDeps (evidence-backed only).
    pub(crate) framework_hint: Option<TsFramework>,
    /// Runner detected from lockfile or `scripts.test` (evidence-backed only).
    pub(crate) runner_hint: Option<TsRunner>,
    /// How much manifest/lockfile evidence backed the resolution.
    pub(crate) confidence: TsPackageConfidence,
    /// Named limitations emitted when required evidence is absent.
    pub(crate) limitations: Vec<TsPackageLimitation>,
}

impl PackageDiscovery {
    /// Produce evidence lines suitable for appending to a `Finding`'s evidence
    /// vector.  The prefix `typescript_package_discovery:` lets the renderer
    /// and test assertions identify these lines without ambiguity.
    pub(crate) fn evidence_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(root) = &self.package_root {
            lines.push(format!(
                "typescript_package_root: {}",
                normalized_path(root)
            ));
        }
        if let Some(ws) = &self.workspace_root {
            lines.push(format!(
                "typescript_workspace_root: {}",
                normalized_path(ws)
            ));
        }
        if let Some(fw) = &self.framework_hint {
            lines.push(format!("typescript_framework_hint: {}", fw.as_str()));
        }
        if let Some(runner) = &self.runner_hint {
            lines.push(format!("typescript_runner_hint: {}", runner.as_str()));
        }
        lines.push(format!(
            "typescript_package_confidence: {}",
            self.confidence.as_str()
        ));
        for limitation in &self.limitations {
            lines.push(format!(
                "typescript_package_limitation: {}",
                limitation.as_str()
            ));
        }
        lines
    }
}

// ─── Enums ────────────────────────────────────────────────────────────────────

/// Evidence-backed test framework.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TsFramework {
    Jest,
    Vitest,
    Bun,
    Mocha,
    NodeTest,
}

impl TsFramework {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Jest => "jest",
            Self::Vitest => "vitest",
            Self::Bun => "bun",
            Self::Mocha => "mocha",
            Self::NodeTest => "node_test",
        }
    }
}

/// Evidence-backed test runner (lockfile / scripts.test heuristic).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TsRunner {
    Bun,
    Pnpm,
    Yarn,
    Npm,
}

impl TsRunner {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Bun => "bun",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
            Self::Npm => "npm",
        }
    }
}

/// Confidence level for package discovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TsPackageConfidence {
    /// `package.json` found with framework + runner evidence.
    High,
    /// `package.json` found; only partial manifest evidence.
    Medium,
    /// `package.json` found but no framework or runner evidence.
    Low,
    /// No `package.json` found at all.
    None,
}

impl TsPackageConfidence {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::None => "none",
        }
    }
}

/// Named limitations produced when required manifest evidence is absent.
///
/// Variant names use a `Missing`/`NotFound` suffix so they are distinct from
/// the wire-format limitation strings (which retain the `_unresolved` suffix
/// for consumer compatibility).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TsPackageLimitation {
    /// No `package.json` ancestor was found for this test file.
    /// Wire name: `typescript_package_root_unresolved`.
    PackageRootNotFound,
    /// A `package.json` was found but no framework dep was detected.
    /// Wire name: `typescript_framework_hint_unresolved`.
    FrameworkHintMissing,
    /// A `package.json` was found but no lockfile/script runner evidence.
    /// Wire name: `typescript_runner_hint_unresolved`.
    RunnerHintMissing,
}

impl TsPackageLimitation {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::PackageRootNotFound => "typescript_package_root_unresolved",
            Self::FrameworkHintMissing => "typescript_framework_hint_unresolved",
            Self::RunnerHintMissing => "typescript_runner_hint_unresolved",
        }
    }
}

// ─── Entry point ──────────────────────────────────────────────────────────────

/// Resolve package/workspace discovery facts for `test_file` relative to
/// `workspace_root`.
///
/// `test_file` may be relative (to `workspace_root`) or absolute.
/// This function only reads real on-disk manifests — no value is ever
/// fabricated from the file path alone.
pub(crate) fn resolve_package_discovery(
    test_file: &Path,
    workspace_root: &Path,
) -> PackageDiscovery {
    // Canonicalize to an absolute path so ancestor traversal is safe on every
    // platform.
    let absolute_test_file = if test_file.is_absolute() {
        test_file.to_path_buf()
    } else {
        workspace_root.join(test_file)
    };

    // Walk upward from the test file's directory to find the nearest package.json.
    let start_dir = match absolute_test_file.parent() {
        Some(dir) => dir.to_path_buf(),
        None => {
            return PackageDiscovery {
                package_root: None,
                workspace_root: None,
                framework_hint: None,
                runner_hint: None,
                confidence: TsPackageConfidence::None,
                limitations: vec![TsPackageLimitation::PackageRootNotFound],
            };
        }
    };

    let pkg_root = find_nearest_package_json(&start_dir, workspace_root);

    let Some(pkg_root) = pkg_root else {
        return PackageDiscovery {
            package_root: None,
            workspace_root: None,
            framework_hint: None,
            runner_hint: None,
            confidence: TsPackageConfidence::None,
            limitations: vec![TsPackageLimitation::PackageRootNotFound],
        };
    };

    // Read the package.json at that root.
    let pkg_json_path = pkg_root.join("package.json");
    let pkg_json_text = match std::fs::read_to_string(&pkg_json_path) {
        Ok(text) => text,
        Err(_) => {
            return PackageDiscovery {
                package_root: None,
                workspace_root: None,
                framework_hint: None,
                runner_hint: None,
                confidence: TsPackageConfidence::None,
                limitations: vec![TsPackageLimitation::PackageRootNotFound],
            };
        }
    };

    let framework_hint = detect_framework(&pkg_json_text);
    let runner_hint_from_script = detect_runner_from_scripts(&pkg_json_text);

    // Detect monorepo workspace root: walk upward from pkg_root looking for
    // pnpm-workspace.yaml or a package.json with "workspaces" field.
    let ws_root =
        find_workspace_root(&pkg_root, workspace_root).unwrap_or_else(|| pkg_root.clone());

    // Detect runner from lockfile (checking at workspace_root and pkg_root).
    let runner_hint =
        detect_runner_from_lockfile(workspace_root, &pkg_root).or(runner_hint_from_script);

    // Determine limitation set.
    let mut limitations = Vec::new();
    if framework_hint.is_none() {
        limitations.push(TsPackageLimitation::FrameworkHintMissing);
    }
    if runner_hint.is_none() {
        limitations.push(TsPackageLimitation::RunnerHintMissing);
    }

    // Determine confidence.
    let confidence = match (framework_hint.is_some(), runner_hint.is_some()) {
        (true, true) => TsPackageConfidence::High,
        (true, false) | (false, true) => TsPackageConfidence::Medium,
        (false, false) => TsPackageConfidence::Low,
    };

    // Convert absolute paths back to relative-from-workspace for stable output.
    let rel_pkg_root = to_relative(&pkg_root, workspace_root);
    let rel_ws_root = to_relative(&ws_root, workspace_root);

    PackageDiscovery {
        package_root: Some(rel_pkg_root),
        workspace_root: Some(rel_ws_root),
        framework_hint,
        runner_hint,
        confidence,
        limitations,
    }
}

// ─── Manifest helpers ─────────────────────────────────────────────────────────

/// Walk upward from `start` (inclusive) to `stop_at` (inclusive) looking for
/// the nearest `package.json`.  Returns `None` when the search exceeds
/// `stop_at` without finding one.
fn find_nearest_package_json(start: &Path, stop_at: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join("package.json").is_file() {
            return Some(current);
        }
        // Stop when we have reached the workspace boundary or the filesystem root.
        if current == stop_at {
            // Check stop_at itself one last time (in case start == stop_at and
            // we need to include the workspace root).
            break;
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }
    // One final check at stop_at.
    if current.join("package.json").is_file() {
        return Some(current);
    }
    None
}

/// Walk upward from `pkg_root` to `stop_at` looking for a monorepo indicator:
/// - a `pnpm-workspace.yaml` file, OR
/// - a `package.json` that contains a `"workspaces"` key.
///
/// Returns `None` when no monorepo root is found above `pkg_root`.
fn find_workspace_root(pkg_root: &Path, stop_at: &Path) -> Option<PathBuf> {
    // Start from the parent of pkg_root (we already know pkg_root is a
    // package root, not the workspace root, unless it's also the mono-root).
    let mut current = pkg_root.to_path_buf();
    // Check current dir (pkg_root) first — it may also be the monorepo root.
    loop {
        if current.join("pnpm-workspace.yaml").is_file() {
            return Some(current.clone());
        }
        if let Ok(text) = std::fs::read_to_string(current.join("package.json"))
            && json_has_workspaces_field(&text)
        {
            return Some(current.clone());
        }
        if current == stop_at {
            break;
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }
    None
}

/// Detect the test framework from `package.json` content.
///
/// Priority (first match wins): jest > vitest > bun-types > mocha > @types/node
fn detect_framework(pkg_json: &str) -> Option<TsFramework> {
    // Gather dep names from dependencies and devDependencies sections.
    // We use simple string matching rather than full JSON parsing to keep
    // dependencies minimal — we only need to know whether certain dep names
    // appear as keys.
    let lower = pkg_json.to_lowercase();
    // Use key-pattern matching: `"<name>"` (quoted key) to avoid false
    // positives in string values.
    let has_dep = |name: &str| -> bool {
        let pattern = format!("\"{name}\"");
        lower.contains(&pattern)
    };

    if has_dep("jest") || has_dep("@types/jest") || has_dep("ts-jest") || has_dep("babel-jest") {
        return Some(TsFramework::Jest);
    }
    if has_dep("vitest") {
        return Some(TsFramework::Vitest);
    }
    if has_dep("bun-types") {
        return Some(TsFramework::Bun);
    }
    if has_dep("mocha") || has_dep("@types/mocha") {
        return Some(TsFramework::Mocha);
    }
    if has_dep("@types/node") {
        return Some(TsFramework::NodeTest);
    }
    None
}

/// Detect the runner from `scripts.test` in `package.json`.
fn detect_runner_from_scripts(pkg_json: &str) -> Option<TsRunner> {
    // Look for the "test" script entry.  We search for `"test":` then extract
    // the command string that follows.
    let lower = pkg_json.to_lowercase();
    let test_script_idx = lower.find("\"test\":")?;
    let after_key = &lower[test_script_idx + "\"test\":".len()..];
    // Skip whitespace.
    let trimmed = after_key.trim_start();
    // Expect a quoted string next.
    let inner = trimmed.strip_prefix('"')?;
    let end = inner.find('"')?;
    let script = &inner[..end];

    if script.contains("bun ") || script.starts_with("bun") {
        return Some(TsRunner::Bun);
    }
    if script.contains("vitest") || script.contains("jest") {
        // Runner inferred from script framework invocation; defer to lockfile
        // for the package manager.
        return None;
    }
    if script.contains("pnpm ") {
        return Some(TsRunner::Pnpm);
    }
    if script.contains("yarn ") {
        return Some(TsRunner::Yarn);
    }
    None
}

/// Detect the runner from lockfile presence.  Checked at both `workspace_root`
/// and `pkg_root` (lockfile may live at the monorepo root only).
fn detect_runner_from_lockfile(workspace_root: &Path, pkg_root: &Path) -> Option<TsRunner> {
    let dirs = [workspace_root, pkg_root];
    for dir in dirs {
        if dir.join("bun.lock").is_file() || dir.join("bun.lockb").is_file() {
            return Some(TsRunner::Bun);
        }
        if dir.join("pnpm-lock.yaml").is_file() {
            return Some(TsRunner::Pnpm);
        }
        if dir.join("yarn.lock").is_file() {
            return Some(TsRunner::Yarn);
        }
        if dir.join("package-lock.json").is_file() {
            return Some(TsRunner::Npm);
        }
    }
    None
}

/// Very cheap check: does the `package.json` text contain a `"workspaces"`
/// key?  We do NOT need to parse JSON — just check if the key appears.
fn json_has_workspaces_field(pkg_json: &str) -> bool {
    pkg_json.to_lowercase().contains("\"workspaces\"")
}

// ─── Verify-command inference ─────────────────────────────────────────────────

/// Infer an evidence-backed verify command for `test_file` using the
/// `PackageDiscovery` facts already resolved for the same package.
///
/// Command mapping (framework takes priority over runner):
/// ```text
/// framework Bun      -> bun test <file>
/// framework Vitest   -> vitest run <file>
/// framework Jest     -> jest <file>
/// framework NodeTest -> node --test <file>
/// (no framework) runner Bun  -> bun test <file>
/// (no framework) runner Npm  -> npm test -- <file>
/// (no framework) runner Pnpm -> pnpm test -- <file>
/// (no framework) runner Yarn -> yarn test <file>
/// ```
///
/// `<file>` is the test file path normalized (`\` → `/`) and expressed
/// relative to `package_root` so the command is runnable from there.
///
/// Fail-closed: when neither a framework nor a runner resolves, returns
/// `None` so the caller emits the named limitation
/// `typescript_test_runner_unresolved` instead of an invented command.
pub(crate) fn verify_command_for_discovery(
    discovery: &PackageDiscovery,
    test_file: &Path,
) -> Option<String> {
    // Must have a known package root; without one there is no runnable CWD.
    let pkg_root = discovery.package_root.as_deref()?;

    // Compute path relative to package_root so the command is runnable from
    // that directory.  `test_file` may already be relative to the workspace
    // root (which is what the diff-mode pipeline uses), and `pkg_root` is
    // also relative to the workspace root.
    let rel_file = test_file
        .strip_prefix(pkg_root)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| test_file.to_path_buf());

    // Normalize separators: CRITICAL for Windows-blessed goldens to pass
    // Linux CI.
    let file_str = normalized_path(&rel_file);

    // Framework takes priority over runner.
    let cmd = match discovery.framework_hint {
        Some(TsFramework::Bun) => format!("bun test {file_str}"),
        Some(TsFramework::Vitest) => format!("vitest run {file_str}"),
        Some(TsFramework::Jest) => format!("jest {file_str}"),
        Some(TsFramework::NodeTest) => format!("node --test {file_str}"),
        // Mocha does not have a simple file-target form in the spec table;
        // fall through to runner fallback.
        Some(TsFramework::Mocha) | None => {
            // No framework resolved: use runner fallback.
            match discovery.runner_hint {
                Some(TsRunner::Bun) => format!("bun test {file_str}"),
                Some(TsRunner::Npm) => format!("npm test -- {file_str}"),
                Some(TsRunner::Pnpm) => format!("pnpm test -- {file_str}"),
                Some(TsRunner::Yarn) => format!("yarn test {file_str}"),
                // Fail-closed: no evidence → no command.
                None => return None,
            }
        }
    };
    Some(cmd)
}

/// Convert an absolute path back to a path relative to `base`.  If the
/// conversion fails (e.g. cross-drive on Windows), keep the absolute path.
fn to_relative(path: &Path, base: &Path) -> PathBuf {
    path.strip_prefix(base)
        .map(|p| {
            if p == Path::new("") {
                PathBuf::from(".")
            } else {
                p.to_path_buf()
            }
        })
        .unwrap_or_else(|_| path.to_path_buf())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    // ── Helpers ────────────────────────────────────────────────────────────────

    fn unique_test_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("ripr-ts-pkg-{name}-{stamp}"))
    }

    fn setup_single_package(root: &Path, pkg_json: &str, lockfile: Option<&str>) {
        let _ = fs::create_dir_all(root.join("src"));
        let _ = fs::create_dir_all(root.join("tests"));
        let _ = fs::write(root.join("package.json"), pkg_json);
        if let Some(lockfile_name) = lockfile {
            let _ = fs::write(root.join(lockfile_name), "");
        }
    }

    fn vitest_pkg_json() -> &'static str {
        r#"{
  "name": "my-pkg",
  "devDependencies": {
    "vitest": "^1.0.0",
    "typescript": "^5.0.0"
  },
  "scripts": {
    "test": "vitest run"
  }
}"#
    }

    fn jest_pkg_json() -> &'static str {
        r#"{
  "name": "my-pkg",
  "devDependencies": {
    "jest": "^29.0.0",
    "@types/jest": "^29.0.0",
    "typescript": "^5.0.0"
  },
  "scripts": {
    "test": "jest"
  }
}"#
    }

    fn bun_pkg_json() -> &'static str {
        r#"{
  "name": "my-bun-pkg",
  "devDependencies": {
    "bun-types": "^1.0.0"
  },
  "scripts": {
    "test": "bun test"
  }
}"#
    }

    fn monorepo_root_pkg_json() -> &'static str {
        r#"{
  "name": "monorepo-root",
  "workspaces": ["packages/*"],
  "devDependencies": {}
}"#
    }

    // ── Unit tests: framework detection ────────────────────────────────────────

    #[test]
    fn detect_framework_jest() {
        let result = detect_framework(jest_pkg_json());
        assert_eq!(result, Some(TsFramework::Jest));
    }

    #[test]
    fn detect_framework_vitest() {
        let result = detect_framework(vitest_pkg_json());
        assert_eq!(result, Some(TsFramework::Vitest));
    }

    #[test]
    fn detect_framework_bun() {
        let result = detect_framework(bun_pkg_json());
        assert_eq!(result, Some(TsFramework::Bun));
    }

    #[test]
    fn detect_framework_none_for_empty_pkg_json() {
        let result = detect_framework(r#"{"name":"empty","devDependencies":{}}"#);
        assert_eq!(result, None);
    }

    #[test]
    fn detect_framework_mocha() {
        let result = detect_framework(r#"{"devDependencies":{"mocha":"^10.0.0"}}"#);
        assert_eq!(result, Some(TsFramework::Mocha));
    }

    #[test]
    fn detect_framework_node_test() {
        let result = detect_framework(r#"{"devDependencies":{"@types/node":"^20.0.0"}}"#);
        assert_eq!(result, Some(TsFramework::NodeTest));
    }

    // ── Unit tests: runner detection ──────────────────────────────────────────

    #[test]
    fn detect_runner_from_scripts_bun() {
        let result = detect_runner_from_scripts(bun_pkg_json());
        assert_eq!(result, Some(TsRunner::Bun));
    }

    #[test]
    fn detect_runner_from_scripts_vitest_returns_none_defers_to_lockfile() {
        // vitest in scripts.test → defer to lockfile detection, not script runner
        let result = detect_runner_from_scripts(vitest_pkg_json());
        assert_eq!(result, None);
    }

    #[test]
    fn detect_runner_from_scripts_pnpm() {
        // A script that explicitly uses pnpm but not vitest/jest as a framework
        let pkg = r#"{"scripts":{"test":"pnpm run test:unit"}}"#;
        let result = detect_runner_from_scripts(pkg);
        assert_eq!(result, Some(TsRunner::Pnpm));
    }

    // ── Integration tests: resolve_package_discovery ─────────────────────────

    #[test]
    fn ts_package_discovery_single_package_vitest_pnpm() {
        let root = unique_test_dir("single-vitest-pnpm");
        setup_single_package(&root, vitest_pkg_json(), Some("pnpm-lock.yaml"));
        let test_file = PathBuf::from("tests/math.test.ts");

        let result = resolve_package_discovery(&test_file, &root);

        assert_eq!(result.package_root, Some(PathBuf::from(".")));
        assert_eq!(result.workspace_root, Some(PathBuf::from(".")));
        assert_eq!(result.framework_hint, Some(TsFramework::Vitest));
        assert_eq!(result.runner_hint, Some(TsRunner::Pnpm));
        assert_eq!(result.confidence, TsPackageConfidence::High);
        assert!(
            !result
                .limitations
                .contains(&TsPackageLimitation::PackageRootNotFound)
        );
    }

    #[test]
    fn ts_package_discovery_single_package_jest_npm() {
        let root = unique_test_dir("single-jest-npm");
        setup_single_package(&root, jest_pkg_json(), Some("package-lock.json"));
        let test_file = PathBuf::from("src/math.test.ts");

        let result = resolve_package_discovery(&test_file, &root);

        assert_eq!(result.framework_hint, Some(TsFramework::Jest));
        assert_eq!(result.runner_hint, Some(TsRunner::Npm));
        assert_eq!(result.confidence, TsPackageConfidence::High);
        assert!(
            !result
                .limitations
                .contains(&TsPackageLimitation::PackageRootNotFound)
        );

        let lines = result.evidence_lines();
        assert!(
            lines
                .iter()
                .any(|l| l.contains("typescript_framework_hint: jest"))
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("typescript_runner_hint: npm"))
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("typescript_package_confidence: high"))
        );
    }

    #[test]
    fn ts_package_discovery_bun_lockfile_wins_over_script() {
        let root = unique_test_dir("bun-lockfile");
        // bun.lockb present — runner should be bun even with vitest script
        setup_single_package(&root, vitest_pkg_json(), Some("bun.lockb"));
        let test_file = PathBuf::from("src/math.test.ts");

        let result = resolve_package_discovery(&test_file, &root);

        assert_eq!(result.runner_hint, Some(TsRunner::Bun));
        assert_eq!(result.framework_hint, Some(TsFramework::Vitest));
    }

    #[test]
    fn ts_package_discovery_monorepo_package_local_root() {
        let root = unique_test_dir("monorepo-pnpm");
        // monorepo root: pnpm-workspace.yaml + root package.json (no framework)
        let _ = fs::create_dir_all(&root);
        let _ = fs::write(
            root.join("pnpm-workspace.yaml"),
            "packages:\n  - packages/*\n",
        );
        let _ = fs::write(root.join("package.json"), monorepo_root_pkg_json());
        let _ = fs::write(root.join("pnpm-lock.yaml"), "");
        // sub-package with its own package.json
        let pkg_dir = root.join("packages").join("auth");
        let _ = fs::create_dir_all(pkg_dir.join("src"));
        let _ = fs::create_dir_all(pkg_dir.join("tests"));
        let _ = fs::write(pkg_dir.join("package.json"), jest_pkg_json());

        let test_file = PathBuf::from("packages/auth/tests/auth.test.ts");
        let result = resolve_package_discovery(&test_file, &root);

        // package_root is the sub-package, not the monorepo root
        assert_eq!(
            result.package_root,
            Some(PathBuf::from("packages/auth")),
            "package_root should be packages/auth, got {:?}",
            result.package_root
        );
        // workspace_root is the monorepo root (has pnpm-workspace.yaml)
        assert_eq!(
            result.workspace_root,
            Some(PathBuf::from(".")),
            "workspace_root should be repo root, got {:?}",
            result.workspace_root
        );
        assert_eq!(result.framework_hint, Some(TsFramework::Jest));
        // Runner comes from pnpm-lock.yaml at workspace root
        assert_eq!(result.runner_hint, Some(TsRunner::Pnpm));
        assert_eq!(result.confidence, TsPackageConfidence::High);
    }

    #[test]
    fn ts_package_discovery_monorepo_workspaces_field_in_package_json() {
        let root = unique_test_dir("monorepo-workspaces-field");
        // No pnpm-workspace.yaml — use "workspaces" field in root package.json
        let _ = fs::create_dir_all(&root);
        let _ = fs::write(root.join("package.json"), monorepo_root_pkg_json());
        let _ = fs::write(root.join("yarn.lock"), "");
        let pkg_dir = root.join("packages").join("ui");
        let _ = fs::create_dir_all(pkg_dir.join("src"));
        let _ = fs::write(pkg_dir.join("package.json"), vitest_pkg_json());

        let test_file = PathBuf::from("packages/ui/src/Button.test.ts");
        let result = resolve_package_discovery(&test_file, &root);

        // workspace_root is the repo root because root package.json has "workspaces"
        assert_eq!(result.workspace_root, Some(PathBuf::from(".")),);
        assert_eq!(result.package_root, Some(PathBuf::from("packages/ui")));
        assert_eq!(result.runner_hint, Some(TsRunner::Yarn));
    }

    #[test]
    fn ts_package_discovery_no_package_json_emits_unresolved_limitation() {
        let root = unique_test_dir("no-pkg-json");
        let _ = fs::create_dir_all(&root);
        // No package.json anywhere in the tree
        let test_file = PathBuf::from("src/math.test.ts");

        let result = resolve_package_discovery(&test_file, &root);

        assert_eq!(result.package_root, None);
        assert_eq!(result.workspace_root, None);
        assert_eq!(result.framework_hint, None);
        assert_eq!(result.runner_hint, None);
        assert_eq!(result.confidence, TsPackageConfidence::None);
        assert!(
            result
                .limitations
                .contains(&TsPackageLimitation::PackageRootNotFound),
            "expected typescript_package_root_unresolved limitation"
        );

        let lines = result.evidence_lines();
        assert!(
            lines
                .iter()
                .any(|l| l
                    .contains("typescript_package_limitation: typescript_package_root_unresolved")),
            "evidence lines missing typescript_package_root_unresolved: {:?}",
            lines
        );
        // Must NOT emit a guessed package_root
        assert!(
            !lines
                .iter()
                .any(|l| l.starts_with("typescript_package_root: ")),
            "must not emit package_root when unresolved: {:?}",
            lines
        );
    }

    #[test]
    fn ts_package_discovery_package_without_framework_emits_framework_hint_unresolved() {
        let root = unique_test_dir("no-framework");
        let minimal = r#"{"name":"no-fw","devDependencies":{}}"#;
        setup_single_package(&root, minimal, Some("yarn.lock"));
        let test_file = PathBuf::from("tests/foo.test.ts");

        let result = resolve_package_discovery(&test_file, &root);

        assert!(result.package_root.is_some());
        assert_eq!(result.framework_hint, None);
        assert!(
            result
                .limitations
                .contains(&TsPackageLimitation::FrameworkHintMissing)
        );
        assert_eq!(result.runner_hint, Some(TsRunner::Yarn));
        // confidence is Medium (runner present, framework absent)
        assert_eq!(result.confidence, TsPackageConfidence::Medium);
    }

    #[test]
    fn ts_package_discovery_evidence_lines_no_package_root() {
        let root = unique_test_dir("evidence-lines-nopkg");
        let _ = fs::create_dir_all(&root);
        let result = resolve_package_discovery(&PathBuf::from("test.test.ts"), &root);
        let lines = result.evidence_lines();
        // Must contain at least one limitation line
        let limit_lines: Vec<_> = lines
            .iter()
            .filter(|l| l.contains("typescript_package_limitation:"))
            .collect();
        assert!(
            !limit_lines.is_empty(),
            "expected limitation lines: {:?}",
            lines
        );
    }

    // ── Unit tests: verify_command_for_discovery ───────────────────────────────

    fn make_discovery(
        package_root: Option<&str>,
        framework_hint: Option<TsFramework>,
        runner_hint: Option<TsRunner>,
    ) -> PackageDiscovery {
        PackageDiscovery {
            package_root: package_root.map(PathBuf::from),
            workspace_root: package_root.map(PathBuf::from),
            framework_hint,
            runner_hint,
            confidence: TsPackageConfidence::High,
            limitations: Vec::new(),
        }
    }

    #[test]
    fn verify_command_framework_jest_produces_jest_command() {
        let discovery = make_discovery(Some("."), Some(TsFramework::Jest), Some(TsRunner::Npm));
        let result = verify_command_for_discovery(&discovery, Path::new("tests/math.test.ts"));
        assert_eq!(result, Some("jest tests/math.test.ts".to_string()));
    }

    #[test]
    fn verify_command_framework_vitest_produces_vitest_run_command() {
        let discovery = make_discovery(Some("."), Some(TsFramework::Vitest), Some(TsRunner::Pnpm));
        let result = verify_command_for_discovery(&discovery, Path::new("src/util.test.ts"));
        assert_eq!(result, Some("vitest run src/util.test.ts".to_string()));
    }

    #[test]
    fn verify_command_framework_bun_produces_bun_test_command() {
        let discovery = make_discovery(Some("."), Some(TsFramework::Bun), Some(TsRunner::Bun));
        let result = verify_command_for_discovery(&discovery, Path::new("tests/app.test.ts"));
        assert_eq!(result, Some("bun test tests/app.test.ts".to_string()));
    }

    #[test]
    fn verify_command_framework_node_test_produces_node_test_command() {
        let discovery = make_discovery(Some("."), Some(TsFramework::NodeTest), Some(TsRunner::Npm));
        let result = verify_command_for_discovery(&discovery, Path::new("tests/core.test.mjs"));
        assert_eq!(result, Some("node --test tests/core.test.mjs".to_string()));
    }

    #[test]
    fn verify_command_no_framework_runner_npm_produces_npm_test_command() {
        let discovery = make_discovery(Some("."), None, Some(TsRunner::Npm));
        let result = verify_command_for_discovery(&discovery, Path::new("tests/math.test.ts"));
        assert_eq!(result, Some("npm test -- tests/math.test.ts".to_string()));
    }

    #[test]
    fn verify_command_no_framework_runner_pnpm_produces_pnpm_test_command() {
        let discovery = make_discovery(Some("."), None, Some(TsRunner::Pnpm));
        let result = verify_command_for_discovery(&discovery, Path::new("tests/math.test.ts"));
        assert_eq!(result, Some("pnpm test -- tests/math.test.ts".to_string()));
    }

    #[test]
    fn verify_command_no_framework_runner_yarn_produces_yarn_test_command() {
        let discovery = make_discovery(Some("."), None, Some(TsRunner::Yarn));
        let result = verify_command_for_discovery(&discovery, Path::new("tests/math.test.ts"));
        assert_eq!(result, Some("yarn test tests/math.test.ts".to_string()));
    }

    #[test]
    fn verify_command_no_framework_runner_bun_produces_bun_test_command() {
        let discovery = make_discovery(Some("."), None, Some(TsRunner::Bun));
        let result = verify_command_for_discovery(&discovery, Path::new("tests/math.test.ts"));
        assert_eq!(result, Some("bun test tests/math.test.ts".to_string()));
    }

    #[test]
    fn verify_command_no_framework_no_runner_returns_none_fail_closed() {
        let discovery = make_discovery(Some("."), None, None);
        let result = verify_command_for_discovery(&discovery, Path::new("tests/math.test.ts"));
        assert_eq!(
            result, None,
            "fail-closed: no command when neither framework nor runner resolves"
        );
    }

    #[test]
    fn verify_command_no_package_root_returns_none_fail_closed() {
        let discovery = make_discovery(None, Some(TsFramework::Jest), Some(TsRunner::Npm));
        let result = verify_command_for_discovery(&discovery, Path::new("tests/math.test.ts"));
        assert_eq!(
            result, None,
            "fail-closed: no command when package_root is None"
        );
    }

    #[test]
    fn verify_command_monorepo_strips_package_root_prefix() {
        // test file is relative to workspace root: packages/auth/tests/token.test.ts
        // package_root is packages/auth, so the command should use tests/token.test.ts
        let discovery = make_discovery(
            Some("packages/auth"),
            Some(TsFramework::Jest),
            Some(TsRunner::Pnpm),
        );
        let result = verify_command_for_discovery(
            &discovery,
            Path::new("packages/auth/tests/token.test.ts"),
        );
        assert_eq!(result, Some("jest tests/token.test.ts".to_string()));
    }

    #[test]
    fn verify_command_framework_takes_priority_over_runner() {
        // Vitest framework with Bun runner → vitest run (not bun test)
        let discovery = make_discovery(Some("."), Some(TsFramework::Vitest), Some(TsRunner::Bun));
        let result = verify_command_for_discovery(&discovery, Path::new("src/foo.test.ts"));
        assert_eq!(result, Some("vitest run src/foo.test.ts".to_string()));
    }

    #[test]
    fn verify_command_normalizes_backslash_separators() {
        // On Windows, paths may use backslashes; the command must normalize them.
        let discovery = make_discovery(Some("."), Some(TsFramework::Jest), None);
        let result =
            verify_command_for_discovery(&discovery, Path::new("tests\\auth\\token.test.ts"));
        assert!(result.is_some(), "expected a command");
        let cmd = result.unwrap_or_default();
        assert!(!cmd.contains('\\'), "backslashes must be normalized: {cmd}");
        assert!(
            cmd.contains("tests/auth/token.test.ts"),
            "expected normalized path: {cmd}"
        );
    }
}
