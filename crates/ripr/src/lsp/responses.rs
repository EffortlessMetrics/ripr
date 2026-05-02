pub(crate) fn initialize_response(id: &str) -> String {
    format!(
        r#"{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{{\"capabilities\":{{\"textDocumentSync\":1,\"hoverProvider\":true,\"codeActionProvider\":true,\"executeCommandProvider\":{{\"commands\":[\"ripr.collectContext\",\"ripr.refresh\"]}}}},\"serverInfo\":{{\"name\":\"ripr\",\"version\":\"{}\"}}}}}}"#,
        env!("CARGO_PKG_VERSION")
    )
}

pub(crate) fn hover_response(id: &str) -> String {
    format!(
        r#"{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{{\"contents\":{{\"kind\":\"markdown\",\"value\":\"ripr estimates static RIPR exposure for changed Rust behavior. Run `ripr check --format json` for current findings.\"}}}}}}"#
    )
}

pub(crate) fn code_action_response(id: &str) -> String {
    format!(
        r#"{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":[{{\"title\":\"Copy ripr context packet\",\"kind\":\"quickfix\",\"command\":{{\"title\":\"Collect ripr context\",\"command\":\"ripr.collectContext\",\"arguments\":[]}}}},{{\"title\":\"Run ripr check\",\"kind\":\"source\",\"command\":{{\"title\":\"Refresh ripr analysis\",\"command\":\"ripr.refresh\",\"arguments\":[]}}}}]}}"#
    )
}
