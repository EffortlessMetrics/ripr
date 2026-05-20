use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn generated_at_unix_ms() -> Result<String, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock before unix epoch: {err}"))?
        .as_millis();
    Ok(format!("unix_ms:{millis}"))
}
