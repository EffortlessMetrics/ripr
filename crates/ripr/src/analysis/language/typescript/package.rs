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
            // `typescript_test_runner` is the detected test framework name — a
            // separate, dedicated evidence field (additive) that later steps
            // use to infer verify commands without re-parsing the manifest.
            lines.push(format!("typescript_test_runner: {}", fw.as_str()));
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
    /// Ava test runner (<https://github.com/avajs/ava>).
    Ava,
}

impl TsFramework {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Jest => "jest",
            Self::Vitest => "vitest",
            Self::Bun => "bun",
            Self::Mocha => "mocha",
            Self::NodeTest => "node_test",
            Self::Ava => "ava",
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
    /// A `package.json` was found, a framework is known, but no
    /// lockfile/script evidence identifies the package manager (npm/pnpm/yarn/bun).
    /// A test command IS derivable directly from the framework binary.
    /// Wire name: `typescript_package_manager_unresolved`.
    PackageManagerUnresolved,
    /// A `package.json` was found but no lockfile/script runner evidence AND
    /// no framework was detected — genuinely no command can be derived.
    /// Wire name: `typescript_runner_hint_unresolved`.
    RunnerHintMissing,
    /// Two or more distinct framework signals matched (e.g. both `jest` and
    /// `vitest` in `devDependencies`). The reported runner is the first match
    /// by fixed priority, which is arbitrary from the evidence — the
    /// limitation discloses the ambiguity and caps confidence at `medium`.
    /// Classification-neutral and additive: the priority pick is still
    /// emitted as `typescript_test_runner: <name>`.
    /// Wire name: `typescript_test_runner_ambiguous`.
    FrameworkAmbiguous,
    /// A `package.json` was found but exceeded the capped read limit, so its
    /// manifest evidence could not be inspected. Fail-closed: no root, no
    /// fabricated values.
    /// Wire name: `typescript_package_manifest_read_capped`.
    PackageManifestReadCapped,
}

impl TsPackageLimitation {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::PackageRootNotFound => "typescript_package_root_unresolved",
            Self::FrameworkHintMissing => "typescript_framework_hint_unresolved",
            Self::PackageManagerUnresolved => "typescript_package_manager_unresolved",
            Self::RunnerHintMissing => "typescript_runner_hint_unresolved",
            Self::FrameworkAmbiguous => "typescript_test_runner_ambiguous",
            Self::PackageManifestReadCapped => "typescript_package_manifest_read_capped",
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

    // Read the package.json at that root (capped: an over-limit manifest is
    // disclosed as its own named limitation, not conflated with "not found").
    let pkg_json_path = pkg_root.join("package.json");
    let pkg_json_text = match read_config_capped(&pkg_json_path) {
        Ok(text) => text,
        Err(err) => {
            let limitation = if err.is_size_limit() {
                TsPackageLimitation::PackageManifestReadCapped
            } else {
                TsPackageLimitation::PackageRootNotFound
            };
            return PackageDiscovery {
                package_root: None,
                workspace_root: None,
                framework_hint: None,
                runner_hint: None,
                confidence: TsPackageConfidence::None,
                limitations: vec![limitation],
            };
        }
    };

    let signals = detect_framework_signals(&pkg_json_text);
    let framework_hint = signals.first().copied();
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
    } else if signals.len() >= 2 {
        // Two or more DISTINCT framework signals matched (e.g. jest + vitest
        // devDeps). The reported runner is the first by fixed priority, which
        // is arbitrary from the evidence — disclose the ambiguity (additive,
        // classification-neutral) and cap confidence at `medium` below.
        limitations.push(TsPackageLimitation::FrameworkAmbiguous);
    }
    if runner_hint.is_none() {
        if framework_hint.is_some() {
            // Framework is known: the framework binary IS the test runner, so a
            // verify command is derivable without the package manager.  Disclose
            // only that the package manager (npm/pnpm/yarn/bun) could not be
            // identified from lockfile evidence — this is an informational gap,
            // not a blocking one.
            limitations.push(TsPackageLimitation::PackageManagerUnresolved);
        } else {
            // Neither framework nor runner resolved: genuinely no command can be
            // derived.  Emit the strong fail-closed limitation.
            limitations.push(TsPackageLimitation::RunnerHintMissing);
        }
    }

    // Determine confidence. Ambiguous framework evidence is capped at Medium:
    // the priority pick is a guess from the manifest's point of view.
    let ambiguous = signals.len() >= 2;
    let confidence = match (framework_hint.is_some(), runner_hint.is_some()) {
        (true, true) if !ambiguous => TsPackageConfidence::High,
        (true, _) | (_, true) => TsPackageConfidence::Medium,
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
        if let Ok(text) = read_config_capped(&current.join("package.json"))
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

/// Detect the TypeScript/JavaScript test framework for a workspace root,
/// using the same signals the adapter's package discovery trusts (#2106):
/// `package.json` dependency/script evidence first (via [`detect_framework`]),
/// then config-file markers, then the bun lockfile. Fail-closed: `None` when
/// no signal matches — callers must report "not detected", never guess.
pub(crate) fn detect_framework_for_root(root: &Path) -> Option<TsFramework> {
    if let Ok(pkg_json) = read_config_capped(&root.join("package.json"))
        && let Some(framework) = detect_framework(&pkg_json)
    {
        return Some(framework);
    }
    if [
        "jest.config.js",
        "jest.config.ts",
        "jest.config.mjs",
        "jest.config.cjs",
    ]
    .iter()
    .any(|file| root.join(file).exists())
    {
        return Some(TsFramework::Jest);
    }
    if ["vitest.config.ts", "vitest.config.js", "vitest.config.mjs"]
        .iter()
        .any(|file| root.join(file).exists())
    {
        return Some(TsFramework::Vitest);
    }
    // Both bun lockfile names count, matching the adapter's runner
    // detection: bun.lockb (binary) and bun.lock (text) (#2106 review).
    if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
        return Some(TsFramework::Bun);
    }
    None
}

/// Parsed view of the `package.json` fields this module consumes.
///
/// Built ONCE from the real manifest via `serde_json` so every downstream
/// detector reads FIELDS, not substrings. This keeps detection honest:
///
/// - a `"test"` key OUTSIDE `scripts` is not the test script;
/// - the word `"workspaces"` inside a string value does not mark a monorepo;
/// - a framework name appearing only inside a free-text value
///   (e.g. `"description": "jest"`) is not dependency evidence.
///
/// `None` when the manifest is not a parseable JSON object — every consumer
/// fails closed in that case (no guessed framework, runner, or workspace).
#[derive(Clone, Debug, PartialEq, Eq)]
struct ManifestFacts {
    /// Lowercased dependency names from `dependencies` + `devDependencies`.
    dep_names: Vec<String>,
    /// Lowercased `scripts.test` command, when present and a string.
    test_script: Option<String>,
    /// Whether a top-level `workspaces` field exists.
    has_workspaces: bool,
}

fn parse_manifest_facts(pkg_json: &str) -> Option<ManifestFacts> {
    let value: serde_json::Value = serde_json::from_str(pkg_json).ok()?;
    let object = value.as_object()?;
    let mut dep_names = Vec::new();
    for section in ["dependencies", "devDependencies"] {
        if let Some(deps) = object.get(section).and_then(serde_json::Value::as_object) {
            dep_names.extend(deps.keys().map(|key| key.to_lowercase()));
        }
    }
    let test_script = object
        .get("scripts")
        .and_then(serde_json::Value::as_object)
        .and_then(|scripts| scripts.get("test"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_lowercase);
    let has_workspaces = object.contains_key("workspaces");
    Some(ManifestFacts {
        dep_names,
        test_script,
        has_workspaces,
    })
}

/// Detect the test framework from `package.json` content.
///
/// Priority (first match wins): jest > vitest > bun-types > ava > mocha > @types/node.
///
/// Two complementary signals are checked:
/// 1. `devDependencies` / `dependencies` keys — evidence-backed dep names.
/// 2. `scripts.test` value — first known framework name found in the script
///    command wins (handles composite scripts like `"xo && npm run build && ava"`).
///
/// Dependency signal wins when present; script-name fallback activates only
/// when no dep match is found.  Fail-closed: `None` when neither signal
/// matches a known framework.
fn detect_framework(pkg_json: &str) -> Option<TsFramework> {
    detect_framework_signals(pkg_json).first().copied()
}

/// Push `framework` into `signals` unless an equal signal is already present.
fn push_framework_signal(signals: &mut Vec<TsFramework>, framework: TsFramework) {
    if !signals.contains(&framework) {
        signals.push(framework);
    }
}

/// Detect every DISTINCT framework signal in `package.json`, in priority order.
///
/// Unlike [`detect_framework`] (first match wins), this returns ALL matched
/// frameworks (deduplicated). When two or more signals match — e.g. both
/// `jest` and `vitest` in `devDependencies` — the caller can disclose the
/// ambiguity instead of silently resolving by fixed priority. Fail-closed:
/// empty when the manifest is unparseable or no signal matches.
fn detect_framework_signals(pkg_json: &str) -> Vec<TsFramework> {
    let Some(facts) = parse_manifest_facts(pkg_json) else {
        return Vec::new();
    };
    let has_dep = |name: &str| facts.dep_names.iter().any(|dep| dep == name);
    let mut signals: Vec<TsFramework> = Vec::new();

    // ── Dependency-signal priority (most reliable) ──────────────────────────
    if has_dep("jest") || has_dep("@types/jest") || has_dep("ts-jest") || has_dep("babel-jest") {
        push_framework_signal(&mut signals, TsFramework::Jest);
    }
    if has_dep("vitest") {
        push_framework_signal(&mut signals, TsFramework::Vitest);
    }
    if has_dep("bun-types") {
        push_framework_signal(&mut signals, TsFramework::Bun);
    }
    if has_dep("ava") {
        push_framework_signal(&mut signals, TsFramework::Ava);
    }
    if has_dep("mocha") || has_dep("@types/mocha") {
        push_framework_signal(&mut signals, TsFramework::Mocha);
    }
    if has_dep("@types/node") {
        push_framework_signal(&mut signals, TsFramework::NodeTest);
    }
    if !signals.is_empty() {
        return signals;
    }

    // ── Script-name fallback (handles composite scripts like "xo && ava") ───
    // Only activates when no dep match was found above (fail-closed). All
    // matched words are collected so a composite script mentioning two
    // frameworks (e.g. `"jest && vitest run"`) discloses its ambiguity.
    let Some(test_script) = facts.test_script else {
        return signals;
    };
    let script_has_word = |word: &str| -> bool {
        let mut haystack: &str = test_script.as_str();
        while let Some(pos) = haystack.find(word) {
            let before_ok = pos == 0
                || haystack
                    .as_bytes()
                    .get(pos - 1)
                    .is_some_and(|b| !b.is_ascii_alphanumeric() && *b != b'_' && *b != b'-');
            let after = pos + word.len();
            let after_ok = after >= haystack.len()
                || haystack
                    .as_bytes()
                    .get(after)
                    .is_some_and(|b| !b.is_ascii_alphanumeric() && *b != b'_' && *b != b'-');
            if before_ok && after_ok {
                return true;
            }
            haystack = &haystack[pos + 1..];
        }
        false
    };
    if script_has_word("jest") {
        push_framework_signal(&mut signals, TsFramework::Jest);
    }
    if script_has_word("vitest") {
        push_framework_signal(&mut signals, TsFramework::Vitest);
    }
    if script_has_word("ava") {
        push_framework_signal(&mut signals, TsFramework::Ava);
    }
    if script_has_word("mocha") {
        push_framework_signal(&mut signals, TsFramework::Mocha);
    }
    // "node --test" or "node:test" patterns
    if test_script.contains("node --test") || test_script.contains("node:test") {
        push_framework_signal(&mut signals, TsFramework::NodeTest);
    }
    // "bun test" pattern (script-only; dep signal already caught bun-types above)
    if test_script.contains("bun test") || test_script.starts_with("bun ") {
        push_framework_signal(&mut signals, TsFramework::Bun);
    }
    signals
}

/// Detect the runner from `scripts.test` in `package.json`.
fn detect_runner_from_scripts(pkg_json: &str) -> Option<TsRunner> {
    let facts = parse_manifest_facts(pkg_json)?;
    let script = facts.test_script?;

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

/// Field check: does the parsed `package.json` object carry a top-level
/// `workspaces` key?  Uses the serde parse (see [`parse_manifest_facts`]) so
/// the word `"workspaces"` inside an unrelated string value does NOT mark a
/// monorepo root. Fail-closed: `false` for unparseable manifests.
fn json_has_workspaces_field(pkg_json: &str) -> bool {
    parse_manifest_facts(pkg_json).is_some_and(|facts| facts.has_workspaces)
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
    let file_str = shell_quote_file_arg(&normalized_path(&rel_file));

    // Framework takes priority over runner.
    let cmd = match discovery.framework_hint {
        Some(TsFramework::Bun) => format!("bun test {file_str}"),
        Some(TsFramework::Vitest) => format!("vitest run {file_str}"),
        Some(TsFramework::Jest) => format!("jest {file_str}"),
        Some(TsFramework::NodeTest) => format!("node --test {file_str}"),
        Some(TsFramework::Ava) => format!("ava {file_str}"),
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

/// Shell-quote a test-file argument for the suggested verify command.
///
/// The primary consumer of the suggested command is an agent that may run it
/// verbatim in a POSIX-like shell, so a hostile file name such as
/// `x$(evil-cmd|sh).test.ts` (legal on Linux) must not become a
/// copy-paste code-execution vector. Plain alphanumeric/relative paths pass
/// through unchanged so the common command stays readable; anything else is
/// single-quoted with embedded single quotes escaped POSIX-style (`'\''`),
/// which suppresses all shell expansion.
fn shell_quote_file_arg(file_str: &str) -> String {
    if !file_str.is_empty()
        && file_str
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-'))
    {
        return file_str.to_string();
    }
    format!("'{}'", file_str.replace('\'', "'\\''"))
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

    fn unique_test_root(label: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("ripr-ts-fw-{label}-{}-{stamp}", std::process::id()))
    }

    #[test]
    fn detect_framework_for_root_uses_package_json_signals() -> Result<(), String> {
        // The #2106 case: ava only visible via package.json — previously
        // doctor reported "not detected" for this working setup.
        let root = unique_test_root("ava");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        std::fs::write(
            root.join("package.json"),
            r#"{"name":"ky","scripts":{"test":"xo && npm run build && ava"}}"#,
        )
        .map_err(|err| format!("write package.json: {err}"))?;

        assert_eq!(detect_framework_for_root(&root), Some(TsFramework::Ava));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn detect_framework_for_root_falls_back_to_config_markers() -> Result<(), String> {
        let root = unique_test_root("jest-config");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        std::fs::write(
            root.join("jest.config.js"),
            "module.exports = {};
",
        )
        .map_err(|err| format!("write jest config: {err}"))?;

        assert_eq!(detect_framework_for_root(&root), Some(TsFramework::Jest));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn detect_framework_for_root_accepts_both_bun_lockfile_names() -> Result<(), String> {
        for lockfile in ["bun.lockb", "bun.lock"] {
            let root = unique_test_root(lockfile);
            std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
            std::fs::write(root.join(lockfile), "").map_err(|err| format!("write lock: {err}"))?;
            assert_eq!(detect_framework_for_root(&root), Some(TsFramework::Bun));
            std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        }
        Ok(())
    }

    #[test]
    fn detect_framework_for_root_is_fail_closed_for_empty_root() -> Result<(), String> {
        let root = unique_test_root("empty");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;

        assert_eq!(detect_framework_for_root(&root), None);

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
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
    fn string_value_equal_to_framework_name_not_credited() {
        // `"jest"` here is a string VALUE, not a dependency key; it must not
        // be credited as framework evidence.
        let pkg = r#"{"name":"demo","description":"jest"}"#;
        let result = detect_framework(pkg);
        assert_eq!(
            result, None,
            "a framework name appearing as a string value must not be credited"
        );
    }

    #[test]
    fn spaced_dependency_key_still_credited() {
        // JSON keys may have whitespace before the colon; the quoted-key
        // heuristic must still credit them.
        let pkg = r#"{"devDependencies":{ "vitest" : "^1.0.0" }}"#;
        let result = detect_framework(pkg);
        assert_eq!(
            result,
            Some(TsFramework::Vitest),
            "spaced dependency keys must still be credited"
        );
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

    #[test]
    fn detect_framework_ava_from_dev_dep() {
        let result =
            detect_framework(r#"{"devDependencies":{"ava":"^6.0.0","typescript":"^5.0.0"}}"#);
        assert_eq!(result, Some(TsFramework::Ava));
    }

    #[test]
    fn detect_framework_ava_from_script_composite_ky_pattern() {
        // Ky-style composite script: ava is the runner but not a dep key.
        let pkg = r#"{"name":"ky","scripts":{"test":"xo && npm run build && ava"}}"#;
        let result = detect_framework(pkg);
        assert_eq!(result, Some(TsFramework::Ava));
    }

    #[test]
    fn detect_framework_ava_from_script_only() {
        // ava in scripts.test only (no devDep)
        let pkg = r#"{"scripts":{"test":"ava"}}"#;
        let result = detect_framework(pkg);
        assert_eq!(result, Some(TsFramework::Ava));
    }

    #[test]
    fn detect_framework_dep_wins_over_script_for_jest_with_ava_script() {
        // jest devDep wins even if ava appears in script (dep-key priority)
        let pkg = r#"{"devDependencies":{"jest":"^29.0.0"},"scripts":{"test":"ava"}}"#;
        let result = detect_framework(pkg);
        assert_eq!(result, Some(TsFramework::Jest));
    }

    #[test]
    fn detect_framework_no_match_no_guess() {
        // No known framework dep or script → fail-closed, no guess
        let pkg = r#"{"scripts":{"test":"xo && npm run build"}}"#;
        let result = detect_framework(pkg);
        assert_eq!(result, None, "must not guess a runner that isn't there");
    }

    #[test]
    fn detect_framework_vitest_from_script_only() {
        let pkg = r#"{"scripts":{"test":"vitest run"}}"#;
        let result = detect_framework(pkg);
        assert_eq!(result, Some(TsFramework::Vitest));
    }

    #[test]
    fn detect_framework_node_test_from_script() {
        let pkg = r#"{"scripts":{"test":"node --test"}}"#;
        let result = detect_framework(pkg);
        assert_eq!(result, Some(TsFramework::NodeTest));
    }

    #[test]
    fn detect_framework_signals_single_match_has_no_ambiguity() {
        // One distinct framework signal → no ambiguity evidence.
        let signals = detect_framework_signals(jest_pkg_json());
        assert_eq!(signals, vec![TsFramework::Jest]);
    }

    #[test]
    fn detect_framework_signals_jest_vitest_collects_both_matches() {
        // Two distinct framework signals → both collected (priority order),
        // so the caller can disclose the ambiguity instead of silently
        // resolving by fixed priority.
        let pkg = r#"{"devDependencies":{"jest":"^29.0.0","vitest":"^1.0.0"}}"#;
        let signals = detect_framework_signals(pkg);
        assert_eq!(signals, vec![TsFramework::Jest, TsFramework::Vitest]);
        // First-match-wins resolution is unchanged for single consumers.
        assert_eq!(detect_framework(pkg), Some(TsFramework::Jest));
    }

    #[test]
    fn detect_framework_jest_and_types_jest_count_as_one_signal() {
        // `jest` + `@types/jest` are two dep names for ONE framework — they
        // must NOT be reported as ambiguous.
        let signals = detect_framework_signals(jest_pkg_json());
        assert_eq!(signals, vec![TsFramework::Jest]);
    }

    #[test]
    fn detect_framework_string_value_mentioning_jest_is_not_evidence() {
        // A framework name inside a free-text string value ("description")
        // must NOT be treated as a dependency. The old quoted-key substring
        // scan false-detected jest here.
        let pkg = r#"{"name":"x","description":"jest","private":true}"#;
        let result = detect_framework(pkg);
        assert_eq!(
            result, None,
            "a framework name inside a string value is not dependency evidence"
        );
    }

    #[test]
    fn test_script_key_outside_scripts_is_not_the_test_script() {
        // A `"test"` key nested outside `scripts` must not feed framework or
        // runner script detection; only `scripts.test` is the test script.
        let pkg = r#"{"config":{"test":"ava"},"scripts":{"test":"echo no-op"}}"#;
        let result = detect_framework(pkg);
        assert_eq!(
            result, None,
            "\"test\" key outside scripts must not be read as the test script"
        );
        let runner = detect_runner_from_scripts(pkg);
        assert_eq!(runner, None);
    }

    #[test]
    fn workspaces_word_inside_string_value_is_not_monorepo_root() {
        // The word "workspaces" inside a string value must NOT mark a monorepo
        // root; only a real top-level `workspaces` field counts.
        let pkg = r#"{"name":"x","description":"uses workspaces for discovery"}"#;
        assert!(
            !json_has_workspaces_field(pkg),
            "workspaces inside a string value must not mark a monorepo root"
        );
        // Positive control: a real top-level field is still detected.
        let real = r#"{"name":"x","workspaces":["packages/*"]}"#;
        assert!(json_has_workspaces_field(real));
    }

    #[test]
    fn evidence_lines_emits_typescript_test_runner_when_framework_detected() {
        // When framework_hint is Some, evidence_lines() must include
        // typescript_test_runner: <name> as an additive field.
        let discovery = PackageDiscovery {
            package_root: Some(PathBuf::from(".")),
            workspace_root: Some(PathBuf::from(".")),
            framework_hint: Some(TsFramework::Ava),
            runner_hint: None,
            confidence: TsPackageConfidence::Medium,
            limitations: Vec::new(),
        };
        let lines = discovery.evidence_lines();
        assert!(
            lines.iter().any(|l| l == "typescript_test_runner: ava"),
            "expected typescript_test_runner: ava in evidence lines; got: {lines:?}"
        );
    }

    #[test]
    fn evidence_lines_no_typescript_test_runner_when_no_framework() {
        // When framework_hint is None, no typescript_test_runner line is emitted.
        let discovery = PackageDiscovery {
            package_root: None,
            workspace_root: None,
            framework_hint: None,
            runner_hint: None,
            confidence: TsPackageConfidence::None,
            limitations: vec![TsPackageLimitation::PackageRootNotFound],
        };
        let lines = discovery.evidence_lines();
        assert!(
            !lines
                .iter()
                .any(|l| l.starts_with("typescript_test_runner: ")),
            "must not emit typescript_test_runner when no framework: {lines:?}"
        );
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
    fn ts_package_discovery_jest_vitest_ambiguity_is_disclosed() {
        let root = unique_test_dir("jest-vitest-ambiguous");
        // Both jest and vitest present — two distinct framework signals.
        let pkg = r#"{
  "name": "multi-runner",
  "devDependencies": {
    "jest": "^29.0.0",
    "vitest": "^1.0.0"
  },
  "scripts": {
    "test": "vitest run"
  }
}"#;
        setup_single_package(&root, pkg, Some("package-lock.json"));
        let test_file = PathBuf::from("tests/math.test.ts");

        let result = resolve_package_discovery(&test_file, &root);

        // Fixed priority still picks the first signal for consumers that need
        // a single runner name...
        assert_eq!(result.framework_hint, Some(TsFramework::Jest));
        // ...but the ambiguity is disclosed additively (classification-neutral).
        assert!(
            result
                .limitations
                .contains(&TsPackageLimitation::FrameworkAmbiguous),
            "expected typescript_test_runner_ambiguous; got {:?}",
            result.limitations
        );
        // Ambiguous evidence is capped at medium confidence.
        assert_eq!(
            result.confidence,
            TsPackageConfidence::Medium,
            "ambiguous runner evidence must not claim high confidence"
        );
        let lines = result.evidence_lines();
        assert!(
            lines
                .iter()
                .any(|l| l == "typescript_package_limitation: typescript_test_runner_ambiguous"),
            "evidence lines missing typescript_test_runner_ambiguous: {:?}",
            lines
        );
        // The resolved runner name is still emitted for consumers.
        assert!(
            lines.iter().any(|l| l == "typescript_test_runner: jest"),
            "evidence lines missing typescript_test_runner: jest: {:?}",
            lines
        );
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
    fn verify_command_framework_ava_produces_ava_command() {
        let discovery = make_discovery(Some("."), Some(TsFramework::Ava), None);
        let result = verify_command_for_discovery(&discovery, Path::new("tests/math.test.ts"));
        assert_eq!(result, Some("ava tests/math.test.ts".to_string()));
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

    #[test]
    fn verify_command_hostile_filename_is_shell_quoted() {
        // Security: a file name containing shell metacharacters is legal on
        // Linux and must not become a code-exec vector when an agent runs the
        // suggested command verbatim.
        let discovery = make_discovery(Some("."), Some(TsFramework::Jest), None);
        let result =
            verify_command_for_discovery(&discovery, Path::new("tests/x$(evil-cmd|sh).test.ts"));
        assert_eq!(
            result,
            Some("jest 'tests/x$(evil-cmd|sh).test.ts'".to_string()),
            "metacharacters must be neutralized by single-quoting"
        );
    }

    #[test]
    fn verify_command_filename_with_space_is_shell_quoted() {
        let discovery = make_discovery(Some("."), Some(TsFramework::Jest), None);
        let result = verify_command_for_discovery(&discovery, Path::new("tests/my file.test.ts"));
        assert_eq!(result, Some("jest 'tests/my file.test.ts'".to_string()));
    }

    #[test]
    fn verify_command_filename_with_single_quote_is_escaped() {
        let discovery = make_discovery(Some("."), Some(TsFramework::Jest), None);
        let result = verify_command_for_discovery(&discovery, Path::new("tests/o'brien.test.ts"));
        assert_eq!(
            result,
            Some("jest 'tests/o'\\''brien.test.ts'".to_string()),
            "embedded single quote must be escaped POSIX-style"
        );
    }

    #[test]
    fn verify_command_plain_filename_stays_unquoted() {
        // Readability: the common case must not acquire quotes.
        let discovery = make_discovery(Some("."), Some(TsFramework::Jest), None);
        let result = verify_command_for_discovery(&discovery, Path::new("tests/math.test.ts"));
        assert_eq!(result, Some("jest tests/math.test.ts".to_string()));
    }

    // ── Gap-3 honesty-clarity controls (RIPR-SPEC-0101) ──────────────────────

    /// Control 1: vitest devDep + no lockfile.
    ///
    /// Reproduces issue #1239: framework=Vitest, runner=None.
    /// `typescript_package_manager_unresolved` MUST be emitted (informational).
    /// `typescript_runner_hint_unresolved` MUST NOT be emitted (misleading).
    /// A `vitest run` command MUST be available.
    #[test]
    fn gap3_vitest_no_lockfile_emits_package_manager_unresolved_not_runner_unresolved() {
        let root = unique_test_dir("gap3-vitest-no-lockfile");
        // package.json has vitest devDep; no lockfile present.
        setup_single_package(&root, vitest_pkg_json(), None);
        let test_file = PathBuf::from("tests/math.test.ts");

        let result = resolve_package_discovery(&test_file, &root);

        // Framework known, runner absent.
        assert_eq!(
            result.framework_hint,
            Some(TsFramework::Vitest),
            "framework must be detected"
        );
        assert_eq!(result.runner_hint, None, "runner must be absent");

        // New informational limitation emitted.
        assert!(
            result
                .limitations
                .contains(&TsPackageLimitation::PackageManagerUnresolved),
            "expected typescript_package_manager_unresolved; got {:?}",
            result.limitations
        );
        // Old strong fail-closed limitation must NOT be emitted.
        assert!(
            !result
                .limitations
                .contains(&TsPackageLimitation::RunnerHintMissing),
            "typescript_runner_hint_unresolved must not be emitted when framework is known; got {:?}",
            result.limitations
        );

        // A verify command must be derivable from the framework alone.
        let cmd = verify_command_for_discovery(&result, &test_file);
        assert!(
            cmd.is_some(),
            "verify command must be available when framework is known; got None"
        );
        let cmd_str = cmd.unwrap_or_default();
        assert!(
            cmd_str.starts_with("vitest run"),
            "expected 'vitest run ...' command; got: {cmd_str}"
        );
    }

    /// Control 2: package.json present, no known framework AND no lockfile.
    ///
    /// Genuinely unresolvable: `typescript_runner_hint_unresolved` (strong,
    /// fail-closed) MUST be emitted; no verify command.
    #[test]
    fn gap3_no_framework_no_lockfile_emits_strong_runner_unresolved() {
        let root = unique_test_dir("gap3-no-fw-no-lock");
        // package.json with only typescript in devDeps — no known framework.
        let minimal = r#"{"name":"no-fw","devDependencies":{"typescript":"^5.0.0"}}"#;
        // No lockfile.
        setup_single_package(&root, minimal, None);
        let test_file = PathBuf::from("tests/foo.test.ts");

        let result = resolve_package_discovery(&test_file, &root);

        assert_eq!(result.framework_hint, None, "framework must be absent");
        assert_eq!(result.runner_hint, None, "runner must be absent");

        // Strong fail-closed limitation must be emitted.
        assert!(
            result
                .limitations
                .contains(&TsPackageLimitation::RunnerHintMissing),
            "expected typescript_runner_hint_unresolved; got {:?}",
            result.limitations
        );
        // Informational limitation must NOT be emitted (no framework to claim it's ok).
        assert!(
            !result
                .limitations
                .contains(&TsPackageLimitation::PackageManagerUnresolved),
            "typescript_package_manager_unresolved must not be emitted when framework is absent; got {:?}",
            result.limitations
        );

        // No verify command — genuinely fail-closed.
        let cmd = verify_command_for_discovery(&result, &test_file);
        assert_eq!(
            cmd, None,
            "no verify command must be emitted when neither framework nor runner resolves"
        );
    }

    /// Control 3: vitest + pnpm-lock.yaml — both resolved, no limitation emitted.
    #[test]
    fn gap3_vitest_pnpm_lockfile_no_limitation() {
        let root = unique_test_dir("gap3-vitest-pnpm");
        setup_single_package(&root, vitest_pkg_json(), Some("pnpm-lock.yaml"));
        let test_file = PathBuf::from("tests/math.test.ts");

        let result = resolve_package_discovery(&test_file, &root);

        assert_eq!(result.framework_hint, Some(TsFramework::Vitest));
        assert_eq!(result.runner_hint, Some(TsRunner::Pnpm));
        assert!(
            !result
                .limitations
                .contains(&TsPackageLimitation::PackageManagerUnresolved),
            "must not emit typescript_package_manager_unresolved when runner resolved"
        );
        assert!(
            !result
                .limitations
                .contains(&TsPackageLimitation::RunnerHintMissing),
            "must not emit typescript_runner_hint_unresolved when runner resolved"
        );

        let cmd = verify_command_for_discovery(&result, &test_file);
        assert!(cmd.is_some(), "verify command must be available");
    }
}
