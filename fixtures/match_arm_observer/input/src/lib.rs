pub enum Status {
    Active,
    Idle,
}

pub fn classify(s: Status) -> i32 {
    match s {
        Status::Active => 1,
        Status::Idle => 0,
    }
}
