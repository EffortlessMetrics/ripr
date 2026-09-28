pub fn render_status(code: u16) -> String {
    if code == 200 {
        "ok".to_string()
    } else {
        format!("error:{code}")
    }
}
