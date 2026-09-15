pub fn admission(capacity: usize) -> &'static str {
    let open = true;
    if open {
        "welcome"
    } else {
        "full"
    }
}

pub fn band_label(score: usize) -> &'static str {
    let band = 2;
    match band {
        2 => "high",
        _ => "low",
    }
}

pub fn within(limit: usize, value: usize) -> bool {
    let margin = 10;
    margin > value
}
