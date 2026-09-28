use propagate_stdout_macro_fixture::report;

#[test]
fn report_does_not_panic() {
    report(5);
}
