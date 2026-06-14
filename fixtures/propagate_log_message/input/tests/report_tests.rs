use propagate_log_message_fixture::report;

#[test]
fn report_does_not_panic() {
    report(5);
}
