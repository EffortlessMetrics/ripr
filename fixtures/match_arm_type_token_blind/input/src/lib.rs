pub enum Mode {
    Warm,
    Frozen,
}

pub fn classify(mode: Mode) -> i32 {
    match mode {
        Mode::Warm => 1,
        Mode::Frozen => -1,
    }
}
