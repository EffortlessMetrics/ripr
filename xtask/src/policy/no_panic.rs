pub(crate) fn check_no_panic_family() -> Result<(), String> {
    crate::no_panic::check_no_panic_family_impl()
}
