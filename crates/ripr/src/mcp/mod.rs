mod framing;
mod gaps;
mod protocol;
mod repair;
mod repair_card;
mod server;
mod transport;
mod workspace;
mod writer;

use std::path::PathBuf;

pub(super) const MAX_MESSAGE_BYTES: usize = 256 * 1024;
pub(super) const MAX_RESPONSE_BYTES: usize = 128 * 1024;

pub(crate) const MCP_HELP: &str = r#"Expose RIPR's bounded, read-only workspace session over the Model Context Protocol.

Usage: ripr mcp [--stdio] [--root PATH]

Options:
  --stdio       Serve newline-delimited MCP JSON-RPC over stdin/stdout. This is
                the default and only transport in the supported slices.
  --root PATH   Use this exact repository root. Without it, RIPR starts at the
                current directory and walks ancestors to the nearest supported
                repository marker.
  --help, -h    Print this help.
  --version, -V Print the MCP server version.

The MCP surface is read-only. It exposes tools `ripr_workspace_status`,
`ripr_refresh`, `ripr_list_gaps`, `ripr_get_gap`, `ripr_prepare_repair`,
`ripr_get_repair_attempt`, `ripr_get_receipt_status`, and
`ripr_get_repair_card`, the resource
`ripr://workspace/status`, and the resource templates
`ripr://snapshot/{snapshot_id}`, `ripr://gap/{canonical_item_id}`,
`ripr://repair-attempt/{attempt_id}`, `ripr://receipt/{receipt_id}`, and
`ripr://repair-card/{canonical_item_id}`.
`ripr_refresh` runs one bounded static analysis per call through the same
shared check authority as `ripr check`; the other tools read the committed
snapshot and never re-run analysis. `ripr_prepare_repair` evaluates producer
repair-readiness facts and may create one in-memory session repair
transaction; `ripr_get_repair_attempt` and `ripr_get_receipt_status` read
session transactions and the durable attempt store without executing
anything. `ripr_get_repair_card` projects the same bounded repair card as
`ripr agent card` and the standard-LSP handoff for one canonical item of the
committed snapshot, assembled by the shared application authority; it edits
nothing, executes nothing, and never upgrades readiness or currentness.
The server does not edit source, execute verification or mutation
commands, launch processes, load project-local provider configuration, or
embed a model provider, and no evidence document is ever a repair
authorization. Protocol messages are the only stdout output. Operational
failures use stderr.
"#;

pub fn run(args: &[String]) -> Result<(), String> {
    let mut explicit_root = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--help" | "-h" => {
                println!("{MCP_HELP}");
                return Ok(());
            }
            "--version" | "-V" => {
                println!("ripr-mcp {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--stdio" => {}
            "--root" => {
                // A following option token is a missing value, not a path
                // (`--root --help` must print help, not start a server
                // rooted at a directory named `--help`; #3525 review).
                let value = match args.get(index + 1) {
                    // Single-dash option tokens (-h, -V) are options too:
                    // treating them as paths would swallow the next flag
                    // (#3587 review).
                    Some(value) if !value.starts_with('-') => value.clone(),
                    Some(unexpected) => {
                        return Err(format!(
                            "missing value for --root; found option {unexpected:?}"
                        ));
                    }
                    None => return Err("missing value for --root".to_string()),
                };
                if explicit_root.is_some() {
                    return Err("--root may be passed only once".to_string());
                }
                explicit_root = Some(PathBuf::from(value));
                index += 1;
            }
            argument => {
                return Err(format!(
                    "unknown mcp argument {argument:?}. Run `ripr mcp --help`."
                ));
            }
        }
        index += 1;
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("create MCP runtime: {error}"))?;
    let result = runtime.block_on(transport::serve_stdio(explicit_root));
    // Tokio stdin uses a non-cancellable blocking read. Once the SDK service
    // has terminated, that read must not hold process exit until stdin EOF.
    runtime.shutdown_background();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parser_rejects_unknown_and_ambiguous_root_arguments() {
        assert_eq!(
            run(&args(&["--bad"])),
            Err("unknown mcp argument \"--bad\". Run `ripr mcp --help`.".to_string())
        );
        assert_eq!(
            run(&args(&["--root"])),
            Err("missing value for --root".to_string())
        );
        // A following option token is a missing value, not a path:
        // `--root --help` must surface help, not a directory named `--help`.
        assert!(
            run(&args(&["--root", "--help"]))
                .err()
                .is_some_and(|err| err.contains("missing value for --root"))
        );
        assert!(
            run(&args(&["--root", "--stdio"]))
                .err()
                .is_some_and(|err| err.contains("missing value for --root"))
        );
        assert_eq!(
            run(&args(&["--root", ".", "--root", "."])),
            Err("--root may be passed only once".to_string())
        );
    }
}
