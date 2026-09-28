use snapshot_oracle_fixture::render_status;

#[test]
fn renders_error_status_snapshot() {
    let rendered = render_status(404);
    insta::assert_snapshot!(rendered);
}
