use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub enum Unit {
    Week,
    Fortnight,
}

impl FromStr for Unit {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        match s {
            "week" => Ok(Unit::Week),
            "fortnight" => Ok(Unit::Fortnight),
            _ => Err(()),
        }
    }
}

pub fn seconds(u: Unit) -> u64 {
    match u {
        Unit::Week => 604_800,
        Unit::Fortnight => 1_209_600,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_total() {
        let total: u64 = ["week", "fortnight"]
            .iter()
            .map(|s| seconds(Unit::from_str(s).unwrap()))
            .sum();
        assert_eq!(total, 1_814_400);
    }

    #[test]
    fn from_str_fortnight() {
        assert!(matches!(Unit::from_str("fortnight"), Ok(Unit::Fortnight)));
    }
}
