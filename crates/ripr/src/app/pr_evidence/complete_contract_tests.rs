//! Independent #1627 controls; integrated only after the frozen-source assembly.
//! No complete-generation API exists yet. The successful snapshot calls below
//! are experimental preparation, and every consumer must still refuse them.
//! Dependency: RustAdapter::with_forced_diff_limits_for_test is an assembly-only
//! forwarding bridge to the existing thread-local limit lookup, not a new policy.
#![cfg(all(feature = "lang-rust", feature = "lang-typescript", feature = "lang-python"))]

use super::*;
use crate::testing::fixture_git::{fixture_git_ok, remove_fixture_tree};

const CONFIG: &str = "[analysis]\nmode = \"draft\"\ninclude_unchanged_tests = true\n[languages]\nenabled = [\"rust\", \"typescript\", \"python\"]\n";
const RUST_BASE: &str = "pub fn eligible(value: i32) -> bool { value > 0 }\n";
const RUST_HEAD: &str = "pub fn eligible(value: i32) -> bool { value > 1 }\n";
const GENERATION_NONCE: &str = "1627";
const LINE_LIMIT: &str = "RIPR_MAX_DIFF_CHANGED_RUST_LINES";

fn generation() -> Value {
    json!({"schema_version":"ripr.complete_execution_experiment.v1",
        "nonce":GENERATION_NONCE, "coverage":"not_established",
        "production_admission":false})
}

fn write(repo: &Path, path: &str, text: &str) -> Result<(), String> {
    let destination = repo.join(path);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(destination, text).map_err(|error| error.to_string())
}

fn read_json(repo: &Path, path: &str) -> Result<Value, String> {
    serde_json::from_slice(&fs::read(repo.join(path)).map_err(|error| error.to_string())?)
        .map_err(|error| format!("{path}: {error}"))
}

fn refused<T>(result: Result<T, String>, label: &str) -> Result<String, String> {
    match result {
        Ok(_) => Err(format!("{label} was admitted")),
        Err(error) => Ok(error),
    }
}

fn no_authority(repo: &Path) -> Result<(), String> {
    for path in [PR_CHECK_SUBJECT_JSON, PR_CHECK_JSON, PR_REVIEW_INPUT_JSON] {
        if repo.join(path).exists() {
            return Err(format!("failed generation retained {path}"));
        }
    }
    assert_eq!(read_json(repo, PR_EVIDENCE_JSON)?["status"], "error");
    Ok(())
}

fn with_fixture(
    name: &str,
    test: impl FnOnce(&Path, &PrEvidenceOptions) -> Result<(), String>,
) -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let owned = std::env::temp_dir().join(format!(
        "ripr-complete-contract-{name}-{}-{stamp}", std::process::id()
    ));
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = remove_fixture_tree(&self.0);
        }
    }
    let cleanup = Cleanup(owned.clone());
    let repo = owned.join("repo");
    fs::create_dir_all(&repo).map_err(|error| error.to_string())?;
    let result = (|| {
        fixture_git_ok(&repo, &["-c", "init.templateDir=", "init", "-q", "-b", "trunk"])?;
        for (key, value) in [
            ("user.name", "RIPR Contract Fixture"),
            ("user.email", "contract@example.invalid"),
            ("commit.gpgSign", "false"),
        ] {
            fixture_git_ok(&repo, &["config", key, value])?;
        }
        write(&repo, ".gitignore", "target/\n")?;
        write(&repo, "Cargo.toml", "[package]\nname = \"boundary-contract\"\nversion = \"0.1.0\"\nedition = \"2021\"\n")?;
        write(&repo, "ripr.toml", CONFIG)?;
        write(&repo, "src/lib.rs", RUST_BASE)?;
        write(&repo, "tests/eligible.rs", "#[test]\nfn boundary() { assert!(!boundary_contract::eligible(1)); }\n")?;
        write(&repo, "src/eligible.ts", "export function eligible(value: number): boolean { return value > 0; }\n")?;
        write(&repo, "src/index.ts", "export { eligible } from './eligible';\n")?;
        write(&repo, "tests/eligible.test.ts", "import { eligible } from '../src/index';\nit('boundary', () => { expect(eligible(1)).toBe(false); });\n")?;
        write(&repo, "service.py", "def eligible(value):\n    return value > 0\n")?;
        write(&repo, "test_service.py", "from service import eligible\n\ndef test_boundary():\n    assert not eligible(1)\n")?;
        fixture_git_ok(&repo, &["add", "-A"])?;
        fixture_git_ok(&repo, &["commit", "-q", "-m", "base"])?;
        let base = resolve_revision(&repo, "HEAD", "commit")?;
        write(&repo, "src/lib.rs", RUST_HEAD)?;
        write(&repo, "src/eligible.ts", "export function eligible(value: number): boolean { return value > 1; }\n")?;
        write(&repo, "service.py", "def eligible(value):\n    return value > 1\n")?;
        fixture_git_ok(&repo, &["commit", "-q", "-a", "-m", "all three predicates"])?;
        let options = PrEvidenceOptions {
            root: ".".into(), base, base_explicit: true,
            head: resolve_revision(&repo, "HEAD", "commit")?, check: false,
        };
        test(&repo, &options)
    })();
    let removed = remove_fixture_tree(&cleanup.0);
    result.and(removed)
}

fn direct_review_admission(repo: &Path, options: &PrEvidenceOptions) -> Result<(), String> {
    let input = CheckInput { root: repo.to_path_buf(), ..CheckInput::default() };
    let config = load_for_root(repo)?;
    let diff = fs::read_to_string(repo.join(PR_CANONICAL_DIFF))
        .map_err(|error| error.to_string())?;
    let admitted = crate::app::review_comments::admit_producer_evidence(
        &repo.join(PR_CHECK_JSON), &input, &config, &options.base, &options.head, &diff,
    ).map_err(|error| error.message)?;
    if admitted.producer_projection.is_empty() {
        return Err("direct review admitted an empty ordinary projection".into());
    }
    Ok(())
}

fn baseline(repo: &Path, options: &PrEvidenceOptions) -> Result<Value, String> {
    write_pr_evidence_with_generation(repo, options, run_ripr_check, None)?;
    check_pr_evidence(repo, options)?;
    direct_review_admission(repo, options)?;
    let check = read_json(repo, PR_CHECK_JSON)?;
    assert_eq!(check["analysis_outcome"]["analysis_complete"], true);
    let findings = check["findings"].as_array().ok_or("missing findings array")?;
    if findings.is_empty() {
        return Err("fixture did not execute nonempty analysis".into());
    }
    Ok(check)
}

#[cfg(all(feature = "lang-typescript", feature = "lang-python"))]
#[test]
fn mixed_whole_output_survives_live_source_test_manifest_and_directory_drift() -> Result<(), String> {
    with_fixture("mixed-whole-output", |repo, options| {
        let expected = baseline(repo, options)?;
        let findings = expected["findings"].as_array().ok_or("missing findings")?;
        for (changed, test) in [
            ("src/lib.rs", "tests/eligible.rs"),
            ("src/eligible.ts", "tests/eligible.test.ts"),
            ("service.py", "test_service.py"),
        ] {
            if !findings.iter().any(|finding| {
                finding["probe"]["file"].as_str().is_some_and(|file| {
                    file.replace('\\', "/").ends_with(changed)
                }) && finding["related_tests"].as_array().is_some_and(|related| {
                    related.iter().any(|related| {
                        related["file"].as_str().is_some_and(|file| {
                            file.replace('\\', "/").ends_with(test)
                        })
                    })
                })
            }) {
                return Err(format!("ordinary baseline has no {changed} -> {test} relation"));
            }
        }
        // Before capture, both working tree and index disagree with literal
        // head. A snapshot copied from ambient files/index must fail equality.
        write(repo, "Cargo.toml", "[package]\nname = \"ambient-decoy\"\nversion = \"0.0.0\"\nedition = \"2021\"\n")?;
        write(repo, "src/lib.rs", "pub fn unrelated_live_owner() -> bool { false }\n")?;
        write(repo, "src/eligible.ts", "export const unrelated_live_owner = false;\n")?;
        write(repo, "src/index.ts", "export const unrelated_live_barrel = false;\n")?;
        write(repo, "service.py", "unrelated_live_owner = False\n")?;
        for path in ["tests/eligible.rs", "tests/eligible.test.ts", "test_service.py"] {
            write(repo, path, "# ambient test decoy\n")?;
        }
        fixture_git_ok(repo, &["add", "Cargo.toml", "src", "tests", "service.py", "test_service.py"])?;
        let status = run_git_output(repo, &["status", "--porcelain", "--untracked-files=no"])?;
        if status.trim().is_empty() {
            return Err("preparation fixture has no staged live drift".into());
        }
        let marker = generation();
        write_pr_evidence_with_generation(repo, options, |repo, options| {
            for path in [
                "Cargo.toml", "src/lib.rs", "src/eligible.ts", "src/index.ts",
                "tests/eligible.rs", "tests/eligible.test.ts", "service.py", "test_service.py",
            ] {
                fs::remove_file(repo.join(path)).map_err(|error| error.to_string())?;
            }
            write(repo, "src/live_only.rs", "pub fn live_decoy() -> bool { false }\n")?;
            write(repo, "tests/live_only.rs", "#[test] fn decoy() { assert!(true); }\n")?;
            write(repo, "src/live_only.ts", "export const live_decoy = false;\n")?;
            write(repo, "live_only.py", "live_decoy = False\n")?;
            run_ripr_check(repo, options)
        }, Some(&marker))?;
        // Same logical root and exact canonical diff: no path, ID, finding,
        // outcome, limitation, test-role, or summary fields may be discarded.
        assert_eq!(read_json(repo, PR_CHECK_JSON)?, expected);
        for path in [PR_EVIDENCE_JSON, PR_CHECK_SUBJECT_JSON, PR_REVIEW_INPUT_JSON] {
            assert_eq!(read_json(repo, path)?["experimental_complete_execution"], marker);
        }
        let error = refused(check_pr_evidence(repo, options), "experimental saved check")?;
        assert!(error.contains("experimental complete-execution"), "{error}");
        let error = refused(direct_review_admission(repo, options), "experimental direct review")?;
        assert!(error.contains("experimental complete-execution"), "{error}");
        Ok(())
    })
}

#[test]
fn late_configuration_head_and_canonical_input_drift_revoke_then_recover() -> Result<(), String> {
    for drift in ["configuration", "head", "canonical"] {
        with_fixture(drift, |repo, options| {
            let expected = baseline(repo, options)?;
            let mut moving = options.clone();
            moving.head = "HEAD".into();
            let error = refused(write_pr_evidence_with_generation(repo, &moving, |repo, options| {
                let check = run_ripr_check(repo, options)?;
                match drift {
                    "configuration" => write(repo, "ripr.toml", &CONFIG.replace("include_unchanged_tests = true", "include_unchanged_tests = false"))?,
                    "head" => {
                        write(repo, "src/lib.rs", "pub fn eligible(value: i32) -> bool { value > 2 }\n")?;
                        fixture_git_ok(repo, &["commit", "-q", "-a", "-m", "moved head"])?;
                    }
                    "canonical" => write(repo, PR_CANONICAL_DIFF, "changed after actual analysis\n")?,
                    _ => return Err("unknown drift fixture".into()),
                }
                Ok(check)
            }, Some(&generation())), drift)?;
            let needle = match drift {
                "configuration" => "committed configuration",
                "head" => "prepared head changed",
                "canonical" => "canonical check input changed",
                _ => return Err("unknown drift expectation".into()),
            };
            assert!(error.contains(needle), "{drift}: {error}");
            no_authority(repo)?;
            write(repo, "ripr.toml", CONFIG)?;
            write(repo, "src/lib.rs", RUST_HEAD)?;
            // Recreate a head whose tree equals the original fixture if a
            // committed drift occurred; no checkout or shared ref is involved.
            if drift == "head" {
                fixture_git_ok(repo, &["commit", "-q", "-a", "-m", "recovered head"])?;
            }
            let recovered = PrEvidenceOptions {
                head: resolve_revision(repo, "HEAD", "commit")?, ..options.clone()
            };
            let actual = baseline(repo, &recovered)?;
            if drift != "head" {
                assert_eq!(actual, expected);
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn actual_module_escape_refuses_an_existing_external_source_then_recovers() -> Result<(), String> {
    with_fixture("module-escape", |repo, options| {
        baseline(repo, options)?;
        let outside = repo.parent().ok_or("fixture has no parent")?.join("outside.rs");
        fs::write(&outside, "pub fn external_marker() -> bool { true }\n")
            .map_err(|error| error.to_string())?;
        if !outside.is_file() {
            return Err("external source control was not created".into());
        }
        write(repo, "src/lib.rs", "#[path = \"../../outside.rs\"]\nmod outside;\npub fn eligible(value: i32) -> bool { value > 1 && outside::external_marker() }\n")?;
        fixture_git_ok(repo, &["commit", "-q", "-a", "-m", "escaped module"])?;
        let escaped = PrEvidenceOptions {
            head: resolve_revision(repo, "HEAD", "commit")?, ..options.clone()
        };
        let error = refused(write_pr_evidence_with_generation(
            repo, &escaped, run_ripr_check, Some(&generation())
        ), "actual module escape")?;
        assert!(error.contains("frozen source") && error.contains("outside"), "{error}");
        no_authority(repo)?;
        write(repo, "src/lib.rs", RUST_HEAD)?;
        fixture_git_ok(repo, &["commit", "-q", "-a", "-m", "confined recovery"])?;
        let recovered = PrEvidenceOptions {
            head: resolve_revision(repo, "HEAD", "commit")?, ..options.clone()
        };
        baseline(repo, &recovered)?;
        Ok(())
    })
}

/// One indivisible handwritten owner, with its only predicate beyond line5400.
/// Padding comments deliberately keep finding count below the4096-entry cap;
/// this fixture discriminates input admission/suffix coverage, not AST cost.
fn owner_7038() -> String {
    let mut source = String::from("pub fn beyond_boundary(value: i32) -> bool {\n");
    for line in 2..=7033 {
        source.push_str(&format!("    // handwritten owner padding {line}\n"));
    }
    source.push_str("    if value > 5400 {\n        return true;\n    }\n    false\n}\n");
    source
}

#[test]
fn ordinary_5400_guard_refuses_one_7038_line_owner_before_partial_selection() -> Result<(), String> {
    with_fixture("7038-owner", |repo, options| {
        let owner = owner_7038();
        assert_eq!(owner.lines().count(), 7038);
        assert_eq!(owner.lines().nth(7033), Some("    if value > 5400 {"));
        // A reachable module in both trees, then one added semantic owner.
        write(repo, "src/lib.rs", &format!("{RUST_HEAD}pub mod oversized;\n"))?;
        write(repo, "src/oversized.rs", "")?;
        fixture_git_ok(repo, &["add", "src/lib.rs", "src/oversized.rs"])?;
        fixture_git_ok(repo, &["commit", "-q", "-m", "declare empty module"])?;
        let base = resolve_revision(repo, "HEAD", "commit")?;
        write(repo, "src/oversized.rs", &owner)?;
        fixture_git_ok(repo, &["add", "src/oversized.rs"])?;
        fixture_git_ok(repo, &["commit", "-q", "-m", "one oversized owner"])?;
        let subject = PrEvidenceOptions {
            // Isolate exactly the single added owner; use literal commits.
            base,
            head: resolve_revision(repo, "HEAD", "commit")?, ..options.clone()
        };
        let numstat = run_git_output(repo, &["diff", "--numstat", &subject.base, &subject.head])?;
        assert_eq!(numstat.trim(), "7038\t0\tsrc/oversized.rs");
        write_diff(repo, &subject)?;
        for partial in ["1", "invalid"] {
            let error = crate::analysis::language::RustAdapter::with_forced_diff_limits_for_test(
                &[(LINE_LIMIT, "5400"), ("RIPR_PARTIAL_DIFF_LINE_BUDGET", partial)],
                || refused(run_ripr_check(repo, &subject), "ordinary oversized owner"),
            )?;
            assert_eq!(error, "diff_scope_oversized: 7038 changed Rust lines across 1 Rust files exceed the RIPR_MAX_DIFF_CHANGED_RUST_LINES limit (5400); analysis was not run to protect runner memory before probe expansion. Repair route: reduce the diff scope, split the extraction PR, run a narrower diff, or raise the limit via RIPR_MAX_DIFF_CHANGED_RUST_LINES=<number>.");
        }
        Ok(())
    })
}

// Future literal expectations; NOT executed or implemented by this test file:
// - The same7038-added/0-removed/1-Rust-owner subject under VerifiedWholeInput
//   must retain a probe at line7034 owned by beyond_boundary, once, after actual
//   whole classification/EOF ledger reduction and within unchanged index caps.
// - A below-cap verified worker must equal the full ordinary Value above under
//   the same effective config, global test context and logical subject paths.
// - Valid completion binds exact base/head/tree/merge-base/canonical-u0/config/
//   analyzer/nonce/profile/EOF-occurrence-reduction identities across producer,
//   saved --check, direct review and rendered --check-output. Change or omit
//   any one binding, or duplicate/omit an occurrence: every consumer refuses.
// - Missing pipe reader, read/drain/EOF/timeout/cancellation, helper denial,
//   allocation/file/index/output overrun or cleanup failure publishes no
//   completed generation. Restoring inputs then runs a fresh nonce successfully.
// - Profile <=2GiB AS, <=256MiB file, lower inherited ceilings, core0,
//   default120s worker deadline/64KiB streams, parent16KiB receipt/64KiB hash
//   scratch. These are finite limits, never an aggregate RSS or carrier-fit claim.
