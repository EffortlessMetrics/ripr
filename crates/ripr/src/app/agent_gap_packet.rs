use crate::output;
use std::path::Path;

/// Render one GapRecord-backed agent packet after adapting the CLI request.
/// Artifact loading and packet orchestration belong to the application layer;
/// the CLI only supplies the requested root, ledger, and gap identity.
pub(crate) fn render_agent_packet_from_gap_ledger(
    root: &Path,
    gap_ledger: &Path,
    gap_id: &str,
) -> Result<String, String> {
    let contents = std::fs::read_to_string(gap_ledger).map_err(|err| {
        format!(
            "agent packet --gap-ledger {} is invalid: read failed: {err}",
            gap_ledger.display()
        )
    })?;
    let source =
        output::gap_decision_ledger_live::parse_gap_record_source_with_provenance_json(&contents)
            .map_err(|err| {
            format!(
                "agent packet --gap-ledger {} is invalid: {err}",
                gap_ledger.display()
            )
        })?;
    let record = source
        .records
        .iter()
        .find(|record| record.gap_id == gap_id || record.canonical_gap_id == gap_id)
        .ok_or_else(|| format!("agent packet gap_id {gap_id} was not found"))?;
    let currentness = output::agent_seam_packets::evaluate_gap_record_source_currentness(
        output::agent_seam_packets::GapRecordSourceInput {
            root,
            gap_ledger_path: gap_ledger,
            ledger_root: source.root.as_deref(),
            source_kind: source.source_kind.as_deref(),
            records_path: source.records_path.as_deref(),
            source_identity_error: source.source_identity_error.as_deref(),
            records: &source.records,
        },
    );
    let (causal_projection, causal_projection_warning) =
        crate::app::causal_projection::CausalDeltaArtifact::load_optional(root);
    if let Some(warning) = causal_projection_warning {
        eprintln!("ripr agent packet: {warning}");
    }
    output::agent_seam_packets::render_agent_gap_record_packet_json_with_live_currentness(
        &output::outcome::display_path(gap_ledger),
        record,
        causal_projection.as_ref(),
        &currentness,
    )
    .map_err(|err| format!("agent packet gap_id {gap_id} {err}"))
}
