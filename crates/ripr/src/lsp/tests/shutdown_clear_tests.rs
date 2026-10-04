use super::*;

const SHUTDOWN_CLEAR_DIRTY_SOURCE: &str =
    "pub fn gate_state(flag: bool) -> bool {\n    if flag { true } else { false }\n}\n";

/// Ceiling for one shutdown-clear exchange. Analysis of the tiny scope
/// fixture takes seconds; the budget only bounds a wedged server so the
/// test fails instead of hanging.
const SHUTDOWN_CLEAR_EXCHANGE_BUDGET: Duration = Duration::from_mins(3);

/// How long the mid-publication test waits for a `shutdown` response while
/// the refresh is barrier-paused. A serialized shutdown cannot answer until
/// the barrier releases, so any response inside this window proves the
/// clear ran concurrently with publication instead of behind the
/// transition guard.
const SHUTDOWN_BLOCKED_POLL_WINDOW: Duration = Duration::from_secs(2);

fn shutdown_clear_init_options() -> serde_json::Value {
    serde_json::json!({
        "baseRef": "HEAD",
        "checkMode": "instant",
        "diagnosticProfile": "full",
    })
}

/// Scope fixture with one committed base plus an uncommitted `src/lib.rs`
/// change that produces real findings (mirrors the consumed-source
/// fixture). Returns the temp root (kept alive by the caller) and the
/// dirty production file.
fn write_shutdown_clear_git_fixture(name: &str) -> Result<(TempLspRoot, PathBuf), String> {
    let root = unique_lsp_test_root(name)?;
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
    let path = root.path().join("src/lib.rs");
    fs::write(&path, SHUTDOWN_CLEAR_DIRTY_SOURCE)
        .map_err(|err| format!("write dirty shutdown-clear source failed: {err}"))?;
    Ok((root, path))
}

struct ShutdownClearClient {
    reader: tokio::io::ReadHalf<tokio::io::DuplexStream>,
    writer: tokio::io::WriteHalf<tokio::io::DuplexStream>,
    server_task: tokio::task::JoinHandle<()>,
    next_id: u64,
}

/// Spawn a duplex server, running `install` against the backend before the
/// server task starts so test barriers arm before any refresh can reach
/// them. Returns the client plus whatever the installer produced.
fn spawn_shutdown_clear_server<T>(
    root: &Path,
    install: impl FnOnce(&Backend) -> Result<T, String>,
) -> Result<(ShutdownClearClient, T), String> {
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let (reader, writer) = tokio::io::split(client_io);
    let (server_read, server_write) = tokio::io::split(server_io);
    let backend_root = root.to_path_buf();
    let (service, socket) =
        LspService::new(move |client| Backend::new(client, backend_root.clone()));
    let installed = install(service.inner())?;
    let server_task = tokio::spawn(async move {
        Server::new(server_read, server_write, socket)
            .serve(service)
            .await;
    });
    Ok((
        ShutdownClearClient {
            reader,
            writer,
            server_task,
            next_id: 1,
        },
        installed,
    ))
}

impl ShutdownClearClient {
    fn request_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    async fn initialize_with_root_uri(&mut self, root_uri: &str) -> Result<(), String> {
        let id = self.request_id();
        write_lsp_message(
            &mut self.writer,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "initialize",
                "params": {
                    "processId": null,
                    "rootUri": root_uri,
                    "initializationOptions": shutdown_clear_init_options(),
                    "capabilities": {},
                }
            }),
        )
        .await?;
        let initialize = read_lsp_response(&mut self.reader, id).await?;
        if initialize.get("error").is_some() {
            return Err(format!("duplex initialize failed: {initialize}"));
        }
        write_lsp_message(
            &mut self.writer,
            serde_json::json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
        )
        .await
    }

    async fn initialize_with_folders(&mut self, folders: serde_json::Value) -> Result<(), String> {
        let id = self.request_id();
        write_lsp_message(
            &mut self.writer,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "initialize",
                "params": {
                    "processId": null,
                    "workspaceFolders": folders,
                    "initializationOptions": shutdown_clear_init_options(),
                    "capabilities": {},
                }
            }),
        )
        .await?;
        let initialize = read_lsp_response(&mut self.reader, id).await?;
        if initialize.get("error").is_some() {
            return Err(format!("duplex initialize failed: {initialize}"));
        }
        write_lsp_message(
            &mut self.writer,
            serde_json::json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
        )
        .await
    }

    async fn fire_execute_command(&mut self, command: &str) -> Result<u64, String> {
        let id = self.request_id();
        write_lsp_message(
            &mut self.writer,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "workspace/executeCommand",
                "params": {"command": command, "arguments": []}
            }),
        )
        .await?;
        Ok(id)
    }

    async fn fire_shutdown(&mut self) -> Result<u64, String> {
        let id = self.request_id();
        write_lsp_message(
            &mut self.writer,
            serde_json::json!({"jsonrpc": "2.0", "id": id, "method": "shutdown", "params": null}),
        )
        .await?;
        Ok(id)
    }

    /// Stop the server after `shutdown` was already exchanged: `exit`,
    /// close, and reap the server task.
    async fn finish_after_shutdown(&mut self) -> Result<(), String> {
        write_lsp_message(
            &mut self.writer,
            serde_json::json!({"jsonrpc": "2.0", "method": "exit", "params": null}),
        )
        .await?;
        self.writer
            .shutdown()
            .await
            .map_err(|err| format!("failed to close shutdown-clear client: {err}"))?;
        match tokio::time::timeout(Duration::from_secs(10), &mut self.server_task).await {
            Ok(join_result) => {
                join_result.map_err(|err| format!("shutdown-clear server task failed: {err}"))?;
            }
            Err(_) => {
                self.server_task.abort();
                return Err(
                    "shutdown-clear server did not stop after exit notification".to_string()
                );
            }
        }
        Ok(())
    }
}

/// A message is one of the awaited responses only when it carries no
/// `method` (excluding server-to-client requests, whose id space is
/// separate) plus a matching `id`.
fn is_awaited_response(message: &serde_json::Value, ids: &[u64]) -> bool {
    message.get("method").is_none()
        && message
            .get("id")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|id| ids.contains(&id))
}

/// Read until every id in `ids` has its response, recording every message
/// in arrival order (responses included).
async fn read_until_responses<R>(
    reader: &mut R,
    ids: &[u64],
) -> Result<Vec<serde_json::Value>, String>
where
    R: AsyncRead + Unpin,
{
    let mut messages = Vec::new();
    let mut remaining: Vec<u64> = ids.to_vec();
    while !remaining.is_empty() {
        let message = read_lsp_message(reader).await?;
        if is_awaited_response(&message, &remaining) {
            let id = message
                .get("id")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| "awaited response carried no id".to_string())?;
            remaining.retain(|pending| *pending != id);
        }
        messages.push(message);
    }
    Ok(messages)
}

/// Read for at most `window`, recording every message. Returns the
/// response for `id` when it arrives inside the window plus everything
/// observed. Used to prove a response does (or does not) arrive while a
/// barrier is held.
async fn poll_for_response<R>(
    reader: &mut R,
    id: u64,
    window: Duration,
) -> Result<(Option<serde_json::Value>, Vec<serde_json::Value>), String>
where
    R: AsyncRead + Unpin,
{
    let deadline = tokio::time::Instant::now() + window;
    let mut messages = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Ok((None, messages));
        }
        match tokio::time::timeout(remaining, read_lsp_message(reader)).await {
            Ok(Ok(message)) => {
                if is_awaited_response(&message, &[id]) {
                    messages.push(message.clone());
                    return Ok((Some(message), messages));
                }
                messages.push(message);
            }
            Ok(Err(err)) => return Err(err),
            Err(_) => return Ok((None, messages)),
        }
    }
}

fn publish_diagnostics_of(message: &serde_json::Value) -> Option<(String, usize)> {
    if message.get("method").and_then(serde_json::Value::as_str)
        != Some("textDocument/publishDiagnostics")
    {
        return None;
    }
    let uri = message
        .pointer("/params/uri")
        .and_then(serde_json::Value::as_str)?
        .to_string();
    let count = message
        .pointer("/params/diagnostics")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    Some((uri, count))
}

/// Read until the server-originated request for `method` arrives,
/// returning the request plus every skipped message (transition
/// publishes must not be dropped while waiting for the round-trip).
async fn read_request_stashing<R>(
    reader: &mut R,
    method: &str,
) -> Result<(serde_json::Value, Vec<serde_json::Value>), String>
where
    R: AsyncRead + Unpin,
{
    let mut stashed = Vec::new();
    loop {
        let message = read_lsp_message(reader).await?;
        if message.get("method").and_then(serde_json::Value::as_str) == Some(method) {
            return Ok((message, stashed));
        }
        stashed.push(message);
    }
}

/// Poll `ripr.collectWorkspaceStatus` until `predicate` holds, stashing
/// every non-response message (folder transitions publish no request,
/// so the bounded poll is the synchronization point and the stash is
/// the transition drain).
async fn poll_workspace_status_stashing(
    client: &mut ShutdownClearClient,
    description: &str,
    predicate: impl Fn(&serde_json::Value) -> bool,
) -> Result<(serde_json::Value, Vec<serde_json::Value>), String> {
    let mut stashed = Vec::new();
    let mut last = serde_json::Value::Null;
    for _ in 0..60 {
        let id = client.request_id();
        write_lsp_message(
            &mut client.writer,
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "workspace/executeCommand",
                "params": {"command": COLLECT_WORKSPACE_STATUS_COMMAND, "arguments": []}
            }),
        )
        .await?;
        let messages = read_until_responses(&mut client.reader, &[id]).await?;
        let response = messages
            .iter()
            .find(|message| is_awaited_response(message, &[id]))
            .cloned()
            .ok_or_else(|| "workspace status response missing".to_string())?;
        if response.get("error").is_some() {
            return Err(format!("workspace status failed: {response}"));
        }
        stashed.extend(
            messages
                .into_iter()
                .filter(|message| !is_awaited_response(message, &[id])),
        );
        last = response["result"]["analysis_status"].clone();
        if predicate(&last) {
            return Ok((last, stashed));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(format!(
        "workspace status never satisfied {description}; last status: {last}"
    ))
}

fn run_shutdown_clear_exchange<Fut>(failure: &str, exchange: Fut) -> Result<(), String>
where
    Fut: std::future::Future<Output = Result<(), String>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start shutdown-clear runtime: {err}"))?;
    runtime.block_on(async {
        match tokio::time::timeout(SHUTDOWN_CLEAR_EXCHANGE_BUDGET, exchange).await {
            Ok(result) => result,
            Err(_) => Err(failure.to_string()),
        }
    })
}

/// `shutdown` during an in-flight refresh publication must serialize
/// behind the shared transition guard (#5202, RIPR-SPEC-0124): the refresh
/// holds the guard while paused at the publication barrier, so shutdown
/// cannot answer until the barrier releases, and every tracked URI's
/// terminal publish is the shutdown empty — never a post-clear batch.
/// Without the guard shutdown answers mid-pause and the released refresh
/// publishes fresh batches after the empties.
#[test]
fn shutdown_during_refresh_publication_clears_last() -> Result<(), String> {
    run_shutdown_clear_exchange(
        "shutdown-during-publication exchange did not complete",
        async {
            let (temp_root, _lib) = write_shutdown_clear_git_fixture("shutdown-clear-mid-publish")?;
            let root = temp_root.path();
            let (mut client, (witness, release)) = spawn_shutdown_clear_server(root, |backend| {
                backend.install_refresh_publication_barrier_for_test()
            })?;
            let root_uri = file_uri_for_path(root)?;
            client.initialize_with_root_uri(root_uri.as_str()).await?;
            let refresh_id = client.fire_execute_command(REFRESH_COMMAND).await?;
            // The refresh owns `workspace_root_transition` and pauses before
            // its first publish; the witness proves the pause, not a race.
            let paused = tokio::time::timeout(Duration::from_mins(2), witness)
                .await
                .map_err(|_elapsed| {
                    "SETUP: refresh never reached the publication barrier".to_string()
                })?
                .map_err(|_dropped| "refresh dropped the publication barrier".to_string())?;
            let _ = paused;
            let shutdown_id = client.fire_shutdown().await?;
            let (early, paused_messages) = poll_for_response(
                &mut client.reader,
                shutdown_id,
                SHUTDOWN_BLOCKED_POLL_WINDOW,
            )
            .await?;
            if let Some(response) = early {
                return Err(format!(
                    "shutdown answered while refresh publication was paused ({response}): the terminal clear is not serialized behind the transition guard"
                ));
            }
            release
                .send(())
                .map_err(|_dropped| "refresh dropped the publication release".to_string())?;
            let mut messages = paused_messages;
            messages.extend(
                read_until_responses(&mut client.reader, &[refresh_id, shutdown_id]).await?,
            );
            let publishes: Vec<(String, usize)> =
                messages.iter().filter_map(publish_diagnostics_of).collect();
            let mut tracked: Vec<String> = publishes
                .iter()
                .filter(|(_, count)| *count > 0)
                .map(|(uri, _)| uri.clone())
                .collect();
            tracked.sort();
            tracked.dedup();
            if tracked.is_empty() {
                return Err(format!(
                    "SETUP: refresh must publish diagnostics before the shutdown race is meaningful: {publishes:?}"
                ));
            }
            let shutdown_position = messages
                .iter()
                .position(|message| is_awaited_response(message, &[shutdown_id]))
                .ok_or_else(|| "shutdown response missing from the exchange".to_string())?;
            for (index, message) in messages.iter().enumerate() {
                if index > shutdown_position
                    && let Some((uri, count)) = publish_diagnostics_of(message)
                    && count > 0
                {
                    return Err(format!(
                        "nonempty publish for {uri} arrived after the shutdown response (tracked: {tracked:?}, publishes: {publishes:?})"
                    ));
                }
            }
            let mut missing = Vec::new();
            for uri in &tracked {
                let position = publishes
                    .iter()
                    .rposition(|(published, _)| published == uri);
                let terminal_empty = position.is_some_and(|index| publishes[index].1 == 0);
                if !terminal_empty {
                    missing.push(uri.clone());
                }
            }
            if !missing.is_empty() {
                return Err(format!(
                    "every tracked URI must end empty after a shutdown race; not terminally cleared: {missing:?} (publishes: {publishes:?})"
                ));
            }
            client.finish_after_shutdown().await
        },
    )
}

/// A refresh cancelled by `shutdown` before it publishes must not roll
/// back after the terminal clear (#5202, RIPR-SPEC-0124): pausing the
/// refresh at the consumed-source barrier (before the transition guard),
/// shutting down, then releasing must keep the wire silent — the root
/// epoch never changed, so without rollback suppression the released
/// refresh republishes (empty previous over its planned URIs) after
/// shutdown answered. Any publish between the shutdown response and the
/// refresh response is the violation; the span — not a timing window —
/// is the oracle.
#[test]
fn shutdown_before_refresh_publication_suppresses_rollback() -> Result<(), String> {
    run_shutdown_clear_exchange(
        "shutdown-before-publication exchange did not complete",
        async {
            let (temp_root, _lib) = write_shutdown_clear_git_fixture("shutdown-clear-pre-publish")?;
            let root = temp_root.path();
            let config = LspAnalysisConfig {
                base_ref: Some("HEAD".to_string()),
                mode: Mode::Instant,
                diagnostic_profile: crate::config::LspDiagnosticProfile::Full,
                ..LspAnalysisConfig::default()
            };
            let (mut client, (witness, release)) = spawn_shutdown_clear_server(root, |backend| {
                backend.install_consumed_source_barrier_for_test(config)
            })?;
            let root_uri = file_uri_for_path(root)?;
            client.initialize_with_root_uri(root_uri.as_str()).await?;
            let refresh_id = client.fire_execute_command(REFRESH_COMMAND).await?;
            let (_generation, produced) = tokio::time::timeout(Duration::from_mins(2), witness)
                .await
                .map_err(|_elapsed| {
                    "SETUP: refresh never reached the consumed-source barrier".to_string()
                })?
                .map_err(|_dropped| "refresh dropped the consumed-source barrier".to_string())?;
            if produced.findings.is_empty() {
                return Err("SETUP: paused refresh computed no findings".to_string());
            }
            // Nothing committed yet, so shutdown tracks nothing and the
            // drain is empty; the rollback span below is where a
            // suppression failure would publish.
            let shutdown_id = client.fire_shutdown().await?;
            let shutdown_span = read_until_responses(&mut client.reader, &[shutdown_id]).await?;
            let shutdown_publishes: Vec<(String, usize)> = shutdown_span
                .iter()
                .filter_map(publish_diagnostics_of)
                .collect();
            if !shutdown_publishes.is_empty() {
                return Err(format!(
                    "SETUP: shutdown before any commit must publish nothing: {shutdown_publishes:?}"
                ));
            }
            release
                .send(())
                .map_err(|_dropped| "refresh dropped the barrier release".to_string())?;
            let rollback_span = read_until_responses(&mut client.reader, &[refresh_id]).await?;
            let resurrected: Vec<(String, usize)> = rollback_span
                .iter()
                .filter_map(publish_diagnostics_of)
                .collect();
            if !resurrected.is_empty() {
                return Err(format!(
                    "cancelled refresh republished after the shutdown clear: {resurrected:?}"
                ));
            }
            client.finish_after_shutdown().await
        },
    )
}

/// A root change already clears every tracked URI, so a later `shutdown`
/// must not publish them a second time (#5202, RIPR-SPEC-0124): after a
/// refresh on folder A, moving the single-folder selection to B empties
/// each of A's tracked URIs exactly once, and the shutdown drain carries
/// no publish for them at all. Both clears drain the same take-once
/// tracking, so the second clear observes nothing left to clear.
#[test]
fn shutdown_after_root_change_publishes_no_duplicates() -> Result<(), String> {
    run_shutdown_clear_exchange(
        "shutdown-after-root-change exchange did not complete",
        async {
            let (temp_a, _lib) = write_shutdown_clear_git_fixture("shutdown-clear-root-a")?;
            let root_b = unique_lsp_test_root("shutdown-clear-root-b")?;
            let root_a = temp_a.path().to_path_buf();
            let root_b_path = root_b.path().to_path_buf();
            let (mut client, ()) =
                spawn_shutdown_clear_server(&root_a, |_backend| Ok::<(), String>(()))?;
            let uri_a = file_uri_for_path(&root_a)?;
            let uri_b = file_uri_for_path(&root_b_path)?;
            client
                .initialize_with_folders(serde_json::json!([workspace_folder_json(&uri_a)]))
                .await?;
            let refresh_id = client.fire_execute_command(REFRESH_COMMAND).await?;
            let refresh_span = read_until_responses(&mut client.reader, &[refresh_id]).await?;
            let refresh_response = refresh_span
                .iter()
                .find(|message| is_awaited_response(message, &[refresh_id]))
                .ok_or_else(|| "refresh response missing".to_string())?;
            if refresh_response.get("error").is_some() {
                return Err(format!(
                    "SETUP: refresh on folder A failed: {refresh_response}"
                ));
            }
            let mut tracked: Vec<String> = refresh_span
                .iter()
                .filter_map(publish_diagnostics_of)
                .filter(|(_, count)| *count > 0)
                .map(|(uri, _)| uri)
                .collect();
            tracked.sort();
            tracked.dedup();
            if tracked.is_empty() {
                return Err("SETUP: refresh on folder A published no diagnostics".to_string());
            }
            write_lsp_message(
                &mut client.writer,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "workspace/didChangeWorkspaceFolders",
                    "params": {"event": {
                        "added": [workspace_folder_json(&uri_b)],
                        "removed": [workspace_folder_json(&uri_a)],
                    }}
                }),
            )
            .await?;
            let (folders_request, mut transition_drain) =
                read_request_stashing(&mut client.reader, "workspace/workspaceFolders").await?;
            write_lsp_message(
                &mut client.writer,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": folders_request["id"].clone(),
                    "result": [workspace_folder_json(&uri_b)]
                }),
            )
            .await?;
            let expected_b = server_path_text(&root_b_path);
            let (status, mut stashed) = poll_workspace_status_stashing(
                &mut client,
                "changed-root selection on B",
                |status| {
                    status_root_state(status) == Some("root_changed")
                        && status["effective_root"].as_str() == Some(expected_b.as_str())
                },
            )
            .await?;
            let _ = status;
            transition_drain.append(&mut stashed);
            let transition_publishes: Vec<(String, usize)> = transition_drain
                .iter()
                .filter_map(publish_diagnostics_of)
                .collect();
            let mut missing = Vec::new();
            for uri in &tracked {
                let cleared = transition_publishes
                    .iter()
                    .any(|(published, count)| published == uri && *count == 0);
                if !cleared {
                    missing.push(uri.clone());
                }
            }
            if !missing.is_empty() {
                return Err(format!(
                    "SETUP: root change must empty every tracked URI once; missing {missing:?} (transition drain: {transition_publishes:?})"
                ));
            }
            let shutdown_id = client.fire_shutdown().await?;
            let shutdown_span = read_until_responses(&mut client.reader, &[shutdown_id]).await?;
            let shutdown_publishes: Vec<(String, usize)> = shutdown_span
                .iter()
                .filter_map(publish_diagnostics_of)
                .collect();
            let duplicates: Vec<(String, usize)> = shutdown_publishes
                .into_iter()
                .filter(|(uri, _)| tracked.contains(uri))
                .collect();
            if !duplicates.is_empty() {
                return Err(format!(
                    "shutdown republished URIs the root change already cleared: {duplicates:?}"
                ));
            }
            client.finish_after_shutdown().await
        },
    )
}
