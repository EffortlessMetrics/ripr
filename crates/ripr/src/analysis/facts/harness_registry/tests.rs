//! Discriminating tests for the repository-governed harness registry
//! (#3532): one per required fixture family, each pinning a boundary the
//! issue names — exact subjects, inert attributes, dynamic limitations,
//! lookalike and ambiguous controls, and stale/wrong-target fail-closed
//! behavior.

use super::*;
use crate::analysis::facts::build_index_with_test_harnesses;
use crate::analysis::facts::model::FileFacts;
use crate::domain::{OracleKind, OracleStrength};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp_dir(name: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("ripr-harness-{name}-{stamp}"));
    fs::create_dir_all(&root)?;
    Ok(TempDir(root))
}

fn registration(
    registration_id: &str,
    target: &str,
    kind: TestHarnessKind,
    adapter: TestHarnessAdapter,
    marker: &str,
) -> TestHarnessRegistration {
    TestHarnessRegistration {
        registration_id: registration_id.to_string(),
        target: PathBuf::from(target),
        kind,
        adapter,
        marker: marker.to_string(),
    }
}

fn custom_target_registration(target: &str) -> TestHarnessRegistration {
    registration(
        "mimic-suite",
        target,
        TestHarnessKind::CustomHarnessTarget,
        TestHarnessAdapter::LibtestMimicV1,
        "libtest_mimic",
    )
}

fn attribute_registration(target: &str, marker: &str) -> TestHarnessRegistration {
    registration(
        "contract-tests",
        target,
        TestHarnessKind::RegisteredAttribute,
        TestHarnessAdapter::ExactAttributeV1,
        marker,
    )
}

/// Append a standard libtest-mimic run entry (#3636) so a fixture's
/// trials resolve into the run argument and stay in the
/// executable-test denominator. Appending at the end keeps every
/// existing source-line assertion exact; `trials_expr` is the
/// expression the appended main binds the trial collection from. A
/// bare-identifier call resolves as a one-level builder; anything the
/// bounded resolver cannot follow (e.g. a method call) leaves the
/// subject admitted as reachability-unknown with the aggregate
/// disclosure.
fn with_harness_main(source: &str, trials_expr: &str) -> String {
    format!(
        "{source}
fn main() {{
    let arguments = libtest_mimic::Arguments::from_args();
    let all_trials = {trials_expr};
    libtest_mimic::run(&arguments, &all_trials);
}}
"
    )
}

fn write_workspace(
    root: &TempDir,
    files: &[(&str, &str)],
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(root.0.join("src"))?;
    fs::create_dir_all(root.0.join("tests"))?;
    // The empty [workspace] table makes the fixture a standalone workspace
    // root (#3634): the harness validation probes `cargo metadata`, and a
    // bare package whose manifest sits inside an enclosing workspace —
    // e.g. a host temp dir under this repo's target/ — would be rejected
    // by cargo as "believes it's in a workspace when it's not".
    fs::write(
        root.0.join("Cargo.toml"),
        "[package]\nname = 'harness-fixture'\nversion = '0.1.0'\nedition = '2024'\n\n[workspace]\n",
    )?;
    for (path, source) in files {
        let full = root.0.join(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full, source)?;
    }
    Ok(())
}

/// Declare a `[[test]]` target with `harness = false` in the workspace
/// manifest (#3608): the parsed Cargo metadata a valid custom-harness
/// registration must be able to point at. A registration whose target is
/// not declared this way records a conflict instead of granting
/// file-wide evidence role.
fn declare_harness_false_target(
    root: &TempDir,
    name: &str,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let mut manifest = std::fs::OpenOptions::new()
        .append(true)
        .open(root.0.join("Cargo.toml"))?;
    writeln!(
        manifest,
        "\n[[test]]\nname = '{name}'\npath = '{path}'\nharness = false"
    )?;
    Ok(())
}

#[test]
fn without_registrations_the_index_is_unchanged() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("no-registrations")?;
    write_workspace(
        &root,
        &[("src/lib.rs", "pub fn production_control() -> i32 { 1 }\n")],
    )?;
    let plain = build_index_with_test_harnesses(&root.0, &[PathBuf::from("src/lib.rs")], &[])?;
    assert!(plain.harness_subjects.is_empty());
    assert!(plain.harness_limitations.is_empty());
    Ok(())
}

#[test]
fn harness_false_target_is_evidence_role_with_exact_trial_subjects()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("mimic-target")?;
    write_workspace(
        &root,
        &[
            (
                "src/lib.rs",
                "pub fn price(amount: i32) -> i32 {\n    amount * 2\n}\n",
            ),
            (
                "tests/price_mimic.rs",
                &with_harness_main(
                    r#"
use libtest_mimic::{Arguments, Trial};

fn parse_args(args: &[String]) -> usize {
    args.len()
}

fn check_beta() -> Result<(), String> {
    Ok(())
}

fn static_trials() -> Vec<Trial> {
    vec![
        Trial::test("alpha_parses", || Ok(())),
        Trial::test("beta_round_trips", check_beta),
    ]
}

fn collected(name: &str) -> Trial {
    Trial::test(name, || Ok(()))
}

fn dynamic_trials(count: usize) -> Vec<Trial> {
    let mut trials = Vec::new();
    for index in 0..count {
        trials.push(Trial::test(format!("case_{index}"), || Ok(())));
    }
    trials
}

#[test]
fn ordinary_test_attribute_is_inert_without_the_harness() {
    assert_eq!(parse_args(&[]), 0);
}
"#,
                    "static_trials()",
                ),
            ),
            (
                "tests/ordinary.rs",
                "#[test]\nfn ordinary_libtest_runs_here() {\n    assert_eq!(1, 1);\n}\n",
            ),
        ],
    )?;
    let files = [
        PathBuf::from("src/lib.rs"),
        PathBuf::from("tests/price_mimic.rs"),
        PathBuf::from("tests/ordinary.rs"),
    ];
    declare_harness_false_target(&root, "price_mimic", "tests/price_mimic.rs")?;
    let registrations = [custom_target_registration("tests/price_mimic.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    // Exact static trial names are the executable subjects.
    let subject_names = index
        .harness_subjects
        .iter()
        .map(|subject| subject.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(subject_names, vec!["alpha_parses", "beta_round_trips"]);
    for subject in &index.harness_subjects {
        assert_eq!(subject.harness_kind, "custom_harness");
        assert_eq!(subject.adapter, "libtest_mimic_v1");
        assert_eq!(subject.marker, "libtest_mimic");
        assert_eq!(subject.file.as_path(), Path::new("tests/price_mimic.rs"));
        assert_eq!(
            subject.selector,
            HarnessSelectorCapability::NamedUnexecuted,
            "the trial name is a selector candidate, represented as unexecuted"
        );
        assert_eq!(subject.claim, HarnessSubjectClaim::NamedInvocation);
        assert_eq!(subject.provenance, "ripr.toml [analysis.test_harnesses]");
    }
    assert!(
        index
            .harness_subjects
            .iter()
            .find(|subject| subject.name == "beta_round_trips")
            .is_some_and(|subject| !subject.body.is_empty() && subject.body.contains("check_beta")),
        "the invocation body is retained so oracle evidence stays observable"
    );

    // The subjects enter the executable-test denominator.
    let test_names = index
        .tests
        .iter()
        .filter(|test| test.file.as_path() == Path::new("tests/price_mimic.rs"))
        .map(|test| test.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(test_names, vec!["alpha_parses", "beta_round_trips"]);

    // The inert `#[test]` attribute does not make the fn a test here:
    // the whole registered target is helper-evidence only.
    let inert = index
        .functions
        .iter()
        .find(|function| function.name == "ordinary_test_attribute_is_inert_without_the_harness")
        .ok_or("missing inert attribute fn")?;
    assert_eq!(inert.source_role, FunctionSourceRole::HarnessHelper);
    assert!(
        index
            .tests
            .iter()
            .all(|test| test.name != "ordinary_test_attribute_is_inert_without_the_harness"),
        "inert attributes must not enter the executable-test denominator"
    );

    // Dynamic shapes remain typed limitations; the run-resolved trials
    // are admitted with no reachability limitation recorded (#3636).
    let codes = index
        .harness_limitations
        .iter()
        .map(|limitation| limitation.code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"dynamic_trial_name"), "{codes:?}");
    assert!(codes.contains(&"dynamic_trial_registration"), "{codes:?}");
    assert!(
        !codes.contains(&"registration_unreachable")
            && !codes.contains(&"registration_reachability_unknown"),
        "run-resolved trials must not carry reachability limitations: {codes:?}"
    );

    // The ordinary libtest target beside the custom target keeps its
    // executable test.
    assert!(
        index
            .tests
            .iter()
            .any(|test| test.name == "ordinary_libtest_runs_here"),
        "an ordinary libtest target beside the custom target is unaffected"
    );
    let ordinary = index
        .functions
        .iter()
        .find(|function| function.name == "ordinary_libtest_runs_here")
        .ok_or("missing ordinary test fn")?;
    assert_eq!(ordinary.source_role, FunctionSourceRole::TestAttribute);

    // Same function name in production and harness files: the production
    // one stays a production subject.
    let production = index
        .functions
        .iter()
        .find(|function| {
            function.name == "price" && function.file.as_path() == Path::new("src/lib.rs")
        })
        .ok_or("missing production fn")?;
    assert_eq!(production.source_role, FunctionSourceRole::Production);
    Ok(())
}

#[test]
fn unanchored_and_ambiguous_trial_paths_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("trial-anchors")?;
    write_workspace(
        &root,
        &[(
            "tests/unanchored.rs",
            r#"
use other_crate::Trial;

fn trials() -> Vec<Trial> {
    vec![Trial::test("unanchored_case", || Ok(()))]
}
"#,
        )],
    )?;
    let files = [PathBuf::from("tests/unanchored.rs")];
    declare_harness_false_target(&root, "unanchored", "tests/unanchored.rs")?;
    let registrations = [custom_target_registration("tests/unanchored.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert!(index.harness_subjects.is_empty());
    assert!(
        index
            .harness_limitations
            .iter()
            .any(|limitation| limitation.code == "unanchored_trial_path"),
        "{:?}",
        index.harness_limitations
    );

    // Same-named imports from two paths are non-clean and named.
    let root = temp_dir("trial-ambiguous")?;
    write_workspace(
        &root,
        &[(
            "tests/ambiguous.rs",
            r#"
use libtest_mimic::Trial;
use other_crate::Trial;

fn trials() -> Vec<Trial> {
    vec![Trial::test("ambiguous_case", || Ok(()))]
}
"#,
        )],
    )?;
    let files = [PathBuf::from("tests/ambiguous.rs")];
    declare_harness_false_target(&root, "ambiguous", "tests/ambiguous.rs")?;
    let registrations = [custom_target_registration("tests/ambiguous.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert!(index.harness_subjects.is_empty());
    let ambiguous = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "ambiguous_import")
        .ok_or("missing ambiguous-import limitation")?;
    assert!(ambiguous.detail.contains("conflicting imports bind Trial"));
    Ok(())
}

#[test]
fn duplicate_trial_names_name_the_conflict_and_stay_one_subject()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("duplicate-trials")?;
    write_workspace(
        &root,
        &[(
            "tests/duplicates.rs",
            r#"
use libtest_mimic::Trial;

fn trials() -> Vec<Trial> {
    vec![
        Trial::test("same_name", || Ok(())),
        Trial::test("same_name", || Ok(())),
    ]
}
"#,
        )],
    )?;
    let files = [PathBuf::from("tests/duplicates.rs")];
    declare_harness_false_target(&root, "duplicates", "tests/duplicates.rs")?;
    let registrations = [custom_target_registration("tests/duplicates.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert_eq!(index.harness_subjects.len(), 1);
    assert!(
        index
            .harness_limitations
            .iter()
            .any(|limitation| limitation.code == "duplicate_subject"
                && limitation.detail.contains("same_name")),
        "{:?}",
        index.harness_limitations
    );
    Ok(())
}

#[test]
fn fully_qualified_trial_calls_match_the_marker_path() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("qualified-trials")?;
    write_workspace(
        &root,
        &[(
            "tests/qualified.rs",
            &with_harness_main(
                "fn trials() -> Vec<libtest_mimic::Trial> {\n    vec![libtest_mimic::Trial::test(\"qualified_case\", || Ok(()))]\n}\n",
                "trials()",
            ),
        )],
    )?;
    let files = [PathBuf::from("tests/qualified.rs")];
    declare_harness_false_target(&root, "qualified", "tests/qualified.rs")?;
    let registrations = [custom_target_registration("tests/qualified.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert_eq!(
        index
            .harness_subjects
            .iter()
            .map(|subject| subject.name.as_str())
            .collect::<Vec<_>>(),
        vec!["qualified_case"]
    );
    // The token-local bare pattern re-matches the inner `Trial` of the
    // qualified path; claiming the invocation must leave no false
    // unanchored/duplicate limitation behind.
    assert!(
        index.harness_limitations.is_empty(),
        "{:?}",
        index.harness_limitations
    );
    Ok(())
}

#[test]
fn qualified_trial_with_bare_import_emits_one_subject_and_no_false_limitation()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("qualified-with-import")?;
    write_workspace(
        &root,
        &[(
            "tests/both_forms.rs",
            &with_harness_main(
                "use libtest_mimic::Trial;\n\nfn trials() -> Vec<Trial> {\n    vec![libtest_mimic::Trial::test(\"both_case\", || Ok(()))]\n}\n",
                "trials()",
            ),
        )],
    )?;
    let files = [PathBuf::from("tests/both_forms.rs")];
    declare_harness_false_target(&root, "both_forms", "tests/both_forms.rs")?;
    let registrations = [custom_target_registration("tests/both_forms.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert_eq!(
        index
            .harness_subjects
            .iter()
            .map(|subject| subject.name.as_str())
            .collect::<Vec<_>>(),
        vec!["both_case"]
    );
    assert!(
        index.harness_limitations.is_empty(),
        "{:?}",
        index.harness_limitations
    );
    Ok(())
}

#[test]
fn plain_idents_inside_trial_bodies_never_become_oracles() -> Result<(), Box<dyn std::error::Error>>
{
    let root = temp_dir("trial-ident-noise")?;
    write_workspace(
        &root,
        &[(
            "tests/ident_noise.rs",
            &with_harness_main(
                "fn trials() -> Vec<libtest_mimic::Trial> {\n    vec![libtest_mimic::Trial::test(\"noise_case\", || {\n        let snapshots = load();\n        assert_eq!(snapshots, 1);\n        Ok(())\n    })]\n}\nfn load() -> i32 {\n    1\n}\n",
                "trials()",
            ),
        )],
    )?;
    let files = [PathBuf::from("tests/ident_noise.rs")];
    declare_harness_false_target(&root, "ident_noise", "tests/ident_noise.rs")?;
    let registrations = [custom_target_registration("tests/ident_noise.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    let subject_test = index
        .tests
        .iter()
        .find(|test| test.name == "noise_case")
        .ok_or("subject test present")?;
    assert_eq!(
        subject_test
            .assertions
            .iter()
            .map(|assertion| assertion.text.as_str())
            .collect::<Vec<_>>(),
        // One macro-shaped oracle with the invocation's real text (the
        // same-line trailing semicolon included, ordinary-parser parity);
        // the `let snapshots` ident never classifies.
        vec!["assert_eq!(snapshots, 1);"]
    );
    Ok(())
}

#[test]
fn snapshot_named_helpers_never_become_oracles_but_snapshot_asserts_do()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("leaf-snapshot-boundary")?;
    write_workspace(
        &root,
        &[(
            "tests/leaf_boundary.rs",
            &with_harness_main(
                "fn trials() -> Vec<libtest_mimic::Trial> {
    vec![libtest_mimic::Trial::test(\"leaf_case\", || {
        snapshot_helper!();
        assert_snapshot!(1);
        Ok(())
    })]
}
",
                "trials()",
            ),
        )],
    )?;
    let files = [PathBuf::from("tests/leaf_boundary.rs")];
    declare_harness_false_target(&root, "leaf_boundary", "tests/leaf_boundary.rs")?;
    let registrations = [custom_target_registration("tests/leaf_boundary.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    let subject_test = index
        .tests
        .iter()
        .find(|test| test.name == "leaf_case")
        .ok_or("subject test present")?;
    assert_eq!(
        subject_test
            .assertions
            .iter()
            .map(|assertion| assertion.text.as_str())
            .collect::<Vec<_>>(),
        // The *_snapshot naming boundary keeps helper-style macros out
        // while the assert_snapshot family still classifies (trailing
        // semicolon included, ordinary-parser parity).
        vec!["assert_snapshot!(1);"]
    );
    Ok(())
}

#[test]
fn helper_callback_bodies_contribute_one_level_of_subject_evidence()
-> Result<(), Box<dyn std::error::Error>> {
    // #3603 gap 1: a trial registered as `Trial::test("name", helper_fn)`
    // exercises the helper's body. The subject's claimed span stays the
    // registration invocation, but its evidence (calls, oracles,
    // literals) must be derived from the resolved helper body one level
    // deep, with real line attribution — otherwise the subject
    // understates observation relative to an ordinary `#[test]` that
    // calls the same helper.
    let root = temp_dir("helper-callback-body")?;
    write_workspace(
        &root,
        &[(
            "tests/helper_body.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn parse_config(raw: &str) -> u16 {
    raw.parse().unwrap_or(0)
}

fn check_beta() -> Result<(), String> {
    let port = parse_config("8080").unwrap();
    assert_eq!(port, 8080);
    Ok(())
}

fn trials() -> Vec<Trial> {
    vec![
        Trial::test("beta_round_trips", check_beta),
        Trial::test("gamma_foreign", other_crate::check),
    ]
}
"#,
                "trials()",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "helper_body", "tests/helper_body.rs")?;
    let files = [PathBuf::from("tests/helper_body.rs")];
    let registrations = [custom_target_registration("tests/helper_body.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    let beta = index
        .harness_subjects
        .iter()
        .find(|subject| subject.name == "beta_round_trips")
        .ok_or("beta subject missing")?;
    // The claimed identity span stays the registration invocation: the
    // helper widens the evidence, not the subject span or body.
    assert_eq!(beta.start_line, 16, "{:?}", beta.start_line);
    assert_eq!(beta.end_line, 16, "{:?}", beta.end_line);
    assert!(
        !beta.body.contains("parse_config"),
        "the subject body stays the invocation span: {:?}",
        beta.body
    );

    // Calls observed in the helper body join the subject with real lines.
    let beta_calls = beta
        .calls
        .iter()
        .map(|call| (call.line, call.name.as_str()))
        .collect::<Vec<_>>();
    assert!(
        beta_calls.contains(&(9, "parse_config")),
        "the helper body's call is subject evidence: {beta_calls:?}"
    );

    // One level deep only: the transitive callee's body (parse_config,
    // lines 4-6) stays unclaimed — its internals are not code the trial
    // subject's own callback text exercises directly.
    assert!(
        beta_calls.iter().all(|(line, _)| *line >= 8),
        "transitive helper bodies stay unclaimed: {beta_calls:?}"
    );

    // Oracles from the helper body carry the helper's real source lines:
    // the helper's own method unwrap (gap 2, helper path) and its macro
    // assertion.
    let unwrap_oracle = beta
        .assertions
        .iter()
        .find(|oracle| oracle.text.contains(".unwrap()"))
        .ok_or("helper unwrap oracle missing")?;
    assert_eq!(unwrap_oracle.line, 9, "{:?}", unwrap_oracle.line);
    assert_eq!(unwrap_oracle.kind, OracleKind::SmokeOnly);
    assert_eq!(unwrap_oracle.strength, OracleStrength::Smoke);
    let assert_eq = beta
        .assertions
        .iter()
        .find(|oracle| oracle.text.starts_with("assert_eq!"))
        .ok_or("helper assert_eq oracle missing")?;
    assert_eq!(assert_eq.line, 10, "{:?}", assert_eq.line);
    // Helper-body literals join the subject evidence.
    assert!(
        beta.literals
            .iter()
            .any(|literal| literal.value == "8080" && literal.line == 10),
        "{:?}",
        beta.literals
    );

    // Fail-closed control: a path callback (`other_crate::check`) is not a
    // bare identifier bound in this file, so nothing is invented for it.
    let gamma = index
        .harness_subjects
        .iter()
        .find(|subject| subject.name == "gamma_foreign")
        .ok_or("gamma subject missing")?;
    assert!(
        gamma.calls.iter().all(|call| call.name != "check"),
        "unresolved callbacks contribute no invented evidence: {:?}",
        gamma.calls
    );
    assert!(gamma.assertions.is_empty(), "{:?}", gamma.assertions);

    // The mirrored TestFact carries the same widened evidence so every
    // existing test consumer sees the subject the same way.
    let beta_test = index
        .tests
        .iter()
        .find(|test| test.name == "beta_round_trips")
        .ok_or("beta test missing")?;
    assert!(
        beta_test
            .calls
            .iter()
            .any(|call| call.name == "parse_config"),
        "{:?}",
        beta_test.calls
    );
    assert!(
        beta_test
            .assertions
            .iter()
            .any(|oracle| oracle.text.starts_with("assert_eq!") && oracle.line == 10),
        "{:?}",
        beta_test.assertions
    );
    Ok(())
}

#[test]
fn shadowed_callback_names_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    // #3603 review: a bare-identifier callback may name a local binding
    // (let/parameter/closure), an import, or a fn that is not even
    // name-visible from the invocation — crediting a same-named
    // file-level fn's body in those cases would credit evidence the
    // trial cannot reach (false exposed). Every such shape fails closed:
    // the trial subject stays, but none of the same-named fn's body
    // evidence is claimed.
    let root = temp_dir("shadowed-callbacks")?;
    write_workspace(
        &root,
        &[(
            "tests/shadowed.rs",
            r#"
use libtest_mimic::Trial;
use other_crate::imported_helper;

fn imported_helper() -> u16 {
    production_call(1)
}

fn closure_shadow_trials() -> Vec<Trial> {
    let shadow_target = || 1u16;
    vec![Trial::test("closure_shadow", shadow_target)]
}

fn param_shadow_trials(shadow_target: fn() -> u16) -> Vec<Trial> {
    vec![Trial::test("param_shadow", shadow_target)]
}

fn nested_mod_shadow_trials() -> Vec<Trial> {
    vec![Trial::test("nested_mod_shadow", nested_helper)]
}

fn import_shadow_trials() -> Vec<Trial> {
    vec![Trial::test("import_shadow", imported_helper)]
}

mod helpers {
    pub fn nested_helper() -> u16 {
        production_call(3)
    }
}

fn shadow_target() -> u16 {
    production_call(4)
}
"#,
        )],
    )?;
    declare_harness_false_target(&root, "shadowed", "tests/shadowed.rs")?;
    let files = [PathBuf::from("tests/shadowed.rs")];
    let registrations = [custom_target_registration("tests/shadowed.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    // The clean file-level fn `shadow_target` exists with distinctive
    // body evidence; every negative subject must claim none of it.
    let credited_evidence = |subject_name: &str| -> bool {
        index
            .harness_subjects
            .iter()
            .find(|subject| subject.name == subject_name)
            .is_some_and(|subject| {
                subject
                    .calls
                    .iter()
                    .any(|call| call.name == "production_call")
                    || subject
                        .assertions
                        .iter()
                        .any(|oracle| oracle.text.contains("production_call"))
            })
    };

    // Every subject still classifies; fail-closed means uncredited
    // evidence, not a missing subject.
    assert_eq!(
        index
            .harness_subjects
            .iter()
            .map(|subject| subject.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "closure_shadow",
            "param_shadow",
            "nested_mod_shadow",
            "import_shadow"
        ]
    );
    for subject in &index.harness_subjects {
        assert_eq!(subject.claim, HarnessSubjectClaim::NamedInvocation);
    }

    // A local closure binding shadows the same-named file-level fn.
    assert!(
        !credited_evidence("closure_shadow"),
        "a local `let` binding must fail closed: {:?}",
        index
            .harness_subjects
            .iter()
            .find(|subject| subject.name == "closure_shadow")
            .map(|subject| (&subject.calls, &subject.assertions))
    );
    // A same-named fn parameter shadows it too.
    assert!(
        !credited_evidence("param_shadow"),
        "a parameter binding must fail closed"
    );
    // A fn that exists only inside a nested module is not name-visible
    // to the invocation.
    assert!(
        !credited_evidence("nested_mod_shadow"),
        "a nested-module fn must fail closed"
    );
    // A top-level import binding the same leaf name makes the callback
    // ambiguous with the import.
    assert!(
        !credited_evidence("import_shadow"),
        "an import binding of the name must fail closed"
    );

    // The clean one-level positive still admits on the same authority:
    // an unshadowed top-level fn's evidence is credited (pinned in full
    // by helper_callback_bodies_contribute_one_level_of_subject_evidence).
    let clean_root = temp_dir("shadowed-clean-control")?;
    write_workspace(
        &clean_root,
        &[(
            "tests/clean_control.rs",
            r#"
use libtest_mimic::Trial;

fn clean_helper() -> u16 {
    production_call(9)
}

fn trials() -> Vec<Trial> {
    vec![Trial::test("clean_case", clean_helper)]
}
"#,
        )],
    )?;
    declare_harness_false_target(&clean_root, "clean_control", "tests/clean_control.rs")?;
    let clean_files = [PathBuf::from("tests/clean_control.rs")];
    let clean_registrations = [custom_target_registration("tests/clean_control.rs")];
    let clean_index =
        build_index_with_test_harnesses(&clean_root.0, &clean_files, &clean_registrations)?;
    assert!(
        clean_index
            .harness_subjects
            .iter()
            .find(|subject| subject.name == "clean_case")
            .is_some_and(|subject| subject
                .calls
                .iter()
                .any(|call| call.name == "production_call")),
        "the clean unshadowed callback still admits its helper body evidence: {:?}",
        clean_index.harness_subjects
    );
    Ok(())
}

#[test]
fn trial_method_oracle_receivers_carry_keyword_and_chained_forms()
-> Result<(), Box<dyn std::error::Error>> {
    // #3603 review: keyword receiver participants (`self`, `await`) are
    // ordinary postfix receivers. Dropping them would truncate the
    // oracle text (`value.unwrap()` instead of
    // `self.value().unwrap()`) and misattribute the observed receiver.
    let root = temp_dir("trial-keyword-receivers")?;
    write_workspace(
        &root,
        &[(
            "tests/keyword_receivers.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

struct Reader;

impl Reader {
    fn value(&self) -> Result<u16, String> {
        Ok(8080)
    }

    fn trials(&self) -> Vec<Trial> {
        vec![
            libtest_mimic::Trial::test("keyword_receivers", || {
                let direct = self.value().unwrap();
                let awaited = self.value().await.unwrap();
                let chained = self.value().map(|port| port).await.unwrap();
                Ok(())
            }),
        ]
    }
}
"#,
                "Reader.trials()",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "keyword_receivers", "tests/keyword_receivers.rs")?;
    let files = [PathBuf::from("tests/keyword_receivers.rs")];
    let registrations = [custom_target_registration("tests/keyword_receivers.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    let subject_test = index
        .tests
        .iter()
        .find(|test| test.name == "keyword_receivers")
        .ok_or("keyword_receivers test missing")?;

    let expected = [
        (14, "self.value().unwrap()"),
        (15, "self.value().await.unwrap()"),
        (16, "self.value().map(|port| port).await.unwrap()"),
    ];
    for (line, text) in expected {
        let oracle = subject_test
            .assertions
            .iter()
            .find(|oracle| oracle.text == text)
            .ok_or_else(|| format!("expected oracle `{text}` in {:?}", subject_test.assertions))?;
        assert_eq!(oracle.line, line, "{:?}", oracle);
        assert_eq!(oracle.kind, OracleKind::SmokeOnly, "{:?}", oracle);
        assert_eq!(oracle.strength, OracleStrength::Smoke, "{:?}", oracle);
        // The observed receiver keeps the full postfix chain.
        assert!(
            oracle.observed_tokens.contains(&"value".to_string()),
            "{:?}",
            oracle.observed_tokens
        );
    }
    // No oracle may carry a truncated receiver: the keyword must be
    // part of the text, not dropped from it.
    assert!(
        subject_test
            .assertions
            .iter()
            .all(|oracle| !oracle.text.starts_with("value.")),
        "{:?}",
        subject_test.assertions
    );
    Ok(())
}

#[test]
fn given_dormant_macro_rules_template_when_trial_scanned_then_no_smoke_oracle_from_template()
-> Result<(), Box<dyn std::error::Error>> {
    // #3603 review: a `macro_rules!` definition inside the trial callback
    // is a dormant token tree. A `.unwrap()` inside the template must not
    // gain smoke evidence while the macro is never invoked.
    let root = temp_dir("trial-dormant-macro-template")?;
    write_workspace(
        &root,
        &[(
            "tests/dormant_macro.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn trials() -> Vec<Trial> {
    vec![libtest_mimic::Trial::test("dormant_template", || {
        macro_rules! dormant {
            () => {
                assert_eq!(ready().unwrap(), 1);
            };
        }
        Ok(())
    })]
}
"#,
                "trials()",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "dormant_macro", "tests/dormant_macro.rs")?;
    let files = [PathBuf::from("tests/dormant_macro.rs")];
    let registrations = [custom_target_registration("tests/dormant_macro.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    let subject_test = index
        .tests
        .iter()
        .find(|test| test.name == "dormant_template")
        .ok_or("dormant_template test missing")?;

    // The full expected oracle set is exactly empty: the claimed span
    // contains no live oracle evidence at all — the template's
    // `assert_eq!` (ExactValue/Strong on the pre-fix head) and its
    // `.unwrap()` both stay unclaimed.
    assert!(
        subject_test.assertions.is_empty(),
        "a dormant macro_rules template must not gain smoke evidence: {:?}",
        subject_test.assertions
    );
    // The subject itself still classifies.
    assert!(
        index
            .harness_subjects
            .iter()
            .any(|subject| subject.name == "dormant_template"),
        "{:?}",
        index.harness_subjects
    );
    Ok(())
}

#[test]
fn given_dormant_macro_rules_template_in_helper_callback_then_no_oracle_either()
-> Result<(), Box<dyn std::error::Error>> {
    // #3603 review, helper-callback path: a bare-identifier helper whose
    // body defines a dormant `macro_rules!` template contributes its
    // body evidence one level deep — but the template's own oracle must
    // never join the subject while live helper evidence still admits.
    let root = temp_dir("trial-dormant-template-helper")?;
    write_workspace(
        &root,
        &[(
            "tests/dormant_template_helper.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn check_template() -> Result<(), String> {
    macro_rules! dormant {
        () => {
            assert_eq!(ready().unwrap(), 1);
        };
    }
    assert_eq!(parse_config("8080"), 8080);
    Ok(())
}

fn parse_config(raw: &str) -> u16 {
    raw.parse().unwrap_or(0)
}

fn trials() -> Vec<Trial> {
    vec![Trial::test("helper_template", check_template)]
}
"#,
                "trials()",
            ),
        )],
    )?;
    declare_harness_false_target(
        &root,
        "dormant_template_helper",
        "tests/dormant_template_helper.rs",
    )?;
    let files = [PathBuf::from("tests/dormant_template_helper.rs")];
    let registrations = [custom_target_registration(
        "tests/dormant_template_helper.rs",
    )];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    let subject_test = index
        .tests
        .iter()
        .find(|test| test.name == "helper_template")
        .ok_or("helper_template test missing")?;

    // The live helper assertion (line 10) stays; the template's oracle
    // (line 7, inside the `macro_rules!` definition spanning lines 5-9)
    // is dropped — the full expected set. The template's calls and
    // literals are equally inert (#3603 round-5, ICPS-adjacent scope).
    let texts = subject_test
        .assertions
        .iter()
        .map(|oracle| (oracle.line, oracle.text.clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        texts,
        vec![(10, "assert_eq!(parse_config(\"8080\"), 8080);".to_string())],
        "{:?}",
        texts
    );
    // The helper's live call evidence still admits (one-level parity);
    // the template's calls and literals never join.
    assert!(
        subject_test
            .calls
            .iter()
            .any(|call| call.name == "parse_config"),
        "{:?}",
        subject_test.calls
    );
    assert!(
        !subject_test.calls.iter().any(|call| call.name == "ready"),
        "{:?}",
        subject_test.calls
    );
    assert!(
        !subject_test
            .literals
            .iter()
            .any(|literal| literal.line >= 5 && literal.line <= 9),
        "{:?}",
        subject_test.literals
    );
    Ok(())
}

#[test]
fn given_block_commented_helper_evidence_then_calls_and_literals_stay_inert()
-> Result<(), Box<dyn std::error::Error>> {
    // #3603 review (devin ICNY): the lexical call and literal extractors
    // scanned block comments as live text, so commented-out calls and
    // numbers joined the trial subject. Both extractors now mask
    // comments (and string contents) before scanning, at the shared
    // extractor, so ordinary `#[test]` bodies get the same guarantee.
    let root = temp_dir("trial-block-comment-evidence")?;
    write_workspace(
        &root,
        &[(
            "tests/block_comment.rs",
            r#"
use libtest_mimic::Trial;

fn check_commented() -> Result<(), String> {
    /* other_call(9);
    let n = 7; */
    live_call(3);
    let live = 5;
    assert_eq!(live, 5);
    Ok(())
}

fn trials() -> Vec<Trial> {
    vec![Trial::test("comment_case", check_commented)]
}
"#,
        )],
    )?;
    declare_harness_false_target(&root, "block_comment", "tests/block_comment.rs")?;
    let files = [PathBuf::from("tests/block_comment.rs")];
    let registrations = [custom_target_registration("tests/block_comment.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    let subject = index
        .harness_subjects
        .iter()
        .find(|subject| subject.name == "comment_case")
        .ok_or("comment_case subject missing")?;

    let call_names = subject
        .calls
        .iter()
        .map(|call| call.name.as_str())
        .collect::<Vec<_>>();
    assert!(
        !call_names.contains(&"other_call"),
        "block-commented call must not join the subject: {call_names:?}"
    );
    assert!(
        call_names.contains(&"live_call"),
        "live call still admits: {call_names:?}"
    );
    let literal_values = subject
        .literals
        .iter()
        .map(|literal| literal.value.as_str())
        .collect::<Vec<_>>();
    assert!(
        !literal_values.contains(&"9") && !literal_values.contains(&"7"),
        "commented numbers must not join the subject: {literal_values:?}"
    );
    assert!(
        literal_values.contains(&"5") && literal_values.contains(&"3"),
        "live literals still admit: {literal_values:?}"
    );
    Ok(())
}

#[test]
fn given_one_line_dormant_macro_then_live_same_line_evidence_survives()
-> Result<(), Box<dyn std::error::Error>> {
    // #3603 review (devin ICOI): the dormant filter compared by line, so
    // a one-line `macro_rules!` definition erased live facts sharing the
    // line. Span masking erases only the definition's bytes; live code
    // before and after it on the same line keeps its evidence.
    let root = temp_dir("trial-one-line-dormant")?;
    write_workspace(
        &root,
        &[(
            "tests/one_line_dormant.rs",
            r#"
use libtest_mimic::Trial;

fn check_inline() -> Result<(), String> {
    macro_rules! m { () => { template_call(9); } } live_call(3); more(5); assert_eq!(2, 2); Ok(())
}

fn trials() -> Vec<Trial> {
    vec![Trial::test("inline_case", check_inline)]
}
"#,
        )],
    )?;
    declare_harness_false_target(&root, "one_line_dormant", "tests/one_line_dormant.rs")?;
    let files = [PathBuf::from("tests/one_line_dormant.rs")];
    let registrations = [custom_target_registration("tests/one_line_dormant.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    let subject = index
        .harness_subjects
        .iter()
        .find(|subject| subject.name == "inline_case")
        .ok_or("inline_case subject missing")?;

    let call_names = subject
        .calls
        .iter()
        .map(|call| call.name.as_str())
        .collect::<Vec<_>>();
    assert!(
        !call_names.contains(&"template_call"),
        "template call must not join: {call_names:?}"
    );
    assert!(
        call_names.contains(&"live_call") && call_names.contains(&"more"),
        "live calls sharing the template's line survive: {call_names:?}"
    );
    let literal_values = subject
        .literals
        .iter()
        .map(|literal| literal.value.as_str())
        .collect::<Vec<_>>();
    assert!(
        !literal_values.contains(&"9"),
        "template literal must not join: {literal_values:?}"
    );
    assert!(
        literal_values.contains(&"3")
            && literal_values.contains(&"5")
            && literal_values.contains(&"2"),
        "live literals sharing the line survive: {literal_values:?}"
    );
    assert!(
        subject
            .assertions
            .iter()
            .any(|oracle| oracle.text.contains("assert_eq!(2, 2)")),
        "the live same-line assertion survives: {:?}",
        subject.assertions
    );
    Ok(())
}

#[test]
fn trial_qualified_assertions_keep_the_full_path() -> Result<(), Box<dyn std::error::Error>> {
    // #3603 review (devin ICPS): qualified assertion invocations
    // (`insta::assert_snapshot!`) classified on their leaf but sliced
    // their text from the leaf, dropping the path prefix. The scanner
    // now slices from the full contiguous macro path, matching the
    // ordinary parser's macro-call slice byte for byte.
    let root = temp_dir("trial-qualified-assertions")?;
    write_workspace(
        &root,
        &[(
            "tests/qualified_assertions.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn trials() -> Vec<Trial> {
    vec![
        libtest_mimic::Trial::test("qualified_insta", || {
            let value = create();
            insta::assert_snapshot![value];
            Ok(())
        }),
        libtest_mimic::Trial::test("qualified_json", || {
            let value = create();
            crate::snap::assert_json_snapshot!(value);
            Ok(())
        }),
    ]
}
"#,
                "trials()",
            ),
        )],
    )?;
    declare_harness_false_target(
        &root,
        "qualified_assertions",
        "tests/qualified_assertions.rs",
    )?;
    let files = [PathBuf::from("tests/qualified_assertions.rs")];
    let registrations = [custom_target_registration("tests/qualified_assertions.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    // Ordinary-parser parity for the same qualified spelling.
    let ordinary =
        "fn ordinary() {\n    let value = create();\n    insta::assert_snapshot![value];\n}";
    let ordinary_oracles =
        parser_oracles_for_function(ordinary, 1).ok_or("ordinary fixture must parse")?;
    let ordinary_oracle = ordinary_oracles
        .iter()
        .find(|oracle| oracle.text.contains("assert_snapshot"))
        .ok_or("ordinary qualified oracle missing")?;

    let insta = index
        .tests
        .iter()
        .find(|test| test.name == "qualified_insta")
        .ok_or("qualified_insta test missing")?;
    let insta_oracle = insta
        .assertions
        .iter()
        .find(|oracle| oracle.text.contains("assert_snapshot"))
        .ok_or("qualified insta oracle missing")?;
    assert_eq!(insta_oracle.text, "insta::assert_snapshot![value];");
    assert_eq!(insta_oracle.text, ordinary_oracle.text, "ordinary parity");
    assert_eq!(insta_oracle.kind, ordinary_oracle.kind);
    assert_eq!(insta_oracle.strength, ordinary_oracle.strength);
    assert_eq!(
        insta_oracle.observed_tokens,
        ordinary_oracle.observed_tokens
    );
    assert_eq!(insta_oracle.line, 8, "{insta_oracle:?}");

    let json = index
        .tests
        .iter()
        .find(|test| test.name == "qualified_json")
        .ok_or("qualified_json test missing")?;
    let json_oracle = json
        .assertions
        .iter()
        .find(|oracle| oracle.text.contains("assert_json_snapshot"))
        .ok_or("qualified json oracle missing")?;
    assert_eq!(
        json_oracle.text,
        "crate::snap::assert_json_snapshot!(value);"
    );
    assert_eq!(json_oracle.line, 13, "{json_oracle:?}");
    Ok(())
}

#[test]
fn dormant_template_spans_cover_the_parsed_definition_and_mask_exactly()
-> Result<(), Box<dyn std::error::Error>> {
    // Mechanism pin for the helper-path template filter: the spans come
    // from the parsed `ast::MacroRules` node, and masking erases exactly
    // those bytes (spaces, newlines preserved) while live code around
    // the template survives for extraction.
    let body = "fn check() {
    macro_rules! dormant {
        () => {
            assert_eq!(1, 1);
        };
    }
    assert_eq!(2, 2);
}";
    let spans = dormant_template_parse_spans(body);
    assert_eq!(spans.len(), 1, "{spans:?}");
    let (start, end) = spans[0];
    let masked = mask_dormant_template_spans(body, &spans);
    assert!(
        !masked.contains("assert_eq!(1, 1)"),
        "template contents must be erased: {masked}"
    );
    assert!(
        masked.contains("assert_eq!(2, 2);"),
        "live code outside the template survives: {masked}"
    );
    assert_eq!(
        masked.lines().count(),
        body.lines().count(),
        "masking preserves line structure"
    );
    assert!(end > start);
    // A body without definitions yields no spans: nothing is masked.
    let plain = "fn check() {
    assert_eq!(1, 1);
}";
    assert!(dormant_template_parse_spans(plain).is_empty());
    assert_eq!(mask_dormant_template_spans(plain, &[]), plain);
    Ok(())
}

#[test]
fn trial_alternative_delimiters_keep_invocation_evidence() -> Result<(), Box<dyn std::error::Error>>
{
    // #3603 review (devin HQB0 + coderabbit Gk75): `assert![..]` and
    // `assert!{..}` previously skipped the group without contributing its
    // contents — tokenless, weak evidence. The trial scanner now emits
    // the complete invocation text and classifies it exactly like the
    // ordinary parser does for `#[test]` functions.
    let root = temp_dir("trial-alt-delimiters")?;
    write_workspace(
        &root,
        &[(
            "tests/alt_delims.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn trials() -> Vec<Trial> {
    vec![
        libtest_mimic::Trial::test("bracket_assert", || {
            let value = 40u16;
            let expected = 40u16;
            assert![value == expected];
            Ok(())
        }),
        libtest_mimic::Trial::test("brace_assert", || {
            let value = 41u16;
            let expected = 41u16;
            assert!{value == expected}
            Ok(())
        }),
        libtest_mimic::Trial::test("newline_semi", || {
            let value = 42u16;
            let expected = 42u16;
            assert!{value == expected}
            ;
            Ok(())
        }),
    ]
}
"#,
                "trials()",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "alt_delims", "tests/alt_delims.rs")?;
    let files = [PathBuf::from("tests/alt_delims.rs")];
    let registrations = [custom_target_registration("tests/alt_delims.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    // Ordinary-parser oracle for the same assertion spelling: kind,
    // strength, text, and observed tokens must match the trial scanner.
    let ordinary = "fn ordinary() {\n    let value = 40u16;\n    let expected = 40u16;\n    assert![value == expected];\n}";
    let ordinary_oracles =
        parser_oracles_for_function(ordinary, 1).ok_or("ordinary fixture must parse")?;
    let ordinary_oracle = ordinary_oracles
        .iter()
        .find(|oracle| oracle.text.contains("assert!"))
        .ok_or("ordinary assert![..] oracle missing")?;

    let bracket = index
        .tests
        .iter()
        .find(|test| test.name == "bracket_assert")
        .ok_or("bracket_assert test missing")?;
    let bracket_oracle = bracket
        .assertions
        .iter()
        .find(|oracle| oracle.text.contains("assert!"))
        .ok_or("assert![..] oracle missing")?;
    assert_eq!(bracket_oracle.text, "assert![value == expected];");
    assert_eq!(bracket_oracle.text, ordinary_oracle.text, "ordinary parity");
    assert_eq!(bracket_oracle.kind, ordinary_oracle.kind);
    assert_eq!(bracket_oracle.strength, ordinary_oracle.strength);
    assert_eq!(
        bracket_oracle.observed_tokens,
        ordinary_oracle.observed_tokens
    );
    assert_eq!(bracket_oracle.line, 9, "{bracket_oracle:?}");

    let brace = index
        .tests
        .iter()
        .find(|test| test.name == "brace_assert")
        .ok_or("brace_assert test missing")?;
    let brace_oracle = brace
        .assertions
        .iter()
        .find(|oracle| oracle.text.contains("assert!"))
        .ok_or("assert!{..} oracle missing")?;
    assert_eq!(brace_oracle.text, "assert!{value == expected}");
    assert_eq!(brace_oracle.kind, ordinary_oracle.kind);
    assert_eq!(brace_oracle.strength, ordinary_oracle.strength);
    assert_eq!(
        brace_oracle.observed_tokens,
        ordinary_oracle.observed_tokens
    );
    assert_eq!(brace_oracle.line, 15, "{brace_oracle:?}");

    // A semicolon on the NEXT line is not part of the invocation: the
    // oracle text ends at the group close (IDV7).
    let newline_semi = index
        .tests
        .iter()
        .find(|test| test.name == "newline_semi")
        .ok_or("newline_semi test missing")?;
    let newline_oracle = newline_semi
        .assertions
        .iter()
        .find(|oracle| oracle.text.contains("assert!"))
        .ok_or("newline_semi oracle missing")?;
    assert_eq!(newline_oracle.text, "assert!{value == expected}");
    assert_eq!(newline_oracle.line, 21, "{newline_oracle:?}");
    Ok(())
}

#[test]
fn trial_dormant_alternative_delimiters_stay_inert() -> Result<(), Box<dyn std::error::Error>> {
    // #3603 review (devin HQBC): `macro_rules!` accepts every delimiter
    // Rust permits. Parenthesized and bracketed dormant templates inside
    // a trial callback previously escaped the definition skip and were
    // scanned as live assertions and method oracles.
    let root = temp_dir("trial-dormant-alt-delims")?;
    write_workspace(
        &root,
        &[(
            "tests/dormant_alt_delims.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn trials() -> Vec<Trial> {
    vec![libtest_mimic::Trial::test("alt_delim_templates", || {
        macro_rules! paren_template (
            () => { assert_eq!(ready().unwrap(), 1); }
        );
        macro_rules! bracket_template [
            () => { assert_eq!(ready().unwrap(), 2); }
        ];
        Ok(())
    })]
}
"#,
                "trials()",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "dormant_alt_delims", "tests/dormant_alt_delims.rs")?;
    let files = [PathBuf::from("tests/dormant_alt_delims.rs")];
    let registrations = [custom_target_registration("tests/dormant_alt_delims.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    let subject_test = index
        .tests
        .iter()
        .find(|test| test.name == "alt_delim_templates")
        .ok_or("alt_delim_templates test missing")?;

    // The full expected oracle set is exactly empty — and the templates'
    // calls and literals are equally inert (IDV-).
    assert!(
        subject_test.assertions.is_empty(),
        "dormant paren/bracket templates must not gain oracles: {:?}",
        subject_test.assertions
    );
    assert!(
        !subject_test.calls.iter().any(|call| call.name == "ready"),
        "{:?}",
        subject_test.calls
    );
    assert!(
        !subject_test
            .literals
            .iter()
            .any(|literal| literal.value == "1" || literal.value == "2"),
        "{:?}",
        subject_test.literals
    );
    // The subject itself still classifies.
    assert!(
        index
            .harness_subjects
            .iter()
            .any(|subject| subject.name == "alt_delim_templates"),
        "{:?}",
        index.harness_subjects
    );
    Ok(())
}

#[test]
fn given_dormant_template_in_helper_then_calls_and_literals_stay_inert()
-> Result<(), Box<dyn std::error::Error>> {
    // #3603 review (devin HQAG): the helper-path dormant filter covered
    // assertions only — the template's calls and literals still merged
    // into the subject. Every merged helper evidence collection now
    // drops template lines while live surrounding evidence stays, so
    // the filter is targeted rather than a blanket drop.
    let root = temp_dir("trial-dormant-helper-facts")?;
    write_workspace(
        &root,
        &[(
            "tests/dormant_helper_facts.rs",
            r#"
use libtest_mimic::Trial;

fn check_shadow() -> Result<(), String> {
    macro_rules! dormant {
        () => {
            template_call(9);
        };
    }
    live_call(7);
    let marker = 5;
    let _ = marker;
    Ok(())
}

fn trials() -> Vec<Trial> {
    vec![Trial::test("helper_facts", check_shadow)]
}
"#,
        )],
    )?;
    declare_harness_false_target(
        &root,
        "dormant_helper_facts",
        "tests/dormant_helper_facts.rs",
    )?;
    let files = [PathBuf::from("tests/dormant_helper_facts.rs")];
    let registrations = [custom_target_registration("tests/dormant_helper_facts.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    let subject = index
        .harness_subjects
        .iter()
        .find(|subject| subject.name == "helper_facts")
        .ok_or("helper_facts subject missing")?;

    let call_names = subject
        .calls
        .iter()
        .map(|call| call.name.as_str())
        .collect::<Vec<_>>();
    assert!(
        !call_names.contains(&"template_call"),
        "template call must not join the subject: {call_names:?}"
    );
    assert!(
        call_names.contains(&"live_call"),
        "live helper call still admits: {call_names:?}"
    );
    let literal_values = subject
        .literals
        .iter()
        .map(|literal| literal.value.as_str())
        .collect::<Vec<_>>();
    assert!(
        !literal_values.contains(&"9"),
        "template literal must not join the subject: {literal_values:?}"
    );
    assert!(
        literal_values.contains(&"7") && literal_values.contains(&"5"),
        "live helper literals still admit: {literal_values:?}"
    );
    Ok(())
}

#[test]
fn given_same_line_functions_then_ast_scope_resolves_shadow()
-> Result<(), Box<dyn std::error::Error>> {
    // #3603 review (devin My3M): two functions sharing one source line
    // made the line-span fallback pick the wrong enclosing scope, so the
    // shadow scan missed the second function's local binding and credited
    // the top-level helper's evidence. The scope now resolves from the
    // invocation token's Fn ancestor: the local shadow fails the callback
    // closed.
    let root = temp_dir("same-line-functions")?;
    write_workspace(
        &root,
        &[(
            "tests/same_line_fns.rs",
            r#"
use libtest_mimic::Trial;

fn shadow_target() -> u16 { production_call(4) }

fn side_a() -> u16 { live_marker(1) } fn side_b() -> Trial { let shadow_target = || 1u16; Trial::test("same_line_shadow", shadow_target) }
"#,
        )],
    )?;
    declare_harness_false_target(&root, "same_line_fns", "tests/same_line_fns.rs")?;
    let files = [PathBuf::from("tests/same_line_fns.rs")];
    let registrations = [custom_target_registration("tests/same_line_fns.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    let subject = index
        .harness_subjects
        .iter()
        .find(|subject| subject.name == "same_line_shadow")
        .ok_or("same_line_shadow subject missing")?;

    // The trial's own enclosing scope (side_b) binds the callback name
    // locally, so the top-level shadow_target fn's evidence is never
    // credited.
    assert!(
        !subject
            .calls
            .iter()
            .any(|call| call.name == "production_call"),
        "the same-line local shadow must fail the callback closed: {:?}",
        subject.calls
    );
    assert!(
        !subject
            .assertions
            .iter()
            .any(|oracle| oracle.text.contains("production_call")),
        "{:?}",
        subject.assertions
    );
    // The subject itself still classifies.
    assert_eq!(subject.claim, HarnessSubjectClaim::NamedInvocation);
    Ok(())
}

#[test]
fn given_const_or_static_shadow_then_callback_fails_closed()
-> Result<(), Box<dyn std::error::Error>> {
    // #3603 review (devin EEaa): `const`/`static` bindings carry a Name,
    // not an identifier pattern, so the IdentPat shadow scan could not
    // see them. A const/static binding of the callback name — local in
    // the enclosing body, or at file level next to a same-named fn —
    // fails closed instead of crediting the fn's evidence.
    let root = temp_dir("const-static-shadow")?;
    write_workspace(
        &root,
        &[(
            "tests/const_static_shadow.rs",
            r#"
use libtest_mimic::Trial;

fn shadow_target() -> u16 {
    production_call(1)
}

fn shadow_target_fn() -> u16 {
    production_call(2)
}

fn local_const_trials() -> Vec<Trial> {
    const shadow_target: u16 = 1;
    let _ = shadow_target;
    vec![Trial::test("const_shadow", shadow_target)]
}

fn local_static_trials() -> Vec<Trial> {
    static shadow_target: u8 = 3;
    let _ = shadow_target;
    vec![Trial::test("static_shadow", shadow_target)]
}

fn item_shadow_trials() -> Vec<Trial> {
    const shadow_target_fn: u16 = 2;
    let _ = shadow_target_fn;
    vec![Trial::test("item_shadow", shadow_target_fn)]
}
"#,
        )],
    )?;
    declare_harness_false_target(&root, "const_static_shadow", "tests/const_static_shadow.rs")?;
    let files = [PathBuf::from("tests/const_static_shadow.rs")];
    let registrations = [custom_target_registration("tests/const_static_shadow.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    let credited = |subject_name: &str| -> bool {
        index
            .harness_subjects
            .iter()
            .find(|subject| subject.name == subject_name)
            .is_some_and(|subject| {
                subject
                    .calls
                    .iter()
                    .any(|call| call.name == "production_call")
            })
    };
    // A local `const` binding of the callback name shadows the fn.
    assert!(
        !credited("const_shadow"),
        "a local const shadow must fail closed: {:?}",
        index
            .harness_subjects
            .iter()
            .find(|subject| subject.name == "const_shadow")
            .map(|subject| &subject.calls)
    );
    // A local `static` binding likewise.
    assert!(
        !credited("static_shadow"),
        "a local static shadow must fail closed"
    );
    // A file-level const binding the same name as a top-level fn blocks
    // that fn's evidence even though exactly one fn fact exists.
    assert!(
        !credited("item_shadow"),
        "a file-level const binding must fail closed"
    );
    // Every subject still classifies: fail-closed means uncredited
    // evidence, not a missing subject.
    let mut names = index
        .harness_subjects
        .iter()
        .map(|subject| subject.name.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(names, vec!["const_shadow", "item_shadow", "static_shadow"]);
    Ok(())
}

#[test]
fn trial_method_oracle_receivers_carry_indexed_cast_operator_forms()
-> Result<(), Box<dyn std::error::Error>> {
    // #3603 review (devin FnF6): receiver parity for the indexed, cast,
    // operator, and negation forms. The negation operator never extends
    // a receiver (a `!` continues the walk only as a macro-call bang),
    // so `!flag.unwrap()` keeps its receiver at `flag` — the under-emit
    // direction.
    let root = temp_dir("trial-receiver-forms")?;
    write_workspace(
        &root,
        &[(
            "tests/receiver_forms.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn trials() -> Vec<Trial> {
    vec![libtest_mimic::Trial::test("receiver_forms", || {
        let cache = vec![Some(1u16)];
        let indexed = cache[0].unwrap();
        let raw = 7u8;
        let cast = (raw as u16).unwrap();
        let left = 2u16;
        let right = Some(3u16);
        let sum = left + right.unwrap();
        let flag = Some(true);
        let ready = !flag.unwrap();
        let _ = (indexed, cast, sum, ready);
        Ok(())
    })]
}
"#,
                "trials()",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "receiver_forms", "tests/receiver_forms.rs")?;
    let files = [PathBuf::from("tests/receiver_forms.rs")];
    let registrations = [custom_target_registration("tests/receiver_forms.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    let subject_test = index
        .tests
        .iter()
        .find(|test| test.name == "receiver_forms")
        .ok_or("receiver_forms test missing")?;

    let expected = [
        (7, "cache[0].unwrap()"),
        (9, "(raw as u16).unwrap()"),
        (12, "right.unwrap()"),
        (14, "flag.unwrap()"),
    ];
    for (line, text) in expected {
        let oracle = subject_test
            .assertions
            .iter()
            .find(|oracle| oracle.text == text)
            .ok_or_else(|| format!("expected oracle `{text}` in {:?}", subject_test.assertions))?;
        assert_eq!(oracle.line, line, "{oracle:?}");
        assert_eq!(oracle.kind, OracleKind::SmokeOnly, "{oracle:?}");
        assert_eq!(oracle.strength, OracleStrength::Smoke, "{oracle:?}");
    }
    // The indexed receiver keeps its full postfix chain in observed
    // tokens; the operator and negation forms keep only the operand.
    let indexed = subject_test
        .assertions
        .iter()
        .find(|oracle| oracle.text == "cache[0].unwrap()")
        .ok_or("indexed oracle missing")?;
    assert!(
        indexed.observed_tokens.contains(&"cache".to_string()),
        "{:?}",
        indexed.observed_tokens
    );
    Ok(())
}

#[test]
fn trial_receiver_walk_stops_at_comparison_operators() -> Result<(), Box<dyn std::error::Error>> {
    // #3603 review (coderabbit LtlN): the receiver walk accepted any
    // R_ANGLE as a turbofish close, so `a < b && c > d.unwrap()` emitted
    // `< b && c > d.unwrap()` as receiver text and inflated
    // observed_tokens — the over-emit direction. An angle group in
    // receiver position is a turbofish only when its `<` is preceded by
    // `::`; comparisons end the receiver at their operand.
    let root = temp_dir("trial-comparison-receivers")?;
    write_workspace(
        &root,
        &[(
            "tests/comparison_receivers.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn parse_u16(raw: &str) -> u16 {
    raw.parse::<u16>().unwrap_or(0)
}

fn trials() -> Vec<Trial> {
    vec![libtest_mimic::Trial::test("comparison_forms", || {
        let a = true;
        let b = true;
        let c = true;
        let d = Some(1u16);
        let check = a < b && c > d.unwrap();
        let parsed = parse_u16("7").parse::<u16>().unwrap();
        let _ = (check, parsed);
        Ok(())
    })]
}
"#,
                "trials()",
            ),
        )],
    )?;
    declare_harness_false_target(
        &root,
        "comparison_receivers",
        "tests/comparison_receivers.rs",
    )?;
    let files = [PathBuf::from("tests/comparison_receivers.rs")];
    let registrations = [custom_target_registration("tests/comparison_receivers.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    let subject_test = index
        .tests
        .iter()
        .find(|test| test.name == "comparison_forms")
        .ok_or("comparison_forms test missing")?;

    // The comparison's operand is the receiver; the operator never
    // extends it and no cross-expression text leaks in.
    let d_oracle = subject_test
        .assertions
        .iter()
        .find(|oracle| oracle.text == "d.unwrap()")
        .ok_or_else(|| format!("expected `d.unwrap()` in {:?}", subject_test.assertions))?;
    assert_eq!(d_oracle.line, 14, "{d_oracle:?}");
    assert!(
        !subject_test
            .assertions
            .iter()
            .any(|oracle| oracle.text.contains("< b")),
        "comparison text must not leak into receivers: {:?}",
        subject_test.assertions
    );
    assert!(
        !d_oracle.observed_tokens.contains(&"b".to_string()),
        "{:?}",
        d_oracle.observed_tokens
    );
    // A genuine turbofish still keeps its generic arguments.
    let turbofish = subject_test
        .assertions
        .iter()
        .find(|oracle| oracle.text.contains("parse::<u16>"))
        .ok_or_else(|| format!("expected turbofish oracle in {:?}", subject_test.assertions))?;
    assert_eq!(turbofish.text, "parse_u16(\"7\").parse::<u16>().unwrap()");
    Ok(())
}

#[test]
fn trial_method_unwrap_expect_oracles_carry_real_lines_and_receivers()
-> Result<(), Box<dyn std::error::Error>> {
    // #3603 gap 2: ordinary `#[test]` parsing records `.unwrap()` /
    // `.expect()` method calls as smoke evidence; the trial token scanner
    // must credit the same oracles for trial subjects, with real line
    // attribution and the receiver expression in the oracle text.
    let root = temp_dir("trial-method-oracles")?;
    write_workspace(
        &root,
        &[(
            "tests/method_oracles.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn parse_port(raw: &str) -> Result<u16, String> {
    raw.parse::<u16>().map_err(|error| error.to_string())
}

fn trials() -> Vec<Trial> {
    vec![
        libtest_mimic::Trial::test("alpha_unwraps", || {
            let port = parse_port("8080").unwrap();
            let doubled = port.unwrap() * 2;
            assert_eq!(doubled, 16160);
            let label: Result<String, String> = Ok(format!("port={doubled}"));
            let shown = label.expect("label ready");
            let unwrapped_value = doubled;
            let _ = unwrapped_value;
            let _ = config.expect;
            println!("{}", doubled.unwrap());
            Result::<u8, u8>::unwrap(Ok(1));
            Ok(())
        }),
    ]
}

struct Config {
    expect: u8,
}
"#,
                "trials()",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "method_oracles", "tests/method_oracles.rs")?;
    let files = [PathBuf::from("tests/method_oracles.rs")];
    let registrations = [custom_target_registration("tests/method_oracles.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    let alpha = index
        .tests
        .iter()
        .find(|test| test.name == "alpha_unwraps")
        .ok_or("alpha test missing")?;

    let oracle_lines = |needle: &str| -> Vec<(usize, &String)> {
        alpha
            .assertions
            .iter()
            .filter(|oracle| oracle.text.contains(needle))
            .map(|oracle| (oracle.line, &oracle.text))
            .collect()
    };
    let unwraps = oracle_lines("unwrap()");
    let expects = oracle_lines(".expect(");
    let assert_eqs = oracle_lines("assert_eq!(");

    // Method oracles exist with receiver-ful text and real source lines.
    assert_eq!(
        unwraps
            .iter()
            .filter(|(_, text)| text.as_str() == "parse_port(\"8080\").unwrap()")
            .count(),
        1,
        "receiver call chain is the oracle text: {unwraps:?}"
    );
    assert!(
        unwraps.contains(&(11, &"parse_port(\"8080\").unwrap()".to_string())),
        "unwrap oracle carries the real source line: {unwraps:?}"
    );
    assert!(
        unwraps.contains(&(12, &"port.unwrap()".to_string())),
        "bare-receiver unwrap keeps the receiver in the text: {unwraps:?}"
    );
    assert!(
        expects.contains(&(15, &"label.expect(\"label ready\")".to_string())),
        "expect oracle with real line and message argument: {expects:?}"
    );
    // The assertion-macro oracle line is the real source line too (and
    // its text carries the same-line trailing semicolon, ordinary-parser
    // parity).
    assert!(
        assert_eqs.contains(&(13, &"assert_eq!(doubled, 16160);".to_string())),
        "macro oracle lines are real source lines: {assert_eqs:?}"
    );

    // Smoke-strength parity with ordinary `#[test]` parsing.
    for oracle in alpha
        .assertions
        .iter()
        .filter(|oracle| oracle.text.ends_with(".unwrap()") || oracle.text.contains(".expect("))
    {
        assert_eq!(oracle.kind, OracleKind::SmokeOnly, "{oracle:?}");
        assert_eq!(oracle.strength, OracleStrength::Smoke, "{oracle:?}");
    }

    // Fail-closed controls: a struct field named `expect`, a path-shaped
    // `Result::unwrap` call, and anything inside a non-assertion macro's
    // input never classify.
    assert!(
        unwraps.iter().all(|(_, text)| *text != "unwrap"),
        "{unwraps:?}"
    );
    assert_eq!(
        expects.len(),
        1,
        "only the real method-position expect classifies: {expects:?}"
    );
    assert!(
        alpha
            .assertions
            .iter()
            .all(|oracle| !oracle.text.contains("::unwrap")
                && !oracle.text.contains("doubled.unwrap()")),
        "path-shaped calls and non-assertion macro inputs stay out: {:?}",
        alpha.assertions
    );
    Ok(())
}

#[test]
fn foreign_trial_paths_are_not_adopted_through_an_import_binding()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("foreign-trial-path")?;
    write_workspace(
        &root,
        &[(
            "tests/foreign_trial.rs",
            "use libtest_mimic::Trial;

fn other() -> Vec<Trial> {
    vec![other_module :: Trial::test(\"foreign_case\", || Ok(()))]
}
",
        )],
    )?;
    let files = [PathBuf::from("tests/foreign_trial.rs")];
    declare_harness_false_target(&root, "foreign_trial", "tests/foreign_trial.rs")?;
    let registrations = [custom_target_registration("tests/foreign_trial.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert!(
        index.harness_subjects.is_empty(),
        "{:?}",
        index.harness_subjects
    );
    assert!(index.tests.is_empty(), "{:?}", index.tests);
    Ok(())
}

#[test]
fn record_field_shape_inside_a_macro_token_tree_still_classifies()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("record-shape-in-macro")?;
    write_workspace(
        &root,
        &[(
            "tests/record_in_macro.rs",
            "use libtest_mimic::Trial;

struct Suite {
    trials: Vec<Trial>,
}

fn suite() -> Suite {
    Suite {
        trials: vec![Suite { trials: Trial::test(\"nested_case\", || Ok(())) }.trials],
    }
}
",
        )],
    )?;
    let files = [PathBuf::from("tests/record_in_macro.rs")];
    declare_harness_false_target(&root, "record_in_macro", "tests/record_in_macro.rs")?;
    let registrations = [custom_target_registration("tests/record_in_macro.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    // Inside vec!'s token tree the record field is raw syntax; the
    // field-name-colon-before-{/, shape keeps the subject classified.
    assert_eq!(
        index
            .harness_subjects
            .iter()
            .map(|subject| subject.name.as_str())
            .collect::<Vec<_>>(),
        vec!["nested_case"]
    );
    Ok(())
}

#[test]
fn macro_input_data_never_becomes_a_subject() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("macro-input-data")?;
    write_workspace(
        &root,
        &[(
            "tests/macro_input.rs",
            "fn debug() -> String {
    stringify!(trial: Trial::test(\"ghost_case\", || Ok(())))
}
",
        )],
    )?;
    let files = [PathBuf::from("tests/macro_input.rs")];
    declare_harness_false_target(&root, "macro_input", "tests/macro_input.rs")?;
    let registrations = [custom_target_registration("tests/macro_input.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    // Macro input is inert data; a `label:` colon inside it must not
    // adopt the record-field exception.
    assert!(
        index.harness_subjects.is_empty(),
        "{:?}",
        index.harness_subjects
    );
    assert!(index.tests.is_empty(), "{:?}", index.tests);
    Ok(())
}

#[test]
fn struct_field_initializers_still_start_trial_paths() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("struct-field-trial")?;
    write_workspace(
        &root,
        &[(
            "tests/field_trial.rs",
            "use libtest_mimic::Trial;

struct Suite {
    trials: Vec<Trial>,
}

fn suite() -> Suite {
    Suite {
        trials: vec![Trial::test(\"field_case\", || Ok(()))],
    }
}
",
        )],
    )?;
    let files = [PathBuf::from("tests/field_trial.rs")];
    declare_harness_false_target(&root, "field_trial", "tests/field_trial.rs")?;
    let registrations = [custom_target_registration("tests/field_trial.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert_eq!(
        index
            .harness_subjects
            .iter()
            .map(|subject| subject.name.as_str())
            .collect::<Vec<_>>(),
        vec!["field_case"]
    );
    Ok(())
}

#[test]
fn dormant_macro_templates_never_become_subjects() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("dormant-macro-template")?;
    write_workspace(
        &root,
        &[(
            "tests/dormant_template.rs",
            "macro_rules! trial_template {
    ($name:literal) => {
        libtest_mimic::Trial::test($name, || Ok(()))
    };
}

fn trials() -> Vec<libtest_mimic::Trial> {
    Vec::new()
}
",
        )],
    )?;
    let files = [PathBuf::from("tests/dormant_template.rs")];
    declare_harness_false_target(&root, "dormant_template", "tests/dormant_template.rs")?;
    let registrations = [custom_target_registration("tests/dormant_template.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    // The macro body is a template, not a registration: an uninvoked
    // template must not fabricate an executable test.
    assert!(
        index.harness_subjects.is_empty(),
        "{:?}",
        index.harness_subjects
    );
    assert!(index.tests.is_empty(), "{:?}", index.tests);
    Ok(())
}

#[test]
fn registered_attribute_promotes_exact_matches_only() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("registered-attribute")?;
    write_workspace(
        &root,
        &[(
            "crates/contract/tests/api.rs",
            r#"
use myco::contract_test;

pub fn price(amount: i32) -> i32 {
    amount * 2
}

#[contract_test]
fn price_round_trips() {
    assert_eq!(price(2), 4);
}

#[myco::contract_test]
fn price_rejects_zero() {
    assert_ne!(price(0), 1);
}

#[myco::contract_tests]
fn lookalike_suffix_is_not_registered() {
    assert_eq!(price(1), 2);
}

#[myco_contract_test]
fn lookalike_prefix_is_not_registered() {
    assert_eq!(price(1), 2);
}

fn plain_helper() -> i32 {
    price(3)
}
"#,
        )],
    )?;
    let target = "crates/contract/tests/api.rs";
    let files = [PathBuf::from(target)];
    let registrations = [attribute_registration(target, "myco::contract_test")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    let mut subject_names = index
        .harness_subjects
        .iter()
        .map(|subject| subject.name.as_str())
        .collect::<Vec<_>>();
    subject_names.sort_unstable();
    assert_eq!(
        subject_names,
        vec!["price_rejects_zero", "price_round_trips"]
    );
    for subject in &index.harness_subjects {
        assert_eq!(subject.harness_kind, "registered_attribute");
        assert_eq!(subject.adapter, "exact_attribute_v1");
        assert_eq!(subject.marker, "myco::contract_test");
        assert_eq!(subject.claim, HarnessSubjectClaim::NamedFunction);
        assert_eq!(subject.selector, HarnessSelectorCapability::NamedUnexecuted);
    }

    let promoted = index
        .functions
        .iter()
        .find(|function| function.name == "price_round_trips")
        .ok_or("missing promoted fn")?;
    assert_eq!(
        promoted.source_role,
        FunctionSourceRole::RegisteredTestAttribute
    );
    assert!(
        index
            .tests
            .iter()
            .any(|test| test.name == "price_round_trips"),
        "registered attribute tests join the executable-test denominator"
    );
    assert!(
        index.functions.iter().all(|function| function.name
            != "lookalike_suffix_is_not_registered"
            || !function.source_role.is_evidence_role()),
        "prefix/suffix lookalikes must never classify"
    );
    assert!(
        index
            .tests
            .iter()
            .all(|test| !test.name.contains("lookalike")),
        "lookalikes must not join the test denominator"
    );
    Ok(())
}

#[test]
fn same_named_unrelated_import_is_ambiguous_and_fails_closed()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("attribute-ambiguous")?;
    write_workspace(
        &root,
        &[(
            "tests/ambiguous_attr.rs",
            r#"
use other_crate::contract_test;

#[contract_test]
fn ambiguous_owner() {
    assert_eq!(1, 1);
}
"#,
        )],
    )?;
    let files = [PathBuf::from("tests/ambiguous_attr.rs")];
    let registrations = [attribute_registration(
        "tests/ambiguous_attr.rs",
        "myco::contract_test",
    )];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert!(index.harness_subjects.is_empty());
    assert!(
        index
            .harness_limitations
            .iter()
            .any(|limitation| limitation.code == "ambiguous_import"),
        "{:?}",
        index.harness_limitations
    );
    let function = index
        .functions
        .iter()
        .find(|function| function.name == "ambiguous_owner")
        .ok_or("missing fn")?;
    assert_eq!(
        function.source_role,
        FunctionSourceRole::Production,
        "an ambiguous import must not classify the fn as an executable test"
    );
    Ok(())
}

#[test]
fn stale_or_foreign_registrations_grant_nothing() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("stale-registration")?;
    write_workspace(
        &root,
        &[(
            "src/lib.rs",
            "pub fn price(amount: i32) -> i32 {\n    amount * 2\n}\n",
        )],
    )?;
    // The registration names a file that is not part of this analysis
    // (stale, removed, or another package's scope).
    let registrations = [custom_target_registration("other_pkg/tests/mimic.rs")];
    let files = [PathBuf::from("src/lib.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert!(index.harness_subjects.is_empty());
    assert!(index.harness_limitations.is_empty());
    assert!(index.tests.is_empty());
    Ok(())
}

#[test]
fn bare_attribute_without_marker_import_stays_unclassified()
-> Result<(), Box<dyn std::error::Error>> {
    // `#[contract_test]` with no import binding at all cannot be established
    // to be the registered marker path; the authority names the gap
    // instead of guessing.
    let root = temp_dir("attribute-unresolved")?;
    write_workspace(
        &root,
        &[(
            "tests/unresolved_attr.rs",
            "#[contract_test]\nfn unresolved_owner() {\n    assert_eq!(1, 1);\n}\n",
        )],
    )?;
    let files = [PathBuf::from("tests/unresolved_attr.rs")];
    let registrations = [attribute_registration(
        "tests/unresolved_attr.rs",
        "myco::contract_test",
    )];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert!(index.harness_subjects.is_empty());
    assert!(
        index
            .harness_limitations
            .iter()
            .any(|limitation| limitation.code == "unresolved_marker_import"),
        "{:?}",
        index.harness_limitations
    );
    Ok(())
}

#[test]
fn cached_index_applies_registrations_identically() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("cached-harness")?;
    let file = PathBuf::from("tests/cached_mimic.rs");
    let source = br#"
use libtest_mimic::Trial;

fn trials() -> Vec<Trial> {
    vec![Trial::test("cached_case", || Ok(()))]
}

#[test]
fn inert_test_in_harness_target() {
    assert_eq!(1, 1);
}

fn main() {
    let arguments = libtest_mimic::Arguments::from_args();
    let all_trials = trials();
    libtest_mimic::run(&arguments, &all_trials);
}
"#
    .to_vec();
    let files = [(file.clone(), source)];
    fs::write(
        root.0.join("Cargo.toml"),
        "[package]\nname = 'harness-fixture'\nversion = '0.1.0'\nedition = '2024'\n\n[workspace]\n",
    )?;
    declare_harness_false_target(&root, "cached_mimic", "tests/cached_mimic.rs")?;
    let registrations = [custom_target_registration("tests/cached_mimic.rs")];

    let cold = crate::analysis::facts::build_index_from_loaded_files_with_cache_and_test_harnesses(
        &root.0,
        &files,
        &registrations,
    )?;
    let warm = crate::analysis::facts::build_index_from_loaded_files_with_cache_and_test_harnesses(
        &root.0,
        &files,
        &registrations,
    )?;
    for index in [&cold.index, &warm.index] {
        assert_eq!(
            index
                .harness_subjects
                .iter()
                .map(|subject| subject.name.as_str())
                .collect::<Vec<_>>(),
            vec!["cached_case"]
        );
        // Only the adapter-established trial subject enters the executable test
        // denominator; the inert #[test] is demoted on both cold and warm runs (#3602).
        assert_eq!(index.tests.len(), 1);
        assert_eq!(index.tests[0].name, "cached_case");
        let file_facts = index.files.get(&file).ok_or("file facts missing")?;
        assert_eq!(file_facts.tests.len(), 1);
        assert_eq!(file_facts.tests[0].name, "cached_case");
        let inert = index
            .functions
            .iter()
            .find(|f| f.name == "inert_test_in_harness_target")
            .ok_or("inert fn present")?;
        assert_eq!(inert.source_role, FunctionSourceRole::HarnessHelper);
    }
    assert_eq!(cold.file_fact_cache.hits, 0);
    assert_eq!(warm.file_fact_cache.hits, 1);
    Ok(())
}

#[test]
fn built_in_test_attributes_precede_a_registered_marker() -> Result<(), Box<dyn std::error::Error>>
{
    // A fn carrying BOTH `#[test]` and the registered attribute must not
    // register twice: the built-in #3499 family keeps precedence.
    let root = temp_dir("builtin-precedence")?;
    write_workspace(
        &root,
        &[(
            "tests/both_attrs.rs",
            r#"
use myco::contract_test;

#[test]
#[contract_test]
fn double_attributed() {
    assert_eq!(1, 1);
}
"#,
        )],
    )?;
    let files = [PathBuf::from("tests/both_attrs.rs")];
    let registrations = [
        custom_target_registration("tests/other.rs"),
        attribute_registration("tests/both_attrs.rs", "myco::contract_test"),
    ];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert!(
        index.harness_subjects.is_empty(),
        "the built-in attribute already owns the fn"
    );
    assert_eq!(
        index
            .tests
            .iter()
            .filter(|test| test.name == "double_attributed")
            .count(),
        1,
        "exactly one executable test fact, from the built-in family"
    );
    let function = index
        .functions
        .iter()
        .find(|function| function.name == "double_attributed")
        .ok_or("missing fn")?;
    assert_eq!(function.source_role, FunctionSourceRole::TestAttribute);
    Ok(())
}

#[test]
fn demote_harness_target_functions_drops_differently_named_test_facts()
-> Result<(), Box<dyn std::error::Error>> {
    // Discriminating regression test for #3602 / chatgpt-codex review finding:
    // Verify that demote_harness_target_functions drops TestFacts overlapping a
    // demoted function's span even when the TestFact's name does NOT match the
    // function's name (which name-based demotion would miss).
    let target = Path::new("tests/custom_harness.rs");
    let mut index = RustIndex::default();
    let function = FunctionFact {
        id: crate::domain::SymbolId("custom_harness::test_fn".to_string()),
        name: "source_function_name".to_string(),
        file: target.to_path_buf(),
        start_line: 10,
        end_line: 20,
        body: "assert_eq!(1, 1);".to_string(),
        calls: Vec::new(),
        returns: Vec::new(),
        literals: Vec::new(),
        source_role: FunctionSourceRole::TestAttribute,
        attrs: vec!["#[test]".to_string()],
        nested_fn_names: Vec::new(),
        let_bindings: Vec::new(),
    };
    let test_fact = TestFact {
        name: "differently_named_test_case".to_string(),
        file: target.to_path_buf(),
        start_line: 10,
        end_line: 20,
        body: "assert_eq!(1, 1);".to_string(),
        calls: Vec::new(),
        assertions: Vec::new(),
        literals: Vec::new(),
        attrs: vec!["#[test]".to_string()],
        nested_fn_names: Vec::new(),
        let_bindings: Vec::new(),
    };
    let file_facts = FileFacts {
        path: target.to_path_buf(),
        functions: vec![function.clone()],
        tests: vec![test_fact.clone()],
        ..Default::default()
    };
    index.files.insert(target.to_path_buf(), file_facts.clone());
    index.functions.push(function);
    index.tests.push(test_fact);

    demote_harness_target_functions(&mut index, target);

    // Span overlap drops the test fact despite the name mismatch.
    assert!(
        index.tests.is_empty(),
        "differently-named TestFact overlapping demoted span must be dropped"
    );
    let facts = index.files.get(target).ok_or("file facts missing")?;
    assert!(
        facts.tests.is_empty(),
        "file-level differently-named TestFact must be dropped"
    );
    assert_eq!(
        facts.functions[0].source_role,
        FunctionSourceRole::HarnessHelper
    );
    assert_eq!(
        index.functions[0].source_role,
        FunctionSourceRole::HarnessHelper
    );
    Ok(())
}

/// #3608: a `custom_harness` registration whose target path does not
/// match any Cargo `[[test]]` target records the conflict and degrades to
/// per-function behavior — the misdeclared file keeps its ordinary
/// classification (a `#[test]` fn stays an executable test; nothing is
/// demoted and no trial subjects appear).
#[test]
fn misdeclared_target_keeps_per_function_roles_and_records_the_conflict()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("misdeclared-target")?;
    write_workspace(
        &root,
        &[
            // The lib target keeps the fixture package cargo-valid; it
            // declares nothing for the misdeclared file.
            ("src/lib.rs", ""),
            (
                "src/misdeclared.rs",
                r#"
use libtest_mimic::Trial;

fn trials() -> Vec<Trial> {
    vec![Trial::test("ghost_trial", || Ok(()))]
}

#[test]
fn ordinary_libtest_still_runs_here() {
    assert_eq!(1, 1);
}
"#,
            ),
        ],
    )?;
    // The manifest declares nothing for src/misdeclared.rs: the
    // registration's target is missing from Cargo metadata.
    let files = [PathBuf::from("src/misdeclared.rs")];
    let registrations = [custom_target_registration("src/misdeclared.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert!(
        index.harness_subjects.is_empty(),
        "a misdeclared target must not establish trial subjects: {:?}",
        index.harness_subjects
    );
    assert!(
        index
            .tests
            .iter()
            .any(|test| test.name == "ordinary_libtest_still_runs_here"),
        "the ordinary #[test] keeps running: the misdeclared file is not demoted"
    );
    let function = index
        .functions
        .iter()
        .find(|function| function.name == "ordinary_libtest_still_runs_here")
        .ok_or("missing fn")?;
    assert_eq!(
        function.source_role,
        FunctionSourceRole::TestAttribute,
        "per-function behavior is retained: no file-wide demotion"
    );
    let conflict = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "target_not_declared")
        .ok_or("missing target_not_declared limitation")?;
    assert!(
        conflict.detail.contains("src/misdeclared.rs"),
        "the conflict names the misdeclared target: {:?}",
        conflict.detail
    );
    Ok(())
}

/// #3608: a target known to Cargo only through autodiscovery still has
/// `harness = true` (Cargo's default), so a `custom_harness` registration
/// on the conventional layout records the harness-flag conflict and the
/// file keeps its ordinary libtest behavior.
#[test]
fn harness_enabled_target_records_conflict_and_keeps_executable_tests()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("harness-enabled-target")?;
    write_workspace(
        &root,
        &[(
            "tests/auto_discovered.rs",
            r#"
use libtest_mimic::Trial;

fn trials() -> Vec<Trial> {
    vec![Trial::test("never_runs_under_libtest", || Ok(()))]
}

#[test]
fn ordinary_libtest_still_runs_here() {
    assert_eq!(1, 1);
}
"#,
        )],
    )?;
    // No [[test]] entry: tests/auto_discovered.rs is autodiscovered with
    // the harness enabled.
    let files = [PathBuf::from("tests/auto_discovered.rs")];
    let registrations = [custom_target_registration("tests/auto_discovered.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert!(index.harness_subjects.is_empty());
    assert_eq!(
        index
            .tests
            .iter()
            .map(|test| test.name.as_str())
            .collect::<Vec<_>>(),
        vec!["ordinary_libtest_still_runs_here"],
        "the autodiscovered #[test] keeps running; no demotion happened"
    );
    let conflict = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "harness_flag_conflict")
        .ok_or("missing harness_flag_conflict limitation")?;
    assert!(
        conflict.detail.contains("harness = true"),
        "the conflict names the still-enabled harness flag: {:?}",
        conflict.detail
    );
    Ok(())
}

/// #3608: an explicit `harness = false` declaration on the conventional
/// `tests/` layout (a name-only `[[test]]` entry) confirms the
/// registration's premise — the adapter runs and the trial subject is
/// established with no conflict recorded.
#[test]
fn explicit_harness_false_declaration_on_conventional_layout_is_accepted()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("declared-conventional")?;
    write_workspace(
        &root,
        &[(
            "tests/suite.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn trials() -> Vec<Trial> {
    vec![Trial::test("declared_case", || Ok(()))]
}
"#,
                "trials()",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "suite", "tests/suite.rs")?;
    let files = [PathBuf::from("tests/suite.rs")];
    let registrations = [custom_target_registration("tests/suite.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert_eq!(
        index
            .harness_subjects
            .iter()
            .map(|subject| subject.name.as_str())
            .collect::<Vec<_>>(),
        vec!["declared_case"]
    );
    assert!(index.harness_limitations.is_empty());
    Ok(())
}

/// #3608: without any readable owning manifest, the `harness = false`
/// premise cannot be established from Cargo metadata; the registration
/// records the typed limitation and grants nothing.
#[test]
fn unreadable_manifest_records_manifest_unavailable_and_grants_nothing()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("manifest-unavailable")?;
    fs::create_dir_all(root.0.join("tests"))?;
    fs::write(
        root.0.join("tests/orphan_mimic.rs"),
        "use libtest_mimic::Trial;\n\nfn trials() -> Vec<Trial> {\n    vec![Trial::test(\"unverifiable_case\", || Ok(()))]\n}\n",
    )?;
    // No Cargo.toml exists at the package root: the premise cannot be
    // established from Cargo metadata.
    let files = [PathBuf::from("tests/orphan_mimic.rs")];
    let registrations = [custom_target_registration("tests/orphan_mimic.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert!(index.harness_subjects.is_empty());
    assert!(
        index
            .harness_limitations
            .iter()
            .any(|limitation| limitation.code == "manifest_unavailable"
                && limitation.detail.contains("tests/orphan_mimic.rs")),
        "{:?}",
        index.harness_limitations
    );
    Ok(())
}

/// #3608 review: a `custom_harness` target declared `harness = false`
/// outside the conventional source directories (`qa/mimic.rs`) resolves
/// ownership by manifest presence, so the adapter runs and the trial
/// subject is established.
#[test]
fn nonconventional_directory_target_declared_harness_false_is_accepted()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("nonconventional-target")?;
    write_workspace(
        &root,
        &[(
            "qa/mimic.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn trials() -> Vec<Trial> {
    vec![Trial::test("qa_case", || Ok(()))]
}
"#,
                "trials()",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "mimic", "qa/mimic.rs")?;
    let files = [PathBuf::from("qa/mimic.rs")];
    let registrations = [custom_target_registration("qa/mimic.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert_eq!(
        index
            .harness_subjects
            .iter()
            .map(|subject| subject.name.as_str())
            .collect::<Vec<_>>(),
        vec!["qa_case"]
    );
    assert!(index.harness_limitations.is_empty());
    Ok(())
}

/// #3608 review: a readable but malformed owning manifest cannot
/// establish the registration's premise — the registry records the
/// `manifest_unavailable` limitation instead of a target typo, and the
/// target keeps its ordinary per-function behavior.
#[test]
fn malformed_manifest_records_manifest_unavailable_and_keeps_per_function_roles()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("malformed-manifest")?;
    fs::create_dir_all(root.0.join("src"))?;
    fs::write(root.0.join("Cargo.toml"), "not [ valid toml")?;
    fs::write(
        root.0.join("src/malformed_mimic.rs"),
        "use libtest_mimic::Trial;\n\nfn trials() -> Vec<Trial> {\n    vec![Trial::test(\"ghost_case\", || Ok(()))]\n}\n\n#[test]\nfn ordinary_libtest_still_runs_here() {\n    assert_eq!(1, 1);\n}\n",
    )?;
    let files = [PathBuf::from("src/malformed_mimic.rs")];
    let registrations = [custom_target_registration("src/malformed_mimic.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert!(
        index.harness_subjects.is_empty(),
        "{:?}",
        index.harness_subjects
    );
    assert!(
        index
            .tests
            .iter()
            .any(|test| test.name == "ordinary_libtest_still_runs_here"),
        "the ordinary #[test] keeps running: nothing is demoted"
    );
    let conflict = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "manifest_unavailable")
        .ok_or("missing manifest_unavailable limitation")?;
    assert!(
        conflict.detail.contains("src/malformed_mimic.rs"),
        "{:?}",
        conflict.detail
    );
    Ok(())
}

/// #3608 review / cache invalidation: the per-file fact cache must not
/// bypass the Cargo target metadata validation. The cold run records the
/// conflict limitation; the warm run (same inputs, cache hit) still
/// reaches the validation and records the same limitation — the registry
/// applies after cache retrieval on every build.
#[test]
fn warm_file_fact_cache_still_reaches_cargo_target_validation()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("warm-validation")?;
    fs::create_dir_all(root.0.join("src"))?;
    // The package carries a lib target so cargo metadata resolves; it
    // deliberately declares nothing for the target file: the
    // registration below is misdeclared.
    fs::write(root.0.join("src/lib.rs"), "")?;
    fs::write(
        root.0.join("Cargo.toml"),
        "[package]\nname = 'harness-fixture'\nversion = '0.1.0'\nedition = '2024'\n\n[workspace]\n",
    )?;
    let file = PathBuf::from("src/misdeclared_mimic.rs");
    let source = br#"
use libtest_mimic::Trial;

fn trials() -> Vec<Trial> {
    vec![Trial::test("ghost_case", || Ok(()))]
}

#[test]
fn ordinary_libtest_still_runs_here() {
    assert_eq!(1, 1);
}
"#
    .to_vec();
    let files = [(file.clone(), source)];
    let registrations = [custom_target_registration("src/misdeclared_mimic.rs")];

    let cold = crate::analysis::facts::build_index_from_loaded_files_with_cache_and_test_harnesses(
        &root.0,
        &files,
        &registrations,
    )?;
    let warm = crate::analysis::facts::build_index_from_loaded_files_with_cache_and_test_harnesses(
        &root.0,
        &files,
        &registrations,
    )?;
    assert_eq!(
        warm.file_fact_cache.hits, 1,
        "the warm run must be a cache hit"
    );
    for (label, index) in [("cold", &cold.index), ("warm", &warm.index)] {
        assert!(
            index.harness_subjects.is_empty(),
            "{label}: a misdeclared target establishes no subjects"
        );
        assert!(
            index
                .harness_limitations
                .iter()
                .any(|limitation| limitation.code == "target_not_declared"
                    && limitation.detail.contains("src/misdeclared_mimic.rs")),
            "{label}: the conflict limitation must be recorded on cache hits too: {:?}",
            index.harness_limitations
        );
        let function = index
            .functions
            .iter()
            .find(|function| function.name == "ordinary_libtest_still_runs_here")
            .ok_or("missing fn")?;
        assert_eq!(
            function.source_role,
            FunctionSourceRole::TestAttribute,
            "{label}: per-function behavior is retained on the warm path too"
        );
    }
    Ok(())
}

/// #3608 review (Fe25): declaration-driven ownership — a workspace-root
/// package's explicit `harness = false` declaration claims its target even
/// when the target sits below a directory containing another (undeclaring)
/// Cargo.toml; the trial subject is established with no conflict recorded.
#[test]
fn root_declaration_claims_a_target_below_a_nested_manifest()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("root-declared-nested")?;
    write_workspace(
        &root,
        &[(
            "below/nested/manifest/dir/mimic.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn trials() -> Vec<Trial> {
    vec![Trial::test("root_claimed_case", || Ok(()))]
}
"#,
                "trials()",
            ),
        )],
    )?;
    // The nested manifest directory declares nothing for the target.
    fs::create_dir_all(root.0.join("below/nested/manifest"))?;
    fs::write(
        root.0.join("below/nested/manifest/Cargo.toml"),
        "[package]\nname = 'nested'\nversion = '0.1.0'\nedition = '2024'\n",
    )?;
    declare_harness_false_target(&root, "mimic", "below/nested/manifest/dir/mimic.rs")?;
    let files = [PathBuf::from("below/nested/manifest/dir/mimic.rs")];
    let registrations = [custom_target_registration(
        "below/nested/manifest/dir/mimic.rs",
    )];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert_eq!(
        index
            .harness_subjects
            .iter()
            .map(|subject| subject.name.as_str())
            .collect::<Vec<_>>(),
        vec!["root_claimed_case"],
        "the root declaration claims the target across the nested manifest"
    );
    assert!(index.harness_limitations.is_empty());
    Ok(())
}

/// #3608 review round four (Gajt): Cargo permits an explicit `[[test]]`
/// path to resolve outside the declaring package's directory — a sibling
/// package's `../../shared/mimic.rs` harness = false declaration claims
/// the shared target, so the adapter establishes its trial subjects.
#[test]
fn shared_target_declared_from_a_sibling_package_is_accepted()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("shared-target")?;
    write_workspace(
        &root,
        &[
            // The root package's lib target keeps the fixture
            // cargo-valid alongside its declared member.
            ("src/lib.rs", ""),
            (
                "shared/mimic.rs",
                &with_harness_main(
                    r#"
use libtest_mimic::Trial;

fn trials() -> Vec<Trial> {
    vec![Trial::test("shared_case", || Ok(()))]
}
"#,
                    "trials()",
                ),
            ),
        ],
    )?;
    // The declaring package sits beside shared/ as a declared workspace
    // member; its target path escapes its own directory. The lib.rs
    // target file keeps the member package cargo-valid.
    fs::create_dir_all(root.0.join("crates/a/src"))?;
    fs::write(root.0.join("crates/a/src/lib.rs"), "")?;
    fs::write(
        root.0.join("crates/a/Cargo.toml"),
        "[package]\nname = 'a'\nversion = '0.1.0'\nedition = '2024'\n\n\
         [[test]]\nname = 'mimic'\npath = '../../shared/mimic.rs'\nharness = false\n",
    )?;
    fs::write(
        root.0.join("Cargo.toml"),
        "[package]\nname = 'harness-fixture'\nversion = '0.1.0'\nedition = '2024'\n\n\
         [workspace]\nmembers = ['crates/a']\n",
    )?;
    let files = [PathBuf::from("shared/mimic.rs")];
    let registrations = [custom_target_registration("shared/mimic.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert_eq!(
        index
            .harness_subjects
            .iter()
            .map(|subject| subject.name.as_str())
            .collect::<Vec<_>>(),
        vec!["shared_case"],
        "the sibling package's declaration claims the shared target"
    );
    assert!(index.harness_limitations.is_empty());
    Ok(())
}

/// #3604/#3636: the adapter's discovery scan is syntactic and bounded by
/// the registered target, so a trial constructor that never reaches the
/// harness's run entry point — one inside an `if false` branch, one
/// built in a helper nothing calls, in a target with no run entry call
/// at all — still claims `HarnessSubjectClaim::NamedInvocation` together
/// with the dead constructor's calls and oracles. The #3636 reachability
/// authority closes the #3635 over-credit boundary from the other side:
/// with no run entry call in the target, the construction cannot reach a
/// run argument, so the subject keeps its syntactic claim and its
/// subject fact but its `TestFact` does not enter the executable-test
/// denominator, and a per-trial `registration_unreachable` limitation
/// names it. The claim stays uniform (no per-subject reachability
/// field); the exclusion is disclosed by the typed limitation.
#[test]
fn given_dead_construction_then_subjects_still_claim_named_invocation()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("dead-construction")?;
    write_workspace(
        &root,
        &[(
            "tests/dead_construction.rs",
            r#"
use libtest_mimic::Trial;

fn production_call(value: u16) -> u16 { value }

fn dead_branch_trials() -> Vec<Trial> {
    if false {
        return vec![Trial::test("dead_branch_trial", || {
            assert_eq!(production_call(1), 1);
        })];
    }
    Vec::new()
}

fn never_called_trials() -> Vec<Trial> {
    vec![Trial::test("unused_helper_trial", || {
        assert_eq!(production_call(2), 2);
    })]
}
"#,
        )],
    )?;
    declare_harness_false_target(&root, "dead_construction", "tests/dead_construction.rs")?;
    let files = [PathBuf::from("tests/dead_construction.rs")];
    let registrations = [custom_target_registration("tests/dead_construction.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    // Both dead constructors still claim a subject, under the one
    // syntactic claim: a named invocation in the registered target.
    let dead_branch = index
        .harness_subjects
        .iter()
        .find(|subject| subject.name == "dead_branch_trial")
        .ok_or("dead_branch_trial subject missing")?;
    let unused_helper = index
        .harness_subjects
        .iter()
        .find(|subject| subject.name == "unused_helper_trial")
        .ok_or("unused_helper_trial subject missing")?;
    for subject in [dead_branch, unused_helper] {
        assert_eq!(
            subject.claim,
            HarnessSubjectClaim::NamedInvocation,
            "the syntactic claim covers dead construction too; the boundary is the claim's documented reachability limit"
        );
        assert_eq!(subject.claim.as_str(), "named_invocation");
    }

    // The dead constructors' own evidence still joins the subject facts.
    for subject in [dead_branch, unused_helper] {
        assert!(
            subject
                .assertions
                .iter()
                .any(|oracle| oracle.text.starts_with("assert_eq!")),
            "the dead constructor's oracle still joins the subject: {:?}",
            subject.assertions
        );
        assert!(
            subject
                .calls
                .iter()
                .any(|call| call.name == "production_call"),
            "the dead constructor's call still joins the subject: {:?}",
            subject.calls
        );
    }

    // #3636: with no run entry call in the target, neither construction
    // can reach a run argument, so neither enters the executable-test
    // denominator — while both subject facts stay.
    assert!(
        index
            .tests
            .iter()
            .all(|test| !test.file.ends_with(Path::new("tests/dead_construction.rs"))),
        "dead constructions must leave the executable-test denominator: {:?}",
        index
            .tests
            .iter()
            .filter(|test| test.file.ends_with(Path::new("tests/dead_construction.rs")))
            .collect::<Vec<_>>()
    );

    // The exclusion is named per trial: one typed limitation each,
    // recording that the run entry point is absent.
    let exclusions: Vec<_> = index
        .harness_limitations
        .iter()
        .filter(|limitation| limitation.code == "registration_unreachable")
        .collect();
    assert_eq!(exclusions.len(), 2, "{:?}", index.harness_limitations);
    for (name, line) in [("dead_branch_trial", 8), ("unused_helper_trial", 16)] {
        let limitation = exclusions
            .iter()
            .find(|limitation| limitation.detail.contains(&format!("trial `{name}`")))
            .ok_or_else(|| {
                format!(
                    "missing exclusion for {name}: {:?}",
                    index.harness_limitations
                )
            })?;
        assert_eq!(limitation.line, line, "{limitation:?}");
        assert!(
            limitation
                .detail
                .contains("no call to the registered harness run entry point"),
            "{limitation:?}"
        );
    }
    assert!(
        index
            .harness_limitations
            .iter()
            .all(|limitation| limitation.code != "registration_reachability_unknown"),
        "an absent run entry is a proof, not an unknown: {:?}",
        index.harness_limitations
    );
    Ok(())
}

/// #3639 review (devin): a qualified call matching the marker suffix
/// inside a longer foreign path (`wrapper::libtest_mimic::run`) can
/// never anchor the entry. Combined with an aliased real entry
/// receiving the trial, the trial must stay admitted as unknown — not
/// excluded by the foreign call's (empty) resolved argument.
#[test]
fn foreign_suffix_run_call_cannot_exclude_trials() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("foreign-suffix")?;
    write_workspace(
        &root,
        &[(
            "tests/foreign_suffix.rs",
            r#"
use libtest_mimic::{run as execute, Arguments, Trial};

fn trials() -> Vec<Trial> {
    vec![Trial::test("foreign_suffix_trial", || Ok(()))]
}

fn main() {
    let arguments = Arguments::from_args();
    wrapper::libtest_mimic::run(&arguments, &Vec::new());
    execute(&arguments, &trials());
}
"#,
        )],
    )?;
    declare_harness_false_target(&root, "foreign_suffix", "tests/foreign_suffix.rs")?;
    let files = [PathBuf::from("tests/foreign_suffix.rs")];
    let registrations = [custom_target_registration("tests/foreign_suffix.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    assert_eq!(
        index
            .tests
            .iter()
            .filter(|test| test.file.ends_with(Path::new("tests/foreign_suffix.rs")))
            .count(),
        1,
        "the trial must not be excluded by the foreign call: {:?}",
        index.harness_limitations
    );
    let unknown = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_reachability_unknown")
        .ok_or_else(|| {
            format!(
                "missing unknown disclosure: {:?}",
                index.harness_limitations
            )
        })?;
    assert!(
        unknown.detail.contains("foreign_suffix_trial"),
        "{unknown:?}"
    );
    assert!(
        index
            .harness_limitations
            .iter()
            .all(|limitation| limitation.code != "registration_unreachable"),
        "{:?}",
        index.harness_limitations
    );
    Ok(())
}

/// #3639 review (devin): a local closure named `run` shadows the
/// imported harness entry — those calls cannot anchor, so the trials
/// stay admitted under the unknown disclosure instead of resolving
/// against the shadow.
#[test]
fn shadowed_run_closure_keeps_trials_with_unknown_disclosure()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("shadow-closure")?;
    write_workspace(
        &root,
        &[(
            "tests/shadow_closure.rs",
            r#"
use libtest_mimic::{run, Arguments, Trial};

fn trials() -> Vec<Trial> {
    vec![Trial::test("shadowed_closure_trial", || Ok(()))]
}

fn main() {
    let run = |arguments: &Arguments, all_trials: &[Trial]| {
        let _ = (arguments, all_trials);
    };
    let arguments = Arguments::from_args();
    run(&arguments, &trials());
}
"#,
        )],
    )?;
    declare_harness_false_target(&root, "shadow_closure", "tests/shadow_closure.rs")?;
    let files = [PathBuf::from("tests/shadow_closure.rs")];
    let registrations = [custom_target_registration("tests/shadow_closure.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    assert_eq!(
        index
            .tests
            .iter()
            .filter(|test| test.file.ends_with(Path::new("tests/shadow_closure.rs")))
            .count(),
        1,
        "the shadowed call must not resolve the trial: {:?}",
        index.harness_limitations
    );
    let unknown = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_reachability_unknown")
        .ok_or_else(|| {
            format!(
                "missing unknown disclosure: {:?}",
                index.harness_limitations
            )
        })?;
    assert!(
        unknown.detail.contains("shadowed_closure_trial"),
        "{unknown:?}"
    );
    assert!(
        index
            .harness_limitations
            .iter()
            .all(|limitation| limitation.code != "registration_unreachable"),
        "{:?}",
        index.harness_limitations
    );
    Ok(())
}

/// #3639 review (devin): a function parameter named `run` shadows the
/// import the same way — unknown disclosure, never an anchor.
#[test]
fn shadowed_run_parameter_keeps_trials_with_unknown_disclosure()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("shadow-param")?;
    write_workspace(
        &root,
        &[(
            "tests/shadow_param.rs",
            r#"
use libtest_mimic::{run, Arguments, Trial};

fn trials() -> Vec<Trial> {
    vec![Trial::test("shadowed_param_trial", || Ok(()))]
}

fn main_with(run: fn(&Arguments, &[Trial])) {
    let arguments = Arguments::from_args();
    run(&arguments, &trials());
}

fn main() {
    main_with(|_, _| {});
}
"#,
        )],
    )?;
    declare_harness_false_target(&root, "shadow_param", "tests/shadow_param.rs")?;
    let files = [PathBuf::from("tests/shadow_param.rs")];
    let registrations = [custom_target_registration("tests/shadow_param.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    assert_eq!(
        index
            .tests
            .iter()
            .filter(|test| test.file.ends_with(Path::new("tests/shadow_param.rs")))
            .count(),
        1,
        "the shadowed parameter call must not resolve the trial: {:?}",
        index.harness_limitations
    );
    let unknown = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_reachability_unknown")
        .ok_or_else(|| {
            format!(
                "missing unknown disclosure: {:?}",
                index.harness_limitations
            )
        })?;
    assert!(
        unknown.detail.contains("shadowed_param_trial"),
        "{unknown:?}"
    );
    Ok(())
}

/// #3639 review (devin): trivia between the final path segment and the
/// call parenthesis does not hide the entry — both the bare and the
/// qualified spellings anchor across whitespace and comments, so the
/// trials they resolve stay reachable with no reachability limitation.
#[test]
fn trivia_before_call_parenthesis_still_anchors() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("trivia-paren")?;
    write_workspace(
        &root,
        &[(
            "tests/trivia_paren.rs",
            r#"
use libtest_mimic::{run, Trial};

fn dead_elsewhere() -> Vec<Trial> {
    vec![Trial::test("trivia_dead_trial", || Ok(()))]
}

fn main() {
    let arguments = libtest_mimic::Arguments::from_args();
    libtest_mimic::run /* trials */ (&arguments, &vec![
        Trial::test("trivia_qualified_trial", || Ok(())),
    ]);
    run /* bare */ (&arguments, &vec![
        Trial::test("trivia_bare_trial", || Ok(())),
    ]);
}
"#,
        )],
    )?;
    declare_harness_false_target(&root, "trivia_paren", "tests/trivia_paren.rs")?;
    let files = [PathBuf::from("tests/trivia_paren.rs")];
    let registrations = [custom_target_registration("tests/trivia_paren.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    let mut admitted: Vec<_> = index
        .tests
        .iter()
        .filter(|test| test.file.ends_with(Path::new("tests/trivia_paren.rs")))
        .map(|test| test.name.as_str())
        .collect();
    admitted.sort_unstable();
    assert_eq!(
        admitted,
        vec!["trivia_bare_trial", "trivia_qualified_trial"],
        "trivia before the parenthesis anchors both spellings: {:?}",
        index.harness_limitations
    );
    let exclusion = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_unreachable")
        .ok_or_else(|| format!("missing exclusion: {:?}", index.harness_limitations))?;
    assert!(
        exclusion.detail.contains("trivia_dead_trial"),
        "{exclusion:?}"
    );
    assert!(
        index
            .harness_limitations
            .iter()
            .all(|limitation| limitation.code != "registration_reachability_unknown"),
        "trivia-tolerant anchoring is complete, not unknown: {:?}",
        index.harness_limitations
    );
    Ok(())
}

/// #3639 review (devin): a trial built by a nested run call inside
/// another trial's callback is a top-level element of ITS argument —
/// the global nesting of the outer callback does not exclude it.
#[test]
fn nested_run_call_inner_trial_is_reachable() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("nested-run")?;
    write_workspace(
        &root,
        &[(
            "tests/nested_run.rs",
            r#"
use libtest_mimic::{Arguments, Trial};

fn dead_elsewhere() -> Vec<Trial> {
    vec![Trial::test("nested_run_dead_trial", || Ok(()))]
}

fn main() {
    let arguments = Arguments::from_args();
    libtest_mimic::run(&arguments, &vec![Trial::test("outer_trial", || {
        let inner_arguments = Arguments::from_args();
        libtest_mimic::run(&inner_arguments, &vec![Trial::test("inner_trial", || Ok(()))]);
    })]);
}
"#,
        )],
    )?;
    declare_harness_false_target(&root, "nested_run", "tests/nested_run.rs")?;
    let files = [PathBuf::from("tests/nested_run.rs")];
    let registrations = [custom_target_registration("tests/nested_run.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    let mut admitted: Vec<_> = index
        .tests
        .iter()
        .filter(|test| test.file.ends_with(Path::new("tests/nested_run.rs")))
        .map(|test| test.name.as_str())
        .collect();
    admitted.sort_unstable();
    assert_eq!(
        admitted,
        vec!["inner_trial", "outer_trial"],
        "the nested run call's own element is reachable: {:?}",
        index.harness_limitations
    );
    let exclusion = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_unreachable")
        .ok_or_else(|| format!("missing exclusion: {:?}", index.harness_limitations))?;
    assert!(
        exclusion.detail.contains("nested_run_dead_trial"),
        "{exclusion:?}"
    );
    assert!(
        index
            .harness_limitations
            .iter()
            .all(|limitation| limitation.code != "registration_reachability_unknown"),
        "{:?}",
        index.harness_limitations
    );
    Ok(())
}

/// #3639 review (devin): an immutable binding to a mutable reference
/// that is later `push`ed before the run call is not fully resolved by
/// its initializer — the pushed trial stays admitted under the unknown
/// disclosure instead of being excluded.
#[test]
fn mut_reference_push_keeps_pushed_trial_admitted() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("mut-push")?;
    write_workspace(
        &root,
        &[(
            "tests/mut_push.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn build_trials() -> Vec<Trial> {
    let trials = &mut vec![Trial::test("mut_push_initial_trial", || Ok(()))];
    trials.push(Trial::test("mut_push_late_trial", || Ok(())));
    trials.clone()
}
"#,
                "build_trials()",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "mut_push", "tests/mut_push.rs")?;
    let files = [PathBuf::from("tests/mut_push.rs")];
    let registrations = [custom_target_registration("tests/mut_push.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    assert_eq!(
        index
            .tests
            .iter()
            .filter(|test| test.file.ends_with(Path::new("tests/mut_push.rs")))
            .count(),
        2,
        "both trials stay admitted when a push intervenes: {:?}",
        index.harness_limitations
    );
    let unknown = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_reachability_unknown")
        .ok_or_else(|| {
            format!(
                "missing unknown disclosure: {:?}",
                index.harness_limitations
            )
        })?;
    assert!(
        unknown.detail.contains("mut_push_late_trial"),
        "{unknown:?}"
    );
    Ok(())
}

/// #3639 review (devin): an unsupported wrapper expression cannot
/// guarantee its contained trials reach the run argument — the wrapper
/// may discard or replace them — so the contained trial resolves to
/// unknown (admitted with disclosure), never silently reachable.
#[test]
fn unsupported_wrapper_trial_resolves_to_unknown() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("wrapper-unknown")?;
    write_workspace(
        &root,
        &[(
            "tests/wrapper_unknown.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn keep_first(trials: Vec<Trial>) -> Trial {
    trials.into_iter().next().expect("some trial")
}
"#,
                "vec![keep_first(vec![Trial::test(\"wrapper_trial\", || Ok(()))])]",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "wrapper_unknown", "tests/wrapper_unknown.rs")?;
    let files = [PathBuf::from("tests/wrapper_unknown.rs")];
    let registrations = [custom_target_registration("tests/wrapper_unknown.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    assert_eq!(
        index
            .tests
            .iter()
            .filter(|test| test.file.ends_with(Path::new("tests/wrapper_unknown.rs")))
            .count(),
        1,
        "the wrapped trial stays admitted: {:?}",
        index.harness_limitations
    );
    let unknown = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_reachability_unknown")
        .ok_or_else(|| {
            format!(
                "missing unknown disclosure: {:?}",
                index.harness_limitations
            )
        })?;
    assert!(unknown.detail.contains("wrapper_trial"), "{unknown:?}");
    Ok(())
}

/// #3639 review (coderabbit): a method-position `suite.run(..)` is not
/// entry evidence in any shape — the foreign fallback must not record
/// it, so a target whose only run-like call is a method call still
/// concludes run-entry absence.
#[test]
fn method_position_run_call_does_not_block_exclusion() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("method-run")?;
    write_workspace(
        &root,
        &[(
            "tests/method_run.rs",
            r#"
use libtest_mimic::Trial;

struct Runner;

impl Runner {
    fn run(&self, _value: u16) {}
}

fn dead_elsewhere() -> Vec<Trial> {
    vec![Trial::test("method_run_dead_trial", || Ok(()))]
}

fn main() {
    let runner = Runner;
    runner.run(1);
}
"#,
        )],
    )?;
    declare_harness_false_target(&root, "method_run", "tests/method_run.rs")?;
    let files = [PathBuf::from("tests/method_run.rs")];
    let registrations = [custom_target_registration("tests/method_run.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    assert!(
        index
            .tests
            .iter()
            .all(|test| !test.file.ends_with(Path::new("tests/method_run.rs"))),
        "a method-only call shape leaves the entry absent: {:?}",
        index.harness_limitations
    );
    assert!(
        index
            .harness_limitations
            .iter()
            .all(|limitation| limitation.code != "registration_reachability_unknown"),
        "method receivers are not entry evidence: {:?}",
        index.harness_limitations
    );
    Ok(())
}

/// #3639 review (devin): an aliased run entry (`use
/// libtest_mimic::run as execute;` plus an `execute(..)` call) cannot be
/// anchored, so it must never conclude unreachability — the trials stay
/// in the executable-test denominator under the aggregate unknown
/// disclosure.
#[test]
fn aliased_run_entry_keeps_trials_with_unknown_disclosure() -> Result<(), Box<dyn std::error::Error>>
{
    let root = temp_dir("alias-entry")?;
    write_workspace(
        &root,
        &[(
            "tests/alias_entry.rs",
            r#"
use libtest_mimic::{run as execute, Arguments, Trial};

fn trials() -> Vec<Trial> {
    vec![Trial::test("aliased_entry_trial", || Ok(()))]
}

fn main() {
    let arguments = Arguments::from_args();
    execute(&arguments, &trials());
}
"#,
        )],
    )?;
    declare_harness_false_target(&root, "alias_entry", "tests/alias_entry.rs")?;
    let files = [PathBuf::from("tests/alias_entry.rs")];
    let registrations = [custom_target_registration("tests/alias_entry.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    assert_eq!(
        index
            .tests
            .iter()
            .filter(|test| test.file.ends_with(Path::new("tests/alias_entry.rs")))
            .count(),
        1,
        "the aliased entry must not exclude the trial: {:?}",
        index.harness_limitations
    );
    let unknown = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_reachability_unknown")
        .ok_or_else(|| {
            format!(
                "missing unknown disclosure: {:?}",
                index.harness_limitations
            )
        })?;
    assert!(
        unknown.detail.contains("aliased_entry_trial"),
        "{unknown:?}"
    );
    assert!(
        index
            .harness_limitations
            .iter()
            .all(|limitation| limitation.code != "registration_unreachable"),
        "an unanchorable entry is unknown, never an exclusion: {:?}",
        index.harness_limitations
    );
    Ok(())
}

/// #3639 review (devin): a bare `run` bound from a non-marker path
/// (`use adapter::run;`) is possibly a re-export of the harness entry —
/// it cannot anchor, and it must not conclude unreachability either.
#[test]
fn reexported_run_entry_keeps_trials_with_unknown_disclosure()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("reexport-entry")?;
    write_workspace(
        &root,
        &[(
            "tests/reexport_entry.rs",
            r#"
mod adapter {
    pub use libtest_mimic::run;
}

use adapter::{run, Arguments};
use libtest_mimic::Trial;

fn trials() -> Vec<Trial> {
    vec![Trial::test("reexport_entry_trial", || Ok(()))]
}

fn main() {
    let arguments = Arguments::from_args();
    run(&arguments, &trials());
}
"#,
        )],
    )?;
    declare_harness_false_target(&root, "reexport_entry", "tests/reexport_entry.rs")?;
    let files = [PathBuf::from("tests/reexport_entry.rs")];
    let registrations = [custom_target_registration("tests/reexport_entry.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    assert_eq!(
        index
            .tests
            .iter()
            .filter(|test| test.file.ends_with(Path::new("tests/reexport_entry.rs")))
            .count(),
        1,
        "the re-exported entry must not exclude the trial: {:?}",
        index.harness_limitations
    );
    let unknown = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_reachability_unknown")
        .ok_or_else(|| {
            format!(
                "missing unknown disclosure: {:?}",
                index.harness_limitations
            )
        })?;
    assert!(
        unknown.detail.contains("reexport_entry_trial"),
        "{unknown:?}"
    );
    assert!(
        index
            .harness_limitations
            .iter()
            .all(|limitation| limitation.code != "registration_unreachable"),
        "a re-exported entry is unknown, never an exclusion: {:?}",
        index.harness_limitations
    );
    Ok(())
}

/// #3639 review (devin): builder resolution traces the returned
/// expression — trials in unused bindings are not part of the run
/// argument and are excluded, without an unknown disclosure.
#[test]
fn builder_unused_binding_trial_is_excluded() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("builder-unused")?;
    write_workspace(
        &root,
        &[(
            "tests/builder_unused.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn build_trials() -> Vec<Trial> {
    let unused = vec![Trial::test("builder_dead_trial", || Ok(()))];
    vec![Trial::test("builder_live_trial", || Ok(()))]
}
"#,
                "build_trials()",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "builder_unused", "tests/builder_unused.rs")?;
    let files = [PathBuf::from("tests/builder_unused.rs")];
    let registrations = [custom_target_registration("tests/builder_unused.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    let admitted: Vec<_> = index
        .tests
        .iter()
        .filter(|test| test.file.ends_with(Path::new("tests/builder_unused.rs")))
        .map(|test| test.name.as_str())
        .collect();
    assert_eq!(
        admitted,
        vec!["builder_live_trial"],
        "only the returned trial enters the denominator: {:?}",
        index.harness_limitations
    );
    let exclusion = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_unreachable")
        .ok_or_else(|| format!("missing exclusion: {:?}", index.harness_limitations))?;
    assert!(
        exclusion.detail.contains("builder_dead_trial"),
        "{exclusion:?}"
    );
    assert!(
        index
            .harness_limitations
            .iter()
            .all(|limitation| limitation.code != "registration_reachability_unknown"),
        "a straight-line builder is a proof, not an unknown: {:?}",
        index.harness_limitations
    );
    Ok(())
}

/// #3639 review (devin): a builder with an early `return` has
/// unsupported control flow — its trials stay admitted under the
/// unknown disclosure, never excluded and never blindly reachable.
#[test]
fn builder_early_return_discloses_unknown() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("builder-return")?;
    write_workspace(
        &root,
        &[(
            "tests/builder_return.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn build_trials() -> Vec<Trial> {
    return vec![Trial::test("early_return_trial", || Ok(()))];
}
"#,
                "build_trials()",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "builder_return", "tests/builder_return.rs")?;
    let files = [PathBuf::from("tests/builder_return.rs")];
    let registrations = [custom_target_registration("tests/builder_return.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    assert_eq!(
        index
            .tests
            .iter()
            .filter(|test| test.file.ends_with(Path::new("tests/builder_return.rs")))
            .count(),
        1,
        "unsupported control flow keeps the trial admitted: {:?}",
        index.harness_limitations
    );
    let unknown = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_reachability_unknown")
        .ok_or_else(|| {
            format!(
                "missing unknown disclosure: {:?}",
                index.harness_limitations
            )
        })?;
    assert!(unknown.detail.contains("early_return_trial"), "{unknown:?}");
    Ok(())
}

/// #3639 review (devin): a builder with a conditional at body depth zero
/// has unsupported control flow — unknown disclosure, not reachable.
#[test]
fn builder_dead_conditional_discloses_unknown() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("builder-cond")?;
    write_workspace(
        &root,
        &[(
            "tests/builder_cond.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn build_trials() -> Vec<Trial> {
    if false {
        vec![Trial::test("cond_dead_trial", || Ok(()))]
    } else {
        vec![Trial::test("cond_live_trial", || Ok(()))]
    }
}
"#,
                "build_trials()",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "builder_cond", "tests/builder_cond.rs")?;
    let files = [PathBuf::from("tests/builder_cond.rs")];
    let registrations = [custom_target_registration("tests/builder_cond.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    assert_eq!(
        index
            .tests
            .iter()
            .filter(|test| test.file.ends_with(Path::new("tests/builder_cond.rs")))
            .count(),
        2,
        "both conditional trials stay admitted: {:?}",
        index.harness_limitations
    );
    let unknown = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_reachability_unknown")
        .ok_or_else(|| {
            format!(
                "missing unknown disclosure: {:?}",
                index.harness_limitations
            )
        })?;
    assert!(
        unknown.detail.contains("cond_dead_trial") && unknown.detail.contains("cond_live_trial"),
        "{unknown:?}"
    );
    Ok(())
}

/// #3639 review (devin): the commas separating multiple direct
/// collection elements are scaffolding — two live trials resolve
/// completely and a dead construction elsewhere is excluded without an
/// unknown disclosure.
#[test]
fn two_trials_in_direct_collection_exclude_dead_construction()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("multi-trials")?;
    write_workspace(
        &root,
        &[(
            "tests/multi_trials.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn dead_elsewhere() -> Vec<Trial> {
    vec![Trial::test("elsewhere_dead_trial", || Ok(()))]
}
"#,
                "vec![Trial::test(\"multi_a\", || Ok(())), Trial::test(\"multi_b\", || Ok(()))]",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "multi_trials", "tests/multi_trials.rs")?;
    let files = [PathBuf::from("tests/multi_trials.rs")];
    let registrations = [custom_target_registration("tests/multi_trials.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    let mut admitted: Vec<_> = index
        .tests
        .iter()
        .filter(|test| test.file.ends_with(Path::new("tests/multi_trials.rs")))
        .map(|test| test.name.as_str())
        .collect();
    admitted.sort_unstable();
    assert_eq!(
        admitted,
        vec!["multi_a", "multi_b"],
        "both collection elements are reachable: {:?}",
        index.harness_limitations
    );
    let exclusion = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_unreachable")
        .ok_or_else(|| format!("missing exclusion: {:?}", index.harness_limitations))?;
    assert!(
        exclusion.detail.contains("elsewhere_dead_trial"),
        "{exclusion:?}"
    );
    assert!(
        index
            .harness_limitations
            .iter()
            .all(|limitation| limitation.code != "registration_reachability_unknown"),
        "a fully resolved argument is a proof, not an unknown: {:?}",
        index.harness_limitations
    );
    Ok(())
}

/// #3639 review (devin): a trial constructed inside another trial's
/// callback is not a collection element — only the outer trial is
/// reachable; the inner construction is excluded by the resolved
/// argument.
#[test]
fn nested_trial_in_callback_is_not_a_collection_element() -> Result<(), Box<dyn std::error::Error>>
{
    let root = temp_dir("nested-trial")?;
    write_workspace(
        &root,
        &[(
            "tests/nested_trial.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;
"#,
                "vec![Trial::test(\"outer_trial\", || {\n        let _hidden = Trial::test(\"inner_trial\", || Ok(()));\n    })]",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "nested_trial", "tests/nested_trial.rs")?;
    let files = [PathBuf::from("tests/nested_trial.rs")];
    let registrations = [custom_target_registration("tests/nested_trial.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    let admitted: Vec<_> = index
        .tests
        .iter()
        .filter(|test| test.file.ends_with(Path::new("tests/nested_trial.rs")))
        .map(|test| test.name.as_str())
        .collect();
    assert_eq!(
        admitted,
        vec!["outer_trial"],
        "only the outer collection element is reachable: {:?}",
        index.harness_limitations
    );
    let exclusion = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_unreachable")
        .ok_or_else(|| format!("missing exclusion: {:?}", index.harness_limitations))?;
    assert!(exclusion.detail.contains("inner_trial"), "{exclusion:?}");
    assert!(
        index
            .harness_limitations
            .iter()
            .all(|limitation| limitation.code != "registration_reachability_unknown"),
        "the argument resolves completely: {:?}",
        index.harness_limitations
    );
    Ok(())
}

/// #3639 review (gemini): `mut` in a `&mut` receiver position is
/// scaffolding — `run(&mut vec![..])` resolves completely instead of
/// degrading to an unknown disclosure.
#[test]
fn mut_receiver_direct_collection_resolves_completely() -> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("mut-arg")?;
    write_workspace(
        &root,
        &[(
            "tests/mut_arg.rs",
            &with_harness_main(
                r#"
use libtest_mimic::Trial;

fn dead_elsewhere() -> Vec<Trial> {
    vec![Trial::test("mut_arg_dead_trial", || Ok(()))]
}
"#,
                "&mut vec![Trial::test(\"mut_arg_trial\", || Ok(()))]",
            ),
        )],
    )?;
    declare_harness_false_target(&root, "mut_arg", "tests/mut_arg.rs")?;
    let files = [PathBuf::from("tests/mut_arg.rs")];
    let registrations = [custom_target_registration("tests/mut_arg.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    let admitted: Vec<_> = index
        .tests
        .iter()
        .filter(|test| test.file.ends_with(Path::new("tests/mut_arg.rs")))
        .map(|test| test.name.as_str())
        .collect();
    assert_eq!(
        admitted,
        vec!["mut_arg_trial"],
        "the &mut receiver form resolves: {:?}",
        index.harness_limitations
    );
    let exclusion = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_unreachable")
        .ok_or_else(|| format!("missing exclusion: {:?}", index.harness_limitations))?;
    assert!(
        exclusion.detail.contains("mut_arg_dead_trial"),
        "{exclusion:?}"
    );
    assert!(
        index
            .harness_limitations
            .iter()
            .all(|limitation| limitation.code != "registration_reachability_unknown"),
        "{:?}",
        index.harness_limitations
    );
    Ok(())
}

/// #3636: every supported run-argument form admits its trials — a
/// direct `vec![]` literal, an array literal, an immutable let-bound
/// chain, a `&local[..]` index, and a one-level builder function — with
/// no reachability limitation recorded.
#[test]
fn given_run_entry_then_supported_argument_forms_admit_trials()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("reachability-admitted")?;
    write_workspace(
        &root,
        &[(
            "tests/admitted_forms.rs",
            r#"
use libtest_mimic::Trial;

fn build_helper() -> Vec<Trial> {
    vec![Trial::test("helper_built", || Ok(()))]
}

fn main() {
    let arguments = libtest_mimic::Arguments::from_args();
    let chained = vec![Trial::test("let_chained", || Ok(()))];
    libtest_mimic::run(&arguments, &vec![Trial::test("inline_vec", || Ok(()))]);
    libtest_mimic::run(&arguments, &[Trial::test("array_literal", || Ok(()))]);
    libtest_mimic::run(&arguments, &chained);
    libtest_mimic::run(&arguments, &chained[..]);
    let built = build_helper();
    libtest_mimic::run(&arguments, &built);
}
"#,
        )],
    )?;
    declare_harness_false_target(&root, "admitted_forms", "tests/admitted_forms.rs")?;
    let files = [PathBuf::from("tests/admitted_forms.rs")];
    let registrations = [custom_target_registration("tests/admitted_forms.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    let mut names = index
        .harness_subjects
        .iter()
        .map(|subject| subject.name.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(
        names,
        vec!["array_literal", "helper_built", "inline_vec", "let_chained"]
    );
    for subject in &index.harness_subjects {
        assert_eq!(subject.claim, HarnessSubjectClaim::NamedInvocation);
    }
    // Every admitted trial keeps its executable-test denominator entry.
    let mut test_names = index
        .tests
        .iter()
        .filter(|test| test.file.ends_with(Path::new("tests/admitted_forms.rs")))
        .map(|test| test.name.as_str())
        .collect::<Vec<_>>();
    test_names.sort_unstable();
    assert_eq!(
        test_names,
        vec!["array_literal", "helper_built", "inline_vec", "let_chained"]
    );
    assert!(
        index.harness_limitations.is_empty(),
        "supported forms must not record reachability limitations: {:?}",
        index.harness_limitations
    );
    Ok(())
}

/// #3636: a dead constructor beside a completely resolved run argument
/// is provably exclusive — the admitted trial stays in the denominator,
/// the excluded one keeps its subject fact, syntactic claim, and
/// evidence but leaves the denominator under a typed limitation naming
/// it.
#[test]
fn given_dead_constructor_beside_resolved_argument_then_it_leaves_the_denominator()
-> Result<(), Box<dyn std::error::Error>> {
    let root = temp_dir("reachability-excluded")?;
    write_workspace(
        &root,
        &[(
            "tests/mixed_reachability.rs",
            r#"
use libtest_mimic::Trial;

fn production_call(value: u16) -> u16 { value }

fn never_called() -> Vec<Trial> {
    vec![Trial::test("excluded_helper_trial", || {
        assert_eq!(production_call(1), 1);
    })]
}

fn main() {
    let arguments = libtest_mimic::Arguments::from_args();
    let all_trials = vec![Trial::test("admitted_trial", || Ok(()))];
    libtest_mimic::run(&arguments, &all_trials);
}
"#,
        )],
    )?;
    declare_harness_false_target(&root, "mixed_reachability", "tests/mixed_reachability.rs")?;
    let files = [PathBuf::from("tests/mixed_reachability.rs")];
    let registrations = [custom_target_registration("tests/mixed_reachability.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    // Both subjects exist under the syntactic claim.
    let excluded = index
        .harness_subjects
        .iter()
        .find(|subject| subject.name == "excluded_helper_trial")
        .ok_or("excluded subject fact must stay")?;
    assert_eq!(excluded.claim, HarnessSubjectClaim::NamedInvocation);
    assert!(
        excluded
            .assertions
            .iter()
            .any(|oracle| oracle.text.starts_with("assert_eq!")),
        "the dead constructor's evidence still joins the subject fact: {:?}",
        excluded.assertions
    );
    assert!(
        index
            .harness_subjects
            .iter()
            .any(|subject| subject.name == "admitted_trial")
    );

    // Only the admitted trial enters the executable-test denominator.
    let test_names: Vec<_> = index
        .tests
        .iter()
        .filter(|test| {
            test.file
                .ends_with(Path::new("tests/mixed_reachability.rs"))
        })
        .map(|test| test.name.as_str())
        .collect();
    assert_eq!(test_names, vec!["admitted_trial"], "{test_names:?}");

    // The exclusion is a typed limitation naming the trial and the
    // complete-resolution proof.
    let limitation = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_unreachable")
        .ok_or("missing registration_unreachable limitation")?;
    assert!(
        limitation.detail.contains("excluded_helper_trial"),
        "{limitation:?}"
    );
    assert!(
        limitation
            .detail
            .contains("resolved completely through the supported resolution forms"),
        "{limitation:?}"
    );
    assert!(
        index
            .harness_limitations
            .iter()
            .all(|limitation| limitation.code != "registration_reachability_unknown"),
        "a complete resolution leaves nothing unknown: {:?}",
        index.harness_limitations
    );
    Ok(())
}

/// #3636: when the bounded resolver cannot connect a trial to the run
/// argument, the trial stays in the denominator (today's behavior) and
/// the gap is disclosed by one aggregate typed limitation naming the
/// trials — never by a fabricated per-subject field, and never by
/// concluding unreachability.
#[test]
fn given_unresolvable_run_argument_then_trials_stay_admitted_and_disclosed()
-> Result<(), Box<dyn std::error::Error>> {
    // A `mut` collection built by push cannot be resolved: the run
    // argument's final value is not provable, so the trial stays with an
    // explicit unknown disclosure.
    let root = temp_dir("reachability-unknown")?;
    write_workspace(
        &root,
        &[(
            "tests/unknown_reachability.rs",
            r#"
use libtest_mimic::Trial;

fn main() {
    let arguments = libtest_mimic::Arguments::from_args();
    let mut all_trials = Vec::new();
    all_trials.push(Trial::test("pushed_trial", || Ok(())));
    libtest_mimic::run(&arguments, &all_trials);
}
"#,
        )],
    )?;
    declare_harness_false_target(
        &root,
        "unknown_reachability",
        "tests/unknown_reachability.rs",
    )?;
    let files = [PathBuf::from("tests/unknown_reachability.rs")];
    let registrations = [custom_target_registration("tests/unknown_reachability.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;

    assert!(
        index
            .harness_subjects
            .iter()
            .any(|subject| subject.name == "pushed_trial"),
        "an unknown must stay admitted: {:?}",
        index.harness_subjects
    );
    assert!(
        index.tests.iter().any(|test| test.name == "pushed_trial"),
        "an unknown keeps today's denominator behavior"
    );
    let disclosure = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_reachability_unknown")
        .ok_or("missing aggregate unknown disclosure")?;
    assert!(
        disclosure.detail.contains("pushed_trial"),
        "the disclosure names the unknown trials: {disclosure:?}"
    );
    assert!(
        disclosure.detail.contains("bound `mut`"),
        "the disclosure names its reason: {disclosure:?}"
    );
    assert!(
        index
            .harness_limitations
            .iter()
            .all(|limitation| limitation.code != "registration_unreachable"),
        "unknown must not degrade into an exclusion: {:?}",
        index.harness_limitations
    );

    // An unsupported run-entry spelling (`run_tests`) never lets the
    // authority conclude absence: the trial stays and the gap is
    // disclosed.
    let root = temp_dir("reachability-run-tests")?;
    write_workspace(
        &root,
        &[(
            "tests/run_tests_spelling.rs",
            r#"
use libtest_mimic::Trial;

fn main() {
    let arguments = libtest_mimic::Arguments::from_args();
    let all_trials = vec![Trial::test("run_tests_trial", || Ok(()))];
    libtest_mimic::run_tests(&arguments, &all_trials);
}
"#,
        )],
    )?;
    declare_harness_false_target(&root, "run_tests_spelling", "tests/run_tests_spelling.rs")?;
    let files = [PathBuf::from("tests/run_tests_spelling.rs")];
    let registrations = [custom_target_registration("tests/run_tests_spelling.rs")];
    let index = build_index_with_test_harnesses(&root.0, &files, &registrations)?;
    assert!(
        index
            .tests
            .iter()
            .any(|test| test.name == "run_tests_trial"),
        "an unsupported entry spelling keeps the trial admitted: {:?}",
        index.tests
    );
    let disclosure = index
        .harness_limitations
        .iter()
        .find(|limitation| limitation.code == "registration_reachability_unknown")
        .ok_or("missing disclosure for unsupported entry spelling")?;
    assert!(disclosure.detail.contains("run_tests"), "{disclosure:?}");
    Ok(())
}

/// #3636 cache discipline: the reachability decision is part of the
/// registry, which re-applies after the file-fact cache loads on every
/// build, so the exclusion is identical on the warm path — a warm hit
/// cannot serve the pre-#3636 denominator.
#[test]
fn warm_file_fact_cache_applies_reachability_identically() -> Result<(), Box<dyn std::error::Error>>
{
    let root = temp_dir("warm-reachability")?;
    fs::create_dir_all(root.0.join("src"))?;
    // A standalone `[workspace]` table makes the fixture its own
    // workspace root: cargo's metadata probe must not walk ancestors,
    // where a host temp dir can sit inside an unrelated workspace tree
    // (#3637 metadata validation).
    fs::write(
        root.0.join("Cargo.toml"),
        "[package]\nname = 'harness-fixture'\nversion = '0.1.0'\nedition = '2024'\n\n[workspace]\n",
    )?;
    let file = PathBuf::from("src/dead_reachability.rs");
    let source = br#"
use libtest_mimic::Trial;

fn trials() -> Vec<Trial> {
    vec![Trial::test("dead_warm_case", || Ok(()))]
}
"#
    .to_vec();
    declare_harness_false_target(&root, "dead_reachability", "src/dead_reachability.rs")?;
    let files = [(file.clone(), source)];
    let registrations = [custom_target_registration("src/dead_reachability.rs")];

    let cold = crate::analysis::facts::build_index_from_loaded_files_with_cache_and_test_harnesses(
        &root.0,
        &files,
        &registrations,
    )?;
    let warm = crate::analysis::facts::build_index_from_loaded_files_with_cache_and_test_harnesses(
        &root.0,
        &files,
        &registrations,
    )?;
    assert_eq!(
        warm.file_fact_cache.hits, 1,
        "the warm run must be a cache hit"
    );
    for (label, index) in [("cold", &cold.index), ("warm", &warm.index)] {
        assert_eq!(index.harness_subjects.len(), 1, "{label}");
        assert_eq!(index.tests.len(), 0, "{label}: dead trials never admit");
        let limitation = index
            .harness_limitations
            .iter()
            .find(|limitation| limitation.code == "registration_unreachable")
            .ok_or_else(|| format!("{label}: missing registration_unreachable limitation"))?;
        assert!(
            limitation.detail.contains("dead_warm_case"),
            "{label}: {limitation:?}"
        );
    }
    Ok(())
}
