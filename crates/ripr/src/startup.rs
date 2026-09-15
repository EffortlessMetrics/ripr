#![forbid(unsafe_code)]

pub(crate) fn run() -> Result<(), String> {
    dispatch(collect_args())
}

fn dispatch(args: Vec<String>) -> Result<(), String> {
    if let Some(mcp_args) = routed_mcp_args(&args) {
        return ripr::mcp::run(&mcp_args);
    }
    ripr::cli::run(args)
}

fn routed_mcp_args(args: &[String]) -> Option<Vec<String>> {
    // #2610: the global --verbose/-v spelling works in any position, so
    // extract it before the route check. `ripr --verbose mcp` and
    // `ripr mcp --verbose` route identically; the verbose diagnostic goes
    // to stderr and MCP protocol frames occupy stdout, so the protocol
    // stream stays clean.
    let mut verbose = false;
    let mut body: Vec<String> = Vec::new();
    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--verbose" | "-v" => verbose = true,
            other => body.push(other.to_string()),
        }
    }
    let is_direct = body.first().is_some_and(|first| first == "mcp");
    let is_help_route = body.first().is_some_and(|first| first == "help")
        && body.get(1).is_some_and(|second| second == "mcp");
    if !is_direct && !is_help_route {
        return None;
    }
    if verbose {
        ripr::set_verbose(true);
        eprintln!("ripr: verbose mode enabled");
    }
    if is_help_route {
        let mut routed = vec!["--help".to_string()];
        routed.extend(body.into_iter().skip(2));
        return Some(routed);
    }
    Some(body.into_iter().skip(1).collect())
}

fn collect_args() -> Vec<String> {
    std::env::args().collect()
}

#[cfg(test)]
mod tests {
    use super::routed_mcp_args;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn startup_routes_direct_and_help_mcp_invocations_before_general_cli() {
        assert_eq!(
            routed_mcp_args(&args(&["ripr", "mcp", "--stdio", "--root", "."])),
            Some(args(&["--stdio", "--root", "."]))
        );
        assert_eq!(
            routed_mcp_args(&args(&["ripr", "help", "mcp"])),
            Some(args(&["--help"]))
        );
        assert_eq!(routed_mcp_args(&args(&["ripr", "check"])), None);
    }

    #[test]
    fn startup_extracts_the_global_verbose_flag_in_any_position() {
        // #2610 contract: --verbose works appended or prepended. Startup
        // strips it before routing so the MCP parser never sees it and the
        // route is identical in both spellings.
        assert_eq!(
            routed_mcp_args(&args(&["ripr", "--verbose", "mcp", "--stdio"])),
            Some(args(&["--stdio"]))
        );
        assert_eq!(
            routed_mcp_args(&args(&["ripr", "mcp", "--verbose", "--stdio"])),
            Some(args(&["--stdio"]))
        );
        assert_eq!(
            routed_mcp_args(&args(&["ripr", "-v", "help", "mcp"])),
            Some(args(&["--help"]))
        );
        assert_eq!(routed_mcp_args(&args(&["ripr", "check"])), None);
    }
}
