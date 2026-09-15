use assertion_shaped_oracle_test_file::fragment_checks::assert_workspace_source_paths_are_stable;
use assertion_shaped_oracle_test_file::{Fragment, Span};

#[test]
fn workspace_source_paths_are_stable() {
    let fragments = vec![Fragment {
        crate_name: "library".to_string(),
        crate_root: Some("crates/library/src/lib.rs".to_string()),
        spans: vec![Span {
            file: "crates/library/src/lib.rs".to_string(),
            line: 3,
        }],
    }];
    assert_workspace_source_paths_are_stable(&fragments);
}
