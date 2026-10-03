//! #4789 / #4835 quarantine: match public static output with real test collection.
//! No-op property spellings must not borrow the ordinary assertion positive.
use ripr::{CheckInput, ExposureClass, Mode, OutputFormat, check_workspace, render_check};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

struct Scratch {
    directory: PathBuf,
    cleanup_attempted: bool,
}
impl Scratch {
    fn new() -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "ripr-property-quarantine-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Self::create_at(path)
    }

    fn create_at(path: PathBuf) -> Result<Self, String> {
        // Establish ownership exclusively before installing the cleanup guard.
        // A stale/preexisting directory is never adopted or recursively removed.
        std::fs::create_dir(&path).map_err(|error| error.to_string())?;
        let scratch = Self {
            directory: path,
            cleanup_attempted: false,
        };
        std::fs::create_dir(scratch.directory.join("src")).map_err(|error| error.to_string())?;
        Ok(scratch)
    }
}
impl Scratch {
    fn cleanup(mut self) -> Result<(), String> {
        self.cleanup_attempted = true;
        std::fs::remove_dir_all(&self.directory).map_err(|error| {
            format!(
                "explicit fixture cleanup failed at {}: {error}",
                self.directory.display()
            )
        })
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if self.cleanup_attempted {
            return;
        }
        if std::thread::panicking() {
            eprintln!(
                "retained failed property control at {}",
                self.directory.display()
            );
            return;
        }
        if let Err(error) = std::fs::remove_dir_all(&self.directory) {
            eprintln!(
                "fallback fixture cleanup failed at {}: {error}",
                self.directory.display()
            );
        }
    }
}

const STREAM_LIMIT: u64 = 64 * 1024;
fn read_bounded_stream(path: &Path) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.take(STREAM_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > STREAM_LIMIT {
        return Err(format!(
            "runtime stream {} exceeds {STREAM_LIMIT} bytes",
            path.display()
        ));
    }
    Ok(bytes)
}

#[test]
fn property_runtime_stream_overflow_is_a_proof_failure() -> Result<(), String> {
    let scratch = Scratch::new()?;
    let output = scratch.directory.join("overflow");
    std::fs::write(&output, vec![b'x'; STREAM_LIMIT as usize + 1])
        .map_err(|error| error.to_string())?;
    let Err(error) = read_bounded_stream(&output) else {
        return Err("oversized runtime stream was accepted".to_string());
    };
    assert!(error.contains(&format!("exceeds {STREAM_LIMIT} bytes")));
    std::fs::write(&output, "small").map_err(|error| error.to_string())?;
    assert_eq!(read_bounded_stream(&output)?, b"small");
    scratch.cleanup()
}

/// Match the existing advisory-write harness: file-backed streams avoid pipe
/// backpressure, and the shared process owner terminates/reaps on timeout or
/// error before Scratch can remove its owned directory.
fn run_bounded(
    mut command: Command,
    root: &Path,
    label: &str,
    budget: Duration,
) -> Result<Output, String> {
    let stdout = root.join(format!("{label}.stdout"));
    let stderr = root.join(format!("{label}.stderr"));
    command
        .stdin(Stdio::null())
        .stdout(std::fs::File::create(&stdout).map_err(|error| error.to_string())?)
        .stderr(std::fs::File::create(&stderr).map_err(|error| error.to_string())?);
    let mut child =
        ripr::process_owner::OwnedProcess::spawn(command).map_err(|error| error.to_string())?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if started.elapsed() >= budget {
            child.terminate_tree()?;
            return Err(format!(
                "{label} exceeded its {budget:?} budget; owned process terminated and reaped"
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    Ok(Output {
        status,
        stdout: read_bounded_stream(&stdout)?,
        stderr: read_bounded_stream(&stderr)?,
    })
}

#[test]
fn property_quarantine_scratch_does_not_adopt_existing_directory() -> Result<(), String> {
    let scratch = Scratch::new()?;
    let marker = scratch.directory.join("existing-owner");
    std::fs::write(&marker, "keep").map_err(|error| error.to_string())?;
    assert!(Scratch::create_at(scratch.directory.clone()).is_err());
    assert_eq!(
        std::fs::read_to_string(marker).map_err(|error| error.to_string())?,
        "keep"
    );
    scratch.cleanup()
}

const OWNER: &str = "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {\n    if amount >= discount_threshold {\n        amount - 10\n    } else {\n        amount\n    }\n}\n";
const DIFF: &str = "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,5 +1,5 @@\n pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {\n-    if amount > discount_threshold {\n+    if amount >= discount_threshold {\n         amount - 10\n     } else {\n         amount\n";

#[test]
fn property_macro_quarantine_matches_runtime_collection_and_discrimination() -> Result<(), String> {
    let assertion = "assert_eq!(discounted_total(100, 100), 90);";
    for (name, tests, exposed, test_count, limit) in [
        ("ordinary", format!("#[test]\nfn boundary() {{ {assertion} }}\n"), true, 1, None),
        ("generic_direct", "macro_rules! prop_assert_eq { ($($args:tt)*) => {} }\n#[test]\nfn boundary() { prop_assert_eq!(discounted_total(100,100),90); assert_eq!(discounted_total::<for<'a> fn(&'a str)>(100,100),90); }\n".to_string(), true, 1, None),
        ("discarded_ensure", "macro_rules! proptest { ($($args:tt)*) => {} }\n#[test]\nfn boundary() {\n let _ = discounted_total(100,100);\n proptest! { ensure!(discounted_total(100,100) == 90, \"discarded\"); }\n}\n".to_string(), false, 1, None),
        ("opaque_declaration", "macro_rules! proptest { ($($args:tt)*) => {} }\n#[test]\nfn boundary() { proptest! { fn discounted_total() {} } }\n".to_string(), false, 1, None),
        ("noop_named_test", "macro_rules! prop_assert_eq { ($($args:tt)*) => {} }\n#[cfg(test)] mod tests {\n#[test]\nfn discounted_total() { prop_assert_eq!(super::discounted_total(100,100),90); }\n}\n".to_string(), false, 1, Some("rust_macro_reach_unresolved")),
        ("mixed_named_direct", "macro_rules! prop_assert_eq { ($($args:tt)*) => {} }\n#[cfg(test)] mod tests {\n#[test]\nfn discounted_total() { prop_assert_eq!(super::discounted_total(100,100),90); assert_eq!(super::discounted_total(100,100),90); }\n}\n".to_string(), true, 1, None),
        ("mixed_direct", format!("macro_rules! prop_assert_eq {{ ($($args:tt)*) => {{}} }}\n#[test]\nfn boundary() {{ prop_assert_eq!(discounted_total(100, 100), 90); {assertion} }}\n"), true, 1, None),
        ("mixed_nondiscriminating", "macro_rules! prop_assert_eq { ($($args:tt)*) => {} }\n#[test]\nfn boundary() { prop_assert_eq!(discounted_total(100, 100), 90); assert_eq!(discounted_total(90, 100), 90); }\n".to_string(), false, 1, None),
        ("mixed_nondiscriminating_reverse", "macro_rules! prop_assert_eq { ($($args:tt)*) => {} }\n#[test]\nfn boundary() { let _note = r#\"é [( ]\"#; assert_eq!(discounted_total(90, 100), 90); /* λ */ prop_assert_eq!(discounted_total(100, 100), 90); }\n".to_string(), false, 1, None),
        ("mixed_helper", "macro_rules! prop_assert_eq { ($($args:tt)*) => {} }\nfn bridge(a: i32, b: i32) -> i32 { discounted_total(a, b) }\n#[test]\nfn boundary() { prop_assert_eq!(discounted_total(100, 100), 90); assert_eq!(bridge(100, 100), 90); }\n".to_string(), true, 1, None),
        ("no_assertion", "#[test]\nfn boundary() { let _ = discounted_total(100, 100); }\n".to_string(), false, 1, None),
        ("noop_property_assertion", "macro_rules! prop_assert_eq { ($($args:tt)*) => {} }\n#[test]\nfn boundary() { prop_assert_eq!(discounted_total(100, 100), 90); }\n".to_string(), false, 1, Some("rust_macro_reach_unresolved")),
        ("noop_proptest", format!("macro_rules! proptest {{ ($($args:tt)*) => {{}} }}\nproptest! {{ #[test] fn boundary() {{ {assertion} }} }}\n"), false, 0, Some("rust_macro_reach_unresolved")),
        ("noop_quickcheck", format!("macro_rules! quickcheck {{ ($($args:tt)*) => {{}} }}\nquickcheck! {{ fn boundary() -> bool {{ {assertion} true }} }}\n"), false, 0, Some("rust_macro_reach_unresolved")),
        ("noop_qualified_collision", "macro_rules! proptest { ($($args:tt)*) => {} }\nproptest! { #[test] fn boundary() { assert_eq!(other::discounted_total(100, 100), 90); } }\n".to_string(), false, 0, Some("rust_macro_reach_unresolved")),
    ] {
        let scratch = Scratch::new()?;
        let root = &scratch.directory;
        std::fs::write(root.join("Cargo.toml"), "[package]\nname=\"property_quarantine\"\nversion=\"0.1.0\"\nedition=\"2024\"\n").map_err(|error| error.to_string())?;
        let owner = if name == "generic_direct" {
            OWNER.replace("discounted_total(", "discounted_total<T>(")
        } else {
            OWNER.to_string()
        };
        let source = format!("{owner}\n{tests}");
        if limit.is_some() {
            let retained = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../fixtures/property_macro_{name}/input/src/lib.rs"));
            assert_eq!(source, std::fs::read_to_string(retained).map_err(|error| error.to_string())?,
                "{name}: governed fixture and runtime subject must be identical");
        }

        std::fs::write(root.join("src/lib.rs"), &source).map_err(|error| error.to_string())?;
        let diff = if name == "generic_direct" {
            DIFF.replace("discounted_total(", "discounted_total<T>(")
        } else {
            DIFF.to_string()
        };
        std::fs::write(root.join("diff.patch"), diff).map_err(|error| error.to_string())?;
        let report = check_workspace(CheckInput {
            root: root.clone(), diff_file: Some(root.join("diff.patch")), mode: Mode::Fast,
            format: OutputFormat::Json, include_unchanged_tests: true, ..CheckInput::default()
        })?;
        assert_eq!(report.findings.len(), 1, "{name}: one intended predicate");
        if name != "mixed_helper" && name != "mixed_named_direct" && name != "generic_direct" { assert_eq!(report.findings[0].class == ExposureClass::Exposed, exposed, "{name}"); }
        let json: serde_json::Value = serde_json::from_str(&render_check(&report, &OutputFormat::Json)?).map_err(|error| error.to_string())?;
        let finding = &json["findings"][0];
        assert_eq!(finding["oracle_strength"], if exposed || name.starts_with("mixed_") { "strong" } else { "none" }, "{name}");
        // Generic argument value transfer has an independent static limitation.
        // This control proves retained call/oracle authority and runtime discrimination,
        // rather than granting stronger boundary activation from opaque text.
        if name == "generic_direct" {
            assert_eq!(finding["ripr"]["reach"]["state"], "yes", "ordinary generic call retains reach: {finding}");
            assert_eq!(finding["related_tests"].as_array().map(Vec::len), Some(1), "ordinary generic call retains its test: {finding}");
            // Hold owner, generic call, diff and ordinary assertion fixed. Removing
            // only the opaque invocation establishes the ordinary producer baseline.
            let baseline_source = source.replace("prop_assert_eq!(discounted_total(100,100),90); ", "");
            assert_ne!(baseline_source, source, "baseline must remove the opaque invocation");
            assert!(!baseline_source.contains("prop_assert_eq!("), "baseline must have no opaque invocation");
            std::fs::write(root.join("src/lib.rs"), &baseline_source).map_err(|error| error.to_string())?;
            let baseline = check_workspace(CheckInput {
                root: root.clone(), diff_file: Some(root.join("diff.patch")), mode: Mode::Fast,
                format: OutputFormat::Json, include_unchanged_tests: true, ..CheckInput::default()
            })?;
            std::fs::write(root.join("src/lib.rs"), &source).map_err(|error| error.to_string())?;
            assert_eq!(baseline.findings.len(), 1, "generic ordinary baseline");
            assert_eq!(report.findings[0].class, baseline.findings[0].class, "opacity cannot change ordinary generic classification");
            assert_eq!(report.findings[0].activation, baseline.findings[0].activation, "opacity cannot change ordinary generic activation support");
            let baseline: serde_json::Value = serde_json::from_str(&render_check(&baseline, &OutputFormat::Json)?).map_err(|error| error.to_string())?;
            let baseline = &baseline["findings"][0];
            assert_eq!(baseline["oracle_strength"], "strong", "generic ordinary baseline: {baseline}");
            assert_eq!(baseline["related_tests"].as_array().map(Vec::len), Some(1));
            for stage in ["reach", "infect", "propagate"] {
                assert_eq!(finding["ripr"][stage]["state"], baseline["ripr"][stage]["state"], "generic ordinary {stage} baseline: candidate={finding}; baseline={baseline}");
            }
            assert_eq!(finding["static_limit_kind"], baseline["static_limit_kind"], "generic ordinary support limit must survive");
        }
        if name == "mixed_named_direct" {
            assert_eq!(finding["ripr"]["reach"]["state"], "yes", "qualified ordinary call survives the declaration name: {finding}");
            assert_eq!(finding["related_tests"].as_array().map(Vec::len), Some(1));
        }
        if name == "mixed_helper" {
            assert_eq!(finding["ripr"]["reach"]["state"], "yes", "independent helper route: {finding}");
            assert!(finding["related_tests"].as_array().is_some_and(|tests| tests.iter().any(|test| test["relation_reason"] == "helper_owner_call")), "independent helper route: {finding}");
        }
        if name == "opaque_declaration" {
            // File proximity remains a suggestion, never a direct call. Opaque
            // expansion uncertainty must not become proof that tests are absent.
            let related = finding["related_tests"].as_array().ok_or("missing related tests")?;
            assert_eq!(related.len(), 1, "opaque declaration proximity: {finding}");
            assert_eq!(related[0]["relation_reason"], "same_test_file", "opaque declaration cannot supply a direct call: {finding}");
            assert_eq!(finding["ripr"]["reach"]["state"], "weak", "opaque declaration cannot supply positive reach: {finding}");
            let baseline_source = source.replace("proptest! { fn discounted_total() {} }", "proptest! {}");
            assert_ne!(baseline_source, source, "baseline must remove the opaque declaration");
            std::fs::write(root.join("src/lib.rs"), &baseline_source).map_err(|error| error.to_string())?;
            let baseline = check_workspace(CheckInput {
                root: root.clone(), diff_file: Some(root.join("diff.patch")), mode: Mode::Fast,
                format: OutputFormat::Json, include_unchanged_tests: true, ..CheckInput::default()
            })?;
            std::fs::write(root.join("src/lib.rs"), &source).map_err(|error| error.to_string())?;
            assert_eq!(baseline.findings.len(), 1, "empty opaque ordinary baseline");
            assert_eq!(report.findings[0].class, baseline.findings[0].class, "opaque declaration cannot add classification authority");
            assert_eq!(report.findings[0].related_tests, baseline.findings[0].related_tests, "opaque declaration cannot add a relation");
            assert_eq!(report.findings[0].ripr.reach.state, baseline.findings[0].ripr.reach.state, "opaque declaration cannot add reach authority");
        }
        if limit == Some("rust_macro_reach_unresolved") {
            for stage in ["reach", "infect", "propagate"] {
                assert_eq!(finding["ripr"][stage]["state"], "unknown", "{name}: discarded property arguments cannot establish {stage}");
            }
            assert_eq!(finding["related_tests"].as_array().map(Vec::len), Some(0), "{name}");
        }
        if let Some(limit) = limit {
            assert_eq!(finding["static_limit_kind"], limit, "{name}");
            let guidance = finding["recommended_next_step"].as_str().ok_or("missing guidance")?;
            assert!(guidance.contains("macro"), "{name}: {guidance}");
            assert!(!guidance.contains("Add "), "{name}: unsupported is not a new-test request");
            let human = render_check(&report, &OutputFormat::Human)?;
            assert!(human.contains("static_limited"), "{name}: {human}");
        }
        for wrong in [false, true] {
            let runtime_source = root.join("runtime.rs");
            std::fs::write(&runtime_source, if wrong { source.replace("amount >= discount_threshold", "amount > discount_threshold") } else { source.clone() }).map_err(|error| error.to_string())?;
            let binary = root.join(format!("runtime{}", std::env::consts::EXE_SUFFIX));
            let mut command = Command::new("rustc");
            command.arg("--edition=2024").arg("--test").arg(&runtime_source).arg("-o").arg(&binary);
            let build = run_bounded(command, root, "rustc", Duration::from_mins(2))?;
            assert!(build.status.success(), "{name}: compilation must succeed: {}", String::from_utf8_lossy(&build.stderr));
            let run = run_bounded(Command::new(&binary), root, "runtime", Duration::from_secs(10))?;
            let stdout = String::from_utf8_lossy(&run.stdout);
            assert!(stdout.contains(&format!("running {test_count} test")), "{name}: {stdout}");
            assert_eq!(run.status.success(), !(wrong && (exposed || name == "mixed_helper")), "{name}: wrong={wrong}: {stdout}");
        }
        scratch.cleanup()?;
    }
    Ok(())
}

/// A malformed neighboring source selects the real fallback path. Its ordinary
/// test control preserves prior static behavior; no runtime claim is made for
/// intentionally malformed source.
#[test]
fn property_macro_fallback_has_no_synthetic_test_evidence() -> Result<(), String> {
    for (name, tests, opaque) in [
        (
            "ordinary",
            "#[test]\nfn boundary() { assert_eq!(discounted_total(100,100),90); }\n",
            false,
        ),
        (
            "proptest",
            "proptest! {\n#[test]\nfn boundary() { assert_eq!(discounted_total(100,100),90); }\n}\n",
            true,
        ),
        (
            "quickcheck",
            "quickcheck /* boundary */ ! {\n#[test]\nfn boundary() { assert_eq!(discounted_total(100,100),90); }\n}\n",
            true,
        ),
        (
            "malformed",
            "proptest! { ([)\n#[test]\nfn boundary() { assert_eq!(discounted_total(100,100),90); }\n",
            true,
        ),
    ] {
        let scratch = Scratch::new()?;
        let root = &scratch.directory;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname=\"fallback_property\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(root.join("src/lib.rs"), OWNER).map_err(|error| error.to_string())?;
        std::fs::create_dir(root.join("tests")).map_err(|error| error.to_string())?;
        std::fs::write(
            root.join("tests/opaque.rs"),
            format!("this is invalid Rust;\n{tests}"),
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(root.join("diff.patch"), DIFF).map_err(|error| error.to_string())?;
        let report = check_workspace(CheckInput {
            root: root.clone(),
            diff_file: Some(root.join("diff.patch")),
            mode: Mode::Fast,
            format: OutputFormat::Json,
            include_unchanged_tests: true,
            ..CheckInput::default()
        })?;
        assert_eq!(report.findings.len(), 1, "{name}");
        let json: serde_json::Value =
            serde_json::from_str(&render_check(&report, &OutputFormat::Json)?)
                .map_err(|error| error.to_string())?;
        let finding = &json["findings"][0];
        if opaque {
            assert!(
                finding["related_tests"]
                    .as_array()
                    .is_some_and(Vec::is_empty),
                "{name}: {finding}"
            );
            assert_eq!(finding["oracle_strength"], "none", "{name}");
            assert_ne!(finding["classification"], "exposed", "{name}");
            assert_eq!(
                finding["static_limit_kind"], "rust_macro_reach_unresolved",
                "{name}: {finding}"
            );
        } else {
            assert_eq!(
                finding["classification"], "exposed",
                "ordinary fallback behavior is unchanged"
            );
        }
        scratch.cleanup()?;
    }
    Ok(())
}

/// Known unrelated packages cannot convert a genuine gap into a limitation.
/// Same-package paths with nonstandard layouts retain a lexical limitation;
/// a same-owner ordinary assertion keeps its independent evidence.
#[test]
fn property_mentions_respect_known_package_boundaries() -> Result<(), String> {
    for (name, mention_path, ordinary, limited) in [
        ("unrelated", "crates/a/src/lib.rs", false, false),
        ("same", "crates/b/src/property.rs", false, true),
        (
            "same_nonstandard",
            "crates/b/custom/nested/src/property.rs",
            false,
            true,
        ),
        ("root_manifest", "loose.rs", false, false),
        ("ordinary", "crates/a/src/lib.rs", true, false),
    ] {
        let scratch = Scratch::new()?;
        let root = &scratch.directory;
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers=[\"crates/b\",\"crates/a\"]\nresolver=\"3\"\n",
        )
        .map_err(|error| error.to_string())?;
        for package in ["a", "b"] {
            std::fs::create_dir_all(root.join(format!("crates/{package}/src")))
                .map_err(|error| error.to_string())?;
            std::fs::write(
                root.join(format!("crates/{package}/Cargo.toml")),
                format!(
                    "[package]\nname=\"package_{package}\"\nversion=\"0.1.0\"\nedition=\"2024\"\n"
                ),
            )
            .map_err(|error| error.to_string())?;
        }
        let source = if ordinary {
            format!(
                "{OWNER}\n#[test] fn boundary() {{ assert_eq!(discounted_total(100,100),90); }}\n"
            )
        } else {
            OWNER.to_string()
        };
        std::fs::write(root.join("crates/b/src/lib.rs"), source)
            .map_err(|error| error.to_string())?;
        std::fs::write(root.join("crates/a/src/lib.rs"), "").map_err(|error| error.to_string())?;
        std::fs::create_dir_all(
            root.join(mention_path)
                .parent()
                .ok_or("missing fixture parent")?,
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(root.join(mention_path), "macro_rules! proptest { ($($tt:tt)*) => {} }\nproptest! { #[test] fn unrelated() { discounted_total(0,0); } }\n").map_err(|error| error.to_string())?;
        // Include an unrelated witness that sorts first: a later in-package
        // witness must not disappear behind the first same-name index entry.
        if limited {
            std::fs::write(
                root.join("crates/a/src/lib.rs"),
                "quickcheck! { fn ignored() { discounted_total(0,0); } }\n",
            )
            .map_err(|error| error.to_string())?;
        }
        std::fs::write(
            root.join("diff.patch"),
            DIFF.replace("src/lib.rs", "crates/b/src/lib.rs"),
        )
        .map_err(|error| error.to_string())?;
        let report = check_workspace(CheckInput {
            root: root.clone(),
            diff_file: Some(root.join("diff.patch")),
            mode: Mode::Deep,
            format: OutputFormat::Json,
            include_unchanged_tests: true,
            ..CheckInput::default()
        })?;
        assert_eq!(report.findings.len(), 1, "{name}");
        let json_text = render_check(&report, &OutputFormat::Json)?;
        let json: serde_json::Value =
            serde_json::from_str(&json_text).map_err(|error| error.to_string())?;
        let finding = &json["findings"][0];
        assert_eq!(
            finding["classification"],
            if ordinary {
                "exposed"
            } else {
                "no_static_path"
            },
            "{name}: {finding}"
        );
        assert_eq!(
            finding["static_limit_kind"].as_str(),
            limited.then_some("rust_macro_reach_unresolved"),
            "{name}: {finding}"
        );
        let human = render_check(&report, &OutputFormat::Human)?;
        let plain_action = "review the unresolved static path and existing tests";
        let named_action = "inspect the named static limitation";
        if ordinary {
            assert!(!human.contains(plain_action), "{name}: {human}");
            assert!(!human.contains(named_action), "{name}: {human}");
        } else {
            assert!(human.contains("State: limited by static analysis (static_limited)"));
            assert_eq!(human.contains(plain_action), !limited, "{name}: {human}");
            assert_eq!(human.contains(named_action), limited, "{name}: {human}");
        }
        assert_eq!(render_check(&report, &OutputFormat::Json)?, json_text);
        scratch.cleanup()?;
    }
    Ok(())
}
