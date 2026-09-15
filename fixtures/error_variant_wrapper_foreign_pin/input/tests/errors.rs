use wrapper_foreign_pin_fixture::{OtherError, try_parse_summary, unrelated_check};

#[test]
fn try_parse_summary_with_foreign_pin() -> Result<(), Box<dyn std::error::Error>> {
    let result = try_parse_summary("@bad;");
    let other = unrelated_check("@bad;");
    if !matches!(result, Err(_)) {
        return Err("callee should fail closed".into());
    }
    if !matches!(other, Err(OtherError::MalformedSource)) {
        return Err("foreign pin should observe OtherError::MalformedSource".into());
    }
    Ok(())
}
