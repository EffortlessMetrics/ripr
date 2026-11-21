#[derive(Debug, PartialEq, Eq)]
pub struct Quote {
    pub total: i32,
    pub discount_applied: bool,
}

pub fn price(amount: i32, discount_threshold: i32) -> Quote {
    if amount >= discount_threshold {
        Quote { total: amount - 100, discount_applied: true }
    } else {
        Quote { total: amount, discount_applied: false }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum InvoiceError {
    InvalidInput,
    InvalidCurrency,
}

pub fn validate_currency(currency: &str) -> Result<(), InvoiceError> {
    if currency == "USD" { Ok(()) } else { Err(InvoiceError::InvalidCurrency) }
}
