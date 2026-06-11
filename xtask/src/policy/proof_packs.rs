pub(crate) fn check_proof_packs() -> Result<(), String> {
    crate::check_proof_packs_impl()
}

/// One `[[pack]]` entry from `policy/proof-packs.toml`, parsed into the shape
/// shared by `check-proof-packs` validation and the `proof route` report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProofPack {
    pub(crate) id: String,
    pub(crate) line: usize,
    pub(crate) paths: Vec<String>,
    pub(crate) required_commands: Vec<String>,
    pub(crate) advisory_commands: Vec<String>,
    pub(crate) ci_lane: Option<String>,
    pub(crate) never_routed: bool,
}

/// Extract `[[pack]]` tables from the proof-pack manifest document, recording
/// structural violations: missing or malformed fields, empty path or required
/// command lists, and non-bare `never_routed` values.
pub(crate) fn parse_proof_packs(
    manifest: &crate::CiLedgerDocument,
    violations: &mut Vec<String>,
) -> Vec<ProofPack> {
    let path = crate::PROOF_PACK_MANIFEST_PATH;
    let mut packs = Vec::new();
    for table in crate::ci_tables(manifest, "pack") {
        let Some(id) = crate::ci_required_table_id(path, table, "id", "proof pack", violations)
        else {
            continue;
        };

        let paths = crate::ci_required_table_array(path, table, "paths", violations);
        if let Some(paths) = &paths
            && paths.is_empty()
        {
            violations.push(format!(
                "{path}:{} proof pack `{id}` must cover at least one path",
                table.line
            ));
        }

        let required_commands =
            crate::ci_required_table_array(path, table, "required_commands", violations);
        if let Some(required) = &required_commands
            && required.is_empty()
        {
            violations.push(format!(
                "{path}:{} proof pack `{id}` must name at least one required command",
                table.line
            ));
        }

        let advisory_commands =
            crate::ci_required_table_array(path, table, "advisory_commands", violations);

        let ci_lane = crate::ci_required_non_empty_table_string(path, table, "ci_lane", violations);
        crate::ci_required_non_empty_table_string(path, table, "proves", violations);
        crate::ci_required_non_empty_table_string(path, table, "does_not_prove", violations);

        let never_routed_value = table.values.get("never_routed");
        if let Some(value) = never_routed_value
            && value.raw != "true"
            && value.raw != "false"
        {
            violations.push(format!(
                "{path}:{} proof pack `{id}` field `never_routed` must be a bare `true` or `false`",
                value.line
            ));
        }
        let never_routed = never_routed_value.map(|value| value.raw.as_str()) == Some("true");

        packs.push(ProofPack {
            id,
            line: table.line,
            paths: paths.unwrap_or_default(),
            required_commands: required_commands.unwrap_or_default(),
            advisory_commands: advisory_commands.unwrap_or_default(),
            ci_lane,
            never_routed,
        });
    }
    packs
}

/// Load and parse `policy/proof-packs.toml` for proof routing. A structurally
/// invalid manifest is an error; `cargo xtask check-proof-packs` explains it.
pub(crate) fn load_proof_packs() -> Result<Vec<ProofPack>, String> {
    let mut violations = Vec::new();
    let document = crate::read_ci_ledger_document(crate::PROOF_PACK_MANIFEST_PATH, &mut violations);
    let packs = document
        .as_ref()
        .map(|document| parse_proof_packs(document, &mut violations))
        .unwrap_or_default();
    if violations.is_empty() {
        Ok(packs)
    } else {
        Err(format!(
            "{} is not usable for proof routing:\n{}\nrun `cargo xtask check-proof-packs` for the full report",
            crate::PROOF_PACK_MANIFEST_PATH,
            violations
                .iter()
                .map(|violation| format!("- {violation}"))
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }
}
