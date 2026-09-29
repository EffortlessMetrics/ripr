//! Records which commit this `ripr` binary was built from.
//!
//! Emits `RIPR_BUILD_COMMIT` (a full commit id, or empty when unknown) and
//! `RIPR_BUILD_COMMIT_DIRTY` (`true` or `false`) for `ripr --version` and
//! `ripr doctor`. Sources, in order:
//!
//! 1. `.cargo_vcs_info.json`, which `cargo package` writes into every packaged
//!    crate. A crates.io install or an unpacked `.crate` has no `.git`, so this
//!    is its only commit record.
//! 2. The enclosing Git checkout, but only when it tracks this manifest. A
//!    crate unpacked inside some unrelated repository must not borrow that
//!    repository's HEAD.
//!
//! Anything else leaves the commit empty: unknown is reported as unknown,
//! never guessed. Both variables are always emitted so an ambient environment
//! variable of the same name cannot stand in for the record.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[path = "src/build_commit_record.rs"]
mod build_commit_record;

use build_commit_record::{is_full_commit_id, parse_cargo_vcs_info};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR");
    let identity = manifest_dir.as_deref().map(Path::new).and_then(|dir| {
        let vcs_info = dir.join(".cargo_vcs_info.json");
        if vcs_info.is_file() {
            println!("cargo:rerun-if-changed=.cargo_vcs_info.json");
            fs::read_to_string(vcs_info)
                .ok()
                .and_then(|text| parse_cargo_vcs_info(&text))
        } else {
            checkout_identity(dir)
        }
    });
    let (commit, dirty) = identity.unwrap_or_default();
    // A commit identifies a clean build's code. A dirty or commit-less build
    // is identified by its sources instead, so persisted analysis caches
    // never serve one such build's results to another.
    let source_digest = if commit.is_empty() || dirty {
        manifest_dir
            .as_deref()
            .map(Path::new)
            .and_then(source_digest)
            .unwrap_or_default()
    } else {
        String::new()
    };
    println!("cargo:rustc-env=RIPR_BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=RIPR_BUILD_COMMIT_DIRTY={dirty}");
    println!("cargo:rustc-env=RIPR_BUILD_SOURCE_DIGEST={source_digest}");
}

/// FNV-1a over the crate sources and the nearest workspace manifest and
/// lockfile: each file's path relative to that workspace root, its length and
/// its bytes, in sorted path order. `None` when any input cannot be read, so
/// an unreadable source never yields a digest that another build could share.
fn source_digest(dir: &Path) -> Option<String> {
    let root = dir
        .ancestors()
        .find(|ancestor| ancestor.join("Cargo.lock").is_file())
        .unwrap_or(dir);
    let mut files = Vec::new();
    for source in CRATE_SOURCES {
        let path = dir.join(source);
        println!("cargo:rerun-if-changed={}", path.display());
        collect_files(&path, &mut files)?;
    }
    for input in ["Cargo.toml", "Cargo.lock"] {
        let path = root.join(input);
        if path.is_file() {
            println!("cargo:rerun-if-changed={}", path.display());
            files.push(path);
        }
    }
    files.sort();
    files.dedup();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for file in &files {
        let bytes = fs::read(file).ok()?;
        let relative = file.strip_prefix(root).unwrap_or(file);
        feed(relative.to_string_lossy().replace('\\', "/").as_bytes());
        feed(&[0]);
        feed(&(bytes.len() as u64).to_le_bytes());
        feed(&bytes);
    }
    Some(format!("{hash:016x}"))
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) -> Option<()> {
    let metadata = fs::metadata(path).ok()?;
    if metadata.is_file() {
        files.push(path.to_path_buf());
    } else if metadata.is_dir() {
        for entry in fs::read_dir(path).ok()? {
            collect_files(&entry.ok()?.path(), files)?;
        }
    }
    Some(())
}

/// Crate sources that decide whether the built binary differs from the
/// commit, relative to the manifest directory.
const CRATE_SOURCES: [&str; 3] = ["src", "Cargo.toml", "build.rs"];

/// Repository-root Cargo inputs a workspace member inherits: the package
/// version, lints and profiles, the lockfile, and Cargo configuration.
const WORKSPACE_INPUTS: [&str; 3] = ["Cargo.toml", "Cargo.lock", ".cargo"];

fn checkout_identity(dir: &Path) -> Option<(String, bool)> {
    git(dir, &["ls-files", "--error-unmatch", "--", "Cargo.toml"])?;
    let commit = git(dir, &["rev-parse", "--verify", "HEAD"])?;
    if !is_full_commit_id(&commit) {
        return None;
    }
    // Re-run when HEAD moves and when an input the dirty flag covers changes.
    // A detached commit rewrites HEAD; a branch commit rewrites that branch's
    // loose ref (created in its directory when the branch was packed-only);
    // pack-refs rewrites packed-refs; a reftable repository rewrites its
    // table directory. Other branches are not watched, so creating or
    // deleting them does not rebuild ripr. Only existing paths are emitted:
    // Cargo treats a missing path as always changed.
    let mut watched: Vec<PathBuf> = ["HEAD", "packed-refs", "reftable"]
        .into_iter()
        .filter_map(|name| git(dir, &["rev-parse", "--git-path", name]))
        .map(|path| dir.join(path))
        .collect();
    if let Some(branch) = git(dir, &["symbolic-ref", "-q", "HEAD"])
        && let Some(path) = git(dir, &["rev-parse", "--git-path", &branch])
    {
        let path = dir.join(path);
        watched.extend(if path.exists() {
            Some(path)
        } else {
            path.parent().map(Path::to_path_buf)
        });
    }
    watched.extend(CRATE_SOURCES.iter().map(|source| dir.join(source)));
    let top = git(dir, &["rev-parse", "--show-toplevel"]).map(PathBuf::from);
    watched.extend(
        top.iter()
            .flat_map(|top| WORKSPACE_INPUTS.iter().map(move |input| top.join(input))),
    );
    for path in watched.iter().filter(|path| path.exists()) {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let top_inputs: Vec<String> = WORKSPACE_INPUTS
        .iter()
        .map(|input| format!(":(top){input}"))
        .collect();
    let mut status = vec![
        "--no-optional-locks",
        "status",
        "--porcelain",
        "--untracked-files=normal",
        "--",
    ];
    status.extend(CRATE_SOURCES);
    status.extend(top_inputs.iter().map(String::as_str));
    let dirty = !git(dir, &status)?.is_empty();
    Some((commit, dirty))
}

/// Run `git` in `dir` and return its trimmed stdout, or `None` when it cannot
/// run or exits nonzero. Inherited repository overrides (set inside Git hooks)
/// are cleared so the query describes this checkout.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|text| text.trim().to_string())
}
