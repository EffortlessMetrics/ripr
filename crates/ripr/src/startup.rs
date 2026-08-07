#![forbid(unsafe_code)]

pub(crate) fn run() -> Result<(), String> {
    let mut args = collect_args();
    // #2610: extract --verbose before command dispatch so it works with any
    // subcommand. The flag is consumed and not passed to the command parser.
    if let Some(pos) = args.iter().position(|a| a == "--verbose" || a == "-v") {
        args.remove(pos);
        ripr::set_verbose(true);
        eprintln!("ripr: verbose mode enabled");
    }
    ripr::cli::run(args)
}

fn collect_args() -> Vec<String> {
    std::env::args().collect()
}
