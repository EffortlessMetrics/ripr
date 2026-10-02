#![forbid(unsafe_code)]

use ripr::cli::CommandError;

pub(crate) fn run() -> Result<(), CommandError> {
    dispatch(collect_args())
}

fn dispatch(args: Vec<String>) -> Result<(), CommandError> {
    if let Some(mcp_args) = routed_mcp_args(&args) {
        return ripr::mcp::run(&mcp_args).map_err(CommandError::from);
    }
    ripr::cli::run(args)
}

fn routed_mcp_args(args: &[String]) -> Option<Vec<String>> {
    // #2610: the global --verbose/-v spelling works in any position, so the
    // route check skips those tokens; `ripr --verbose mcp` and
    // `ripr mcp --verbose` route identically. #5009: on the MCP route the
    // flag is removed by the same single owner the CLI dispatch uses
    // (`ripr::cli::extract_global_verbose`), so the stripping rule cannot
    // diverge between command families. The verbose diagnostic goes to
    // stderr and MCP protocol frames occupy stdout, so the protocol stream
    // stays clean.
    let body: Vec<&String> = args
        .iter()
        .skip(1)
        .filter(|arg| !matches!(arg.as_str(), "--verbose" | "-v"))
        .collect();
    let is_direct = body.first().is_some_and(|first| *first == "mcp");
    let is_help_route = body.first().is_some_and(|first| *first == "help")
        && body.get(1).is_some_and(|second| *second == "mcp");
    if !is_direct && !is_help_route {
        return None;
    }
    let mut owned = args.to_vec();
    if ripr::cli::extract_global_verbose(&mut owned) {
        ripr::set_verbose(true);
        eprintln!("ripr: verbose mode enabled");
    }
    if is_help_route {
        let mut routed = vec!["--help".to_string()];
        // `owned` still carries argv[0]; drop it plus the `help mcp` pair.
        routed.extend(owned.into_iter().skip(3));
        return Some(routed);
    }
    // `owned` still carries argv[0]; drop it plus the `mcp` command itself.
    Some(owned.into_iter().skip(2).collect())
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

    #[test]
    fn startup_treats_a_repeated_global_verbose_as_one_enable() {
        // #5009 regression guard for the MCP startup path: the shared
        // extraction removes every occurrence, so a repeat routes to the
        // same MCP argv as a single flag — the same rule the CLI dispatch
        // path pins in `cli::mod.rs`.
        assert_eq!(
            routed_mcp_args(&args(&["ripr", "-v", "-v", "mcp", "--stdio"])),
            Some(args(&["--stdio"]))
        );
        assert_eq!(
            routed_mcp_args(&args(&["ripr", "mcp", "--stdio", "--verbose", "-v"])),
            Some(args(&["--stdio"]))
        );
        assert_eq!(
            routed_mcp_args(&args(&["ripr", "-v", "-v", "help", "mcp"])),
            Some(args(&["--help"]))
        );
    }
}
