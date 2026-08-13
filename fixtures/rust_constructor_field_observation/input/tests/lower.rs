use rust_constructor_field_observation_fixture::{Storage, lower_ast};

#[test]
fn lower_ast_preserves_storage() {
    let statement = lower_ast("item".to_string(), Storage::Our);
    assert_eq!(statement.storage, Storage::Our);
}
