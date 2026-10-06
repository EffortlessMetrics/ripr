#[test]
fn initial_materialized_preparation_failure_evidence_is_retained() -> Result<(), String> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-preparation-evidence-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&root).map_err(|error| error.to_string())?;
    let result = (|| {
        let packet_relative = format!("{INITIAL_FIXTURE}/validation-packet");
        let fixture = root.join(INITIAL_FIXTURE);
        let packet = root.join(&packet_relative);
        fs::create_dir_all(packet.join("build-preparation")).map_err(|error| error.to_string())?;
        fs::write(
            root.join("workspace/synthetic-fixture/fixture-repository/.git/HEAD"),
            format!("{}\n", "a".repeat(40)),
        )
        .map_err(|error| error.to_string())?;
        let preflight = json!({"schema":"ripr.source_promotion_preflight.v1","source_parent":"a".repeat(40),
            "swarm_parent":"b".repeat(40),"dry_merge":{"reviewed_resolved_tree":"c".repeat(40)}});
        let preflight_bytes = serde_json::to_vec(&preflight).map_err(|error| error.to_string())?;
        let preflight_hash = format!("{:x}", Sha256::digest(&preflight_bytes));
        fs::write(fixture.join("preflight.json"), &preflight_bytes)
            .map_err(|error| error.to_string())?;
        let resolution = json!({"schema":"ripr.source_promotion_resolution.v1","source_parent":"a".repeat(40),
            "swarm_parent":"b".repeat(40),"reviewed_join_tree":"c".repeat(40),"preflight_sha256":preflight_hash});
        let resolution_bytes =
            serde_json::to_vec(&resolution).map_err(|error| error.to_string())?;
        fs::write(fixture.join("resolution.json"), &resolution_bytes)
            .map_err(|error| error.to_string())?;
        let reason = "materialized preparation deadline exceeded";
        let commands = INITIAL_REQUIRED_COMMANDS.iter().enumerate().map(|(position,name)|json!({
            "command":name,"subject_role":if *name=="check-command-catalog" {"source_parent_trusted_checker_self_health"} else {"reviewed_tree_source_governance_contract"},
            "timeout_bound_ms":180000,"state":if position<3 {"passed"} else {"not_run"},
            "exit_code":if position<3 {Some(0)} else {None},"evidence_present":position<3,
            "failure_reason":if position==3 {Some(reason)} else {None},
        })).collect::<Vec<_>>();
        let report = json!({"schema":"ripr.source_promotion_resolved_tree_validation.v1","tool_version":env!("CARGO_PKG_VERSION"),
            "status":"rejected","source_parent":"a".repeat(40),"swarm_parent":"b".repeat(40),"reviewed_tree":"c".repeat(40),
            "preflight":{"verified":true,"sha256":preflight_hash},
            "resolution_manifest":{"verified":true,"sha256":format!("{:x}",Sha256::digest(&resolution_bytes))},
            "trusted_checker":{"source_sha":"a".repeat(40),"executable_sha256":"d".repeat(64)},
            "materialization":{"created":true,"reviewed_tree":"c".repeat(40),"disposable_commit":"e".repeat(40)},
            "required_command_catalog":INITIAL_REQUIRED_COMMANDS,"commands":commands});
        let report_bytes = serde_json::to_vec(&report).map_err(|error| error.to_string())?;
        fs::write(packet.join("resolved-tree-validation.json"), &report_bytes)
            .map_err(|error| error.to_string())?;
        let mut members = vec![
            json!({"path":"resolved-tree-validation.json","bytes":report_bytes.len(),"sha256":format!("{:x}",Sha256::digest(&report_bytes))}),
        ];
        let mut classes = Vec::new();
        let mut original_streams = Vec::new();
        for position in 0..4 {
            let mut row = json!({"program":"cargo","argv":["test","-p","ripr","--test","causal_delta_fixture","--no-run"],
                "setup_only":true,"duration_ms":if position==0 {238000} else {30000},
                "timed_out":position==3,"exit_code":if position<3 {Some(0)} else {None},
                "owned_settlement":"confirmed_by_capture_owner","observed_peak_growth_after_bytes":123,
                "failure_reason":if position==3 {Some(reason)} else {None}});
            for stream in ["stdout", "stderr"] {
                let relative = format!("build-preparation/{:02}.{stream}.log", position + 1);
                let bytes = [
                    vec![b'x'; 16 * 1024],
                    format!("CLASS_{}_OWNED_{stream}", position + 1).into_bytes(),
                ]
                .concat();
                fs::write(packet.join(&relative), &bytes).map_err(|error| error.to_string())?;
                row[stream] = json!({"path":relative,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&bytes)),"truncated":false});
                members.push(row[stream].clone());
                original_streams.push((relative, bytes));
            }
            classes.push(row);
        }
        let manifest = json!({"schema":"ripr.materialized_build_preparation.v1","setup_only":true,"policy_acceptance_credit":false,
            "source_parent":"a".repeat(40),"reviewed_tree":"c".repeat(40),"disposable_commit":"e".repeat(40),
            "total_setup_seconds_limit":300,"settlement_reserve_seconds":30,"cargo_build_jobs":1,
            "stream_bytes_limit":2*1024*1024,"known_path_growth_bytes_limit":4u64*1024*1024*1024,
            "observed_peak_growth_bytes":123,"storage_scope":"known observed paths, no hard quota",
            "failure_reason":reason,"classes":classes});
        let manifest_bytes = serde_json::to_vec(&manifest).map_err(|error| error.to_string())?;
        fs::write(packet.join("build-preparation.json"), &manifest_bytes)
            .map_err(|error| error.to_string())?;
        members.push(json!({"path":"build-preparation.json","bytes":manifest_bytes.len(),"sha256":format!("{:x}",Sha256::digest(&manifest_bytes))}));
        let index = json!({"schema":"ripr.source_promotion_resolved_tree_packet.v1","status":"rejected","complete":true,"files":members});
        fs::write(
            packet.join("packet-index.json"),
            serde_json::to_vec(&index).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;

        // The real preparation failure has no failed governed command. The
        // old reader returned only "required command failure sequence mismatch".
        let context = initial_required_command_failure_output(&root, false);
        if !context.contains("state=observed preparation_failure=true")
            || !context.contains("CLASS_4_OWNED_stdout")
            || !context.contains("CLASS_4_OWNED_stderr")
            || !context.contains("238000")
            || !context.contains("owned_settlement")
            || context.contains("required command failure sequence mismatch")
        {
            return Err(
                "preparation failure was masked by governed sequence interpretation".into(),
            );
        }
        export_initial_preparation_evidence(&root, &root, &packet_relative, "initial", nonce)?;
        let directory = root.join(format!(
            "target/ripr/reports/source-promotion-build-preparation-initial-{}-{nonce}",
            std::process::id()
        ));
        if fs::read(directory.join("build-preparation.json")).map_err(|error| error.to_string())?
            != manifest_bytes
        {
            return Err("preparation manifest bytes were substituted".into());
        }
        for (relative, bytes) in &original_streams {
            if fs::read(directory.join(relative)).map_err(|error| error.to_string())? != *bytes {
                return Err("a class stream was missing, substituted or reduced to a tail".into());
            }
        }
        let copied: Value = serde_json::from_slice(
            &fs::read(directory.join("diagnostic-copy.json")).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        if copied["complete_copy"] != true
            || copied["diagnostic_only"] != true
            || copied["policy_acceptance_credit"] != false
        {
            return Err("diagnostic copy falsely claimed acceptance or lost completeness".into());
        }
        // Changed bytes cannot receive the original digest, and their refusal
        // cannot erase earlier classes or the preparation manifest.
        fs::write(
            packet.join("build-preparation/04.stderr.log"),
            b"ALTERED_MUST_NOT_BE_COPIED",
        )
        .map_err(|error| error.to_string())?;
        export_initial_preparation_evidence(&root, &root, &packet_relative, "initial", nonce + 1)?;
        let partial = root.join(format!(
            "target/ripr/reports/source-promotion-build-preparation-initial-{}-{}",
            std::process::id(),
            nonce + 1
        ));
        let copied: Value = serde_json::from_slice(
            &fs::read(partial.join("diagnostic-copy.json")).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        if copied["complete_copy"] != false
            || !copied["missing"].as_array().is_some_and(|rows| {
                rows.len() == 1 && rows[0]["class"] == 4 && rows[0]["stream"] == "stderr"
            })
            || partial.join("build-preparation/04.stderr.log").exists()
            || fs::read(partial.join("build-preparation/01.stdout.log"))
                .map_err(|error| error.to_string())?
                != original_streams[0].1
            || fs::read(partial.join("build-preparation.json"))
                .map_err(|error| error.to_string())?
                != manifest_bytes
        {
            return Err(
                "changed final-class evidence erased valid siblings or earned complete copy".into(),
            );
        }
        match export_initial_preparation_evidence(&root, &root, &packet_relative, "initial", nonce)
        {
            Err(reason) if reason == "exclusive preparation export directory claim failed" => {}
            other => {
                return Err(format!(
                    "preparation evidence lost exclusive ownership: {other:?}"
                ));
            }
        }
        Ok(())
    })();
    let cleanup = fs::remove_dir_all(root).map_err(|error| error.to_string());
    result.and(cleanup)
}
