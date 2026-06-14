#[derive(Debug, PartialEq, Eq)]
pub enum CalcError {
    Negative,
    TooLarge,
}

pub fn compute(x: i32) -> Result<i32, CalcError> {
    if x < 0 {
        return Err(CalcError::Negative);
    }
    if x > 1000 {
        return Err(CalcError::TooLarge);
    }
    Ok(x * 2)
}
