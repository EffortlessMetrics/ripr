use boxed_wrapper_downcast_witness_fixture::{
    ChecksumError, GlyphError, ParseSummaryError, ThemeError, checksum_summary, glyph_summary,
    length_summary, parse_summary, render_summary, theme_summary, try_glyph_summary,
    try_length_summary, try_parse_summary, try_theme_summary,
};

#[test]
fn parse_summary_fails_closed_on_malformed_source()
-> Result<(), Box<dyn std::error::Error>> {
    let result = try_parse_summary("@bad;");
    if !matches!(result, Err(ParseSummaryError::MalformedSource)) {
        return Err("malformed source should produce ParseSummaryError::MalformedSource".into());
    }
    Ok(())
}

#[test]
fn parse_summary_boxed_variant_propagates_malformed_source()
-> Result<(), Box<dyn std::error::Error>> {
    let error = parse_summary("@bad;")
        .err()
        .ok_or("boxed variant must also fail closed on error input")?;
    if !matches!(
        error.downcast_ref::<ParseSummaryError>(),
        Some(ParseSummaryError::MalformedSource)
    ) {
        return Err("boxed variant must preserve ParseSummaryError::MalformedSource".into());
    }
    Ok(())
}

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

#[test]
fn length_summary_only_checks_is_err() {
    let result = length_summary("way-too-long-input-that-exceeds-the-forty-char-limit!!");
    assert!(result.is_err());
}

#[test]
fn glyph_summary_downcast_witness_on_stringified_wrapper()
-> Result<(), Box<dyn std::error::Error>> {
    let error = glyph_summary("#glyph")
        .err()
        .ok_or("glyph wrapper must fail closed on malformed glyphs")?;
    if !matches!(
        error.downcast_ref::<GlyphError>(),
        Some(GlyphError::MalformedGlyph)
    ) {
        return Err("glyph variant witness requires the typed conversion".into());
    }
    Ok(())
}

#[test]
fn theme_summary_ignores_matches_result() -> Result<(), Box<dyn std::error::Error>> {
    let error = theme_summary("&theme")
        .err()
        .ok_or("theme wrapper must fail closed on duplicate themes")?;
    let _ = matches!(
        error.downcast_ref::<ThemeError>(),
        Some(ThemeError::DuplicateTheme)
    );
    Ok(())
}
