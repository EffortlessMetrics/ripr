//! Alias selection controls, including the owner/test and barrel consumers.

use super::*;
use crate::analysis::language::typescript::{
    ReExportIndex, extract_owners, extract_tests, find_related_tests,
};
use crate::domain::RelationReason;
use serde_json::{Value, json};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("fixture clock: {err}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-tsconfig-precedence-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&root).map_err(|err| format!("create {}: {err}", root.display()))?;
        let workspace = Self(root);
        workspace.write("package.json", "{}")?;
        for path in ["src/cart.ts", "fallback/feature/cart.ts", "literal.ts"] {
            workspace.write(path, "export const value = 1;\n")?;
        }
        Ok(workspace)
    }

    fn write(&self, path: &str, source: &str) -> Result<(), String> {
        let absolute = self.0.join(path);
        let parent = absolute.parent().ok_or("fixture path has no parent")?;
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
        fs::write(&absolute, source).map_err(|err| format!("write {}: {err}", absolute.display()))
    }

    fn map(&self, paths: Value) -> Result<TsAliasMap, String> {
        let config = json!({"compilerOptions": {"baseUrl": ".", "paths": paths}});
        self.write("tsconfig.json", &config.to_string())?;
        load_alias_map(&self.0).ok_or_else(|| "fixture alias map did not load".to_string())
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Rotations and reversals cover all permutations for the two/three-key cases
/// below. This deterministically exercises the old first-entry-wins defect,
/// rather than depending on a particular randomized HashMap iteration order.
fn assert_resolution_in_every_order(map: &TsAliasMap, specifier: &str, expected: Option<&str>) {
    for offset in 0..map.glob_entries.len().max(1) {
        for reverse in [false, true] {
            let mut reordered = map.clone();
            if !reordered.glob_entries.is_empty() {
                reordered.glob_entries.rotate_left(offset);
            }
            if reverse {
                reordered.glob_entries.reverse();
            }
            assert_eq!(
                reordered.resolve(specifier).as_deref(),
                expected.map(Path::new),
                "{specifier}; entries: {:?}",
                reordered.glob_entries
            );
        }
    }
}

#[test]
fn longest_prefix_wins_independently_of_storage_order() -> Result<(), String> {
    let workspace = Workspace::new()?;
    let map = workspace.map(json!({
        "*": ["literal.ts"],
        "@/*": ["fallback/*"],
        "@/feature/*": ["src/*"]
    }))?;
    assert_resolution_in_every_order(&map, "@/feature/cart", Some("src/cart.ts"));
    Ok(())
}

#[test]
fn exact_key_wins_over_matching_globs() -> Result<(), String> {
    let workspace = Workspace::new()?;
    let map = workspace.map(json!({
        "@/*": ["fallback/*"],
        "@/feature/*": ["src/*"],
        "@/feature/cart": ["literal.ts"]
    }))?;
    assert_resolution_in_every_order(&map, "@/feature/cart", Some("literal.ts"));
    Ok(())
}

#[test]
fn unsupported_winning_keys_block_broader_aliases() -> Result<(), String> {
    let workspace = Workspace::new()?;
    let control = workspace.map(json!({"@/*": ["fallback/*"]}))?;
    assert_eq!(
        control.resolve("@/feature/cart"),
        Some(PathBuf::from("fallback/feature/cart.ts"))
    );
    for key in ["@/feature/cart", "@/feature/*"] {
        for values in [
            json!([]),
            json!(["src/cart", "other/cart"]),
            json!(["src/*/*"]),
        ] {
            let mut paths = serde_json::Map::new();
            paths.insert("@/*".to_string(), json!(["fallback/*"]));
            paths.insert(key.to_string(), values);
            let map = workspace.map(Value::Object(paths))?;
            assert_resolution_in_every_order(&map, "@/feature/cart", None);
        }
    }
    // Unsupported, nonmatching keys must not poison an unrelated valid alias.
    let map = workspace.map(json!({
        "@/*": ["fallback/*"],
        "@/unrelated/*": []
    }))?;
    assert_resolution_in_every_order(&map, "@/feature/cart", Some("fallback/feature/cart.ts"));
    Ok(())
}

#[test]
fn unresolved_winner_does_not_fall_back_to_an_existing_broader_target() -> Result<(), String> {
    let workspace = Workspace::new()?;
    workspace.write("src/cart.tsx", "export const value = 2;\n")?;
    for paths in [
        json!({"@/*": ["fallback/*"], "@/feature/*": ["missing/*"]}),
        json!({"@/*": ["fallback/*"], "@/feature/cart": ["missing/cart"]}),
        json!({"@/*": ["fallback/*"], "@/feature/*": ["src/*"]}),
        json!({"@/*": ["fallback/*"], "@/feature/cart": ["src/cart"]}),
    ] {
        let map = workspace.map(paths)?;
        assert_resolution_in_every_order(&map, "@/feature/cart", None);
    }
    Ok(())
}

#[test]
fn equal_longest_prefixes_remain_unresolved() -> Result<(), String> {
    let workspace = Workspace::new()?;
    let map = workspace.map(json!({
        "@/feature/*": ["src/*"],
        "@/feature/*cart": ["literal.ts"]
    }))?;
    assert_resolution_in_every_order(&map, "@/feature/cart", None);
    // Existence of only one candidate is not authority to break a key tie.
    fs::remove_file(workspace.0.join("literal.ts"))
        .map_err(|err| format!("remove tie fixture: {err}"))?;
    assert_resolution_in_every_order(&map, "@/feature/cart", None);
    Ok(())
}

#[test]
fn lower_prefix_ties_do_not_block_a_unique_longer_prefix() -> Result<(), String> {
    let workspace = Workspace::new()?;
    let map = workspace.map(json!({
        "@/*": ["fallback/*"],
        "@/*cart": ["literal.ts"],
        "@/feature/*": ["src/*"]
    }))?;
    assert_resolution_in_every_order(&map, "@/feature/cart", Some("src/cart.ts"));
    Ok(())
}

#[test]
fn suffix_must_match_before_prefix_precedence_applies() -> Result<(), String> {
    let workspace = Workspace::new()?;
    let map = workspace.map(json!({
        "@/*": ["fallback/*"],
        "@/feature/*/index": ["src/*"]
    }))?;
    assert_resolution_in_every_order(&map, "@/feature/cart", Some("fallback/feature/cart.ts"));
    assert_resolution_in_every_order(&map, "@/feature/cart/index", Some("src/cart.ts"));
    Ok(())
}

#[test]
fn direct_and_barrel_relations_do_not_borrow_a_same_named_fallback_owner() -> Result<(), String> {
    let workspace = Workspace::new()?;
    let source = "export function shippingFee(total: number) { return total > 50 ? 0 : 5; }\n";
    let owner_file = Path::new("src/cart.ts");
    let decoy_file = Path::new("fallback/feature/cart.ts");
    workspace.write("src/cart.ts", source)?;
    workspace.write("fallback/feature/cart.ts", source)?;
    workspace.write(
        "src/barrel.ts",
        "export { shippingFee } from '@/feature/cart';\n",
    )?;
    let owners = extract_owners(owner_file, source);
    let decoys = extract_owners(decoy_file, source);
    assert_eq!(owners.len(), 1, "nonempty real-owner control");
    assert_eq!(decoys.len(), 1, "nonempty same-named decoy control");
    let owner = owners.first().ok_or("owner extraction failed")?;
    let decoy = decoys.first().ok_or("decoy extraction failed")?;
    assert_eq!(owner.name, decoy.name);

    for blocked in [false, true] {
        let target = if blocked {
            json!(["src/*", "other/*"])
        } else {
            json!(["src/*"])
        };
        let mut map = workspace.map(json!({
            "@/*": ["fallback/*"],
            "@/feature/*": target
        }))?;
        // Force the old implementation's wrong winning order.
        map.glob_entries.sort_by_key(|entry| entry.prefix.len());
        let barrel_source = fs::read_to_string(workspace.0.join("src/barrel.ts"))
            .map_err(|err| format!("read barrel fixture: {err}"))?;
        let mut sources = std::collections::HashMap::new();
        sources.insert(PathBuf::from("src/barrel.ts"), barrel_source);
        let index = ReExportIndex::build(
            &[PathBuf::from("src/barrel.ts")],
            &sources,
            &workspace.0,
            Some(&map),
            |_| false,
        );
        for (import_source, reason) in [
            ("@/feature/cart", RelationReason::DirectOwnerCall),
            ("../src/barrel", RelationReason::ReExportChainFollowed),
        ] {
            let test_file = Path::new("tests/cart.test.ts");
            let test_source = format!(
                "import {{ shippingFee }} from '{import_source}';\n\
                 test('shipping fee', () => {{ expect(shippingFee(51)).toBe(0); }});\n"
            );
            let tests = extract_tests(test_file, &test_source);
            assert_eq!(tests.len(), 1, "nonempty parsed-test control");
            let related = find_related_tests(owner, &tests, Some(&workspace.0), &index, Some(&map));
            let unrelated =
                find_related_tests(decoy, &tests, Some(&workspace.0), &index, Some(&map));
            assert!(
                unrelated.is_empty(),
                "{import_source} credited the decoy: {unrelated:?}"
            );
            if blocked {
                assert!(
                    related.is_empty(),
                    "unsupported winning mapping was credited"
                );
            } else {
                assert_eq!(related.len(), 1, "{import_source} lost its real owner");
                let relation = related.first().ok_or("related test disappeared")?;
                assert_eq!(relation.file, test_file);
                assert_eq!(relation.relation_reason, Some(reason));
            }
        }
    }
    Ok(())
}
