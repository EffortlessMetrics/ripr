//! Actual separate source/controller fixture for the #4510 admission seam.
//! This characterizes registry machinery; it does not qualify an installation.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::{
    CandidateOperation, PINNED_JSON, RegistryDocument, controllers, evaluate_candidate_registry,
    pinned_artifact, read_artifact_tree, render_projection, repository_registry,
    resolve_candidate_authority, retire_template, sha256_hex, tree,
};

struct OwnedFixture {
    root: PathBuf,
    finished: bool,
}

impl OwnedFixture {
    fn finish(mut self) -> Result<(), String> {
        fs::remove_dir_all(&self.root)
            .map_err(|error| format!("owned fixture cleanup failed: {error}"))?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for OwnedFixture {
    fn drop(&mut self) {
        if !self.finished {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn git(root: &Path, values: &[&str]) -> Result<String, String> {
    let args = values
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    let output = crate::run::capture_bytes_in_dir_with_timeout(
        Path::new("git"),
        &args,
        root,
        &[],
        &["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"],
        Duration::from_secs(30),
        "4510 separate-source fixture Git",
    )?;
    if output.timed_out || !output.status.is_some_and(|status| status.success()) {
        return Err(format!(
            "fixture Git {values:?} in {} failed: timed_out={}, status={:?}, duration={:?}, stdout(first 4096 bytes)={}, stderr(first 4096 bytes)={}",
            root.display(),
            output.timed_out,
            output.status,
            output.duration,
            String::from_utf8_lossy(
                output
                    .stdout
                    .get(..output.stdout.len().min(4096))
                    .unwrap_or_default()
            ),
            String::from_utf8_lossy(
                output
                    .stderr
                    .get(..output.stderr.len().min(4096))
                    .unwrap_or_default()
            )
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|error| format!("fixture Git output is not UTF-8: {error}"))
}

fn require_refusal<T>(result: Result<T, String>, expected: &str) -> Result<(), String> {
    match result {
        Ok(_) => Err(format!("custody unexpectedly accepted {expected}")),
        Err(error) if error.contains(expected) => Ok(()),
        Err(error) => Err(format!(
            "expected custody refusal {expected}; actual unrelated failure: {error}"
        )),
    }
}

fn initialize(root: &Path) -> Result<(), String> {
    git(root, &["init", "--initial-branch=main"])?;
    for (key, value) in [
        ("user.name", "RIPR fixture"),
        ("user.email", "fixture@example.invalid"),
        ("core.autocrlf", "false"),
        ("core.fsmonitor", "false"),
        ("commit.gpgsign", "false"),
        ("tag.gpgsign", "false"),
        ("core.hooksPath", ".fixture-empty-hooks"),
    ] {
        git(root, &["config", "--local", key, value])?;
    }
    Ok(())
}

fn replace_row(
    registry: &mut Value,
    artifact: &[u8],
    sha: &str,
    tree_id: &str,
    reference: &str,
) -> Result<(), String> {
    let row = registry
        .get_mut("artifacts")
        .and_then(Value::as_array_mut)
        .and_then(|rows| {
            rows.iter_mut()
                .find(|row| row.get("path") == Some(&json!(PINNED_JSON)))
        })
        .ok_or_else(|| "legal fixture has no candidate row".to_string())?;
    let object = row
        .as_object_mut()
        .ok_or_else(|| "legal candidate row is not an object".to_string())?;
    object.insert("sha256".to_string(), json!(sha256_hex(artifact)));
    object.insert(
        "candidate".to_string(),
        json!({"sha": sha, "tree": tree_id, "ref": reference}),
    );
    Ok(())
}

fn create_source_fixture() -> Result<SourceFixture, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("fixture clock: {error}"))?
        .as_nanos();
    let owned =
        std::env::temp_dir().join(format!("ripr-4510-source-{}-{stamp}", std::process::id()));
    fs::create_dir(&owned).map_err(|error| format!("exclusive fixture root: {error}"))?;
    let guard = OwnedFixture {
        root: owned,
        finished: false,
    };
    let source = guard.root.join("source é");
    let controller = guard.root.join("controller with spaces");
    fs::create_dir(&source).map_err(|error| error.to_string())?;
    fs::create_dir(&controller).map_err(|error| error.to_string())?;
    initialize(&source)?;
    fs::create_dir(source.join("src")).map_err(|error| error.to_string())?;
    fs::write(source.join("Cargo.toml"),
        "[package]\nname='ripr'\nversion='0.11.0'\nedition='2024'\ninclude=['src/**','Cargo.toml','Cargo.lock']\n\n[workspace]\n")
        .map_err(|error| error.to_string())?;
    fs::write(
        source.join("src/main.rs"),
        "fn main() { println!(\"fixture source A\"); }\n",
    )
    .map_err(|error| error.to_string())?;
    fs::write(
        source.join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.
version = 4

[[package]]
name = 'ripr'
version = '0.11.0'
",
    )
    .map_err(|error| error.to_string())?;
    fs::write(source.join(".gitignore"), "src/foreign.rs\n").map_err(|error| error.to_string())?;
    fs::write(source.join("source.bin"), [0, b'\n', 255, b' ', b'\n'])
        .map_err(|error| error.to_string())?;
    let fixture_input = source.join("fixtures/boundary_gap/input");
    fs::create_dir_all(&fixture_input).map_err(|error| error.to_string())?;
    fs::write(fixture_input.join("Cargo.toml"), "selected fixture A\n")
        .map_err(|error| error.to_string())?;
    git(
        &source,
        &[
            "add",
            "--",
            "Cargo.toml",
            "Cargo.lock",
            "src/main.rs",
            ".gitignore",
            "source.bin",
            "fixtures",
        ],
    )?;
    git(
        &source,
        &["commit", "-m", "fixture candidate before controller"],
    )?;
    let sha = git(&source, &["rev-parse", "HEAD"])?;
    let tree_id = git(&source, &["rev-parse", "HEAD^{tree}"])?;
    let reference = format!("refs/tags/ripr-release-0.11.0-{sha}");
    git(&source, &["update-ref", &reference, &sha])?;
    if git(&source, &["rev-parse", &reference])? != sha {
        return Err("fixture ref does not name the actual source commit".to_string());
    }

    let mut artifact: Value =
        serde_json::from_slice(&pinned_artifact()).map_err(|error| error.to_string())?;
    artifact
        .as_object_mut()
        .ok_or_else(|| "legal candidate artifact is not an object".to_string())?
        .insert("selected_swarm_parent".to_string(), json!(sha));
    let artifact = serde_json::to_vec_pretty(&artifact).map_err(|error| error.to_string())?;
    let mut registry = retire_template(repository_registry());
    replace_row(&mut registry, &artifact, &sha, &tree_id, &reference)?;
    let fixture = tree(&registry, &[(PINNED_JSON, artifact.clone())]);
    // Write the actual controller snapshot, including its derived projection.
    let document: RegistryDocument =
        serde_json::from_value(registry).map_err(|error| error.to_string())?;
    if render_projection(&document.artifacts).is_empty() {
        return Err("fixture projection unexpectedly empty".to_string());
    }
    initialize(&controller)?;
    for (path, bytes) in &fixture.files {
        let destination = controller.join(path);
        let parent = destination
            .parent()
            .ok_or_else(|| "fixture path has no parent".to_string())?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        fs::write(destination, bytes).map_err(|error| error.to_string())?;
    }
    fs::create_dir(controller.join("policy")).map_err(|error| error.to_string())?;
    fs::write(
        controller.join("policy/release-targets.toml"),
        include_bytes!("../../../../../../policy/release-targets.toml"),
    )
    .map_err(|error| error.to_string())?;
    git(&controller, &["add", "--", "docs", "policy"])?;
    git(
        &controller,
        &["commit", "-m", "control packet after source selection"],
    )?;
    if git(&controller, &["rev-parse", "HEAD"])? == sha {
        return Err("controller must be distinct from immutable source commit".to_string());
    }
    Ok(SourceFixture {
        guard,
        source,
        controller,
        sha,
        reference,
        artifact,
    })
}

struct SourceFixture {
    guard: OwnedFixture,
    source: PathBuf,
    controller: PathBuf,
    sha: String,
    reference: String,
    artifact: Vec<u8>,
}

#[test]
fn registered_source_fixture_precedes_separate_controller_commit() -> Result<(), String> {
    let SourceFixture {
        guard,
        source,
        controller,
        sha,
        artifact,
        ..
    } = create_source_fixture()?;
    let readback = read_artifact_tree(&controller);
    let outcome = evaluate_candidate_registry(&readback, &controllers());
    let validated = outcome.validated().map_err(|error| {
        format!(
            "legal actual-source fixture invalid: {error}; {:?}",
            outcome.violations
        )
    })?;
    let granted = resolve_candidate_authority(
        &validated,
        "0.11.0",
        &artifact,
        CandidateOperation::ExactCandidate,
    )?;
    if granted.candidate_sha() != Some(sha.as_str()) {
        return Err("registry did not retain actual candidate SHA".to_string());
    }
    if !git(
        &source,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?
    .is_empty()
    {
        return Err("candidate source unexpectedly dirty".to_string());
    }
    let mut changed = artifact;
    changed.push(b' ');
    if resolve_candidate_authority(
        &validated,
        "0.11.0",
        &changed,
        CandidateOperation::ExactCandidate,
    )
    .is_ok()
    {
        return Err("changed actual controller artifact bytes admitted".to_string());
    }
    guard.finish()
}

#[test]
fn admitted_source_rechecks_actual_git_and_controller_bytes() -> Result<(), String> {
    use crate::reports::release::candidate_harness::{AdmittedSource, QualificationInput};
    let fixture = create_source_fixture()?;
    let input = QualificationInput::new(
        fixture.controller.clone(),
        fixture.source.clone(),
        PathBuf::from(PINNED_JSON),
    )?;
    let admitted = AdmittedSource::admit(&input, "0.11.0")?;
    let original_controller = read_artifact_tree(&fixture.controller);
    let mut wrong_tree_registry = retire_template(repository_registry());
    replace_row(
        &mut wrong_tree_registry,
        &fixture.artifact,
        &fixture.sha,
        &"0".repeat(40),
        &fixture.reference,
    )?;
    let wrong_tree = tree(
        &wrong_tree_registry,
        &[(PINNED_JSON, fixture.artifact.clone())],
    );
    let wrong_tree_outcome = evaluate_candidate_registry(&wrong_tree, &controllers());
    wrong_tree_outcome
        .validated()
        .map_err(|error| format!("wrong-tree registry fixture is not legal: {error}"))?;
    for (path, bytes) in &wrong_tree.files {
        fs::write(fixture.controller.join(path), bytes).map_err(|error| error.to_string())?;
    }
    require_refusal(
        AdmittedSource::admit(&input, "0.11.0"),
        &format!(
            "candidate source identity changed at {}^{{tree}}",
            fixture.sha
        ),
    )?;
    for (path, bytes) in &original_controller.files {
        fs::write(fixture.controller.join(path), bytes).map_err(|error| error.to_string())?;
    }
    AdmittedSource::admit(&input, "0.11.0")?.revalidate()?;
    if admitted.committed_file("source.bin") != Some([0, b'\n', 255, b' ', b'\n'].as_slice()) {
        return Err("Git binary batch capture changed non-UTF8/NUL/newline blob bytes".to_string());
    }
    if admitted.source_sha()? != fixture.sha
        || admitted.root()
            != fixture
                .source
                .canonicalize()
                .map_err(|error| error.to_string())?
    {
        return Err("source admission ignored selected source root/identity".to_string());
    }
    git(
        &fixture.source,
        &[
            "commit",
            "--allow-empty",
            "-m",
            "different actual source identity",
        ],
    )?;
    let other_sha = git(&fixture.source, &["rev-parse", "HEAD"])?;
    require_refusal(
        admitted.revalidate(),
        "candidate source identity changed at HEAD",
    )?;
    git(&fixture.source, &["checkout", "--detach", &fixture.sha])?;
    admitted.revalidate()?;
    git(
        &fixture.source,
        &["update-ref", &fixture.reference, &other_sha],
    )?;
    require_refusal(
        admitted.revalidate(),
        &format!("candidate source identity changed at {}", fixture.reference),
    )?;
    git(
        &fixture.source,
        &["update-ref", &fixture.reference, &fixture.sha],
    )?;
    admitted.revalidate()?;
    fs::write(
        fixture.source.join("src/main.rs"),
        "fn main() {}
",
    )
    .map_err(|error| error.to_string())?;
    require_refusal(
        admitted.revalidate(),
        "candidate source checkout is not clean",
    )?;
    git(&fixture.source, &["checkout", "--", "src/main.rs"])?;
    admitted.revalidate()?;
    let path = fixture.controller.join(PINNED_JSON);
    let mut changed = fixture.artifact.clone();
    changed.push(b' ');
    fs::write(&path, changed).map_err(|error| error.to_string())?;
    require_refusal(admitted.revalidate(), "candidate_digest")?;
    fs::write(path, &fixture.artifact).map_err(|error| error.to_string())?;
    admitted.revalidate()?;
    fixture.guard.finish()
}

#[test]
fn source_admission_refuses_same_alias_and_nested_physical_roots() -> Result<(), String> {
    use crate::reports::release::candidate_harness::{AdmittedSource, QualificationInput};
    let fixture = create_source_fixture()?;
    let artifact = PathBuf::from(PINNED_JSON);
    let separate = QualificationInput::new(
        fixture.controller.clone(),
        fixture.source.clone(),
        artifact.clone(),
    )?;
    AdmittedSource::admit(&separate, "0.11.0")?;
    let nested_source = fixture.controller.join("nested source");
    fs::create_dir(&nested_source).map_err(|error| error.to_string())?;
    for source in [
        fixture.controller.clone(),
        fixture.controller.join("."),
        nested_source,
    ] {
        let input = QualificationInput::new(fixture.controller.clone(), source, artifact.clone())?;
        require_refusal(
            AdmittedSource::admit(&input, "0.11.0"),
            "source/controller roots must be physically separate",
        )?;
    }
    // The other containment direction uses an actual valid controller tree,
    // copied below the source. The topology guard must win before dirty-source
    // or wrong-HEAD errors; no admitted handle is fabricated.
    let nested_controller = fixture.source.join("nested controller");
    fs::create_dir(&nested_controller).map_err(|error| error.to_string())?;
    let tree = read_artifact_tree(&fixture.controller);
    for (path, bytes) in tree.files {
        let output = nested_controller.join(path);
        let parent = output
            .parent()
            .ok_or_else(|| "nested controller file parent missing".to_string())?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        fs::write(output, bytes).map_err(|error| error.to_string())?;
    }
    fs::create_dir(nested_controller.join("policy")).map_err(|error| error.to_string())?;
    fs::copy(
        fixture.controller.join("policy/release-targets.toml"),
        nested_controller.join("policy/release-targets.toml"),
    )
    .map_err(|error| error.to_string())?;
    let input = QualificationInput::new(nested_controller, fixture.source.clone(), artifact)?;
    require_refusal(
        AdmittedSource::admit(&input, "0.11.0"),
        "source/controller roots must be physically separate",
    )?;
    fixture.guard.finish()
}

#[test]
fn real_package_install_custody_rejects_ignored_foreign_and_changed_bytes() -> Result<(), String> {
    use crate::reports::release::candidate_harness::{
        AdmittedSource, AttributedArchive, CandidateExecution, QualificationInput,
    };
    let fixture = create_source_fixture()?;
    // A valid ancestor Cargo workspace must not capture the attributed install input.
    // The selected source remains its own standalone workspace; archive bytes stay exact.
    fs::write(
        fixture.guard.root.join("Cargo.toml"),
        "[package]\nname='controller-workspace-fixture'\nversion='0.1.0'\nedition='2024'\n[workspace]\n",
    )
    .map_err(|error| error.to_string())?;
    fs::create_dir(fixture.guard.root.join("src")).map_err(|error| error.to_string())?;
    fs::write(
        fixture.guard.root.join("src/lib.rs"),
        "pub fn controller_fixture() {}\n",
    )
    .map_err(|error| error.to_string())?;
    let input = QualificationInput::new(
        fixture.controller.clone(),
        fixture.source.clone(),
        PathBuf::from(PINNED_JSON),
    )?;
    let owned = fixture.guard.root.join("actual package producer");
    fs::create_dir(&owned).map_err(|error| error.to_string())?;
    let archive = AttributedArchive::produce(AdmittedSource::admit(&input, "0.11.0")?, &owned)?;
    let archive_path = archive.archive_path().to_path_buf();
    let original = fs::read(&archive_path).map_err(|error| error.to_string())?;
    let mut changed = original.clone();
    changed.push(b' ');
    fs::write(&archive_path, changed).map_err(|error| error.to_string())?;
    require_refusal(
        archive.revalidate(),
        "produced archive bytes changed after attribution",
    )?;
    fs::write(&archive_path, original).map_err(|error| error.to_string())?;
    archive.revalidate()?;
    let installed = archive.install(&owned)?;
    if installed.fixture_bytes("Cargo.toml")? != b"selected fixture A\n" {
        return Err(
            "authentic fixture bytes did not come from admitted committed source".to_string(),
        );
    }
    let foreign = fixture.guard.root.join("foreign launch B");
    fs::create_dir(&foreign).map_err(|error| error.to_string())?;
    fs::write(
        foreign.join("Cargo.toml"),
        "[package]\nname='ripr'\nversion='0.11.0'\nedition='2024'\n",
    )
    .map_err(|error| error.to_string())?;
    fs::create_dir(foreign.join("src")).map_err(|error| error.to_string())?;
    fs::write(
        foreign.join("src/main.rs"),
        "fn main() { println!(\"decoy source B\"); }\n",
    )
    .map_err(|error| error.to_string())?;
    let output = CandidateExecution::Qualified(&installed).run(
        &[],
        &foreign,
        "actual installed source A token",
    )?;
    if !output.success || output.stdout.trim() != "fixture source A" {
        return Err(format!(
            "selected package A/foreign launch B custody lost: {}",
            output.stdout
        ));
    }
    let executable = fs::read(installed.binary()).map_err(|error| error.to_string())?;
    let mut changed = executable.clone();
    changed.push(b' ');
    fs::write(installed.binary(), changed).map_err(|error| error.to_string())?;
    require_refusal(
        installed.revalidate(),
        "installed executable bytes changed after custody capture",
    )?;
    fs::write(installed.binary(), executable).map_err(|error| error.to_string())?;
    installed.revalidate()?;
    fs::write(
        fixture.source.join("src/foreign.rs"),
        "pub fn foreign() {}\n",
    )
    .map_err(|error| error.to_string())?;
    if !git(
        &fixture.source,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?
    .is_empty()
    {
        return Err("ignored-included-file fixture is not actually Git-clean".to_string());
    }
    let rejected_root = fixture.guard.root.join("ignored included package");
    fs::create_dir(&rejected_root).map_err(|error| error.to_string())?;
    let refusal =
        AttributedArchive::produce(AdmittedSource::admit(&input, "0.11.0")?, &rejected_root);
    match refusal {
        Err(error)
            if error.contains(
                "packaged ordinary entry is not the committed source blob: src/foreign.rs",
            ) => {}
        Err(error)
            if error.contains("qualified cargo package failed with native status")
                && error.contains("src/foreign.rs")
                && (error.contains("not yet committed") || error.contains("uncommitted")) => {}
        Err(error) => {
            return Err(format!(
                "ignored-file control had unrelated failure: {error}"
            ));
        }
        Ok(_) => {
            return Err(
                "Git-clean ignored included foreign file was attributed to source A".to_string(),
            );
        }
    }
    fixture.guard.finish()
}
