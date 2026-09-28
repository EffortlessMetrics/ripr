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
    let mut profile = output::doctor::DoctorProfile::Analysis;
    let mut root_args: Vec<&str> = Vec::new();
    let mut arguments = args.iter();
    while let Some(arg) = arguments.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                help::print_doctor_help();
                return Ok(());
            }
            "--json" => json_output = true,
            "--profile" => {
                profile = match arguments.next().map(String::as_str) {
                    Some("analysis") => output::doctor::DoctorProfile::Analysis,
                    Some("source-build") => output::doctor::DoctorProfile::SourceBuild,
                    Some(other) => {
                        return Err(format!(
                            "unknown doctor profile `{other}`; expected analysis or source-build"
                        ));
                    }
                    None => return Err("missing value for --profile".to_string()),
                };
            }
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
        return doctor_json(&root, profile);
    }

    // Human-readable path.
    let core_evaluation = output::doctor::evaluate_doctor_core_with_config_for_profile(
        &root,
        &detect_languages(&root),
        profile,
    );
    let mut report = core_evaluation.report;
    let core_report = &report;
    let mut ok = matches!(core_report.status, output::doctor::DoctorStatus::Pass);
    let enabled_languages = enabled_languages(&core_evaluation.config);
    println!("ripr doctor");
    println!("- root: {}", output::path::human_path(&root));
    for line in output::doctor_binary::probe_binary_identity().human_lines() {
        println!("{line}");
    }
    println!(
        "- RIPR {} (source build requires Rust {})",
        report.ripr_version, report.ripr_build_msrv
    );
    let profile_name = match profile {
        output::doctor::DoctorProfile::Analysis => "analysis",
        output::doctor::DoctorProfile::SourceBuild => "source-build",
    };
    println!("- profile: {profile_name}");

    ok &= report_doctor_core_check(core_report, "root_directory");
    ok &= report_doctor_core_check(core_report, "cargo_toml");
    ok &= report_doctor_core_check(core_report, "git_repository");
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
        print!("{}", output::doctor::DOCTOR_FAILED_LINE);
        Err("doctor found issues".to_string())
    }
}

/// Typed JSON doctor output. Captures top-level checks and language runtime
/// probes as structured values. Deeper sub-checks (cache, Perl, and test
/// surfaces) remain on the human-oriented path for a follow-up PR to type
/// individually. See #1771 / #1614.
fn doctor_json(root: &Path, profile: output::doctor::DoctorProfile) -> Result<(), String> {
    let evaluation = output::doctor::evaluate_doctor_core_with_config_for_profile(
        root,
        &detect_languages(root),
        profile,
    );
    let mut report = evaluation.report;
    report.binary = Some(output::doctor_binary::probe_binary_identity());
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
    // A skipped check (not applicable to this root) prints as an
    // informational `-` line carrying its reason and never fails doctor.
    let marker = match check.status {
        output::doctor::DoctorCheckStatus::Pass => "✓",
        output::doctor::DoctorCheckStatus::Fail => "!",
        output::doctor::DoctorCheckStatus::Advisory => "~",
        output::doctor::DoctorCheckStatus::Skipped => "-",
    };
    println!(
        "{marker} {}",
        check.evidence.as_deref().unwrap_or(check.name.as_str())
    );
    check.status != output::doctor::DoctorCheckStatus::Fail
}

fn print_doctor_start_here_guidance(root: &Path) {
    // First-run honesty: name the packet only as present when it exists.
    // An unconditional path reads as an existing artifact on a fresh
    // workspace where `ripr first-pr` has never run (RIPR-SPEC-0051 names
    // the path, not its existence). `is_file` (not `exists`) so a directory
    // squatting the packet path cannot read as openable evidence.
    // The safe next action follows the packet's existence, because `first-pr`
    // composes the packet out of artifacts `ripr check` produces -- it runs no
    // analysis of its own (the boundary `help --all` states). Recommending it
    // on a fresh workspace dead-ends: measured, it returns `missing_artifacts`
    // and answers with `Regeneration command: ripr check ...`, which is the
    // command this same screen already prints three lines below. Two different
    // first commands on one screen, one of which bounces straight back to the
    // other, is not a route.
    if root.join("target/ripr/reports/start-here.md").is_file() {
        println!("- Start-here packet: target/ripr/reports/start-here.md (present; open it first)");
        println!(
            "- Safe next action: open that packet; `ripr first-pr --root {} --base <ref> --head HEAD` refreshes it",
            root.display()
        );
    } else {
        println!(
            "- Start-here packet: target/ripr/reports/start-here.md (not yet generated; `ripr first-pr` composes it once analysis evidence exists)"
        );
        println!(
            "- Safe next action: run the recommended first command below; it produces the evidence the packet is composed from"
        );
    }
    println!(
        "- Recovery states: missing artifact, stale evidence, wrong root, malformed artifact, no actionable gap, preview-limited evidence"
    );
    println!(
        "- Proof rail: verify command, receipt command, and receipt path are advisory static movement evidence"
    );
    // First-run honesty: when the working tree has uncommitted changes,
    // a committed-history `ripr check` analyzes committed history only and would
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
        // No `--base origin/main`: this screen is read in whatever repository
        // the user has, and that ref does not exist in one whose default
        // branch is not `main`. Without a base, the loader resolves the
        // repository's own default (`analysis::diff::load::resolve_default_base`).
        println!("- Recommended first command: ripr check");
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

    // TypeScript: package.json, tsconfig.json, or a TypeScript-source
    // extension (.ts/.tsx/.mts/.cts) at a shallow depth. #4116: both
    // extension families consume the shared TS/JS extension authority.
    if shallow_has_file(root, "package.json")
        || shallow_has_file(root, "tsconfig.json")
        || analysis::TYPESCRIPT_SOURCE_EXTENSIONS
            .iter()
            .any(|extension| shallow_has_extension(root, extension))
    {
        found.push(LanguageId::TypeScript);
    }

    // JavaScript: JavaScript-source extensions (.js/.jsx/.mjs/.cjs), only
    // when no TypeScript markers were found.
    if !found.contains(&LanguageId::TypeScript)
        && analysis::JAVASCRIPT_SOURCE_EXTENSIONS
            .iter()
            .any(|extension| shallow_has_extension(root, extension))
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

    // Perl: the same predicate that gates the Perl preview section, so a
    // CPAN layout with modules below `lib/Name/` is detected and a `.t` under
    // `target/` is not.
    if perl_project_detected(root) {
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
        // A language runtime is an analysis capability, not a prerequisite
        // for building RIPR from source: the source-build profile keeps the
        // probe visible but never lets it decide that profile's status.
        let required = report.profile == output::doctor::DoctorProfile::Analysis
            && runtime_probe_is_required(language, tool, enabled);
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

/// The primary runtime probe per language. Python's command name and install
/// hint depend on the host ([`python_runtime_for_os`]).
fn primary_runtime_probes() -> [(&'static str, &'static str, &'static str); 3] {
    let (python_tool, python_hint) = python_runtime_for_os(std::env::consts::OS);
    [
        ("typescript", "node", "install Node.js"),
        ("javascript", "node", "install Node.js"),
        ("python", python_tool, python_hint),
    ]
}

/// The Python command doctor probes and the install hint it prints, for a
/// host OS name as `std::env::consts::OS` spells it (#4378). Pure so every
/// host's answer is testable from any host.
///
/// The python.org and winget installers put `python` (not `python3`) on a
/// Windows PATH, and `apt` does not exist there, so Windows probes `python`
/// with a Windows install route. RIPR never runs the interpreter itself; the
/// probe only tells a user whether their Python verify route can start.
fn python_runtime_for_os(os: &str) -> (&'static str, &'static str) {
    match os {
        "windows" => (
            "python",
            "install Python and put `python` on PATH (e.g. winget install Python.Python.3.13)",
        ),
        "macos" => ("python3", "install python3 (e.g. brew install python)"),
        _ => ("python3", "install python3 (e.g. apt install python3)"),
    }
}

fn primary_runtime(language: &str) -> Option<(&'static str, &'static str)> {
    primary_runtime_probes()
        .into_iter()
        .find(|(candidate, _, _)| *candidate == language)
        .map(|(_, tool, hint)| (tool, hint))
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
    for (language, tool, hint) in primary_runtime_probes() {
        let enabled_language = enabled.iter().any(|id| id.as_str() == language);
        if enabled_language
            && !probes
                .iter()
                .any(|(found, detected_tool, _)| *found == language && *detected_tool == tool)
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
        let (python_tool, python_hint) = python_runtime_for_os(std::env::consts::OS);
        probes.push(("python", python_tool, python_hint));
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
                        lines.push(format!(
                            "perl: adapter NOT compiled; {}",
                            id.unavailable_adapter_recovery()
                        ));
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
/// Hidden and build/dependency directories that the Perl file walks skip,
/// so vendored or generated files neither inflate counts nor detect Perl.
fn is_skipped_walk_dir(path: &Path) -> bool {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    name.starts_with('.') || matches!(name, "target" | "node_modules" | "blib")
}

/// True for a real directory entry. `DirEntry::file_type` does not follow
/// symlinks, so a `src/loop -> .` link cannot send doctor's walks into an
/// unbounded descent (the other workspace walkers already skip links). An
/// entry whose type cannot be read is not descended. Non-directory entries
/// are counted only when `Path::is_file` holds, so a directory link named
/// `x.pm` is neither descended nor counted as a Perl file.
fn is_walkable_dir(entry: &std::fs::DirEntry) -> bool {
    entry.file_type().is_ok_and(|kind| kind.is_dir())
}

/// True when the workspace has a CPAN build marker or any `.pm`, `.pl` or
/// `.t` file outside skipped directories. One walk that stops at the first
/// hit; `detect_languages` and the Perl preview share this definition.
fn perl_project_detected(root: &Path) -> bool {
    fn any_perl_file(dir: &Path) -> bool {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        entries.flatten().any(|entry| {
            let path = entry.path();
            if is_walkable_dir(&entry) {
                !is_skipped_walk_dir(&path) && any_perl_file(&path)
            } else {
                path.is_file()
                    && matches!(
                        path.extension().and_then(|e| e.to_str()),
                        Some("pm" | "pl" | "t")
                    )
            }
        })
    }
    has_perl_project_markers(root) || any_perl_file(root)
}

fn count_files(root: &Path, ext: &str) -> usize {
    fn count_recursive(dir: &Path, ext: &str) -> usize {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return 0;
        };
        let mut n = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            if is_walkable_dir(&entry) {
                if is_skipped_walk_dir(&path) {
                    continue;
                }
                n += count_recursive(&path, ext);
            } else if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some(ext) {
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
    if !perl_project_detected(root) {
        return;
    }
    let pm_count = count_files(root, "pm");
    let pl_count = count_files(root, "pl");
    let t_count = count_files(root, "t");

    println!("- Perl preview:");
    println!("  project: {pm_count} .pm, {pl_count} .pl, {t_count} .t");

    // lang-perl compiled? (cfg!(feature = "lang-perl") is build-time constant.)
    if cfg!(feature = "lang-perl") {
        println!("  adapter: compiled (lang-perl feature ON)");
    } else {
        println!("  adapter: NOT compiled in this ripr binary (see next)");
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

    // Find a compatible exporter: one that answers `--version` AND accepts
    // the managed `ripr-facts` subcommand. A binary that only answers
    // `--version` (for example the published perllsp LSP server) is reported
    // as found-but-incompatible, never as a working exporter.
    if let Some(refused) = crate::config::load_for_root(root)
        .ok()
        .and_then(|config| config.perl().refused_executable().map(Path::to_path_buf))
    {
        println!(
            "  executable: ignoring [perl].executable `{}` from ripr.toml (not run); set {}=1 to trust it",
            refused.display(),
            crate::config::PERL_EXECUTABLE_OPT_IN_ENV
        );
    }
    let exporter = probe_perl_exporter(root);
    for line in perl_exporter_lines(&exporter) {
        println!("  {line}");
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

    // Exact next command: branch on whether the adapter is compiled in,
    // whether managed mode is configured, and whether a COMPATIBLE exporter
    // is present.
    let next = perl_next_command(
        LanguageId::Perl.is_available(),
        producer_configured.as_deref(),
        exporter.compatible_bin(),
    );
    println!("  next: {next}");
}

/// Whether `[perl].producer` is configured in the root's ripr config. Returns
/// the configured producer name, or None if not set / config unreadable.
fn perl_producer_configured(root: &Path) -> Option<String> {
    let config = crate::config::load_for_root(root).ok()?;
    config.perl().producer().map(|s| s.to_string())
}

/// Result of probing for a Perl fact exporter.
#[derive(Debug, PartialEq, Eq)]
enum PerlExporterProbe {
    /// Answers `--version` and accepts the managed `ripr-facts` subcommand.
    Compatible { bin: String, version: String },
    /// Answers `--version` but rejects `ripr-facts`, so managed mode would
    /// fail against it. Not an exporter ripr can use.
    Incompatible { bin: String, version: String },
    /// No candidate answered `--version`.
    NotFound,
}

impl PerlExporterProbe {
    fn compatible_bin(&self) -> Option<&str> {
        match self {
            PerlExporterProbe::Compatible { bin, .. } => Some(bin),
            _ => None,
        }
    }
}

/// Upper bound on bytes captured from each probe stream. Help and version
/// text is small; an unknown binary must not flood the doctor.
const PERL_EXPORTER_PROBE_OUTPUT_LIMIT: usize = 64 * 1024;

/// Probe for a compatible Perl fact exporter. Honors `[perl].executable`
/// when set; otherwise probes PATH for `perl-ripr-facts` (canonical, post
/// perl-lsp-swarm #3294), then `perllsp` (the compatibility name managed
/// mode invokes for `producer = "perllsp"` or `"perl-lsp"`). A bare
/// `perl-lsp` binary is not probed: managed mode never invokes that name,
/// and unrelated crates install binaries called `perl-lsp`.
///
/// Compatibility is a capability probe, not an end-to-end proof: the
/// candidate must exit successfully for `ripr-facts --help` and its help
/// must mention `--schema`, the first flag of the managed argv. Both probes
/// run under the configured `[perl].timeout_ms` deadline with bounded
/// capture and a null stdin, so an LSP server that waits on stdin cannot
/// hang the doctor. Packet validity is still only checked by `ripr check`.
fn probe_perl_exporter(root: &Path) -> PerlExporterProbe {
    let config = crate::config::load_for_root(root).ok();
    let timeout =
        std::time::Duration::from_millis(config.as_ref().map_or(30_000, |c| c.perl().timeout_ms()));
    // `[perl].executable` from ripr.toml is only probed when the user opts
    // in (see `PerlConfig::executable`); doctor is usually the first command
    // run in a fresh clone and must not execute a repository-chosen program.
    let explicit = config
        .as_ref()
        .and_then(|c| c.perl().executable().map(|p| p.display().to_string()));
    let candidates: Vec<String> = match explicit {
        Some(path) => vec![path],
        None => vec![
            crate::domain::PERL_FACT_EXPORTER.to_string(),
            "perllsp".to_string(),
        ],
    };
    let mut first_incompatible = None;
    for candidate in &candidates {
        let Some(version) = run_exporter_probe(candidate, &["--version"], timeout)
            .filter(|output| output.status.success())
            .map(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            })
        else {
            continue;
        };
        let version = if version.is_empty() {
            "version unknown".to_string()
        } else {
            version
        };
        let bin = which(candidate)
            .then(|| resolve_binary_path(candidate))
            .flatten()
            .unwrap_or_else(|| candidate.clone());
        if exporter_accepts_ripr_facts(candidate, timeout) {
            return PerlExporterProbe::Compatible { bin, version };
        }
        first_incompatible.get_or_insert(PerlExporterProbe::Incompatible { bin, version });
    }
    first_incompatible.unwrap_or(PerlExporterProbe::NotFound)
}

/// Whether `candidate ripr-facts --help` succeeds and documents `--schema`.
fn exporter_accepts_ripr_facts(candidate: &str, timeout: std::time::Duration) -> bool {
    run_exporter_probe(candidate, &["ripr-facts", "--help"], timeout).is_some_and(|output| {
        output.status.success()
            && (String::from_utf8_lossy(&output.stdout).contains("--schema")
                || String::from_utf8_lossy(&output.stderr).contains("--schema"))
    })
}

/// The single exporter spawn site for doctor: bounded, deadline-enforced,
/// null stdin. `None` when the binary cannot be spawned, times out, or
/// exceeds the capture limit.
fn run_exporter_probe(
    candidate: &str,
    args: &[&str],
    timeout: std::time::Duration,
) -> Option<std::process::Output> {
    let mut command = std::process::Command::new(candidate);
    command.args(args);
    crate::git::collect_output_with_deadline_and_limit(
        command,
        timeout,
        PERL_EXPORTER_PROBE_OUTPUT_LIMIT,
        &format!("Perl fact exporter probe `{candidate}`"),
    )
    .ok()
}

/// Doctor lines for an exporter probe result.
fn perl_exporter_lines(exporter: &PerlExporterProbe) -> Vec<String> {
    match exporter {
        PerlExporterProbe::Compatible { bin, version } => vec![format!(
            "exporter: compatible `{bin}` ({version}) accepts `ripr-facts` (capability probe only; packets are validated by `ripr check`)"
        )],
        PerlExporterProbe::Incompatible { bin, version } => vec![
            format!(
                "exporter: found `{bin}` ({version}) but it does not accept `ripr-facts`; not a compatible exporter"
            ),
            format!(
                "note: managed mode runs `<exporter> ripr-facts --schema {} ...`; the compatible exporter is `{}`, which is not yet published",
                crate::app::PERL_FACT_PACKET_SCHEMA,
                crate::domain::PERL_FACT_EXPORTER
            ),
        ],
        PerlExporterProbe::NotFound => vec![format!(
            "exporter: NOT found (expected `{}` or a `perllsp` wrapper on PATH, or [perl].executable with {}=1); `{}` is not yet published",
            crate::domain::PERL_FACT_EXPORTER,
            crate::config::PERL_EXECUTABLE_OPT_IN_ENV,
            crate::domain::PERL_FACT_EXPORTER
        )],
    }
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

/// Choose the exact next command based on adapter availability, producer
/// configuration, and whether a COMPATIBLE exporter was found.
///
/// An uncompiled adapter comes first: every other recommendation (a
/// `ripr.toml` edit, `--perl-facts`) fails against this binary, so the only
/// honest next step is the shared prerequisite text.
fn perl_next_command(
    adapter_compiled: bool,
    producer_configured: Option<&str>,
    compatible_exporter: Option<&str>,
) -> String {
    if !adapter_compiled {
        return LanguageId::Perl.unavailable_adapter_recovery();
    }
    let managed = producer_configured.is_some_and(crate::app::is_managed_perl_producer);
    if managed && compatible_exporter.is_some() {
        // Managed mode + compatible producer present: ripr invokes the
        // exporter itself. There is no --languages flag (#2105): perl is
        // enabled through config, and check then runs the enabled set.
        // Name the additive edit, not a replacement list, so a user with
        // TypeScript/Python already enabled keeps them (#2105 review).
        "add \"perl\" to [languages] enabled in ripr.toml, then: ripr check".to_string()
    } else if managed {
        // Managed mode configured but no compatible producer.
        format!(
            "install a compatible Perl fact exporter (`{}`, not yet published) on PATH, or set [perl].executable and {}=1, and add \"perl\" to [languages] enabled in ripr.toml, then: ripr check",
            crate::domain::PERL_FACT_EXPORTER,
            crate::config::PERL_EXECUTABLE_OPT_IN_ENV
        )
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
        "  Preview-language evidence is advisory and does not block by default; \
        it yields repair cards or packets only for findings that satisfy the full \
        actionability, edit, verify, and receipt contract"
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
            output::path::human_path(&cache_dir)
        );
    } else {
        println!("- Cache location: {}", output::path::human_path(&cache_dir));
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
        if is_walkable_dir(&entry) {
            total = total.saturating_add(dir_size_bytes(&entry.path()));
        } else if let Ok(meta) = entry.metadata() {
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
                    println!("- Config path: {}", output::path::human_path(path));
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
            println!(
                "- Config path: {}",
                output::path::human_path(&root.join(CONFIG_FILE_NAME))
            );
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
        let host_python = python_runtime_for_os(std::env::consts::OS).0;
        assert_eq!(tools, vec![host_python, "pytest"]);
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
        assert_eq!(tools, vec![host_python]);
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
    fn detect_languages_finds_a_cpan_layout_perl_project() -> Result<(), String> {
        // A module two levels under `lib/` is below the shallow scan, yet
        // the Perl preview counts it; detection must agree, or doctor prints
        // "none detected" beside a Perl section counting files. Files under a
        // skipped directory (`target/`, `node_modules/`) detect nothing.
        let cases: [(&str, &[&str]); 4] = [
            ("deep", &["lib/Acme/Calc.pm", "README"]),
            ("tests", &["t/calc.t"]),
            ("marker", &["Makefile.PL"]),
            ("none", &["target/example.t", "node_modules/pkg/lib/X.pm"]),
        ];
        for (name, files) in cases {
            let root = unique_command_test_dir(&format!("detect-perl-{name}"));
            for file in files {
                let path = root.join(file);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|err| format!("mkdir: {err}"))?;
                }
                std::fs::write(&path, "1;\n").map_err(|err| format!("write {file}: {err}"))?;
            }
            let expected = if name == "none" {
                Vec::new()
            } else {
                vec![LanguageId::Perl]
            };
            assert_eq!(detect_languages(&root), expected, "{name}");
            std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        }
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn perl_walks_do_not_follow_directory_symlink_loops() -> Result<(), String> {
        // A `src/loop -> .` link made `ripr doctor` descend forever. The Perl
        // and cache-size walks must terminate, and a `.pm` reachable only
        // through a link does not count, matching the Rust/Python/TypeScript
        // walkers.
        let root = unique_command_test_dir("perl-symlink-loop");
        let src = root.join("src");
        std::fs::create_dir_all(&src).map_err(|err| format!("mkdir: {err}"))?;
        std::os::unix::fs::symlink(".", src.join("loop"))
            .map_err(|err| format!("symlink: {err}"))?;
        std::os::unix::fs::symlink("../src", src.join("up"))
            .map_err(|err| format!("symlink: {err}"))?;
        assert!(!perl_project_detected(&root));
        assert_eq!(count_files(&root, "pm"), 0);

        // A directory link whose name ends in `.pm` is not a Perl file.
        let other = root.join("other");
        std::fs::create_dir_all(&other).map_err(|err| format!("mkdir: {err}"))?;
        std::os::unix::fs::symlink("../other", src.join("linked.pm"))
            .map_err(|err| format!("symlink: {err}"))?;
        assert!(!perl_project_detected(&root));
        assert_eq!(count_files(&root, "pm"), 0);
        // The cache-size walk terminates on the same loops.
        let size_before = dir_size_bytes(&root);

        std::fs::write(src.join("Real.pm"), "1;\n").map_err(|err| format!("write: {err}"))?;
        assert!(perl_project_detected(&root));
        assert_eq!(count_files(&root, "pm"), 1);
        assert_eq!(dir_size_bytes(&root), size_before + 3);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn detect_languages_treats_modern_ts_js_extensions_as_language_markers() -> Result<(), String> {
        // #4116: .mts/.cts are TypeScript markers and .mjs/.cjs are
        // JavaScript markers through the shared extension authority; a
        // mixed root is labeled TypeScript only, and near-misses stay
        // undetected.
        for (extension, expected) in [
            ("mts", LanguageId::TypeScript),
            ("cts", LanguageId::TypeScript),
            ("mjs", LanguageId::JavaScript),
            ("cjs", LanguageId::JavaScript),
        ] {
            let root = unique_command_test_dir(&format!("detect-{extension}"));
            std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
            std::fs::write(root.join(format!("index.{extension}")), "export {};\n")
                .map_err(|err| format!("write source: {err}"))?;
            assert_eq!(detect_languages(&root), vec![expected], ".{extension} root");
            std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        }

        let root = unique_command_test_dir("detect-mixed-ts-js");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        std::fs::write(root.join("index.ts"), "export {};\n")
            .map_err(|err| format!("write source: {err}"))?;
        std::fs::write(root.join("helper.mjs"), "export {};\n")
            .map_err(|err| format!("write source: {err}"))?;
        assert_eq!(detect_languages(&root), vec![LanguageId::TypeScript]);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;

        let root = unique_command_test_dir("detect-near-miss");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        std::fs::write(root.join("index.mjsx"), "export {};\n")
            .map_err(|err| format!("write source: {err}"))?;
        assert!(
            !detect_languages(&root).contains(&LanguageId::TypeScript)
                && !detect_languages(&root).contains(&LanguageId::JavaScript),
            ".mjsx must not detect as a TS/JS marker"
        );
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
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

    /// #4378: `ripr doctor` on Windows suggested `apt install python3` and
    /// failed a host whose installer put `python` (not `python3`) on PATH.
    #[test]
    fn python_runtime_probe_and_hint_follow_the_host() {
        let (windows_tool, windows_hint) = python_runtime_for_os("windows");
        assert_eq!(windows_tool, "python");
        assert!(windows_hint.contains("winget install"), "{windows_hint}");
        assert!(!windows_hint.contains("apt"), "{windows_hint}");

        let (macos_tool, macos_hint) = python_runtime_for_os("macos");
        assert_eq!(macos_tool, "python3");
        assert!(macos_hint.contains("brew install"), "{macos_hint}");
        assert!(!macos_hint.contains("apt"), "{macos_hint}");

        assert_eq!(
            python_runtime_for_os("linux"),
            ("python3", "install python3 (e.g. apt install python3)")
        );

        // The configured-language probe and the detected-language probe use
        // the same host answer, so a Windows root is never probed twice.
        let host = python_runtime_for_os(std::env::consts::OS);
        assert_eq!(primary_runtime("python"), Some(host));
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
                .any(|probe| probe.language == "python"
                    && probe.tool == python_runtime_for_os(std::env::consts::OS).0)
        );
        std::fs::remove_dir_all(&configured_only_root)
            .map_err(|err| format!("remove configured-only root: {err}"))?;
        Ok(())
    }

    #[test]
    fn source_build_profile_keeps_enabled_language_runtime_failure_advisory() -> Result<(), String>
    {
        // PR #4196 review: an enabled TypeScript root with no node is an
        // analysis failure, but not a RIPR source-build prerequisite. The
        // same probe outcome must fail analysis and stay advisory for
        // source-build, while still being reported.
        let root = unique_command_test_dir("runtime-probe-source-build");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        std::fs::write(root.join("package.json"), "{}")
            .map_err(|err| format!("write package marker: {err}"))?;
        let missing_node = |tool: &str, _isolated: bool| {
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
        };
        let mut outcomes = Vec::new();
        for profile in [
            output::doctor::DoctorProfile::Analysis,
            output::doctor::DoctorProfile::SourceBuild,
        ] {
            let mut report = output::doctor::DoctorReport::new(&root.display().to_string());
            report.profile = profile;
            let ok = add_language_runtime_probes(
                &root,
                &[LanguageId::TypeScript],
                &mut report,
                false,
                missing_node,
            );
            let node = report
                .runtime_probes
                .iter()
                .find(|probe| probe.tool == "node")
                .ok_or_else(|| format!("missing node probe for {profile:?}"))?;
            outcomes.push((
                profile,
                ok,
                report.status,
                node.required,
                node.status,
                output::doctor::doctor_report_result(&report).is_ok(),
            ));
        }
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        assert_eq!(
            outcomes,
            vec![
                (
                    output::doctor::DoctorProfile::Analysis,
                    false,
                    output::doctor::DoctorStatus::Fail,
                    true,
                    output::doctor::DoctorStatus::Fail,
                    false,
                ),
                (
                    output::doctor::DoctorProfile::SourceBuild,
                    true,
                    output::doctor::DoctorStatus::Pass,
                    false,
                    output::doctor::DoctorStatus::Fail,
                    true,
                ),
            ]
        );
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
        for compiled in [true, false] {
            for (producer, found) in [
                (Some("perllsp"), Some("perllsp")),
                (Some("perllsp"), None),
                (Some("perl-ripr-facts"), None),
                (None, None),
            ] {
                let command = perl_next_command(compiled, producer, found);
                assert!(
                    !command.contains("--languages"),
                    "recommendation must not name --languages: {command}"
                );
            }
        }
        // The managed-present branch points at the config-driven route.
        let managed = perl_next_command(true, Some("perllsp"), Some("perllsp"));
        assert!(managed.contains("[languages]"));
        // #3886: a bare `ripr check` resolves the default branch; `check`
        // has no `--head`, and `origin/main` need not exist.
        assert!(managed.ends_with("then: ripr check"));
        let unpublished = perl_next_command(true, Some("perllsp"), None);
        assert!(unpublished.ends_with("then: ripr check"));
        // A repo `[perl].executable` is ignored without the user opt-in, so
        // recommending it must name the opt-in.
        assert!(
            unpublished.contains("set [perl].executable and RIPR_ALLOW_REPO_PERL_EXECUTABLE=1"),
            "{unpublished}"
        );
        // The packet-mode branch is unchanged.
        let packet = perl_next_command(true, None, None);
        assert!(packet.contains("--perl-facts"));
    }

    #[test]
    fn perl_next_command_names_the_unpublished_exporter_not_perllsp() {
        // Managed mode without a compatible exporter: the install hint must
        // name the canonical exporter and say it is not published, not the
        // argv-incompatible `perllsp` LSP server.
        let missing = perl_next_command(true, Some("perllsp"), None);
        assert!(
            !missing.contains("install perllsp"),
            "must not recommend installing perllsp: {missing}"
        );
        assert!(
            missing.contains("`perl-ripr-facts`") && missing.contains("not yet published"),
            "must name the unpublished canonical exporter: {missing}"
        );
    }

    #[test]
    fn perl_next_command_without_adapter_never_suggests_a_rejected_edit() {
        // In a build without `lang-perl`, adding perl to [languages] makes
        // `ripr check` exit 2, and --perl-facts cannot be analyzed either,
        // so every branch must return the shared prerequisite text.
        for (producer, found) in [
            (Some("perllsp"), Some("perllsp")),
            (Some("perl-ripr-facts"), None),
            (None, None),
        ] {
            let command = perl_next_command(false, producer, found);
            assert_eq!(command, LanguageId::Perl.unavailable_adapter_recovery());
            assert!(
                !command.contains("then: ripr check") && !command.contains("--perl-facts <"),
                "uncompiled adapter must not get a runnable-looking next step: {command}"
            );
        }
    }

    #[test]
    fn perl_exporter_lines_never_call_an_incompatible_binary_found() {
        let incompatible = perl_exporter_lines(&PerlExporterProbe::Incompatible {
            bin: "/opt/bin/perllsp".to_string(),
            version: "perllsp 0.17.0".to_string(),
        });
        let first = incompatible.first().map(String::as_str).unwrap_or("");
        assert!(
            first.contains("does not accept `ripr-facts`")
                && first.contains("not a compatible exporter")
                && !first.starts_with("exporter: found at"),
            "incompatible exporter must not read as found/working: {incompatible:?}"
        );
        // The second line is the prerequisite pointer: the argv ripr sends and
        // the exporter that would accept it.
        assert_eq!(incompatible.len(), 2, "{incompatible:?}");
        let note = incompatible.get(1).map(String::as_str).unwrap_or("");
        assert!(
            note.starts_with("note: ")
                && note.contains(crate::app::PERL_FACT_PACKET_SCHEMA)
                && note.contains(crate::domain::PERL_FACT_EXPORTER)
                && note.contains("not yet published"),
            "incompatible exporter must name the argv and the compatible exporter: {note:?}"
        );
        let compatible = perl_exporter_lines(&PerlExporterProbe::Compatible {
            bin: "/opt/bin/perl-ripr-facts".to_string(),
            version: "perl-ripr-facts 0.1.0".to_string(),
        });
        assert!(
            compatible
                .first()
                .is_some_and(|line| line.starts_with("exporter: compatible")),
            "compatible exporter line: {compatible:?}"
        );
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

        let report = output::doctor::evaluate_doctor_core(&dir, &detect_languages(&dir));
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
        if config_check.status != output::doctor::DoctorCheckStatus::Fail {
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
        // Found by name, not by position: the check list grows, and an index
        // pins the order rather than the claim.
        let config_in_json = value["checks"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|check| check["name"] == "config");
        if value["status"] != "fail" || config_in_json.is_none_or(|check| check["status"] != "fail")
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

        let report = output::doctor::evaluate_doctor_core(&root, &detect_languages(&root));
        if report.status != output::doctor::DoctorStatus::Fail {
            return Err(format!("missing root should fail, got {:?}", report.status));
        }
        let root_check = report
            .checks
            .iter()
            .find(|check| check.name == "root_directory")
            .ok_or_else(|| "missing root-directory check".to_string())?;
        if root_check.status != output::doctor::DoctorCheckStatus::Fail {
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
        if doctor_json(&dir, output::doctor::DoctorProfile::Analysis).is_ok() {
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
        // No ripr.toml → defaults to enabled = ["rust"] only.
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
