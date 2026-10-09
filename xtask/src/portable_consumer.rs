//! Stage and prove the portable native ripr consumer packet (#4713).
//!
//! The Python file is the packet-local transport. This module is the
//! repository producer and the discriminating harness: it writes a packet
//! from an explicit payload, then invokes `run.py` with a bounded process.

use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::time::Duration;

const RUN_PY: &str = "tools/python/portable-ripr-consumer/run.py";
const PACKET_SCHEMA: &str = "ripr.portable_consumer.packet/v1";
#[cfg(unix)]
const CONSUMER_TIMEOUT: Duration = Duration::from_secs(30);

fn workspace_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask CARGO_MANIFEST_DIR has no parent workspace".to_string())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buf)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn packet_digest(binary_relative: &str, binary_sha: &str, script_sha: &str) -> String {
    let mut lines = [
        format!("{binary_relative}={binary_sha}"),
        format!("run.py={script_sha}"),
    ];
    lines.sort();
    format!("{:x}", Sha256::digest(lines.join("\n").as_bytes()))
}

#[cfg(unix)]
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

struct TempDirGuard {
    path: PathBuf,
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        if let Ok(()) = fs::remove_dir_all(&self.path) {}
    }
}

fn temp_dir(label: &str) -> Result<TempDirGuard, String> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "ripr-portable-consumer-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).map_err(|error| format!("temp dir: {error}"))?;
    Ok(TempDirGuard { path: dir })
}

fn make_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)
            .map_err(|error| format!("metadata {}: {error}", path.display()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)
            .map_err(|error| format!("chmod {}: {error}", path.display()))?;
    }
    let _ = path;
    Ok(())
}

fn stage_packet(
    packet_dir: &Path,
    payload: &Path,
    binary_relative: &str,
    platform: &str,
) -> Result<(), String> {
    fs::create_dir_all(packet_dir).map_err(|error| format!("create packet: {error}"))?;
    let root = workspace_root()?;
    let script_src = root.join(RUN_PY);
    let script_dst = packet_dir.join("run.py");
    fs::copy(&script_src, &script_dst).map_err(|error| format!("copy run.py: {error}"))?;
    let binary_dst = packet_dir.join(binary_relative);
    if let Some(parent) = binary_dst.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("create payload parent: {error}"))?;
    }
    fs::copy(payload, &binary_dst).map_err(|error| format!("copy payload: {error}"))?;
    make_executable(&binary_dst)?;
    let binary_sha256 = sha256_file(&binary_dst)?;
    let script_sha256 = sha256_file(&script_dst)?;
    let packet_digest = packet_digest(binary_relative, &binary_sha256, &script_sha256);
    let manifest = serde_json::json!({
        "schema": PACKET_SCHEMA,
        "packet_version": "1",
        "packet_digest": packet_digest,
        "native_payload": {
            "relative_path": binary_relative,
            "sha256": binary_sha256,
            "version": "test-stub",
            "build_identity": "explicit-test-payload",
            "source_route": "explicit-binary",
            "platform": platform,
        },
        "consumer": {
            "script": "run.py",
            "sha256": script_sha256,
            "python_requires": ">=3.11",
        },
        "operations": ["check", "pilot"],
        "timeout_seconds_default": 5,
    });
    fs::write(
        packet_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).map_err(|error| format!("manifest json: {error}"))?,
    )
    .map_err(|error| format!("write manifest: {error}"))?;
    Ok(())
}

fn write_stub(path: &Path, body: &str) -> Result<(), String> {
    fs::write(path, body).map_err(|error| format!("write stub {}: {error}", path.display()))?;
    make_executable(path)
}

fn host_platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

const COMPLETE_STUB: &str = r#"#!/bin/sh
printf 'ARGV=%s\n' "$*" >&2
printf 'CWD=%s\n' "$PWD" >&2
printf 'PACKET_RAN\n' >&2
printf '%s\n' '{"schema_version":"0.2","tool":"stub","findings":[{"id":"gap-1"}],"analysis_outcome":{"analysis_complete":true,"outcome":{"kind":"complete_with_findings"}}}'
exit 0
"#;

#[cfg(unix)]
const ZERO_STUB: &str = r#"#!/bin/sh
printf '%s\n' '{"schema_version":"0.2","tool":"stub","findings":[],"analysis_outcome":{"analysis_complete":true,"outcome":{"kind":"complete_with_findings"}}}'
exit 0
"#;

#[cfg(unix)]
const MALFORMED_STUB: &str = r#"#!/bin/sh
printf 'not-json\n'
exit 0
"#;

#[cfg(unix)]
const EMPTY_STUB: &str = r#"#!/bin/sh
exit 0
"#;

#[cfg(unix)]
const FAIL_STUB: &str = r#"#!/bin/sh
printf 'boom\n' >&2
exit 7
"#;

#[cfg(unix)]
const SLEEP_STUB: &str = r#"#!/bin/sh
sleep 30
exit 0
"#;

#[cfg(unix)]
const DECOY_STUB: &str = r#"#!/bin/sh
printf 'DECOY_RAN\n' >&2
printf '%s\n' '{"schema_version":"0.2","findings":[],"decoy":true}'
exit 0
"#;

#[cfg(unix)]
const LIMITATION_STUB: &str = r#"#!/bin/sh
printf '%s\n' '{"schema_version":"0.2","tool":"stub","findings":[{"id":"limited-1"}],"analysis_outcome":{"analysis_complete":true,"outcome":{"kind":"partial_with_limitations"}}}'
exit 0
"#;

#[cfg(unix)]
const PARTIAL_STUB: &str = r#"#!/bin/sh
printf '%s\n' '{"schema_version":"0.2","tool":"stub","findings":[{"id":"partial-1"}],"analysis_outcome":{"analysis_complete":false,"outcome":{"kind":"incomplete"}}}'
exit 0
"#;

#[cfg(unix)]
const MUTATE_STUB: &str = r#"#!/bin/sh
root=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --root)
      root="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done
if [ -n "$root" ]; then
  printf 'mutated\n' > "$root/mutated.txt"
fi
printf '%s\n' '{"schema_version":"0.2","tool":"stub","findings":[{"id":"gap-1"}],"analysis_outcome":{"analysis_complete":true,"outcome":{"kind":"complete_with_findings"}}}'
exit 0
"#;

#[cfg(unix)]
const PILOT_STUB: &str = r#"#!/bin/sh
out=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --out)
      out="$2"
      shift 2
      ;;
    *)
      printf 'ARGV=%s\n' "$1" >&2
      shift
      ;;
  esac
done
printf 'human pilot terminal\n'
if [ -n "$out" ]; then
  mkdir -p "$out"
  printf '%s\n' '{"schema_version":"0.2","tool":"ripr","status":"complete","findings":[{"id":"seam-1"}]}' > "$out/pilot-summary.json"
fi
printf 'PILOT_RAN\n' >&2
exit 0
"#;

#[cfg(unix)]
const PILOT_TERMINAL_ONLY_STUB: &str = r#"#!/bin/sh
printf 'human pilot terminal without artifact\n'
exit 0
"#;

struct Staged {
    _guard: TempDirGuard,
    packet: PathBuf,
    #[cfg(unix)]
    subject: PathBuf,
    #[cfg(unix)]
    out: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use crate::run::capture_output_with_timeout;
    use serde_json::Value;
    #[cfg(unix)]
    use serde_json::json;

    #[cfg(unix)]
    struct Consumption {
        status: i32,
        stderr: String,
        receipt: Option<Value>,
    }

    #[cfg(unix)]
    fn invoke(
        packet: &Path,
        subject: &Path,
        out: &Path,
        extra: &[&str],
        path_prefix: Option<&Path>,
    ) -> Result<Consumption, String> {
        let python = python3()?;
        let script = packet.join("run.py");
        let mut args = vec![
            script.to_string_lossy().into_owned(),
            "--packet".to_string(),
            packet.to_string_lossy().into_owned(),
            "--subject-root".to_string(),
            subject.to_string_lossy().into_owned(),
            "--out".to_string(),
            out.to_string_lossy().into_owned(),
            "--operation".to_string(),
            "check".to_string(),
        ];
        args.extend(extra.iter().map(|value| (*value).to_string()));
        let path_value =
            path_prefix.map(|prefix| format!("{}:/usr/bin:/bin", prefix.to_string_lossy()));
        let env_refs: Vec<(&str, &str)> = match path_value.as_deref() {
            Some(path) => vec![("PATH", path)],
            None => Vec::new(),
        };
        let output = capture_output_with_timeout(
            python.to_str().ok_or("python3 path is not utf-8")?,
            &args,
            &env_refs,
            CONSUMER_TIMEOUT,
            "portable consumer run.py",
        )?;
        if output.timed_out {
            return Err(format!(
                "python3 timed out after {CONSUMER_TIMEOUT:?}: stderr={}",
                output.stderr
            ));
        }
        let status = output
            .status
            .and_then(|status| status.code())
            .ok_or_else(|| "python3 returned no exit status".to_string())?;
        let receipt_path = out.join("packet-consumption-receipt.json");
        let receipt = if receipt_path.is_file() {
            Some(
                serde_json::from_str(
                    &fs::read_to_string(&receipt_path)
                        .map_err(|error| format!("read receipt: {error}"))?,
                )
                .map_err(|error| format!("parse receipt: {error}"))?,
            )
        } else {
            None
        };
        Ok(Consumption {
            status,
            stderr: output.stderr,
            receipt,
        })
    }

    #[cfg(unix)]
    fn consume(
        packet: &Path,
        subject: &Path,
        out: &Path,
        extra: &[&str],
        path_prefix: Option<&Path>,
    ) -> Result<(i32, Value), String> {
        let consumption = invoke(packet, subject, out, extra, path_prefix)?;
        let receipt = consumption.receipt.ok_or_else(|| {
            format!(
                "missing receipt (status={}, stderr={})",
                consumption.status, consumption.stderr
            )
        })?;
        Ok((consumption.status, receipt))
    }

    #[cfg(unix)]
    fn expect_class(
        status: i32,
        receipt: &Value,
        class: &str,
        success: bool,
    ) -> Result<(), String> {
        if success {
            if status != 0 {
                return Err(format!(
                    "expected success for {class}, got {status}: {receipt}"
                ));
            }
        } else if status == 0 {
            return Err(format!("{class} was accepted: {receipt}"));
        }
        if receipt["classification"] != class {
            return Err(format!("expected {class}, got {receipt}"));
        }
        Ok(())
    }

    fn staged_stub(label: &str, stub: &str) -> Result<Staged, String> {
        let root = temp_dir(label)?;
        let packet = root.path.join("packet");
        let subject = root.path.join("subject");
        #[cfg(unix)]
        let out = root.path.join("out");
        fs::create_dir_all(&subject).map_err(|error| format!("subject: {error}"))?;
        fs::write(subject.join("lib.py"), "value = 1\n")
            .map_err(|error| format!("subject file: {error}"))?;
        let stub_path = root.path.join("payload.sh");
        write_stub(&stub_path, stub)?;
        stage_packet(&packet, &stub_path, "ripr", &host_platform())?;
        Ok(Staged {
            _guard: root,
            packet,
            #[cfg(unix)]
            subject,
            #[cfg(unix)]
            out,
        })
    }

    fn load_manifest(packet: &Path) -> Result<Value, String> {
        serde_json::from_str(
            &fs::read_to_string(packet.join("manifest.json"))
                .map_err(|error| format!("manifest: {error}"))?,
        )
        .map_err(|error| format!("parse manifest: {error}"))
    }

    #[cfg(unix)]
    fn write_manifest(packet: &Path, manifest: &Value) -> Result<(), String> {
        fs::write(
            packet.join("manifest.json"),
            serde_json::to_vec_pretty(manifest)
                .map_err(|error| format!("manifest json: {error}"))?,
        )
        .map_err(|error| format!("write manifest: {error}"))
    }

    #[cfg(unix)]
    fn require_no_launch(receipt: &Value) -> Result<(), String> {
        if receipt["exit_code"] != Value::Null {
            return Err(format!(
                "payload launched before fail-closed class: {receipt}"
            ));
        }
        Ok(())
    }

    #[cfg(unix)]
    fn tree_digest_of(subject: &Path) -> Result<String, String> {
        let python = python3()?;
        let script = workspace_root()?.join(RUN_PY);
        let probe = r#"
import importlib.util, pathlib, sys
spec = importlib.util.spec_from_file_location("portable_ripr_consumer", sys.argv[1])
mod = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = mod
spec.loader.exec_module(mod)
print(mod.tree_digest(pathlib.Path(sys.argv[2])))
"#;
        let output = capture_output_with_timeout(
            python
                .to_str()
                .ok_or_else(|| "python3 path is not utf-8".to_string())?,
            &[
                "-c".to_string(),
                probe.to_string(),
                script.to_string_lossy().into_owned(),
                subject.to_string_lossy().into_owned(),
            ],
            &[("PYTHONDONTWRITEBYTECODE", "1")],
            Duration::from_secs(20),
            "tree_digest",
        )?;
        if !output.status.is_some_and(|status| status.success()) {
            return Err(format!(
                "tree_digest failed: stdout={} stderr={}",
                output.stdout, output.stderr
            ));
        }
        Ok(output.stdout.trim().to_string())
    }

    #[test]
    fn consumer_source_does_not_search_path_or_open_a_network_client() -> Result<(), String> {
        let source = fs::read_to_string(workspace_root()?.join(RUN_PY))
            .map_err(|error| format!("read run.py: {error}"))?;
        for forbidden in [
            "shutil.which",
            "import urllib",
            "from urllib",
            "import socket",
            "http.client",
            "Popen([\"cargo\"",
            "Popen(['cargo'",
            "Popen([\"rustc\"",
            "Popen(['rustc'",
        ] {
            if source.contains(forbidden) {
                return Err(format!("run.py contains forbidden token {forbidden}"));
            }
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn run_py_compiles_with_stdlib_python() -> Result<(), String> {
        let python = python3()?;
        let script = workspace_root()?.join(RUN_PY);
        let output = capture_output_with_timeout(
            python.to_str().ok_or("python3 path is not utf-8")?,
            &[
                "-m".to_string(),
                "py_compile".to_string(),
                script.to_string_lossy().into_owned(),
            ],
            &[("PYTHONDONTWRITEBYTECODE", "1")],
            Duration::from_secs(20),
            "py_compile run.py",
        )?;
        if !output.status.is_some_and(|status| status.success()) {
            return Err(format!(
                "py_compile failed: stdout={} stderr={}",
                output.stdout, output.stderr
            ));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn product_json_classes_are_discriminated_without_launch() -> Result<(), String> {
        let python = python3()?;
        let script = workspace_root()?.join(RUN_PY);
        let cases_dir = temp_dir("classify-matrix")?;
        let cases_path = cases_dir.path.join("cases.json");
        let cases = json!([
            {"id": "empty", "stdout": "", "require_nonzero": true, "class": "partial_product_output"},
            {"id": "malformed", "stdout": "not-json\n", "require_nonzero": true, "class": "malformed_product_output"},
            {"id": "missing-schema", "stdout": "{\"findings\":[{\"id\":\"x\"}]}", "require_nonzero": true, "class": "malformed_product_output"},
            {"id": "non-object", "stdout": "[1]", "require_nonzero": true, "class": "malformed_product_output"},
            {
                "id": "complete",
                "stdout": "{\"schema_version\":\"0.2\",\"findings\":[{\"id\":\"gap-1\"}],\"analysis_outcome\":{\"analysis_complete\":true,\"outcome\":{\"kind\":\"complete_with_findings\"}}}",
                "require_nonzero": true,
                "class": "complete"
            },
            {
                "id": "zero-required",
                "stdout": "{\"schema_version\":\"0.2\",\"findings\":[],\"analysis_outcome\":{\"analysis_complete\":true,\"outcome\":{\"kind\":\"complete_with_findings\"}}}",
                "require_nonzero": true,
                "class": "zero_required_subjects"
            },
            {
                "id": "zero-allowed",
                "stdout": "{\"schema_version\":\"0.2\",\"findings\":[],\"analysis_outcome\":{\"analysis_complete\":true,\"outcome\":{\"kind\":\"complete_with_findings\"}}}",
                "require_nonzero": false,
                "class": "complete"
            },
            {
                "id": "typed-limitation",
                "stdout": "{\"schema_version\":\"0.2\",\"findings\":[{\"id\":\"limited-1\"}],\"analysis_outcome\":{\"analysis_complete\":true,\"outcome\":{\"kind\":\"partial_with_limitations\"}}}",
                "require_nonzero": true,
                "class": "typed_product_limitation"
            },
            {
                "id": "partial-kind-without-complete-flag",
                "stdout": "{\"schema_version\":\"0.2\",\"findings\":[{\"id\":\"limited-1\"}],\"analysis_outcome\":{\"outcome\":{\"kind\":\"partial_with_limitations\"}}}",
                "require_nonzero": true,
                "class": "typed_product_limitation"
            },
            {
                "id": "unsupported-input",
                "stdout": "{\"schema_version\":\"0.2\",\"findings\":[{\"id\":\"x\"}],\"analysis_outcome\":{\"analysis_complete\":true,\"outcome\":{\"kind\":\"unsupported_input\"}}}",
                "require_nonzero": true,
                "class": "partial_product_output"
            },
            {
                "id": "analysis-failed",
                "stdout": "{\"schema_version\":\"0.2\",\"findings\":[{\"id\":\"x\"}],\"analysis_outcome\":{\"analysis_complete\":true,\"outcome\":{\"kind\":\"analysis_failed\"}}}",
                "require_nonzero": true,
                "class": "partial_product_output"
            },
            {
                "id": "invented-limitation-kind",
                "stdout": "{\"schema_version\":\"0.2\",\"findings\":[{\"id\":\"limited-1\"}],\"analysis_outcome\":{\"analysis_complete\":true,\"outcome\":{\"kind\":\"complete_with_limitations\"}}}",
                "require_nonzero": true,
                "class": "malformed_product_output"
            },
            {
                "id": "partial-outcome",
                "stdout": "{\"schema_version\":\"0.2\",\"findings\":[{\"id\":\"partial-1\"}],\"analysis_outcome\":{\"analysis_complete\":false,\"outcome\":{\"kind\":\"incomplete\"}}}",
                "require_nonzero": true,
                "class": "partial_product_output"
            },
            {
                "id": "selected-count-field",
                "stdout": "{\"schema_version\":\"0.2\",\"selected_subject_count\":3}",
                "require_nonzero": true,
                "class": "complete"
            },
            {
                "id": "wrong-version",
                "stdout": "{\"schema_version\":\"0.1\",\"findings\":[{\"id\":\"x\"}]}",
                "require_nonzero": true,
                "class": "malformed_product_output"
            },
            {
                "id": "version-only",
                "stdout": "{\"schema_version\":\"0.2\"}",
                "require_nonzero": false,
                "class": "malformed_product_output"
            }
        ]);
        let expected_len = cases
            .as_array()
            .ok_or_else(|| "classification matrix cases must be an array".to_string())?
            .len();
        fs::write(
            &cases_path,
            serde_json::to_vec(&cases).map_err(|error| format!("cases json: {error}"))?,
        )
        .map_err(|error| format!("write cases: {error}"))?;
        let probe = r#"
import importlib.util, json, pathlib, sys
spec = importlib.util.spec_from_file_location("portable_ripr_consumer", sys.argv[1])
mod = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = mod
spec.loader.exec_module(mod)
cases = json.loads(pathlib.Path(sys.argv[2]).read_text(encoding="utf-8"))
results = []
for case in cases:
    try:
        classification, _, selected, _ = mod.classify_product_json(
            case["stdout"].encode("utf-8"),
            case["require_nonzero"],
        )
    except mod.ConsumerError as exc:
        classification, selected = exc.classification, None
    results.append({
        "id": case["id"],
        "classification": classification,
        "selected": selected,
        "expected": case["class"],
    })
print(json.dumps(results))
"#;
        let output = capture_output_with_timeout(
            python.to_str().ok_or("python3 path is not utf-8")?,
            &[
                "-c".to_string(),
                probe.to_string(),
                script.to_string_lossy().into_owned(),
                cases_path.to_string_lossy().into_owned(),
            ],
            &[("PYTHONDONTWRITEBYTECODE", "1")],
            Duration::from_secs(20),
            "classify_product_json matrix",
        )?;
        if !output.status.is_some_and(|status| status.success()) {
            return Err(format!(
                "classification matrix failed: stdout={} stderr={}",
                output.stdout, output.stderr
            ));
        }
        let results: Vec<Value> = serde_json::from_str(output.stdout.trim())
            .map_err(|error| format!("parse matrix: {error}: {}", output.stdout))?;
        if results.len() != expected_len {
            return Err(format!(
                "expected {expected_len} classification cases, got {results:?}"
            ));
        }
        for result in results {
            if result["classification"] != result["expected"] {
                return Err(format!("classification matrix mismatch: {result}"));
            }
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn complete_stub_packet_runs_the_packet_payload_not_a_path_decoy() -> Result<(), String> {
        let staged = staged_stub("complete", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let decoy_dir = packet
            .parent()
            .ok_or_else(|| "packet directory has no parent".to_string())?
            .join("decoy");
        fs::create_dir_all(&decoy_dir).map_err(|error| format!("decoy dir: {error}"))?;
        write_stub(&decoy_dir.join("ripr"), DECOY_STUB)?;
        let (status, receipt) = consume(packet, subject, out, &[], Some(&decoy_dir))?;
        expect_class(status, &receipt, "complete", true)?;
        if receipt["selected_subject_count"] != 1 {
            return Err(format!("expected one subject, got {receipt}"));
        }
        let stderr = fs::read_to_string(out.join("stderr.bin"))
            .map_err(|error| format!("stderr.bin: {error}"))?;
        if !stderr.contains("PACKET_RAN") {
            return Err(format!("packet stub did not run: {stderr}"));
        }
        if stderr.contains("DECOY_RAN") {
            return Err("PATH decoy was selected as the payload".to_string());
        }
        if receipt["binary"]["relative_path"] != "ripr" {
            return Err(format!("receipt lost payload identity: {receipt}"));
        }
        if receipt["packet"]["packet_digest"]
            .as_str()
            .unwrap_or("")
            .is_empty()
        {
            return Err("receipt omitted packet digest".to_string());
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn foreign_cwd_still_passes_the_explicit_subject_root() -> Result<(), String> {
        let staged = staged_stub("foreign-cwd", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let foreign = packet
            .parent()
            .ok_or_else(|| "packet directory has no parent".to_string())?
            .join("foreign");
        fs::create_dir_all(&foreign).map_err(|error| format!("foreign: {error}"))?;
        let foreign_str = foreign.to_string_lossy().into_owned();
        let (status, receipt) =
            consume(packet, subject, out, &["--foreign-cwd", &foreign_str], None)?;
        expect_class(status, &receipt, "complete", true)?;
        let stderr = fs::read_to_string(out.join("stderr.bin"))
            .map_err(|error| format!("stderr.bin: {error}"))?;
        if !stderr.contains(&format!("CWD={foreign_str}")) {
            return Err(format!("expected foreign cwd, got {stderr}"));
        }
        if !stderr.contains(&format!("--root {}", subject.display())) {
            return Err(format!("explicit root missing from argv: {stderr}"));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn digest_mismatch_fails_closed_before_launch() -> Result<(), String> {
        let staged = staged_stub("digest", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let expected = load_manifest(packet)?["native_payload"]["sha256"]
            .as_str()
            .ok_or("missing expected payload sha")?
            .to_string();
        fs::write(packet.join("ripr"), b"tampered\n")
            .map_err(|error| format!("tamper payload: {error}"))?;
        make_executable(&packet.join("ripr"))?;
        let observed = format!("{:x}", Sha256::digest(b"tampered\n"));
        let (status, receipt) = consume(packet, subject, out, &[], None)?;
        expect_class(status, &receipt, "digest_mismatch", false)?;
        require_no_launch(&receipt)?;
        if receipt["binary"]["sha256"] != observed {
            return Err(format!(
                "receipt omitted the observed payload digest: {receipt}"
            ));
        }
        if receipt["binary"]["sha256"] == expected {
            return Err(
                "receipt recorded the expected digest instead of the observed payload hash"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn consumer_script_digest_mismatch_fails_closed_before_launch() -> Result<(), String> {
        let staged = staged_stub("script-digest", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let mut script =
            fs::read(packet.join("run.py")).map_err(|error| format!("read run.py: {error}"))?;
        script.extend_from_slice(b"\n# tampered\n");
        fs::write(packet.join("run.py"), script)
            .map_err(|error| format!("tamper run.py: {error}"))?;
        let (status, receipt) = consume(packet, subject, out, &[], None)?;
        expect_class(status, &receipt, "digest_mismatch", false)?;
        require_no_launch(&receipt)
    }

    #[cfg(unix)]
    #[test]
    fn packet_digest_field_mismatch_fails_closed_before_launch() -> Result<(), String> {
        let staged = staged_stub("packet-digest", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let mut manifest = load_manifest(packet)?;
        manifest["packet_digest"] = json!("deadbeef");
        write_manifest(packet, &manifest)?;
        let (status, receipt) = consume(packet, subject, out, &[], None)?;
        expect_class(status, &receipt, "digest_mismatch", false)?;
        require_no_launch(&receipt)?;
        if receipt["packet"]["packet_digest"] == "deadbeef" {
            return Err(
                "receipt recorded the expected packet digest instead of the observed listing hash"
                    .to_string(),
            );
        }
        if receipt["packet"]["packet_digest"]
            .as_str()
            .unwrap_or("")
            .is_empty()
        {
            return Err(format!(
                "receipt omitted the observed packet digest: {receipt}"
            ));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn missing_executable_fails_closed() -> Result<(), String> {
        let staged = staged_stub("missing", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        fs::remove_file(packet.join("ripr")).map_err(|error| format!("remove payload: {error}"))?;
        let (status, receipt) = consume(packet, subject, out, &[], None)?;
        expect_class(status, &receipt, "missing_executable", false)?;
        require_no_launch(&receipt)
    }

    #[cfg(unix)]
    #[test]
    fn missing_manifest_is_incompatible_payload() -> Result<(), String> {
        let staged = staged_stub("missing-manifest", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        fs::remove_file(packet.join("manifest.json"))
            .map_err(|error| format!("remove manifest: {error}"))?;
        let (status, receipt) = consume(packet, subject, out, &[], None)?;
        expect_class(status, &receipt, "incompatible_payload", false)?;
        require_no_launch(&receipt)
    }

    #[cfg(unix)]
    #[test]
    fn platform_mismatch_fails_closed_before_launch() -> Result<(), String> {
        let staged = staged_stub("platform", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let mut manifest = load_manifest(packet)?;
        manifest["native_payload"]["platform"] = json!("windows-aarch64");
        write_manifest(packet, &manifest)?;
        let (status, receipt) = consume(packet, subject, out, &[], None)?;
        expect_class(status, &receipt, "incompatible_payload", false)?;
        require_no_launch(&receipt)
    }

    #[cfg(unix)]
    #[test]
    fn relative_path_escape_fails_closed_before_launch() -> Result<(), String> {
        let staged = staged_stub("escape", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let outside = packet
            .parent()
            .ok_or_else(|| "packet directory has no parent".to_string())?
            .join("outside");
        fs::create_dir_all(&outside).map_err(|error| format!("outside: {error}"))?;
        fs::rename(packet.join("ripr"), outside.join("ripr"))
            .map_err(|error| format!("move payload: {error}"))?;
        let mut manifest = load_manifest(packet)?;
        manifest["native_payload"]["relative_path"] = json!("../outside/ripr");
        write_manifest(packet, &manifest)?;
        let (status, receipt) = consume(packet, subject, out, &[], None)?;
        expect_class(status, &receipt, "incompatible_payload", false)?;
        require_no_launch(&receipt)
    }

    #[cfg(unix)]
    #[test]
    fn malformed_product_json_fails_closed() -> Result<(), String> {
        let staged = staged_stub("malformed", MALFORMED_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let (status, receipt) = consume(packet, subject, out, &[], None)?;
        expect_class(status, &receipt, "malformed_product_output", false)
    }

    #[cfg(unix)]
    #[test]
    fn empty_stdout_is_partial_product_output() -> Result<(), String> {
        let staged = staged_stub("empty", EMPTY_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let (status, receipt) = consume(packet, subject, out, &[], None)?;
        expect_class(status, &receipt, "partial_product_output", false)
    }

    #[cfg(unix)]
    #[test]
    fn typed_product_limitation_is_not_complete() -> Result<(), String> {
        let staged = staged_stub("limitation", LIMITATION_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let (status, receipt) = consume(packet, subject, out, &[], None)?;
        expect_class(status, &receipt, "typed_product_limitation", false)?;
        if receipt["selected_subject_count"] != 1 {
            return Err(format!("limitation receipt lost findings count: {receipt}"));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn incomplete_analysis_outcome_is_partial_product_output() -> Result<(), String> {
        let staged = staged_stub("partial", PARTIAL_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let (status, receipt) = consume(packet, subject, out, &[], None)?;
        expect_class(status, &receipt, "partial_product_output", false)
    }

    #[cfg(unix)]
    #[test]
    fn zero_subjects_fail_closed_when_required() -> Result<(), String> {
        let staged = staged_stub("zero", ZERO_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let (status, receipt) =
            consume(packet, subject, out, &["--require-nonzero-subjects"], None)?;
        expect_class(status, &receipt, "zero_required_subjects", false)
    }

    #[cfg(unix)]
    #[test]
    fn zero_subjects_are_complete_when_not_required() -> Result<(), String> {
        let staged = staged_stub("zero-ok", ZERO_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let (status, receipt) = consume(packet, subject, out, &[], None)?;
        expect_class(status, &receipt, "complete", true)?;
        if receipt["selected_subject_count"] != 0 {
            return Err(format!("expected zero selected subjects, got {receipt}"));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn payload_timeout_is_classified_not_as_a_semantic_failure() -> Result<(), String> {
        let staged = staged_stub("timeout", SLEEP_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let (status, receipt) = consume(packet, subject, out, &["--timeout-seconds", "1"], None)?;
        expect_class(status, &receipt, "timeout", false)?;
        if receipt["stdout_digest"] == Value::Null {
            return Err(format!(
                "timeout receipt dropped captured stdout: {receipt}"
            ));
        }
        if !out.join("stdout.bin").is_file() || !out.join("stderr.bin").is_file() {
            return Err("timeout did not persist captured streams".to_string());
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn nonzero_payload_exit_is_execution_failure() -> Result<(), String> {
        let staged = staged_stub("fail", FAIL_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let (status, receipt) = consume(packet, subject, out, &[], None)?;
        expect_class(status, &receipt, "execution_failure", false)?;
        if receipt["exit_code"] != 7 {
            return Err(format!("expected exit 7, got {receipt}"));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unknown_operation_fails_closed() -> Result<(), String> {
        let staged = staged_stub("unknown-op", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let (status, receipt) = consume(packet, subject, out, &["--operation", "repair"], None)?;
        expect_class(status, &receipt, "unknown_operation", false)?;
        require_no_launch(&receipt)
    }

    #[cfg(unix)]
    #[test]
    fn unwritable_output_fails_closed() -> Result<(), String> {
        let staged = staged_stub("unwritable", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let not_a_dir = packet
            .parent()
            .ok_or_else(|| "packet directory has no parent".to_string())?
            .join("out-is-a-file");
        fs::write(&not_a_dir, b"nope\n").map_err(|error| format!("out file: {error}"))?;
        let consumption = invoke(packet, subject, &not_a_dir, &[], None)?;
        if consumption.status == 0 {
            return Err("file-as-out was accepted".to_string());
        }
        if !consumption.stderr.contains("unwritable_output") {
            return Err(format!(
                "expected unwritable_output, got {}",
                consumption.stderr
            ));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn subject_digest_drift_fails_closed() -> Result<(), String> {
        let staged = staged_stub("digest-drift", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let (status, receipt) = consume(
            packet,
            subject,
            out,
            &["--subject-digest", "deadbeef"],
            None,
        )?;
        expect_class(status, &receipt, "subject_identity_drift", false)?;
        require_no_launch(&receipt)
    }

    #[test]
    fn packet_digest_matches_the_producer_formula() -> Result<(), String> {
        let staged = staged_stub("digest-formula", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let manifest = load_manifest(packet)?;
        let binary_sha = manifest["native_payload"]["sha256"]
            .as_str()
            .ok_or("missing binary sha")?;
        let script_sha = manifest["consumer"]["sha256"]
            .as_str()
            .ok_or("missing script sha")?;
        let expected = packet_digest("ripr", binary_sha, script_sha);
        if manifest["packet_digest"] != expected {
            return Err(format!(
                "producer digest drifted: {} vs {expected}",
                manifest["packet_digest"]
            ));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn arm64_machine_names_match_rust_aarch64() -> Result<(), String> {
        let python = python3()?;
        let script = workspace_root()?.join(RUN_PY);
        let probe = r#"
import importlib.util, json, sys
spec = importlib.util.spec_from_file_location("portable_ripr_consumer", sys.argv[1])
mod = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = mod
spec.loader.exec_module(mod)
print(json.dumps({
    "arm64": mod.normalize_machine("arm64"),
    "ARM64": mod.normalize_machine("ARM64"),
    "aarch64": mod.normalize_machine("aarch64"),
    "amd64": mod.normalize_machine("amd64"),
    "x86_64": mod.normalize_machine("x86_64"),
}))
"#;
        let output = capture_output_with_timeout(
            python.to_str().ok_or("python3 path is not utf-8")?,
            &[
                "-c".to_string(),
                probe.to_string(),
                script.to_string_lossy().into_owned(),
            ],
            &[("PYTHONDONTWRITEBYTECODE", "1")],
            Duration::from_secs(20),
            "normalize_machine",
        )?;
        if !output.status.is_some_and(|status| status.success()) {
            return Err(format!(
                "normalize_machine failed: stdout={} stderr={}",
                output.stdout, output.stderr
            ));
        }
        let value: Value = serde_json::from_str(output.stdout.trim())
            .map_err(|error| format!("parse machines: {error}"))?;
        if value["arm64"] != "aarch64"
            || value["ARM64"] != "aarch64"
            || value["aarch64"] != "aarch64"
            || value["amd64"] != "x86_64"
            || value["x86_64"] != "x86_64"
        {
            return Err(format!("machine aliases drifted: {value}"));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn pilot_reads_the_summary_artifact_not_terminal_stdout() -> Result<(), String> {
        let staged = staged_stub("pilot", PILOT_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let (status, receipt) = consume(packet, subject, out, &["--operation", "pilot"], None)?;
        expect_class(status, &receipt, "complete", true)?;
        if receipt["selected_subject_count"] != 1 {
            return Err(format!("pilot findings were not projected: {receipt}"));
        }
        if !receipt["argv"]
            .as_array()
            .ok_or("missing argv")?
            .iter()
            .any(|value| value == "--out")
        {
            return Err(format!("pilot argv omitted --out: {receipt}"));
        }
        let stdout = fs::read_to_string(out.join("stdout.bin"))
            .map_err(|error| format!("stdout.bin: {error}"))?;
        if !stdout.contains("human pilot terminal") {
            return Err(format!("expected terminal stdout, got {stdout}"));
        }
        if !out.join("pilot-summary.json").is_file() {
            return Err("pilot-summary.json was not written".to_string());
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn stale_pilot_summary_is_not_classified_as_complete() -> Result<(), String> {
        let staged = staged_stub("pilot-stale", PILOT_TERMINAL_ONLY_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        fs::create_dir_all(out).map_err(|error| format!("out dir: {error}"))?;
        fs::write(
            out.join("pilot-summary.json"),
            br#"{"schema_version":"0.2","tool":"stale","findings":[{"id":"leftover"}]}"#,
        )
        .map_err(|error| format!("stale summary: {error}"))?;
        let (status, receipt) = consume(packet, subject, out, &["--operation", "pilot"], None)?;
        expect_class(status, &receipt, "partial_product_output", false)?;
        if out.join("pilot-summary.json").is_file() {
            return Err("stale pilot-summary.json was classified instead of cleared".to_string());
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn missing_pilot_summary_is_partial_product_output() -> Result<(), String> {
        let staged = staged_stub("pilot-missing", PILOT_TERMINAL_ONLY_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let (status, receipt) = consume(packet, subject, out, &["--operation", "pilot"], None)?;
        expect_class(status, &receipt, "partial_product_output", false)
    }

    #[cfg(unix)]
    #[test]
    fn argv_template_must_start_with_the_allowlisted_operation() -> Result<(), String> {
        let staged = staged_stub("argv-verb", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let mut manifest = load_manifest(packet)?;
        manifest["operation_profiles"] = json!({
            "check": {
                "argv_template": ["doctor", "--root", "{subject_root}"]
            }
        });
        write_manifest(packet, &manifest)?;
        let (status, receipt) = consume(packet, subject, out, &[], None)?;
        expect_class(status, &receipt, "incompatible_payload", false)?;
        require_no_launch(&receipt)
    }

    #[cfg(unix)]
    #[test]
    fn pilot_rejects_diff_before_launch() -> Result<(), String> {
        let staged = staged_stub("pilot-diff", PILOT_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let diff = packet
            .parent()
            .ok_or_else(|| "packet directory has no parent".to_string())?
            .join("change.diff");
        fs::write(&diff, b"--- a\n+++ b\n").map_err(|error| format!("diff: {error}"))?;
        let diff_arg = diff.to_string_lossy().into_owned();
        let (status, receipt) = consume(
            packet,
            subject,
            out,
            &["--operation", "pilot", "--diff", &diff_arg],
            None,
        )?;
        expect_class(status, &receipt, "unknown_operation", false)?;
        require_no_launch(&receipt)
    }

    #[cfg(unix)]
    #[test]
    fn subject_file_cap_is_environment_unavailable_not_drift() -> Result<(), String> {
        let staged = staged_stub("subject-cap", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        for index in 0..1001 {
            fs::write(subject.join(format!("f{index}.py")), b"x = 1\n")
                .map_err(|error| format!("cap file: {error}"))?;
        }
        let (status, receipt) = consume(
            packet,
            subject,
            out,
            &["--subject-digest", "deadbeef"],
            None,
        )?;
        expect_class(status, &receipt, "environment_unavailable", false)?;
        require_no_launch(&receipt)
    }

    #[cfg(unix)]
    #[test]
    fn subject_digest_is_rechecked_after_launch() -> Result<(), String> {
        let staged = staged_stub("digest-after", MUTATE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        let before = tree_digest_of(subject)?;
        let (status, receipt) =
            consume(packet, subject, out, &["--subject-digest", &before], None)?;
        expect_class(status, &receipt, "subject_identity_drift", false)?;
        if receipt["exit_code"] == Value::Null {
            return Err(format!(
                "post-launch drift was reported before the payload ran: {receipt}"
            ));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn existing_output_symlink_is_unwritable() -> Result<(), String> {
        let staged = staged_stub("symlink-out", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        fs::create_dir_all(out).map_err(|error| format!("out dir: {error}"))?;
        let escaped = packet
            .parent()
            .ok_or_else(|| "packet directory has no parent".to_string())?
            .join("escaped.txt");
        fs::write(&escaped, b"keep\n").map_err(|error| format!("escaped: {error}"))?;
        std::os::unix::fs::symlink(&escaped, out.join("stdout.bin"))
            .map_err(|error| format!("symlink stdout.bin: {error}"))?;
        let consumption = invoke(packet, subject, out, &[], None)?;
        if consumption.status == 0 {
            return Err("symlink out artifact was accepted".to_string());
        }
        if !consumption.stderr.contains("unwritable_output") {
            return Err(format!(
                "expected unwritable_output, got {}",
                consumption.stderr
            ));
        }
        let retained =
            fs::read_to_string(&escaped).map_err(|error| format!("read escaped: {error}"))?;
        if retained != "keep\n" {
            return Err(format!("symlink write escaped the out dir: {retained}"));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn write_probe_symlink_is_unwritable() -> Result<(), String> {
        let staged = staged_stub("probe-symlink", COMPLETE_STUB)?;
        let packet = &staged.packet;
        let subject = &staged.subject;
        let out = &staged.out;
        fs::create_dir_all(out).map_err(|error| format!("out dir: {error}"))?;
        let escaped = packet
            .parent()
            .ok_or_else(|| "packet directory has no parent".to_string())?
            .join("escaped-probe.txt");
        fs::write(&escaped, b"keep-probe\n").map_err(|error| format!("escaped: {error}"))?;
        std::os::unix::fs::symlink(&escaped, out.join(".portable-consumer-write-probe"))
            .map_err(|error| format!("symlink probe: {error}"))?;
        let consumption = invoke(packet, subject, out, &[], None)?;
        if consumption.status == 0 {
            return Err("write-probe symlink was accepted".to_string());
        }
        if !consumption.stderr.contains("unwritable_output") {
            return Err(format!(
                "expected unwritable_output, got {}",
                consumption.stderr
            ));
        }
        let retained =
            fs::read_to_string(&escaped).map_err(|error| format!("read escaped: {error}"))?;
        if retained != "keep-probe\n" {
            return Err(format!("probe write escaped the out dir: {retained}"));
        }
        Ok(())
    }
}
