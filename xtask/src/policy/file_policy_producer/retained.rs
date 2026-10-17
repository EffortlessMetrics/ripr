fn producer_summary(report: &Report, bytes: &[u8]) -> Value {
    serde_json::json!({"schema": SCHEMA, "mode": report.mode, "subject_tree": report.subject_tree,
        "source_parent": report.source_parent, "checker_sha256": report.checker_sha256,
        "producer_root": report.inputs.root, "subject_head": report.subject_inputs.head,
        "references_sha256": report.references_sha256, "reference_count": report.references.len(),
        "path": "commands/file-policy-test-producer.json", "sha256": sha(bytes), "complete": true})
}

pub(crate) fn verify_retained(
    summary: &Value,
    bytes: &[u8],
    mut read_indexed: impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    let report: Report = serde_json::from_slice(bytes)
        .map_err(|error| format!("invalid retained Cargo producer: {error}"))?;
    let commands = report
        .references
        .iter()
        .map(|row| (row.line, row.command.clone()))
        .collect::<Vec<_>>();
    if report.schema != SCHEMA
        || !report.complete
        || report.failure_reason.is_some()
        || report.failed_invocation.is_some()
        || report.references.is_empty()
        || report.references_sha256 != json_sha(&commands)?
        || report.subject_tree != report.subject_inputs.tree
        || match report.mode.as_str() {
            "exact_source_parent_checkout" => {
                report.inputs.head != report.source_parent
                    || report.inputs.tree != report.subject_tree
            }
            "reviewed_tree_worktree" => report.inputs != report.subject_inputs,
            _ => true,
        }
        || *summary != producer_summary(&report, bytes)
    {
        return Err("retained Cargo producer differs from its verified summary".to_string());
    }
    for (index, reference) in report.references.iter().enumerate() {
        if reference.line == 0
            || !crate::is_cargo_test_command(&reference.command)
            || reference.actual_args != actual_args(&reference.command)
            || reference.exit_code != 0
            || reference.timed_out
            || reference.stdout_truncated
            || reference.stderr_truncated
            || reference.selected_tests == 0
            || reference.artifacts.is_empty()
        {
            return Err(
                "retained Cargo producer has an incomplete or unsuccessful reference".to_string(),
            );
        }
        let mut stdout = None;
        for (stream, name, digest) in [
            ("stdout", &reference.stdout_path, &reference.stdout_sha256),
            ("stderr", &reference.stderr_path, &reference.stderr_sha256),
        ] {
            if *name != format!("file-policy-test-{:02}.{stream}.log", index + 1) {
                return Err("retained producer has a noncanonical log path".to_string());
            }
            let bytes = read_indexed(&format!("commands/{name}"))?;
            if bytes.len() > STREAM_LIMIT || sha(&bytes) != *digest {
                return Err("retained producer log differs from indexed bytes".to_string());
            }
            if stream == "stdout" {
                stdout = Some(String::from_utf8(bytes).map_err(|error| error.to_string())?);
            }
        }
        let stdout = stdout.ok_or("missing retained stdout")?;
        let (graph, records) = artifact_records(
            &stdout,
            &report.metadata,
            &report.inputs.root,
            &reference.command,
            &report.test_profile,
            None,
        )?;
        if reference.selected_tests
            != stdout
                .lines()
                .filter(|line| line.ends_with(": test"))
                .count()
            || graph != reference.compiler_graph_sha256
            || records.len() != reference.artifacts.len()
        {
            return Err(
                "retained producer listing or graph differs from its reference".to_string(),
            );
        }
        for (record, artifact) in records.iter().zip(&reference.artifacts) {
            let valid_sha = |value: &str| {
                value.len() == 64
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            };
            if artifact.compiler_record != *record
                || record["executable"].as_str().map(Path::new)
                    != Some(artifact.executable.as_path())
                || !artifact.executable.starts_with(Path::new(
                    report.metadata["target_directory"]
                        .as_str()
                        .ok_or("retained metadata lacks target directory")?,
                ))
                || artifact.depfile != artifact.executable.with_extension("d")
                || !valid_sha(&artifact.executable_sha256)
                || !valid_sha(&artifact.depfile_sha256)
            {
                return Err("retained producer artifact differs from Cargo records".to_string());
            }
        }
    }
    Ok(())
}
