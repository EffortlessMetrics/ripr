use super::{run_ripr, unique_temp_workspace};

/// Real direct-owner matches, not a fabricated Finding or declaration count.
/// Six/eight are inherited positives; nine discriminates count loss at packing.
#[test]
fn direct_owner_related_total_survives_packing_in_json_and_human() -> Result<(), String> {
    let root = unique_temp_workspace("prepack-related-count");
    let result = (|| {
        let mut classifications = std::collections::BTreeMap::new();
        for count in [6, 8, 9] {
            let fixture = root.join(format!("count-{count}"));
            std::fs::create_dir_all(fixture.join("src")).map_err(|error| error.to_string())?;
            std::fs::write(fixture.join("Cargo.toml"),
                "[package]\nname = \"related_count_control\"\nversion = \"0.0.0\"\nedition = \"2021\"\n")
                .map_err(|error| error.to_string())?;
            let mut source = "pub fn record_mark(values: &mut Vec<u32>) {\n    values.push(1);\n}\n#[cfg(test)]\nmod tests {\nuse super::record_mark;\n".to_string();
            for index in 0..count {
                source.push_str(&format!("#[test]\nfn case_{index:02}() {{\nlet mut values = Vec::new();\nrecord_mark(&mut values);\nassert_eq!(values, vec![1]);\n}}\n"));
            }
            source.push_str("}\n");
            std::fs::write(fixture.join("src/lib.rs"), source)
                .map_err(|error| error.to_string())?;
            let diff = fixture.join("change.patch");
            std::fs::write(&diff, "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,3 @@\n pub fn record_mark(values: &mut Vec<u32>) {\n-    values.push(0);\n+    values.push(1);\n }\n")
                .map_err(|error| error.to_string())?;
            let root_arg = fixture.to_string_lossy();
            let diff_arg = diff.to_string_lossy();
            let json = run_ripr(&[
                "check", "--root", &root_arg, "--diff", &diff_arg, "--format", "json",
            ]);
            assert!(
                json.status.success(),
                "JSON count-{count}: {}",
                String::from_utf8_lossy(&json.stderr)
            );
            let report: serde_json::Value =
                serde_json::from_slice(&json.stdout).map_err(|error| error.to_string())?;
            let findings = report["findings"].as_array().ok_or("missing findings")?;
            assert_eq!(
                findings.len(),
                2,
                "the changed sink must produce both bounded families"
            );
            let mut observed_families = std::collections::BTreeSet::new();
            for finding in findings {
                let family = finding["probe"]["family"]
                    .as_str()
                    .ok_or("missing family")?;
                observed_families.insert(family);
                assert!(
                    matches!(family, "call_deletion" | "side_effect"),
                    "unexpected family {family}"
                );
                assert_eq!(finding["related_tests_total"], serde_json::json!(count));
                let rows = finding["related_tests"]
                    .as_array()
                    .ok_or("missing related rows")?;
                assert_eq!(rows.len(), count.min(8));
                for (index, row) in rows.iter().enumerate() {
                    assert_eq!(row["name"], serde_json::json!(format!("case_{index:02}")));
                    assert_eq!(row["relation_reason"], "direct_owner_call");
                    assert_eq!(row["relation_confidence"], "high");
                }
                if count == 6 {
                    classifications.insert(family.to_string(), finding["classification"].clone());
                } else {
                    assert_eq!(
                        classifications.get(family),
                        Some(&finding["classification"]),
                        "packing metadata must not change exposure"
                    );
                }
            }
            assert_eq!(
                observed_families,
                ["call_deletion", "side_effect"].into_iter().collect()
            );
            // Repeat on the same source/cache keys: classification must rebuild
            // the metadata from cached parser facts, rather than packed evidence.
            let warm = run_ripr(&[
                "check", "--root", &root_arg, "--diff", &diff_arg, "--format", "json",
            ]);
            assert!(
                warm.status.success(),
                "warm JSON count-{count}: {}",
                String::from_utf8_lossy(&warm.stderr)
            );
            let warm_report: serde_json::Value =
                serde_json::from_slice(&warm.stdout).map_err(|error| error.to_string())?;
            assert_eq!(
                warm_report["findings"], report["findings"],
                "same-key repeated findings must preserve count and semantic rows"
            );
            let human = run_ripr(&[
                "check", "--root", &root_arg, "--diff", &diff_arg, "--format", "human",
            ]);
            assert!(
                human.status.success(),
                "human count-{count}: {}",
                String::from_utf8_lossy(&human.stderr)
            );
            let human_text = String::from_utf8(human.stdout).map_err(|error| error.to_string())?;
            assert!(
                human_text.contains(&format!("1 of {count}")),
                "human total must match JSON for count-{count}: {human_text}"
            );
        }
        Ok(())
    })();
    let cleanup = std::fs::remove_dir_all(&root).map_err(|error| error.to_string());
    result.and(cleanup)
}
