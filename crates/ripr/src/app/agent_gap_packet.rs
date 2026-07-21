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
    let records =
        output::gap_decision_ledger::parse_gap_records_json(&contents).map_err(|err| {
            format!(
                "agent packet --gap-ledger {} is invalid: {err}",
                gap_ledger.display()
            )
        })?;
    let record = records
        .iter()
        .find(|record| record.gap_id == gap_id || record.canonical_gap_id == gap_id)
        .ok_or_else(|| format!("agent packet gap_id {gap_id} was not found"))?;
    let (causal_projection, causal_projection_warning) =
        crate::app::causal_projection::CausalDeltaArtifact::load_optional(root);
    if let Some(warning) = causal_projection_warning {
        eprintln!("ripr agent packet: {warning}");
    }
    output::agent_seam_packets::render_agent_gap_record_packet_json_with_causal(
        &output::outcome::display_path(gap_ledger),
        record,
        causal_projection.as_ref(),
    )
    .map_err(|err| format!("agent packet gap_id {gap_id} {err}"))
}
