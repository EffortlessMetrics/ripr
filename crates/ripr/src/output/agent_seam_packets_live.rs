#[path = "agent_seam_packets_live/currentness.rs"]
mod currentness;
#[path = "agent_seam_packets_live/render.rs"]
mod render;

pub(crate) use crate::output::agent_seam_packets_legacy::*;
pub(crate) use currentness::{GapRecordSourceInput, evaluate_gap_record_source_currentness};
pub(crate) use render::{
    render_agent_gap_record_packet_json_with_live_currentness,
    render_agent_gap_record_queue_json_with_currentness,
};
