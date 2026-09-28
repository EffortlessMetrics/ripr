#[derive(Debug, PartialEq)]
pub struct OpError {
    pub code: i32,
    pub message: String,
}

pub fn cancelled_error() -> OpError {
    OpError {
        code: -32800,
        message: "operation cancelled".to_string(),
    }
}

pub fn run_slow(cancelled: bool) -> Result<&'static str, OpError> {
    if cancelled {
        return Err(OpError { code: -32800, message: "operation cancelled".to_string() });
    }
    Ok("completed")
}
