//! Metadata-only ignored preflight; no normal binary/API or analysis receipt.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
#[cfg(any(feature = "lang-typescript", feature = "lang-python"))]
use std::collections::HashMap;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub(crate) const STOP: &str = "source_owner_preflight_stop: native analysis NOT_RUN";
const MAX_METADATA_BYTES: usize = 4 * 1024 * 1024;
const BUDGETS: [&str; 10] = [
    "RIPR_MAX_DIFF_INDEX_FILES",
    "RIPR_MAX_DIFF_CHANGED_RUST_LINES",
    "RIPR_PARTIAL_DIFF_FILE_BUDGET",
    "RIPR_PARTIAL_DIFF_LINE_BUDGET",
    "RIPR_TS_MAX_WORKSPACE_FILES",
    "RIPR_TS_MAX_FILE_READ_BYTES",
    "RIPR_TS_MAX_WORKSPACE_READ_BYTES",
    "RIPR_PYTHON_MAX_WORKSPACE_FILES",
    "RIPR_PYTHON_MAX_FILE_READ_BYTES",
    "RIPR_PYTHON_MAX_WORKSPACE_READ_BYTES",
];
#[derive(Default)]
struct Observation {
    stages: BTreeMap<String, Value>,
    limits: BTreeMap<String, u64>,
    events: Vec<String>,
    attempts: BTreeMap<String, u64>,
    open: BTreeSet<PathBuf>,
}
thread_local! { static CURRENT: RefCell<Option<Observation>> = const { RefCell::new(None) }; }
struct Restore(Option<Observation>);
impl Drop for Restore {
    fn drop(&mut self) {
        CURRENT.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}
pub(crate) fn active() -> bool {
    CURRENT.with(|slot| slot.borrow().is_some())
}
pub(crate) fn put(stage: &str, value: Value) {
    CURRENT.with(|slot| {
        if let Some(state) = slot.borrow_mut().as_mut() {
            state.events.push(stage.to_owned());
            state.stages.insert(stage.to_owned(), value);
        }
    });
}
pub(crate) fn limit(name: &str, value: usize) {
    CURRENT.with(|slot| {
        if let Some(state) = slot.borrow_mut().as_mut() {
            state.limits.insert(name.to_owned(), value as u64);
        }
    });
}
pub(crate) fn continue_walk() -> bool {
    if !active() {
        return true;
    }
    match super::cancellation::checkpoint() {
        Ok(()) => true,
        Err(error) => {
            put("cooperative_abort", json!({"error": error}));
            false
        }
    }
}
pub(crate) fn paths_identity<'a>(paths: impl IntoIterator<Item = &'a Path>) -> String {
    let mut paths: Vec<_> = paths
        .into_iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect();
    paths.sort();
    let mut hash = Sha256::new();
    for path in paths {
        hash.update((path.len() as u64).to_le_bytes());
        hash.update(path.as_bytes());
    }
    format!("sha256:{:x}", hash.finalize())
}
pub(crate) fn bytes_identity(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
pub(crate) fn open_paths(paths: &BTreeSet<PathBuf>) {
    CURRENT.with(|slot| {
        if let Some(state) = slot.borrow_mut().as_mut() {
            state.open.extend(paths.iter().cloned());
        }
    });
}
pub(crate) fn rust_loaded(files: &[(PathBuf, Vec<u8>)], selected_count: usize) {
    let open = CURRENT.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|state| state.open.clone())
            .unwrap_or_default()
    });
    let bytes: u64 = files.iter().map(|(_, bytes)| bytes.len() as u64).sum();
    let lines: usize = files
        .iter()
        .map(|(_, bytes)| bytes.split_inclusive(|byte| *byte == b'\n').count())
        .sum();
    let open_lines: usize = files
        .iter()
        .filter(|(path, _)| open.contains(path))
        .map(|(_, bytes)| bytes.split_inclusive(|byte| *byte == b'\n').count())
        .sum();
    put(
        "rust_loaded",
        json!({"selected_files": selected_count, "loaded_files": files.len(),
        "absent_at_head": selected_count.saturating_sub(files.len()), "source_bytes": bytes,
        "loaded_source_lines": lines, "loaded_open_source_lines": open_lines,
        "paths_identity": paths_identity(files.iter().map(|(path, _)| path.as_path())),
        "main_ast": "NOT_RUN", "probe_generation": "NOT_RUN", "classification": "NOT_RUN"}),
    );
}
#[cfg(any(feature = "lang-typescript", feature = "lang-python"))]
pub(crate) fn read_attempt(language: &str) {
    CURRENT.with(|slot| {
        if let Some(state) = slot.borrow_mut().as_mut() {
            *state.attempts.entry(language.to_owned()).or_default() += 1;
        }
    });
}
#[cfg(any(feature = "lang-typescript", feature = "lang-python"))]
pub(crate) fn preview_read(
    language: &str,
    sources: &HashMap<PathBuf, String>,
    limits: usize,
    io_failures: usize,
    file_limit: u64,
    workspace_budget: u64,
) {
    let attempts = CURRENT.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(|state| state.attempts.get(language))
            .copied()
            .unwrap_or(0)
    });
    CURRENT.with(|slot| {
        if let Some(state) = slot.borrow_mut().as_mut() {
            let prefix = if language == "typescript" {
                "RIPR_TS"
            } else {
                "RIPR_PYTHON"
            };
            state
                .limits
                .insert(format!("{prefix}_MAX_FILE_READ_BYTES"), file_limit);
            state.limits.insert(
                format!("{prefix}_MAX_WORKSPACE_READ_BYTES"),
                workspace_budget,
            );
        }
    });
    put(
        &format!("{language}_source_read"),
        json!({"attempted_paths": attempts, "accepted_files": sources.len(),
        "successful_source_bytes": sources.values().map(|source| source.len() as u64).sum::<u64>(),
        "limit_entries": limits, "io_failures": io_failures,
        "paths_identity": paths_identity(sources.keys().map(PathBuf::as_path)),
        "later_config_package_reads": "NOT_REACHED", "parse_limits": "NOT_COMPUTED", "classification": "NOT_RUN"}),
    );
}
pub(crate) fn observe(
    run: impl FnOnce() -> Result<(), String>,
) -> Result<(Result<(), String>, Value), String> {
    if active() {
        return Err("source-owner observation cannot be nested".to_owned());
    }
    let previous = CURRENT.with(|slot| slot.borrow_mut().replace(Observation::default()));
    let _restore = Restore(previous);
    let result = run();
    let state = CURRENT
        .with(|slot| slot.borrow_mut().take())
        .ok_or("source-owner observation disappeared")?;
    Ok((
        result,
        json!({"stages": state.stages, "resolved_limits": state.limits, "stage_order": state.events, "operation_counts": state.attempts}),
    ))
}
fn finish(
    result: Result<(), String>,
    mut report: Value,
    enabled: &[crate::domain::LanguageId],
) -> Value {
    use crate::domain::LanguageId;
    let stages = &report["stages"];
    let required: Vec<_> = enabled
        .iter()
        .map(|language| match language {
            LanguageId::Rust => "rust_loaded",
            LanguageId::TypeScript | LanguageId::JavaScript => "typescript_source_read",
            LanguageId::Python => "python_source_read",
            LanguageId::Perl => "unsupported_adapter",
        })
        .collect();
    let stopped = result.as_ref().err().is_some_and(|error| error == STOP);
    let reached = stopped
        && stages.get("parsed_diff").is_some()
        && required.iter().all(|stage| stages.get(*stage).is_some());
    let refused = report["operation_counts"]
        .as_object()
        .is_some_and(|counts| {
            counts.iter().any(|(name, value)| {
                name.ends_with("_discovery_io") && value.as_u64().unwrap_or(0) != 0
            })
        })
        || stages.get("cooperative_abort").is_some()
        || stages["rust_partition"]["partial"].as_bool() == Some(true)
        || stages["rust_loaded"]["absent_at_head"]
            .as_u64()
            .unwrap_or(0)
            != 0
        || stages["typescript_discovery"]["truncated"].as_bool() == Some(true)
        || stages["python_discovery"]["refused_files"]
            .as_u64()
            .unwrap_or(0)
            != 0
        || ["typescript_source_read", "python_source_read"]
            .iter()
            .any(|stage| {
                stages[*stage]["limit_entries"].as_u64().unwrap_or(0) != 0
                    || stages[*stage]["io_failures"].as_u64().unwrap_or(0) != 0
            });
    report["schema"] = json!("ripr.source_owner_preflight.v1");
    report["counter_stage_complete"] = json!(reached);
    report["preflight_admitted"] = json!(reached && !refused);
    report["analysis_complete"] = json!(false);
    report["native_analysis"] = json!("NOT_RUN");
    report["strict_producer_admission"] = json!("NOT_RUN");
    report["requested_stages"] = json!(required);
    report["enabled_languages"] = json!(
        enabled
            .iter()
            .map(|language| language.as_str())
            .collect::<Vec<_>>()
    );
    report["terminal_error"] = json!(result.err());
    report["scope_limit"] = json!(
        "Counter preparation only; Rust module-role AST, final index/working set, later preview config/package reads and parse/probe/classifier costs remain unknown. Worktree corpus is observed, not an atomic physical snapshot."
    );
    report
}
fn run(input: crate::app::CheckInput, config: &crate::config::RiprConfig) -> Result<Value, String> {
    let enabled = config.languages().enabled();
    if enabled.contains(&crate::domain::LanguageId::Perl)
        || config.perl().producer().is_some()
        || input.perl_facts_path.is_some()
    {
        return Err("source-owner preflight has no Perl counter boundary; managed/external producer NOT_RUN".to_owned());
    }
    if enabled.contains(&crate::domain::LanguageId::TypeScript)
        && enabled.contains(&crate::domain::LanguageId::JavaScript)
    {
        return Err("source-owner preflight duplicate TypeScript/JavaScript adapter requests are not yet separably counted; NOT_RUN".to_owned());
    }
    let (result, report) =
        observe(|| crate::app::check_workspace_with_config(input, config).map(|_| ()))?;
    Ok(finish(result, report, enabled))
}
fn required(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|error| format!("{name} is required: {error}"))
}
fn full_hex(value: &str, width: usize) -> bool {
    value.len() == width
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn git_text(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = crate::git::run_git_output_with_deadline(
        root,
        args,
        Some(std::time::Duration::from_secs(300)),
    )
    .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "source-owner Git identity refused: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout)
        .map(|text| text.trim().to_owned())
        .map_err(|error| error.to_string())
}
fn packet_identity(path: &Path) -> Result<(u64, String), String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut buffer = [0_u8; 8192];
    let mut bytes = 0_u64;
    let mut digest = Sha256::new();
    loop {
        super::cancellation::checkpoint()?;
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        if bytes > 256 * 1024 * 1024 {
            return Err("source-owner packet exceeds existing CLI input cap".to_owned());
        }
        digest.update(&buffer[..count]);
    }
    Ok((bytes, format!("sha256:{:x}", digest.finalize())))
}
fn require_consumed_packet(report: &Value, bytes: u64, sha256: &str) -> Result<(), String> {
    let observed = &report["stages"]["parser_input"];
    if observed["input_bytes"].as_u64() != Some(bytes)
        || observed["input_sha256"].as_str() != Some(sha256)
    {
        return Err(
            "source-owner consumed parser packet identity missing or mismatched".to_owned(),
        );
    }
    Ok(())
}
fn driver() -> Result<Value, String> {
    let root = std::fs::canonicalize(required("RIPR_SOURCE_PREFLIGHT_ROOT")?)
        .map_err(|error| error.to_string())?;
    let base = required("RIPR_SOURCE_PREFLIGHT_BASE")?;
    let head = required("RIPR_SOURCE_PREFLIGHT_HEAD")?;
    let tree = required("RIPR_SOURCE_PREFLIGHT_TREE")?;
    let expected_diff = required("RIPR_SOURCE_PREFLIGHT_DIFF_SHA256")?;
    if ![&base, &head, &tree]
        .iter()
        .all(|value| full_hex(value, 40))
        || !full_hex(&expected_diff, 64)
    {
        return Err(
            "source-owner preflight requires exact lowercase Git and SHA256 identities".to_owned(),
        );
    }
    let git_root = std::fs::canonicalize(git_text(&root, &["rev-parse", "--show-toplevel"])?)
        .map_err(|error| error.to_string())?;
    if git_root != root {
        return Err(
            "source-owner preflight requires the canonical full repository root".to_owned(),
        );
    }
    let observed_head = git_text(&root, &["rev-parse", "HEAD", "HEAD^{tree}"])?;
    if observed_head != format!("{head}\n{tree}")
        || git_text(&root, &["merge-base", &base, &head])? != base
    {
        return Err("source-owner full subject HEAD/tree/base identity mismatch".to_owned());
    }
    // Ordinary tracked Git cleanliness, not atomic physical-byte proof or ignored-source membership.
    let clean = crate::git::run_git_output_with_deadline(
        &root,
        &["diff", "--quiet", "HEAD", "--"],
        Some(std::time::Duration::from_secs(300)),
    )
    .map_err(|error| error.to_string())?;
    if !clean.status.success() {
        return Err("source-owner preflight refuses tracked worktree changes".to_owned());
    }
    let diff = PathBuf::from(required("RIPR_SOURCE_PREFLIGHT_DIFF")?);
    let (packet_bytes, packet_sha256) = packet_identity(&diff)?;
    if packet_sha256 != format!("sha256:{expected_diff}") {
        return Err("source-owner whole packet identity mismatch".to_owned());
    }
    let canonical = super::load_pr_evidence_diff_range(&root, &base, &head)?;
    if canonical.len() as u64 != packet_bytes
        || bytes_identity(canonical.as_bytes()) != packet_sha256
    {
        return Err(
            "source-owner packet is not the whole canonical base-triple-dot-head presentation"
                .to_owned(),
        );
    }
    drop(canonical);
    let mut requested = BTreeMap::new();
    for name in BUDGETS {
        let value = required(name)?
            .trim()
            .parse::<u64>()
            .map_err(|error| format!("{name}: {error}"))?;
        if value == 0 {
            return Err(format!("{name} must be explicitly positive"));
        }
        requested.insert(name.to_owned(), value);
    }
    let config = crate::config::load_for_root(&root)?;
    let mut input = crate::app::CheckInput {
        root: root.clone(),
        base: Some(base.clone()),
        diff_file: Some(diff.clone()),
        include_unchanged_tests: false,
        format: crate::app::OutputFormat::Json,
        git_timeout: Some(std::time::Duration::from_secs(300)),
        ..Default::default()
    };
    crate::config::apply_to_check_input(
        &mut input,
        &config,
        crate::config::CheckInputExplicit {
            mode: false,
            include_unchanged_tests: true,
        },
    );
    let effective_mode = input.mode.as_str().to_owned();
    let mut report = run(input, &config)?;
    require_consumed_packet(&report, packet_bytes, &packet_sha256)?;
    let (after_bytes, after_sha256) = packet_identity(&diff)?;
    if after_bytes != packet_bytes
        || after_sha256 != packet_sha256
        || git_text(&root, &["rev-parse", "HEAD", "HEAD^{tree}"])? != observed_head
        || crate::config::load_for_root(&root)? != config
    {
        return Err(
            "source-owner subject/config/packet changed during counter preparation".to_owned(),
        );
    }
    let resolved = &report["resolved_limits"];
    let budgets_match = requested
        .iter()
        .all(|(name, value)| resolved.get(name).and_then(Value::as_u64) == Some(*value));
    report["preflight_admitted"] =
        json!(report["preflight_admitted"].as_bool() == Some(true) && budgets_match);
    report["requested_limits"] = json!(requested);
    report["resolved_limits_match_requested"] = json!(budgets_match);
    report["subject"] = json!({"root": root, "base": base, "head": head, "tree": tree, "packet_bytes": packet_bytes, "packet_sha256": packet_sha256,
        "mode": effective_mode, "include_unchanged_tests": false, "open_rust_paths": 0,
        "config_path": config.source_path(), "config_source_sha256": config.source_text().map(|text| bytes_identity(text.as_bytes())),
        "finding_config_identity": crate::config::check_artifact_config_identity_hash(&config), "analyzer_version": env!("CARGO_PKG_VERSION"),
        "features": {"rust": cfg!(feature="lang-rust"), "typescript": cfg!(feature="lang-typescript"), "python": cfg!(feature="lang-python"), "perl": cfg!(feature="lang-perl")}});
    Ok(report)
}
#[test]
#[ignore = "metadata preparation only; needs exact whole-subject identities, finite budgets and separately admitted hosted envelope"]
fn full_source_owner_preflight() -> Result<(), String> {
    let started = std::time::Instant::now();
    let token = super::cancellation::AnalysisCancellationToken::with_budget(
        started,
        std::time::Duration::from_secs(300),
        std::sync::Arc::new(std::time::Instant::now),
    );
    let result = super::cancellation::with_token(&token, driver);
    let report = match result {
        Ok(report) => report,
        Err(error) => {
            json!({"schema": "ripr.source_owner_preflight.v1", "counter_stage_complete": false,
            "preflight_admitted": false, "analysis_complete": false, "native_analysis": "NOT_RUN", "terminal_error": error})
        }
    };
    let bytes = serde_json::to_vec(&report).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_METADATA_BYTES {
        return Err("source-owner preflight metadata exceeds 4MiB".to_owned());
    }
    println!(
        "RIPR_SOURCE_PREFLIGHT_JSON {}",
        String::from_utf8(bytes).map_err(|error| error.to_string())?
    );
    if report["preflight_admitted"].as_bool() != Some(true) {
        return Err("source-owner preflight refused; native analysis NOT_RUN".to_owned());
    }
    Ok(())
}
#[test]
fn observer_restores_and_never_turns_zero_results_into_native_acceptance() -> Result<(), String> {
    let (result, report) = observe(|| {
        put("parsed_diff", json!({"accepted_paths": 0}));
        Ok(())
    })?;
    let finished = finish(result, report, &[]);
    assert_eq!(finished["counter_stage_complete"], false);
    assert_eq!(finished["analysis_complete"], false);
    assert_eq!(finished["native_analysis"], "NOT_RUN");
    assert!(!active());
    let token = super::cancellation::AnalysisCancellationToken::new();
    token.cancel(super::cancellation::AnalysisAbortKind::DeadlineExceeded);
    let (_, report) = super::cancellation::with_token(&token, || {
        observe(|| {
            assert!(!continue_walk());
            Err(STOP.to_owned())
        })
    })?;
    assert!(
        report["stages"]["cooperative_abort"]["error"]
            .as_str()
            .is_some_and(|error| error.contains("DeadlineExceeded"))
    );
    assert!(!active());
    Ok(())
}

#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-python"
))]
pub(crate) struct OwnedFixture {
    pub(crate) root: PathBuf,
}
#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-python"
))]
impl Drop for OwnedFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-python"
))]
impl OwnedFixture {
    pub(crate) fn new() -> Result<Self, String> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        for _ in 0..16 {
            let root = std::env::temp_dir().join(format!(
                "ripr-source-counter-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            match std::fs::create_dir(&root) {
                Ok(()) => {
                    let fixture = Self { root };
                    fixture.seed("Cargo.toml", b"[package]\nname=\"counter_fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\n[workspace]\n")?;
                    return Ok(fixture);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.to_string()),
            }
        }
        Err("source-counter fixture could not acquire an exclusive root".to_owned())
    }
    pub(crate) fn seed(&self, relative: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(path, bytes).map_err(|error| error.to_string())
    }
}
#[cfg(feature = "lang-rust")]
#[test]
fn actual_rust_preparation_preserves_generated_scope_and_disabled_absence() -> Result<(), String> {
    let fixture = OwnedFixture::new()?;
    fixture.seed("src/lib.rs", b"pub fn positive(x: i32) -> bool { x > 0 }\n")?;
    fixture.seed(
        "src/generated.rs",
        b"// @generated\npub fn generated() {}\n",
    )?;
    let diff = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-pub fn positive(x: i32) -> bool { x >= 0 }\n+pub fn positive(x: i32) -> bool { x > 0 }\ndiff --git a/src/generated.rs b/src/generated.rs\n--- a/src/generated.rs\n+++ b/src/generated.rs\n@@ -2 +2 @@\n-pub fn generated() { }\n+pub fn generated() {}\n";
    fixture.seed("subject.diff", diff.as_bytes())?;
    let input = crate::app::CheckInput {
        root: fixture.root.clone(),
        diff_file: Some(fixture.root.join("subject.diff")),
        include_unchanged_tests: false,
        ..Default::default()
    };
    let config = crate::config::RiprConfig::default();
    let report = run(input.clone(), &config)?;
    assert_eq!(report["counter_stage_complete"], false, "{report:#}");
    assert_eq!(report["native_analysis"], "NOT_RUN");
    assert_eq!(report["analysis_complete"], false);
    assert_eq!(report["terminal_error"], STOP);
    assert_eq!(report["stages"]["parsed_diff"]["accepted_paths"], 2);
    assert_eq!(
        report["stages"]["rust_eligible"]["excluded_generated_files"],
        1
    );
    require_consumed_packet(&report, diff.len() as u64, &bytes_identity(diff.as_bytes()))?;
    for refused in [
        require_consumed_packet(
            &report,
            diff.len() as u64 + 1,
            &bytes_identity(diff.as_bytes()),
        ),
        require_consumed_packet(&report, diff.len() as u64, &bytes_identity(b"wrong packet")),
        require_consumed_packet(
            &json!({}),
            diff.len() as u64,
            &bytes_identity(diff.as_bytes()),
        ),
    ] {
        match refused {
            Err(error) => assert_eq!(
                error,
                "source-owner consumed parser packet identity missing or mismatched"
            ),
            Ok(()) => return Err("known-wrong consumed packet was admitted".to_owned()),
        }
    }
    assert!(
        report["stages"]["pipeline_stop"]["typed_preparation_limitations"]
            .as_array()
            .is_some_and(|items| items
                .iter()
                .any(|item| item["kind"] == "LanguageScopeUnsupported"
                    && item["stage"] == "LanguageAdapter"
                    && item["affected_items"] == 1)),
        "{report:#}"
    );
    assert_eq!(
        report["stages"]["rust_eligible"]["eligible_rust_changed_lines"],
        2
    );
    assert_eq!(
        report["stages"]["rust_pre_ast"]["status"],
        "NOT_COMPUTED_CORE_AST_REQUIRED"
    );
    assert_eq!(
        report["stages"]["rust_pre_ast"]["source_role_module_graph"],
        "NOT_RUN_AST_REQUIRED"
    );
    assert!(report["stages"]["rust_pre_ast"]["final_index_files"].is_null());
    assert!(report["stages"]["rust_loaded"].is_null());
    assert_eq!(report["preflight_admitted"], false);
    let mut disabled = config;
    disabled.languages.enabled.clear();
    let disabled_report = run(input, &disabled)?;
    assert_eq!(disabled_report["enabled_languages"], json!([]));
    assert!(disabled_report["stages"]["rust_loaded"].is_null());
    assert_eq!(disabled_report["native_analysis"], "NOT_RUN");
    assert!(!active());
    Ok(())
}
