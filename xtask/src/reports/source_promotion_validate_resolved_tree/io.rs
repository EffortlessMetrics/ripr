fn render_markdown(report: &Value) -> Result<String, String> {
    let structured = serde_json::to_string_pretty(report)
        .map_err(|error| format!("failed to serialize structured receipt: {error}"))?;
    let schema = string_field(report, "schema")?;
    let status = string_field(report, "status")?;
    let source = report
        .get("source_parent")
        .and_then(Value::as_str)
        .unwrap_or("not available");
    let swarm = report
        .get("swarm_parent")
        .and_then(Value::as_str)
        .unwrap_or("not available");
    let tree = report
        .get("reviewed_tree")
        .and_then(Value::as_str)
        .unwrap_or("not available");
    Ok(format!(
        "# Resolved-tree validation\n\n- Schema: `{schema}`\n- Status: **{status}**\n- SOURCE_PARENT: `{source}`\n- SWARM_PARENT: `{swarm}`\n- REVIEWED_TREE: `{tree}`\n\n## Claim boundary\n\nThis source-parent-selected receipt reports the named repository-governance commands on one exact reviewed tree. `check-command-catalog` is trusted-checker self-health, not candidate command authority. The packet is published atomically only after its JSON, Markdown, command logs, and completion index are complete. It does not construct J, move an authoritative ref, qualify product/editor behavior, or authorize publication.\n\n## Structured receipt\n\n```json\n{structured}\n```\n"
    ))
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    if args.first().map(String::as_str) != Some(SOURCE_PROMOTION_VALIDATE_RESOLVED_TREE_SUBCOMMAND)
    {
        return Err(usage());
    }
    let mut values = BTreeMap::<&str, String>::new();
    let mut index = 1;
    while index < args.len() {
        let key = args[index].as_str();
        if !key.starts_with("--") || index + 1 >= args.len() {
            return Err(usage());
        }
        if !matches!(
            key,
            "--source-parent"
                | "--swarm-parent"
                | "--reviewed-tree"
                | "--preflight"
                | "--preflight-sha256"
                | "--resolution-manifest"
                | "--resolution-sha256"
                | "--out"
        ) {
            return Err(format!("unknown option {key}\n{}", usage()));
        }
        let value = args[index + 1].clone();
        if value.trim().is_empty() || value.starts_with("--") {
            return Err(format!("missing value for {key}\n{}", usage()));
        }
        if values.insert(key, value).is_some() {
            return Err(format!("duplicate option {key}"));
        }
        index += 2;
    }
    let required = |key: &str| {
        values
            .get(key)
            .cloned()
            .ok_or_else(|| format!("missing {key}\n{}", usage()))
    };
    let source_parent = required("--source-parent")?;
    let swarm_parent = required("--swarm-parent")?;
    let reviewed_tree = required("--reviewed-tree")?;
    let preflight_sha256 = required("--preflight-sha256")?;
    let resolution_sha256 = required("--resolution-sha256")?;
    validate_exact_hex("--source-parent", &source_parent, 40)?;
    validate_exact_hex("--swarm-parent", &swarm_parent, 40)?;
    validate_exact_hex("--reviewed-tree", &reviewed_tree, 40)?;
    validate_exact_hex("--preflight-sha256", &preflight_sha256, 64)?;
    validate_exact_hex("--resolution-sha256", &resolution_sha256, 64)?;
    let repo = std::env::current_dir()
        .map_err(|error| format!("failed to read current repository directory: {error}"))?;
    let out = values
        .get("--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(SOURCE_PROMOTION_RESOLVED_TREE_DEFAULT_OUT));
    Ok(Options {
        repo,
        source_parent,
        swarm_parent,
        reviewed_tree,
        preflight: PathBuf::from(required("--preflight")?),
        preflight_sha256,
        resolution_manifest: PathBuf::from(required("--resolution-manifest")?),
        resolution_sha256,
        out,
    })
}

fn usage() -> String {
    "usage: cargo xtask source-promotion validate-resolved-tree --source-parent <40-char-sha> --swarm-parent <40-char-sha> --reviewed-tree <40-char-tree> --preflight <path> --preflight-sha256 <64-char-digest> --resolution-manifest <path> --resolution-sha256 <64-char-digest> [--out <dir>]".to_string()
}

fn output_path_from_args(args: &[String]) -> Option<PathBuf> {
    value_after(args, "--out").map(PathBuf::from)
}

fn input_echo(args: &[String]) -> InputEcho {
    InputEcho {
        source_parent: value_after(args, "--source-parent"),
        swarm_parent: value_after(args, "--swarm-parent"),
        reviewed_tree: value_after(args, "--reviewed-tree"),
        preflight_path: value_after(args, "--preflight"),
        preflight_sha256: value_after(args, "--preflight-sha256"),
        resolution_path: value_after(args, "--resolution-manifest"),
        resolution_sha256: value_after(args, "--resolution-sha256"),
    }
}

fn input_echo_from_options(options: &Options) -> InputEcho {
    InputEcho {
        source_parent: Some(options.source_parent.clone()),
        swarm_parent: Some(options.swarm_parent.clone()),
        reviewed_tree: Some(options.reviewed_tree.clone()),
        preflight_path: Some(path_for_receipt(&options.repo, &options.preflight)),
        preflight_sha256: Some(options.preflight_sha256.clone()),
        resolution_path: Some(path_for_receipt(
            &options.repo,
            &options.resolution_manifest,
        )),
        resolution_sha256: Some(options.resolution_sha256.clone()),
    }
}

fn value_after(args: &[String], key: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == key && !pair[1].trim().is_empty() && !pair[1].starts_with("--"))
        .map(|pair| pair[1].clone())
}

fn read_bound_json(
    repo: &Path,
    path: &Path,
    expected_digest: &str,
    label: &str,
) -> Result<(Value, String), String> {
    reject_parent_components(path, label)?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo.join(path)
    };
    let metadata = fs::symlink_metadata(&candidate)
        .map_err(|error| format!("failed to inspect {label} {}: {error}", candidate.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("{label} must be a non-symlink regular file"));
    }
    let canonical_repo = repo
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize repository root: {error}"))?;
    let canonical = candidate
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize {label}: {error}"))?;
    if !canonical.starts_with(&canonical_repo) {
        return Err(format!("{label} escapes the source checkout"));
    }
    let bytes = fs::read(&canonical)
        .map_err(|error| format!("failed to read {label} {}: {error}", canonical.display()))?;
    let actual_digest = digest_bytes(&bytes);
    if actual_digest != expected_digest {
        return Err(format!(
            "{label} SHA-256 mismatch: expected {expected_digest}, observed {actual_digest}"
        ));
    }
    let value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("malformed {label} JSON: {error}"))?;
    let relative = canonical
        .strip_prefix(&canonical_repo)
        .map_err(|error| format!("{label} is outside the source checkout: {error}"))?;
    Ok((value, normalize_path(relative)))
}

fn reject_parent_components(path: &Path, label: &str) -> Result<(), String> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!("{label} path contains a parent-directory escape"));
    }
    Ok(())
}

fn ensure_directory_path(path: &Path) -> Result<(), String> {
    reject_parent_components(path, "output parent")?;
    let mut current = if path.is_absolute() {
        PathBuf::new()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("failed to read current directory: {error}"))?
    };
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(Path::new(std::path::MAIN_SEPARATOR_STR)),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err("output parent contains a parent-directory escape".to_string());
            }
            Component::Normal(part) => {
                current.push(part);
                match fs::symlink_metadata(&current) {
                    Ok(metadata) => {
                        if metadata.file_type().is_symlink() || !metadata.is_dir() {
                            return Err(format!(
                                "output parent component is not a non-symlink directory: {}",
                                current.display()
                            ));
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        fs::create_dir(&current).map_err(|create_error| {
                            format!(
                                "failed to create output parent {}: {create_error}",
                                current.display()
                            )
                        })?;
                    }
                    Err(error) => {
                        return Err(format!(
                            "failed to inspect output parent {}: {error}",
                            current.display()
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("packet file has no parent: {}", path.display()))?;
    let parent_metadata = fs::symlink_metadata(parent)
        .map_err(|error| format!("failed to inspect packet file parent: {error}"))?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Err("packet file parent is not a non-symlink directory".to_string());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("failed to create new packet file {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("failed to write packet file {}: {error}", path.display()))?;
    file.flush()
        .map_err(|error| format!("failed to flush packet file {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("failed to sync packet file {}: {error}", path.display()))?;
    Ok(())
}

fn packet_entries(root: &Path) -> Result<Vec<Value>, String> {
    let mut paths = Vec::<PathBuf>::new();
    collect_packet_paths(root, root, &mut paths)?;
    paths.sort();
    paths
        .into_iter()
        .filter(|relative| relative != Path::new(PACKET_INDEX))
        .map(|relative| {
            let path = root.join(&relative);
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("failed to inspect packet evidence {}: {error}", path.display()))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "packet evidence is not a non-symlink regular file: {}",
                    path.display()
                ));
            }
            let bytes = fs::read(&path)
                .map_err(|error| format!("failed to read packet evidence {}: {error}", path.display()))?;
            Ok(serde_json::json!({
                "path": normalize_path(&relative),
                "bytes": bytes.len(),
                "sha256": digest_bytes(&bytes),
            }))
        })
        .collect()
}

fn collect_packet_paths(root: &Path, current: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries = fs::read_dir(current)
        .map_err(|error| format!("failed to enumerate packet directory {}: {error}", current.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to enumerate packet directory entry: {error}"))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("failed to inspect packet entry {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("packet entry is a symlink: {}", path.display()));
        }
        if metadata.is_dir() {
            collect_packet_paths(root, &path, paths)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| format!("packet entry escaped staging root: {error}"))?;
            paths.push(relative.to_path_buf());
        } else {
            return Err(format!("packet entry has unsupported type: {}", path.display()));
        }
    }
    Ok(())
}

fn verify_exact_commit(repo: &Path, value: &str, label: &str) -> Result<(), String> {
    let resolved = git(
        repo,
        &["rev-parse", "--verify", &format!("{value}^{{commit}}")],
        &[],
    )?;
    if resolved.trim() != value {
        return Err(format!("{label} is not an exact commit object"));
    }
    Ok(())
}

fn verify_exact_tree(repo: &Path, value: &str) -> Result<(), String> {
    let resolved = git(
        repo,
        &["rev-parse", "--verify", &format!("{value}^{{tree}}")],
        &[],
    )?;
    if resolved.trim() != value {
        return Err("--reviewed-tree is not an exact tree object".to_string());
    }
    Ok(())
}

fn snapshot_refs(repo: &Path) -> Result<String, String> {
    git(
        repo,
        &[
            "for-each-ref",
            "--sort=refname",
            "--format=%(refname)%00%(objectname)",
            "refs",
        ],
        &[],
    )
}

fn snapshot_worktrees(repo: &Path) -> Result<String, String> {
    git(repo, &["worktree", "list", "--porcelain"], &[])
}

fn git(repo: &Path, args: &[&str], envs: &[(&str, &str)]) -> Result<String, String> {
    let mut owned_args = vec!["--no-replace-objects".to_string()];
    owned_args.extend(args.iter().map(|value| (*value).to_string()));
    let output = capture_output_in_dir_with_timeout_bounded(
        Path::new("git"),
        &owned_args,
        envs,
        repo,
        GIT_TIMEOUT,
        MAX_STREAM_BYTES,
        &format!("git {}", args.join(" ")),
    )?;
    if output.timed_out {
        return Err(format!(
            "git {} exceeded the 60 second bound",
            args.join(" ")
        ));
    }
    if !output
        .status
        .as_ref()
        .is_some_and(|status| status.success())
    {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            output.stderr.trim()
        ));
    }
    Ok(output.stdout)
}

fn create_exclusive_temp_dir(prefix: &str, identity: &str) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock predates Unix epoch: {error}"))?
        .as_nanos();
    for attempt in 0..TEMP_ATTEMPTS {
        let seed = format!(
            "{prefix}:{}:{timestamp}:{attempt}:{identity}",
            std::process::id()
        );
        let token = digest_bytes(seed.as_bytes());
        let candidate = std::env::temp_dir().join(format!("{prefix}-{}", &token[..24]));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "failed to create exclusive materialization directory: {error}"
                ));
            }
        }
    }
    Err("failed to allocate an exclusive materialization directory".to_string())
}

fn digest_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("failed to open {} for hashing: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("failed to hash {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_exact_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn validate_exact_hex(label: &str, value: &str, length: usize) -> Result<(), String> {
    if !is_exact_lower_hex(value, length) {
        return Err(format!(
            "{label} must be an exact {length}-character lowercase hexadecimal identity"
        ));
    }
    Ok(())
}

fn string_field<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|field| !field.trim().is_empty())
        .ok_or_else(|| format!("missing string field {key}"))
}

fn worktree_listing_contains_path(listing: &str, candidate: &str) -> bool {
    listing
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .map(|path| path.replace('\\', "/"))
        .any(|path| {
            if cfg!(windows) {
                path.eq_ignore_ascii_case(candidate)
            } else {
                path == candidate
            }
        })
}

fn path_for_receipt(repo: &Path, path: &Path) -> String {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo.join(path)
    };
    match (repo.canonicalize(), candidate.canonicalize()) {
        (Ok(root), Ok(canonical)) => canonical
            .strip_prefix(root)
            .map(normalize_path)
            .unwrap_or_else(|_| "outside-source-checkout".to_string()),
        _ => normalize_path(path),
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
