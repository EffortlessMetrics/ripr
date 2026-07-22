//! Provenance-bound results from an explicitly executed [`CommandSpec`].
//!
//! This type records process observations; it does not execute commands and
//! does not decide whether static RIPR evidence improved. Consumers must
//! validate a result against the producer-owned command spec and the exact
//! repository identities they observed around the run.

use super::{CommandAuthorityBoundary, CommandRole, CommandSpec};
use sha2::{Digest, Sha256};
use std::fmt;

const SHA256_PREFIX: &str = "sha256:";
const SHA256_HEX_LENGTH: usize = 64;

/// The schema version for [`VerificationExecutionResultV1`].
pub const VERIFICATION_EXECUTION_RESULT_SCHEMA_VERSION: &str = "1";

/// The process-level disposition observed by a bounded command runner.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationProcessDispositionV1 {
    Completed,
    FailedToStart,
    Cancelled,
    TimedOut,
    OutputLimitExceeded,
}

/// The repository currentness classification attached to an execution result.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationCurrentnessV1 {
    Current,
    DirtyWorktree,
    HistoricalNoncurrent,
    Unavailable,
}

/// A bounded observation from one explicitly executed, producer-owned command.
///
/// A completed process is not proof that a static gap closed, that tests are
/// sufficient, or that runtime mutation was confirmed. The result only binds
/// the process observation to its command specification and repository state.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VerificationExecutionResultV1 {
    pub schema_version: String,
    pub root_identity: String,
    pub head_before: String,
    pub head_after: String,
    pub command_spec_sha256: String,
    pub process_disposition: VerificationProcessDispositionV1,
    pub exit_status: Option<i32>,
    pub stdout_sha256: String,
    pub stderr_sha256: String,
    pub currentness: VerificationCurrentnessV1,
}

/// Validation failures for a transported execution result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerificationExecutionResultValidationError {
    EmptyField(&'static str),
    InvalidSchemaVersion(String),
    InvalidHead {
        field: &'static str,
        value: String,
    },
    InvalidSha256 {
        field: &'static str,
        value: String,
    },
    RootIdentityMismatch {
        expected: String,
        actual: String,
    },
    HeadMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },
    CommandSpecInvalid(String),
    CommandSpecDigestMismatch {
        expected: String,
        actual: String,
    },
    NonVerificationCommandRole {
        role: CommandRole,
        authority_boundary: CommandAuthorityBoundary,
    },
    MissingExitStatus,
    UnexpectedExitStatus,
    CurrentnessUnavailable,
    CurrentnessHeadMismatch {
        before: String,
        after: String,
    },
}

impl fmt::Display for VerificationExecutionResultValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => write!(formatter, "{field} must not be empty"),
            Self::InvalidSchemaVersion(version) => {
                write!(
                    formatter,
                    "unsupported verification result schema version {version:?}"
                )
            }
            Self::InvalidHead { field, value } => {
                write!(
                    formatter,
                    "{field} must be a full 40-character Git SHA, got {value:?}"
                )
            }
            Self::InvalidSha256 { field, value } => {
                write!(
                    formatter,
                    "{field} must be sha256:<64 lowercase hex>, got {value:?}"
                )
            }
            Self::RootIdentityMismatch { expected, actual } => write!(
                formatter,
                "verification result root identity mismatch: expected {expected:?}, got {actual:?}"
            ),
            Self::HeadMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "verification result {field} mismatch: expected {expected:?}, got {actual:?}"
            ),
            Self::CommandSpecInvalid(error) => {
                write!(
                    formatter,
                    "verification result command spec is invalid: {error}"
                )
            }
            Self::CommandSpecDigestMismatch { expected, actual } => write!(
                formatter,
                "verification result command spec digest mismatch: expected {expected}, got {actual}"
            ),
            Self::NonVerificationCommandRole {
                role,
                authority_boundary,
            } => write!(
                formatter,
                "verification result requires a verify command with verification_route_only, got role {role:?} and authority {authority_boundary:?}"
            ),
            Self::MissingExitStatus => {
                write!(
                    formatter,
                    "completed verification result is missing exit_status"
                )
            }
            Self::UnexpectedExitStatus => write!(
                formatter,
                "non-completed verification result must not carry exit_status"
            ),
            Self::CurrentnessUnavailable => write!(
                formatter,
                "verification result currentness is unavailable and cannot be validated as current"
            ),
            Self::CurrentnessHeadMismatch { before, after } => write!(
                formatter,
                "current verification result changed HEAD during execution: before {before}, after {after}"
            ),
        }
    }
}

impl std::error::Error for VerificationExecutionResultValidationError {}

impl VerificationExecutionResultV1 {
    pub const SCHEMA_VERSION: &'static str = VERIFICATION_EXECUTION_RESULT_SCHEMA_VERSION;

    /// Validate the result against the exact execution context observed by a
    /// producer. This is the only domain entry point for accepting a result.
    pub fn validate_against(
        &self,
        command_spec: &CommandSpec,
        expected_root_identity: &str,
        expected_head_before: &str,
        expected_head_after: &str,
    ) -> Result<(), VerificationExecutionResultValidationError> {
        if self.schema_version != Self::SCHEMA_VERSION {
            return Err(
                VerificationExecutionResultValidationError::InvalidSchemaVersion(
                    self.schema_version.clone(),
                ),
            );
        }
        validate_non_empty("root_identity", &self.root_identity)?;
        validate_non_empty("command_spec_sha256", &self.command_spec_sha256)?;
        validate_non_empty("stdout_sha256", &self.stdout_sha256)?;
        validate_non_empty("stderr_sha256", &self.stderr_sha256)?;
        validate_head("head_before", &self.head_before)?;
        validate_head("head_after", &self.head_after)?;
        validate_sha256("command_spec_sha256", &self.command_spec_sha256)?;
        validate_sha256("stdout_sha256", &self.stdout_sha256)?;
        validate_sha256("stderr_sha256", &self.stderr_sha256)?;
        validate_non_empty("expected_root_identity", expected_root_identity)?;
        validate_head("expected_head_before", expected_head_before)?;
        validate_head("expected_head_after", expected_head_after)?;

        if self.root_identity != expected_root_identity {
            return Err(
                VerificationExecutionResultValidationError::RootIdentityMismatch {
                    expected: expected_root_identity.to_string(),
                    actual: self.root_identity.clone(),
                },
            );
        }
        if self.head_before != expected_head_before {
            return Err(VerificationExecutionResultValidationError::HeadMismatch {
                field: "head_before",
                expected: expected_head_before.to_string(),
                actual: self.head_before.clone(),
            });
        }
        if self.head_after != expected_head_after {
            return Err(VerificationExecutionResultValidationError::HeadMismatch {
                field: "head_after",
                expected: expected_head_after.to_string(),
                actual: self.head_after.clone(),
            });
        }

        command_spec.validate().map_err(|error| {
            VerificationExecutionResultValidationError::CommandSpecInvalid(error.to_string())
        })?;
        if command_spec.role != CommandRole::Verify
            || command_spec.authority_boundary != CommandAuthorityBoundary::VerificationRouteOnly
        {
            return Err(
                VerificationExecutionResultValidationError::NonVerificationCommandRole {
                    role: command_spec.role,
                    authority_boundary: command_spec.authority_boundary,
                },
            );
        }
        let expected_digest = command_spec_sha256(command_spec)?;
        if self.command_spec_sha256 != expected_digest {
            return Err(
                VerificationExecutionResultValidationError::CommandSpecDigestMismatch {
                    expected: expected_digest,
                    actual: self.command_spec_sha256.clone(),
                },
            );
        }

        match self.process_disposition {
            VerificationProcessDispositionV1::Completed if self.exit_status.is_none() => {
                Err(VerificationExecutionResultValidationError::MissingExitStatus)
            }
            VerificationProcessDispositionV1::Completed => Ok(()),
            VerificationProcessDispositionV1::FailedToStart
            | VerificationProcessDispositionV1::Cancelled
            | VerificationProcessDispositionV1::TimedOut
            | VerificationProcessDispositionV1::OutputLimitExceeded
                if self.exit_status.is_some() =>
            {
                Err(VerificationExecutionResultValidationError::UnexpectedExitStatus)
            }
            VerificationProcessDispositionV1::FailedToStart
            | VerificationProcessDispositionV1::Cancelled
            | VerificationProcessDispositionV1::TimedOut
            | VerificationProcessDispositionV1::OutputLimitExceeded => Ok(()),
        }?;

        if self.currentness == VerificationCurrentnessV1::Unavailable {
            return Err(VerificationExecutionResultValidationError::CurrentnessUnavailable);
        }
        if matches!(
            self.currentness,
            VerificationCurrentnessV1::Current | VerificationCurrentnessV1::DirtyWorktree
        ) && self.head_before != self.head_after
        {
            return Err(
                VerificationExecutionResultValidationError::CurrentnessHeadMismatch {
                    before: self.head_before.clone(),
                    after: self.head_after.clone(),
                },
            );
        }
        Ok(())
    }
}

/// Hash the canonical serialized command specification used by the result
/// binding. The human display string is included only as ordinary typed data;
/// consumers never reconstruct argv from it.
pub fn command_spec_sha256(
    command_spec: &CommandSpec,
) -> Result<String, VerificationExecutionResultValidationError> {
    let bytes = serde_json::to_vec(command_spec).map_err(|error| {
        VerificationExecutionResultValidationError::CommandSpecInvalid(error.to_string())
    })?;
    Ok(sha256_bytes(&bytes))
}

fn validate_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), VerificationExecutionResultValidationError> {
    if value.trim().is_empty() {
        Err(VerificationExecutionResultValidationError::EmptyField(
            field,
        ))
    } else {
        Ok(())
    }
}

fn validate_head(
    field: &'static str,
    value: &str,
) -> Result<(), VerificationExecutionResultValidationError> {
    if value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(VerificationExecutionResultValidationError::InvalidHead {
            field,
            value: value.to_string(),
        })
    }
}

fn validate_sha256(
    field: &'static str,
    value: &str,
) -> Result<(), VerificationExecutionResultValidationError> {
    let digest = value.strip_prefix(SHA256_PREFIX).unwrap_or_default();
    if digest.len() == SHA256_HEX_LENGTH
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(VerificationExecutionResultValidationError::InvalidSha256 {
            field,
            value: value.to_string(),
        })
    }
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CancellationPolicy, CommandAuthorityBoundary, CommandCostClass, CommandExecutionMode,
        CommandPlatform, CommandRole, EnvironmentPolicy, ExpectedResultParser, NetworkPolicy,
        StdinPolicy,
    };

    const HEAD_BEFORE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HEAD_AFTER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const ROOT: &str = "root:fixture";
    const ZERO_DIGEST: &str =
        "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    fn command_spec() -> CommandSpec {
        CommandSpec {
            schema_version: CommandSpec::SCHEMA_VERSION.to_string(),
            command_id: "ripr:test:verification".to_string(),
            role: CommandRole::Verify,
            execution_mode: CommandExecutionMode::Direct,
            program: "cargo".to_string(),
            args: vec!["test".to_string(), "boundary".to_string()],
            cwd: ".".to_string(),
            env_set: Vec::new(),
            env_passthrough: Vec::new(),
            environment_policy: EnvironmentPolicy::Clean,
            stdin: StdinPolicy::Null,
            timeout_ms: 60_000,
            cancellation: CancellationPolicy::Allowed,
            network_policy: NetworkPolicy::Forbidden,
            expected_result_parser: ExpectedResultParser::ExitCode,
            expected_exit_codes: vec![0],
            expected_writes: Vec::new(),
            cost_class: CommandCostClass::CompileOrTest,
            platforms: vec![CommandPlatform::Windows],
            display: "cargo test boundary".to_string(),
            authority_boundary: CommandAuthorityBoundary::VerificationRouteOnly,
        }
    }

    fn result(spec: &CommandSpec) -> Result<VerificationExecutionResultV1, String> {
        Ok(VerificationExecutionResultV1 {
            schema_version: VerificationExecutionResultV1::SCHEMA_VERSION.to_string(),
            root_identity: ROOT.to_string(),
            head_before: HEAD_BEFORE.to_string(),
            head_after: HEAD_AFTER.to_string(),
            command_spec_sha256: command_spec_sha256(spec).map_err(|error| error.to_string())?,
            process_disposition: VerificationProcessDispositionV1::Completed,
            exit_status: Some(0),
            stdout_sha256: ZERO_DIGEST.to_string(),
            stderr_sha256: ZERO_DIGEST.to_string(),
            currentness: VerificationCurrentnessV1::Current,
        })
    }

    #[test]
    fn valid_result_round_trips_and_validates_against_exact_context() -> Result<(), String> {
        let spec = command_spec();
        let result = result(&spec)?;
        let encoded = serde_json::to_string(&result).map_err(|error| error.to_string())?;
        let decoded: VerificationExecutionResultV1 =
            serde_json::from_str(&encoded).map_err(|error| error.to_string())?;
        decoded
            .validate_against(&spec, ROOT, HEAD_BEFORE, HEAD_AFTER)
            .map_err(|error| error.to_string())
    }

    #[test]
    fn wrong_root_and_command_digest_fail_closed() -> Result<(), String> {
        let spec = command_spec();
        let mut wrong_root = result(&spec)?;
        wrong_root.root_identity = "root:other".to_string();
        if !matches!(
            wrong_root.validate_against(&spec, ROOT, HEAD_BEFORE, HEAD_AFTER),
            Err(VerificationExecutionResultValidationError::RootIdentityMismatch { .. })
        ) {
            return Err("wrong repository root was accepted".to_string());
        }

        let mut wrong_digest = result(&spec)?;
        wrong_digest.command_spec_sha256 = ZERO_DIGEST.to_string();
        if !matches!(
            wrong_digest.validate_against(&spec, ROOT, HEAD_BEFORE, HEAD_AFTER),
            Err(VerificationExecutionResultValidationError::CommandSpecDigestMismatch { .. })
        ) {
            return Err("fabricated command-spec digest was accepted".to_string());
        }

        let mut receipt_spec = command_spec();
        receipt_spec.role = CommandRole::Receipt;
        receipt_spec.authority_boundary = CommandAuthorityBoundary::ReceiptRouteOnly;
        if !matches!(
            result(&spec)?.validate_against(&receipt_spec, ROOT, HEAD_BEFORE, HEAD_AFTER),
            Err(VerificationExecutionResultValidationError::NonVerificationCommandRole { .. })
        ) {
            return Err("receipt command was accepted as verification evidence".to_string());
        }
        Ok(())
    }

    #[test]
    fn malformed_commitments_and_heads_fail_closed() -> Result<(), String> {
        let spec = command_spec();
        let mut malformed = result(&spec)?;
        malformed.head_before = format!("{}A", "a".repeat(39));
        if !matches!(
            malformed.validate_against(&spec, ROOT, HEAD_BEFORE, HEAD_AFTER),
            Err(VerificationExecutionResultValidationError::InvalidHead { .. })
        ) {
            return Err("malformed HEAD was accepted".to_string());
        }

        let mut malformed_output = result(&spec)?;
        malformed_output.stdout_sha256 = "sha256:UPPER".to_string();
        if !matches!(
            malformed_output.validate_against(&spec, ROOT, HEAD_BEFORE, HEAD_AFTER),
            Err(VerificationExecutionResultValidationError::InvalidSha256 { .. })
        ) {
            return Err("malformed output commitment was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn impossible_process_and_currentness_states_fail_closed() -> Result<(), String> {
        let spec = command_spec();
        let mut missing_status = result(&spec)?;
        missing_status.exit_status = None;
        if !matches!(
            missing_status.validate_against(&spec, ROOT, HEAD_BEFORE, HEAD_AFTER),
            Err(VerificationExecutionResultValidationError::MissingExitStatus)
        ) {
            return Err("completed result without exit status was accepted".to_string());
        }

        let mut timed_out_status = result(&spec)?;
        timed_out_status.process_disposition = VerificationProcessDispositionV1::TimedOut;
        if !matches!(
            timed_out_status.validate_against(&spec, ROOT, HEAD_BEFORE, HEAD_AFTER),
            Err(VerificationExecutionResultValidationError::UnexpectedExitStatus)
        ) {
            return Err("timed-out result with exit status was accepted".to_string());
        }

        let mut changed_head = result(&spec)?;
        changed_head.head_after = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
        if !matches!(
            changed_head.validate_against(
                &spec,
                ROOT,
                HEAD_BEFORE,
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ),
            Err(VerificationExecutionResultValidationError::CurrentnessHeadMismatch { .. })
        ) {
            return Err("current result with changed HEAD was accepted".to_string());
        }

        let mut unavailable = result(&spec)?;
        unavailable.currentness = VerificationCurrentnessV1::Unavailable;
        if !matches!(
            unavailable.validate_against(&spec, ROOT, HEAD_BEFORE, HEAD_AFTER),
            Err(VerificationExecutionResultValidationError::CurrentnessUnavailable)
        ) {
            return Err("unavailable currentness was accepted as validated".to_string());
        }
        Ok(())
    }

    #[test]
    fn command_spec_digest_is_pinned_to_serialized_field_order() -> Result<(), String> {
        let digest = command_spec_sha256(&command_spec()).map_err(|error| error.to_string())?;
        if digest != "sha256:7594ebd8d5ea61336f33c236c213c87467b752befec1b582d7cc99ce42f8a5ab" {
            return Err(format!("unexpected command-spec digest: {digest}"));
        }
        Ok(())
    }
}
