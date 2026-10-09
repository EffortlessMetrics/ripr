//! Real Cargo regressions for the fresh-build transaction (#5036, #5038).
//!
//! Both workspaces exist before the first build. Their same-name/version path
//! dependency has older, equal-length but different source bytes. Cargo can
//! falsely consider it fresh when intermediate storage is shared, even though
//! the newly compiled application correctly identifies workspace B.
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{
    BuildIdentity, BuildLocation, build_fresh_binary, pretty_json, sha256_file,
    validate_build_identity,
};
use crate::run::capture_bytes_in_dir_with_timeout;

const CHILD_ROOT: &str = "RIPR_HOST_BUILD_TEST_ROOT";
const EXPECTED_CONFIG: &str = "RIPR_HOST_BUILD_EXPECTED_CONFIG";
const CONFIG_MARKER: &str = "RIPR_HOST_BUILD_CONFIG_MARKER";
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
             println!(\"subject={subject} core={{}} config={{}}\", host_build_core::behavior(),\n\
             option_env!(\"RIPR_HOST_BUILD_CONFIG_MARKER\").unwrap_or(\"absent\"));\n\
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
#[ignore = "subprocess entry, exercised by the fresh-build regression tests"]
fn fresh_build_child() -> Result<(), String> {
    let root = std::env::var_os(CHILD_ROOT).ok_or("missing child fixture root")?;
    let root = PathBuf::from(root);
    let expected_config = std::env::var(EXPECTED_CONFIG).map_err(|error| error.to_string())?;
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
        validate_build_identity(&attempt, &build, BuildLocation::Attempt(&attempt))?;
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
            expected: format!("subject={subject} core={behavior} config={expected_config}"),
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

#[derive(Clone, Copy, Default)]
struct Overrides {
    inherited: bool,
    repository: bool,
    ancestor: bool,
    cargo_home: bool,
}

fn run_sequence(root: &Path, overrides: Overrides) -> Result<(), String> {
    fixture(&root.join("A"), "A", "OLD")?;
    fixture(&root.join("B"), "B", "NEW")?;
    let mut configurations = Vec::new();
    if overrides.repository {
        configurations.push(root.join("A/.cargo/config.toml"));
        configurations.push(root.join("B/.cargo/config.toml"));
    }
    if overrides.ancestor {
        configurations.push(root.join(".cargo/config.toml"));
    }
    let cargo_home = root.join("cargo-home");
    if overrides.cargo_home {
        configurations.push(cargo_home.join("config.toml"));
    }
    // Each config redirects to a distinct shared directory so the mixed case
    // exercises file/environment precedence as well as command-line authority.
    let mut configuration_digests = Vec::new();
    let mut redirected_directories = Vec::new();
    for (index, configuration) in configurations.iter().enumerate() {
        let redirect = root.join(format!("config-intermediates-{index}"));
        // The two repository configs must share intermediates for A/B to expose
        // false freshness. Ancestor and home configs already apply to both.
        let redirect = if overrides.repository && index < 2 {
            root.join("repository-intermediates")
        } else {
            redirect
        };
        redirected_directories.push(redirect.clone());
        let redirect = redirect.to_str().ok_or("config redirect is not UTF-8")?;
        write(
            configuration,
            format!(
                "[build]\nbuild-dir = {}\n[env]\n{CONFIG_MARKER} = {{ value = \"preserved\", force = true }}\n",
                toml::Value::String(redirect.to_string()),
            ),
        )?;
        configuration_digests.push(sha256_file(configuration)?);
    }
    let expected_config = if configurations.is_empty() {
        "absent"
    } else {
        "preserved"
    };
    let root_text = root.to_str().ok_or("fixture root is not UTF-8")?;
    let shared = root.join("shared-intermediates");
    let shared_text = shared.to_str().ok_or("shared path is not UTF-8")?;
    let mut envs = vec![
        (CHILD_ROOT, root_text),
        (EXPECTED_CONFIG, expected_config),
        ("CARGO_INCREMENTAL", "0"),
        ("CARGO_TERM_VERBOSE", "true"),
    ];
    if overrides.inherited {
        envs.push(("CARGO_BUILD_BUILD_DIR", shared_text));
    }
    if overrides.cargo_home {
        envs.push((
            "CARGO_HOME",
            cargo_home.to_str().ok_or("Cargo home is not UTF-8")?,
        ));
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
        &["CARGO_BUILD_BUILD_DIR", CONFIG_MARKER],
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
    for (configuration, digest) in configurations.iter().zip(configuration_digests) {
        if sha256_file(configuration)? != digest {
            return Err(format!(
                "caller configuration changed: {}",
                configuration.display()
            ));
        }
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
    if shared.exists() || redirected_directories.iter().any(|path| path.exists()) {
        return Err(
            "fresh build wrote to a caller's redirected intermediate directory".to_string(),
        );
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
    run_sequence(&root.join("ordinary"), Overrides::default())?;
    run_sequence(
        &root.join("inherited"),
        Overrides {
            inherited: true,
            ..Overrides::default()
        },
    )?;
    finish_successful_fixture(&root)
}

#[test]
fn fresh_build_overrides_configured_intermediates() -> Result<(), String> {
    let root = super::tests::scratch("build-config-isolation")?;
    let cases = [
        (
            "repository's λ space",
            Overrides {
                repository: true,
                ..Overrides::default()
            },
        ),
        (
            "ancestor",
            Overrides {
                ancestor: true,
                ..Overrides::default()
            },
        ),
        (
            "cargo-home",
            Overrides {
                cargo_home: true,
                ..Overrides::default()
            },
        ),
        (
            "repository-and-environment",
            Overrides {
                repository: true,
                inherited: true,
                ..Overrides::default()
            },
        ),
        (
            "all-precedence-layers",
            Overrides {
                repository: true,
                ancestor: true,
                cargo_home: true,
                inherited: true,
            },
        ),
    ];
    let mut failures = Vec::new();
    for (label, overrides) in cases {
        if let Err(error) = run_sequence(&root.join(label), overrides) {
            failures.push(format!("{label}: {error}"));
        }
    }
    if !failures.is_empty() {
        return Err(failures.join("\n"));
    }
    finish_successful_fixture(&root)
}

fn finish_successful_fixture(root: &Path) -> Result<(), String> {
    // Only this successful invocation's owned scratch is disposable. Failed
    // attempts return above, and deliberate qualification can retain all bytes.
    if std::env::var(KEEP_EVIDENCE).as_deref() == Ok("1") {
        println!("retained successful build evidence: {}", root.display());
    } else {
        fs::remove_dir_all(root).map_err(|error| error.to_string())?;
        println!("removed successful build fixture: {}", root.display());
    }
    Ok(())
}
