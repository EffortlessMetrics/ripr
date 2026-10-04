//! Live pip and uv install-to-use journey against the candidate `ripr` binary.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::identity::{
    DISTRIBUTION, EXECUTABLE, InstalledPayload, WheelIdentity, parse_wheel_filename,
    require_matching_consumer_payloads, require_matching_payload, sha256_file,
    version_only_evidence_is_insufficient,
};
use super::isolation::{
    require_clean_consumer_path, require_installed_beats_planted, require_project_python_idle,
    require_uninstall_leaves_planted_and_project,
};
use super::oracles::{
    bind_continuation_to_finding, extract_explain_continuation,
    require_explicit_rust_only_is_limited_not_clean, require_identity_preserving_explain,
    require_no_config_python_preview,
};
use super::support::{
    SpawnRequest, TempRoot, copy_tree, pack_wheel, python3, require_success, run, run_bash_script,
    stdout_text, workspace_root, write_executable,
};

const UV_VERSION: &str = "0.12.19";
const BASE_PATH: &str = "/usr/bin:/bin";

struct ClientLayout {
    name: &'static str,
    bin_dir: PathBuf,
    installed: PathBuf,
    planted: PathBuf,
    planted_sentinel: PathBuf,
    project_python_sentinel: PathBuf,
    extra_path: String,
}

pub(crate) fn run_live_pip_and_uv_journey() -> Result<(), String> {
    let python = python3()?;
    let payload = PathBuf::from(env!("CARGO_BIN_EXE_ripr"));
    if !payload.is_file() {
        return Err(format!(
            "candidate ripr binary missing at {}; live journey cannot claim a wheel payload",
            payload.display()
        ));
    }
    let version = env!("CARGO_PKG_VERSION");
    let root = TempRoot::new("live")?;
    let evidence = root.path.join("evidence");
    let wheelhouse = evidence.join("wheelhouse");
    fs::create_dir_all(&wheelhouse).map_err(|error| format!("mkdir wheelhouse: {error}"))?;

    let identity = pack_wheel(&python, &payload, &wheelhouse, version)?;
    parse_wheel_filename(&identity.filename, version)?;

    let fixture_src = workspace_root().join("fixtures/python/basic");
    if !fixture_src.join("diff.patch").is_file() {
        return Err(format!(
            "Python no-config fixture missing at {}",
            fixture_src.display()
        ));
    }
    let project = root.path.join("consumer ü");
    let fixture = project.join("fixtures/python/basic");
    copy_tree(&fixture_src, &fixture)?;
    require_clean_consumer_path(BASE_PATH, Some(&project))?;

    let pip = install_with_pip(&python, &root.path, &wheelhouse, &identity)?;
    let uv = install_with_uv(&python, &root.path, &wheelhouse, &identity)?;
    require_matching_consumer_payloads(&pip.payload, &uv.payload)?;

    exercise_client(&pip, &identity, &fixture, &root.path)?;
    exercise_client(&uv, &identity, &fixture, &root.path)?;

    reinstall_and_uninstall(&pip, &uv, &wheelhouse, &identity, &fixture)?;

    write_receipt(&identity, &pip.payload, &uv.payload, version)?;
    Ok(())
}

struct InstalledClient {
    layout: ClientLayout,
    payload: InstalledPayload,
    uv_env: Vec<(String, String)>,
    uv_program: Option<PathBuf>,
}

fn install_with_pip(
    python: &Path,
    root: &Path,
    wheelhouse: &Path,
    identity: &WheelIdentity,
) -> Result<InstalledClient, String> {
    let venv = root.join("pip-consumer");
    let planted_dir = root.join("planted-pip");
    let planted = planted_dir.join(EXECUTABLE);
    let planted_sentinel = root.join("planted-pip-executed");
    let project_python_sentinel = root.join("pip-project-python-executed");
    create_venv(python, &venv)?;
    write_planted(&planted, &planted_sentinel)?;
    require_clean_consumer_path(BASE_PATH, None)?;

    let pip = venv.join("bin/python");
    let wheelhouse_s = wheelhouse.to_str().ok_or("wheelhouse path is not UTF-8")?;
    let spec = format!("{DISTRIBUTION}=={}", identity.version);
    let output = run(SpawnRequest {
        program: &pip,
        args: &[
            "-m",
            "pip",
            "install",
            "--disable-pip-version-check",
            "--no-index",
            "--find-links",
            wheelhouse_s,
            &spec,
        ],
        cwd: None,
        path: Some(BASE_PATH),
        extra_env: &[],
        clear_env: false,
    })?;
    require_success(&output, "pip install ripr-rs from local wheelhouse")?;

    let installed = venv.join("bin").join(EXECUTABLE);
    let payload = InstalledPayload {
        path: installed.display().to_string(),
        sha256: sha256_file(&installed)?,
    };
    require_matching_payload(identity, &payload)?;
    let extra_path = format!(
        "{}:{}:{BASE_PATH}",
        venv.join("bin").display(),
        planted_dir.display()
    );
    require_installed_beats_planted(&extra_path, &installed, &planted, &planted_sentinel)?;
    Ok(InstalledClient {
        layout: ClientLayout {
            name: "pip",
            bin_dir: venv.join("bin"),
            installed,
            planted,
            planted_sentinel,
            project_python_sentinel,
            extra_path,
        },
        payload,
        uv_env: Vec::new(),
        uv_program: None,
    })
}

fn install_with_uv(
    python: &Path,
    root: &Path,
    wheelhouse: &Path,
    identity: &WheelIdentity,
) -> Result<InstalledClient, String> {
    let bootstrap = root.join("uv-bootstrap");
    create_venv(python, &bootstrap)?;
    let bootstrap_python = bootstrap.join("bin/python");
    let spec = format!("uv=={UV_VERSION}");
    let output = run(SpawnRequest {
        program: &bootstrap_python,
        args: &["-m", "pip", "install", "--disable-pip-version-check", &spec],
        cwd: None,
        path: Some(BASE_PATH),
        extra_env: &[],
        clear_env: false,
    })?;
    require_success(
        &output,
        "stage the pinned uv client into an isolated bootstrap venv (network is allowed only for this staging step)",
    )?;

    let uv = bootstrap.join("bin/uv");
    if !uv.is_file() {
        return Err(
            "uv client did not install a `uv` executable into the bootstrap venv".to_string(),
        );
    }
    let version = run(SpawnRequest {
        program: &uv,
        args: &["--version"],
        cwd: None,
        path: Some(BASE_PATH),
        extra_env: &[],
        clear_env: false,
    })?;
    require_success(&version, "uv --version")?;
    let version_text = stdout_text(&version);
    if !version_text.contains(UV_VERSION) {
        return Err(format!(
            "staged uv version `{version_text}` does not contain pinned {UV_VERSION}"
        ));
    }

    let tool_dir = root.join("uv-tools");
    let bin_dir = root.join("uv-bin");
    let cache = root.join("uv-cache");
    fs::create_dir_all(&bin_dir).map_err(|error| format!("mkdir uv bin: {error}"))?;
    let planted_dir = root.join("planted-uv");
    let planted = planted_dir.join(EXECUTABLE);
    let planted_sentinel = root.join("planted-uv-executed");
    let project_python_sentinel = root.join("uv-project-python-executed");
    write_planted(&planted, &planted_sentinel)?;
    require_clean_consumer_path(BASE_PATH, None)?;

    let wheel = wheelhouse.join(&identity.filename);
    let wheel_s = wheel.to_str().ok_or("wheel path is not UTF-8")?;
    let env = uv_env(&tool_dir, &bin_dir, &cache);
    let env_refs: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let output = run(SpawnRequest {
        program: &uv,
        args: &["tool", "install", wheel_s],
        cwd: None,
        path: Some(BASE_PATH),
        extra_env: &env_refs,
        clear_env: false,
    })?;
    require_success(&output, "uv tool install local ripr-rs wheel")?;

    let installed = bin_dir.join(EXECUTABLE);
    let payload = InstalledPayload {
        path: installed.display().to_string(),
        sha256: sha256_file(&installed)?,
    };
    require_matching_payload(identity, &payload)?;
    let extra_path = format!(
        "{}:{}:{}:{BASE_PATH}",
        bin_dir.display(),
        planted_dir.display(),
        bootstrap.join("bin").display()
    );
    require_installed_beats_planted(&extra_path, &installed, &planted, &planted_sentinel)?;
    Ok(InstalledClient {
        layout: ClientLayout {
            name: "uv",
            bin_dir,
            installed,
            planted,
            planted_sentinel,
            project_python_sentinel,
            extra_path,
        },
        payload,
        uv_env: env,
        uv_program: Some(uv),
    })
}

fn exercise_client(
    client: &InstalledClient,
    identity: &WheelIdentity,
    fixture: &Path,
    root: &Path,
) -> Result<(), String> {
    rewrite_project_python(fixture, &client.layout.project_python_sentinel)?;
    let foreign = root.join(format!("{} foreign cwd", client.layout.name));
    fs::create_dir_all(&foreign).map_err(|error| format!("mkdir foreign cwd: {error}"))?;

    let check_json = root.join(format!("{}-check.json", client.layout.name));
    let human_txt = root.join(format!("{}-check.txt", client.layout.name));
    let home = root.join(format!("{}-home", client.layout.name));
    fs::create_dir_all(&home).map_err(|error| format!("mkdir consumer home: {error}"))?;
    let fixture_s = fixture.to_str().ok_or("fixture path is not UTF-8")?;
    let diff = fixture.join("diff.patch");
    let diff_s = diff.to_str().ok_or("diff path is not UTF-8")?;
    let json_s = check_json.to_str().ok_or("json path is not UTF-8")?;
    let home_s = home.to_str().ok_or("consumer home is not UTF-8")?;
    let extra_env = [
        ("HTTP_PROXY", "http://127.0.0.1:9"),
        ("HTTPS_PROXY", "http://127.0.0.1:9"),
        ("NO_PROXY", ""),
        ("HOME", home_s),
    ];

    let json_out = run_installed(
        client,
        &[
            "check", "--root", fixture_s, "--diff", diff_s, "--mode", "fast", "--json",
        ],
        Some(&foreign),
        &extra_env,
    )?;
    require_success(
        &json_out,
        &format!("{} no-config python check --json", client.layout.name),
    )?;
    fs::write(&check_json, &json_out.stdout).map_err(|error| format!("write {json_s}: {error}"))?;
    let report: Value = serde_json::from_slice(&json_out.stdout)
        .map_err(|error| format!("{} JSON is not an object: {error}", client.layout.name))?;
    let finding_id = require_no_config_python_preview(&report)?;

    let human_out = run_installed(
        client,
        &[
            "check", "--root", fixture_s, "--diff", diff_s, "--mode", "fast",
        ],
        Some(&foreign),
        &extra_env,
    )?;
    require_success(
        &human_out,
        &format!("{} no-config python check", client.layout.name),
    )?;
    fs::write(&human_txt, &human_out.stdout)
        .map_err(|error| format!("write human output: {error}"))?;
    let continuation = extract_explain_continuation(&stdout_text(&human_out))?;
    bind_continuation_to_finding(&continuation, &finding_id)?;
    require_identity_preserving_explain(&continuation)?;

    let version_out = run_installed(client, &["--version"], Some(&foreign), &[])?;
    require_success(&version_out, &format!("{} --version", client.layout.name))?;
    let version_text = stdout_text(&version_out);
    if !version_text.contains(&identity.version) {
        return Err(format!(
            "{} --version `{version_text}` does not contain {}",
            client.layout.name, identity.version
        ));
    }
    version_only_evidence_is_insufficient(true, true)?;

    let explain_script = root.join(format!("{}-explain.sh", client.layout.name));
    let explain_out = run_bash_script(
        &explain_script,
        &format!("{continuation}\n"),
        &foreign,
        &client.layout.extra_path,
        &extra_env,
    )?;
    require_success(
        &explain_out,
        &format!(
            "{} product-emitted explain continuation",
            client.layout.name
        ),
    )?;
    let explain_text = stdout_text(&explain_out);
    if explain_text.trim().is_empty() || !explain_text.contains(&finding_id) {
        return Err(format!(
            "{} explain output did not retain finding {finding_id}",
            client.layout.name
        ));
    }
    require_project_python_idle(&client.layout.project_python_sentinel)?;
    if client.layout.planted_sentinel.exists() {
        return Err(format!(
            "{} planted PATH executable ran during analysis",
            client.layout.name
        ));
    }

    fs::write(
        fixture.join("ripr.toml"),
        "[languages]\nenabled = [\"rust\"]\n",
    )
    .map_err(|error| format!("write explicit config: {error}"))?;
    let disabled = run_installed(
        client,
        &[
            "check", "--root", fixture_s, "--diff", diff_s, "--mode", "fast", "--json",
        ],
        Some(&foreign),
        &extra_env,
    )?;
    require_success(
        &disabled,
        &format!(
            "{} explicit rust-only python check --json",
            client.layout.name
        ),
    )?;
    let disabled_report: Value = serde_json::from_slice(&disabled.stdout).map_err(|error| {
        format!(
            "{} explicit-config JSON is not an object: {error}",
            client.layout.name
        )
    })?;
    require_explicit_rust_only_is_limited_not_clean(&disabled_report)?;
    fs::remove_file(fixture.join("ripr.toml"))
        .map_err(|error| format!("remove ripr.toml: {error}"))?;
    Ok(())
}

fn reinstall_and_uninstall(
    pip: &InstalledClient,
    uv: &InstalledClient,
    wheelhouse: &Path,
    identity: &WheelIdentity,
    fixture: &Path,
) -> Result<(), String> {
    let pip_python = pip.layout.bin_dir.join("python");
    let wheelhouse_s = wheelhouse.to_str().ok_or("wheelhouse path is not UTF-8")?;
    let spec = format!("{DISTRIBUTION}=={}", identity.version);
    let reinstall = run(SpawnRequest {
        program: &pip_python,
        args: &[
            "-m",
            "pip",
            "install",
            "--disable-pip-version-check",
            "--force-reinstall",
            "--no-index",
            "--find-links",
            wheelhouse_s,
            &spec,
        ],
        cwd: None,
        path: Some(BASE_PATH),
        extra_env: &[],
        clear_env: false,
    })?;
    require_success(&reinstall, "pip force-reinstall ripr-rs")?;
    let reinstalled = InstalledPayload {
        path: pip.layout.installed.display().to_string(),
        sha256: sha256_file(&pip.layout.installed)?,
    };
    require_matching_payload(identity, &reinstalled)?;

    let uninstall = run(SpawnRequest {
        program: &pip_python,
        args: &["-m", "pip", "uninstall", "-y", DISTRIBUTION],
        cwd: None,
        path: Some(BASE_PATH),
        extra_env: &[],
        clear_env: false,
    })?;
    require_success(&uninstall, "pip uninstall ripr-rs")?;
    require_uninstall_leaves_planted_and_project(
        &pip.layout.extra_path,
        &pip.layout.installed,
        &pip.layout.planted,
        &fixture.join("src/pricing.py"),
    )?;

    let uv_program = uv
        .uv_program
        .as_ref()
        .ok_or("uv client missing uv program")?;
    let env_refs: Vec<(&str, &str)> = uv
        .uv_env
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let wheel = wheelhouse.join(&identity.filename);
    let wheel_s = wheel.to_str().ok_or("wheel path is not UTF-8")?;
    let uv_reinstall = run(SpawnRequest {
        program: uv_program,
        args: &["tool", "install", "--force", wheel_s],
        cwd: None,
        path: Some(BASE_PATH),
        extra_env: &env_refs,
        clear_env: false,
    })?;
    require_success(&uv_reinstall, "uv tool install --force")?;
    require_matching_payload(
        identity,
        &InstalledPayload {
            path: uv.layout.installed.display().to_string(),
            sha256: sha256_file(&uv.layout.installed)?,
        },
    )?;
    let uv_uninstall = run(SpawnRequest {
        program: uv_program,
        args: &["tool", "uninstall", DISTRIBUTION],
        cwd: None,
        path: Some(BASE_PATH),
        extra_env: &env_refs,
        clear_env: false,
    })?;
    require_success(&uv_uninstall, "uv tool uninstall ripr-rs")?;
    require_uninstall_leaves_planted_and_project(
        &uv.layout.extra_path,
        &uv.layout.installed,
        &uv.layout.planted,
        &fixture.join("src/pricing.py"),
    )?;
    Ok(())
}

fn run_installed(
    client: &InstalledClient,
    args: &[&str],
    cwd: Option<&Path>,
    extra_env: &[(&str, &str)],
) -> Result<std::process::Output, String> {
    // Name lookup on Unix follows the harness PATH, not the child PATH, so
    // check/--version exec the recorded installed payload. The product
    // continuation is a different authority: bash looks up `ripr` there.
    run(SpawnRequest {
        program: &client.layout.installed,
        args,
        cwd,
        path: Some(&client.layout.extra_path),
        extra_env,
        clear_env: true,
    })
}

fn create_venv(python: &Path, dest: &Path) -> Result<(), String> {
    let dest_s = dest.to_str().ok_or("venv path is not UTF-8")?;
    let output = run(SpawnRequest {
        program: python,
        args: &["-m", "venv", dest_s],
        cwd: None,
        path: Some(BASE_PATH),
        extra_env: &[],
        clear_env: false,
    })?;
    require_success(&output, &format!("python3 -m venv {dest_s}"))
}

fn write_planted(path: &Path, sentinel: &Path) -> Result<(), String> {
    let sentinel_s = sentinel.to_str().ok_or("sentinel path is not UTF-8")?;
    write_executable(
        path,
        &format!(
            "#!/usr/bin/env bash\nprintf 'planted PATH ripr was executed\\n' > '{sentinel_s}'\nexit 97\n"
        ),
    )
}

fn rewrite_project_python(fixture: &Path, sentinel: &Path) -> Result<(), String> {
    let sentinel_s = sentinel
        .to_str()
        .ok_or("project sentinel path is not UTF-8")?;
    write_executable(
        &fixture.join(".venv/bin/python"),
        &format!(
            "#!/usr/bin/env bash\nprintf 'project virtualenv was executed\\n' > '{sentinel_s}'\nexit 98\n"
        ),
    )
}

fn uv_env(tool_dir: &Path, bin_dir: &Path, cache: &Path) -> Vec<(String, String)> {
    vec![
        ("UV_OFFLINE".to_string(), "1".to_string()),
        ("UV_TOOL_DIR".to_string(), tool_dir.display().to_string()),
        ("UV_TOOL_BIN_DIR".to_string(), bin_dir.display().to_string()),
        ("UV_CACHE_DIR".to_string(), cache.display().to_string()),
    ]
}

fn write_receipt(
    identity: &WheelIdentity,
    pip: &InstalledPayload,
    uv: &InstalledPayload,
    version: &str,
) -> Result<(), String> {
    let reports = workspace_root().join("target/ripr/reports");
    fs::create_dir_all(&reports).map_err(|error| format!("mkdir reports: {error}"))?;
    let receipt = json!({
        "schema_version": 1,
        "issue": 4626,
        "channel": "pypi-local-wheelhouse",
        "publication_attempted": false,
        "distribution": DISTRIBUTION,
        "executable": EXECUTABLE,
        "native_version": version,
        "payload_source": "candidate_binary_wheel",
        "compatibility_state": "unqualified",
        "compatibility_owner": "issue:#4489",
        "wheel": {
            "filename": identity.filename,
            "sha256": identity.wheel_sha256,
            "tag": identity.tag,
            "installed_payload_sha256": identity.payload_sha256,
        },
        "consumers": {
            "pip": {"path": pip.path, "sha256": pip.sha256},
            "uv": {"path": uv.path, "sha256": uv.sha256},
        },
        "claim_boundary": "Clean pip and uv install-to-use proof for a local wheel of this candidate binary. Not Maturin adapter ownership (#4490), not admitted native compatibility (#4489), not CI orchestration (#4493), and not a PyPI publication or support-tier claim.",
    });
    let path = reports.join("pypi-consumer-journey-receipt.json");
    fs::write(
        &path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&receipt)
                .map_err(|error| format!("serialize receipt: {error}"))?
        ),
    )
    .map_err(|error| format!("write {}: {error}", path.display()))?;
    Ok(())
}
