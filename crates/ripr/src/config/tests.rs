use super::python::PYTHON_PROJECT_EXCLUDED_DIRS;
#[cfg(feature = "lang-python")]
use super::python::{PYTHON_PROJECT_MARKERS, PYTHON_SOURCE_DIR_MARKERS};
use super::*;
use crate::analysis::seams::SeamGripClass;
use crate::domain::{ExposureClass, OracleKind};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

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
fn config_file_discovery_records_source_metadata() -> Result<(), String> {
    let root = temp_root("present")?;
    let config_path = root.join(CONFIG_FILE_NAME);
    fs::write(&config_path, "[analysis]\nmode = \"fast\"\n")
        .map_err(|err| format!("write config failed: {err}"))?;

    let config = load_for_root(&root)?;

    assert_eq!(config.source_path(), Some(config_path.as_path()));
    assert_eq!(config.source_text(), Some("[analysis]\nmode = \"fast\"\n"));
    assert_eq!(config.analysis().mode(), Some(&Mode::Fast));
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
