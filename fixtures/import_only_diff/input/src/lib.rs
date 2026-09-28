use std::cmp::Ordering;

pub fn compare_total(left: i32, right: i32) -> std::cmp::Ordering {
    let _ = Ordering::Equal;
    left.cmp(&right)
}
