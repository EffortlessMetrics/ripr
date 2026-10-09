use super::*;

#[test]
fn direct_manifest_caps_proof_inputs_and_aggregate_bytes() -> Result<(), String> {
    let mut fixture = Fixture::new()?;
    let original = fixture.document["qualification"]["proof_inputs"].clone();
    let input = original[0].clone();
    fixture.document["qualification"]["proof_inputs"] = json!(vec![input; MAX_PROOF_INPUTS + 1]);
    refusal(
        fixture.admit(&fixture.write()?),
        "proof_inputs exceeds 64-entry budget",
    )?;
    fixture.document["qualification"]["proof_inputs"] = original;
    // Four individually valid 16 MiB files exceed the 64 MiB total once the
    // manifest and required owner packets are counted. Sparse files keep this
    // fixture cheap on disk; the refused fourth file is never retained.
    let zeros = vec![0u8; 16 * 1024 * 1024];
    let hash = digest(&zeros);
    drop(zeros);
    for index in 0..4 {
        let name = format!("large-{index}.bin");
        std::fs::File::create(fixture.root.join(&name))
            .and_then(|file| file.set_len(16 * 1024 * 1024))
            .map_err(|error| error.to_string())?;
        fixture.document["qualification"]["proof_inputs"]
            .as_array_mut()
            .ok_or("fixture proof inputs missing")?
            .push(json!({"owner_issue": 4510, "path": name, "sha256": hash}));
    }
    refusal(fixture.admit(&fixture.write()?), "64 MiB aggregate")
}

#[test]
fn owned_reads_require_regular_contained_files_and_available_budget() -> Result<(), String> {
    let fixture = Fixture::new()?;
    for (path, budget, reason) in [
        ("selection.json", 0, "aggregate retained-byte budget"),
        ("selection.json", 1, "byte budget"),
        (
            "../escape",
            MAX_RETAINED_BYTES,
            "ordinary root-relative path",
        ),
    ] {
        match read_owned(&fixture.root, path, budget) {
            Err(error) if error.contains(reason) => (),
            result => return Err(format!("wrong bounded read result for {path}: {result:?}")),
        }
    }
    std::fs::create_dir(fixture.root.join("directory")).map_err(|error| error.to_string())?;
    match read_owned(&fixture.root, "directory", MAX_RETAINED_BYTES) {
        Err(error) if error.contains("not a regular file") => (),
        result => return Err(format!("non-file admission: {result:?}")),
    }
    let bytes = read_owned(&fixture.root, "selection.json", MAX_RETAINED_BYTES)?;
    if bytes != b"{\"synthetic\":\"selection\"}" {
        return Err("ordinary snapshot changed retained bytes".to_string());
    }
    Ok(())
}
