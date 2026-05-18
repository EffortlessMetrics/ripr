//! Render the first-run pilot packet summary.
//!
//! `ripr pilot` joins existing repo-exposure and agent packet artifacts with
//! one small operator summary. It does not change classification semantics or
//! run additional analysis.

mod commands;
mod ranking;
mod render;
mod types;

pub(crate) use render::{
    render_pilot_summary_json, render_pilot_summary_md, render_pilot_terminal,
    render_pilot_timeout_summary_json, render_pilot_timeout_summary_md,
    render_pilot_timeout_terminal,
};
pub(crate) use types::{PILOT_SUMMARY_SCHEMA_VERSION, PilotArtifacts, PilotSummaryContext};

#[cfg(test)]
mod tests;
