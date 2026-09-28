pub(crate) fn check_ci_lane_whitelist() -> Result<(), String> {
    crate::check_ci_lane_whitelist_impl()
}
