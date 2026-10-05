use super::*;
use tokio::io::AsyncReadExt;

#[test]
fn readable_giant_id_cannot_escape_the_output_cap() -> Result<(), String> {
    let message = ServerJsonRpcMessage::error(
        ErrorData::internal_error("failure", None),
        Some(rmcp::model::RequestId::String(
            "\n".repeat(70 * 1024).into(),
        )),
    );
    let error = encode_message(&message)
        .err()
        .ok_or_else(|| "giant correlated ID unexpectedly fit".to_string())?;
    if error.to_string() != "MCP output limit" {
        return Err("giant ID refusal lost bounded reason".into());
    }
    Ok(())
}

#[test]
fn exactly_max_response_bytes_passes_and_one_more_falls_back() -> Result<(), String> {
    // The writer backstop admits exactly-MAX frames, matching the semantic
    // layers' `> MAX` typed refusal (#5254 item 5). The overflow case pads
    // the message text, not the id: a giant id falls back to a hard error
    // by design, which is a different contract.
    let message_with_pad = |pad_len: usize| {
        ServerJsonRpcMessage::error(
            ErrorData::internal_error("x".repeat(pad_len), None),
            Some(rmcp::model::RequestId::String("pad".into())),
        )
    };
    let base_len = serde_json::to_vec(&message_with_pad(0))
        .map_err(|error| error.to_string())?
        .len();
    let pad_len = super::super::MAX_RESPONSE_BYTES
        .checked_sub(base_len)
        .ok_or_else(|| "fixture exceeds the response bound".to_string())?;
    let exact = encode_message(&message_with_pad(pad_len)).map_err(|error| error.to_string())?;
    // The newline is appended after the cap check, so the frame is MAX + 1.
    if exact.len() != super::super::MAX_RESPONSE_BYTES + 1 {
        return Err(format!(
            "exactly-MAX frame has wrong length: {}",
            exact.len()
        ));
    }
    let exact_text = String::from_utf8(exact).map_err(|error| error.to_string())?;
    if !exact_text.contains("\"code\":-32603")
        || exact_text.contains("MCP response exceeds the configured byte limit")
    {
        return Err("exactly-MAX frame was replaced by the fallback".into());
    }
    let over = encode_message(&message_with_pad(pad_len + 1)).map_err(|error| error.to_string())?;
    let over_text = String::from_utf8(over).map_err(|error| error.to_string())?;
    if !over_text.contains("MCP response exceeds the configured byte limit") {
        return Err(format!("MAX+1 frame missed the fallback: {over_text}"));
    }
    Ok(())
}

#[tokio::test]
async fn cancelled_partial_write_resumes_without_repeating_bytes() -> Result<(), String> {
    use std::{
        future::{Future, poll_fn},
        task::Poll,
    };
    let message = ServerJsonRpcMessage::error(
        ErrorData::internal_error("partial write witness", None),
        Some(rmcp::model::RequestId::Number(9)),
    );
    let expected = encode_message(&message).map_err(|error| error.to_string())?;
    let (sink, mut source) = tokio::io::duplex(2);
    let mut writer = FrameWriter::new(sink);
    {
        let mut sending = Box::pin(writer.send(&message));
        poll_fn(|context| match sending.as_mut().poll(context) {
            Poll::Pending => Poll::Ready(Ok(())),
            Poll::Ready(_) => Poll::Ready(Err("tiny sink must pause a partial frame".to_string())),
        })
        .await?;
    }
    if writer.cursor == 0 {
        return Err("control never wrote its prefix before cancellation".into());
    }
    let collector = tokio::spawn(async move {
        let mut bytes = Vec::new();
        source
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| error.to_string())?;
        Ok::<_, String>(bytes)
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), writer.close())
        .await
        .map_err(|_error| "resumed writer timed out".to_string())?
        .map_err(|error| error.to_string())?;
    let actual = collector
        .await
        .map_err(|_error| "output collector failed".to_string())??;
    if actual != expected {
        return Err("cancelled send duplicated or truncated frame bytes".into());
    }
    Ok(())
}
