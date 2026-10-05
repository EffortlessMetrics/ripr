use super::*;

/// #6632: the backend must hand every refresh to the one long-lived analysis
/// thread. A refresh routed back through `tokio::task::spawn_blocking` still
/// produces diagnostics, but lands on whichever pool thread is free and
/// brings back per-arena RSS growth, so only the thread identity tells.
#[test]
fn every_backend_refresh_runs_on_the_named_analysis_thread() -> Result<(), String> {
    let root = unique_lsp_test_root("analysis-thread-routing")?;
    write_lsp_scope_fixture(root.path())?;
    run_lsp_scope_git(root.path(), &["init"])?;
    run_lsp_scope_git(
        root.path(),
        &["config", "user.email", "ripr@example.invalid"],
    )?;
    run_lsp_scope_git(root.path(), &["config", "user.name", "RIPR Test"])?;
    run_lsp_scope_git(
        root.path(),
        &["add", "Cargo.toml", "src/lib.rs", "tests/end_to_end.rs"],
    )?;
    run_lsp_scope_git(root.path(), &["commit", "-m", "base"])?;
    fs::write(
        root.path().join("src/lib.rs"),
        "pub fn gate_state(flag: bool) -> bool { !flag }\n",
    )
    .map_err(|err| format!("write changed source failed: {err}"))?;

    work_done_progress_runtime()?.block_on(async {
        let (service, socket) =
            LspService::new(|client| Backend::new(client, root.path().to_path_buf()));
        // Direct-backend pattern: client output fails fast instead of
        // blocking; the claim is where the refresh runs, not delivery.
        drop(socket);
        let backend = service.inner();
        backend.initialize_test_workspace_root();
        let analysis_thread = backend.refresh_scheduler_for_test().analysis_thread();
        if !analysis_thread.job_threads_for_test().is_empty() {
            return Err("SETUP: analysis thread ran work before any refresh".to_string());
        }
        let mut snapshot_ids = Vec::new();
        for attempt in ["first", "second"] {
            tokio::time::timeout(
                Duration::from_mins(2),
                backend.refresh_diagnostics(RefreshScope::Full, RefreshReason::ExplicitRefresh),
            )
            .await
            .map_err(|_elapsed| format!("{attempt} refresh exceeded test deadline"))?;
            // Each refresh must commit its own snapshot, so a second refresh
            // that failed after its job started cannot pass on the first.
            let snapshot_id = backend
                .latest_analysis_snapshot()
                .and_then(|snapshot| snapshot.refresh.snapshot_id)
                .ok_or_else(|| format!("SETUP: {attempt} refresh committed no snapshot"))?;
            if snapshot_ids.contains(&snapshot_id) {
                return Err(format!(
                    "SETUP: {attempt} refresh committed no new snapshot ({snapshot_id})"
                ));
            }
            snapshot_ids.push(snapshot_id);
        }
        let threads = analysis_thread.job_threads_for_test();
        let names: Vec<_> = threads.iter().map(|(_, name)| name.as_deref()).collect();
        if names != [Some("ripr-lsp-analysis"); 2] {
            return Err(format!(
                "refreshes must run on the analysis thread; ran on {threads:?}"
            ));
        }
        // Same name is not enough: a fresh thread per refresh would bring
        // back one malloc arena per thread.
        if threads[0].0 != threads[1].0 {
            return Err(format!(
                "refreshes must reuse one long-lived thread; ran on {threads:?}"
            ));
        }
        Ok(())
    })
}
