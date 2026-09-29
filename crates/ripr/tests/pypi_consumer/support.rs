//! Shared spawn, temp-tree, wheel-pack, and fixture helpers.
//!
//! One spawn site owns every subprocess. Consumer runs use a cleared
//! environment so ambient Cargo/rustc/ripr cannot leak in.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::Output;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::identity::{WHEEL_NORMALIZED_PREFIX, WheelIdentity, sha256_file};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) struct TempRoot {
    pub(crate) path: PathBuf,
}

impl TempRoot {
    pub(crate) fn new(label: &str) -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "ripr-pypi-consumer-{label}-{}-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed),
            stamp
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).map_err(|error| format!("mkdir {}: {error}", path.display()))?;
        Ok(Self { path })
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(crate) struct SpawnRequest<'a> {
    pub(crate) program: &'a Path,
    pub(crate) args: &'a [&'a str],
    pub(crate) cwd: Option<&'a Path>,
    pub(crate) path: Option<&'a str>,
    pub(crate) extra_env: &'a [(&'a str, &'a str)],
    pub(crate) clear_env: bool,
}

pub(crate) fn run(request: SpawnRequest<'_>) -> Result<Output, String> {
    let mut command = Command::new(request.program);
    command.args(request.args);
    if request.clear_env {
        command.env_clear();
        if let Some(path) = request.path {
            command.env("PATH", path);
        }
        if let Ok(home) = std::env::var("HOME") {
            command.env("HOME", home);
        }
        if let Ok(tmp) = std::env::var("TMPDIR") {
            command.env("TMPDIR", tmp);
        }
        command.env("LC_ALL", "C");
    } else if let Some(path) = request.path {
        command.env("PATH", path);
    }
    for (key, value) in request.extra_env {
        command.env(key, value);
    }
    if let Some(cwd) = request.cwd {
        command.current_dir(cwd);
    }
    command.output().map_err(|error| {
        format!(
            "spawn {} {:?} failed: {error}",
            request.program.display(),
            request.args
        )
    })
}

pub(crate) fn require_success(output: &Output, what: &str) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{what} failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

pub(crate) fn run_bash_script(
    script: &Path,
    body: &str,
    cwd: &Path,
    path: &str,
    extra_env: &[(&str, &str)],
) -> Result<Output, String> {
    fs::write(script, body).map_err(|error| format!("write {}: {error}", script.display()))?;
    let script_s = script
        .to_str()
        .ok_or_else(|| format!("{} is not UTF-8", script.display()))?;
    run(SpawnRequest {
        program: Path::new("/bin/bash"),
        args: &["--noprofile", "--norc", "--", script_s],
        cwd: Some(cwd),
        path: Some(path),
        extra_env,
        clear_env: true,
    })
}

pub(crate) fn stdout_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub(crate) fn python3() -> Result<PathBuf, String> {
    let path = PathBuf::from("/usr/bin/python3");
    if path.is_file() {
        return Ok(path);
    }
    Err(
        "python3 is required for the pip/uv consumer journey and was not found at /usr/bin/python3"
            .to_string(),
    )
}

pub(crate) fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub(crate) fn copy_tree(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|error| format!("mkdir {}: {error}", dst.display()))?;
    let entries = fs::read_dir(src).map_err(|error| format!("read {}: {error}", src.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("walk {}: {error}", src.display()))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("stat {}: {error}", entry.path().display()))?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target).map_err(|error| {
                format!(
                    "copy {} -> {}: {error}",
                    entry.path().display(),
                    target.display()
                )
            })?;
        }
    }
    Ok(())
}

pub(crate) fn host_wheel_tag() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("py3-none-linux_x86_64"),
        ("linux", "aarch64") => Ok("py3-none-linux_aarch64"),
        ("macos", "x86_64") => Ok("py3-none-macosx_10_12_x86_64"),
        ("macos", "aarch64") => Ok("py3-none-macosx_11_0_arm64"),
        (os, arch) => Err(format!(
            "this #4626 slice does not claim wheel tags for {os}/{arch}"
        )),
    }
}

pub(crate) fn pack_wheel(
    python: &Path,
    payload: &Path,
    dest_dir: &Path,
    version: &str,
) -> Result<WheelIdentity, String> {
    fs::create_dir_all(dest_dir)
        .map_err(|error| format!("mkdir {}: {error}", dest_dir.display()))?;
    let tag = host_wheel_tag()?;
    let script = dest_dir.join("pack_wheel.py");
    fs::write(&script, PACK_WHEEL_PY).map_err(|error| format!("write packer: {error}"))?;
    let output = run(SpawnRequest {
        program: python,
        args: &[
            script.to_str().ok_or("packer path is not UTF-8")?,
            payload.to_str().ok_or("payload path is not UTF-8")?,
            dest_dir.to_str().ok_or("dest path is not UTF-8")?,
            version,
            tag,
        ],
        cwd: None,
        path: None,
        extra_env: &[],
        clear_env: false,
    })?;
    require_success(&output, "pack local ripr-rs wheel")?;
    let filename = format!("{WHEEL_NORMALIZED_PREFIX}{version}-{tag}.whl");
    let wheel_path = dest_dir.join(&filename);
    let wheel_sha256 = sha256_file(&wheel_path)?;
    let payload_sha256 = sha256_file(payload)?;
    let _ = fs::remove_file(&script);
    Ok(WheelIdentity {
        filename,
        wheel_sha256,
        payload_sha256,
        version: version.to_string(),
        tag: tag.to_string(),
    })
}

pub(crate) fn write_executable(path: &Path, body: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("mkdir {}: {error}", parent.display()))?;
    }
    fs::write(path, body).map_err(|error| format!("write {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("chmod {}: {error}", path.display()))?;
    }
    Ok(())
}

const PACK_WHEEL_PY: &str = r#"
import base64
import hashlib
import sys
import zipfile
from pathlib import Path

payload = Path(sys.argv[1])
dest = Path(sys.argv[2])
version = sys.argv[3]
tag = sys.argv[4]
script_bytes = payload.read_bytes()
assert script_bytes, "payload is empty"

filename = f"ripr_rs-{version}-{tag}.whl"
script_name = f"ripr_rs-{version}.data/scripts/ripr"
metadata_name = f"ripr_rs-{version}.dist-info/METADATA"
wheel_name = f"ripr_rs-{version}.dist-info/WHEEL"
record_name = f"ripr_rs-{version}.dist-info/RECORD"

metadata = (
    "Metadata-Version: 2.3\n"
    "Name: ripr-rs\n"
    f"Version: {version}\n"
    "Summary: local candidate wheel for the #4626 pip/uv consumer journey\n"
    "Requires-Python: >=3.10\n"
).encode()
wheel = (
    "Wheel-Version: 1.0\n"
    "Generator: ripr-pypi-consumer-journey\n"
    "Root-Is-Purelib: false\n"
    f"Tag: {tag}\n"
).encode()

def digest(data: bytes) -> str:
    encoded = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=").decode("ascii")
    return f"sha256={encoded}"

record = (
    f"{script_name},{digest(script_bytes)},{len(script_bytes)}\n"
    f"{metadata_name},{digest(metadata)},{len(metadata)}\n"
    f"{wheel_name},{digest(wheel)},{len(wheel)}\n"
    f"{record_name},,\n"
).encode()

wheel_path = dest / filename
with zipfile.ZipFile(wheel_path, "w") as archive:
    info = zipfile.ZipInfo(script_name)
    info.external_attr = 0o100755 << 16
    archive.writestr(info, script_bytes)
    archive.writestr(metadata_name, metadata)
    archive.writestr(wheel_name, wheel)
    archive.writestr(record_name, record)
print(wheel_path)
"#;
