use parity_err_guard::discounted_total;

#[test]
fn boundary_matches_expected() -> Result<(), String> {
    let actual = discounted_total(100, 100);
    let expected = 90;
    if actual != expected {
        return Err(format!("actual={actual:?}"));
    }
    Ok(())
}
