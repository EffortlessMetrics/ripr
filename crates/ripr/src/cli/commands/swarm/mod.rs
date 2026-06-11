mod ingest;
mod queue;

use crate::cli::help;

pub(super) fn run(args: &[String]) -> Result<(), String> {
    let Some((subcommand, rest)) = args.split_first() else {
        help::print_swarm_help();
        return Ok(());
    };
    match subcommand.as_str() {
        "--help" | "-h" => {
            help::print_swarm_help();
            Ok(())
        }
        "queue" => {
            if rest.iter().any(|arg| arg == "--help" || arg == "-h") {
                help::print_swarm_queue_help();
                return Ok(());
            }
            queue::run(queue::parse_options(rest)?)
        }
        "ingest" => {
            if rest.iter().any(|arg| arg == "--help" || arg == "-h") {
                help::print_swarm_ingest_help();
                return Ok(());
            }
            ingest::run(ingest::parse_options(rest)?)
        }
        other => Err(format!(
            "unknown swarm subcommand {other:?}; expected `queue` or `ingest`"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn rejects_unknown_subcommands_and_missing_root() {
        assert_eq!(
            run(&args(&["unknown"])),
            Err("unknown swarm subcommand \"unknown\"; expected `queue` or `ingest`".to_string())
        );
        assert_eq!(run(&args(&[])), Ok(()));
        assert_eq!(run(&args(&["queue", "--help"])), Ok(()));
        assert_eq!(run(&args(&["ingest", "--help"])), Ok(()));
        assert_eq!(
            run(&args(&[
                "queue",
                "--root",
                "target/ripr/missing-swarm-queue-root",
                "--language",
                "python",
            ])),
            Err(
                "swarm queue root target/ripr/missing-swarm-queue-root is not a directory"
                    .to_string()
            )
        );
        assert_eq!(
            run(&args(&[
                "ingest",
                "--root",
                "target/ripr/missing-swarm-ingest-root",
                "--result",
                "agent-result.json",
            ])),
            Err(
                "swarm ingest root target/ripr/missing-swarm-ingest-root is not a directory"
                    .to_string()
            )
        );
    }
}
