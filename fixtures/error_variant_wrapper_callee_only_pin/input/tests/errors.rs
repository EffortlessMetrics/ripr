use wrapper_callee_only_pin_fixture::{ParseSummaryError, try_parse_summary};

#[test]
fn try_parse_summary_pins_malformed_source() -> Result<(), Box<dyn std::error::Error>> {
    let result = try_parse_summary("@bad;");
    if !matches!(result, Err(ParseSummaryError::MalformedSource)) {
        return Err("callee pin should observe ParseSummaryError::MalformedSource".into());
    }
    Ok(())
}
