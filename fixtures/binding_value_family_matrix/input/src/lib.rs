pub fn body_of(label: &str) -> &str {
    let body = label.strip_prefix("pre-").map_or("none", |s| s);
    if body == "fix" {
        "matched"
    } else {
        body
    }
}

pub fn gate(label: &str, want: bool) -> bool {
    let open = label.starts_with("pre-");
    open == want
}

pub fn within(input: &str, other: &str) -> bool {
    let size = other.len();
    size == 3
}

pub fn bump(base: usize) -> bool {
    let next = base.checked_add(1).map_or(0, |v| v);
    next == 4
}

pub fn head(label: &str) -> bool {
    let first = label.chars().next().map_or('x', |c| c);
    first == 'x'
}

pub fn tail(label: &str) -> bool {
    let last = label.chars().next_back().map_or('x', |c| c);
    last == 'x'
}
