pub(crate) fn matches_greater(value: u64, limit: u64) -> bool {
    if value != limit {
        return value > limit;
    }

    false
}
