#[derive(Debug, PartialEq, Eq)]
pub enum CalcError {
    Negative,
    TooLarge,
}

impl std::fmt::Display for CalcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CalcError::Negative => write!(f, "negative input error"),
            CalcError::TooLarge => write!(f, "too large input error"),
        }
    }
}

pub fn compute(x: i32) -> Result<i32, CalcError> {
    if x < 0 {
        return Err(CalcError::Negative);
    }
    Ok(x * 2)
}
