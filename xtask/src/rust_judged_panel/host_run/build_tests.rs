//! Real Cargo regression for the fresh-build transaction (#5036).
//!
//! Both workspaces exist before the first build. Their same-name/version path
//! dependency has older, equal-length but different source bytes. Cargo can
//! falsely consider it fresh when intermediate storage is shared, even though
//! the newly compiled application correctly identifies workspace B.
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{BuildIdentity, build_fresh_binary, pretty_json, sha256_file, validate_build_identity};
use crate::run::capture_bytes_in_dir_with_timeout;

const CHILD_ROOT: &str = "RIPR_HOST_BUILD_TEST_ROOT";
const KEEP_EVIDENCE: &str = "RIPR_HOST_BUILD_KEEP_EVIDENCE";
const CHILD_TEST: &str = "rust_judged_panel::host_run::build_tests::fresh_build_child";
const VERSION: &str = "0.0.1";

fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), String> {
    let parent = path.parent().ok_or("fixture path has no parent")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    fs::write(path, contents).map_err(|error| error.to_string())
}

fn fixture(root: &Path, subject: &str, behavior: &str) -> Result<(), String> {
    write(
        &root.join("Cargo.toml"),
        format!(
            "[workspace]\nmembers = [\"app\", \"core\"]\nresolver = \"2\"\n\
             [workspace.package]\nversion = \"{VERSION}\"\n"
        ),
    )?;
    write(
        &root.join("Cargo.lock"),
        "version = 4\n\n[[package]]\nname = \"host-build-core\"\nversion = \"0.0.1\"\n\n\
         [[package]]\nname = \"ripr\"\nversion = \"0.0.1\"\n\
         dependencies = [\"host-build-core\"]\n",
    )?;
    write(
        &root.join("core/Cargo.toml"),
        "[package]\nname = \"host-build-core\"\nversion.workspace = true\nedition = \"2024\"\n",
    )?;
    let core = root.join("core/src/lib.rs");
    write(
        &core,
        format!("pub fn behavior() -> &'static str {{ \"{behavior}\" }}\n"),
    )?;
    // Avoid depending on the filesystem clock resolution for the stale-core
    // control. Both dependency sources predate every build in this fixture.
    fs::OpenOptions::new()
        .write(true)
        .open(core)
        .and_then(|file| file.set_modified(UNIX_EPOCH + Duration::from_hours(262_968)))
        .map_err(|error| error.to_string())?;
    write(
        &root.join("app/Cargo.toml"),
        "[package]\nname = \"ripr\"\nversion.workspace = true\nedition = \"2024\"\n\
         [dependencies]\nhost-build-core = { path = \"../core\" }\n",
    )?;
    write(
        &root.join("app/src/main.rs"),
        format!(
            "fn main() {{\n\
             if std::env::args().any(|arg| arg == \"--version\") {{\n\
             println!(\"ripr {{}}\", env!(\"CARGO_PKG_VERSION\"));\n\
             }} else {{\n\
             println!(\"subject={subject} core={{}}\", host_build_core::behavior());\n\
             }}\n}}\n"
        ),
    )
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Observation {
    subject: String,
    expected: String,
    actual: String,
    source_sha256: String,
    build: BuildIdentity,
}

#[test]
#[ignore = "subprocess entry, exercised by fresh_build_ignores_inherited_intermediates"]
fn fresh_build_child() -> Result<(), String> {
    let root = std::env::var_os(CHILD_ROOT).ok_or("missing child fixture root")?;
    let root = PathBuf::from(root);
    let mut observations = Vec::new();
    for (index, subject, behavior) in [(0, "A", "OLD"), (1, "B", "NEW"), (2, "A", "OLD")] {
        let workspace = root.join(subject);
        // Force the application to rebuild while leaving the path dependency's
        // old mtime intact. This distinguishes current application identity
        // from current dependency behavior, including the return to A.
        fs::OpenOptions::new()
            .write(true)
            .open(workspace.join("app/src/main.rs"))
            .and_then(|file| file.set_modified(SystemTime::now() + Duration::from_secs(2)))
            .map_err(|error| error.to_string())?;
        let attempt = root.join(format!("attempt-{index}-{subject}"));
        let build = build_fresh_binary(&workspace, &attempt)?;
        validate_build_identity(&attempt, &build)?;
        let output = capture_bytes_in_dir_with_timeout(
            Path::new(&build.executed_binary_path),
            &[],
            &workspace,
            &[],
            &[],
            Duration::from_secs(10),
            "fresh-build fixture behavior",
        )?;
        write(&attempt.join("behavior-stdout.bin"), &output.stdout)?;
        write(&attempt.join("behavior-stderr.bin"), &output.stderr)?;
        if output.timed_out || !output.status.is_some_and(|status| status.success()) {
            return Err(format!(
                "fixture binary did not execute successfully; evidence: {}; stdout: {}; stderr: {}",
                attempt.display(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            ));
        }
        let actual = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
        let observation = Observation {
            subject: subject.to_string(),
            expected: format!("subject={subject} core={behavior}"),
            actual: actual.trim().to_string(),
            source_sha256: sha256_file(&workspace.join("core/src/lib.rs"))?,
            build,
        };
        println!("{}", String::from_utf8_lossy(&pretty_json(&observation)?));
        observations.push(observation);
        write(&root.join("observations.json"), pretty_json(&observations)?)?;
    }
    Ok(())
}

fn run_sequence(root: &Path, inherit_build_dir: bool) -> Result<(), String> {
    fixture(&root.join("A"), "A", "OLD")?;
    fixture(&root.join("B"), "B", "NEW")?;
    let root_text = root.to_str().ok_or("fixture root is not UTF-8")?;
    let shared = root.join("shared-intermediates");
    let shared_text = shared.to_str().ok_or("shared path is not UTF-8")?;
    let mut envs = vec![
        (CHILD_ROOT, root_text),
        ("CARGO_INCREMENTAL", "0"),
        ("CARGO_TERM_VERBOSE", "true"),
    ];
    if inherit_build_dir {
        envs.push(("CARGO_BUILD_BUILD_DIR", shared_text));
    }
    let output = capture_bytes_in_dir_with_timeout(
        &std::env::current_exe().map_err(|error| error.to_string())?,
        &[
            "--exact".to_string(),
            CHILD_TEST.to_string(),
            "--ignored".to_string(),
            "--nocapture".to_string(),
        ],
        &root.join("A"),
        &envs,
        &["CARGO_BUILD_BUILD_DIR"],
        Duration::from_mins(2),
        "fresh-build isolated test child",
    )?;
    write(&root.join("child-stdout.bin"), &output.stdout)?;
    write(&root.join("child-stderr.bin"), &output.stderr)?;
    if output.timed_out || !output.status.is_some_and(|status| status.success()) {
        return Err(format!(
            "fresh-build child failed; evidence: {}; stdout: {}; stderr: {}",
            root.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        ));
    }
    let observations: Vec<Observation> = serde_json::from_slice(
        &fs::read(root.join("observations.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if observations.len() != 3 {
        return Err("A -> B -> A requires three executed subjects".to_string());
    }
    let mut binaries = std::collections::BTreeSet::new();
    for observation in &observations {
        if observation.actual != observation.expected {
            return Err(format!(
                "fresh-build behavior mismatch: expected `{}`, observed `{}`; evidence: {}",
                observation.expected,
                observation.actual,
                root.display(),
            ));
        }
        let build = &observation.build;
        if build.binary_bytes == 0
            || build.binary_sha256 != sha256_file(Path::new(&build.executed_binary_path))?
            || !build
                .binary_version
                .split_whitespace()
                .any(|part| part == VERSION)
            || !binaries.insert(&build.executed_binary_path)
        {
            return Err("fixture build identity is empty, stale or reused".to_string());
        }
    }
    if observations[0].source_sha256 == observations[1].source_sha256 {
        return Err("fixture dependency implementations must differ".to_string());
    }
    println!(
        "three actual A -> B -> A builds passed; fixture: {}",
        root.display()
    );
    Ok(())
}

#[test]
fn fresh_build_ignores_inherited_intermediates() -> Result<(), String> {
    let root = super::tests::scratch("build-isolation")?;
    // Ordinary isolated behavior is a separate control; setting the hostile
    // environment is confined to the libtest child, never this process.
    run_sequence(&root.join("ordinary"), false)?;
    run_sequence(&root.join("inherited"), true)?;
    // Only this successful invocation's owned scratch is disposable. Failed
    // attempts return above, and deliberate qualification can retain all bytes.
    if std::env::var(KEEP_EVIDENCE).as_deref() == Ok("1") {
        println!("retained successful build evidence: {}", root.display());
    } else {
        fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
        println!("removed successful build fixture: {}", root.display());
    }
    Ok(())
}
