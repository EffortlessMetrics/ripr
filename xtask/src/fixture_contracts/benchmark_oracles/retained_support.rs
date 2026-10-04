//! Verify declared reproduction inputs without replaying historical commands.

use super::*;

#[derive(Clone, Copy)]
enum Shape {
    Bytes,
    Array,
    Object,
}

pub(super) fn validate(root: &Path, pairing: &Value) -> Result<(), String> {
    if let Some(workspace) = pairing.get("original_workspace") {
        if !workspace.is_object() {
            return Err(
                "semantic oracle retained support original_workspace must be an object".to_string(),
            );
        }
        for (group, required, shape) in [
            ("archives", &["parent", "fixed"][..], Shape::Bytes),
            ("inventories", &["parent", "fixed"][..], Shape::Array),
            (
                "provenance",
                &[
                    "parent-git-commit.json",
                    "fixed-git-commit.json",
                    "parent-git-tree.json",
                    "fixed-git-tree.json",
                ][..],
                Shape::Object,
            ),
        ] {
            let entries = workspace[group].as_object().ok_or_else(|| {
                format!(
                    "semantic oracle retained support original_workspace.{group} must be an object"
                )
            })?;
            for name in required {
                if !entries.contains_key(*name) {
                    return Err(format!(
                        "semantic oracle retained support original_workspace.{group} is missing {name}"
                    ));
                }
            }
            for (name, descriptor) in entries {
                verify(
                    root,
                    descriptor,
                    &format!("original_workspace.{group}.{name}"),
                    shape,
                )?;
            }
        }
    }
    for field in [
        "native_packet",
        "native_receipt",
        "resolution_receipt",
        "offline_setup_receipt",
        "capture_checker_interruption",
    ] {
        if let Some(descriptor) = pairing.get(field) {
            verify(root, descriptor, field, Shape::Object)?;
        }
    }
    Ok(())
}

fn verify(root: &Path, descriptor: &Value, label: &str, shape: Shape) -> Result<(), String> {
    let result = match shape {
        Shape::Bytes => verify_file(root, descriptor),
        Shape::Array | Shape::Object => retained_json(root, descriptor).and_then(|value| {
            let (matches, expected) = match shape {
                Shape::Array => (value.is_array(), "array"),
                _ => (value.is_object(), "object"),
            };
            if matches {
                Ok(())
            } else {
                Err(format!("payload must be a JSON {expected}"))
            }
        }),
    };
    result.map_err(|error| format!("semantic oracle retained support {label}: {error}"))
}
