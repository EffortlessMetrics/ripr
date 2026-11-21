mod eval;

pub fn accepts_boundary(value: u64) -> bool {
    public_entry(value)
}

fn public_entry(value: u64) -> bool {
    step_one(value)
}

fn step_one(value: u64) -> bool {
    step_two(value)
}

fn step_two(value: u64) -> bool {
    eval::matches_greater(value, 3)
}
