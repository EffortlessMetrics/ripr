pub fn route(kind: &str) -> &'static str {
    match kind {
        "sensor" => "sensor-v2",
        "focused-test" => "sensor-v2",
        _ => "other",
    }
}
