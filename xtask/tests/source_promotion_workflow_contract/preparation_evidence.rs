// Fixture diagnostics only. These copies never confer validation or admission.
fn owned_initial_report_directory(repo: &Path) -> Result<PathBuf, String> {
    let metadata = fs::symlink_metadata(repo).map_err(|_error| "report owner unavailable")?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("unexpected report owner type".into());
    }
    let mut directory = repo.to_path_buf();
    for part in ["target", "ripr", "reports"] {
        directory.push(part);
        match fs::symlink_metadata(&directory) {
            Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_dir() => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&directory)
                    .map_err(|_error| "owned report directory claim failed")?;
            }
            _ => return Err("unexpected owned report directory type".into()),
        }
    }
    let canonical = directory
        .canonicalize()
        .map_err(|_error| "report directory identity unavailable")?;
    if !canonical.starts_with(
        repo.canonicalize()
            .map_err(|_error| "report owner identity unavailable")?,
    ) {
        return Err("report directory escaped owner".into());
    }
    Ok(directory)
}

fn initial_preparation_manifest(
    root: &Path,
    packet: &str,
    report: &Value,
    index: &Value,
) -> Result<(Vec<u8>, Value), String> {
    let bytes =
        initial_diagnostic_bytes(root, &format!("{packet}/build-preparation.json"), 64 * 1024)?;
    let member = initial_diagnostic_member(index, "build-preparation.json")?;
    if member["bytes"].as_u64() != Some(bytes.len() as u64)
        || member["sha256"].as_str() != Some(format!("{:x}", Sha256::digest(&bytes)).as_str())
    {
        return Err("preparation manifest/index identity mismatch".into());
    }
    let manifest: Value =
        serde_json::from_slice(&bytes).map_err(|_error| "preparation manifest malformed")?;
    if manifest["schema"] != "ripr.materialized_build_preparation.v1"
        || manifest["setup_only"] != true
        || manifest["policy_acceptance_credit"] != false
        || manifest["source_parent"] != report["source_parent"]
        || manifest["reviewed_tree"] != report["reviewed_tree"]
        || manifest["disposable_commit"] != report["materialization"]["disposable_commit"]
        || !manifest["source_parent"].is_string()
        || !manifest["reviewed_tree"].is_string()
        || !manifest["disposable_commit"].is_string()
        || manifest["cargo_build_jobs"] != 1
        || manifest["stream_bytes_limit"] != 2 * 1024 * 1024
        || manifest["known_path_growth_bytes_limit"] != 4u64 * 1024 * 1024 * 1024
        || manifest["settlement_reserve_seconds"] != 30
        || !manifest["classes"]
            .as_array()
            .is_some_and(|rows| rows.len() <= 5)
    {
        return Err("preparation manifest/report identity or resource scope mismatch".into());
    }
    Ok((bytes, manifest))
}

fn initial_preparation_stream_bytes(
    root: &Path,
    packet: &str,
    index: &Value,
    row: &Value,
    position: usize,
    stream: &str,
) -> Result<(String, Vec<u8>), String> {
    let relative = format!("build-preparation/{:02}.{stream}.log", position + 1);
    let receipt = &row[stream];
    if receipt["path"].as_str() != Some(relative.as_str())
        || !receipt["bytes"]
            .as_u64()
            .is_some_and(|bytes| bytes <= 2 * 1024 * 1024)
        || !receipt["sha256"].is_string()
    {
        return Err("preparation stream receipt mapping or bound mismatch".into());
    }
    let member = initial_diagnostic_member(index, &relative)?;
    if member["bytes"] != receipt["bytes"] || member["sha256"] != receipt["sha256"] {
        return Err("preparation stream receipt/index identity mismatch".into());
    }
    let bytes = initial_diagnostic_bytes(root, &format!("{packet}/{relative}"), 2 * 1024 * 1024)?;
    if receipt["bytes"].as_u64() != Some(bytes.len() as u64)
        || receipt["sha256"].as_str() != Some(format!("{:x}", Sha256::digest(&bytes)).as_str())
    {
        return Err("preparation stream digest/extent changed".into());
    }
    Ok((relative, bytes))
}

fn initial_preparation_failure_context(
    root: &Path,
    packet: &str,
    report: &Value,
    index: &Value,
) -> Result<Option<(String, String)>, String> {
    if !index["files"].as_array().is_some_and(|files| {
        files
            .iter()
            .any(|member| member["path"] == "build-preparation.json")
    }) {
        return Ok(None);
    }
    let (bytes, manifest) = initial_preparation_manifest(root, packet, report, index)?;
    let Some(reason) = manifest["failure_reason"].as_str() else {
        return Ok(None);
    };
    let mut context = format!(
        "state=observed preparation_failure=true setup_only=true policy_acceptance_credit=false manifest_bytes={} manifest_sha256={:x}\n{}\n",
        bytes.len(),
        Sha256::digest(&bytes),
        initial_diagnostic_text(&bytes, 24 * 1024),
    );
    // The full bounded streams for every class are exported independently.
    // Keep the failing/last class tails closest to the rendered failure.
    let rows = manifest["classes"]
        .as_array()
        .ok_or("preparation classes unavailable")?;
    if let Some((position, row)) = rows.iter().enumerate().next_back() {
        for stream in ["stdout", "stderr"] {
            match initial_preparation_stream_bytes(root, packet, index, row, position, stream) {
                Ok((path, bytes)) => {
                    let tail = &bytes[bytes.len().saturating_sub(8 * 1024)..];
                    context.push_str(&format!(
                        "preparation_stream path={path} file_bytes={} sha256={:x} tail_bytes={} truncated={}\n{}\n",
                        bytes.len(),
                        Sha256::digest(&bytes),
                        tail.len(),
                        bytes.len() > tail.len(),
                        initial_diagnostic_text(tail, 8 * 1024),
                    ));
                }
                Err(error) => context.push_str(&format!(
                    "preparation_stream state=unavailable class={} stream={stream} reason={error}\n",
                    position + 1,
                )),
            }
        }
    }
    Ok(Some((reason.to_string(), context)))
}

fn write_initial_preparation_copy(
    directory: &Path,
    relative: &str,
    bytes: &[u8],
    members: &mut Vec<Value>,
) -> Result<(), String> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join(relative))
        .map_err(|_error| "exclusive preparation copy claim failed")?;
    file.write_all(bytes)
        .map_err(|_error| "preparation copy write failed")?;
    file.flush()
        .map_err(|_error| "preparation copy flush failed")?;
    members.push(
        json!({"path":relative,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes))}),
    );
    Ok(())
}

fn export_initial_preparation_evidence(
    repo: &Path,
    root: &Path,
    packet: &str,
    phase: &str,
    nonce: u128,
) -> Result<String, String> {
    if !matches!(phase, "initial" | "public") {
        return Err("unknown preparation export phase".into());
    }
    let report_bytes = initial_diagnostic_bytes(
        root,
        &format!("{packet}/resolved-tree-validation.json"),
        128 * 1024,
    )?;
    let index_bytes =
        initial_diagnostic_bytes(root, &format!("{packet}/packet-index.json"), 128 * 1024)?;
    let report: Value =
        serde_json::from_slice(&report_bytes).map_err(|_error| "validation report malformed")?;
    let index: Value =
        serde_json::from_slice(&index_bytes).map_err(|_error| "packet index malformed")?;
    if report["schema"] != "ripr.source_promotion_resolved_tree_validation.v1"
        || report["tool_version"] != env!("CARGO_PKG_VERSION")
        || !matches!(report["status"].as_str(), Some("rejected" | "validated"))
        || index["schema"] != "ripr.source_promotion_resolved_tree_packet.v1"
        || index["status"] != report["status"]
        || index["complete"] != true
    {
        return Err("preparation export packet schema/status mismatch".into());
    }
    let report_member = initial_diagnostic_member(&index, "resolved-tree-validation.json")?;
    if report_member["bytes"].as_u64() != Some(report_bytes.len() as u64)
        || report_member["sha256"].as_str()
            != Some(format!("{:x}", Sha256::digest(&report_bytes)).as_str())
    {
        return Err("validation report/index identity mismatch".into());
    }
    let (manifest_bytes, manifest) = initial_preparation_manifest(root, packet, &report, &index)?;
    let name = format!(
        "source-promotion-build-preparation-{phase}-{}-{nonce}",
        std::process::id()
    );
    let directory = owned_initial_report_directory(repo)?.join(&name);
    fs::create_dir(&directory)
        .map_err(|_error| "exclusive preparation export directory claim failed")?;
    fs::create_dir(directory.join("build-preparation"))
        .map_err(|_error| "preparation export stream directory claim failed")?;
    let mut members = Vec::new();
    for (relative, bytes) in [
        ("resolved-tree-validation.json", &report_bytes),
        ("packet-index.json", &index_bytes),
        ("build-preparation.json", &manifest_bytes),
    ] {
        write_initial_preparation_copy(&directory, relative, bytes, &mut members)?;
    }
    let mut missing = Vec::new();
    for (position, row) in manifest["classes"]
        .as_array()
        .ok_or("preparation classes unavailable")?
        .iter()
        .enumerate()
    {
        for stream in ["stdout", "stderr"] {
            let copied =
                initial_preparation_stream_bytes(root, packet, &index, row, position, stream)
                    .and_then(|(relative, bytes)| {
                        write_initial_preparation_copy(&directory, &relative, &bytes, &mut members)
                    });
            if let Err(reason) = copied {
                missing.push(json!({"class":position+1,"stream":stream,"reason":reason}));
            }
        }
    }
    // Maximum: two 128KiB inputs, one 64KiB manifest, ten 2MiB streams,
    // and this <=64KiB receipt. A bad/missing stream does not erase the others.
    let receipt = json!({
        "schema":"ripr.materialized_build_preparation_diagnostic_copy.v1",
        "diagnostic_only":true,"policy_acceptance_credit":false,
        "phase":phase,"packet":packet,"source_parent":report["source_parent"],
        "reviewed_tree":report["reviewed_tree"],"disposable_commit":manifest["disposable_commit"],
        "complete_copy":missing.is_empty(),"missing":missing,"files":members,
        "manifest_bytes_limit":64*1024,"stream_bytes_limit":2*1024*1024,
        "classes_limit":5,"total_retained_bytes_limit":20*1024*1024+384*1024,
        "non_claim":"Copy completeness is not validation, build success, policy acceptance or native qualification. Storage remains observed known paths, not a hard quota.",
    });
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|_error| "preparation copy receipt serialization failed")?;
    if bytes.len() > 64 * 1024 {
        return Err("preparation copy receipt exceeds retention bound".into());
    }
    write_initial_preparation_copy(&directory, "diagnostic-copy.json", &bytes, &mut Vec::new())?;
    Ok(format!(
        "path=target/ripr/reports/{name}/diagnostic-copy.json bytes={} sha256={:x} complete_copy={}",
        bytes.len(),
        Sha256::digest(&bytes),
        receipt["complete_copy"],
    ))
}

fn retain_initial_preparation_evidence(
    repo: &Path,
    root: &Path,
    packet: &str,
    phase: &str,
    nonce: u128,
) {
    match export_initial_preparation_evidence(repo, root, packet, phase, nonce) {
        Ok(identity) => println!("retained_preparation_evidence {identity}"),
        Err(reason) => {
            eprintln!("preparation evidence retention unavailable phase={phase}: {reason}")
        }
    }
}

include!("preparation_evidence_control.rs");
