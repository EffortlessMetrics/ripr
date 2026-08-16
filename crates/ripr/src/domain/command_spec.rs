//! Typed, producer-owned command descriptions.
//!
//! A [`CommandSpec`] describes a route for a consumer. It is not permission
//! to execute that route. In particular, the human display string is never
//! the source from which a consumer may reconstruct `program` or `args`.

use std::fmt;
use std::path::{Component, Path};

const MAX_DISPLAY_LENGTH: usize = 512;
const MAX_TIMEOUT_MS: u64 = 3_600_000;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandSpec {
    pub schema_version: String,
    pub command_id: String,
    pub role: CommandRole,
    pub execution_mode: CommandExecutionMode,
    pub program: String,
    pub args: Vec<String>,
    #[serde(rename = "working_directory", alias = "cwd")]
    pub cwd: String,
    pub env_set: Vec<EnvironmentAssignment>,
    pub env_passthrough: Vec<String>,
    #[serde(rename = "environment", alias = "environment_policy")]
    pub environment_policy: EnvironmentPolicy,
    pub stdin: StdinPolicy,
    pub timeout_ms: u64,
    pub cancellation: CancellationPolicy,
    #[serde(rename = "network", alias = "network_policy")]
    pub network_policy: NetworkPolicy,
    pub expected_result_parser: ExpectedResultParser,
    pub expected_exit_codes: Vec<i32>,
    pub expected_writes: Vec<String>,
    pub cost_class: CommandCostClass,
    pub platforms: Vec<CommandPlatform>,
    #[serde(rename = "human_display", alias = "display")]
    pub display: String,
    pub authority_boundary: CommandAuthorityBoundary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandRole {
    Verify,
    Receipt,
    Regeneration,
    Inspection,
    TargetedRerun,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandExecutionMode {
    Direct,
    ShellRequired,
    Manual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StdinPolicy {
    Null,
    Declared,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationPolicy {
    Allowed,
    NotAllowed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    Forbidden,
    NotRequired,
    Declared,
    Unrestricted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentPolicy {
    Clean,
    Declared,
    Inherited,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EnvironmentAssignment {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedResultParser {
    ExitCode,
    DeclaredJson,
    DeclaredText,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandCostClass {
    Unknown,
    Pilot,
    Check,
    ProjectionOnly,
    BoundedStaticAnalysis,
    FullRepoAnalysis,
    CompileOrTest,
    RuntimeMutationOrImport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandPlatform {
    Linux,
    Macos,
    Windows,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandAuthorityBoundary {
    VerificationRouteOnly,
    ReceiptRouteOnly,
    RegenerationRouteOnly,
    InspectionRouteOnly,
    TargetedRerunRouteOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandSpecValidationError {
    EmptyField(&'static str),
    EmbeddedNul(&'static str),
    InvalidSchemaVersion(String),
    InvalidRelativePath { field: &'static str, value: String },
    DisplayTooLong,
    InvalidTimeout(u64),
    EmptyExpectedExitCodes,
    EmptyPlatforms,
    InvalidEnvironmentName(String),
    AuthorityRoleMismatch,
}

impl fmt::Display for CommandSpecValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => write!(formatter, "{field} must not be empty"),
            Self::EmbeddedNul(field) => write!(formatter, "{field} must not contain NUL"),
            Self::InvalidSchemaVersion(version) => {
                write!(formatter, "unsupported command schema version {version:?}")
            }
            Self::InvalidRelativePath { field, value } => {
                write!(formatter, "{field} must be a root-relative path: {value:?}")
            }
            Self::DisplayTooLong => {
                write!(
                    formatter,
                    "display must be at most {MAX_DISPLAY_LENGTH} characters"
                )
            }
            Self::InvalidTimeout(timeout_ms) => {
                write!(
                    formatter,
                    "timeout must be between 1 and {MAX_TIMEOUT_MS} ms, got {timeout_ms}"
                )
            }
            Self::EmptyExpectedExitCodes => {
                write!(formatter, "expected_exit_codes must not be empty")
            }
            Self::EmptyPlatforms => write!(formatter, "platforms must not be empty"),
            Self::InvalidEnvironmentName(name) => {
                write!(formatter, "invalid environment variable name {name:?}")
            }
            Self::AuthorityRoleMismatch => {
                write!(formatter, "authority boundary does not match command role")
            }
        }
    }
}

impl std::error::Error for CommandSpecValidationError {}

impl CommandSpec {
    pub const SCHEMA_VERSION: &'static str = "1";

    /// Validate fields that must be true before a producer-owned route can be
    /// advertised to a machine consumer. This does not check executable
    /// availability and does not authorize process execution.
    pub fn validate(&self) -> Result<(), CommandSpecValidationError> {
        if self.schema_version != Self::SCHEMA_VERSION {
            return Err(CommandSpecValidationError::InvalidSchemaVersion(
                self.schema_version.clone(),
            ));
        }
        validate_non_empty("command_id", &self.command_id)?;
        validate_non_empty("program", &self.program)?;
        validate_non_empty("display", &self.display)?;
        validate_no_nul("command_id", &self.command_id)?;
        validate_no_nul("program", &self.program)?;
        validate_no_nul("display", &self.display)?;
        for argument in &self.args {
            validate_no_nul("args", argument)?;
        }
        validate_root_relative_path("cwd", &self.cwd)?;
        for path in &self.expected_writes {
            validate_root_relative_path("expected_writes", path)?;
        }
        if self.timeout_ms == 0 || self.timeout_ms > MAX_TIMEOUT_MS {
            return Err(CommandSpecValidationError::InvalidTimeout(self.timeout_ms));
        }
        if self.expected_exit_codes.is_empty() {
            return Err(CommandSpecValidationError::EmptyExpectedExitCodes);
        }
        if self.platforms.is_empty() {
            return Err(CommandSpecValidationError::EmptyPlatforms);
        }
        for assignment in &self.env_set {
            validate_environment_name(&assignment.name)?;
            validate_no_nul("environment value", &assignment.value)?;
        }
        for name in &self.env_passthrough {
            validate_environment_name(name)?;
        }
        if !authority_matches_role(self.role, self.authority_boundary) {
            return Err(CommandSpecValidationError::AuthorityRoleMismatch);
        }
        Ok(())
    }
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), CommandSpecValidationError> {
    if value.trim().is_empty() {
        return Err(CommandSpecValidationError::EmptyField(field));
    }
    if field == "display" && value.chars().count() > MAX_DISPLAY_LENGTH {
        return Err(CommandSpecValidationError::DisplayTooLong);
    }
    Ok(())
}

fn validate_no_nul(field: &'static str, value: &str) -> Result<(), CommandSpecValidationError> {
    if value.contains('\0') {
        Err(CommandSpecValidationError::EmbeddedNul(field))
    } else {
        Ok(())
    }
}

fn validate_root_relative_path(
    field: &'static str,
    value: &str,
) -> Result<(), CommandSpecValidationError> {
    validate_no_nul(field, value)?;
    if value.trim().is_empty() {
        return Err(CommandSpecValidationError::EmptyField(field));
    }
    let path = Path::new(value);
    let has_drive_prefix = value.as_bytes().get(1) == Some(&b':');
    let has_host_independent_escape = value.starts_with('/')
        || value.starts_with('\\')
        || value.split(['/', '\\']).any(|component| component == "..");
    let safe = !path.is_absolute()
        && !has_drive_prefix
        && !has_host_independent_escape
        && path.components().all(|component| {
            !matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        });
    if safe {
        Ok(())
    } else {
        Err(CommandSpecValidationError::InvalidRelativePath {
            field,
            value: value.to_string(),
        })
    }
}

fn validate_environment_name(name: &str) -> Result<(), CommandSpecValidationError> {
    if name.is_empty()
        || name.contains('\0')
        || name.contains('=')
        || name.chars().any(char::is_whitespace)
    {
        Err(CommandSpecValidationError::InvalidEnvironmentName(
            name.to_string(),
        ))
    } else {
        Ok(())
    }
}

fn authority_matches_role(role: CommandRole, boundary: CommandAuthorityBoundary) -> bool {
    matches!(
        (role, boundary),
        (
            CommandRole::Verify,
            CommandAuthorityBoundary::VerificationRouteOnly
        ) | (
            CommandRole::Receipt,
            CommandAuthorityBoundary::ReceiptRouteOnly
        ) | (
            CommandRole::Regeneration,
            CommandAuthorityBoundary::RegenerationRouteOnly
        ) | (
            CommandRole::Inspection,
            CommandAuthorityBoundary::InspectionRouteOnly
        ) | (
            CommandRole::TargetedRerun,
            CommandAuthorityBoundary::TargetedRerunRouteOnly
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verify_spec() -> CommandSpec {
        CommandSpec {
            schema_version: "1".to_string(),
            command_id: "cmd:verify:pricing".to_string(),
            role: CommandRole::Verify,
            execution_mode: CommandExecutionMode::Direct,
            program: "cargo".to_string(),
            args: vec![
                "test".to_string(),
                "-p".to_string(),
                "pricing crate".to_string(),
                "--".to_string(),
                "--exact".to_string(),
            ],
            cwd: ".".to_string(),
            env_set: vec![EnvironmentAssignment {
                name: "RIPR_FIXTURE".to_string(),
                value: "boundary value".to_string(),
            }],
            env_passthrough: vec!["CARGO_HOME".to_string()],
            environment_policy: EnvironmentPolicy::Declared,
            stdin: StdinPolicy::Null,
            timeout_ms: 120_000,
            cancellation: CancellationPolicy::Allowed,
            network_policy: NetworkPolicy::Forbidden,
            expected_result_parser: ExpectedResultParser::ExitCode,
            expected_exit_codes: vec![0],
            expected_writes: vec!["target/**".to_string()],
            cost_class: CommandCostClass::CompileOrTest,
            platforms: vec![CommandPlatform::Linux, CommandPlatform::Windows],
            display: "cargo test -p 'pricing crate' -- --exact".to_string(),
            authority_boundary: CommandAuthorityBoundary::VerificationRouteOnly,
        }
    }

    #[test]
    fn invalid_paths_and_program_values_fail_closed() -> Result<(), String> {
        let mut cases = Vec::new();
        let mut traversal = verify_spec();
        traversal.cwd = "../outside".to_string();
        cases.push(traversal.validate());
        let mut absolute = verify_spec();
        let drive_letter = char::from(b'C');
        absolute.expected_writes = vec![format!("{drive_letter}:/outside")];
        cases.push(absolute.validate());
        let mut nul_cwd = verify_spec();
        nul_cwd.cwd = "work\0dir".to_string();
        cases.push(nul_cwd.validate());
        let mut nul_write = verify_spec();
        nul_write.expected_writes = vec!["out\0/**".to_string()];
        cases.push(nul_write.validate());
        let mut blank_program = verify_spec();
        blank_program.program = " ".to_string();
        cases.push(blank_program.validate());
        let mut nul_argument = verify_spec();
        nul_argument.args.push("bad\0arg".to_string());
        cases.push(nul_argument.validate());
        if cases.iter().any(Result::is_ok) {
            return Err("malformed command spec was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn shell_and_manual_modes_remain_explicit() -> Result<(), String> {
        let mut shell = verify_spec();
        shell.execution_mode = CommandExecutionMode::ShellRequired;
        shell.validate().map_err(|error| error.to_string())?;
        let mut manual = verify_spec();
        manual.execution_mode = CommandExecutionMode::Manual;
        manual.validate().map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn mismatched_authority_is_rejected() -> Result<(), String> {
        let mut receipt = verify_spec();
        receipt.role = CommandRole::Receipt;
        if !matches!(
            receipt.validate(),
            Err(CommandSpecValidationError::AuthorityRoleMismatch)
        ) {
            return Err("a verify authority was accepted for a receipt command".to_string());
        }
        Ok(())
    }
}
