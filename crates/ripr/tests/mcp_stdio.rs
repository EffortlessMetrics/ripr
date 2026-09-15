use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn workspace_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "crate manifest directory has no workspace parent".to_string())
}

fn run_mcp(root: &Path, chunks: &[&[u8]]) -> Result<Output, String> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ripr"))
        .args(["mcp", "--stdio", "--root"])
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("spawn ripr mcp: {error}"))?;
    let Some(mut stdin) = child.stdin.take() else {
        return Err("spawned MCP process did not expose stdin".to_string());
    };
    // Write requests from a thread and drain both output pipes
    // concurrently: combined replies can exceed the OS pipe capacity
    // (Windows anonymous pipes are small), and a server blocked writing
    // stdout while the harness waits for exit would deadlock the test
    // (#3587 review).
    let stdin_chunks: Vec<Vec<u8>> = chunks.iter().map(|chunk| chunk.to_vec()).collect();
    let writer = std::thread::spawn(move || {
        for chunk in &stdin_chunks {
            if stdin.write_all(chunk).is_err() || stdin.flush().is_err() {
                return;
            }
        }
    });
    let mut stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| "spawned MCP process did not expose stdout".to_string())?;
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| "spawned MCP process did not expose stderr".to_string())?;
    let stdout_reader = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout_pipe, &mut buffer);
        buffer
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stderr_pipe, &mut buffer);
        buffer
    });

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut status = None;
    let mut timed_out = false;
    loop {
        match child
            .try_wait()
            .map_err(|error| format!("poll ripr mcp: {error}"))?
        {
            Some(exit) => {
                status = Some(exit);
                break;
            }
            None if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            None => {
                child
                    .kill()
                    .map_err(|error| format!("kill hung ripr mcp: {error}"))?;
                child
                    .wait()
                    .map_err(|error| format!("reap hung ripr mcp: {error}"))?;
                timed_out = true;
                break;
            }
        }
    }
    let _ = writer.join();
    let stdout = stdout_reader
        .join()
        .map_err(|_join_error| "stdout reader panicked")?;
    let stderr = stderr_reader
        .join()
        .map_err(|_join_error| "stderr reader panicked")?;
    let status = status.ok_or("ripr mcp status was not collected")?;
    if timed_out {
        return Err(format!(
            "ripr mcp did not exit after stdin EOF\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr)
        ));
    }
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn response_lines(output: &Output) -> Result<Vec<Value>, String> {
    if !output.status.success() {
        return Err(format!(
            "ripr mcp failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    if !output.stderr.is_empty() {
        return Err(format!(
            "successful MCP session contaminated stderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = std::str::from_utf8(&output.stdout)
        .map_err(|error| format!("MCP stdout is not UTF-8: {error}"))?;
    stdout
        .lines()
        .map(|line| {
            serde_json::from_str::<Value>(line)
                .map_err(|error| format!("MCP stdout line is not JSON-RPC: {error}: {line}"))
        })
        .collect()
}

fn current_meta() -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientCapabilities": {},
        "io.modelcontextprotocol/clientInfo": {
            "name": "ripr-integration-test",
            "version": "1"
        }
    })
}

fn line(value: Value) -> Result<Vec<u8>, String> {
    let mut encoded = serde_json::to_vec(&value).map_err(|error| error.to_string())?;
    encoded.push(b'\n');
    Ok(encoded)
}

#[test]
fn legacy_stdio_lifecycle_lists_and_reads_the_same_bounded_status() -> Result<(), String> {
    let root = workspace_root()?;
    let initialize = line(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "ripr-integration-test", "version": "1" }
        }
    }))?;
    let split = initialize.len() / 2;
    let remaining = [
        line(json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }))?,
        line(json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }))?,
        line(json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "resources/list",
            "params": {}
        }))?,
        line(json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "ripr_workspace_status",
                "arguments": {}
            }
        }))?,
        line(json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "resources/read",
            "params": { "uri": "ripr://workspace/status" }
        }))?,
        line(json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "ping",
            "params": {}
        }))?,
    ]
    .concat();
    let output = run_mcp(
        &root,
        &[&initialize[..split], &initialize[split..], &remaining],
    )?;
    let responses = response_lines(&output)?;
    if responses.len() != 6 {
        return Err(format!("expected 6 MCP responses, got {}", responses.len()));
    }
    if responses[0]
        .pointer("/result/protocolVersion")
        .and_then(Value::as_str)
        != Some("2025-11-25")
    {
        return Err("legacy initialize did not negotiate the requested version".to_string());
    }
    if responses[1]
        .pointer("/result/tools/0/name")
        .and_then(Value::as_str)
        != Some("ripr_workspace_status")
    {
        return Err("tools/list did not expose the status tool".to_string());
    }
    if responses[2]
        .pointer("/result/resources/0/uri")
        .and_then(Value::as_str)
        != Some("ripr://workspace/status")
    {
        return Err("resources/list did not expose the status resource".to_string());
    }
    let structured = responses[3]
        .pointer("/result/structuredContent")
        .ok_or_else(|| "tool result omitted structuredContent".to_string())?;
    if structured
        .pointer("/workspace/authority/source_edit_capability")
        .and_then(Value::as_str)
        != Some("none")
        || structured
            .pointer("/workspace/authority/verification_execution_capability")
            .and_then(Value::as_str)
            != Some("none")
        || structured
            .pointer("/workspace/authority/mutation_execution_capability")
            .and_then(Value::as_str)
            != Some("none")
        || structured
            .pointer("/workspace/authority/model_provider")
            .and_then(Value::as_str)
            != Some("none")
    {
        return Err("status authority boundary drifted".to_string());
    }
    let resource_text = responses[4]
        .pointer("/result/contents/0/text")
        .and_then(Value::as_str)
        .ok_or_else(|| "resource result omitted JSON text".to_string())?;
    let resource_status: Value = serde_json::from_str(resource_text)
        .map_err(|error| format!("resource status is not JSON: {error}"))?;
    if &resource_status != structured {
        return Err("tool and resource projected different status payloads".to_string());
    }
    let canonical = root
        .canonicalize()
        .map_err(|error| error.to_string())?
        .to_string_lossy()
        .into_owned();
    if String::from_utf8_lossy(&output.stdout).contains(&canonical) {
        return Err("MCP stdout leaked the canonical repository path".to_string());
    }
    if responses[5].get("result") != Some(&json!({})) {
        return Err("legacy ping did not return an empty result".to_string());
    }
    Ok(())
}

#[test]
fn current_discovery_requires_metadata_and_rejects_legacy_ping() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!("ripr-mcp-missing-root-{}", std::process::id()));
    if root.is_dir() {
        std::fs::remove_dir_all(&root)
            .map_err(|error| format!("remove stale test root directory: {error}"))?;
    } else if root.exists() {
        std::fs::remove_file(&root)
            .map_err(|error| format!("remove stale test root file: {error}"))?;
    }
    let request_bytes = [
        line(json!({
            "jsonrpc": "2.0",
            "id": "discover",
            "method": "server/discover",
            "params": { "_meta": current_meta() }
        }))?,
        // The rejection row proves discovery cannot pass without the
        // required `_meta` metadata (negative experiment for the gate).
        line(json!({
            "jsonrpc": "2.0",
            "id": "discover-no-meta",
            "method": "server/discover",
            "params": {}
        }))?,
        line(json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": { "requestId": "unused", "reason": "test" }
        }))?,
        line(json!({
            "jsonrpc": "2.0",
            "id": "tools",
            "method": "tools/list",
            "params": { "_meta": current_meta() }
        }))?,
        line(json!({
            "jsonrpc": "2.0",
            "id": "status",
            "method": "tools/call",
            "params": {
                "_meta": current_meta(),
                "name": "ripr_workspace_status",
                "arguments": {}
            }
        }))?,
        line(json!({
            "jsonrpc": "2.0",
            "id": "ping",
            "method": "ping",
            "params": { "_meta": current_meta() }
        }))?,
    ]
    .concat();
    let output = run_mcp(&root, &[&request_bytes])?;
    let responses = response_lines(&output)?;
    if responses.len() != 5 {
        return Err(format!(
            "cancellation notification must not emit a response; got {} lines",
            responses.len()
        ));
    }
    if responses[0]
        .pointer("/result/supportedVersions")
        .and_then(Value::as_array)
        .is_none_or(|versions| !versions.iter().any(|value| value == "2026-07-28"))
    {
        return Err("discovery omitted the current protocol version".to_string());
    }
    // The missing-metadata discovery must be rejected by protocol error,
    // never accepted (negative experiment for the _meta gate).
    if responses[1].pointer("/error/code").and_then(Value::as_i64) != Some(-32602) {
        return Err(format!(
            "discovery without _meta must be invalid-params: {}",
            responses[1]
        ));
    }
    if responses[1].pointer("/id").and_then(Value::as_str) != Some("discover-no-meta") {
        return Err(format!(
            "discovery without _meta must retain its request id: {}",
            responses[1]
        ));
    }
    if responses[2]
        .pointer("/result/resultType")
        .and_then(Value::as_str)
        != Some("complete")
    {
        return Err("current tools/list result omitted resultType".to_string());
    }
    if responses[3]
        .pointer("/result/structuredContent/workspace/workspace_state")
        .and_then(Value::as_str)
        != Some("unavailable")
        || responses[3]
            .pointer("/result/structuredContent/workspace/root/error_code")
            .and_then(Value::as_str)
            != Some("root_missing")
    {
        return Err("invalid explicit root did not fail closed in status".to_string());
    }
    let rejected_root = root.to_string_lossy();
    if String::from_utf8_lossy(&output.stdout).contains(rejected_root.as_ref()) {
        return Err("invalid explicit root leaked into MCP stdout".to_string());
    }
    if responses[4].pointer("/error/code").and_then(Value::as_i64) != Some(-32601) {
        return Err("current lifecycle ping must be method-not-found".to_string());
    }
    Ok(())
}
