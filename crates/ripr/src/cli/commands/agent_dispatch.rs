use crate::cli::agent::AgentCommand;
use crate::cli::help;

pub(super) fn run_agent_help_command(command: &AgentCommand) -> Option<Result<(), String>> {
    match command {
        AgentCommand::Help => Some(print_help(help::print_agent_help)),
        AgentCommand::StartHelp => Some(print_help(help::print_agent_start_help)),
        AgentCommand::BriefHelp => Some(print_help(help::print_agent_brief_help)),
        AgentCommand::PacketHelp => Some(print_help(help::print_agent_packet_help)),
        AgentCommand::VerifyHelp => Some(print_help(help::print_agent_verify_help)),
        AgentCommand::VerifyExecuteHelp => Some(print_help(help::print_agent_verify_execute_help)),
        AgentCommand::ReceiptHelp => Some(print_help(help::print_agent_receipt_help)),
        AgentCommand::StatusHelp => Some(print_help(help::print_agent_status_help)),
        AgentCommand::ReviewSummaryHelp => Some(print_help(help::print_agent_review_summary_help)),
        AgentCommand::RepairHelp => Some(print_help(help::print_agent_repair_help)),
        _ => None,
    }
}

fn print_help(printer: fn()) -> Result<(), String> {
    printer();
    Ok(())
}
