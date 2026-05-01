#![forbid(unsafe_code)]

fn main() {
    if let Err(err) = ripr::cli::run(std::env::args().collect()) {
        eprintln!("ripr: {err}");
        std::process::exit(2);
    }
}
