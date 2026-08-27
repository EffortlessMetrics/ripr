fn parse_command_args(
    args: &[String],
    subcommand: &str,
    value_keys: &[&str],
    flag_keys: &[&str],
) -> Result<ParsedArgs, String> {
    if args.first().map(String::as_str) != Some(subcommand) {
        return Err(control_usage());
    }
    let allowed_values = value_keys.iter().copied().collect::<BTreeSet<_>>();
    let allowed_flags = flag_keys.iter().copied().collect::<BTreeSet<_>>();
    let mut values = BTreeMap::new();
    let mut flags = BTreeSet::new();
    let mut index = 1;
    while index < args.len() {
        let key = args[index].as_str();
        if allowed_flags.contains(key) {
            if !flags.insert(key.to_string()) {
                return Err(format!("duplicate flag {key}"));
            }
            index += 1;
            continue;
        }
        if !allowed_values.contains(key) {
            return Err(format!("unknown option {key}\n{}", control_usage()));
        }
        let Some(value) = args.get(index + 1) else {
            return Err(format!("missing value for {key}\n{}", control_usage()));
        };
        if value.trim().is_empty() || value.starts_with("--") {
            return Err(format!("missing value for {key}\n{}", control_usage()));
        }
        if values.insert(key.to_string(), value.clone()).is_some() {
            return Err(format!("duplicate option {key}"));
        }
        index += 2;
    }
    Ok(ParsedArgs { values, flags })
}

fn current_repo() -> Result<PathBuf, String> {
    std::env::current_dir()
        .map_err(|error| format!("failed to read current repository directory: {error}"))
}

fn validate_exact_hex(name: &str, value: &str, width: usize) -> Result<(), String> {
    if !is_exact_lower_hex(value, width) {
        return Err(format!(
            "{name} must be exactly {width} lowercase hexadecimal characters"
        ));
    }
    Ok(())
}

fn is_exact_lower_hex(value: &str, width: usize) -> bool {
    value.len() == width
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn json_string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn json_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn empty_failure_reasons(value: &Value) -> bool {
    value
        .get("failure_reasons")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read_regular_file(path: &Path, label: &str) -> Result<Vec<u8>, String> {
    reject_parent_components(path, label)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {label} {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("{label} must be a non-symlink regular file"));
    }
    fs::read(path).map_err(|error| format!("failed to read {label} {}: {error}", path.display()))
}

fn read_json(path: &Path, label: &str) -> Result<(Value, Vec<u8>, String), String> {
    let bytes = read_regular_file(path, label)?;
    let digest = digest_bytes(&bytes);
    let value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("malformed {label} JSON: {error}"))?;
    Ok((value, bytes, digest))
}

fn read_bound_json(
    path: &Path,
    expected_digest: &str,
    label: &str,
) -> Result<(Value, Vec<u8>), String> {
    let (value, bytes, actual) = read_json(path, label)?;
    if actual != expected_digest {
        return Err(format!(
            "{label} SHA-256 mismatch: expected {expected_digest}, observed {actual}"
        ));
    }
    Ok((value, bytes))
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

fn safe_packet_relative_path(value: &str, label: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    if path.is_absolute() || value.is_empty() {
        return Err(format!("{label} must be a non-empty relative path"));
    }
    reject_parent_components(&path, label)?;
    if path
        .components()
        .any(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
    {
        return Err(format!("{label} must stay inside its packet"));
    }
    Ok(path)
}

fn validate_packet_directory(path: &Path, label: &str) -> Result<(), String> {
    reject_parent_components(path, label)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {label} {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{label} must be a non-symlink directory"));
    }
    Ok(())
}

fn collect_regular_files(root: &Path) -> Result<Vec<String>, String> {
    fn walk(root: &Path, current: &Path, files: &mut Vec<String>) -> Result<(), String> {
        let mut entries = fs::read_dir(current)
            .map_err(|error| {
                format!(
                    "failed to enumerate packet directory {}: {error}",
                    current.display()
                )
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to enumerate packet entry: {error}"))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                format!("failed to inspect packet entry {}: {error}", path.display())
            })?;
            if metadata.file_type().is_symlink() {
                return Err(format!("packet entry is a symlink: {}", path.display()));
            }
            if metadata.is_dir() {
                walk(root, &path, files)?;
            } else if metadata.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|error| format!("packet entry escaped root: {error}"))?;
                files.push(normalize_path(relative));
            } else {
                return Err(format!(
                    "packet entry has unsupported type: {}",
                    path.display()
                ));
            }
        }
        Ok(())
    }

    validate_packet_directory(root, "packet")?;
    let mut files = Vec::new();
    walk(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn read_indexed_packet(
    root: &Path,
    expected_schema: &str,
    expected_kind: Option<&str>,
    expected_status: Option<&str>,
    required_report: &str,
) -> Result<IndexedPacket, String> {
    validate_packet_directory(root, "packet")?;
    let index_path = root.join(PACKET_INDEX);
    let (index, _, index_sha256) = read_json(&index_path, "packet index")?;
    if json_string(&index, "schema") != Some(expected_schema) {
        return Err("packet index uses an unsupported schema".to_string());
    }
    if json_bool(&index, "complete") != Some(true) {
        return Err("packet index is not complete".to_string());
    }
    if let Some(kind) = expected_kind
        && json_string(&index, "kind") != Some(kind)
    {
        return Err(format!("packet index kind is not {kind}"));
    }
    if let Some(status) = expected_status
        && json_string(&index, "status") != Some(status)
    {
        return Err(format!("packet index status is not {status}"));
    }
    let entries = index
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| "packet index is missing files".to_string())?;
    if entries.is_empty() {
        return Err("packet index files cannot be empty".to_string());
    }

    let mut files = BTreeMap::new();
    let mut previous: Option<String> = None;
    for entry in entries {
        let path_text = json_string(entry, "path")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "packet index entry is missing path".to_string())?;
        if previous.as_deref().is_some_and(|prior| prior >= path_text) {
            return Err("packet index paths must be strictly sorted and unique".to_string());
        }
        previous = Some(path_text.to_string());
        let relative = safe_packet_relative_path(path_text, "packet index path")?;
        if path_text == PACKET_INDEX {
            return Err("packet index must not index itself".to_string());
        }
        let expected_bytes = entry
            .get("bytes")
            .and_then(Value::as_u64)
            .ok_or_else(|| "packet index entry is missing bytes".to_string())?;
        let expected_digest = json_string(entry, "sha256")
            .filter(|value| is_exact_lower_hex(value, 64))
            .ok_or_else(|| "packet index entry has invalid sha256".to_string())?;
        let full_path = root.join(relative);
        let contents = read_regular_file(&full_path, "packet evidence")?;
        if contents.len() as u64 != expected_bytes {
            return Err(format!("packet byte count mismatch for {path_text}"));
        }
        let actual_digest = digest_bytes(&contents);
        if actual_digest != expected_digest {
            return Err(format!("packet digest mismatch for {path_text}"));
        }
        files.insert(
            path_text.to_string(),
            IndexedFile {
                sha256: actual_digest,
                contents,
            },
        );
    }

    if !files.contains_key(required_report) {
        return Err(format!(
            "packet is missing required report {required_report}"
        ));
    }
    let actual_files = collect_regular_files(root)?;
    let mut expected_files = files.keys().cloned().collect::<Vec<_>>();
    expected_files.push(PACKET_INDEX.to_string());
    expected_files.sort();
    if actual_files != expected_files {
        return Err(format!(
            "packet file inventory differs from its index: expected {expected_files:?}, observed {actual_files:?}"
        ));
    }

    Ok(IndexedPacket {
        index_sha256,
        files,
    })
}

fn packet_json(packet: &IndexedPacket, path: &str, label: &str) -> Result<Value, String> {
    let indexed = packet
        .files
        .get(path)
        .ok_or_else(|| format!("packet is missing {path}"))?;
    serde_json::from_slice(&indexed.contents)
        .map_err(|error| format!("malformed {label} JSON: {error}"))
}

fn packet_file_sha256(packet: &IndexedPacket, path: &str) -> Result<String, String> {
    packet
        .files
        .get(path)
        .map(|file| file.sha256.clone())
        .ok_or_else(|| format!("packet is missing {path}"))
}

#[derive(Debug)]
struct ControlPacketReservation {
    out: PathBuf,
}

fn reserve_control_packet_output(
    out: &Path,
    kind: &str,
    reconciliation_context: &Value,
) -> Result<ControlPacketReservation, String> {
    reject_parent_components(out, "control packet output")?;
    if fs::symlink_metadata(out).is_ok() {
        return Err(format!(
            "control packet output already exists: {}",
            out.display()
        ));
    }
    let parent = out
        .parent()
        .ok_or_else(|| "control packet output has no parent directory".to_string())?;
    ensure_directory_path(parent)?;
    fs::create_dir(out).map_err(|error| {
        format!(
            "failed to reserve exclusive control packet output {}: {error}",
            out.display()
        )
    })?;
    let attempt = serde_json::json!({
        "packet_schema": CONTROL_PACKET_SCHEMA,
        "kind": kind,
        "status": "reserved_before_side_effects",
        "complete": false,
        "reconciliation_context": reconciliation_context,
        "claim_boundary": "If packet-index.json is absent, a controller attempt was reserved but its final Git and remote state is unknown and must be reconciled before retry.",
    });
    let attempt_bytes = serde_json::to_string_pretty(&attempt)
        .map(|value| format!("{value}\n"))
        .map_err(|error| format!("failed to serialize control attempt journal: {error}"));
    let write_result = attempt_bytes
        .and_then(|bytes| write_new_file(&out.join("control-attempt.json"), bytes.as_bytes()));
    if let Err(error) = write_result {
        let _ = fs::remove_dir_all(out);
        return Err(error);
    }
    Ok(ControlPacketReservation {
        out: out.to_path_buf(),
    })
}

fn write_control_packet(
    out: &Path,
    kind: &str,
    report_name: &str,
    report: &Value,
    title: &str,
    claim_boundary: &str,
) -> Result<(), String> {
    let reservation = reserve_control_packet_output(out, kind, &Value::Null)?;
    write_reserved_control_packet(
        &reservation,
        kind,
        report_name,
        report,
        title,
        claim_boundary,
    )
}

fn write_reserved_control_packet(
    reservation: &ControlPacketReservation,
    kind: &str,
    report_name: &str,
    report: &Value,
    title: &str,
    claim_boundary: &str,
) -> Result<(), String> {
    let out = &reservation.out;
    (|| {
        let json = serde_json::to_string_pretty(report)
            .map_err(|error| format!("failed to serialize {kind} receipt: {error}"))?;
        let markdown_name = report_name
            .strip_suffix(".json")
            .map(|stem| format!("{stem}.md"))
            .ok_or_else(|| "control report name must end in .json".to_string())?;
        let markdown = render_control_markdown(title, report, claim_boundary)?;
        write_new_file(&out.join(report_name), format!("{json}\n").as_bytes())?;
        write_new_file(&out.join(&markdown_name), markdown.as_bytes())?;
        let mut entries = Vec::new();
        for relative in [
            "control-attempt.json".to_string(),
            report_name.to_string(),
            markdown_name,
        ] {
            let bytes = read_regular_file(&out.join(&relative), "staged packet file")?;
            entries.push(serde_json::json!({
                "path": relative,
                "bytes": bytes.len(),
                "sha256": digest_bytes(&bytes),
            }));
        }
        entries.sort_by(|left, right| json_string(left, "path").cmp(&json_string(right, "path")));
        let index = serde_json::json!({
            "schema": CONTROL_PACKET_SCHEMA,
            "kind": kind,
            "status": json_string(report, "status").unwrap_or("rejected"),
            "complete": true,
            "files": entries,
        });
        let index_json = serde_json::to_string_pretty(&index)
            .map_err(|error| format!("failed to serialize control packet index: {error}"))?;
        write_new_file(
            &out.join(PACKET_INDEX),
            format!("{index_json}\n").as_bytes(),
        )?;
        println!("Wrote {}", out.join(report_name).display());
        println!("Wrote {}", out.join(PACKET_INDEX).display());
        Ok(())
    })()
}

fn render_control_markdown(
    title: &str,
    report: &Value,
    claim_boundary: &str,
) -> Result<String, String> {
    let structured = serde_json::to_string_pretty(report)
        .map_err(|error| format!("failed to serialize structured receipt: {error}"))?;
    let status = json_string(report, "status").unwrap_or("rejected");
    Ok(format!(
        "# {title}\n\n- Status: **{status}**\n\n## Claim boundary\n\n{claim_boundary}\n\n## Structured receipt\n\n```json\n{structured}\n```\n"
    ))
}

fn ensure_directory_path(path: &Path) -> Result<(), String> {
    reject_parent_components(path, "output parent")?;
    let mut current = if path.is_absolute() {
        PathBuf::new()
    } else {
        current_repo()?
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
                                "output parent is not a non-symlink directory: {}",
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
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("failed to create packet file {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("failed to write packet file {}: {error}", path.display()))?;
    file.flush()
        .map_err(|error| format!("failed to flush packet file {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("failed to sync packet file {}: {error}", path.display()))
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn command_output(repo: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .current_dir(repo)
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .args(args)
        .output()
        .map_err(|error| format!("failed to run {program} {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "{program} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("{program} output was not UTF-8: {error}"))
}

fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    command_output(repo, "git", args)
}

fn exact_commit(repo: &Path, value: &str, label: &str) -> Result<(), String> {
    validate_exact_hex(label, value, 40)?;
    let kind = git(repo, &["cat-file", "-t", value])?;
    if kind.trim() != "commit" {
        return Err(format!("{label} is not a commit object"));
    }
    let resolved = git(
        repo,
        &["rev-parse", "--verify", &format!("{value}^{{commit}}")],
    )?;
    if resolved.trim() != value {
        return Err(format!("{label} is not an exact commit object"));
    }
    Ok(())
}

fn exact_tree(repo: &Path, value: &str, label: &str) -> Result<(), String> {
    validate_exact_hex(label, value, 40)?;
    let kind = git(repo, &["cat-file", "-t", value])?;
    if kind.trim() != "tree" {
        return Err(format!("{label} is not a tree object"));
    }
    Ok(())
}

fn read_commit_ref(repo: &Path, reference: &str, label: &str) -> Result<String, String> {
    validate_full_ref(reference, label)?;
    let resolved = git(
        repo,
        &["rev-parse", "--verify", &format!("{reference}^{{commit}}")],
    )?;
    let value = resolved.trim().to_string();
    validate_exact_hex(label, &value, 40)?;
    Ok(value)
}

fn read_tree_identity(repo: &Path, tree: &str) -> Result<String, String> {
    exact_tree(repo, tree, "join tree")?;
    Ok(tree.to_string())
}

fn validate_full_ref(reference: &str, label: &str) -> Result<(), String> {
    if !reference.starts_with("refs/")
        || reference.contains("..")
        || reference.contains('~')
        || reference.contains('^')
        || reference.contains(':')
        || reference.contains('?')
        || reference.contains('*')
        || reference.contains('[')
        || reference.contains('\\')
        || reference.ends_with('/')
        || reference.ends_with(".lock")
        || reference
            .split('/')
            .any(|part| part.is_empty() || part == ".")
    {
        return Err(format!("{label} is not a safe fully-qualified Git ref"));
    }
    Ok(())
}

fn refs_digest(repo: &Path) -> Result<String, String> {
    let output = git(
        repo,
        &[
            "for-each-ref",
            "--format=%(refname)%00%(objectname)",
            "refs/heads",
            "refs/tags",
        ],
    )?;
    Ok(digest_bytes(output.as_bytes()))
}

fn current_head(repo: &Path) -> Result<String, String> {
    let value = git(repo, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    let value = value.trim().to_string();
    validate_exact_hex("HEAD", &value, 40)?;
    Ok(value)
}

fn clean_checkout(repo: &Path) -> Result<bool, String> {
    Ok(
        git(repo, &["status", "--porcelain=v1", "--untracked-files=all"])?
            .trim()
            .is_empty(),
    )
}

fn current_executable_sha256() -> Result<String, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("failed to locate running xtask executable: {error}"))?;
    let bytes = read_regular_file(&executable, "running xtask executable")?;
    Ok(digest_bytes(&bytes))
}

fn file_sha256(path: &Path, label: &str) -> Result<String, String> {
    Ok(digest_bytes(&read_regular_file(path, label)?))
}

fn resolve_candidate_path(repo: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo.join(path)
    }
}

fn parse_remote_ref(output: &str, target_ref: &str) -> Result<Option<String>, String> {
    let lines = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return Ok(None);
    }
    if lines.len() != 1 {
        return Err(format!(
            "remote returned multiple rows for exact ref {target_ref}"
        ));
    }
    let mut parts = lines[0].split_whitespace();
    let sha = parts
        .next()
        .ok_or_else(|| "remote ref row is missing object identity".to_string())?;
    let reference = parts
        .next()
        .ok_or_else(|| "remote ref row is missing ref name".to_string())?;
    if parts.next().is_some() || reference != target_ref {
        return Err("remote ref row is malformed or names a different ref".to_string());
    }
    validate_exact_hex("remote ref object", sha, 40)?;
    Ok(Some(sha.to_string()))
}

fn read_remote_ref(repo: &Path, remote: &str, target_ref: &str) -> Result<Option<String>, String> {
    let output = git(repo, &["ls-remote", "--refs", remote, target_ref])?;
    parse_remote_ref(&output, target_ref)
}

fn read_optional_local_ref(repo: &Path, target_ref: &str) -> Result<Option<String>, String> {
    validate_full_ref(target_ref, "local candidate ref")?;
    let output = Command::new("git")
        .current_dir(repo)
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{target_ref}^{{commit}}"),
        ])
        .output()
        .map_err(|error| format!("failed to read local candidate ref: {error}"))?;
    if output.status.success() {
        let value = String::from_utf8(output.stdout)
            .map_err(|error| format!("local candidate ref output was not UTF-8: {error}"))?
            .trim()
            .to_string();
        validate_exact_hex("local candidate ref object", &value, 40)?;
        return Ok(Some(value));
    }
    if output.status.code() == Some(1) && output.stdout.is_empty() {
        return Ok(None);
    }
    Err(format!(
        "failed to read local candidate ref: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn read_remote_urls(repo: &Path, remote: &str) -> Result<(String, String), String> {
    let fetch = git(repo, &["remote", "get-url", remote])?
        .trim()
        .to_string();
    let push = git(repo, &["remote", "get-url", "--push", remote])?
        .trim()
        .to_string();
    if fetch.is_empty() || push.is_empty() || fetch.contains('\n') || push.contains('\n') {
        return Err("source remote URL is empty or ambiguous".to_string());
    }
    Ok((fetch, push))
}

fn update_local_ref(
    repo: &Path,
    target_ref: &str,
    new_value: Option<&str>,
    expected_old: Option<&str>,
) -> Result<(), String> {
    validate_full_ref(target_ref, "local candidate ref")?;
    let zero = "0000000000000000000000000000000000000000";
    let old = expected_old.unwrap_or(zero);
    validate_exact_hex("expected local candidate ref", old, 40)?;
    let args = match new_value {
        Some(value) => {
            validate_exact_hex("new local candidate ref", value, 40)?;
            vec!["update-ref", target_ref, value, old]
        }
        None => vec!["update-ref", "-d", target_ref, old],
    };
    git(repo, &args).map(|_| ())
}

fn commit_parents(repo: &Path, commit: &str) -> Result<Vec<String>, String> {
    let output = git(repo, &["rev-list", "--parents", "-n", "1", commit])?;
    let mut parts = output.split_whitespace();
    let head = parts
        .next()
        .ok_or_else(|| "commit parent output is empty".to_string())?;
    if head != commit {
        return Err("commit parent output did not begin with the exact commit".to_string());
    }
    Ok(parts.map(ToString::to_string).collect())
}

fn commit_tree(repo: &Path, commit: &str) -> Result<String, String> {
    let value = git(repo, &["rev-parse", &format!("{commit}^{{tree}}")])?;
    let value = value.trim().to_string();
    validate_exact_hex("commit tree", &value, 40)?;
    Ok(value)
}

fn write_rejection_or_combine(
    out: &Path,
    kind: &str,
    report_name: &str,
    report: &Value,
    title: &str,
    claim_boundary: &str,
    reason: String,
) -> Result<(), String> {
    match write_control_packet(out, kind, report_name, report, title, claim_boundary) {
        Ok(()) => Err(reason),
        Err(write_error) => Err(format!(
            "{reason}; failed to publish rejection packet: {write_error}"
        )),
    }
}

fn write_reserved_rejection_or_combine(
    reservation: &ControlPacketReservation,
    kind: &str,
    report_name: &str,
    report: &Value,
    title: &str,
    claim_boundary: &str,
    reason: String,
) -> Result<(), String> {
    match write_reserved_control_packet(
        reservation,
        kind,
        report_name,
        report,
        title,
        claim_boundary,
    ) {
        Ok(()) => Err(reason),
        Err(write_error) => Err(format!(
            "{reason}; failed to publish rejection packet: {write_error}"
        )),
    }
}
