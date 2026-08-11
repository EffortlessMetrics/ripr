use crate::{Fragment, expected_root};

/// Oracle helper: walks deserialized fragments and asserts workspace source
/// path invariants. Only tests call it; it has no production callers.
pub fn assert_workspace_source_paths_are_stable(fragments: &[Fragment]) {
    for fragment in fragments {
        let expected = expected_root(&fragment.crate_name);
        let missing = if fragment.crate_root.is_none() { 1 } else { 0 };
        assert_eq!(missing, 0, "crate `{}` lost its source root", fragment.crate_name);
        assert_eq!(
            fragment.crate_root.as_deref().unwrap_or(""),
            expected,
            "crate `{}` reported an unexpected source root",
            fragment.crate_name,
        );
        for span in &fragment.spans {
            assert!(
                !span.file.contains('\\'),
                "span path `{}` keeps a native separator",
                span.file,
            );
        }
    }
}
