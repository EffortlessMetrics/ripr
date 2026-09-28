use wrapper_wrong_receiver_pin_fixture::{ParseSummaryError, ParserB, try_parse_summary};

#[test]
fn parse_summary_callee_witness_pins_variant() -> Result<(), Box<dyn std::error::Error>> {
    let result = try_parse_summary("@bad;");
    if !matches!(result, Err(ParseSummaryError::MalformedSource)) {
        return Err("callee pin should observe ParseSummaryError::MalformedSource".into());
    }
    Ok(())
}

#[test]
fn parse_summary_other_receiver_pins_same_variant() -> Result<(), Box<dyn std::error::Error>> {
    let parser = ParserB;
    let result = parser.parse_summary("@bad;");
    if !matches!(result, Err(ParseSummaryError::MalformedSource)) {
        return Err("other receiver pin should observe ParseSummaryError::MalformedSource".into());
    }
    Ok(())
}
