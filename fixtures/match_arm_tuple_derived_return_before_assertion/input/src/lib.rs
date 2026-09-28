use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, PartialEq, Eq)]
pub struct Receipt {
    pub id: String,
}

pub fn terminalize_proof<'a>(
    receipts: &'a [Receipt],
    receipt_request_ids: &BTreeMap<String, Vec<String>>,
    request_set: &BTreeSet<String>,
    task_id: &str,
) -> Vec<(&'a Receipt, &'static str)> {
    receipts
        .iter()
        .filter_map(|receipt| {
            let request_identity_matches = receipt_request_ids
                .get(&receipt.id)
                .is_some_and(|receipt_requests| {
                    receipt_requests
                        .iter()
                        .any(|request_id| request_set.contains(request_id.as_str()))
                });
            let task_identity_matches = receipt.id == task_id;
            let relation = match (request_identity_matches, task_identity_matches) {
                (true, true) => "request_and_task_identity",
                (true, false) => "request_identity_v2",
                (false, true) => "task_identity",
                (false, false) => return None,
            };
            Some((receipt, relation))
        })
        .collect()
}
