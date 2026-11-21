use ripr::domain::{
    CancellationPolicy, CommandAuthorityBoundary, CommandCostClass, CommandExecutionMode,
    CommandPlatform, CommandRole, CommandSpec, EnvironmentPolicy, ExpectedResultParser,
    NetworkPolicy, StdinPolicy, VerificationCurrentnessV1, VerificationExecutionResultV1,
    VerificationProcessDispositionV1, command_spec_sha256,
};

/// The published JSON Schema and the Rust struct must agree about the v1
/// execution result. They are separate artifacts validated by different
/// consumers, so drift between them is invisible until an external validator
/// rejects output RIPR considers valid.
///
/// Two directions are checked:
/// - every field the schema marks `required` is present in a serialized result;
/// - every serialized field is declared in the schema's `properties`.
///
/// A field added to the struct with `#[serde(default)]` must therefore stay
/// optional in the schema, or a result written by an older RIPR stops
/// validating under the same `schema_version`.
#[test]
fn serialized_result_conforms_to_the_published_schema() -> Result<(), String> {
    let schema_text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/ripr/repair-assurance.schema.json"),
    )
    .map_err(|error| format!("read repair-assurance schema: {error}"))?;
    let schema: serde_json::Value =
        serde_json::from_str(&schema_text).map_err(|error| format!("parse schema: {error}"))?;
    let definition = schema
        .pointer("/$defs/execution_result")
        .ok_or("schema must define $defs/execution_result")?;

    let serialized =
        serde_json::to_value(execution_result()?).map_err(|error| error.to_string())?;
    let object = serialized
        .as_object()
        .ok_or("a serialized result must be a JSON object")?;

    let properties = definition
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .ok_or("schema definition must declare properties")?;
    for field in object.keys() {
        assert!(
            properties.contains_key(field),
            "serialized field {field} is not declared in the schema"
        );
    }
    let required = definition
        .get("required")
        .and_then(serde_json::Value::as_array)
        .ok_or("schema definition must declare required")?;
    let required: Vec<&str> = required
        .iter()
        .map(|entry| entry.as_str().ok_or("required entries must be strings"))
        .collect::<Result<_, _>>()?;
    for field in &required {
        assert!(
            object.contains_key(*field),
            "schema requires {field} but a serialized result omits it"
        );
    }

    // The compatibility direction that actually breaks consumers: a field the
    // Rust side defaults must not be required by the schema, or a result
    // written before that field existed stops validating under the same
    // `schema_version` even though `validate_against` still accepts it.
    let mut legacy = object.clone();
    let defaulted = [
        "duration_ms",
        "exit_signal",
        "stdout_bytes",
        "stderr_bytes",
        "stdout_truncated",
        "stderr_truncated",
        "cancellation_requested",
    ];
    for field in defaulted {
        legacy.remove(field);
        assert!(
            !required.contains(&field),
            "{field} is serde-defaulted in Rust but required by the schema; \
             an older serialized result would fail schema validation while \
             still passing validate_against"
        );
    }
    // And the Rust side must genuinely accept that older payload.
    let decoded: VerificationExecutionResultV1 =
        serde_json::from_value(serde_json::Value::Object(legacy))
            .map_err(|error| format!("a result without defaulted fields must decode: {error}"))?;
    decoded
        .validate_against(&command_spec(), ROOT, HEAD, HEAD)
        .map_err(|error| format!("a legacy result must still validate: {error}"))?;
    Ok(())
}

/// One fully populated v1 result, shared by the round-trip and schema
/// conformance tests so both judge the same serialized shape.
fn execution_result() -> Result<VerificationExecutionResultV1, String> {
    Ok(VerificationExecutionResultV1 {
        schema_version: VerificationExecutionResultV1::SCHEMA_VERSION.to_string(),
        root_identity: ROOT.to_string(),
        head_before: HEAD.to_string(),
        head_after: HEAD.to_string(),
        // A compatibility fixture must never substitute a placeholder for a
        // failed real commitment; propagate the digest error instead.
        command_spec_sha256: command_spec_sha256(&command_spec())
            .map_err(|error| error.to_string())?,
        process_disposition: VerificationProcessDispositionV1::Completed,
        exit_status: Some(0),
        stdout_sha256: ZERO_DIGEST.to_string(),
        stderr_sha256: ZERO_DIGEST.to_string(),
        currentness: VerificationCurrentnessV1::Current,
        duration_ms: 0,
        exit_signal: None,
        stdout_bytes: 0,
        stderr_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        cancellation_requested: false,
    })
}

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
    let result = execution_result()?;
    let encoded = serde_json::to_string(&result).map_err(|error| error.to_string())?;
    let decoded: VerificationExecutionResultV1 =
        serde_json::from_str(&encoded).map_err(|error| error.to_string())?;
    decoded
        .validate_against(&spec, ROOT, HEAD, HEAD)
        .map_err(|error| error.to_string())
}
