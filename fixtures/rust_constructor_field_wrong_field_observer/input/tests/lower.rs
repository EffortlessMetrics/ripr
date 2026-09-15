use rust_constructor_field_wrong_field_observer_fixture::{Storage, lower_ast};

#[test]
fn lower_ast_only_checks_name() {
    let statement = lower_ast("item".to_string(), Storage::Our);
    assert_eq!(statement.name, "item");
}
