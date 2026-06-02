#![forbid(unsafe_code)]

mod startup;

fn main() {
    if let Err(err) = startup::run() {
        report_failure(&err);
        std::process::exit(exit_code());
    }
}

fn report_failure(err: &str) {
    eprintln!("ripr: {err}");
}

const fn exit_code() -> i32 {
    2
}
