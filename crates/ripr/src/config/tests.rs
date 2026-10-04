use super::python::{PYTHON_EXCLUDED_DIRS, PYTHON_VENDOR_DIR};
#[cfg(feature = "lang-python")]
use super::python::{PYTHON_PROJECT_MARKERS, PYTHON_SOURCE_DIR_MARKERS};
use super::*;
use crate::analysis::seams::SeamGripClass;
use crate::domain::{ExposureClass, OracleKind};
use proptest::prelude::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn semantic_config_diagnostic_locates_canonical_value_not_comment_or_other_leaf() {
    let source = "# reachable_unrevealed = \"eror\"\n[severity.seams]\nreachable_unrevealed = \"warning\"\n[severity.findings]\nreachable_unrevealed = \"eror\"\n";
    let diagnostic = parse_config_diagnostic(source).expect_err("invalid nested severity");
    assert_eq!(
        diagnostic.config_path.as_deref(),
        Some("severity.findings.reachable_unrevealed")
    );
    assert_eq!(diagnostic.location_status, ConfigLocationStatus::Exact);
    assert_eq!(
        diagnostic.location.as_ref().map(|range| (
            range.start.line,
            range.start.column,
            range.end.column
        )),
        Some((5, 24, 30))
    );
    assert_eq!(diagnostic.invalid_value.as_deref(), Some("\"eror\""));
    assert_eq!(diagnostic.expected_values, ["info", "warning", "note"]);
}

#[test]
fn semantic_config_diagnostic_preserves_crlf_quoted_key_and_inline_table() {
    let source = "# mode = \"wrong\"\r\n\"analysis\" = { \"mode\" = \"eror\" }\r\n";
    let diagnostic = parse_config_diagnostic(source).expect_err("invalid inline analysis mode");
    assert_eq!(diagnostic.config_path.as_deref(), Some("analysis.mode"));
    assert_eq!(diagnostic.location_status, ConfigLocationStatus::Exact);
    assert_eq!(
        diagnostic.location.as_ref().map(|range| (
            range.start.line,
            range.start.column,
            range.end.column
        )),
        Some((2, 25, 31))
    );
    assert_eq!(diagnostic.invalid_value.as_deref(), Some("\"eror\""));
}

#[test]
fn semantic_config_diagnostic_locates_oracle_and_admits_unavailable_fallback() {
    let source = "[oracles]\nsnapshot_strength = \"bad\"\n";
    let diagnostic = parse_config_diagnostic(source).expect_err("invalid oracle strength");
    assert_eq!(
        diagnostic.config_path.as_deref(),
        Some("oracles.snapshot_strength")
    );
    assert_eq!(
        diagnostic
            .location
            .as_ref()
            .map(|range| (range.start.line, range.start.column)),
        Some((2, 21))
    );
    assert_eq!(
        diagnostic.expected_values,
        ["strong", "medium", "weak", "smoke", "none", "unknown"]
    );

    let diagnostic = parse_config_diagnostic("[analysis]\nunknown = true\n")
        .expect_err("structural error retains the native parser message");
    assert_eq!(
        diagnostic.location_status,
        ConfigLocationStatus::Unavailable
    );
    assert!(diagnostic.message.contains("invalid ripr.toml:"));
}

#[test]
fn semantic_config_diagnostic_names_profile_expected_values() {
    let source = "[lsp]\ndiagnostic_profile = \"verbose\"\n";
    let diagnostic = parse_config_diagnostic(source).expect_err("invalid lsp diagnostic_profile");
    assert_eq!(
        diagnostic.config_path.as_deref(),
        Some("lsp.diagnostic_profile")
    );
    assert_eq!(diagnostic.location_status, ConfigLocationStatus::Exact);
    assert_eq!(diagnostic.invalid_value.as_deref(), Some("\"verbose\""));
    assert_eq!(diagnostic.expected_values, ["actionable", "full"]);
}

fn temp_root(name: &str) -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-config-{name}-{stamp}"));
    fs::create_dir_all(&root).map_err(|err| format!("create temp root failed: {err}"))?;
    Ok(root)
}

fn write_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    fs::write(path, text).map_err(|err| format!("write {} failed: {err}", path.display()))
}

#[test]
fn missing_config_uses_behavior_preserving_defaults() -> Result<(), String> {
    let root = temp_root("missing")?;
    let config = load_for_root(&root)?;

    assert!(config.source_path().is_none());
    assert!(config.analysis().mode().is_none());
    assert_eq!(config.lsp().seam_diagnostics(), Some(true));
    assert_eq!(
        config.reports().max_related_tests(),
        DEFAULT_CONTEXT_RELATED_TESTS
    );
    assert_eq!(config.languages().enabled_owned(), vec![LanguageId::Rust]);
    Ok(())
}

/// A `ripr.toml` that is a dangling or self-referencing symlink is an
/// unreadable config, not an absent one: the run must fail naming the file
/// instead of silently analyzing with defaults.
#[cfg(unix)]
#[test]
fn unresolvable_config_symlink_is_a_loud_error_not_defaults() -> Result<(), String> {
    for (label, target) in [("dangling", "no-such-target.toml"), ("loop", "ripr.toml")] {
        let root = temp_root(label)?;
        std::os::unix::fs::symlink(target, root.join("ripr.toml"))
            .map_err(|err| format!("symlink failed: {err}"))?;
        let error = match load_for_root(&root) {
            Ok(_) => return Err(format!("{label}: expected an error, got a config")),
            Err(error) => error,
        };
        assert!(error.contains("ripr.toml"), "{label}: {error}");
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn dangling_config_symlink_at_a_nearer_ancestor_is_not_skipped() -> Result<(), String> {
    let outer = temp_root("ancestor-walk")?;
    write_file(&outer.join("ripr.toml"), "")?;
    let middle = outer.join("middle");
    let nested = middle.join("nested");
    fs::create_dir_all(&nested).map_err(|err| format!("mkdir failed: {err}"))?;
    std::os::unix::fs::symlink("no-such-target.toml", middle.join("ripr.toml"))
        .map_err(|err| format!("symlink failed: {err}"))?;

    let error = match load_for_root(&nested) {
        Ok(_) => return Err("expected an error, got a config".to_string()),
        Err(error) => error,
    };

    assert!(error.contains("middle"), "{error}");
    let _ = fs::remove_dir_all(&outer);
    Ok(())
}

#[cfg(feature = "lang-python")]
#[test]
fn missing_config_detects_root_python_project_markers() -> Result<(), String> {
    for marker in PYTHON_PROJECT_MARKERS {
        let root = temp_root(&format!("python-marker-{}", marker.replace('.', "-")))?;
        write_file(&root.join(marker), "")?;

        let config = load_for_root(&root)?;

        assert!(config.source_path().is_none());
        assert_eq!(
            config.languages().enabled_owned(),
            vec![LanguageId::Rust, LanguageId::Python],
            "{marker} should enable Python preview defaults"
        );
        let _ = fs::remove_dir_all(&root);
    }
    Ok(())
}

#[cfg(feature = "lang-python")]
#[test]
fn missing_config_detects_python_source_under_src_or_tests() -> Result<(), String> {
    for source_dir in PYTHON_SOURCE_DIR_MARKERS {
        let root = temp_root(&format!("python-source-{source_dir}"))?;
        write_file(
            &root.join(source_dir).join("pricing.py"),
            "def price():\n    return 1\n",
        )?;

        let config = load_for_root(&root)?;

        assert_eq!(
            config.languages().enabled_owned(),
            vec![LanguageId::Rust, LanguageId::Python],
            "{source_dir}/ with Python files should enable Python preview defaults"
        );
        let _ = fs::remove_dir_all(&root);
    }
    Ok(())
}

#[test]
fn missing_config_does_not_treat_empty_src_or_tests_as_python() -> Result<(), String> {
    let root = temp_root("empty-python-marker-dirs")?;
    fs::create_dir_all(root.join("src")).map_err(|err| format!("create src failed: {err}"))?;
    fs::create_dir_all(root.join("tests")).map_err(|err| format!("create tests failed: {err}"))?;
    write_file(&root.join("src/lib.rs"), "pub fn price() -> u32 { 1 }\n")?;

    let config = load_for_root(&root)?;

    assert_eq!(config.languages().enabled_owned(), vec![LanguageId::Rust]);
    let _ = fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn missing_config_ignores_excluded_python_directories_and_generated_files() -> Result<(), String> {
    let root = temp_root("excluded-python-sources")?;
    for excluded_dir in PYTHON_EXCLUDED_DIRS
        .iter()
        .copied()
        .chain([PYTHON_VENDOR_DIR])
    {
        write_file(
            &root.join("src").join(excluded_dir).join("ignored.py"),
            "x = 1\n",
        )?;
    }
    write_file(&root.join("src/generated_client.py"), "x = 1\n")?;
    write_file(&root.join("tests/service_pb2.py"), "x = 1\n")?;

    let config = load_for_root(&root)?;

    assert_eq!(config.languages().enabled_owned(), vec![LanguageId::Rust]);
    let _ = fs::remove_dir_all(&root);
    Ok(())
}

#[cfg(feature = "lang-python")]
#[test]
fn explicit_config_keeps_python_preview_disabled_even_with_project_markers() -> Result<(), String> {
    let root = temp_root("explicit-rust-only-python-root")?;
    write_file(
        &root.join("pyproject.toml"),
        "[project]\nname = \"sample\"\n",
    )?;
    write_file(
        &root.join(CONFIG_FILE_NAME),
        "[languages]\nenabled = [\"rust\"]\n",
    )?;

    let config = load_for_root(&root)?;

    assert!(config.source_path().is_some());
    assert_eq!(config.languages().enabled_owned(), vec![LanguageId::Rust]);
    let _ = fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn languages_section_absent_defaults_to_rust() -> Result<(), String> {
    let config = parse_config("[analysis]\nmode = \"draft\"\n")?;
    assert_eq!(config.languages().enabled_owned(), vec![LanguageId::Rust]);
    Ok(())
}

#[test]
fn languages_section_present_with_only_rust_matches_default() -> Result<(), String> {
    let config = parse_config(
        r#"
[languages]
enabled = ["rust"]
"#,
    )?;
    assert_eq!(config.languages().enabled_owned(), vec![LanguageId::Rust]);
    Ok(())
}

#[test]
fn languages_section_accepts_custom_generated_file_patterns() -> Result<(), String> {
    let config = parse_config(
        r#"
[languages]
enabled = ["rust"]

[languages.rust]
generated_file_patterns = ["*.gen.rs", "src/generated/**/*.rs"]
"#,
    )?;

    assert_eq!(
        config.languages().rust.generated_file_patterns.as_slice(),
        &["*.gen.rs".to_string(), "src/generated/**/*.rs".to_string()]
    );
    Ok(())
}

#[test]
fn generated_file_patterns_reject_empty_duplicate_and_escape_values() {
    for (text, expected) in [
        (
            "[languages.rust]\ngenerated_file_patterns = [\"\"]\n",
            "must not be empty",
        ),
        (
            "[languages.rust]\ngenerated_file_patterns = [\"*.gen.rs\", \"*.gen.rs\"]\n",
            "more than once",
        ),
        (
            "[languages.rust]\ngenerated_file_patterns = [\"../generated/**/*.rs\"]\n",
            "must stay within the repository",
        ),
        (
            "[languages.rust]\ngenerated_file_patterns = ['src\\generated\\*.rs']\n",
            "uses backslashes",
        ),
        (
            "[languages.rust]\ngenerated_file_patterns = [\"*.gen.rs\\u000A\"]\n",
            "control characters",
        ),
    ] {
        let result = parse_config(text);
        assert!(
            matches!(result, Err(ref message) if message.contains(expected)),
            "expected {expected:?} in {result:?}"
        );
    }
}

#[cfg(all(feature = "lang-typescript", feature = "lang-python"))]
#[test]
fn languages_section_accepts_preview_adapters_in_order() -> Result<(), String> {
    let config = parse_config(
        r#"
[languages]
enabled = ["rust", "typescript", "python"]
"#,
    )?;
    assert_eq!(
        config.languages().enabled_owned(),
        vec![LanguageId::Rust, LanguageId::TypeScript, LanguageId::Python]
    );
    Ok(())
}

#[cfg(not(feature = "lang-python"))]
#[test]
fn languages_section_rejects_unavailable_python_adapter() {
    let result = parse_config(
        r#"
[languages]
enabled = ["rust", "python"]
"#,
    );
    assert!(
        matches!(result, Err(ref message) if message.contains("lang-python")),
        "expected missing lang-python error, got {result:?}"
    );
}

#[cfg(not(feature = "lang-typescript"))]
#[test]
fn languages_section_rejects_unavailable_typescript_adapter() {
    let result = parse_config(
        r#"
[languages]
enabled = ["rust", "typescript"]
"#,
    );
    assert!(
        matches!(result, Err(ref message) if message.contains("lang-typescript")),
        "expected missing lang-typescript error, got {result:?}"
    );
}

#[cfg(not(feature = "lang-perl"))]
#[test]
fn languages_section_rejects_unavailable_perl_adapter() -> Result<(), String> {
    let result = parse_config(
        r#"
[languages]
enabled = ["rust", "perl"]
"#,
    );
    let Err(message) = result.as_ref() else {
        return Err(format!("expected missing lang-perl error, got {result:?}"));
    };
    assert!(
        message.contains("lang-perl"),
        "expected missing lang-perl error, got {message}"
    );
    // The error must say how to get a working setup, not only what is
    // missing: a `lang-perl` build (with its install command) AND the
    // unpublished Perl fact exporter. Rebuilding alone is not enough.
    for required in [
        "cargo install ripr --features lang-perl",
        "`perl-ripr-facts`",
        "not yet published",
        "no released ripr setup analyzes Perl yet",
    ] {
        assert!(
            message.contains(required),
            "config error must name `{required}`; got: {message}"
        );
    }
    Ok(())
}

#[test]
fn languages_section_allows_empty_enabled_list() -> Result<(), String> {
    let config = parse_config(
        r#"
[languages]
enabled = []
"#,
    )?;
    assert!(config.languages().enabled_owned().is_empty());
    Ok(())
}

#[test]
fn languages_section_rejects_unknown_language() {
    let result = parse_config(
        r#"
[languages]
enabled = ["ruby"]
"#,
    );
    assert!(matches!(result, Err(ref message) if message.contains("ruby")));
}

#[test]
fn languages_section_rejects_duplicate_entry() {
    let result = parse_config(
        r#"
[languages]
enabled = ["rust", "rust"]
"#,
    );
    assert!(matches!(result, Err(ref message) if message.contains("more than once")));
}

#[test]
fn languages_section_rejects_unknown_field() {
    let result = parse_config(
        r#"
[languages]
enabled = ["rust"]
extra = true
"#,
    );
    assert!(
        matches!(result, Err(ref message) if message.contains("extra") || message.contains("unknown field"))
    );
}

#[test]
fn bun_ub_profile_absent_by_default() -> Result<(), String> {
    let config = parse_config("[languages]\nenabled = [\"rust\"]\n")?;
    assert!(config.profiles().bun_ub().is_none());
    Ok(())
}

#[test]
fn bun_ub_profile_parses_advisory_roots_and_bridge_hint_path() -> Result<(), String> {
    let config = parse_config(
        r#"
[languages]
enabled = ["rust"]

[profiles.bun_ub]
test_roots = [
  "test/js/**/*.test.ts",
  "test/js/**/*.test.js",
]
bridge_hints = "ripr.bun.bridge.toml"
"#,
    )?;

    assert_eq!(config.languages().enabled_owned(), vec![LanguageId::Rust]);
    let profile = config
        .profiles()
        .bun_ub()
        .ok_or_else(|| "expected Bun UB profile".to_string())?;
    assert_eq!(
        profile.test_roots(),
        &[
            "test/js/**/*.test.ts".to_string(),
            "test/js/**/*.test.js".to_string()
        ]
    );
    assert_eq!(profile.display_bridge_hints(), "ripr.bun.bridge.toml");
    Ok(())
}

#[test]
fn bun_ub_profile_rejects_missing_required_fields() {
    let missing_roots = parse_config(
        r#"
[profiles.bun_ub]
bridge_hints = "ripr.bun.bridge.toml"
"#,
    );
    assert!(
        matches!(missing_roots, Err(ref message) if message.contains("test_roots is required"))
    );

    let missing_bridge = parse_config(
        r#"
[profiles.bun_ub]
test_roots = ["test/js/**/*.test.ts"]
"#,
    );
    assert!(
        matches!(missing_bridge, Err(ref message) if message.contains("bridge_hints is required"))
    );
}

#[test]
fn bun_ub_profile_rejects_unsafe_or_ambiguous_paths() {
    let empty_roots = parse_config(
        r#"
[profiles.bun_ub]
test_roots = []
bridge_hints = "ripr.bun.bridge.toml"
"#,
    );
    assert!(matches!(empty_roots, Err(ref message) if message.contains("at least one test root")));

    let duplicate_roots = parse_config(
        r#"
[profiles.bun_ub]
test_roots = ["test/js/**/*.test.ts", "test/js/**/*.test.ts"]
bridge_hints = "ripr.bun.bridge.toml"
"#,
    );
    assert!(matches!(duplicate_roots, Err(ref message) if message.contains("more than once")));

    let unsafe_root = parse_config(
        r#"
[profiles.bun_ub]
test_roots = ["../bun/test/js/**/*.test.ts"]
bridge_hints = "ripr.bun.bridge.toml"
"#,
    );
    assert!(
        matches!(unsafe_root, Err(ref message) if message.contains("must stay within the repository"))
    );

    let unsafe_bridge = parse_config(
        r#"
[profiles.bun_ub]
test_roots = ["test/js/**/*.test.ts"]
bridge_hints = "scheme:ripr.bun.bridge.toml"
"#,
    );
    assert!(matches!(unsafe_bridge, Err(ref message) if message.contains("repository-relative")));
}

#[test]
fn bun_ub_profile_rejects_unknown_fields() {
    let result = parse_config(
        r#"
[profiles.bun_ub]
test_roots = ["test/js/**/*.test.ts"]
bridge_hints = "ripr.bun.bridge.toml"
runtime = "bun"
"#,
    );
    assert!(
        matches!(result, Err(ref message) if message.contains("runtime") || message.contains("unknown field"))
    );
}

#[test]
fn config_file_sets_core_operational_defaults() -> Result<(), String> {
    let config = parse_config(
        r#"
[analysis]
mode = "deep"
include_unchanged_tests = false

[oracles]
snapshot_strength = "strong"
mock_expectation_strength = "strong"
broad_error_strength = "medium"

[lsp]
seam_diagnostics = true

[reports]
max_related_tests = 9

[suppressions]
path = ".ripr/custom-suppressions.toml"

[severity.findings]
exposed = "note"
weakly_exposed = "info"
reachable_unrevealed = "warning"
no_static_path = "note"
infection_unknown = "info"
propagation_unknown = "warning"
static_unknown = "warning"

[severity.seams]
strongly_gripped = "off"
weakly_gripped = "warning"
ungripped = "info"
reachable_unrevealed = "note"
activation_unknown = "info"
propagation_unknown = "warning"
observation_unknown = "note"
discrimination_unknown = "info"
opaque = "note"
intentional = "off"
suppressed = "off"
    "#,
    )?;

    assert_eq!(config.analysis().mode(), Some(&Mode::Deep));
    assert_eq!(config.analysis().include_unchanged_tests(), Some(false));
    assert_eq!(
        config.oracles().snapshot_strength(),
        &OracleStrength::Strong
    );
    assert_eq!(
        config.oracles().mock_expectation_strength(),
        &OracleStrength::Strong
    );
    assert_eq!(
        config.oracles().broad_error_strength(),
        &OracleStrength::Medium
    );
    assert_eq!(config.lsp().seam_diagnostics(), Some(true));
    assert_eq!(config.reports().max_related_tests(), 9);
    assert_eq!(
        config.suppressions().display_path(),
        ".ripr/custom-suppressions.toml"
    );
    assert_eq!(
        config.severity().for_exposure(&ExposureClass::Exposed),
        ConfigSeverity::Note
    );
    assert_eq!(
        config
            .severity()
            .for_exposure(&ExposureClass::WeaklyExposed),
        ConfigSeverity::Info
    );
    assert_eq!(
        config
            .severity()
            .for_exposure(&ExposureClass::ReachableUnrevealed),
        ConfigSeverity::Warning
    );
    assert_eq!(
        config.severity().for_exposure(&ExposureClass::NoStaticPath),
        ConfigSeverity::Note
    );
    assert_eq!(
        config
            .severity()
            .for_exposure(&ExposureClass::InfectionUnknown),
        ConfigSeverity::Info
    );
    assert_eq!(
        config
            .severity()
            .for_exposure(&ExposureClass::PropagationUnknown),
        ConfigSeverity::Warning
    );
    assert_eq!(
        config
            .severity()
            .for_exposure(&ExposureClass::StaticUnknown),
        ConfigSeverity::Warning
    );
    assert_eq!(
        config.severity().for_seam(SeamGripClass::StronglyGripped),
        ConfigSeverity::Off
    );
    assert_eq!(
        config.severity().for_seam(SeamGripClass::WeaklyGripped),
        ConfigSeverity::Warning
    );
    assert_eq!(
        config.severity().for_seam(SeamGripClass::Ungripped),
        ConfigSeverity::Info
    );
    assert_eq!(
        config
            .severity()
            .for_seam(SeamGripClass::ReachableUnrevealed),
        ConfigSeverity::Note
    );
    assert_eq!(
        config.severity().for_seam(SeamGripClass::ActivationUnknown),
        ConfigSeverity::Info
    );
    assert_eq!(
        config
            .severity()
            .for_seam(SeamGripClass::PropagationUnknown),
        ConfigSeverity::Warning
    );
    assert_eq!(
        config
            .severity()
            .for_seam(SeamGripClass::ObservationUnknown),
        ConfigSeverity::Note
    );
    assert_eq!(
        config
            .severity()
            .for_seam(SeamGripClass::DiscriminationUnknown),
        ConfigSeverity::Info
    );
    assert_eq!(
        config.severity().for_seam(SeamGripClass::Opaque),
        ConfigSeverity::Note
    );
    assert_eq!(
        config.severity().for_seam(SeamGripClass::Intentional),
        ConfigSeverity::Off
    );
    assert_eq!(
        config.severity().for_seam(SeamGripClass::Suppressed),
        ConfigSeverity::Off
    );
    assert!(config.profiles().bun_ub().is_none());
    Ok(())
}

#[test]
fn generated_init_config_is_conservative_and_parseable() -> Result<(), String> {
    let config = parse_config(generated_init_config())?;

    assert_eq!(config.analysis().mode(), Some(&Mode::Draft));
    assert_eq!(config.analysis().include_unchanged_tests(), Some(true));
    assert_eq!(
        config.oracles().snapshot_strength(),
        &OracleStrength::Medium
    );
    assert_eq!(
        config.oracles().mock_expectation_strength(),
        &OracleStrength::Medium
    );
    assert_eq!(
        config.oracles().broad_error_strength(),
        &OracleStrength::Weak
    );
    assert_eq!(config.lsp().seam_diagnostics(), Some(true));
    assert_eq!(
        config.reports().max_related_tests(),
        DEFAULT_CONTEXT_RELATED_TESTS
    );
    assert_eq!(
        config.suppressions().display_path(),
        DEFAULT_SUPPRESSIONS_PATH
    );
    assert_eq!(
        config.severity().for_seam(SeamGripClass::StronglyGripped),
        ConfigSeverity::Off
    );
    assert_eq!(
        config.severity().for_seam(SeamGripClass::WeaklyGripped),
        ConfigSeverity::Warning
    );
    assert_eq!(
        config.severity().for_seam(SeamGripClass::Ungripped),
        ConfigSeverity::Warning
    );
    assert_eq!(
        config
            .severity()
            .for_seam(SeamGripClass::ReachableUnrevealed),
        ConfigSeverity::Warning
    );
    assert_eq!(
        config.severity().for_seam(SeamGripClass::Intentional),
        ConfigSeverity::Off
    );
    assert_eq!(
        config.severity().for_seam(SeamGripClass::Suppressed),
        ConfigSeverity::Off
    );
    assert!(config.profiles().bun_ub().is_none());
    Ok(())
}

#[test]
fn generated_init_config_matches_builtin_defaults() -> Result<(), String> {
    let builtin = RiprConfig::default();
    let generated = parse_config(generated_init_config())?;

    let mut builtin_input = CheckInput::default();
    apply_to_check_input(&mut builtin_input, &builtin, CheckInputExplicit::default());
    let mut generated_input = CheckInput::default();
    apply_to_check_input(
        &mut generated_input,
        &generated,
        CheckInputExplicit::default(),
    );

    assert_eq!(builtin_input.mode, generated_input.mode);
    assert_eq!(
        builtin_input.include_unchanged_tests,
        generated_input.include_unchanged_tests
    );
    assert_eq!(builtin.oracles(), generated.oracles());
    assert_eq!(builtin.lsp(), generated.lsp());
    assert_eq!(builtin.reports(), generated.reports());
    assert_eq!(builtin.suppressions(), generated.suppressions());
    assert_eq!(builtin.profiles(), generated.profiles());

    for class in [
        ExposureClass::Exposed,
        ExposureClass::WeaklyExposed,
        ExposureClass::ReachableUnrevealed,
        ExposureClass::NoStaticPath,
        ExposureClass::InfectionUnknown,
        ExposureClass::PropagationUnknown,
        ExposureClass::StaticUnknown,
    ] {
        assert_eq!(
            builtin.severity().for_exposure(&class),
            generated.severity().for_exposure(&class)
        );
    }

    for class in [
        SeamGripClass::StronglyGripped,
        SeamGripClass::WeaklyGripped,
        SeamGripClass::Ungripped,
        SeamGripClass::ReachableUnrevealed,
        SeamGripClass::ActivationUnknown,
        SeamGripClass::PropagationUnknown,
        SeamGripClass::ObservationUnknown,
        SeamGripClass::DiscriminationUnknown,
        SeamGripClass::Opaque,
        SeamGripClass::Intentional,
        SeamGripClass::Suppressed,
    ] {
        assert_eq!(
            builtin.severity().for_seam(class),
            generated.severity().for_seam(class)
        );
    }

    Ok(())
}

#[test]
fn generated_init_config_matches_checked_in_example() -> Result<(), String> {
    let example_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ripr.toml.example");
    let example = fs::read_to_string(&example_path)
        .map_err(|err| format!("read {} failed: {err}", example_path.display()))?;
    assert_eq!(generated_init_config(), example.as_str());
    Ok(())
}

#[test]
fn unknown_language_error_includes_perl_fact_packet_guidance() -> Result<(), String> {
    let Err(error) = parse_config(
        r#"
[languages]
enabled = ["ruby"]
"#,
    ) else {
        return Err("unknown language should fail configuration parsing".to_string());
    };
    if !error.contains("--perl-facts <path>") {
        return Err(format!(
            "unknown-language guidance should name the Perl fact packet option: {error}"
        ));
    }
    Ok(())
}

#[test]
fn config_file_discovery_records_source_metadata() -> Result<(), String> {
    let root = temp_root("present")?;
    let config_path = root.join(CONFIG_FILE_NAME);
    fs::write(&config_path, "[analysis]\nmode = \"fast\"\n")
        .map_err(|err| format!("write config failed: {err}"))?;

    let config = load_for_root(&root)?;
    let expected_path = fs::canonicalize(&config_path)
        .map_err(|err| format!("canonicalize config failed: {err}"))?;
    let source_path = config.source_path().map(Path::to_path_buf);
    let source_text = config.source_text().map(str::to_owned);
    let mode = config.analysis().mode().cloned();
    fs::remove_dir_all(&root).map_err(|err| format!("remove present fixture failed: {err}"))?;

    if source_path != Some(expected_path) {
        return Err(format!("unexpected source path: {source_path:?}"));
    }
    if source_text.as_deref() != Some("[analysis]\nmode = \"fast\"\n") {
        return Err(format!("unexpected source text: {source_text:?}"));
    }
    if mode != Some(Mode::Fast) {
        return Err(format!("unexpected config mode: {mode:?}"));
    }
    Ok(())
}

#[test]
fn config_file_discovery_walks_up_from_nested_root() -> Result<(), String> {
    let root = temp_root("nested")?;
    let nested = root.join("crates/member/src");
    fs::create_dir_all(&nested).map_err(|err| format!("create nested root failed: {err}"))?;
    write_file(
        &root.join(CONFIG_FILE_NAME),
        "[analysis]\nmode = \"fast\"\n",
    )?;
    write_file(
        &root.join("Cargo.toml"),
        "[package]\nname = \"nested-config\"\nversion = \"0.1.0\"\n",
    )?;

    let config = load_for_root(&nested)?;
    let source_path = config.source_path().map(Path::to_path_buf);
    let mode = config.analysis().mode().cloned();
    let expected_source_path = fs::canonicalize(root.join(CONFIG_FILE_NAME))
        .map_err(|err| format!("canonicalize discovered config failed: {err}"))?;
    fs::remove_dir_all(&root).map_err(|err| format!("remove nested fixture failed: {err}"))?;

    if source_path != Some(expected_source_path) {
        return Err(format!(
            "unexpected discovered config path: {source_path:?}"
        ));
    }
    if mode != Some(Mode::Fast) {
        return Err(format!("unexpected discovered config mode: {mode:?}"));
    }
    Ok(())
}

#[test]
fn config_file_discovery_stops_at_git_boundary() -> Result<(), String> {
    let root = temp_root("git-boundary")?;
    let boundary = root.join("isolated");
    let nested = boundary.join("src");
    fs::create_dir_all(&nested).map_err(|err| format!("create nested root failed: {err}"))?;
    fs::create_dir(boundary.join(".git"))
        .map_err(|err| format!("create git boundary failed: {err}"))?;
    write_file(
        &root.join(CONFIG_FILE_NAME),
        "[analysis]\nmode = \"fast\"\n",
    )?;

    let config = load_for_root(&nested)?;
    let source_path = config.source_path().map(Path::to_path_buf);
    fs::remove_dir_all(&root).map_err(|err| format!("remove boundary fixture failed: {err}"))?;

    if source_path.is_some() {
        return Err(format!(
            "config discovery crossed .git boundary: {source_path:?}"
        ));
    }
    Ok(())
}

#[test]
fn oracle_strength_literals_round_trip_through_config() -> Result<(), String> {
    let weak_smoke_none = parse_config(
        r#"
[oracles]
snapshot_strength = "weak"
mock_expectation_strength = "smoke"
broad_error_strength = "none"
"#,
    )?;
    assert_eq!(
        weak_smoke_none.oracles().snapshot_strength(),
        &OracleStrength::Weak
    );
    assert_eq!(
        weak_smoke_none.oracles().mock_expectation_strength(),
        &OracleStrength::Smoke
    );
    assert_eq!(
        weak_smoke_none.oracles().broad_error_strength(),
        &OracleStrength::None
    );

    let unknown = parse_config("[oracles]\nbroad_error_strength = \"unknown\"\n")?;
    assert_eq!(
        unknown.oracles().broad_error_strength(),
        &OracleStrength::Unknown
    );
    Ok(())
}

#[test]
fn explicit_cli_mode_wins_over_config_mode() -> Result<(), String> {
    let config = parse_config("[analysis]\nmode = \"deep\"\n")?;
    let mut input = CheckInput {
        mode: Mode::Instant,
        include_unchanged_tests: true,
        ..CheckInput::default()
    };
    apply_to_check_input(
        &mut input,
        &config,
        CheckInputExplicit {
            mode: true,
            include_unchanged_tests: false,
        },
    );
    assert_eq!(input.mode, Mode::Instant);
    Ok(())
}

#[test]
fn config_mode_applies_when_cli_mode_is_not_explicit() -> Result<(), String> {
    let config = parse_config("[analysis]\nmode = \"ready\"\n")?;
    let mut input = CheckInput::default();
    apply_to_check_input(&mut input, &config, CheckInputExplicit::default());
    assert_eq!(input.mode, Mode::Ready);
    Ok(())
}

#[test]
fn malformed_or_unknown_config_is_actionable() {
    let invalid_mode = parse_config("[analysis]\nmode = \"slow\"\n");
    assert!(matches!(invalid_mode, Err(message) if message.contains("analysis.mode")));

    let unknown_field = parse_config("[analysis]\nunknown = true\n");
    assert!(matches!(unknown_field, Err(message) if message.contains("unknown field")));

    let invalid_oracle = parse_config("[oracles]\nsnapshot_strength = \"mystery\"\n");
    assert!(
        matches!(invalid_oracle, Err(message) if message.contains("oracles.snapshot_strength `mystery`"))
    );

    let finding_off = parse_config("[severity.findings]\nweakly_exposed = \"off\"\n");
    assert!(matches!(finding_off, Err(message) if message.contains("use suppressions")));

    let bad_severity = parse_config("[severity.findings]\nweakly_exposed = \"loud\"\n");
    assert!(
        matches!(bad_severity, Err(message) if message.contains("severity.findings.weakly_exposed"))
    );
}

#[test]
fn config_rejects_unsafe_suppression_paths() {
    for text in [
        "[suppressions]\npath = \"\"\n".to_string(),
        "[suppressions]\npath = \"../outside.toml\"\n".to_string(),
        format!("[suppressions]\npath = \"{}tmp/suppressions.toml\"\n", '/'),
        "[suppressions]\npath = \"file:tmp/suppressions.toml\"\n".to_string(),
        "[suppressions]\npath = 'a\\b.toml'\n".to_string(),
    ] {
        assert!(
            parse_config(&text).is_err(),
            "expected invalid path for {text:?}"
        );
    }
}

#[test]
fn oracle_policy_rewrites_configurable_oracle_strengths() {
    let policy = OraclePolicy {
        snapshot_strength: OracleStrength::Strong,
        mock_expectation_strength: OracleStrength::Weak,
        broad_error_strength: OracleStrength::Medium,
    };
    assert_eq!(
        policy.strength_for_kind(&OracleKind::Snapshot, OracleStrength::Medium),
        OracleStrength::Strong
    );
    assert_eq!(
        policy.strength_for_kind(&OracleKind::MockExpectation, OracleStrength::Medium),
        OracleStrength::Weak
    );
    assert_eq!(
        policy.strength_for_kind(&OracleKind::BroadError, OracleStrength::Weak),
        OracleStrength::Medium
    );
    assert_eq!(
        policy.strength_for_kind(&OracleKind::ExactValue, OracleStrength::Strong),
        OracleStrength::Strong
    );
}

// ── Check-artifact config identity (RIPR-SPEC-0140) ──

/// The closed config-identity contract: every `ripr.toml` field is
/// classified exactly once. The producer
/// (`RiprConfig::check_artifact_identity_fields`) destructures every config
/// struct without a `..` rest pattern, so an unclassified new field fails
/// compilation; this test pins the classification itself so a silent
/// re-classification fails CI.
#[test]
fn check_artifact_identity_fields_classify_every_config_field() -> Result<(), String> {
    let fields = RiprConfig::default().check_artifact_identity_fields();
    let mut actual = fields
        .iter()
        .map(|field| (field.name, field.role))
        .collect::<Vec<_>>();
    actual.sort_by(|left, right| left.0.cmp(right.0));

    let finding_affecting = [
        "languages.rust.generated_file_patterns",
        "languages.rust.handwritten_files",
        "analysis.production_like_targets",
        "analysis.test_harnesses",
        "oracles.broad_error_strength",
        "oracles.mock_expectation_strength",
        "oracles.snapshot_strength",
        "perl.cache_dir",
        "perl.executable",
        "perl.producer",
        "perl.timeout_ms",
        "typescript.resolve_tsconfig_paths",
    ];
    let captured_elsewhere = [
        "analysis.include_unchanged_tests",
        "analysis.mode",
        "languages.enabled",
    ];
    let mut expected = finding_affecting
        .iter()
        .map(|name| (*name, ConfigIdentityRole::FindingAffecting))
        .chain(
            captured_elsewhere
                .iter()
                .map(|name| (*name, ConfigIdentityRole::CapturedElsewhere)),
        )
        .collect::<Vec<_>>();
    for name in [
        "severity.findings.exposed",
        "severity.findings.weakly_exposed",
        "severity.findings.reachable_unrevealed",
        "severity.findings.no_static_path",
        "severity.findings.infection_unknown",
        "severity.findings.propagation_unknown",
        "severity.findings.static_unknown",
        "severity.seams.strongly_gripped",
        "severity.seams.weakly_gripped",
        "severity.seams.ungripped",
        "severity.seams.reachable_unrevealed",
        "severity.seams.activation_unknown",
        "severity.seams.propagation_unknown",
        "severity.seams.observation_unknown",
        "severity.seams.discrimination_unknown",
        "severity.seams.opaque",
        "severity.seams.intentional",
        "severity.seams.suppressed",
        "lsp.seam_diagnostics",
        "lsp.diagnostic_profile",
        "reports.max_related_tests",
        "suppressions.path",
        "profiles.bun_ub",
        "source_path",
        "source_text",
    ] {
        expected.push((name, ConfigIdentityRole::Excluded));
    }
    expected.sort_by(|left, right| left.0.cmp(right.0));

    if actual != expected {
        return Err(format!(
            "config identity classification drifted\nexpected: {expected:?}\nactual:   {actual:?}"
        ));
    }

    // Every finding-affecting field carries a canonical value; every other
    // field names where it is captured or why it is excluded.
    for field in &fields {
        match field.role {
            ConfigIdentityRole::FindingAffecting => {
                if field.value.is_none() {
                    return Err(format!(
                        "finding-affecting field {} has no canonical value",
                        field.name
                    ));
                }
            }
            _ => {
                if field.note.is_empty() {
                    return Err(format!(
                        "field {} has no capture/exclusion note",
                        field.name
                    ));
                }
            }
        }
    }
    Ok(())
}

#[test]
fn check_artifact_config_identity_hash_tracks_finding_affecting_fields_only() -> Result<(), String>
{
    let base = RiprConfig::default();
    let base_hash = check_artifact_config_identity_hash(&base);

    // A finding-affecting change (oracle policy) changes the identity.
    let oracles_changed = tests_only_parse("[oracles]\nsnapshot_strength = \"strong\"\n")?;
    if check_artifact_config_identity_hash(&oracles_changed) == base_hash {
        return Err("oracle policy change must change the config identity".to_string());
    }

    // A finding-affecting change (TypeScript adapter option) changes it too.
    let ts_changed = tests_only_parse("[typescript]\nresolve_tsconfig_paths = true\n")?;
    if check_artifact_config_identity_hash(&ts_changed) == base_hash {
        return Err(
            "typescript.resolve_tsconfig_paths change must change the config identity".to_string(),
        );
    }

    // A finding-affecting change (custom Rust generated-file patterns) changes
    // which source files enter the analysis and therefore changes identity.
    let generated_patterns_changed = tests_only_parse(
        "[languages]\nenabled = [\"rust\"]\n\n[languages.rust]\ngenerated_file_patterns = [\"*.gen.rs\"]\n",
    )?;
    if check_artifact_config_identity_hash(&generated_patterns_changed) == base_hash {
        return Err(
            "languages.rust.generated_file_patterns change must change the config identity"
                .to_string(),
        );
    }

    // Rust-only configuration does not affect a Python-only analysis identity.
    let mut python_only = RiprConfig::default();
    python_only.languages.enabled = vec![LanguageId::Python];
    let mut python_only_with_rust_patterns = python_only.clone();
    python_only_with_rust_patterns
        .languages
        .rust
        .generated_file_patterns = vec!["*.gen.rs".to_string()];
    if check_artifact_config_identity_hash(&python_only_with_rust_patterns)
        != check_artifact_config_identity_hash(&python_only)
    {
        return Err(
            "Rust generated-file patterns must not change a Python-only config identity"
                .to_string(),
        );
    }

    // A render-only change (severity display) does NOT change the identity:
    // render-time knobs are honored fresh by the consuming command.
    let severity_changed = tests_only_parse("[severity.findings]\nexposed = \"warning\"\n")?;
    if check_artifact_config_identity_hash(&severity_changed) != base_hash {
        return Err("severity display is render-only and must not change the identity".to_string());
    }

    // A render-only change (reports bound) does NOT change it either.
    let reports_changed = tests_only_parse("[reports]\nmax_related_tests = 9\n")?;
    if check_artifact_config_identity_hash(&reports_changed) != base_hash {
        return Err(
            "reports.max_related_tests is render-only and must not change the identity".to_string(),
        );
    }
    Ok(())
}

// --- Property-based tests (#2751) ---
//
// The raw model is deliberately separate from the effective configuration:
// TOML parsing applies defaults and validates policy values. These properties
// therefore check both boundaries independently: serde must preserve a valid
// raw document, and the production parser must accept that document.

proptest! {
    #[test]
    fn proptest_valid_raw_configs_round_trip_and_parse(
        mode in prop::option::of(prop::sample::select(vec![
            "instant".to_string(),
            "draft".to_string(),
            "fast".to_string(),
            "deep".to_string(),
            "ready".to_string(),
        ])),
        include_unchanged_tests in prop::option::of(any::<bool>()),
        snapshot_strength in prop::option::of(prop::sample::select(valid_oracle_strengths())),
        mock_expectation_strength in prop::option::of(prop::sample::select(valid_oracle_strengths())),
        broad_error_strength in prop::option::of(prop::sample::select(valid_oracle_strengths())),
        exposed in prop::option::of(prop::sample::select(valid_finding_severities())),
        weakly_exposed in prop::option::of(prop::sample::select(valid_finding_severities())),
        strongly_gripped in prop::option::of(prop::sample::select(valid_seam_severities())),
        seam_diagnostics in prop::option::of(any::<bool>()),
        diagnostic_profile in prop::option::of(prop::sample::select(vec![
            "actionable".to_string(),
            "full".to_string(),
        ])),
        max_related_tests in any::<usize>(),
        suppression_path in valid_repository_paths(),
        resolve_tsconfig_paths in any::<bool>(),
        producer in any::<String>(),
        executable in any::<String>(),
        timeout_ms in any::<u64>(),
        cache_dir in valid_repository_paths(),
    ) {
        let raw = RawConfig {
            analysis: Some(RawAnalysisConfig {
                mode: spanned_value(mode),
                include_unchanged_tests,
                production_like_targets: None,
                test_harnesses: None,
            }),
            oracles: Some(RawOraclePolicy {
                snapshot_strength: spanned_value(snapshot_strength),
                mock_expectation_strength: spanned_value(mock_expectation_strength),
                broad_error_strength: spanned_value(broad_error_strength),
            }),
            severity: Some(RawSeverityConfig {
                findings: Some(RawFindingSeverityConfig {
                    exposed: spanned_value(exposed),
                    weakly_exposed: spanned_value(weakly_exposed),
                    ..Default::default()
                }),
                seams: Some(RawSeamSeverityConfig {
                    strongly_gripped: spanned_value(strongly_gripped),
                    ..Default::default()
                }),
            }),
            lsp: Some(RawLspConfig {
                seam_diagnostics,
                diagnostic_profile: spanned_value(diagnostic_profile),
            }),
            reports: Some(RawReportsConfig {
                max_related_tests: Some(max_related_tests),
            }),
            suppressions: Some(RawSuppressionsConfig {
                path: Some(suppression_path),
            }),
            languages: Some(RawLanguagesConfig {
                enabled: Some(vec!["rust".to_string()]),
                rust: None,
            }),
            profiles: None,
            typescript: Some(RawTypescriptConfig {
                resolve_tsconfig_paths: Some(resolve_tsconfig_paths),
            }),
            perl: Some(RawPerlConfig {
                producer: Some(producer),
                executable: Some(executable),
                timeout_ms: Some(timeout_ms),
                cache_dir: Some(cache_dir),
            }),
        };

        let serialized = match toml::to_string(&raw) {
            Ok(serialized) => serialized,
            Err(error) => {
                prop_assert!(false, "valid raw config must serialize: {error}");
                return Ok(());
            }
        };
        let reparsed = match toml::from_str::<RawConfig>(&serialized) {
            Ok(reparsed) => reparsed,
            Err(error) => {
                prop_assert!(false, "serialized raw config must deserialize: {error}");
                return Ok(());
            }
        };

        prop_assert_eq!(reparsed, raw);
        let effective = parse_config(&serialized);
        prop_assert!(
            effective.is_ok(),
            "serialized valid config must parse: {effective:?}\n{serialized}"
        );
    }
}

fn spanned_value(value: Option<String>) -> Option<toml::Spanned<String>> {
    // Raw config fields carry source spans; a round-trip fixture only needs a
    // placeholder range because `Spanned` equality compares the value alone.
    value.map(|value| toml::Spanned::new(0..value.len(), value))
}

#[test]
fn perl_cache_dir_must_stay_within_the_repository() -> Result<(), String> {
    for cache_dir in ["/etc/ripr-outside", "../outside", "target/../../x"] {
        let text = format!("[perl]\ncache_dir = \"{cache_dir}\"\n");
        let error = match tests_only_parse(&text) {
            Ok(config) => {
                return Err(format!(
                    "perl.cache_dir `{cache_dir}` escaped the repository: {:?}",
                    config.perl.cache_dir
                ));
            }
            Err(error) => error,
        };
        if !error.contains("perl.cache_dir") {
            return Err(format!("the error must name the field: {error}"));
        }
    }
    let config = tests_only_parse("[perl]\ncache_dir = \"./target/ripr/perl-facts\"\n")?;
    if config.perl.cache_dir.as_deref() != Some(std::path::Path::new("target/ripr/perl-facts")) {
        return Err(format!(
            "a relative cache dir must be kept: {:?}",
            config.perl.cache_dir
        ));
    }
    Ok(())
}

fn valid_oracle_strengths() -> Vec<String> {
    ["strong", "medium", "weak", "smoke", "none", "unknown"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn valid_finding_severities() -> Vec<String> {
    ["info", "warning", "note"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn valid_seam_severities() -> Vec<String> {
    ["off", "info", "warning", "note"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn valid_repository_paths() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        ".ripr/suppressions.toml".to_string(),
        "config/ripr.toml".to_string(),
        "fixtures/policy.toml".to_string(),
    ])
}

#[test]
fn parse_config_reads_production_like_targets() -> Result<(), String> {
    // #3283: the opt-in parses as workspace-relative paths and joins the
    // check-artifact identity as FindingAffecting.
    let config_text =
        "[analysis]\nproduction_like_targets = [\"tests/api_contract.rs\", \"benches/perf.rs\"]\n";
    let raw = toml::from_str::<RawConfig>(config_text).map_err(|error| error.to_string())?;
    let config = RiprConfig::from_raw(raw, config_text).map_err(|diagnostic| diagnostic.message)?;
    let targets = config.analysis().production_like_targets();
    assert_eq!(targets.len(), 2);
    assert!(
        targets.contains(&std::path::PathBuf::from("tests/api_contract.rs"))
            && targets.contains(&std::path::PathBuf::from("benches/perf.rs"))
    );
    let identity = config.check_artifact_identity_fields();
    let field = identity
        .iter()
        .find(|field| field.name == "analysis.production_like_targets")
        .ok_or("identity must classify the opt-in")?;
    assert_eq!(
        field.value.as_deref(),
        Some("2\u{0}benches/perf.rs\u{0}tests/api_contract.rs"),
        "the identity encoding must be injective: count + NUL separators"
    );
    Ok(())
}

#[test]
fn parse_config_rejects_absolute_production_like_target() -> Result<(), String> {
    // Fail closed: an absolute path can never match a workspace-relative
    // discovery identity, so it must be a named parse error. TOML literal
    // strings (single quotes) keep backslashes intact for the windows arm.
    let outside = if cfg!(windows) {
        concat!("C:", '\\', "outside", '\\', "target.rs")
    } else {
        "/outside/target.rs"
    };
    let text = format!("[analysis]\nproduction_like_targets = ['{outside}']\n");
    let raw = toml::from_str::<RawConfig>(&text).map_err(|error| error.to_string())?;
    match RiprConfig::from_raw(raw, &text) {
        Err(error) => assert!(
            error.message.contains("production_like_targets"),
            "error must name the field: {}",
            error.message
        ),
        Ok(_) => return Err("absolute opt-in paths must fail closed".to_string()),
    }
    Ok(())
}

#[test]
fn parse_config_normalizes_curdir_harness_targets_to_canonical_identity() -> Result<(), String> {
    // `./tests/a.rs` and `tests/a.rs` must be one target identity: the
    // canonical form drops `.` segments while keeping `/` separators, so
    // the duplicate-target guard cannot be bypassed by a `./` prefix and
    // shared `./`-prefixed path settings keep loading.
    let registrations = parse_test_harnesses(
        "[[analysis.test_harnesses]]
registration_id = 'x'
target = './tests/a.rs'
kind = 'custom_harness'
adapter = 'libtest_mimic_v1'
marker = 'libtest_mimic'
",
    )?;
    assert_eq!(
        registrations[0].target,
        std::path::PathBuf::from("tests/a.rs")
    );
    Ok(())
}

fn parse_test_harnesses(toml_text: &str) -> Result<Vec<TestHarnessRegistration>, String> {
    let raw = toml::from_str::<RawConfig>(toml_text).map_err(|error| error.to_string())?;
    RiprConfig::from_raw(raw, toml_text)
        .map(|config| config.analysis.test_harnesses)
        .map_err(|diagnostic| diagnostic.message)
}

/// The named parse error a registration config must produce; `Err` when it
/// unexpectedly parsed.
fn harness_parse_error(toml_text: &str) -> Result<String, String> {
    match parse_test_harnesses(toml_text) {
        Err(error) => Ok(error),
        Ok(_) => Err("harness registrations must fail closed".to_string()),
    }
}

#[test]
fn parse_config_reads_exact_harness_registrations() -> Result<(), String> {
    // #3532: both supported families parse with exact identities and join
    // the check-artifact identity as FindingAffecting.
    let config_text = r#"
[analysis]
[[analysis.test_harnesses]]
registration_id = "mimic-suite"
target = "tests/price_mimic.rs"
kind = "custom_harness"
adapter = "libtest_mimic_v1"
marker = "libtest_mimic"

[[analysis.test_harnesses]]
registration_id = "contract-tests"
target = "crates/pricing/tests/api.rs"
kind = "registered_attribute"
adapter = "exact_attribute_v1"
marker = "myco::contract_test"
"#;
    let registrations = parse_test_harnesses(config_text)?;
    assert_eq!(registrations.len(), 2);
    assert_eq!(registrations[0].registration_id, "mimic-suite");
    assert_eq!(
        registrations[0].target,
        std::path::PathBuf::from("tests/price_mimic.rs")
    );
    assert_eq!(registrations[0].kind, TestHarnessKind::CustomHarnessTarget);
    assert_eq!(registrations[0].adapter, TestHarnessAdapter::LibtestMimicV1);
    assert_eq!(registrations[0].marker, "libtest_mimic");
    assert_eq!(registrations[1].marker, "myco::contract_test");

    let identity = RiprConfig::from_raw(
        toml::from_str::<RawConfig>(config_text).map_err(|error| error.to_string())?,
        config_text,
    )
    .map_err(|diagnostic| diagnostic.message)?
    .check_artifact_identity_fields();
    let field = identity
        .iter()
        .find(|field| field.name == "analysis.test_harnesses")
        .ok_or("identity must classify test_harnesses")?;
    assert_eq!(field.role, ConfigIdentityRole::FindingAffecting);
    // Pin the canonical encoding itself: the value feeds artifact reuse,
    // so an accidental encoding drift must fail here (a deliberate change
    // bumps CHECK_ARTIFACT_CONFIG_IDENTITY_VERSION and this pin).
    assert_eq!(
        field.value.as_deref(),
        Some(
            "2 11:mimic-suite 20:tests/price_mimic.rs 14:custom_harness 16:libtest_mimic_v1 13:libtest_mimic  14:contract-tests 27:crates/pricing/tests/api.rs 20:registered_attribute 18:exact_attribute_v1 19:myco::contract_test "
        )
    );
    Ok(())
}

#[test]
fn parse_config_rejects_unknown_harness_adapter_version() -> Result<(), String> {
    // Unknown adapter/schema versions fail closed at parse time.
    let error = harness_parse_error(
        "[[analysis.test_harnesses]]\nregistration_id = 'x'\ntarget = 'tests/a.rs'\nkind = 'custom_harness'\nadapter = 'libtest_mimic_v9'\nmarker = 'libtest_mimic'\n",
    )?;
    assert!(
        error.contains("libtest_mimic_v9") && error.contains("unknown"),
        "the error must name the rejected value: {error}"
    );
    Ok(())
}

#[test]
fn parse_config_rejects_harness_kind_adapter_mismatch() -> Result<(), String> {
    let error = harness_parse_error(
        "[[analysis.test_harnesses]]\nregistration_id = 'x'\ntarget = 'tests/a.rs'\nkind = 'registered_attribute'\nadapter = 'libtest_mimic_v1'\nmarker = 'contract_test'\n",
    )?;
    assert!(error.contains("does not support kind"), "{error}");
    Ok(())
}

#[test]
fn parse_config_rejects_harness_root_escape_and_lookalike_marker() -> Result<(), String> {
    // Root escape fails closed through the shared relative-path rule.
    let escape_error = harness_parse_error(
        "[[analysis.test_harnesses]]\nregistration_id = 'x'\ntarget = '../outside/mimic.rs'\nkind = 'custom_harness'\nadapter = 'libtest_mimic_v1'\nmarker = 'libtest_mimic'\n",
    )?;
    assert!(escape_error.contains("target"), "{escape_error}");

    // A lookalike wildcard marker never parses: registration is exact.
    let marker_error = harness_parse_error(
        "[[analysis.test_harnesses]]\nregistration_id = 'x'\ntarget = 'tests/a.rs'\nkind = 'registered_attribute'\nadapter = 'exact_attribute_v1'\nmarker = 'myco::contract_*'\n",
    )?;
    assert!(
        marker_error.contains("exact identifier path"),
        "{marker_error}"
    );
    Ok(())
}

#[test]
fn parse_config_rejects_conflicting_harness_registrations() -> Result<(), String> {
    // Duplicate registration ids are non-clean.
    let id_error = harness_parse_error(
        "[[analysis.test_harnesses]]\nregistration_id = 'same'\ntarget = 'tests/a.rs'\nkind = 'custom_harness'\nadapter = 'libtest_mimic_v1'\nmarker = 'libtest_mimic'\n\n[[analysis.test_harnesses]]\nregistration_id = 'same'\ntarget = 'tests/b.rs'\nkind = 'custom_harness'\nadapter = 'libtest_mimic_v1'\nmarker = 'libtest_mimic'\n",
    )?;
    assert!(id_error.contains("registered twice"), "{id_error}");

    // Two registrations claiming the same target are non-clean.
    let target_error = harness_parse_error(
        "[[analysis.test_harnesses]]\nregistration_id = 'one'\ntarget = 'tests/a.rs'\nkind = 'custom_harness'\nadapter = 'libtest_mimic_v1'\nmarker = 'libtest_mimic'\n\n[[analysis.test_harnesses]]\nregistration_id = 'two'\ntarget = 'tests/a.rs'\nkind = 'custom_harness'\nadapter = 'libtest_mimic_v1'\nmarker = 'libtest_mimic'\n",
    )?;
    assert!(
        target_error.contains("claimed by two registrations"),
        "{target_error}"
    );
    Ok(())
}

#[test]
fn perl_executable_from_repo_config_needs_user_opt_in() -> Result<(), String> {
    let config =
        parse_config("[perl]\nproducer = \"perl-ripr-facts\"\nexecutable = \"./tools/x\"\n")?;
    let perl = config.perl();
    assert_eq!(perl.executable_for_opt_in(false), None);
    assert_eq!(
        perl.executable_for_opt_in(true),
        Some(Path::new("./tools/x"))
    );
    Ok(())
}

#[test]
fn a_valid_key_in_the_wrong_table_names_its_table() -> Result<(), String> {
    // #4534: a top-level `mode` is a real key in the wrong place, not a typo.
    let Err(message) = parse_config("mode = \"draft\"\n") else {
        return Err("a top-level `mode` must be refused".to_string());
    };
    assert!(
        message.contains("unknown field `mode`")
            && message.ends_with("`mode` is a valid key, but it belongs under [analysis]"),
        "expected the [analysis] hint, got {message}"
    );
    let Err(message) = parse_config("[analysis]\nmax_related_tests = 3\n") else {
        return Err("`max_related_tests` under [analysis] must be refused".to_string());
    };
    assert!(
        message.ends_with("belongs under [reports]"),
        "expected the [reports] hint, got {message}"
    );
    Ok(())
}

#[test]
fn a_config_error_summary_keeps_the_table_hint_but_no_source_text() -> Result<(), String> {
    // #4532/#4534: doctor JSON and editor notices carry the summary, which
    // must keep the allowlisted hint and drop the parser's source excerpt.
    let Err(message) = parse_config("secret_marker_mode = 1\nmode = \"draft\"\n") else {
        return Err("a top-level `mode` must be refused".to_string());
    };
    let summary = config_error_summary(&message);
    assert!(
        !summary.contains('\n') && !summary.contains("secret_marker"),
        "summary must be one source-free line, got {summary}"
    );
    assert!(
        summary.ends_with("; `mode` is a valid key, but it belongs under [analysis]"),
        "summary must keep the hint, got {summary}"
    );
    // A line that only looks like a hint is file content and is dropped.
    let forged = "TOML parse error\n`secret` is a valid key, but it belongs under [analysis]";
    assert_eq!(config_error_summary(forged), "TOML parse error");
    Ok(())
}

#[test]
fn an_unknown_key_gets_no_table_hint() -> Result<(), String> {
    // The hint names a table only for real keys; a typo or a key shared by
    // two tables keeps the plain serde error.
    for text in [
        "[analysis]\nmdoe = \"draft\"\n",
        "reachable_unrevealed = \"warning\"\n",
    ] {
        let Err(message) = parse_config(text) else {
            return Err(format!("`{text}` must be refused"));
        };
        assert!(
            !message.contains("belongs under"),
            "no table hint expected for `{text}`, got {message}"
        );
    }
    Ok(())
}

#[test]
fn an_invalid_oracle_strength_names_its_key() -> Result<(), String> {
    let Err(message) = parse_config("[oracles]\nbroad_error_strength = \"mega\"\n") else {
        return Err("an unknown strength must be refused".to_string());
    };
    assert!(
        message.starts_with("oracles.broad_error_strength `mega` is not supported"),
        "expected the key in the error, got {message}"
    );
    Ok(())
}

#[test]
fn an_unknown_language_error_names_only_the_repair() -> Result<(), String> {
    let Err(message) = parse_config("[languages]\nenabled = [\"cobol\"]\n") else {
        return Err("an unknown language must be refused".to_string());
    };
    assert!(
        message.contains("unknown language `cobol`") && !message.contains("Campaign"),
        "expected no internal tracking reference, got {message}"
    );
    Ok(())
}

#[test]
fn bytes_fingerprint_matches_text_fingerprint_and_keeps_invalid_bytes_distinct() {
    // Recorded check artifacts hash `--diff` input bytes; UTF-8 input keeps
    // the hash it had as text, and two invalid bytes never share one.
    assert_eq!(
        bytes_fingerprint("-a\n+b\n".as_bytes()),
        config_fingerprint("-a\n+b\n")
    );
    assert_ne!(
        bytes_fingerprint(b"+caf\x80\n"),
        bytes_fingerprint(b"+caf\x81\n")
    );
}

#[test]
fn rust_handwritten_files_accepts_scoped_recovery_configuration() -> Result<(), String> {
    let config = parse_config(
        "[languages.rust]\nhandwritten_files = ['tests/./generated_weight_tests.rs']\n",
    )?;
    assert_eq!(
        config.languages.rust.handwritten_files,
        ["tests/generated_weight_tests.rs"]
    );
    Ok(())
}

#[test]
fn handwritten_files_rejects_ambiguous_or_escaping_paths() {
    // Construct the synthetic drive prefix as in the local-context checker's
    // own negative fixtures; this is not a real machine path in source.
    let drive = "C";
    let drive_path = format!("['{drive}:/schema.rs']");
    for paths in [
        "['']",
        "['.']",
        "['../schema.rs']",
        "['/tmp/schema.rs']",
        drive_path.as_str(),
        "['src\\schema.rs']",
        "['src/*.rs']",
        "['src/file?.rs']",
        "['src/[a].rs']",
        "['src/schema.txt']",
        "['src/schema.rs', './src//schema.rs']",
        r#"["src/schema\u000A.rs"]"#,
    ] {
        let result = parse_config(&format!("[languages.rust]\nhandwritten_files = {paths}\n"));
        assert!(
            result.is_err(),
            "invalid exact-path declaration was accepted: {paths}"
        );
    }
}

#[test]
fn handwritten_files_identities_follow_semantic_paths_and_consumed_language() -> Result<(), String>
{
    let default = RiprConfig::default();
    let first = parse_config(
        "[languages.rust]\nhandwritten_files = ['src/./schema.rs', 'tests/generated_a.rs']\n",
    )?;
    let reordered = parse_config(
        "[languages.rust]\nhandwritten_files = ['tests/generated_a.rs', './src//schema.rs']\n",
    )?;
    assert_eq!(
        check_artifact_config_identity_hash(&first),
        check_artifact_config_identity_hash(&reordered)
    );
    assert_eq!(
        repo_exposure_config_identity_hash(&first),
        repo_exposure_config_identity_hash(&reordered)
    );
    assert_ne!(
        check_artifact_config_identity_hash(&default),
        check_artifact_config_identity_hash(&first)
    );
    assert_ne!(
        repo_exposure_config_identity_hash(&default),
        repo_exposure_config_identity_hash(&first)
    );
    let mut python = default.clone();
    python.languages.enabled = vec![LanguageId::Python];
    let mut python_with_rust = first.clone();
    python_with_rust.languages.enabled = vec![LanguageId::Python];
    assert_eq!(
        check_artifact_config_identity_hash(&python),
        check_artifact_config_identity_hash(&python_with_rust)
    );
    // Repo exposure remains Rust-only even when diff adapter selection is Python.
    assert_eq!(
        repo_exposure_config_identity_hash(&first),
        repo_exposure_config_identity_hash(&python_with_rust)
    );
    assert_ne!(
        repo_exposure_config_identity_hash(&python),
        repo_exposure_config_identity_hash(&python_with_rust)
    );
    Ok(())
}
