use super::*;
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;

async fn read_frame<R: AsyncRead + Unpin>(
    reader: &mut FrameReader<R>,
) -> Result<FrameRead, String> {
    reader.read_frame().await.map_err(|error| error.to_string())
}

#[tokio::test]
async fn empty_or_syntax_invalid_eof_before_initialize_is_normal() -> Result<(), String> {
    for input in [b"".as_slice(), b"{unfinished".as_slice()] {
        serve(
            input,
            Vec::<u8>::new(),
            WorkspaceStatus::resolve_with_root(None).0,
            None,
        )
        .await?;
    }
    Ok(())
}

#[tokio::test]
async fn fatal_output_limit_wakes_receive_while_input_remains_open() -> Result<(), String> {
    use std::{
        future::{Future, poll_fn},
        task::Poll,
    };
    let (held_input, reader) = tokio::io::duplex(64);
    let mut transport = BoundedTransport {
        reader: FrameReader::new(reader),
        writer: Arc::new(Mutex::new(FrameWriter::new(Vec::<u8>::new()))),
        failure: Arc::new(TransportFailure::default()),
        admission: Arc::new(Admission::default()),
        pending_protocol_error: None,
        writer_needs_drain: false,
    };
    let sending = transport.send(ServerJsonRpcMessage::error(
        ErrorData::internal_error("bounded refusal", None),
        Some(rmcp::model::RequestId::String(
            "\n".repeat(70 * 1024).into(),
        )),
    ));
    let mut receiving = Box::pin(transport.receive());
    poll_fn(|context| match receiving.as_mut().poll(context) {
        Poll::Pending => Poll::Ready(Ok(())),
        Poll::Ready(_) => Poll::Ready(Err("open input did not suspend receive".to_string())),
    })
    .await?;
    let failure = sending
        .await
        .err()
        .ok_or_else(|| "giant correlated response unexpectedly fit".to_string())?;
    if super::super::writer::failure_reason(&failure) != "MCP output limit" {
        return Err("actual output refusal lost its fixed failure classification".into());
    }
    if tokio::time::timeout(std::time::Duration::from_secs(5), receiving)
        .await
        .map_err(|_error| "fatal send did not wake open-input receive".to_string())?
        .is_some()
    {
        return Err("fatal send did not terminate receive".into());
    }
    drop(held_input);
    Ok(())
}

#[tokio::test]
async fn sdk_syntax_ignore_and_typed_shape_error_recover_next_request() -> Result<(), String> {
    let input = b"{not json}\ntrue\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"control\",\"version\":\"1\"}}}\n";
    let writer = Arc::new(Mutex::new(FrameWriter::new(Vec::<u8>::new())));
    let mut transport = BoundedTransport {
        reader: FrameReader::new(input.as_slice()),
        writer: writer.clone(),
        failure: Arc::new(TransportFailure::default()),
        admission: Arc::new(Admission::default()),
        pending_protocol_error: None,
        writer_needs_drain: false,
    };
    let message = transport
        .receive()
        .await
        .ok_or_else(|| "typed recovery lost valid request".to_string())?;
    if !matches!(message, ClientJsonRpcMessage::Request(_)) {
        return Err("typed recovery did not return the request".into());
    }
    let guard = writer.lock().await;
    let frames: Vec<_> = guard
        .output()
        .split(|byte| *byte == b'\n')
        .filter(|frame| !frame.is_empty())
        .collect();
    if frames.len() != 1 {
        return Err("syntax must be ignored; exactly one typed-shape error expected".into());
    }
    let frame = frames
        .first()
        .ok_or_else(|| "typed-shape error missing".to_string())?;
    let error: Value = serde_json::from_slice(frame).map_err(|error| error.to_string())?;
    if error.get("id").is_some()
        || error.pointer("/error/code").and_then(Value::as_i64) != Some(-32600)
    {
        return Err("SDK typed-shape error ID/code differs".into());
    }
    Ok(())
}

#[tokio::test]
async fn framing_delivers_valid_undelimited_eof_and_then_closes() -> Result<(), String> {
    let mut reader = FrameReader::new(b"{\"eof\":true}".as_slice());
    match read_frame(&mut reader).await? {
        FrameRead::Frame(bytes) if bytes == b"{\"eof\":true}" => {}
        _ => return Err("undelimited EOF lost valid frame".into()),
    }
    if !matches!(read_frame(&mut reader).await?, FrameRead::Eof) {
        return Err("EOF did not remain closed".into());
    }
    Ok(())
}

#[tokio::test]
async fn consumed_protocol_error_survives_receive_cancellation_behind_busy_writer()
-> Result<(), String> {
    use std::{
        future::{Future, poll_fn},
        task::Poll,
    };
    let mut input = vec![b'x'; super::super::MAX_MESSAGE_BYTES + 1];
    input.extend_from_slice(b"\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"control\",\"version\":\"1\"}}}\n");
    let writer = Arc::new(Mutex::new(FrameWriter::new(Vec::<u8>::new())));
    let held = writer.lock().await;
    let mut transport = BoundedTransport {
        reader: FrameReader::new(input.as_slice()),
        writer: writer.clone(),
        failure: Arc::new(TransportFailure::default()),
        admission: Arc::new(Admission::default()),
        pending_protocol_error: None,
        writer_needs_drain: false,
    };
    {
        let mut receiving = Box::pin(transport.receive());
        poll_fn(|context| match receiving.as_mut().poll(context) {
            Poll::Pending => Poll::Ready(Ok(())),
            Poll::Ready(_) => Poll::Ready(Err(
                "busy writer must pause the consumed-frame error".to_string()
            )),
        })
        .await?;
    }
    if transport.pending_protocol_error.is_none() {
        return Err("consumed invalid frame lost its pending error".into());
    }
    drop(held);
    let message = tokio::time::timeout(std::time::Duration::from_secs(5), transport.receive())
        .await
        .map_err(|_error| "protocol error resume timed out".to_string())?
        .ok_or_else(|| "transport failed to recover next valid message".to_string())?;
    if !matches!(message, ClientJsonRpcMessage::Request(_)) {
        return Err("next valid request was not recovered".into());
    }
    let guard = writer.lock().await;
    let lines: Vec<_> = guard
        .output()
        .split(|byte| *byte == b'\n')
        .filter(|frame| !frame.is_empty())
        .collect();
    if lines.len() != 1 {
        return Err("consumed-frame error was missing or duplicated".into());
    }
    let frame = lines
        .first()
        .ok_or_else(|| "error frame missing".to_string())?;
    let error: Value = serde_json::from_slice(frame).map_err(|error| error.to_string())?;
    if error.pointer("/error/code").and_then(Value::as_i64) != Some(-32600) {
        return Err("recovered protocol error code differs".into());
    }
    Ok(())
}

#[tokio::test]
async fn framing_accepts_coalesced_lines_and_crlf() -> Result<(), String> {
    let input = b"{\"one\":1}\r\n{\"two\":2}\n";
    let mut reader = FrameReader::new(&input[..]);
    let first = read_frame(&mut reader).await?;
    let second = read_frame(&mut reader).await?;
    let eof = read_frame(&mut reader).await?;

    match first {
        FrameRead::Frame(value) if value.as_slice() == b"{\"one\":1}" => {}
        _ => return Err("first coalesced frame drifted".to_string()),
    }
    match second {
        FrameRead::Frame(value) if value.as_slice() == b"{\"two\":2}" => {}
        _ => return Err("second coalesced frame drifted".to_string()),
    }
    if !matches!(eof, FrameRead::Eof) {
        return Err("framing must end cleanly at EOF".to_string());
    }
    Ok(())
}

#[tokio::test]
async fn framing_reassembles_a_message_split_across_reads() -> Result<(), String> {
    let (mut writer, reader) = tokio::io::duplex(4);
    let writer_task = tokio::spawn(async move {
        writer
            .write_all(b"{\"split\":")
            .await
            .map_err(|error| error.to_string())?;
        writer
            .write_all(b"true}\n")
            .await
            .map_err(|error| error.to_string())
    });
    let mut reader = FrameReader::new(reader);
    let frame = read_frame(&mut reader).await?;
    writer_task
        .await
        .map_err(|error| format!("fragment writer task failed: {error}"))??;
    match frame {
        FrameRead::Frame(value) if value.as_slice() == b"{\"split\":true}" => Ok(()),
        _ => Err("fragmented frame was not reassembled".to_string()),
    }
}

#[tokio::test]
async fn framing_retains_a_partial_message_when_receive_is_cancelled() -> Result<(), String> {
    use std::future::{Future, poll_fn};
    use std::task::Poll;

    // SDK receive runs inside select!, so cancellation can happen after
    // consuming a prefix but before the delimiter arrives. Poll once to
    // that exact Pending boundary; no wall-clock timing or spawned actor
    // decides whether the cancellation actually reached the partial read.
    let (mut writer, reader) = tokio::io::duplex(64);
    writer
        .write_all(b"{\"split\":")
        .await
        .map_err(|error| error.to_string())?;
    let mut reader = FrameReader::new(reader);
    {
        let mut receiving = Box::pin(read_frame(&mut reader));
        poll_fn(|context| match receiving.as_mut().poll(context) {
            Poll::Pending => Poll::Ready(Ok(())),
            Poll::Ready(_) => Poll::Ready(Err(
                "partial-read control unexpectedly completed before its delimiter".to_string(),
            )),
        })
        .await?;
    }
    writer
        .write_all(b"true}\n")
        .await
        .map_err(|error| error.to_string())?;
    let completed =
        tokio::time::timeout(std::time::Duration::from_secs(5), read_frame(&mut reader))
            .await
            .map_err(|_error| {
                "cancelled receive did not complete after its delimiter".to_string()
            })??;
    match completed {
        FrameRead::Frame(value) if value.as_slice() == b"{\"split\":true}" => Ok(()),
        _ => Err("cancelled receive discarded the consumed prefix".to_string()),
    }
}

#[tokio::test]
async fn oversized_frame_is_discarded_without_allocating_past_the_cap() -> Result<(), String> {
    let mut input = vec![b'x'; super::super::MAX_MESSAGE_BYTES + 1];
    input.push(b'\n');
    input.extend_from_slice(b"{}\n");
    let mut reader = FrameReader::new(input.as_slice());
    if !matches!(read_frame(&mut reader).await?, FrameRead::Oversized) {
        return Err("oversized frame must fail closed".to_string());
    }
    match read_frame(&mut reader).await? {
        FrameRead::Frame(value) if value.as_slice() == b"{}" => Ok(()),
        _ => Err("reader did not recover after oversized frame".to_string()),
    }
}

#[tokio::test]
async fn oversized_response_fallback_keeps_the_known_request_id() -> Result<(), String> {
    let response = ServerJsonRpcMessage::error(
        ErrorData::internal_error(
            "large response",
            Some(json!({ "blob": "x".repeat(super::super::MAX_RESPONSE_BYTES + 1) })),
        ),
        Some(rmcp::model::RequestId::Number(7)),
    );
    let mut output =
        super::super::writer::encode_message(&response).map_err(|error| error.to_string())?;
    if output.last() == Some(&b'\n') {
        let _trailing_newline = output.pop();
    }
    let parsed: Value = serde_json::from_slice(&output)
        .map_err(|error| format!("fallback response is not JSON: {error}"))?;
    if parsed.get("id") != Some(&json!(7)) {
        return Err(format!(
            "over-cap fallback dropped the known request id: {parsed}"
        ));
    }
    if parsed.pointer("/error/code").and_then(Value::as_i64) != Some(-32603) {
        return Err("over-cap fallback error code drifted".to_string());
    }
    if parsed
        .pointer("/error/data/maxResponseBytes")
        .and_then(Value::as_u64)
        != Some(super::super::MAX_RESPONSE_BYTES as u64)
    {
        return Err("over-cap fallback omitted the byte limit".to_string());
    }
    Ok(())
}

#[tokio::test]
async fn oversized_response_fallback_preserves_sdk_unknown_id_omission() -> Result<(), String> {
    let response = ServerJsonRpcMessage::error(
        ErrorData::internal_error("x".repeat(super::super::MAX_RESPONSE_BYTES + 1), None),
        None,
    );
    let mut output =
        super::super::writer::encode_message(&response).map_err(|error| error.to_string())?;
    if output.last() == Some(&b'\n') {
        let _trailing_newline = output.pop();
    }
    let parsed: Value = serde_json::from_slice(&output)
        .map_err(|error| format!("fallback response is not JSON: {error}"))?;
    if parsed.get("id").is_some() {
        return Err(format!(
            "over-cap fallback must preserve SDK unknown-id omission: {parsed}"
        ));
    }
    Ok(())
}
