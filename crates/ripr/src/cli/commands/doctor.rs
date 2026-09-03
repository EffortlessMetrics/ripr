//! Dispatch and environment reporting for `ripr doctor`.
//!
//! This is the CLI adapter layer only. Analysis, evaluation, and rendering
//! semantics live in `crate::app`, `crate::analysis`, and `crate::output`.
//! This module owns argv parsing, probe/report printing, and exit mapping
//! for the doctor command family.

use crate::analysis;
use crate::app::Mode;
use crate::cli::help;
use crate::cli::suggest::unknown_argument;
use crate::config::{CONFIG_FILE_NAME, DEFAULT_LSP_SEAM_DIAGNOSTICS, RiprConfig, load_for_root};
use crate::domain::{LanguageId, LanguageStatus};
use crate::output;
use std::path::{Path, PathBuf};

pub(in crate::cli) fn doctor(args: &[String]) -> Result<(), String> {
    let mut json_output = false;
    let mut root_args: Vec<&str> = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--help" | "-h" => {
                help::print_doctor_help();
                return Ok(());
            }
            "--json" => json_output = true,
            _ => root_args.push(arg.as_str()),
        }
    }
    let root = match root_args.as_slice() {
        [] => PathBuf::from("."),
        ["--root"] => return Err("missing value for --root".to_string()),
        ["--root", value] => PathBuf::from(value),
        [other, ..] => return Err(unknown_argument("doctor", other)),
    };

    if json_output {
        return doctor_json(&root);
    }

    // Human-readable path (unchanged behavior).
    let core_evaluation = output::doctor::evaluate_doctor_core_with_config(&root);
    let mut report = core_evaluation.report;
    let core_report = &report;
    let mut ok = matches!(core_report.status, output::doctor::DoctorStatus::Pass);
    let enabled_languages = enabled_languages(&core_evaluation.config);
    println!("ripr doctor");
    println!("- root: {}", root.display());

    ok &= report_doctor_core_check(core_report, "root_directory");
    ok &= report_doctor_core_check(core_report, "cargo_toml");
    report_config_status(&root, core_evaluation.config, &mut ok);
    report_cache_status(&root);
    report_detected_languages(&root);
    ok &= add_language_runtime_probes(&root, &enabled_languages, &mut report, true, probe_runtime);
    suggest_preview_language_enablement(&root);
    report_detected_test_surfaces(&root);
    report_perl_preview(&root);
    report_known_limitations();

    for tool in output::doctor::DOCTOR_TOOLS {
        ok &= report_doctor_core_check(&report, &format!("tool_{tool}"));
    }

    print_doctor_start_here_guidance(&root);

    if ok && report.status == output::doctor::DoctorStatus::Pass {
        println!("✓ doctor checks passed");
        Ok(())
    } else {
        println!("! doctor checks failed; run `ripr doctor --help` for usage");
        Err("doctor found issues".to_string())
    }
}

/// Typed JSON doctor output. Captures top-level checks and language runtime
/// probes as structured values. Deeper sub-checks (cache, Perl, and test
/// surfaces) remain on the human-oriented path for a follow-up PR to type
/// individually. See #1771 / #1614.
fn doctor_json(root: &Path) -> Result<(), String> {
    let evaluation = output::doctor::evaluate_doctor_core_with_config(root);
    let mut report = evaluation.report;
    let enabled_languages = enabled_languages(&evaluation.config);
    let _ =
        add_language_runtime_probes(root, &enabled_languages, &mut report, false, probe_runtime);
    println!("{}", report.render_json()?);
    output::doctor::doctor_report_result(&report)
}

fn enabled_languages(config: &Result<RiprConfig, String>) -> Vec<LanguageId> {
    config
        .as_ref()
        .map(|config| config.languages().enabled().to_vec())
        .unwrap_or_default()
}

fn report_doctor_core_check(report: &output::doctor::DoctorReport, name: &str) -> bool {
    let Some(check) = report.checks.iter().find(|check| check.name == name) else {
        println!("! missing doctor core check: {name}");
        return false;
    };
    let marker = match check.status {
        output::doctor::DoctorStatus::Pass => "✓",
        output::doctor::DoctorStatus::Fail => "!",
    };
    println!(
        "{marker} {}",
        check.evidence.as_deref().unwrap_or(check.name.as_str())
    );
    check.status == output::doctor::DoctorStatus::Pass
}

fn print_doctor_start_here_guidance(root: &Path) {
    println!("- Start-here packet: target/ripr/reports/start-here.md");
    println!(
        "- Safe next action: run `ripr first-pr --root {} --base origin/main --head HEAD` after setup passes",
        root.display()
    );
    println!(
        "- Recovery states: missing artifact, stale evidence, wrong root, malformed artifact, no actionable gap, preview-limited evidence"
    );
    println!(
        "- Proof rail: verify command, receipt command, and receipt path are advisory static movement evidence"
    );
    // First-run honesty: when the working tree has uncommitted changes,
    // `ripr check --base origin/main` analyzes committed history only and would
    // silently exclude the user's draft (the RIPR-SPEC-0112 dirty-worktree case).
    // Route them to the command that actually covers their edits instead of the
    // one that looks clean while ignoring them. Reuses the same helper as the
    // check-time disclosure (reuse, don't fork).
    if analysis::working_tree_has_tracked_changes(root) {
        println!("- Recommended first command: ripr check --base HEAD --worktree");
        println!(
            "- Scope note: `--worktree` analyzes staged and unstaged tracked edits; untracked files remain out of scope until staged or supplied through `--diff`."
        );
    } else {
        println!("- Recommended first command: ripr check --base origin/main");
    }
}

/// Language-to-status mapping used by the doctor first-run diagnosis.
///
/// Only Rust is `Stable`. All preview surfaces (TypeScript, JavaScript,
/// Python, Perl) carry `Preview` per `LanguageStatus::as_str()` and
/// RIPR-SPEC-0026.
fn language_status(id: LanguageId) -> LanguageStatus {
    match id {
        LanguageId::Rust => LanguageStatus::Stable,
        LanguageId::TypeScript | LanguageId::JavaScript | LanguageId::Python | LanguageId::Perl => {
            LanguageStatus::Preview
        }
    }
}

/// Shallow marker scan: look for files/dirs that indicate a language is
/// present. Only inspects `root`, `root/src/`, and immediate child dirs of
/// `root` — no recursion, no AST parsing, no workspace pipeline.
///
/// Returns `false` (no marker found) when any `read_dir` call fails; doctor
/// must never panic or OOM on a scan error.
fn shallow_has_extension(root: &Path, extension: &str) -> bool {
    let dirs_to_scan: [&Path; 2] = [root, &root.join("src")];
    for dir in dirs_to_scan {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some(extension) {
                    return true;
                }
            }
        }
    }
    // Also scan one level of child dirs of root.
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let child = entry.path();
            if child.is_dir() {
                let sub_entries = std::fs::read_dir(&child).into_iter().flatten().flatten();
                for sub in sub_entries {
                    let path = sub.path();
                    if path.extension().and_then(|e| e.to_str()) == Some(extension) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn shallow_has_file(root: &Path, name: &str) -> bool {
    root.join(name).exists() || root.join("src").join(name).exists()
}

/// Detect which languages have concrete file markers in this workspace.
/// Returns detected `LanguageId`s in a stable order (Rust first, then
/// TypeScript, JavaScript, Python, Perl).
fn detect_languages(root: &Path) -> Vec<LanguageId> {
    let mut found = Vec::new();

    // Rust: Cargo.toml at root OR .rs files in root/src
    if root.join("Cargo.toml").exists() || shallow_has_extension(root, "rs") {
        found.push(LanguageId::Rust);
    }

    // TypeScript: package.json, tsconfig.json, .ts or .tsx files
    if shallow_has_file(root, "package.json")
        || shallow_has_file(root, "tsconfig.json")
        || shallow_has_extension(root, "ts")
        || shallow_has_extension(root, "tsx")
    {
        found.push(LanguageId::TypeScript);
    }

    // JavaScript: .js or .jsx files (only when no TS markers already found)
    if !found.contains(&LanguageId::TypeScript)
        && (shallow_has_extension(root, "js") || shallow_has_extension(root, "jsx"))
    {
        found.push(LanguageId::JavaScript);
    }

    // Python: pyproject.toml, setup.py, setup.cfg, pytest.ini, or .py files
    if shallow_has_file(root, "pyproject.toml")
        || shallow_has_file(root, "setup.py")
        || shallow_has_file(root, "setup.cfg")
        || shallow_has_file(root, "pytest.ini")
        || shallow_has_extension(root, "py")
    {
        found.push(LanguageId::Python);
    }

    // Perl: .pl or .pm files
    if shallow_has_extension(root, "pl") || shallow_has_extension(root, "pm") {
        found.push(LanguageId::Perl);
    }

    found
}

/// Probe the runtimes the verify/proof route needs for each detected
/// language (#2071). A primary runtime is required only when its language is
/// enabled in the effective config. Detected-but-disabled preview runtimes
/// and optional test/package runners remain advisory.
fn add_language_runtime_probes<F>(
    root: &Path,
    enabled: &[LanguageId],
    report: &mut output::doctor::DoctorReport,
    print: bool,
    mut probe: F,
) -> bool
where
    F: FnMut(&str, bool) -> (output::doctor::DoctorStatus, String),
{
    let mut ok = true;
    for (language, tool, hint) in language_runtime_probes_for(root, enabled) {
        let (status, evidence) = probe(tool, tool == "yarn");
        let required = runtime_probe_is_required(language, tool, enabled);
        report.add_runtime_probe(language, tool, status, &evidence, required, hint);
        if required && status == output::doctor::DoctorStatus::Fail {
            ok = false;
        }
        if print && let Some(probe) = report.runtime_probes.last() {
            println!("{}", language_runtime_probe_display_line(probe));
        }
    }
    ok
}

fn probe_runtime(tool: &str, isolated: bool) -> (output::doctor::DoctorStatus, String) {
    // yarn loads project config on --version; probe it isolated so a
    // hostile checkout cannot execute code via doctor (#2183 review).
    if isolated {
        output::doctor::doctor_tool_check_isolated(tool)
    } else {
        output::doctor::doctor_tool_check(tool)
    }
}

fn runtime_probe_is_required(language: &str, tool: &str, enabled: &[LanguageId]) -> bool {
    // The language's primary runtime is the configured capability doctor can
    // require. A detected package manager or test runner is still useful
    // evidence, but remains advisory because a project may use another route.
    primary_runtime(language).is_some_and(|(primary, _)| primary == tool)
        && enabled.iter().any(|id| id.as_str() == language)
}

const PRIMARY_RUNTIME_PROBES: &[(&str, &str, &str)] = &[
    ("typescript", "node", "install Node.js"),
    ("javascript", "node", "install Node.js"),
    (
        "python",
        "python3",
        "install python3 (e.g. apt install python3)",
    ),
];

fn primary_runtime(language: &str) -> Option<(&'static str, &'static str)> {
    PRIMARY_RUNTIME_PROBES
        .iter()
        .find(|(candidate, _, _)| *candidate == language)
        .map(|(_, tool, hint)| (*tool, *hint))
}

fn language_runtime_probes_for(
    root: &Path,
    enabled: &[LanguageId],
) -> Vec<(&'static str, &'static str, &'static str)> {
    let mut probes = language_runtime_probes(root);
    append_missing_primary_runtime_probes(&mut probes, enabled);
    probes
}

fn append_missing_primary_runtime_probes(
    probes: &mut Vec<(&'static str, &'static str, &'static str)>,
    enabled: &[LanguageId],
) {
    for (language, tool, hint) in PRIMARY_RUNTIME_PROBES {
        let enabled_language = enabled.iter().any(|id| id.as_str() == *language);
        if enabled_language
            && !probes
                .iter()
                .any(|(found, detected_tool, _)| *found == *language && *detected_tool == *tool)
        {
            probes.push((language, tool, hint));
        }
    }
}

fn language_runtime_probe_display_line(probe: &output::doctor::DoctorRuntimeProbe) -> String {
    if probe.status == output::doctor::DoctorStatus::Fail && !probe.required {
        format!(
            "- {} verify-route runtime: {} — optional; {}",
            probe.language, probe.evidence, probe.hint
        )
    } else {
        language_runtime_probe_line(&probe.language, probe.status, &probe.evidence, &probe.hint)
    }
}

/// One doctor output line for a runtime probe (#2071). Pure so the emitted
/// contract (labels, evidence, install hint) is directly testable (#2183
/// review).
fn language_runtime_probe_line(
    language: &str,
    status: output::doctor::DoctorStatus,
    evidence: &str,
    hint: &str,
) -> String {
    match status {
        output::doctor::DoctorStatus::Pass => {
            format!("✓ {language} verify-route runtime: {evidence}")
        }
        output::doctor::DoctorStatus::Fail => {
            format!("! {language} verify-route runtime: {evidence} — {hint}")
        }
    }
}

/// The (language, tool, install hint) runtime probes for a root (#2071).
/// Factored from the printer so the probe list is directly testable.
fn language_runtime_probes(root: &Path) -> Vec<(&'static str, &'static str, &'static str)> {
    let detected = detect_languages(root);
    let mut probes: Vec<(&str, &str, &str)> = Vec::new();
    if detected.contains(&LanguageId::Python) {
        probes.push((
            "python",
            "python3",
            "install python3 (e.g. apt install python3)",
        ));
        // Reuse the shared framework detector (#2183 review) — no parallel
        // marker list. Gated behind lang-python: the detector lives in the
        // Python adapter which is not compiled under --no-default-features
        // --features lang-rust (#2418).
        #[cfg(feature = "lang-python")]
        {
            if analysis::detect_python_test_framework(root) == Some("pytest") {
                probes.push((
                    "python",
                    "pytest",
                    "install pytest (e.g. pip install pytest)",
                ));
            }
        }
    }
    for id in [LanguageId::TypeScript, LanguageId::JavaScript] {
        if !detected.contains(&id) {
            continue;
        }
        // Label with the actually-detected language (#2183 review): a
        // JS-only workspace must not read "typescript".
        let lang = id.as_str();
        probes.push((lang, "node", "install Node.js"));
        if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
            probes.push((lang, "bun", "install Bun"));
        }
        if root.join("pnpm-lock.yaml").exists() {
            probes.push((lang, "pnpm", "install pnpm"));
        }
        if root.join("yarn.lock").exists() {
            probes.push((lang, "yarn", "install Yarn"));
        }
    }
    probes
}

/// Print `- Detected languages: rust (stable), typescript (preview), …`
///
/// Each entry shows its canonical `LanguageStatus` tier in parentheses.
/// Appends `[adapter not compiled]` when `LanguageId::is_available()` is
/// false for the detected language. If no markers are found, prints
/// `none detected` rather than claiming any language.
fn report_detected_languages(root: &Path) {
    let detected = detect_languages(root);
    if detected.is_empty() {
        println!("- Detected languages: none detected");
        return;
    }
    let entries: Vec<String> = detected
        .iter()
        .map(|id| {
            let tier = language_status(*id).as_str().to_string();
            let available = id.is_available();
            if available {
                format!("{} ({})", id.as_str(), tier)
            } else {
                format!("{} ({}) [adapter not compiled]", id.as_str(), tier)
            }
        })
        .collect();
    println!("- Detected languages: {}", entries.join(", "));
}

/// When a preview language is detected in `root` but is not yet enabled in
/// `ripr.toml`, print a copy-paste-ready TOML block so the user can enable
/// it in a single edit.
///
/// Gated on BOTH conditions to stay fail-closed:
/// 1. `LanguageId::is_available()` — the adapter was compiled into this binary
///    (`cfg!(feature = "lang-<x>")`). If the feature was not compiled in, a
///    user cannot enable the adapter regardless of `ripr.toml`.
/// 2. The language is NOT already in `config.languages().enabled()`.
///
/// Emits nothing when either condition fails, when the root has no config
/// file, or when the config cannot be loaded.
fn suggest_preview_language_enablement(root: &Path) {
    for line in preview_language_enable_suggestions(root) {
        println!("{line}");
    }
}

/// Pure computation for `suggest_preview_language_enablement` — returns the
/// tip lines (ready to print) for each preview language that is detected,
/// available (compiled in), and not yet enabled in `ripr.toml`.
///
/// Returns an empty vec when there is nothing to suggest. Separated from the
/// printing logic so it can be covered by unit tests without stdout capture.
fn preview_language_enable_suggestions(root: &Path) -> Vec<String> {
    let detected = detect_languages(root);
    let preview_detected: Vec<LanguageId> = detected
        .into_iter()
        .filter(|id| matches!(language_status(*id), LanguageStatus::Preview))
        .collect();
    if preview_detected.is_empty() {
        return Vec::new();
    }
    let config = match load_for_root(root) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let enabled = config.languages().enabled();
    let mut suggestions = Vec::new();
    for id in &preview_detected {
        if id.is_available() && !enabled.contains(id) {
            // Perl detects as a preview language (see language_status). In a
            // default build, `LanguageId::Perl.is_available()` is
            // `cfg!(feature="lang-perl")` == false, so the Tip never fires for
            // Perl anyway. This guard is defense-in-depth for the
            // `--features lang-perl` build: even when the Cargo feature is ON,
            // the adapter is still scaffold-only (#[cfg(test)] mod perl; not
            // production-routable, pipeline fail-closed stub). Suggesting
            // `enabled = ["rust", "perl"]` in that build would mislead: the
            // user would enable it and get zero analysis plus an explicit
            // error. Detection at detect_languages() stays honest; only the
            // enablement Tip is suppressed for Perl until Campaign 31 (#1379)
            // lands the production bridge. TypeScript/Python are real preview
            // adapters and remain Tip-eligible.
            if matches!(id, LanguageId::Perl) {
                continue;
            }
            suggestions.push(format!(
                "- Tip: {} files detected but the adapter is not enabled. To analyze them, add to ripr.toml:\n\n  [languages]\n  enabled = [\"rust\", \"{}\"]",
                id.as_str(),
                id.as_str(),
            ));
        }
    }
    suggestions
}

/// Detect test-framework markers per detected language.
///
/// Reports `<lang>: test framework not detected` rather than guessing when
/// no clear marker is found — the function never claims a framework it cannot
/// confirm.
fn report_detected_test_surfaces(root: &Path) {
    let lines = detected_test_surface_lines(root);
    if !lines.is_empty() {
        println!("- Detected test surfaces: {}", lines.join("; "));
    }
}

/// Build the detected test-surface lines for doctor (#2106). Split from the
/// printer so the output contract is directly testable.
fn detected_test_surface_lines(root: &Path) -> Vec<String> {
    let detected = detect_languages(root);
    if detected.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<String> = Vec::new();
    for id in &detected {
        match id {
            LanguageId::Rust => {
                // Cargo.toml presence is the Rust test surface marker
                // (`cargo test` and `#[cfg(test)]` are available in any
                // Cargo workspace).
                if root.join("Cargo.toml").exists() {
                    lines.push("rust: cargo test (#[cfg(test)])".to_string());
                } else {
                    lines.push("rust: test framework not detected".to_string());
                }
            }
            LanguageId::Python => {
                // One shared detector (#2106): the same pytest/unittest
                // marker set the adapter's code-level detection implies.
                #[cfg(feature = "lang-python")]
                let framework = analysis::detect_python_test_framework(root);
                #[cfg(not(feature = "lang-python"))]
                let framework: Option<&'static str> =
                    if root.join("pytest.ini").exists() || root.join("pyproject.toml").exists() {
                        Some("pytest")
                    } else {
                        None
                    };
                match framework {
                    Some(name) => lines.push(format!("python: {name}")),
                    None => lines.push("python: test framework not detected".to_string()),
                }
            }
            LanguageId::TypeScript | LanguageId::JavaScript => {
                // One shared detector (#2106): the same package.json /
                // config-file signals the adapter's package discovery trusts.
                let lang = id.as_str();
                #[cfg(feature = "lang-typescript")]
                let framework = analysis::detect_typescript_test_framework(root);
                #[cfg(not(feature = "lang-typescript"))]
                let framework: Option<&'static str> = if root.join("jest.config.js").exists()
                    || root.join("jest.config.ts").exists()
                    || root.join("jest.config.mjs").exists()
                    || root.join("jest.config.cjs").exists()
                {
                    Some("jest")
                } else if root.join("vitest.config.ts").exists()
                    || root.join("vitest.config.js").exists()
                    || root.join("vitest.config.mjs").exists()
                {
                    Some("vitest")
                } else if root.join("bun.lockb").exists() {
                    Some("bun")
                } else {
                    None
                };
                match framework {
                    Some(name) => lines.push(format!("{lang}: {name}")),
                    None => lines.push(format!("{lang}: test framework not detected")),
                }
            }
            LanguageId::Perl => {
                // Phase D PR 2 (#1408): upgraded Perl doctor diagnostics.
                let pm_count = count_files(root, "pm");
                let pl_count = count_files(root, "pl");
                let t_count = count_files(root, "t");
                if pm_count > 0 || pl_count > 0 || t_count > 0 {
                    let framework = detect_perl_framework(root);
                    lines.push(format!(
                        "perl: {} .pm, {} .pl, {} .t; framework: {}",
                        pm_count, pl_count, t_count, framework
                    ));
                    // Report adapter availability.
                    if id.is_available() {
                        lines.push("perl: adapter compiled (lang-perl feature ON)".to_string());
                    } else {
                        lines.push(
                            "perl: adapter NOT compiled (build with --features lang-perl)"
                                .to_string(),
                        );
                    }
                    // Report runner availability.
                    if which("prove") {
                        lines.push("perl: prove available on PATH".to_string());
                    } else {
                        lines.push("perl: prove NOT found on PATH".to_string());
                    }
                    // Report exact first command.
                    if id.is_available() {
                        lines.push("perl: first command: ripr check --perl-facts <packet.json> --diff <diff.patch> --json".to_string());
                    }
                } else {
                    lines.push("perl: no Perl files detected".to_string());
                }
            }
        }
    }
    lines
}

/// Count files with a given extension under the root (recursive). Used by the
/// Perl preview to report real .pm/.pl/.t counts. Campaign 31 item 5: the
/// prior `shallow_has_extension as usize` returned only 0/1, not a real count.
fn count_files(root: &Path, ext: &str) -> usize {
    fn count_recursive(dir: &Path, ext: &str) -> usize {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return 0;
        };
        let mut n = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip hidden + build/dependency dirs that inflate counts.
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if name.starts_with('.') || matches!(name, "target" | "node_modules" | "blib") {
                    continue;
                }
                n += count_recursive(&path, ext);
            } else if path.extension().and_then(|e| e.to_str()) == Some(ext) {
                n += 1;
            }
        }
        n
    }
    count_recursive(root, ext)
}

/// Detect the Perl test framework from .t files (shallow scan).
fn detect_perl_framework(root: &Path) -> &'static str {
    let t_dir = root.join("t");
    let Ok(entries) = std::fs::read_dir(&t_dir) else {
        return "not detected (no t/ directory)";
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "t")
            && let Ok(content) = std::fs::read_to_string(&path)
        {
            if content.contains("use Test2::V0") {
                return "Test2::V0";
            }
            if content.contains("use Test::More") {
                return "Test::More";
            }
            if content.contains("use Test::Exception") {
                return "Test::Exception";
            }
            if content.contains("use Test::Fatal") {
                return "Test::Fatal";
            }
        }
    }
    "not detected"
}

/// Check if a binary is available on PATH.
fn which(bin: &str) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("which")
            .arg(bin)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
    #[cfg(not(unix))]
    {
        std::process::Command::new("where")
            .arg(bin)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
}

/// Rich Perl preview for the doctor (Campaign 31 item 5). Reports everything a
/// maintainer needs to know whether Perl analysis is available and how to
/// invoke it: project markers, lang-perl compiled, [perl] producer configured,
/// perllsp/perl-lsp found + version, schema compatible, t/ and t2/ roots,
/// detected test frameworks, runner availability, and an exact next command.
///
/// Conservative throughout: every line reports only what the static layer can
/// determine. No claim is made that the producer works end-to-end (that is the
/// two-binary proof, item 3). Prints only when Perl markers are detected.
fn report_perl_preview(root: &Path) {
    let pm_count = count_files(root, "pm");
    let pl_count = count_files(root, "pl");
    let t_count = count_files(root, "t");
    let has_markers = pm_count > 0 || pl_count > 0 || t_count > 0 || has_perl_project_markers(root);
    if !has_markers {
        return;
    }

    println!("- Perl preview:");
    println!("  project: {pm_count} .pm, {pl_count} .pl, {t_count} .t");

    // lang-perl compiled? (cfg!(feature = "lang-perl") is build-time constant.)
    if cfg!(feature = "lang-perl") {
        println!("  adapter: compiled (lang-perl feature ON)");
    } else {
        println!("  adapter: NOT compiled (build with --features lang-perl)");
    }

    // [perl] producer configured? + Perl facts exporter found? + version?
    let producer_configured = perl_producer_configured(root);
    match producer_configured.as_deref() {
        Some("perl-ripr-facts") => {
            println!("  producer: configured as `perl-ripr-facts` (canonical)")
        }
        Some("perllsp") => println!("  producer: configured as `perllsp` (compatibility wrapper)"),
        Some("perl-lsp") => {
            println!("  producer: configured as `perl-lsp` (compatibility wrapper)")
        }
        Some(other) => println!("  producer: configured as `{other}`"),
        None => println!("  producer: not configured (managed mode off)"),
    }

    // Find the producer binary and its version. Try canonical first, then wrappers.
    let (found_bin, version) = producer_binary_and_version(root);
    match (found_bin.as_deref(), version.as_deref()) {
        (Some(bin), Some(ver)) => {
            println!("  exporter: found at {bin} (version {ver})");
            // If only a wrapper was found (not the canonical exporter), explain.
            if bin.contains("perllsp") || bin.contains("perl-lsp") {
                if which("perl-ripr-facts") {
                    // Canonical also present — no warning needed.
                } else {
                    println!(
                        "  note: `{bin}` must delegate to the batch perl-ripr-facts exporter; RIPR does not use LSP protocol"
                    );
                }
            }
        }
        (Some(bin), None) => println!("  exporter: found at {bin} (version unknown)"),
        _ => println!(
            "  exporter: NOT found on PATH (expected: perl-ripr-facts, perllsp, or perl-lsp)"
        ),
    }

    // schema compatible? (always reports the schema this ripr build consumes.)
    println!("  schema: {} expected", crate::app::PERL_FACT_PACKET_SCHEMA);

    // t/ and t2/ roots detected?
    let roots = detect_perl_test_roots(root);
    println!("  test roots: {roots}");

    // Detected test frameworks.
    let frameworks = detect_perl_frameworks(root);
    println!("  frameworks: {frameworks}");

    // Runner availability: prove/yath/carton/dzil.
    let mut runners: Vec<&str> = Vec::new();
    if which("prove") {
        runners.push("prove");
    }
    if which("yath") {
        runners.push("yath");
    }
    if which("carton") {
        runners.push("carton");
    }
    if which("dzil") {
        runners.push("dzil");
    }
    let runners_str = if runners.is_empty() {
        "none found on PATH".to_string()
    } else {
        runners.join(", ")
    };
    println!("  runners: {runners_str}");

    // Exact next command: branch on whether managed mode is configured and
    // whether the producer is present.
    let next = perl_next_command(producer_configured.as_deref(), found_bin.as_deref());
    println!("  next: {next}");
}

/// Whether `[perl].producer` is configured in the root's ripr config. Returns
/// the configured producer name, or None if not set / config unreadable.
fn perl_producer_configured(root: &Path) -> Option<String> {
    let config = crate::config::load_for_root(root).ok()?;
    config.perl().producer().map(|s| s.to_string())
}

/// Resolve the producer binary path and version. Honors `[perl].executable`
/// when set; otherwise probes PATH for `perl-ripr-facts` (canonical, post
/// perl-lsp-swarm #3294), then `perllsp`/`perl-lsp` (compatibility wrappers).
/// Returns (resolved_path, version_string) where version comes from
/// `--version` stdout.
fn producer_binary_and_version(root: &Path) -> (Option<String>, Option<String>) {
    // Honor explicit [perl].executable first.
    let explicit = crate::config::load_for_root(root)
        .ok()
        .and_then(|c| c.perl().executable().map(|p| p.display().to_string()));
    let candidates: Vec<String> = match explicit {
        Some(path) => vec![path],
        None => vec![
            "perl-ripr-facts".to_string(),
            "perllsp".to_string(),
            "perl-lsp".to_string(),
        ],
    };
    for candidate in &candidates {
        let probe = std::process::Command::new(candidate)
            .arg("--version")
            .output();
        if let Ok(output) = probe
            && output.status.success()
        {
            let version = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            let resolved = which(candidate)
                .then(|| resolve_binary_path(candidate))
                .flatten();
            return (resolved.or_else(|| Some(candidate.clone())), Some(version));
        }
    }
    (None, None)
}

/// Best-effort resolution of a PATH binary to an absolute path for display.
/// Falls back to the name itself if resolution is unavailable.
fn resolve_binary_path(bin: &str) -> Option<String> {
    // `which`/`where` already proved existence; re-run capturing stdout.
    let lookup = if cfg!(unix) { "which" } else { "where" };
    std::process::Command::new(lookup)
        .arg(bin)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .map(|s| s.trim().to_string())
        })
}

/// Detect CPAN-style project markers beyond .pm/.pl/.t files: Makefile.PL,
/// Build.PL, cpanfile. These confirm a real CPAN-style project a producer can
/// index.
fn has_perl_project_markers(root: &Path) -> bool {
    ["Makefile.PL", "Build.PL", "cpanfile"]
        .iter()
        .any(|marker| root.join(marker).is_file())
}

/// Detect Perl test directories: `t/` and `t2/`. Returns a human-readable
/// summary.
fn detect_perl_test_roots(root: &Path) -> String {
    let has_t = root.join("t").is_dir();
    let has_t2 = root.join("t2").is_dir();
    match (has_t, has_t2) {
        (true, true) => "t/ and t2/ detected".to_string(),
        (true, false) => "t/ detected".to_string(),
        (false, true) => "t2/ detected".to_string(),
        (false, false) => "none detected".to_string(),
    }
}

/// Detect Perl test frameworks from .t files in t/ and t2/. Returns a
/// comma-separated list of detected frameworks (Test::More, Test2::V0/V1/Suite,
/// Test::Exception, Test::Fatal), or "none detected".
fn detect_perl_frameworks(root: &Path) -> String {
    let mut found: Vec<&str> = Vec::new();
    let mut contents: Vec<String> = Vec::new();
    for dir in ["t", "t2"] {
        let test_dir = root.join(dir);
        let Ok(entries) = std::fs::read_dir(&test_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "t")
                && let Ok(content) = std::fs::read_to_string(&path)
            {
                contents.push(content);
            }
        }
    }
    let blob = contents.join("\n");
    if blob.contains("use Test2::V1") || blob.contains("use Test2::Bundle::More") {
        found.push("Test2::V1");
    }
    if blob.contains("use Test2::V0") || blob.contains("use Test2::Tools::Basic") {
        found.push("Test2::V0");
    }
    if blob.contains("use Test2::Suite") {
        found.push("Test2::Suite");
    }
    if blob.contains("use Test::More") {
        found.push("Test::More");
    }
    if blob.contains("use Test::Exception") {
        found.push("Test::Exception");
    }
    if blob.contains("use Test::Fatal") {
        found.push("Test::Fatal");
    }
    if found.is_empty() {
        "none detected".to_string()
    } else {
        found.join(", ")
    }
}

/// Choose the exact next command based on producer configuration + presence.
fn perl_next_command(producer_configured: Option<&str>, found_bin: Option<&str>) -> String {
    let managed = matches!(
        producer_configured,
        Some("perl-ripr-facts") | Some("perllsp") | Some("perl-lsp")
    );
    if managed && found_bin.is_some() {
        // Managed mode + producer present: ripr invokes the exporter
        // itself. There is no --languages flag (#2105): perl is enabled
        // through config, and check then runs the enabled set.
        // Name the additive edit, not a replacement list, so a user with
        // TypeScript/Python already enabled keeps them (#2105 review).
        "add \"perl\" to [languages] enabled in ripr.toml, then: ripr check --base origin/main --head HEAD".to_string()
    } else if managed {
        // Managed mode configured but producer missing.
        "install perllsp on PATH (or set [perl].executable) and add \"perl\" to [languages] enabled in ripr.toml, then: ripr check --base origin/main --head HEAD".to_string()
    } else {
        // Explicit packet mode (or producer absent): supply --perl-facts.
        "ripr check --perl-facts <packet.json> --diff <diff.patch> --json".to_string()
    }
}

/// Print static limitation notes for the doctor first-run diagnosis.
///
/// Every statement is conservative: no claim is made beyond what the static
/// analysis layer can actually determine. Wording sources:
///   - `language.rs` doc comment: TypeScript/JavaScript/Python/Perl are
///     preview surfaces.
///   - `StaticLimitKind::CrossLanguageOracleVisibilityUnresolved` wire string
///     and its doc comment.
///   - 0.9.0 CHANGELOG non-claims.
fn report_known_limitations() {
    println!("- Known limitations:");
    println!(
        "  TypeScript/JavaScript/Bun analysis is preview (advisory only); \
        not stable support — findings are additive, not gating"
    );
    println!(
        "  Cross-language oracle visibility is fail-closed: an FFI/binding seam tested \
        from another language reads as cross_language_oracle_visibility_unresolved, \
        not a Rust gap — verify the external oracle directly"
    );
    println!(
        "  Full-repo repo-exposure analysis applies a default cap of {} seams; \
        set RIPR_REPO_EXPOSURE_SEAM_LIMIT=0 to analyze all seams.",
        analysis::DEFAULT_REPO_EXPOSURE_SEAM_LIMIT
    );
    println!(
        "  Preview-language evidence does not emit public repair packets and \
        does not block by default"
    );
}

fn report_cache_status(root: &Path) {
    let cache_dir = analysis::seam_cache::cache_base_dir(root);
    let relocated =
        std::env::var(analysis::seam_cache::CACHE_DIR_ENV).is_ok_and(|v| !v.trim().is_empty());
    let size_bytes = dir_size_bytes(&cache_dir);
    let size_display = format_bytes(size_bytes);
    if relocated {
        println!(
            "- Cache location: {} (RIPR_CACHE_DIR active)",
            cache_dir.display()
        );
    } else {
        println!("- Cache location: {}", cache_dir.display());
    }
    println!("- Cache size: {size_display} (run `ripr cache status` for details)");
}

/// Recursively sum file sizes under `dir`. Returns 0 when the directory
/// does not exist or cannot be read — cache absence is not a problem.
fn dir_size_bytes(dir: &Path) -> u64 {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };
    let mut total: u64 = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            total = total.saturating_add(dir_size_bytes(&path));
        } else if let Ok(meta) = std::fs::metadata(&path) {
            total = total.saturating_add(meta.len());
        }
    }
    total
}

/// Format a byte count in human-readable form (B, KB, MB, GB).
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = 1_024 * KB;
    const GB: u64 = 1_024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn report_config_status(root: &Path, config: Result<RiprConfig, String>, ok: &mut bool) {
    match config {
        Ok(config) => {
            match config.source_path() {
                Some(path) => {
                    println!("✓ Config: loaded {CONFIG_FILE_NAME}");
                    println!("- Config path: {}", path.display());
                }
                None => println!("✓ Config: not found; using built-in defaults"),
            }
            let analysis_mode = config
                .analysis()
                .mode()
                .map(Mode::as_str)
                .unwrap_or_else(|| Mode::Draft.as_str());
            println!("- Analysis mode default: {analysis_mode}");
            println!(
                "- LSP seam diagnostics default: {}",
                config
                    .lsp()
                    .seam_diagnostics()
                    .unwrap_or(DEFAULT_LSP_SEAM_DIAGNOSTICS)
            );
            println!(
                "- Suppressions path: {}",
                config.suppressions().display_path()
            );
            let languages = config
                .languages()
                .enabled()
                .iter()
                .map(|language| language.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            println!("- Enabled languages: {languages}");
            if let Some(profile) = config.profiles().bun_ub() {
                println!("- Bun UB profile: configured (preview advisory only)");
                println!("- Bun UB test roots: {}", profile.test_roots().join(", "));
                println!("- Bun UB bridge hints: {}", profile.display_bridge_hints());
                println!(
                    "- Bun UB authority: no runtime Bun, tsc, tsserver, generated tests, gates, badges, baselines, or support-tier promotion"
                );
            } else {
                println!("- Bun UB profile: not configured");
            }
        }
        Err(err) => {
            println!("! Config: invalid {CONFIG_FILE_NAME}");
            println!("- Config path: {}", root.join(CONFIG_FILE_NAME).display());
            println!("  error: {err}");
            *ok = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{args, unique_command_test_dir};
    use super::*;

    #[test]
    #[cfg(all(feature = "lang-python", feature = "lang-typescript"))]
    fn language_runtime_probes_follow_detected_languages() -> Result<(), String> {
        // #2071: rust-only roots get no probes; a python root with pytest
        // markers gets python3 + pytest; a bun workspace adds bun.
        let root = unique_command_test_dir("probe-rust");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        assert!(language_runtime_probes(&root).is_empty());
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;

        let root = unique_command_test_dir("probe-python");
        let tests_dir = root.join("tests");
        std::fs::create_dir_all(&tests_dir).map_err(|err| format!("create tests dir: {err}"))?;
        std::fs::write(tests_dir.join("test_x.py"), "import unittest\n")
            .map_err(|err| format!("write test: {err}"))?;
        std::fs::write(root.join("conftest.py"), "")
            .map_err(|err| format!("write conftest: {err}"))?;
        let tools: Vec<&str> = language_runtime_probes(&root)
            .iter()
            .map(|(_, tool, _)| *tool)
            .collect();
        assert_eq!(tools, vec!["python3", "pytest"]);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;

        let root = unique_command_test_dir("probe-bun");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        std::fs::write(root.join("package.json"), "{}")
            .map_err(|err| format!("write pkg: {err}"))?;
        std::fs::write(root.join("bun.lockb"), "").map_err(|err| format!("write lock: {err}"))?;
        let tools: Vec<&str> = language_runtime_probes(&root)
            .iter()
            .map(|(_, tool, _)| *tool)
            .collect();
        assert_eq!(tools, vec!["node", "bun"]);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;

        // #2183 review: a bare pyproject.toml is PEP 517 packaging, not
        // pytest evidence — only python3 is probed.
        let root = unique_command_test_dir("probe-pyproject-only");
        std::fs::create_dir_all(root.join("tests"))
            .map_err(|err| format!("create tests: {err}"))?;
        std::fs::write(root.join("pyproject.toml"), "[project]\nname = \"x\"\n")
            .map_err(|err| format!("write pyproject: {err}"))?;
        std::fs::write(root.join("tests/test_x.py"), "import unittest\n")
            .map_err(|err| format!("write test: {err}"))?;
        let tools: Vec<&str> = language_runtime_probes(&root)
            .iter()
            .map(|(_, tool, _)| *tool)
            .collect();
        assert_eq!(tools, vec!["python3"]);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;

        // A JS-only workspace is labeled javascript, not typescript (#2183
        // review).
        let root = unique_command_test_dir("probe-js-only");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        std::fs::write(root.join("index.js"), "console.log(1);\n")
            .map_err(|err| format!("write js: {err}"))?;
        let labels: Vec<&str> = language_runtime_probes(&root)
            .iter()
            .map(|(language, _, _)| *language)
            .collect();
        assert_eq!(labels, vec!["javascript"]);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;

        // pnpm and yarn lockfiles add their runners.
        for (lockfile, tool) in [("pnpm-lock.yaml", "pnpm"), ("yarn.lock", "yarn")] {
            let root = unique_command_test_dir("probe-pm");
            std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
            std::fs::write(root.join("package.json"), "{}")
                .map_err(|err| format!("write pkg: {err}"))?;
            std::fs::write(root.join(lockfile), "").map_err(|err| format!("write lock: {err}"))?;
            let tools: Vec<&str> = language_runtime_probes(&root)
                .iter()
                .map(|(_, candidate, _)| *candidate)
                .collect();
            assert_eq!(tools, vec!["node", tool], "lockfile {lockfile}");
            std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        }
        Ok(())
    }

    #[test]
    fn enabled_language_keeps_primary_runtime_required_when_other_tool_is_detected() {
        let mut probes = vec![("typescript", "yarn", "install Yarn")];
        append_missing_primary_runtime_probes(&mut probes, &[LanguageId::TypeScript]);
        assert_eq!(
            probes,
            vec![
                ("typescript", "yarn", "install Yarn"),
                ("typescript", "node", "install Node.js"),
            ]
        );
        assert!(runtime_probe_is_required(
            "typescript",
            "node",
            &[LanguageId::TypeScript]
        ));
        assert!(!runtime_probe_is_required(
            "typescript",
            "yarn",
            &[LanguageId::TypeScript]
        ));
    }

    #[test]
    fn language_runtime_probe_line_names_labels_evidence_and_hint() {
        // #2183 review: the emitted contract is pinned, not just the list.
        let pass = language_runtime_probe_line(
            "python",
            output::doctor::DoctorStatus::Pass,
            "Python 3.12.3",
            "install python3",
        );
        assert!(pass.starts_with('✓'));
        assert!(pass.contains("python verify-route runtime: Python 3.12.3"));
        assert!(!pass.contains("install python3"));
        let fail = language_runtime_probe_line(
            "python",
            output::doctor::DoctorStatus::Fail,
            "python3 not available",
            "install python3 (e.g. apt install python3)",
        );
        assert!(fail.starts_with('!'));
        assert!(
            fail.contains("python3 not available — install python3 (e.g. apt install python3)")
        );
    }

    #[test]
    fn configured_primary_runtime_failure_fails_report_and_optional_preview_does_not()
    -> Result<(), String> {
        let root = unique_command_test_dir("runtime-probe-requirement");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        std::fs::write(root.join("package.json"), "{}")
            .map_err(|err| format!("write package marker: {err}"))?;

        let mut required_report = output::doctor::DoctorReport::new(&root.display().to_string());
        let required_ok = add_language_runtime_probes(
            &root,
            &[LanguageId::TypeScript],
            &mut required_report,
            false,
            |tool, _isolated| {
                if tool == "node" {
                    (
                        output::doctor::DoctorStatus::Fail,
                        "node not available".to_string(),
                    )
                } else {
                    (
                        output::doctor::DoctorStatus::Pass,
                        format!("{tool} available"),
                    )
                }
            },
        );
        assert!(!required_ok);
        assert_eq!(required_report.status, output::doctor::DoctorStatus::Fail);
        if output::doctor::doctor_report_result(&required_report).is_ok() {
            return Err("required runtime failure unexpectedly passed doctor".to_string());
        }
        let node_probe = required_report
            .runtime_probes
            .iter()
            .find(|probe| probe.tool == "node")
            .ok_or_else(|| "missing node runtime probe".to_string())?;
        assert!(node_probe.required);
        assert_eq!(node_probe.status, output::doctor::DoctorStatus::Fail);

        let mut optional_report = output::doctor::DoctorReport::new(&root.display().to_string());
        let optional_ok = add_language_runtime_probes(
            &root,
            &[LanguageId::Rust],
            &mut optional_report,
            false,
            |tool, _isolated| {
                if tool == "node" {
                    (
                        output::doctor::DoctorStatus::Fail,
                        "node not available".to_string(),
                    )
                } else {
                    (
                        output::doctor::DoctorStatus::Pass,
                        format!("{tool} available"),
                    )
                }
            },
        );
        assert!(optional_ok);
        assert_eq!(optional_report.status, output::doctor::DoctorStatus::Pass);
        output::doctor::doctor_report_result(&optional_report)
            .map_err(|error| format!("optional runtime failure should remain advisory: {error}"))?;
        let node_probe = optional_report
            .runtime_probes
            .iter()
            .find(|probe| probe.tool == "node")
            .ok_or_else(|| "missing optional node runtime probe".to_string())?;
        assert!(!node_probe.required);
        assert_eq!(node_probe.status, output::doctor::DoctorStatus::Fail);

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;

        let configured_only_root = unique_command_test_dir("runtime-probe-configured-only");
        std::fs::create_dir_all(&configured_only_root)
            .map_err(|err| format!("create configured-only root: {err}"))?;
        let mut configured_only_report =
            output::doctor::DoctorReport::new(&configured_only_root.display().to_string());
        let configured_only_ok = add_language_runtime_probes(
            &configured_only_root,
            &[LanguageId::Python],
            &mut configured_only_report,
            false,
            |tool, _isolated| {
                (
                    output::doctor::DoctorStatus::Fail,
                    format!("{tool} not available"),
                )
            },
        );
        assert!(!configured_only_ok);
        assert!(
            configured_only_report
                .runtime_probes
                .iter()
                .any(|probe| probe.language == "python" && probe.tool == "python3")
        );
        std::fs::remove_dir_all(&configured_only_root)
            .map_err(|err| format!("remove configured-only root: {err}"))?;
        Ok(())
    }

    #[test]
    #[cfg(all(feature = "lang-python", feature = "lang-typescript"))]
    fn doctor_reports_unittest_and_package_only_ts_frameworks() -> Result<(), String> {
        // #2106 review: doctor output coverage for frameworks only visible
        // through the shared detectors.
        let root = unique_command_test_dir("doctor-unittest");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        std::fs::write(root.join("test_pricing.py"), "import unittest\n")
            .map_err(|err| format!("write test file: {err}"))?;
        let lines = detected_test_surface_lines(&root);
        assert!(
            lines.iter().any(|line| line == "python: unittest"),
            "expected python: unittest in {lines:?}"
        );
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;

        let root = unique_command_test_dir("doctor-ava");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        std::fs::write(
            root.join("package.json"),
            r#"{"name":"ky","scripts":{"test":"xo && npm run build && ava"}}"#,
        )
        .map_err(|err| format!("write package.json: {err}"))?;
        let lines = detected_test_surface_lines(&root);
        assert!(
            lines.iter().any(|line| line == "typescript: ava"),
            "expected typescript: ava in {lines:?}"
        );
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn doctor_requires_root_value() {
        assert_eq!(
            doctor(&args(&["--root"])),
            Err("missing value for --root".to_string())
        );
    }

    #[test]
    fn perl_next_command_never_recommends_a_flag_check_rejects() {
        // #2105: `ripr check` has no --languages flag; every doctor
        // recommendation must stay within the check parser's contract.
        for (producer, found) in [
            (Some("perllsp"), Some("perllsp")),
            (Some("perllsp"), None),
            (Some("perl-ripr-facts"), None),
            (None, None),
        ] {
            let command = perl_next_command(producer, found);
            assert!(
                !command.contains("--languages"),
                "recommendation must not name --languages: {command}"
            );
        }
        // The managed-present branch points at the config-driven route.
        let managed = perl_next_command(Some("perllsp"), Some("perllsp"));
        assert!(managed.contains("[languages]"));
        assert!(managed.contains("ripr check --base origin/main --head HEAD"));
        // The packet-mode branch is unchanged.
        let packet = perl_next_command(None, None);
        assert!(packet.contains("--perl-facts"));
    }

    #[test]
    fn doctor_rejects_unknown_arguments() {
        assert_eq!(
            doctor(&args(&["--bogus"])),
            Err("unknown doctor argument \"--bogus\". Run `ripr doctor --help`.".to_string())
        );
    }

    #[test]
    fn doctor_accepts_default_root() {
        assert_eq!(doctor(&args(&[])), Ok(()));
    }

    #[test]
    fn doctor_core_report_fails_closed_for_invalid_config() -> Result<(), String> {
        let dir = unique_command_test_dir("doctor-invalid-config");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        std::fs::write(dir.join(CONFIG_FILE_NAME), "[invalid\n")
            .map_err(|err| format!("write invalid config: {err}"))?;

        let report = output::doctor::evaluate_doctor_core(&dir);
        if report.status != output::doctor::DoctorStatus::Fail {
            return Err(format!(
                "invalid config should fail, got {:?}",
                report.status
            ));
        }
        let config_check = report
            .checks
            .iter()
            .find(|check| check.name == "config")
            .ok_or_else(|| "missing config check".to_string())?;
        if config_check.status != output::doctor::DoctorStatus::Fail {
            return Err(format!(
                "invalid config check should fail, got {:?}",
                config_check.status
            ));
        }
        if !config_check
            .evidence
            .as_deref()
            .is_some_and(|evidence| evidence.contains("invalid ripr.toml"))
        {
            return Err(format!(
                "invalid config evidence was not actionable: {:?}",
                config_check.evidence
            ));
        }

        let json = report.render_json()?;
        let value: serde_json::Value =
            serde_json::from_str(&json).map_err(|err| format!("parse report JSON: {err}"))?;
        if value["status"] != "fail"
            || value["checks"][2]["name"] != "config"
            || value["checks"][2]["status"] != "fail"
        {
            return Err(format!("unexpected invalid-config JSON report: {value}"));
        }

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn doctor_core_report_fails_closed_for_missing_root() -> Result<(), String> {
        let root = unique_command_test_dir("doctor-missing-root");
        if root.exists() {
            return Err(format!("test root unexpectedly exists: {}", root.display()));
        }

        let report = output::doctor::evaluate_doctor_core(&root);
        if report.status != output::doctor::DoctorStatus::Fail {
            return Err(format!("missing root should fail, got {:?}", report.status));
        }
        let root_check = report
            .checks
            .iter()
            .find(|check| check.name == "root_directory")
            .ok_or_else(|| "missing root-directory check".to_string())?;
        if root_check.status != output::doctor::DoctorStatus::Fail {
            return Err(format!(
                "missing root check should fail, got {:?}",
                root_check.status
            ));
        }
        if !root_check
            .evidence
            .as_deref()
            .is_some_and(|evidence| evidence.contains("does not exist"))
        {
            return Err(format!(
                "missing root evidence was not actionable: {:?}",
                root_check.evidence
            ));
        }
        Ok(())
    }

    // Deterministic missing-tool and empty-report-passes assertions live with
    // the moved model in `output::doctor::tests` now
    // (`doctor_tool_check_fails_closed_for_guaranteed_missing_tool`,
    // `empty_report_is_pass`). This test keeps the integration-level proof
    // that the `--json` doctor path fails closed for a malformed config.
    #[test]
    fn doctor_json_and_tool_failures_return_errors() -> Result<(), String> {
        let dir = unique_command_test_dir("doctor-json-invalid-config");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        std::fs::write(dir.join(CONFIG_FILE_NAME), "[invalid\n")
            .map_err(|err| format!("write invalid config: {err}"))?;
        if doctor_json(&dir).is_ok() {
            let _ = std::fs::remove_dir_all(&dir);
            return Err("invalid JSON doctor report unexpectedly passed".to_string());
        }
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn doctor_human_projection_fails_for_missing_root() -> Result<(), String> {
        let root = unique_command_test_dir("doctor-human-missing-root");
        if root.exists() {
            return Err(format!("test root unexpectedly exists: {}", root.display()));
        }
        let root_arg = root.to_string_lossy().into_owned();
        if doctor(&args(&["--root", &root_arg])).is_ok() {
            return Err("human doctor unexpectedly passed for missing root".to_string());
        }
        Ok(())
    }

    #[test]
    fn doctor_json_flag_accepts_explicit_root() -> Result<(), String> {
        doctor(&args(&["--json", "--root", "."]))
    }

    // --- preview_language_enable_suggestions tests ---

    /// When TypeScript files are detected in a directory that has no ripr.toml
    /// (so the config defaults to `["rust"]`) AND the `lang-typescript` feature
    /// was compiled in, we expect a suggestion line containing the copy-paste
    /// TOML block.
    #[cfg(feature = "lang-typescript")]
    #[test]
    fn doctor_suggests_typescript_when_detected_and_not_enabled() -> Result<(), String> {
        let dir = unique_command_test_dir("suggest-ts-detected");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create dir: {err}"))?;
        // Drop a .ts file so TypeScript is detected.
        std::fs::write(dir.join("index.ts"), "export const x = 1;\n")
            .map_err(|err| format!("write ts: {err}"))?;
        // Keep the default-disabled contract explicit rather than relying on
        // ancestor lookup, since the source repository may itself configure a
        // preview adapter for its hosted evidence workflow.
        std::fs::write(
            dir.join(CONFIG_FILE_NAME),
            "[languages]\nenabled = [\"rust\"]\n",
        )
        .map_err(|err| format!("write default config: {err}"))?;
        let suggestions = preview_language_enable_suggestions(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            !suggestions.is_empty(),
            "expected a suggestion when TS detected and not enabled"
        );
        let joined = suggestions.join("\n");
        assert!(
            joined.contains("typescript"),
            "suggestion must name the language; got:\n{joined}"
        );
        assert!(
            joined.contains(r#"enabled = ["rust", "typescript"]"#),
            "suggestion must contain copy-paste TOML block; got:\n{joined}"
        );
        Ok(())
    }

    /// When TypeScript is explicitly listed in ripr.toml `enabled`, no
    /// suggestion should appear even if .ts files are present.
    #[cfg(feature = "lang-typescript")]
    #[test]
    fn doctor_no_suggestion_when_typescript_already_enabled() -> Result<(), String> {
        let dir = unique_command_test_dir("suggest-ts-already-enabled");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create dir: {err}"))?;
        std::fs::write(dir.join("index.ts"), "export const x = 1;\n")
            .map_err(|err| format!("write ts: {err}"))?;
        // ripr.toml explicitly enables typescript.
        std::fs::write(
            dir.join("ripr.toml"),
            "[languages]\nenabled = [\"rust\", \"typescript\"]\n",
        )
        .map_err(|err| format!("write ripr.toml: {err}"))?;
        let suggestions = preview_language_enable_suggestions(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            suggestions.is_empty(),
            "expected no suggestions when typescript already enabled; got: {suggestions:?}"
        );
        Ok(())
    }

    /// When no preview-language files are detected (only Rust), the suggestion
    /// list must be empty regardless of config.
    #[test]
    fn doctor_no_suggestion_when_no_preview_language_detected() -> Result<(), String> {
        let dir = unique_command_test_dir("suggest-no-preview");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create dir: {err}"))?;
        // Only a Cargo.toml → Rust only, no preview language detected.
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .map_err(|err| format!("write Cargo.toml: {err}"))?;
        let suggestions = preview_language_enable_suggestions(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            suggestions.is_empty(),
            "expected no suggestions for Rust-only dir; got: {suggestions:?}"
        );
        Ok(())
    }

    /// When the binary was built WITHOUT the `lang-typescript` feature (adapter
    /// not compiled in), the suggestion must be suppressed even if .ts files are
    /// present. The user cannot enable an adapter that isn't in the binary.
    #[cfg(not(feature = "lang-typescript"))]
    #[test]
    fn doctor_no_suggestion_when_typescript_adapter_not_compiled() -> Result<(), String> {
        let dir = unique_command_test_dir("suggest-ts-not-compiled");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create dir: {err}"))?;
        std::fs::write(dir.join("index.ts"), "export const x = 1;\n")
            .map_err(|err| format!("write ts: {err}"))?;
        // No ripr.toml → defaults to enabled = ["rust"] only.
        let suggestions = preview_language_enable_suggestions(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            suggestions.is_empty(),
            "expected no suggestions when lang-typescript feature is not compiled; got: {suggestions:?}"
        );
        Ok(())
    }
}
