fn vsix_inventory_entry(name: &str, size: u64) -> super::VsixEntry {
    super::VsixEntry {
        name: name.to_string(),
        size,
        compressed_size: size / 4,
    }
}

#[test]
fn vsix_inventory_rejects_workspace_build_output() -> Result<(), String> {
    let approved = vec![
        vsix_inventory_entry("extension/package.json", 2),
        vsix_inventory_entry("extension/distribution.json", 20),
        vsix_inventory_entry("extension/out/src/client.js", 100),
        vsix_inventory_entry("extension/node_modules/dep/target/index.js", 100),
    ];
    super::check_vsix_inventory(
        &approved,
        super::VSIX_MAX_ENTRIES,
        super::VSIX_MAX_UNCOMPRESSED_BYTES,
    )?;

    for name in [
        "extension/target/debug/ripr-1775-sentinel.bin",
        "extension/out/libripr-0123.rlib",
        "extension/out/libripr-0123.rmeta",
        "extension/build/.fingerprint/ripr-0123/lib-ripr",
        "extension/build/incremental/ripr-0123/query-cache.bin",
    ] {
        let mut entries = approved.clone();
        entries.push(vsix_inventory_entry(name, 10));
        let Err(error) = super::check_vsix_inventory(
            &entries,
            super::VSIX_MAX_ENTRIES,
            super::VSIX_MAX_UNCOMPRESSED_BYTES,
        ) else {
            return Err(format!("{name} must be rejected as workspace build output"));
        };
        assert!(error.contains(name), "{error}");
    }
    Ok(())
}

#[test]
fn vsix_inventory_bounds_entry_count_and_unpacked_size() -> Result<(), String> {
    let entries = vec![
        vsix_inventory_entry("extension/package.json", 10),
        vsix_inventory_entry("extension/out/a.js", 10),
        vsix_inventory_entry("extension/out/b.js", 10),
    ];
    super::check_vsix_inventory(&entries, 3, 30)?;

    let Err(count) = super::check_vsix_inventory(&entries, 2, 30) else {
        return Err("an entry count above the bound must be rejected".to_string());
    };
    assert!(
        count.contains("3 entries, above the 2-entry bound"),
        "{count}"
    );

    let Err(size) = super::check_vsix_inventory(&entries, 3, 29) else {
        return Err("an unpacked size above the bound must be rejected".to_string());
    };
    assert!(
        size.contains("30 bytes, above the 29-byte bound"),
        "{size}"
    );

    const { assert!(super::VSIX_MAX_ENTRIES > 410 && super::VSIX_MAX_ENTRIES < 2_805) };
    const {
        assert!(
            super::VSIX_MAX_UNCOMPRESSED_BYTES > 3 * 1024 * 1024
                && super::VSIX_MAX_UNCOMPRESSED_BYTES < 2_300 * 1024 * 1024
        )
    };
    Ok(())
}
