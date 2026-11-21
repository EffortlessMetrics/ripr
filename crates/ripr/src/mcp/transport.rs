#[cfg(test)]
use super::MAX_MESSAGE_BYTES;
use super::protocol;
use super::server::{McpServer, bounded_error_response};
use crate::workspace_status::WorkspaceStatus;
use serde_json::{Value, json};
use std::path::PathBuf;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

pub(super) async fn serve_stdio(explicit_root: Option<PathBuf>) -> Result<(), String> {
    let status = WorkspaceStatus::resolve(explicit_root);
    serve(tokio::io::stdin(), tokio::io::stdout(), status).await
}

async fn serve<R, W>(reader: R, mut writer: W, status: WorkspaceStatus) -> Result<(), String>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut reader = BufReader::new(reader);
    let mut server = McpServer::new(status);
    loop {
        match read_frame(&mut reader).await? {
            FrameRead::Eof => return Ok(()),
            FrameRead::Empty => {}
            FrameRead::Oversized => {
                let response = bounded_error_response(
                    protocol::ERROR_INVALID_REQUEST,
                    "MCP message exceeds the configured byte limit",
                    Some(json!({ "maxMessageBytes": super::MAX_MESSAGE_BYTES })),
                );
                write_response(&mut writer, &response).await?;
            }
            FrameRead::Frame(frame) => {
                if let Some(response) = server.handle_frame(&frame) {
                    write_response(&mut writer, &response).await?;
                }
            }
        }
    }
}

async fn write_response<W>(writer: &mut W, response: &Value) -> Result<(), String>
where
    W: AsyncWrite + Unpin,
{
    let encoded =
        serde_json::to_vec(response).map_err(|error| format!("serialize MCP response: {error}"))?;
    let encoded = if encoded.len() > super::MAX_RESPONSE_BYTES {
        let fallback = bounded_error_response(
            protocol::ERROR_INTERNAL,
            "MCP response exceeds the configured byte limit",
            Some(json!({ "maxResponseBytes": super::MAX_RESPONSE_BYTES })),
        );
        serde_json::to_vec(&fallback)
            .map_err(|error| format!("serialize bounded MCP response: {error}"))?
    } else {
        encoded
    };
    writer
        .write_all(&encoded)
        .await
        .map_err(|error| format!("write MCP response: {error}"))?;
    writer
        .write_all(b"\n")
        .await
        .map_err(|error| format!("write MCP response delimiter: {error}"))?;
    writer
        .flush()
        .await
        .map_err(|error| format!("flush MCP response: {error}"))
}

enum FrameRead {
    Eof,
    Empty,
    Oversized,
    Frame(Vec<u8>),
}

async fn read_frame<R>(reader: &mut R) -> Result<FrameRead, String>
where
    R: AsyncBufRead + Unpin,
{
    let mut frame = Vec::new();
    let mut oversized = false;
    loop {
        let (consumed, saw_newline, saw_eof) = {
            let available = reader
                .fill_buf()
                .await
                .map_err(|error| format!("read MCP request: {error}"))?;
            if available.is_empty() {
                (0, false, true)
            } else if let Some(newline) = available.iter().position(|byte| *byte == b'\n') {
                append_bounded(&mut frame, &available[..newline], &mut oversized);
                (newline + 1, true, false)
            } else {
                append_bounded(&mut frame, available, &mut oversized);
                (available.len(), false, false)
            }
        };

        if saw_eof {
            if oversized {
                return Ok(FrameRead::Oversized);
            }
            if frame.is_empty() {
                return Ok(FrameRead::Eof);
            }
            trim_carriage_return(&mut frame);
            return Ok(FrameRead::Frame(frame));
        }
        reader.consume(consumed);
        if saw_newline {
            if oversized {
                return Ok(FrameRead::Oversized);
            }
            trim_carriage_return(&mut frame);
            return if frame.is_empty() {
                Ok(FrameRead::Empty)
            } else {
                Ok(FrameRead::Frame(frame))
            };
        }
    }
}

fn append_bounded(frame: &mut Vec<u8>, bytes: &[u8], oversized: &mut bool) {
    if *oversized {
        return;
    }
    let Some(next_len) = frame.len().checked_add(bytes.len()) else {
        *oversized = true;
        frame.clear();
        return;
    };
    if next_len > super::MAX_MESSAGE_BYTES {
        *oversized = true;
        frame.clear();
        return;
    }
    frame.extend_from_slice(bytes);
}

fn trim_carriage_return(frame: &mut Vec<u8>) {
    if frame.last() == Some(&b'\r') {
        let _removed = frame.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn framing_accepts_coalesced_lines_and_crlf() -> Result<(), String> {
        let input = b"{\"one\":1}\r\n{\"two\":2}\n";
        let mut reader = BufReader::new(&input[..]);
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
        let mut reader = BufReader::new(reader);
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
    async fn oversized_frame_is_discarded_without_allocating_past_the_cap() -> Result<(), String> {
        let mut input = vec![b'x'; super::MAX_MESSAGE_BYTES + 1];
        input.push(b'\n');
        input.extend_from_slice(b"{}\n");
        let mut reader = BufReader::new(input.as_slice());
        if !matches!(read_frame(&mut reader).await?, FrameRead::Oversized) {
            return Err("oversized frame must fail closed".to_string());
        }
        match read_frame(&mut reader).await? {
            FrameRead::Frame(value) if value.as_slice() == b"{}" => Ok(()),
            _ => Err("reader did not recover after oversized frame".to_string()),
        }
    }
}
