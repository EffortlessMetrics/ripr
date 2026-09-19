//! Installed first-hour qualification harness — #1674 slice A (harness core).
//!
//! Report-only `cargo xtask first-hour`. Owns the installed-artifact
//! authority every later slice consumes: an explicit `.crate`/installed
//! executable identity, fixture roots, a per-step process/file/command
//! ledger, cleanup authority, and a deterministic receipt skeleton.
//! Product behavior is exercised only through the installed executable;
//! a PATH/worktree binary, existing cache, or helper API is never a
//! substitute.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use serde_json::{Value, json};
use tar::Archive;

use crate::run;

use super::eval_sweep_check::sha256_hex;

const USAGE: &str = "\
cargo xtask first-hour --crate <path.crate> --prefix <clean-dir> --out <receipt-dir>
  --fixture-root <dir>
Installs the packaged candidate into a clean prefix and records the
installed-artifact identity. Later slices consume the receipt; this slice
proves the authority (identity + ledger + cleanup), not any journey.";

const RECEIPT_FILE: &str = "first-hour.json";
const SCHEMA_VERSION: &str = "0.1";

// ---------------------------------------------------------------------------
// Args
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct FirstHourArgs {
    crate_path: String,
    prefix: String,
    out: String,
    fixture_root: String,
}

fn take_value(args: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("first-hour {flag} requires a value\n{USAGE}"))
}

fn parse_args(args: &[String]) -> Result<FirstHourArgs, String> {
    let mut crate_path: Option<String> = None;
    let mut prefix: Option<String> = None;
    let mut out: Option<String> = None;
    let mut fixture_root: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--crate" => crate_path = Some(take_value(args, &mut index, "--crate")?),
            "--prefix" => prefix = Some(take_value(args, &mut index, "--prefix")?),
            "--out" => out = Some(take_value(args, &mut index, "--out")?),
            "--fixture-root" => {
                fixture_root = Some(take_value(args, &mut index, "--fixture-root")?);
            }
            "--help" | "-h" => return Err(USAGE.to_string()),
            other => return Err(format!("first-hour unknown argument `{other}`\n{USAGE}")),
        }
        index += 1;
    }
    Ok(FirstHourArgs {
        crate_path: crate_path.ok_or_else(|| format!("first-hour --crate is required\n{USAGE}"))?,
        prefix: prefix.ok_or_else(|| format!("first-hour --prefix is required\n{USAGE}"))?,
        out: out.ok_or_else(|| format!("first-hour --out is required\n{USAGE}"))?,
        fixture_root: fixture_root
            .ok_or_else(|| format!("first-hour --fixture-root is required\n{USAGE}"))?,
    })
}

// ---------------------------------------------------------------------------
// Installed-artifact identity
// ---------------------------------------------------------------------------

/// Identity of one installed candidate. Every later slice resolves product
/// invocations through exactly this executable; nothing else may stand in.
#[derive(Clone, Debug)]
struct InstalledSubject {
    crate_path: String,
    crate_sha256: String,
    prefix: PathBuf,
    executable: PathBuf,
    executable_sha256: String,
    executable_size: u64,
    version_output: String,
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes =
        std::fs::read(path).map_err(|error| format!("read `{}`: {error}", path.display()))?;
    Ok(sha256_hex(&bytes))
}

/// Base directory for harness-owned staging: the process temp directory
/// unless it sits inside the enclosing Cargo workspace (observed when
/// `TMPDIR` points at a repo-local target dir), in which case the user's
/// home cache. An extracted candidate staged inside the workspace would
/// be discovered as a member and refused by `cargo install`.
fn staging_base() -> Result<PathBuf, String> {
    let temp = std::env::temp_dir();
    let workspace = std::env::current_dir()
        .ok()
        .and_then(|cwd| enclosing_workspace_root(&cwd));
    match workspace {
        Some(root) if temp.starts_with(&root) => std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".cache").join("ripr-first-hour"))
            .ok_or_else(|| {
                "temp dir sits inside the cargo workspace and HOME is unset".to_string()
            }),
        _ => Ok(temp),
    }
}

fn unix_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Ledger and cleanup authority
// ---------------------------------------------------------------------------

/// One retained step record: exact argv, cwd, outcome. The receipt replays
/// the journey from the ledger, never from memory.
#[derive(Clone, Debug)]
struct LedgerEntry {
    step: String,
    argv: Vec<String>,
    cwd: String,
    started_epoch_secs: u64,
    status: String,
}

struct Harness {
    roots: Vec<PathBuf>,
    ledger: Vec<LedgerEntry>,
}

/// Workspace root enclosing `dir`, if any: the nearest ancestor holding a
/// manifest that declares `[workspace]`.
fn enclosing_workspace_root(dir: &Path) -> Option<PathBuf> {
    let mut current = if dir.is_file() {
        dir.parent().map(Path::to_path_buf)
    } else {
        Some(dir.to_path_buf())
    };
    while let Some(candidate) = current {
        let manifest = candidate.join("Cargo.toml");
        if manifest.is_file()
            && let Ok(text) = std::fs::read_to_string(&manifest)
            && text.lines().any(|line| line.trim() == "[workspace]")
        {
            return Some(candidate);
        }
        current = candidate.parent().map(Path::to_path_buf);
    }
    None
}

/// Absolute, lexically normalized form of a caller-supplied path for
/// containment checks. Relative paths resolve against the current
/// directory; `.` pops nothing and `..` pops one normal component, so a
/// `..` that would escape the anchor is refused instead of silently
/// changing which tree a check compares. Symlinked ancestors of
/// not-yet-existing paths cannot be resolved — callers pass paths that
/// must not exist yet, so the check stays lexical and fail-closed.
fn absolute_normalized(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("current dir: {error}"))?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        use std::path::Component::{CurDir, Normal, ParentDir, Prefix, RootDir};
        match component {
            Prefix(_) | RootDir => normalized.push(component.as_os_str()),
            CurDir => {}
            ParentDir => {
                if !normalized.pop() {
                    return Err(format!(
                        "first-hour path `{}` escapes its anchor",
                        path.display()
                    ));
                }
            }
            Normal(_) => normalized.push(component.as_os_str()),
        }
    }
    Ok(normalized)
}

impl Harness {
    /// Creates a harness-owned staging directory registered for Drop-guard
    /// cleanup. Only directories this function creates are ever registered:
    /// caller-supplied paths are never adopted for deletion. Ownership is
    /// claimed with exclusive creation: a fresh salted candidate is created
    /// with `create_dir`, and an `AlreadyExists` collision retries with a
    /// new candidate instead of deleting whatever occupies the path — the
    /// harness never removes a directory it did not create. Staging is
    /// always placed outside the enclosing Cargo workspace: an extracted
    /// package staged inside it would be discovered as a workspace member
    /// and `cargo install` would refuse it.
    fn owned_staging(&mut self, label: &str) -> Result<PathBuf, String> {
        let base = staging_base()?;
        Self::owned_staging_in(self, &base, label)
    }

    /// Staging creation under an explicit base, so tests can prove a
    /// not-yet-existing base (the home-cache fallback on a fresh runner) is
    /// created, not assumed.
    fn owned_staging_in(harness: &mut Self, base: &Path, label: &str) -> Result<PathBuf, String> {
        // The base itself (temp dir or home cache fallback) may not exist
        // yet — the fallback is only constructed, never created. Creating
        // it is safe: create_dir_all never deletes, and exclusivity below
        // still applies to the candidate, never the base.
        std::fs::create_dir_all(base)
            .map_err(|error| format!("create {label} staging base: {error}"))?;
        for attempt in 0..100 {
            let root = base.join(format!(
                "ripr-first-hour-{label}-{}-{:?}-{attempt}-{}",
                std::process::id(),
                std::thread::current().id(),
                unix_epoch_secs()
            ));
            match std::fs::create_dir(&root) {
                Ok(()) => {
                    harness.roots.push(root.clone());
                    return Ok(root);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!("create {label} staging: {error}"));
                }
            }
        }
        Err(format!(
            "create {label} staging: no free candidate after 100 tries"
        ))
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        for root in &self.roots {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

// ---------------------------------------------------------------------------
// Install into a clean prefix
// ---------------------------------------------------------------------------

fn executable_name() -> String {
    format!("ripr{}", std::env::consts::EXE_SUFFIX)
}

/// Extracts a `.crate` archive into an owned staging directory, enforcing
/// the packaged-crate shape (one top-level root, no traversal/absolute/link
/// members — the same admission policy as the release installer). Returns
/// the extracted package root for `cargo install --path`.
fn extract_crate_archive(harness: &mut Harness, archive: &Path) -> Result<PathBuf, String> {
    let staging = harness.owned_staging("crate-extract")?;
    let file = std::fs::File::open(archive)
        .map_err(|error| format!("open package archive `{}`: {error}", archive.display()))?;
    let mut tar = Archive::new(GzDecoder::new(file));
    let mut expected_root: Option<PathBuf> = None;
    for (index, entry_result) in tar
        .entries()
        .map_err(|error| format!("read package archive entries: {error}"))?
        .enumerate()
    {
        let mut entry =
            entry_result.map_err(|error| format!("read package entry {index}: {error}"))?;
        let enclosed = entry
            .path()
            .map_err(|error| format!("read package entry {index} path: {error}"))?
            .into_owned();
        if enclosed.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        }) {
            return Err(format!(
                "package entry {enclosed:?} escapes extraction root"
            ));
        }
        let mut root = PathBuf::new();
        root.push(
            enclosed
                .components()
                .next()
                .ok_or_else(|| format!("package entry {index} is empty"))?,
        );
        match &expected_root {
            Some(expected) if *expected == root => {}
            Some(expected) => {
                return Err(format!(
                    "package entry {enclosed:?} is outside expected root {expected:?}"
                ));
            }
            None => expected_root = Some(root),
        }
        if entry.header().entry_type().is_symlink() || entry.header().entry_type().is_hard_link() {
            return Err(format!("package entry {enclosed:?} is a link"));
        }
        let output = staging.join(&enclosed);
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create package parent: {error}"))?;
        }
        entry
            .unpack(&output)
            .map_err(|error| format!("extract package entry {enclosed:?}: {error}"))?;
    }
    let root = expected_root.ok_or_else(|| "package archive holds no entries".to_string())?;
    let package_root = staging.join(root);
    if !package_root.join("Cargo.toml").is_file() {
        return Err(format!(
            "extracted package root `{}` holds no Cargo.toml",
            package_root.display()
        ));
    }
    Ok(package_root)
}

/// Installs the packaged candidate into a clean prefix with ordinary Cargo
/// installation tooling and binds the installed identity. The prefix must
/// not exist: reinstalling over a live prefix would mix generations. The
/// `.crate` archive is extracted into harness-owned staging first: `cargo
/// install --path` requires a package directory, never the archive itself.
fn install_package(
    harness: &mut Harness,
    crate_path: &str,
    prefix: &Path,
) -> Result<InstalledSubject, String> {
    if prefix.exists() {
        return Err(format!(
            "first-hour --prefix `{}` already exists; slice A never reinstalls over a live prefix",
            prefix.display()
        ));
    }
    let crate_digest = sha256_file(Path::new(crate_path))?;
    let package_root = extract_crate_archive(harness, Path::new(crate_path))?;
    // `--locked`: the shipped package carries its Cargo.lock. Fresh index
    // resolution drifts dependency versions (observed: unicode-ident /
    // unicode-properties skew breaking the build); qualification installs
    // the exact locked bytes or fails loudly.
    let argv = vec![
        "install".to_string(),
        "--locked".to_string(),
        "--root".to_string(),
        prefix.to_string_lossy().to_string(),
        "--path".to_string(),
        package_root.to_string_lossy().to_string(),
    ];
    let step_argv = std::iter::once("cargo".to_string())
        .chain(argv.clone())
        .collect::<Vec<_>>();
    let cwd = std::env::current_dir().map_err(|error| format!("current dir: {error}"))?;
    let started = unix_epoch_secs();
    let status = run::run_output_owned("cargo", &argv)
        .map(|_| "installed".to_string())
        .map_err(|error| format!("cargo install failed: {error}"));
    harness.ledger.push(LedgerEntry {
        step: "install".to_string(),
        argv: step_argv,
        cwd: cwd.to_string_lossy().to_string(),
        started_epoch_secs: started,
        status: status.clone().unwrap_or_else(|error| error),
    });
    status?;
    // `cargo install --root` places executables under `<prefix>/bin` on
    // every host; Windows adds `.exe` through `executable_name`.
    let executable = prefix.join("bin").join(executable_name());
    if !executable.is_file() {
        return Err(format!(
            "cargo install reported success but `{}` is absent",
            executable.display()
        ));
    }
    let executable_sha256 = sha256_file(&executable)?;
    let executable_size = std::fs::metadata(&executable)
        .map_err(|error| format!("stat installed binary: {error}"))?
        .len();
    let version_argv = vec!["--version".to_string()];
    let step_argv = vec![
        executable.to_string_lossy().to_string(),
        "--version".to_string(),
    ];
    let started_version = unix_epoch_secs();
    let version_output = run::run_output_owned(&executable.to_string_lossy(), &version_argv)
        .map_err(|error| format!("installed --version failed: {error}"))?;
    harness.ledger.push(LedgerEntry {
        step: "installed-version".to_string(),
        argv: step_argv,
        cwd: cwd.to_string_lossy().to_string(),
        started_epoch_secs: started_version,
        status: "recorded".to_string(),
    });
    Ok(InstalledSubject {
        crate_path: crate_path.to_string(),
        crate_sha256: crate_digest,
        prefix: prefix.to_path_buf(),
        executable,
        executable_sha256,
        executable_size,
        version_output: version_output.trim().to_string(),
    })
}

/// Refuses any executable that is not the installed identity. Later slices
/// resolve every product invocation through this check, so a PATH/worktree
/// binary can never silently substitute for the candidate.
fn admit_installed_executable(subject: &InstalledSubject, candidate: &Path) -> Result<(), String> {
    let digest = sha256_file(candidate)?;
    if digest != subject.executable_sha256 {
        return Err(format!(
            "wrong binary: `{}` digest does not match the installed identity; refusing",
            candidate.display()
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Receipt skeleton
// ---------------------------------------------------------------------------

fn ledger_json(entry: &LedgerEntry) -> Value {
    json!({
        "step": entry.step,
        "argv": entry.argv,
        "cwd": entry.cwd,
        "started_epoch_secs": entry.started_epoch_secs,
        "status": entry.status,
    })
}

fn write_receipt(out: &Path, subject: &InstalledSubject, harness: &Harness) -> Result<(), String> {
    let receipt = json!({
        "schema_version": SCHEMA_VERSION,
        "subject": {
            "crate_path": subject.crate_path,
            "crate_sha256": subject.crate_sha256,
            "prefix": subject.prefix.to_string_lossy(),
            "executable": subject.executable.to_string_lossy(),
            "executable_sha256": subject.executable_sha256,
            "executable_size": subject.executable_size,
            "version_output": subject.version_output,
        },
        "ledger": harness.ledger.iter().map(ledger_json).collect::<Vec<_>>(),
        "slices_completed": ["A"],
    });
    std::fs::create_dir_all(out).map_err(|error| format!("create out dir: {error}"))?;
    let text = serde_json::to_string_pretty(&receipt)
        .map_err(|error| format!("render receipt: {error}"))?;
    std::fs::write(out.join(RECEIPT_FILE), format!("{text}\n"))
        .map_err(|error| format!("write receipt: {error}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub(crate) fn first_hour(args: &[String]) -> Result<(), String> {
    let parsed = parse_args(args)?;
    let mut harness = Harness {
        roots: Vec::new(),
        ledger: Vec::new(),
    };
    // Fail-closed ownership, checked before any install work: a
    // pre-existing fixture root is never adopted for deletion. The harness
    // creates it, so cleanup can only remove what the harness made.
    // Comparisons use absolute lexically-normalized paths: a raw
    // `starts_with` on unresolved components would miss a `--fixture-root`
    // containing `..` and let the receipt (or the install tree) land inside
    // the cleanup tree.
    let fixture_root = PathBuf::from(&parsed.fixture_root);
    if fixture_root.exists() {
        return Err(format!(
            "first-hour --fixture-root `{}` already exists; slice A never deletes pre-existing directories",
            fixture_root.display()
        ));
    }
    let fixture_norm = absolute_normalized(&fixture_root)?;
    // The receipt must outlive cleanup: --out equal to or nested under the
    // harness-cleaned fixture root would be written and then deleted before
    // the command returns success.
    let out_root = PathBuf::from(&parsed.out);
    let out_norm = absolute_normalized(&out_root)?;
    if out_norm == fixture_norm || out_norm.starts_with(&fixture_norm) {
        return Err(format!(
            "first-hour --out `{}` must not equal or nest under --fixture-root `{}`; the receipt would be cleaned before return",
            out_root.display(),
            fixture_root.display()
        ));
    }
    let prefix = PathBuf::from(&parsed.prefix);
    // The install tree must outlive cleanup too: a --prefix equal to or
    // nested under --fixture-root would be created by `cargo install`
    // before the fixture root exists, then adopted into the cleanup set —
    // the command would return success after deleting the installed
    // executable the receipt points at.
    let prefix_norm = absolute_normalized(&prefix)?;
    if prefix_norm == fixture_norm || prefix_norm.starts_with(&fixture_norm) {
        return Err(format!(
            "first-hour --prefix `{}` must not equal or nest under --fixture-root `{}`; the install tree would be cleaned before return",
            prefix.display(),
            fixture_root.display()
        ));
    }
    let subject = install_package(&mut harness, &parsed.crate_path, &prefix)?;
    admit_installed_executable(&subject, &subject.executable)?;
    std::fs::create_dir_all(&fixture_root)
        .map_err(|error| format!("create fixture root: {error}"))?;
    harness.roots.push(fixture_root);
    write_receipt(Path::new(&parsed.out), &subject, &harness)?;
    println!(
        "first-hour slice A: installed {} ({}) ledger {} steps",
        subject.executable.display(),
        subject.version_output,
        harness.ledger.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns the harness's typed error, or fails the test when the call
    /// succeeded (the expect_err shape without the banned method).
    fn refusal_of<T>(result: Result<T, String>) -> Result<String, String> {
        match result {
            Ok(_) => Err("expected a typed refusal; the call succeeded".to_string()),
            Err(error) => Ok(error),
        }
    }

    #[test]
    fn wrong_binary_is_refused_against_the_installed_identity() -> Result<(), String> {
        let dir =
            std::env::temp_dir().join(format!("ripr-first-hour-wrong-bin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|error| format!("fixture dir: {error}"))?;
        let real = dir.join("ripr");
        let impostor = dir.join("other-ripr");
        std::fs::write(&real, b"installed-bytes")
            .map_err(|error| format!("write real: {error}"))?;
        std::fs::write(&impostor, b"different-bytes")
            .map_err(|error| format!("write impostor: {error}"))?;
        let subject = InstalledSubject {
            crate_path: "fixture.crate".to_string(),
            crate_sha256: "0".repeat(64),
            prefix: dir.clone(),
            executable: real.clone(),
            executable_sha256: sha256_hex(b"installed-bytes"),
            executable_size: 15,
            version_output: "ripr 0.11.0".to_string(),
        };
        assert!(matches!(
            admit_installed_executable(&subject, &real),
            Ok(())
        ));
        assert!(matches!(
            admit_installed_executable(&subject, &impostor),
            Err(error) if error.contains("wrong binary")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn install_refuses_a_live_prefix() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-first-hour-live-prefix-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|error| format!("live prefix fixture: {error}"))?;
        let mut harness = Harness {
            roots: Vec::new(),
            ledger: Vec::new(),
        };
        assert!(matches!(
            install_package(&mut harness, "missing.crate", &dir),
            Err(error) if error.contains("already exists")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn hostile_archive_members_are_rejected_before_extraction() -> Result<(), String> {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        let dir = std::env::temp_dir().join(format!(
            "ripr-first-hour-hostile-crate-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|error| format!("fixture dir: {error}"))?;
        let archive = dir.join("evil.crate");
        {
            let file = std::fs::File::create(&archive)
                .map_err(|error| format!("archive file: {error}"))?;
            let mut raw: Vec<u8> = Vec::new();
            // Raw ustar member with an exact (possibly hostile) name,
            // bypassing safe builders that refuse to construct it.
            // Decimal byte offsets per POSIX ustar: name 0, mode 100,
            // uid 108, gid 116, size 124, mtime 136, chksum 148,
            // typeflag 156, magic 257.
            fn raw_member(out: &mut Vec<u8>, name: &[u8], data: &[u8]) {
                let mut header = [0u8; 512];
                header[..name.len().min(100)].copy_from_slice(&name[..name.len().min(100)]);
                header[100..108].copy_from_slice(b"0000644\0");
                header[108..116].copy_from_slice(b"0000000\0");
                header[116..124].copy_from_slice(b"0000000\0");
                let size = format!("{:011o}\0", data.len());
                header[124..136].copy_from_slice(&size.as_bytes()[..12]);
                header[136..148].copy_from_slice(b"00000000000\0");
                header[148..156].copy_from_slice(b"        ");
                header[156] = b'0';
                header[257..263].copy_from_slice(b"ustar\0");
                let checksum: u32 = header.iter().map(|byte| *byte as u32).sum();
                header[148..156].copy_from_slice(format!("{checksum:06o}\0 ").as_bytes());
                out.extend_from_slice(&header);
                out.extend_from_slice(data);
                out.resize(out.len() + (512 - data.len() % 512) % 512, 0);
            }
            raw_member(&mut raw, b"pkg-evil/Cargo.toml", b"evil");
            raw_member(&mut raw, b"../escape", b"evil");
            use std::io::Write;
            let mut encoder = GzEncoder::new(file, Compression::default());
            encoder
                .write_all(&raw)
                .map_err(|error| format!("gzip body: {error}"))?;
            encoder
                .write_all(&[0u8; 1024])
                .map_err(|error| format!("gzip trailer: {error}"))?;
            encoder
                .finish()
                .map_err(|error| format!("gzip flush: {error}"))?;
        }
        let mut harness = Harness {
            roots: Vec::new(),
            ledger: Vec::new(),
        };
        assert!(matches!(
            extract_crate_archive(&mut harness, &archive),
            Err(error) if error.contains("escapes extraction root")
        ));
        // Nothing materialized outside staging.
        assert!(!dir.join("escape").exists());
        drop(harness);
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn pre_existing_fixture_root_is_never_adopted_for_deletion() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-first-hour-preexisting-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|error| format!("pre-existing dir: {error}"))?;
        let sentinel = dir.join("user-data.txt");
        std::fs::write(&sentinel, b"precious").map_err(|error| format!("sentinel: {error}"))?;
        assert!(matches!(
            first_hour(&[
                "--crate".to_string(),
                "missing.crate".to_string(),
                "--prefix".to_string(),
                dir.join("prefix").to_string_lossy().to_string(),
                "--out".to_string(),
                dir.join("out").to_string_lossy().to_string(),
                "--fixture-root".to_string(),
                dir.to_string_lossy().to_string(),
            ]),
            Err(error) if error.contains("already exists")
        ));
        // The pre-existing directory and its contents survive.
        assert!(
            std::fs::read(&sentinel).map_err(|error| format!("sentinel survives: {error}"))?
                == b"precious"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn receipt_nested_under_fixture_root_is_refused_before_any_work() -> Result<(), String> {
        let base =
            std::env::temp_dir().join(format!("ripr-first-hour-overlap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        assert!(matches!(
            first_hour(&[
                "--crate".to_string(),
                "missing.crate".to_string(),
                "--prefix".to_string(),
                base.join("prefix").to_string_lossy().to_string(),
                "--out".to_string(),
                base.join("fixtures")
                    .join("out")
                    .to_string_lossy()
                    .to_string(),
                "--fixture-root".to_string(),
                base.join("fixtures").to_string_lossy().to_string(),
            ]),
            Err(error) if error.contains("must not equal or nest under")
        ));
        // Nothing was created: no install, no fixture root, no receipt.
        assert!(!base.exists());
        Ok(())
    }

    #[test]
    fn prefix_nested_under_fixture_root_is_refused_before_any_work() -> Result<(), String> {
        let base = std::env::temp_dir().join(format!(
            "ripr-first-hour-prefix-nest-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        assert!(matches!(
            first_hour(&[
                "--crate".to_string(),
                "missing.crate".to_string(),
                "--prefix".to_string(),
                base.join("fixtures")
                    .join("prefix")
                    .to_string_lossy()
                    .to_string(),
                "--out".to_string(),
                base.join("out").to_string_lossy().to_string(),
                "--fixture-root".to_string(),
                base.join("fixtures").to_string_lossy().to_string(),
            ]),
            Err(error) if error.contains("must not equal or nest under")
        ));
        // Nothing was created: no install tree, no fixture root, no receipt.
        assert!(!base.exists());
        Ok(())
    }

    #[test]
    fn dotdot_evasion_of_receipt_containment_is_refused() -> Result<(), String> {
        let base =
            std::env::temp_dir().join(format!("ripr-first-hour-dotdot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        // `--out base/fixtures/../fixtures/out` normalizes inside the
        // fixture root; a raw lexical check on the un-normalized form would
        // still catch this spelling, so also cover the reverse: a fixture
        // root that escapes to the parent of --out.
        assert!(matches!(
            first_hour(&[
                "--crate".to_string(),
                "missing.crate".to_string(),
                "--prefix".to_string(),
                base.join("prefix").to_string_lossy().to_string(),
                "--out".to_string(),
                base.join("fixtures")
                    .join("out")
                    .to_string_lossy()
                    .to_string(),
                "--fixture-root".to_string(),
                base.join("other")
                    .join("..")
                    .join("fixtures")
                    .to_string_lossy()
                    .to_string(),
            ]),
            Err(error) if error.contains("must not equal or nest under")
        ));
        assert!(!base.exists());
        Ok(())
    }

    #[test]
    fn staging_claims_exclusive_ownership_without_deleting() -> Result<(), String> {
        let mut harness = Harness {
            roots: Vec::new(),
            ledger: Vec::new(),
        };
        let first = harness.owned_staging("exclusive")?;
        let second = harness.owned_staging("exclusive")?;
        assert!(first != second);
        // A squatter the harness did not create is never removed: occupy a
        // neighbouring path and prove it survives staging creation.
        let squatter = first
            .parent()
            .ok_or_else(|| "staging has no parent".to_string())?
            .join("ripr-first-hour-squatter");
        let _ = std::fs::remove_dir_all(&squatter);
        std::fs::create_dir_all(&squatter).map_err(|error| format!("squatter: {error}"))?;
        let sentinel = squatter.join("keep.txt");
        std::fs::write(&sentinel, b"not-ours").map_err(|error| format!("sentinel: {error}"))?;
        let _third = harness.owned_staging("exclusive")?;
        assert!(
            std::fs::read(&sentinel).map_err(|error| format!("squatter survives: {error}"))?
                == b"not-ours"
        );
        drop(harness);
        assert!(!first.exists());
        assert!(!second.exists());
        assert!(squatter.exists());
        let _ = std::fs::remove_dir_all(&squatter);
        Ok(())
    }

    #[test]
    fn staging_creates_a_missing_base_instead_of_assuming_it() -> Result<(), String> {
        // The home-cache fallback on a fresh runner does not exist yet;
        // exclusive creation must build the base, or every later step fails
        // with a staging error that masks the real work (observed on the
        // hosted lane: hostile-archive test failed with a staging error).
        let missing = std::env::temp_dir().join(format!(
            "ripr-first-hour-missing-base-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&missing);
        let mut harness = Harness {
            roots: Vec::new(),
            ledger: Vec::new(),
        };
        let root = Harness::owned_staging_in(&mut harness, &missing, "missing-base")?;
        assert!(root.is_dir());
        drop(harness);
        assert!(!root.exists());
        let _ = std::fs::remove_dir_all(&missing);
        Ok(())
    }

    #[test]
    fn staging_base_never_sits_inside_the_enclosing_workspace() -> Result<(), String> {
        let base = staging_base()?;
        let cwd = std::env::current_dir().map_err(|error| format!("current dir: {error}"))?;
        if let Some(root) = enclosing_workspace_root(&cwd) {
            assert!(
                !base.starts_with(&root),
                "staging `{}` sits inside workspace `{}`",
                base.display(),
                root.display()
            );
        }
        Ok(())
    }

    #[test]
    fn arg_surface_requires_every_identity_input() -> Result<(), String> {
        let _ = refusal_of(parse_args(&[]))?;
        let _ = refusal_of(parse_args(&["--crate".to_string(), "a.crate".to_string()]))?;
        let Ok(parsed) = parse_args(&[
            "--crate".to_string(),
            "a.crate".to_string(),
            "--prefix".to_string(),
            "p".to_string(),
            "--out".to_string(),
            "o".to_string(),
            "--fixture-root".to_string(),
            "f".to_string(),
        ]) else {
            return Err("complete arg surface must parse".to_string());
        };
        assert_eq!(parsed.prefix, "p");
        Ok(())
    }
}
