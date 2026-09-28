//! The projection must be observed before the containing test returns.

use super::{named_function, parsed, top_level_projection_observes};

const RECEIPTS: &str = r#"let receipts = vec![Receipt { id: "receipt-1".to_string() }];"#;
const OWNER: &str = "let terminal = terminalize_proof(&receipts);";
const LENGTH: &str = "assert_eq!(terminal.len(), 1);";
const IDENTITY: &str = r#"assert_eq!(terminal[0].0.id, "receipt-1");"#;
const RELATION: &str = r#"assert_eq!(terminal[0].1, "request_identity_v2");"#;

fn admission(body: &str) -> Result<Option<bool>, String> {
    let source = format!("#[test] fn projection() {{ {body} }}");
    let root = parsed(&source).ok_or("return-boundary fixture must parse")?;
    let function = named_function(&root, "projection")
        .ok_or("return-boundary fixture must have one projection test")?;
    Ok(top_level_projection_observes(
        &function,
        "terminalize_proof",
        "request_identity_v2",
    ))
}

#[test]
fn projection_return_boundary_requires_every_observation_before_return() -> Result<(), String> {
    for assertions in [
        [LENGTH, IDENTITY, RELATION],
        [LENGTH, RELATION, IDENTITY],
        [IDENTITY, LENGTH, RELATION],
        [IDENTITY, RELATION, LENGTH],
        [RELATION, LENGTH, IDENTITY],
        [RELATION, IDENTITY, LENGTH],
    ] {
        let statements = [RECEIPTS, OWNER, assertions[0], assertions[1], assertions[2]];
        assert_eq!(admission(&statements.join("\n"))?, Some(true));
        // Return before the input, owner, or any required observation must
        // reject. A return after all observations must not over-reject.
        for position in 0..=statements.len() {
            let mut with_return = statements.to_vec();
            with_return.insert(position, "return;");
            assert_eq!(
                admission(&with_return.join("\n"))?,
                Some(position == statements.len()),
                "return position {position}; assertions {assertions:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn projection_return_boundary_does_not_search_nested_scopes_or_text() -> Result<(), String> {
    for prefix in [
        "let _not_called = || { return; };",
        "fn not_called() { return; }",
        "// return; is only a comment\n",
        r#"let _note = "return;";"#,
    ] {
        let body = [prefix, RECEIPTS, OWNER, LENGTH, IDENTITY, RELATION].join("\n");
        assert_eq!(admission(&body)?, Some(true), "{prefix}");
    }
    Ok(())
}

#[test]
fn projection_return_boundary_preserves_whole_body_owner_uniqueness() -> Result<(), String> {
    let body = [
        RECEIPTS,
        OWNER,
        LENGTH,
        IDENTITY,
        RELATION,
        "return;",
        "let another = terminalize_proof(&receipts);",
    ]
    .join("\n");
    assert_eq!(admission(&body)?, None);
    Ok(())
}
