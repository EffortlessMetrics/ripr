fn read_regular(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > limit {
        return Err(format!(
            "producer evidence is not a bounded regular file: {}",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("producer evidence exceeded bound".to_string());
    }
    Ok(bytes)
}

fn actual_args(command: &str) -> Vec<String> {
    let mut args = vec!["test".to_string()];
    args.extend(command.split_whitespace().skip(2).map(ToString::to_string));
    args.extend(
        ["--message-format=json", "--", "--list", "--format", "terse"].map(ToString::to_string),
    );
    args
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn json_sha(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_vec(value)
        .map(|bytes| sha(&bytes))
        .map_err(|error| format!("serialize producer identity: {error}"))
}

fn file_sha(path: &Path) -> Result<String, String> {
    file_sha_before(path, None)
}

fn remaining(deadline: Option<Instant>) -> Result<Duration, String> {
    deadline.map_or(Ok(LIMIT), |end| {
        end.checked_duration_since(Instant::now())
            .filter(|value| !value.is_zero())
            .map(|value| value.min(LIMIT))
            .ok_or_else(|| {
                "file-policy producer verification exceeded the shared command deadline".to_string()
            })
    })
}

#[derive(Clone, PartialEq)]
struct FileStamp {
    length: u64,
    modified: std::time::SystemTime,
    created: Option<std::time::SystemTime>,
    #[cfg(unix)]
    unix_identity: (u64, u64, i64, i64),
}

fn stamp(path: &Path) -> Result<FileStamp, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("producer file is not regular".to_string());
    }
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    Ok(FileStamp {
        length: metadata.len(),
        modified: metadata.modified().map_err(|error| error.to_string())?,
        created: metadata.created().ok(),
        #[cfg(unix)]
        unix_identity: (
            metadata.dev(),
            metadata.ino(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        ),
    })
}

type Observations = BTreeMap<PathBuf, (FileStamp, String)>;
thread_local! {
    static OBSERVED_FILES: std::cell::RefCell<Option<Observations>> = const { std::cell::RefCell::new(None) };
}

// Digest reuse lasts for one synchronous producer/verification call only.
// Every observation checks file identity; completion rehashes each unique file.
struct ObservedFiles;
impl ObservedFiles {
    fn start() -> Result<Self, String> {
        OBSERVED_FILES.with_borrow_mut(|files| {
            if files.is_some() {
                return Err("nested producer observation is unsupported".to_string());
            }
            *files = Some(BTreeMap::new());
            Ok(Self)
        })
    }
    fn finish(&self, deadline: Option<Instant>) -> Result<(), String> {
        let files = OBSERVED_FILES
            .with_borrow(|files| files.clone())
            .ok_or("missing file observations")?;
        for (path, (identity, digest)) in files {
            if stamp(&path)? != identity
                || hash_bytes_before(&path, deadline)? != digest
                || stamp(&path)? != identity
            {
                return Err("producer file moved during its observation lifetime".to_string());
            }
        }
        Ok(())
    }
}
impl Drop for ObservedFiles {
    fn drop(&mut self) {
        OBSERVED_FILES.with_borrow_mut(|files| *files = None);
    }
}

fn file_sha_before(path: &Path, deadline: Option<Instant>) -> Result<String, String> {
    remaining(deadline)?;
    let identity = stamp(path)?;
    let observed = OBSERVED_FILES
        .with_borrow(|files| files.as_ref().and_then(|files| files.get(path).cloned()));
    if let Some((previous, digest)) = observed {
        if previous != identity {
            return Err("producer file identity moved after selection".to_string());
        }
        return Ok(digest);
    }
    let digest = hash_bytes_before(path, deadline)?;
    if stamp(path)? != identity {
        return Err("producer file moved while hashing".to_string());
    }
    OBSERVED_FILES.with_borrow_mut(|files| {
        if let Some(files) = files {
            files.insert(path.to_path_buf(), (identity, digest.clone()));
        }
    });
    Ok(digest)
}

fn hash_bytes_before(path: &Path, deadline: Option<Instant>) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("inspect producer file {}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(format!("producer file is not regular: {}", path.display()));
    }
    let mut input = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        remaining(deadline)?;
        let count = input.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn regular_root(path: &Path) -> Result<PathBuf, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("producer root is not a live non-symlink directory".to_string());
    }
    path.canonicalize().map_err(|error| error.to_string())
}

fn capture(
    program: &Path,
    args: &[String],
    root: &Path,
) -> Result<crate::run::TimedBoundedOutput, String> {
    capture_output_in_dir_with_timeout_bounded(
        program,
        args,
        &[("GIT_NO_REPLACE_OBJECTS", "1")],
        root,
        LIMIT,
        STREAM_LIMIT,
        "file-policy source-bound producer",
    )
}

fn successful(program: &Path, args: &[&str], root: &Path) -> Result<String, String> {
    successful_before(program, args, root, None)
}

fn successful_before(
    program: &Path,
    args: &[&str],
    root: &Path,
    deadline: Option<Instant>,
) -> Result<String, String> {
    let args = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    let output = capture_output_in_dir_with_timeout_bounded(
        program,
        &args,
        &[("GIT_NO_REPLACE_OBJECTS", "1")],
        root,
        remaining(deadline)?,
        STREAM_LIMIT,
        "file-policy producer identity observation",
    )?;
    if output.timed_out
        || output.stdout_truncated
        || output.stderr_truncated
        || !output.status.is_some_and(|status| status.success())
    {
        return Err(format!(
            "producer command did not complete: {} {args:?}; timeout={}; stdout={}; stderr={}",
            program.display(),
            output.timed_out,
            output.stdout,
            output.stderr
        ));
    }
    Ok(output.stdout.trim().to_string())
}

fn compiler() -> PathBuf {
    std::env::var_os("RUSTC")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO"))
                .with_file_name(format!("rustc{}", std::env::consts::EXE_SUFFIX))
        })
}

fn inputs(root: &Path) -> Result<Inputs, String> {
    inputs_before(root, None)
}

fn inputs_before(root: &Path, deadline: Option<Instant>) -> Result<Inputs, String> {
    let git = |args: &[&str]| successful_before(Path::new("git"), args, root, deadline);
    let hash = |path: &Path| file_sha_before(path, deadline);
    let root = regular_root(root)?;
    for name in ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"] {
        if std::env::var_os(name).is_some_and(|value| !value.is_empty()) {
            return Err("source-bound producer does not support compiler wrappers".to_string());
        }
    }
    if !git(&["status", "--porcelain=v1"])?.is_empty() {
        return Err("producer source checkout is not clean".to_string());
    }
    let mut configs = BTreeMap::new();
    for ancestor in root.ancestors() {
        for name in ["config", "config.toml"] {
            let path = ancestor.join(".cargo").join(name);
            if path.try_exists().map_err(|error| error.to_string())? {
                configs.insert(path.clone(), hash(&path)?);
            }
        }
    }
    let cargo_home = match std::env::var_os("CARGO_HOME") {
        Some(home) if !home.is_empty() => PathBuf::from(home),
        Some(_) => return Err("empty CARGO_HOME is not a supported producer context".to_string()),
        None => PathBuf::from(
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .ok_or("Cargo home is unavailable")?,
        )
        .join(".cargo"),
    };
    let home = if cargo_home.is_absolute() {
        cargo_home
    } else {
        root.join(cargo_home)
    };
    {
        for name in ["config", "config.toml"] {
            let path = home.join(name);
            if path.try_exists().map_err(|error| error.to_string())? {
                configs.insert(path.clone(), hash(&path)?);
            }
        }
    }
    configs.insert(
        root.join("rust-toolchain.toml"),
        hash(&root.join("rust-toolchain.toml"))?,
    );
    // Hash relevant values, without recording registry credentials or arbitrary
    // inherited environment. Cargo supplies package-derived values itself.
    let environment = std::env::vars_os()
        .filter_map(|(key, value)| {
            let key = key.to_string_lossy().into_owned();
            let relevant = matches!(
                key.as_str(),
                "PATH"
                    | "CARGO_HOME"
                    | "RUSTUP_HOME"
                    | "RUSTUP_TOOLCHAIN"
                    | "RUSTC"
                    | "RUSTC_WRAPPER"
                    | "RUSTC_WORKSPACE_WRAPPER"
                    | "RUSTFLAGS"
                    | "CARGO_ENCODED_RUSTFLAGS"
                    | "CARGO_TARGET_DIR"
                    | "CARGO_NET_OFFLINE"
            ) || key.starts_with("CARGO_BUILD_")
                || key.starts_with("CARGO_PROFILE_")
                || key.starts_with("CARGO_TARGET_");
            relevant.then(|| (key, sha(value.as_encoded_bytes())))
        })
        .collect::<BTreeMap<_, _>>();
    Ok(Inputs {
        head: git(&["rev-parse", "HEAD"])?,
        tree: git(&["rev-parse", "HEAD^{tree}"])?,
        lock_sha256: hash(&root.join("Cargo.lock"))?,
        cargo_sha256: hash(Path::new(env!("CARGO")))?,
        rustc_sha256: hash(&compiler())?,
        config_sha256: configs,
        environment_sha256: json_sha(&environment)?,
        root,
    })
}

fn anchor() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "trusted checker has no source-root anchor".to_string())
        .and_then(regular_root)
}

fn choose_root(
    subject: &Path,
    source_parent: &str,
    candidate: Option<&Path>,
) -> Result<(String, Inputs), String> {
    choose_root_before(subject, source_parent, candidate, None)
}

fn choose_root_before(
    subject: &Path,
    source_parent: &str,
    candidate: Option<&Path>,
    deadline: Option<Instant>,
) -> Result<(String, Inputs), String> {
    let subject = inputs_before(subject, deadline)?;
    if let Some(candidate) = candidate
        && let Ok(source) = inputs_before(candidate, deadline)
        && source.head == source_parent
        && source.tree == subject.tree
    {
        return Ok(("exact_source_parent_checkout".to_string(), source));
    }
    Ok(("reviewed_tree_worktree".to_string(), subject))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("create producer evidence {}: {error}", path.display()))?;
    file.write_all(bytes).map_err(|error| error.to_string())
}

fn test_profile(inputs: &Inputs) -> Result<Value, String> {
    if std::env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("CARGO_PROFILE_")) {
        return Err(
            "source-bound producer does not support Cargo profile environment overrides"
                .to_string(),
        );
    }
    for path in inputs.config_sha256.keys().filter(|path| {
        path.file_name()
            .is_some_and(|name| name == "config" || name == "config.toml")
    }) {
        let value: toml::Value = toml::from_str(
            &String::from_utf8(read_regular(path, STREAM_LIMIT as u64)?)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        if value.get("profile").is_some() {
            return Err(
                "source-bound producer does not support Cargo config profile overrides".to_string(),
            );
        }
    }
    let value: toml::Value = toml::from_str(
        &String::from_utf8(read_regular(
            &inputs.root.join("Cargo.toml"),
            STREAM_LIMIT as u64,
        )?)
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let mut opt = "0".to_string();
    let mut debug = serde_json::json!(2);
    let mut assertions = true;
    let mut overflow = None;
    for name in ["dev", "test"] {
        if let Some(profile) = value
            .get("profile")
            .and_then(|profiles| profiles.get(name))
            .and_then(toml::Value::as_table)
        {
            if ["package", "build-override", "inherits"]
                .iter()
                .any(|key| profile.contains_key(*key))
            {
                return Err("source-bound producer does not support custom test profile inheritance or package overrides".to_string());
            }
            if let Some(value) = profile.get("opt-level") {
                opt = value
                    .as_integer()
                    .map(|value| value.to_string())
                    .or_else(|| value.as_str().map(ToString::to_string))
                    .ok_or("invalid test opt-level")?;
            }
            if let Some(value) = profile.get("debug") {
                debug = match value {
                    toml::Value::Boolean(value) => serde_json::json!(if *value { 2 } else { 0 }),
                    toml::Value::Integer(value) if (0..=2).contains(value) => {
                        serde_json::json!(value)
                    }
                    toml::Value::String(value) => match value.as_str() {
                        "none" => serde_json::json!(0),
                        "limited" => serde_json::json!(1),
                        "full" => serde_json::json!(2),
                        "line-tables-only" | "line-directives-only" => serde_json::json!(value),
                        _ => return Err("unsupported test debuginfo".to_string()),
                    },
                    _ => return Err("unsupported test debuginfo".to_string()),
                };
            }
            if let Some(value) = profile.get("debug-assertions") {
                assertions = value.as_bool().ok_or("invalid debug assertions")?;
            }
            if let Some(value) = profile.get("overflow-checks") {
                overflow = Some(value.as_bool().ok_or("invalid overflow checks")?);
            }
        }
    }
    Ok(
        serde_json::json!({"opt_level": opt, "debuginfo": debug, "debug_assertions": assertions, "overflow_checks": overflow.unwrap_or(assertions), "test": true}),
    )
}
