use std::str::FromStr;

#[wrapper_binding_attribute_macros::alter_unit]
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

pub fn seconds_bridge(u: Unit) -> u64 {
    seconds(u)
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
        assert_eq!(total, 1_209_600);
    }

    #[test]
    fn bridge_fortnight() {
        assert_eq!(seconds_bridge(Unit::Fortnight), 604_800);
    }

    #[test]
    fn from_str_fortnight() {
        assert!(matches!(Unit::from_str("fortnight"), Ok(Unit::Fortnight)));
    }
}
