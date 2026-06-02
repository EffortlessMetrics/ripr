#![forbid(unsafe_code)]

pub(crate) fn run() -> Result<(), String> {
    ripr::cli::run(collect_args())
}

fn collect_args() -> Vec<String> {
    std::env::args().collect()
}
