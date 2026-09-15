use boxed_wrapper_fail_closed_fixture::{ChecksumError, ParseSummaryError, checksum_summary, render_summary};

#[test]
fn checksum_summary_pins_wrong_sibling_variant() -> Result<(), Box<dyn std::error::Error>> {
    let error = checksum_summary("payload?")
        .err()
        .ok_or("checksum wrapper must fail closed on bad checksums")?;
    if !matches!(
        error.downcast_ref::<ChecksumError>(),
        Some(ChecksumError::MalformedPayload)
    ) {
        return Err("checksum witness pinned the malformed-payload sibling".into());
    }
    Ok(())
}

#[test]
fn render_summary_observes_other_enum_variant() -> Result<(), Box<dyn std::error::Error>> {
    let error = render_summary("zhex")
        .err()
        .ok_or("render wrapper must fail closed on bad hex input")?;
    if !matches!(
        error.downcast_ref::<ParseSummaryError>(),
        Some(ParseSummaryError::MalformedSource)
    ) {
        return Err("render witness pinned the parse-summary variant instead".into());
    }
    Ok(())
}
