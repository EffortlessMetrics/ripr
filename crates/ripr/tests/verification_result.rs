use ripr::domain::{
    CancellationPolicy, CommandAuthorityBoundary, CommandCostClass, CommandExecutionMode,
    CommandPlatform, CommandRole, CommandSpec, EnvironmentPolicy, ExpectedResultParser,
    NetworkPolicy, StdinPolicy, VerificationCurrentnessV1, VerificationExecutionResultV1,
    VerificationProcessDispositionV1, command_spec_sha256,
};

const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ROOT: &str = "root:fixture";
const ZERO_DIGEST: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

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

#[test]
fn verification_result_round_trips_and_validates_against_exact_context() -> Result<(), String> {
    let spec = command_spec();
    let result = VerificationExecutionResultV1 {
        schema_version: VerificationExecutionResultV1::SCHEMA_VERSION.to_string(),
        root_identity: ROOT.to_string(),
        head_before: HEAD.to_string(),
        head_after: HEAD.to_string(),
        command_spec_sha256: command_spec_sha256(&spec).map_err(|error| error.to_string())?,
        process_disposition: VerificationProcessDispositionV1::Completed,
        exit_status: Some(0),
        stdout_sha256: ZERO_DIGEST.to_string(),
        stderr_sha256: ZERO_DIGEST.to_string(),
        currentness: VerificationCurrentnessV1::Current,
    };
    let encoded = serde_json::to_string(&result).map_err(|error| error.to_string())?;
    let decoded: VerificationExecutionResultV1 =
        serde_json::from_str(&encoded).map_err(|error| error.to_string())?;
    decoded
        .validate_against(&spec, ROOT, HEAD, HEAD)
        .map_err(|error| error.to_string())
}
