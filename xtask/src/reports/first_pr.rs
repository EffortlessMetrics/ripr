pub(crate) fn first_pr(args: &[String]) -> Result<(), String> {
    let mut cli_args = Vec::with_capacity(args.len() + 2);
    cli_args.push("ripr".to_string());
    cli_args.push("first-pr".to_string());
    cli_args.extend(args.iter().cloned());
    ripr::cli::run(cli_args)
}
