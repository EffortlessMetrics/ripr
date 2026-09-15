use harness_suppression::price;

fn driver() -> Result<(), String> {
    let output = price(120, 100).to_string();
    if !output.contains("110") {
        return Err(format!("unexpected output: {output}"));
    }
    Ok(())
}

fn checked(value: i32) -> Result<i32, String> {
    if value < 0 {
        return Err(format!("negative: {value}"));
    }
    Ok(value)
}

#[test]
fn contract_journey() -> Result<(), String> {
    driver()?;
    let value = checked(price(50, 100)).map_err(|error| format!("checked: {error}"))?;
    assert_eq!(value, 50);
    if price(100, 100) != 90 {
        return Err("boundary mismatch".to_string());
    }
    assert_eq!(price(100, 100), 90);
    Ok(())
}
