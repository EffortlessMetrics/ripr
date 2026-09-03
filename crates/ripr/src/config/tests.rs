use super::python::PYTHON_PROJECT_EXCLUDED_DIRS;
#[cfg(feature = "lang-python")]
use super::python::{PYTHON_PROJECT_MARKERS, PYTHON_SOURCE_DIR_MARKERS};
use super::*;
use crate::analysis::seams::SeamGripClass;
use crate::domain::{ExposureClass, OracleKind};
use proptest::prelude::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(name: &str) -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-config-{name}-{stamp}"));
    fs::create_dir_all(&root).map_err(|err| format!("create temp root failed: {err}"))?;
    fs::create_dir(root.join(".git"))
        .map_err(|err| format!("create test boundary failed: {err}"))?;
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
    for excluded_dir in PYTHON_PROJECT_EXCLUDED_DIRS {
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
        config.languages().generated_file_patterns(),
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
fn languages_section_rejects_unavailable_perl_adapter() {
    let result = parse_config(
        r#"
[languages]
enabled = ["rust", "perl"]
"#,
    );
    assert!(
        matches!(result, Err(ref message) if message.contains("lang-perl")),
        "expected missing lang-perl error, got {result:?}"
    );
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
    assert!(matches!(invalid_oracle, Err(message) if message.contains("oracle strength")));

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
        cache_dir in any::<String>(),
    ) {
        let raw = RawConfig {
            analysis: Some(RawAnalysisConfig {
                mode,
                include_unchanged_tests,
            }),
            oracles: Some(RawOraclePolicy {
                snapshot_strength,
                mock_expectation_strength,
                broad_error_strength,
            }),
            severity: Some(RawSeverityConfig {
                findings: Some(RawFindingSeverityConfig {
                    exposed,
                    weakly_exposed,
                    ..Default::default()
                }),
                seams: Some(RawSeamSeverityConfig {
                    strongly_gripped,
                    ..Default::default()
                }),
            }),
            lsp: Some(RawLspConfig {
                seam_diagnostics,
                diagnostic_profile,
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
