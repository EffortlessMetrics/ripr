pub enum Subject {
    Invalid(std::string::String),
    Ambiguous(std::string::String),
}

pub struct Wrap(std::string::String);

pub fn note(message: String) {
    Subject::Invalid(message.clone());
}

pub fn send(message: &str) {
    record(message.trim());
}

fn record(message: &str) {
    let _ = message;
}
