use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

pub(super) enum FrameRead {
    Eof,
    Empty,
    Oversized,
    Frame(Vec<u8>),
}

/// Partial bytes and discard state live outside the cancelled receive future.
pub(super) struct FrameReader<R> {
    reader: BufReader<R>,
    frame: Vec<u8>,
    oversized: bool,
}
impl<R: AsyncRead + Unpin> FrameReader<R> {
    pub(super) fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            frame: Vec::new(),
            oversized: false,
        }
    }
    pub(super) async fn read_frame(&mut self) -> Result<FrameRead, std::io::Error> {
        loop {
            let available = self.reader.fill_buf().await?;
            let eof = available.is_empty();
            let newline = available.iter().position(|byte| *byte == b'\n');
            let content = newline.map_or(available, |end| available.get(..end).unwrap_or_default());
            if !self.oversized {
                match self.frame.len().checked_add(content.len()) {
                    Some(len) if len <= super::MAX_MESSAGE_BYTES => {
                        self.frame.extend_from_slice(content)
                    }
                    _ => {
                        self.oversized = true;
                        self.frame.clear();
                    }
                }
            }
            let consumed = match newline {
                Some(end) => end.saturating_add(1),
                None => available.len(),
            };
            self.reader.consume(consumed);
            if eof || newline.is_some() {
                if std::mem::take(&mut self.oversized) {
                    return Ok(FrameRead::Oversized);
                }
                if self.frame.last() == Some(&b'\r') {
                    let _removed = self.frame.pop();
                }
                let frame = std::mem::take(&mut self.frame);
                return Ok(if frame.is_empty() {
                    if eof {
                        FrameRead::Eof
                    } else {
                        FrameRead::Empty
                    }
                } else {
                    FrameRead::Frame(frame)
                });
            }
        }
    }
}
