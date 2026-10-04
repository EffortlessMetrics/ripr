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
