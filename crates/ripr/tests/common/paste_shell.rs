//! Shell execution half of the printed-command paste harness (#5188, #5232,
//! #5247, #5269 each pinned one printed command by string or in one shell).
//!
//! A printed command is only as good as what a shell makes of it, so each one
//! is executed by the shells a user pastes into. The `ripr` and `git` on
//! `PATH` are argv recorders: they log what the shell actually passed and
//! exit 0, so the harness observes splitting, mis-parsing and wrong-root
//! binding without running an analysis per command.
//!
//! Shells that are not installed are reported as absent. The caller decides
//! whether absence is a skip (a developer machine) or a failure (CI), because
//! a lane that silently drops a shell proves nothing about it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A shell a printed command may be pasted into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Sh,
    Zsh,
    /// PowerShell 7 (`pwsh`), on every platform.
    Pwsh,
    /// Windows PowerShell 5.1 (`powershell.exe`).
    WindowsPowershell,
}

impl Shell {
    pub const ALL: [Shell; 5] = [
        Shell::Bash,
        Shell::Sh,
        Shell::Zsh,
        Shell::Pwsh,
        Shell::WindowsPowershell,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Sh => "sh",
            Shell::Zsh => "zsh",
            Shell::Pwsh => "pwsh",
            Shell::WindowsPowershell => "powershell",
        }
    }

    /// Whether the shell reads the PowerShell form of a printed command.
    pub fn is_powershell(self) -> bool {
        matches!(self, Shell::Pwsh | Shell::WindowsPowershell)
    }

    /// Shells the platform is expected to provide, which must not be missing
    /// in CI: a Unix runner has every POSIX shell and `pwsh`; Windows has Git
    /// Bash, `pwsh` and Windows PowerShell. `sh` is only meaningful on Unix.
    pub fn expected_on_this_platform(self) -> bool {
        match self {
            Shell::Bash | Shell::Pwsh => true,
            Shell::Sh | Shell::Zsh => !cfg!(windows),
            Shell::WindowsPowershell => cfg!(windows),
        }
    }
}

/// One printed command to execute.
pub struct Case {
    pub id: usize,
    /// The command text exactly as ripr printed it for this shell.
    pub command: String,
    pub cwd: PathBuf,
}

/// One recorded invocation of the `ripr` or `git` shim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recorded {
    pub case: usize,
    pub program: String,
    pub cwd: String,
    pub args: Vec<String>,
}

/// What one shell made of one case.
pub struct Outcome {
    pub calls: Vec<Recorded>,
    /// The shell's own complaint (a parse error, a failed redirect), when it
    /// reported one.
    pub shell_error: Option<String>,
}

const RECORDER_SOURCE: &str = r#"
use std::io::Write;
fn main() {
    let Some(log) = std::env::var_os("RIPR_PASTE_LOG") else { std::process::exit(97) };
    let program = std::env::current_exe()
        .ok()
        .and_then(|path| path.file_stem().map(|stem| stem.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let case = std::env::var("RIPR_PASTE_CASE").unwrap_or_default();
    let cwd = std::env::current_dir()
        .map(|dir| dir.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut record = format!("{case}\0{program}\0{cwd}");
    for arg in std::env::args_os().skip(1) {
        record.push('\0');
        record.push_str(&arg.to_string_lossy());
    }
    record.push('\x1e');
    let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(log) else {
        std::process::exit(98)
    };
    if file.write_all(record.as_bytes()).is_err() {
        std::process::exit(99)
    }
}
"#;

/// Compile the argv recorder once and install it as both `ripr` and `git`.
pub fn install_recorders(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|err| format!("create {}: {err}", dir.display()))?;
    let source = dir.join("recorder.rs");
    std::fs::write(&source, RECORDER_SOURCE)
        .map_err(|err| format!("write {}: {err}", source.display()))?;
    let suffix = std::env::consts::EXE_SUFFIX;
    let ripr = dir.join(format!("ripr{suffix}"));
    let output = Command::new("rustc")
        .arg("--edition=2021")
        .arg("-O")
        .arg("-o")
        .arg(&ripr)
        .arg(&source)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .map_err(|err| format!("spawn rustc for the argv recorder: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "compile the argv recorder failed\n{}",
            describe(&output)
        ));
    }
    std::fs::copy(&ripr, dir.join(format!("git{suffix}")))
        .map_err(|err| format!("install the git recorder: {err}"))?;
    Ok(())
}

pub fn describe(output: &Output) -> String {
    format!(
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The path spelling a shell under test receives. Git Bash loses the
/// backslashes of a Windows path to MSYS argument processing, so separators
/// are spelled forward.
fn shell_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// Locate a shell, confirming it can run a probe script from a host path (a
/// bare `bash.exe` on a Windows `PATH` is frequently WSL bash, which cannot
/// open drive-letter paths).
pub fn locate(shell: Shell, probe_dir: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    match shell {
        Shell::Bash if cfg!(windows) => {
            for var in ["ProgramFiles", "ProgramFiles(x86)"] {
                if let Some(prefix) = std::env::var_os(var) {
                    // `usr/bin/bash.exe` is the MSYS shell itself. `bin/bash.exe` is
                    // a launcher that puts Git's own `mingw64/bin` in front of
                    // `PATH`, which would hide the `git` recorder.
                    for relative in ["Git/usr/bin/bash.exe", "Git/bin/bash.exe"] {
                        candidates.push(PathBuf::from(&prefix).join(relative));
                    }
                }
            }
        }
        Shell::Bash => candidates.extend([PathBuf::from("/bin/bash"), PathBuf::from("bash")]),
        Shell::Sh => candidates.extend([PathBuf::from("/bin/sh"), PathBuf::from("sh")]),
        Shell::Zsh => candidates.extend([PathBuf::from("/bin/zsh"), PathBuf::from("zsh")]),
        Shell::Pwsh => candidates.push(PathBuf::from("pwsh")),
        Shell::WindowsPowershell => candidates.push(PathBuf::from("powershell")),
    }
    if !shell.expected_on_this_platform() {
        return None;
    }
    std::fs::create_dir_all(probe_dir).ok()?;
    let (script, body): (PathBuf, &[u8]) = if shell.is_powershell() {
        (
            probe_dir.join(format!("probe-{}.ps1", shell.name())),
            b"'ok'\n",
        )
    } else {
        (
            probe_dir.join(format!("probe-{}.sh", shell.name())),
            b"printf 'ok\\n'\n",
        )
    };
    std::fs::write(&script, body).ok()?;
    candidates.into_iter().find(|candidate| {
        let mut command = Command::new(candidate);
        if shell.is_powershell() {
            command.args(["-NoProfile", "-NonInteractive", "-File"]);
        }
        command
            .arg(shell_path(&script))
            .output()
            .is_ok_and(|output| {
                output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "ok"
            })
    })
}

fn path_env(recorders: &Path) -> Result<std::ffi::OsString, String> {
    let existing = std::env::var_os("PATH").ok_or_else(|| "PATH is not set".to_string())?;
    std::env::join_paths(
        std::iter::once(recorders.to_path_buf()).chain(std::env::split_paths(&existing)),
    )
    .map_err(|err| format!("join PATH entries: {err}"))
}

/// Parse the recorder log into per-call records.
fn read_log(log: &Path) -> Result<Vec<Recorded>, String> {
    let bytes = match std::fs::read(log) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(format!("read {}: {err}", log.display())),
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut calls = Vec::new();
    for record in text.split('\x1e').filter(|record| !record.is_empty()) {
        let mut fields = record.split('\0');
        let case = fields
            .next()
            .and_then(|field| field.parse::<usize>().ok())
            .ok_or_else(|| format!("recorder log has a record without a case id: {record:?}"))?;
        let program = fields.next().unwrap_or_default().to_string();
        let cwd = fields.next().unwrap_or_default().to_string();
        calls.push(Recorded {
            case,
            program,
            cwd,
            args: fields.map(str::to_string).collect(),
        });
    }
    Ok(calls)
}

/// Run every case under `shell`, each from its own working directory, and
/// return what the recorders saw per case id.
pub fn run_cases(
    shell: Shell,
    executable: &Path,
    scratch: &Path,
    recorders: &Path,
    cases: &[Case],
) -> Result<Vec<Outcome>, String> {
    let dir = scratch.join(format!("run-{}", shell.name()));
    std::fs::create_dir_all(&dir).map_err(|err| format!("create {}: {err}", dir.display()))?;
    let log = dir.join("calls.log");
    let _ = std::fs::remove_file(&log);
    let path = path_env(recorders)?;
    let mut errors: Vec<Option<String>> = (0..cases.len()).map(|_| None).collect();
    // Git Bash may still reorder `PATH` at start-up, so a non-interactive
    // MSYS shell re-asserts the recorders at the front before the case runs.
    // The case text itself is untouched.
    let bash_env = dir.join("bash_env.sh");
    if cfg!(windows) && shell == Shell::Bash {
        let quoted = shell_path(recorders).replace('\'', "'\\''");
        let line = format!("PATH=\"$(/usr/bin/cygpath -u '{quoted}'):$PATH\"\n");
        std::fs::write(&bash_env, line)
            .map_err(|err| format!("write {}: {err}", bash_env.display()))?;
    }
    if shell.is_powershell() {
        run_powershell(shell, executable, &dir, &log, &path, cases, &mut errors)?;
    } else {
        for (index, case) in cases.iter().enumerate() {
            let script = dir.join(format!("case-{}.sh", case.id));
            std::fs::write(&script, case.command.as_bytes())
                .map_err(|err| format!("write {}: {err}", script.display()))?;
            let mut command = Command::new(executable);
            if shell == Shell::Zsh {
                // Match what a user's interactive paste does not do: no rc
                // files, so a stray `setopt` cannot hide a quoting failure.
                command.arg("-f");
            }
            if cfg!(windows) && shell == Shell::Bash {
                command.env("BASH_ENV", shell_path(&bash_env));
            }
            let output = command
                .arg(shell_path(&script))
                .current_dir(&case.cwd)
                .env("PATH", &path)
                .env("RIPR_PASTE_LOG", &log)
                .env("RIPR_PASTE_CASE", case.id.to_string())
                // Git Bash rewrites arguments that look like POSIX paths or
                // path lists before a native program sees them. That is the
                // MSYS layer, not ripr, so it is switched off to isolate the
                // printed command.
                .env("MSYS_NO_PATHCONV", "1")
                .env("MSYS2_ARG_CONV_EXCL", "*")
                .output()
                .map_err(|err| format!("spawn {}: {err}", shell.name()))?;
            if !output.status.success()
                && let Some(slot) = errors.get_mut(index)
            {
                *slot = Some(describe(&output));
            }
        }
    }
    let calls = read_log(&log)?;
    Ok(cases
        .iter()
        .zip(errors)
        .map(|(case, shell_error)| Outcome {
            calls: calls
                .iter()
                .filter(|call| call.case == case.id)
                .cloned()
                .collect(),
            shell_error,
        })
        .collect())
}

/// PowerShell starts slowly, so every case runs in one process. Each command
/// goes through `Invoke-Expression` inside its own `try`, which parses and
/// runs it as the same text a paste into a prompt would, and a parse error is
/// caught per case instead of ending the batch.
fn run_powershell(
    shell: Shell,
    executable: &Path,
    dir: &Path,
    log: &Path,
    path: &std::ffi::OsStr,
    cases: &[Case],
    errors: &mut [Option<String>],
) -> Result<(), String> {
    let cases_json = serde_json::Value::Array(
        cases
            .iter()
            .map(|case| {
                serde_json::json!({
                    "id": case.id,
                    "cwd": case.cwd.display().to_string(),
                    "command": case.command,
                })
            })
            .collect(),
    );
    let cases_file = dir.join("cases.json");
    std::fs::write(&cases_file, cases_json.to_string())
        .map_err(|err| format!("write {}: {err}", cases_file.display()))?;
    let results = dir.join("results.tsv");
    let _ = std::fs::remove_file(&results);
    let runner = dir.join("runner.ps1");
    let script = "\
param([string]$CasesFile, [string]$ResultsFile)
$ErrorActionPreference = 'Stop'
# Windows PowerShell 5.1 hands a piped JSON array on as one object, so the
# text is passed by parameter and the result enumerated as it stands.
$cases = ConvertFrom-Json -InputObject (Get-Content -LiteralPath $CasesFile -Raw -Encoding UTF8)
$utf8 = New-Object System.Text.UTF8Encoding($false)
foreach ($case in $cases) {
  $env:RIPR_PASTE_CASE = [string]$case.id
  try {
    Set-Location -LiteralPath $case.cwd
    Invoke-Expression $case.command
  } catch {
    $message = ($_.Exception.Message -replace '[\\r\\n\\t]+', ' ')
    [System.IO.File]::AppendAllText($ResultsFile, \"$($case.id)`t$message`n\", $utf8)
  }
}
";
    // A BOM lets Windows PowerShell 5.1 read the non-ASCII text correctly.
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(script.as_bytes());
    std::fs::write(&runner, bytes).map_err(|err| format!("write {}: {err}", runner.display()))?;
    let output = Command::new(executable)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&runner)
        .arg(&cases_file)
        .arg(&results)
        .env("PATH", path)
        .env("RIPR_PASTE_LOG", log)
        .output()
        .map_err(|err| format!("spawn {}: {err}", shell.name()))?;
    if !output.status.success() {
        return Err(format!(
            "{} runner script failed outright\n{}",
            shell.name(),
            describe(&output)
        ));
    }
    if let Ok(text) = std::fs::read_to_string(&results) {
        for line in text.lines() {
            let Some((id, message)) = line.split_once('\t') else {
                continue;
            };
            if let Some(index) = id
                .parse::<usize>()
                .ok()
                .and_then(|id| cases.iter().position(|case| case.id == id))
                && let Some(slot) = errors.get_mut(index)
            {
                *slot = Some(message.to_string());
            }
        }
    }
    Ok(())
}
