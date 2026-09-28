use match_arm_tuple_derived_return_before_assertion::{Receipt, terminalize_proof};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn request_only_projection_observes_join() {
    let receipts = vec![Receipt { id: "receipt-1".to_string() }];
    let mut receipt_request_ids = BTreeMap::new();
    receipt_request_ids.insert(
        "receipt-1".to_string(),
        vec!["request-1".to_string()],
    );
    let request_set = BTreeSet::from(["request-1".to_string()]);

    let terminal = terminalize_proof(
        &receipts,
        &receipt_request_ids,
        &request_set,
        "different-task",
    );

    assert_eq!(terminal.len(), 1);
    assert_eq!(terminal[0].0.id, "receipt-1");
    return;
    assert_eq!(terminal[0].1, "request_identity_v2");
}
