pub fn rate(value: i32) -> i32 { value }

#[cfg(test)]
mod plain {
    fn plain_helper(value: i32) -> i32 { value }
    #[test]
    fn plain_case() { assert_eq!(super::plain::plain_helper(1), 1); }
}

#[cfg(all(unix, test))]
mod conjunct {
    fn conjunct_helper(value: i32) -> i32 { value }
    #[test]
    fn conjunct_case() { assert_eq!(super::conjunct::conjunct_helper(2), 2); }
}

#[cfg(not(not(test)))]
mod negated {
    fn negated_helper(value: i32) -> i32 { value }
    #[test]
    fn negated_case() { assert_eq!(super::negated::negated_helper(3), 3); }
}
