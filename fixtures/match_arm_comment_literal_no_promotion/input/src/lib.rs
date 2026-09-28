pub fn route(kind: &str) -> &'static str {
    match kind {
        "sensor" /* "focused-test" */ => "sensor-v2",
        "focused-test" => "proof",
        _ => "other",
    }
}
