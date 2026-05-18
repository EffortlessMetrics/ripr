use crate::app::Mode;
use std::path::{Path, PathBuf};

pub(crate) const PILOT_SUMMARY_SCHEMA_VERSION: &str = "0.2";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PilotArtifacts {
    pub(crate) repo_exposure_json: PathBuf,
    pub(crate) repo_exposure_md: PathBuf,
    pub(crate) agent_seam_packets_json: PathBuf,
    pub(crate) pilot_summary_json: PathBuf,
    pub(crate) pilot_summary_md: PathBuf,
}

#[derive(Clone, Copy)]
pub(crate) struct PilotSummaryContext<'a> {
    pub(crate) root: &'a Path,
    pub(crate) mode: &'a Mode,
    pub(crate) config_path: Option<&'a Path>,
    pub(crate) max_seams: usize,
    pub(crate) timeout_ms: u64,
    pub(crate) artifacts: &'a PilotArtifacts,
}
