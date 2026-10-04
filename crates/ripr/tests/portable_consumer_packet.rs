//! Native packet journey for the portable consumer (#4713).
//!
//! Stages a packet around the worktree-built `ripr` binary and invokes stdlib
//! `run.py` with a PATH decoy and no Cargo on PATH. The consumer must still
//! analyze an explicit subject root from a foreign cwd.
//!
//! The subject is the Rust `fixtures/boundary_gap` fixture so this journey
//! stays valid under `--no-default-features --features lang-rust` (the
//! merge-gate rust-only feature lane). A Python fixture would exit 2 from a
//! rust-only payload.

#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::Output;

fn python3() -> Result<PathBuf, String> {
    for candidate in [
        "/usr/bin/python3",
        "/usr/local/bin/python3",
        "/opt/homebrew/bin/python3",
    ] {
        let path = Path::new(candidate);
        if path.is_file() {
            return Ok(path.to_path_buf());
        }
    }
    Err(
        "python3 is not available at /usr/bin/python3, /usr/local/bin/python3, or /opt/homebrew/bin/python3"
            .to_string(),
    )
}

fn run_python(python: &Path, args: &[String], path: Option<&str>) -> Result<Output, String> {
    let mut command = Command::new(python);
    command.args(args);
    if let Some(path) = path {
        command.env("PATH", path);
    }
    command
        .output()
        .map_err(|error| format!("spawn python3 {args:?}: {error}"))
}

fn sha256_file(python: &Path, path: &Path) -> Result<String, String> {
    let script = "import hashlib,sys;h=hashlib.sha256()\nwith open(sys.argv[1],'rb') as handle:\n    for chunk in iter(lambda: handle.read(1024*1024), b''):\n        h.update(chunk)\nprint(h.hexdigest())";
    let output = run_python(
        python,
        &[
            "-c".to_string(),
            script.to_string(),
            path.to_string_lossy().into_owned(),
        ],
        None,
    )?;
    if !output.status.success() {
        return Err(format!(
            "hash {} failed: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let digest = String::from_utf8(output.stdout)
        .map_err(|error| format!("hash stdout: {error}"))?
        .trim()
        .to_string();
    if digest.len() != 64 {
        return Err(format!("unexpected digest {digest}"));
    }
    Ok(digest)
}

fn packet_digest(
    python: &Path,
    binary_relative: &str,
    binary_sha: &str,
    script_sha: &str,
) -> Result<String, String> {
    let mut lines = [
        format!("{binary_relative}={binary_sha}"),
        format!("run.py={script_sha}"),
    ];
    lines.sort();
    let joined = lines.join("\n");
    let script =
        "import hashlib,sys; print(hashlib.sha256(sys.argv[1].encode('utf-8')).hexdigest())";
    let output = run_python(
        python,
        &["-c".to_string(), script.to_string(), joined],
        None,
    )?;
    if !output.status.success() {
        return Err(format!(
            "packet digest failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn workspace_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|error| format!("workspace root: {error}"))
}

struct TempDirGuard {
    path: PathBuf,
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        if let Ok(()) = fs::remove_dir_all(&self.path) {}
    }
}

fn temp_root(label: &str) -> Result<TempDirGuard, String> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "ripr-native-packet-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).map_err(|error| format!("temp dir: {error}"))?;
    Ok(TempDirGuard { path: dir })
}

fn copy_file(src: &Path, dst: &Path) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("mkdir {}: {error}", parent.display()))?;
    }
    fs::copy(src, dst)
        .map_err(|error| format!("copy {} -> {}: {error}", src.display(), dst.display()))?;
    Ok(())
}

fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)
        .map_err(|error| format!("metadata {}: {error}", path.display()))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).map_err(|error| format!("chmod {}: {error}", path.display()))
}

fn host_platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn stage_boundary_subject(workspace: &Path, subject: &Path, diff: &Path) -> Result<(), String> {
    copy_file(
        &workspace.join("fixtures/boundary_gap/input/Cargo.toml"),
        &subject.join("Cargo.toml"),
    )?;
    copy_file(
        &workspace.join("fixtures/boundary_gap/input/src/lib.rs"),
        &subject.join("src/lib.rs"),
    )?;
    copy_file(
        &workspace.join("fixtures/boundary_gap/input/tests/pricing.rs"),
        &subject.join("tests/pricing.rs"),
    )?;
    copy_file(&workspace.join("fixtures/boundary_gap/diff.patch"), diff)
}

fn stage_native_packet(
    python: &Path,
    workspace: &Path,
    packet: &Path,
    payload: &Path,
) -> Result<(), String> {
    copy_file(
        &workspace.join("tools/python/portable-ripr-consumer/run.py"),
        &packet.join("run.py"),
    )?;
    copy_file(payload, &packet.join("ripr"))?;
    make_executable(&packet.join("ripr"))?;
    let binary_sha = sha256_file(python, &packet.join("ripr"))?;
    let script_sha = sha256_file(python, &packet.join("run.py"))?;
    let digest = packet_digest(python, "ripr", &binary_sha, &script_sha)?;
    let manifest = serde_json::json!({
        "schema": "ripr.portable_consumer.packet/v1",
        "packet_version": "1",
        "packet_digest": digest,
        "native_payload": {
            "relative_path": "ripr",
            "sha256": binary_sha,
            "version": "worktree",
            "build_identity": "CARGO_BIN_EXE_ripr",
            "source_route": "explicit-binary",
            "platform": host_platform(),
        },
        "consumer": {
            "script": "run.py",
            "sha256": script_sha,
            "python_requires": ">=3.11",
        },
        "operations": ["check", "pilot"],
        "timeout_seconds_default": 120,
    });
    fs::write(
        packet.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).map_err(|error| format!("manifest json: {error}"))?,
    )
    .map_err(|error| format!("manifest: {error}"))
}

struct NativeJourney {
    _root: TempDirGuard,
    python: PathBuf,
    packet: PathBuf,
    subject: PathBuf,
    out: PathBuf,
    foreign: PathBuf,
    decoy: PathBuf,
    diff: PathBuf,
}

fn stage_native_journey(label: &str) -> Result<NativeJourney, String> {
    let python = python3()?;
    let workspace = workspace_root()?;
    let root = temp_root(label)?;
    let packet = root.path.join("packet");
    let subject = root.path.join("subject");
    let out = root.path.join("out");
    let foreign = root.path.join("foreign");
    let decoy = root.path.join("decoy");
    let diff = root.path.join("change.diff");
    fs::create_dir_all(&packet).map_err(|error| format!("packet: {error}"))?;
    fs::create_dir_all(&out).map_err(|error| format!("out: {error}"))?;
    fs::create_dir_all(&foreign).map_err(|error| format!("foreign: {error}"))?;
    fs::create_dir_all(&decoy).map_err(|error| format!("decoy: {error}"))?;
    stage_boundary_subject(&workspace, &subject, &diff)?;
    stage_native_packet(
        &python,
        &workspace,
        &packet,
        Path::new(env!("CARGO_BIN_EXE_ripr")),
    )?;
    let decoy_payload = decoy.join("ripr");
    fs::write(
        &decoy_payload,
        "#!/bin/sh\nprintf 'DECOY_RAN\\n' >&2\nexit 99\n",
    )
    .map_err(|error| format!("decoy: {error}"))?;
    make_executable(&decoy_payload)?;
    Ok(NativeJourney {
        _root: root,
        python,
        packet,
        subject,
        out,
        foreign,
        decoy,
        diff,
    })
}

fn consume_native(
    journey: &NativeJourney,
    operation: &str,
    extra: &[&str],
) -> Result<(std::process::Output, serde_json::Value), String> {
    let path = format!("{}:/usr/bin:/bin", journey.decoy.display());
    let mut args = vec![
        journey.packet.join("run.py").to_string_lossy().into_owned(),
        "--packet".to_string(),
        journey.packet.to_string_lossy().into_owned(),
        "--subject-root".to_string(),
        journey.subject.to_string_lossy().into_owned(),
        "--out".to_string(),
        journey.out.to_string_lossy().into_owned(),
        "--operation".to_string(),
        operation.to_string(),
        "--foreign-cwd".to_string(),
        journey.foreign.to_string_lossy().into_owned(),
        "--timeout-seconds".to_string(),
        "120".to_string(),
    ];
    args.extend(extra.iter().map(|value| (*value).to_string()));
    let output = run_python(&journey.python, &args, Some(&path))?;
    let receipt_path = journey.out.join("packet-consumption-receipt.json");
    if !receipt_path.is_file() {
        return Err(format!(
            "missing receipt: status={:?} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let receipt: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&receipt_path).map_err(|error| format!("read receipt: {error}"))?,
    )
    .map_err(|error| format!("parse receipt: {error}"))?;
    Ok((output, receipt))
}

#[test]
fn native_packet_analyzes_a_boundary_gap_without_path_or_compiler_fallback() -> Result<(), String> {
    let journey = stage_native_journey("boundary")?;
    let (output, receipt) = consume_native(
        &journey,
        "check",
        &[
            "--diff",
            journey.diff.to_str().ok_or("diff path is not utf-8")?,
            "--require-nonzero-subjects",
        ],
    )?;
    if !output.status.success() {
        return Err(format!("native packet failed: {receipt}"));
    }
    if receipt["classification"] != "complete" {
        return Err(format!("expected complete, got {receipt}"));
    }
    let selected = receipt["selected_subject_count"]
        .as_u64()
        .ok_or_else(|| format!("missing selected_subject_count: {receipt}"))?;
    if selected == 0 {
        return Err(format!(
            "boundary-gap fixture produced zero findings: {receipt}"
        ));
    }
    let stderr = fs::read_to_string(journey.out.join("stderr.bin"))
        .map_err(|error| format!("stderr.bin: {error}"))?;
    if stderr.contains("DECOY_RAN") {
        return Err("PATH decoy was selected as the payload".to_string());
    }
    let stdout =
        fs::read(journey.out.join("stdout.bin")).map_err(|error| format!("stdout.bin: {error}"))?;
    let product: serde_json::Value =
        serde_json::from_slice(&stdout).map_err(|error| format!("product json: {error}"))?;
    if product["schema_version"] != "0.2" {
        return Err(format!("unexpected product schema: {product}"));
    }
    if product["tool"] != "ripr" {
        return Err(format!("consumer rewrote tool identity: {product}"));
    }
    Ok(())
}

#[test]
fn native_packet_pilot_consumes_the_summary_artifact() -> Result<(), String> {
    let journey = stage_native_journey("pilot")?;
    let (output, receipt) = consume_native(&journey, "pilot", &[])?;
    if !output.status.success() {
        return Err(format!("native pilot failed: {receipt}"));
    }
    if receipt["classification"] != "complete" {
        return Err(format!("expected complete pilot, got {receipt}"));
    }
    if !receipt["argv"]
        .as_array()
        .ok_or("missing argv")?
        .iter()
        .any(|value| value == "--out")
    {
        return Err(format!("pilot argv omitted --out: {receipt}"));
    }
    let summary = fs::read(journey.out.join("pilot-summary.json"))
        .map_err(|error| format!("pilot-summary.json: {error}"))?;
    let product: serde_json::Value =
        serde_json::from_slice(&summary).map_err(|error| format!("pilot json: {error}"))?;
    if product["schema_version"] != "0.2" {
        return Err(format!("unexpected pilot schema: {product}"));
    }
    if product["tool"] != "ripr" {
        return Err(format!("consumer rewrote pilot tool identity: {product}"));
    }
    let stdout = fs::read_to_string(journey.out.join("stdout.bin"))
        .map_err(|error| format!("stdout.bin: {error}"))?;
    if stdout.trim_start().starts_with('{') {
        return Err(format!(
            "pilot classified terminal stdout instead of the summary artifact: {stdout}"
        ));
    }
    Ok(())
}
