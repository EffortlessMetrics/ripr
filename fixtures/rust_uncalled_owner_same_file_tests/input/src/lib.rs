pub fn discount(total: u32) -> u32 {
    if total >= 100 { total - 10 } else { total }
}

pub fn untested_rounding(cents: u32) -> u32 {
    (cents + 49) / 100
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discount_applies_at_100() {
        assert_eq!(discount(100), 90);
    }

    #[test]
    fn discount_small_unchanged() {
        assert_eq!(discount(50), 50);
    }
}
