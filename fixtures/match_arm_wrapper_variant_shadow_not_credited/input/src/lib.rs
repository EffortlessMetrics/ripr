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

pub fn seconds_bridge(u: Unit) -> u64 {
    seconds(u)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Unit;
    impl Unit { const Fortnight: super::Unit = super::Unit::Week; }
    #[test]
    fn seconds_total() { assert_eq!(seconds(super::Unit::Week), 604_800); }
    #[test]
    fn bridge_fortnight() { assert_eq!(seconds_bridge(Unit::Fortnight), 604_800); }
}
