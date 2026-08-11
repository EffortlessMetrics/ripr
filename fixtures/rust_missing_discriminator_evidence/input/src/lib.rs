#[derive(Debug, PartialEq)]
pub enum PricingError {
    Other,
    Boundary,
}

pub fn classify_boundary(value: i32) -> Result<(), PricingError> {
    if value == 100 {
        Err(PricingError::Boundary)
    } else {
        Err(PricingError::Other)
    }
}
