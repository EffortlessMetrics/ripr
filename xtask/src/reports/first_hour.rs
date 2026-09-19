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

use serde_json::{Value, json};

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

impl Harness {
    fn record(&mut self, step: &str, argv: Vec<String>, cwd: &Path, status: String) {
        self.ledger.push(LedgerEntry {
            step: step.to_string(),
            argv,
            cwd: cwd.to_string_lossy().to_string(),
            started_epoch_secs: unix_epoch_secs(),
            status,
        });
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

/// Installs the packaged candidate into a clean prefix with ordinary Cargo
/// installation tooling and binds the installed identity. The prefix must
/// not exist: reinstalling over a live prefix would mix generations.
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
    let argv = vec![
        "install".to_string(),
        "--root".to_string(),
        prefix.to_string_lossy().to_string(),
        "--path".to_string(),
        crate_path.to_string(),
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
    let version_output = run::run_output_owned(&executable.to_string_lossy(), &version_argv)
        .map_err(|error| format!("installed --version failed: {error}"))?;
    harness.record(
        "installed-version",
        vec![
            executable.to_string_lossy().to_string(),
            "--version".to_string(),
        ],
        &cwd,
        "recorded".to_string(),
    );
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
    let prefix = PathBuf::from(&parsed.prefix);
    let subject = install_package(&mut harness, &parsed.crate_path, &prefix)?;
    admit_installed_executable(&subject, &subject.executable)?;
    let fixture_root = PathBuf::from(&parsed.fixture_root);
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

    #[test]
    fn wrong_binary_is_refused_against_the_installed_identity() {
        let dir =
            std::env::temp_dir().join(format!("ripr-first-hour-wrong-bin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("fixture dir");
        let real = dir.join("ripr");
        let impostor = dir.join("other-ripr");
        std::fs::write(&real, b"installed-bytes").expect("write real");
        std::fs::write(&impostor, b"different-bytes").expect("write impostor");
        let subject = InstalledSubject {
            crate_path: "fixture.crate".to_string(),
            crate_sha256: "0".repeat(64),
            prefix: dir.clone(),
            executable: real.clone(),
            executable_sha256: sha256_hex(b"installed-bytes"),
            executable_size: 15,
            version_output: "ripr 0.11.0".to_string(),
        };
        assert!(admit_installed_executable(&subject, &real).is_ok());
        let refused = admit_installed_executable(&subject, &impostor);
        assert!(refused.is_err());
        assert!(refused.unwrap_err().contains("wrong binary"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_refuses_a_live_prefix() {
        let dir = std::env::temp_dir().join(format!(
            "ripr-first-hour-live-prefix-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("live prefix fixture");
        let mut harness = Harness {
            roots: Vec::new(),
            ledger: Vec::new(),
        };
        let refused = install_package(&mut harness, "missing.crate", &dir);
        assert!(refused.is_err());
        assert!(refused.unwrap_err().contains("already exists"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn arg_surface_requires_every_identity_input() {
        assert!(parse_args(&[]).is_err());
        assert!(parse_args(&["--crate".to_string(), "a.crate".to_string(),]).is_err());
        let parsed = parse_args(&[
            "--crate".to_string(),
            "a.crate".to_string(),
            "--prefix".to_string(),
            "p".to_string(),
            "--out".to_string(),
            "o".to_string(),
            "--fixture-root".to_string(),
            "f".to_string(),
        ]);
        assert!(parsed.is_ok());
    }
}
