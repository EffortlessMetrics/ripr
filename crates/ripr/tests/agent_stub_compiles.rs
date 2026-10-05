//! #5355 / #5453: `ripr agent stub --write` must leave the user's crate compiling.
//! Each ready inline stub is written into a fresh copy of one fixture crate,
//! the file is compiled with rustc as a test crate, and the stub's test must
//! run and stop at its own `ripr:` `todo!()`. Integration-file stubs take a
//! cargo-built fixture with an established `tests/` layout, then
//! `cargo test --test <stem>` from the stub's own `run_command`. Cases the
//! producer cannot make compile must be refused with a named reason instead.
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const FIXTURE: &str = r#"pub const LIMIT: u32 = 10;

pub fn price(amount: u32, threshold: u32) -> u32 {
    if amount >= threshold { amount - 10 } else { amount }
}

pub struct Opaque(u32);

pub fn wrap(n: u32) -> Opaque {
    if n > LIMIT { Opaque(n) } else { Opaque(0) }
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    Empty,
    TooLong(usize),
}

impl ParseError {
    pub fn from_code(code: u8) -> Result<u8, Self> {
        if code == 0 {
            return Err(Self::Empty);
        }
        Ok(code)
    }
}

pub struct Parser {
    pub max: usize,
}

impl Parser {
    pub fn parse(&self, input: &str) -> Result<usize, ParseError> {
        if input.len() > self.max {
            return Err(ParseError::TooLong(input.len()));
        }
        Ok(input.len())
    }

    pub fn clamp(&self, subject: usize) -> usize {
        if subject > self.max { self.max } else { subject }
    }

    pub fn boxed(self: Box<Self>, n: usize) -> usize {
        if n > 7 { n } else { self.max }
    }
}

impl std::fmt::Display for Parser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.max > 3 { write!(f, "big") } else { write!(f, "small") }
    }
}

pub type Res<T> = std::result::Result<T, AppError>;

#[derive(Debug)]
pub enum AppError {
    Bad,
}

pub fn checked(n: u8) -> Res<u8> {
    if n > 5 { Err(AppError::Bad) } else { Ok(n) }
}

pub fn read_len(n: u8) -> std::io::Result<u8> {
    if n > 4 { Ok(n) } else { Err(std::io::Error::other("short")) }
}

pub struct Duration(pub u64);

pub fn wait(n: u64) -> Duration {
    if n > 9 { Duration(n) } else { Duration(0) }
}

use std::sync::Mutex as Vec;

pub fn lock(n: u8) -> Vec<u8> {
    if n > 6 { Vec::new(n) } else { Vec::new(0) }
}

pub mod shapes {
    pub struct S {
        pub v: u8,
    }

    impl self::S {
        pub fn get(&self, n: u8) -> u8 {
            if n > 2 { n } else { self.v }
        }

        pub fn merge(&self, other: Self, n: u8) -> u8 {
            if n > 1 { other.v } else { self.v }
        }
    }

    #[derive(Debug, PartialEq)]
    pub enum E {
        Bad,
    }

    impl self::E {
        pub fn check(n: u8) -> Result<u8, Self> {
            if n == 8 {
                return Err(Self::Bad);
            }
            Ok(n)
        }
    }

    #[cfg(test)]
    mod tests {}
}

pub mod inner {
    pub struct Cfg {
        pub on: bool,
    }

    pub fn flag(cfg: &self::Cfg, n: i32) -> bool {
        if n < 0 { cfg.on } else { !cfg.on }
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn smoke() {}
    }
}
"#;

/// #5471: owner shapes from real crates (bytesize, humantime, semver) that
/// were refused with reasons that did not name the blocker.
const FIXTURE_5471: &str = r#"use std::str::FromStr;

pub fn early(x: u32) -> u32 {
    if x > 40 { 1 } else { 0 }
}

#[cfg(all(test, feature = "slow"))]
mod slow_tests {
    #[test]
    fn smoke() {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {}
}

pub struct Parser<'a> {
    pub src: &'a str,
}

impl<'a> Parser<'a> {
    pub fn parse_unit(&mut self, start: usize, end: usize) -> Result<u64, String> {
        if end > start { Ok(1) } else { Err(String::from(self.src)) }
    }
}

#[derive(Debug, PartialEq)]
pub enum Unit {
    Second,
    Minute,
}

#[derive(Debug, PartialEq)]
pub enum UnitError {
    Unknown,
}

impl FromStr for Unit {
    type Err = UnitError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "s" => Ok(Unit::Second),
            "m" => Ok(Unit::Minute),
            _ => Err(UnitError::Unknown),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Version {
    pub fn next_minor(&self) -> Version {
        Version {
            major: self.major,
            minor: self.minor + 1,
            patch: 0,
        }
    }
}

pub struct Wrap<T>(pub T);

impl<T: Copy> Wrap<T> {
    pub fn pick(&self, n: u8) -> u8 {
        if n > 3 { n } else { 0 }
    }
}

pub struct Local;

const _: () = {
    impl Local {
        pub fn hidden(&self, n: u8) -> u8 {
            if n > 11 { n } else { 0 }
        }
    }
};

#[cfg(test)]
mod more_tests {
    #[test]
    fn smoke() {}
}
"#;

enum Expect {
    /// The stub is written, compiles, and its test stops at a `ripr:` todo.
    StopsAtRiprTodo,
    /// No stub; the refusal names this kind.
    Refused(&'static str),
}

#[test]
fn written_stubs_compile_and_stop_at_their_own_todo() -> Result<(), String> {
    let cases = [
        ("amount >= threshold", Expect::StopsAtRiprTodo),
        // Return type with no visible PartialEq: the assertion is the fill-in.
        ("n > LIMIT", Expect::StopsAtRiprTodo),
        // `Self::Empty` inside `impl ParseError` is respelled from the test.
        ("code == 0", Expect::StopsAtRiprTodo),
        ("return Err(ParseError::TooLong", Expect::StopsAtRiprTodo),
        // A parameter named `subject` must not shadow the receiver.
        ("subject > self.max", Expect::StopsAtRiprTodo),
        // `self::Cfg` is respelled for the child test module.
        ("n < 0", Expect::StopsAtRiprTodo),
        // Result aliases, qualified paths and shadowed std names are not
        // assumed comparable: the assertion is the fill-in.
        ("n > 5 { Err", Expect::StopsAtRiprTodo),
        ("n > 4 { Ok", Expect::StopsAtRiprTodo),
        ("n > 9", Expect::StopsAtRiprTodo),
        // `impl self::S` is respelled for the child test module.
        ("n > 2 { n }", Expect::StopsAtRiprTodo),
        // `Self` in a parameter or error pattern is respelled once, not twice.
        ("n > 1 { other.v }", Expect::StopsAtRiprTodo),
        ("n == 8", Expect::StopsAtRiprTodo),
        // A type imported under a std name is not assumed comparable.
        ("n > 6", Expect::StopsAtRiprTodo),
        ("n > 7", Expect::Refused("owner_unsupported")),
        // A trait method with a `&self` receiver is called through
        // `<Parser as std::fmt::Display>::fmt(&subject, ..)`.
        ("self.max > 3", Expect::StopsAtRiprTodo),
    ]
    .map(|(needle, expect)| (FIXTURE, needle, expect));
    let cases_5471 = [
        // Two plain inline test modules: the nearest one after the owner.
        // The nearer `cfg(all(test, feature = "slow"))` module is skipped;
        // a stub there would not run in this plain test build.
        ("x > 40", Expect::StopsAtRiprTodo),
        // `impl<'a> Parser<'a>` with a `&mut self` method.
        ("end > start", Expect::StopsAtRiprTodo),
        // `impl FromStr for Unit` returning `Result<Self, Self::Err>`.
        ("\"m\" =>", Expect::StopsAtRiprTodo),
        // A field of the struct literal the owner returns.
        ("minor: self.minor + 1", Expect::StopsAtRiprTodo),
        ("n > 3 { n }", Expect::Refused("owner_generic_impl")),
        // An impl inside a `const _` block cannot be named from a test module.
        ("n > 11", Expect::Refused("owner_unsupported")),
    ]
    .map(|(needle, expect)| (FIXTURE_5471, needle, expect));
    for (fixture, needle, expect) in cases.into_iter().chain(cases_5471) {
        let line = fixture
            .lines()
            .position(|text| text.contains(needle))
            .map(|index| index + 1)
            .ok_or_else(|| format!("fixture has `{needle}`"))?;
        let scratch = Scratch::new()?;
        let root = scratch.directory.clone();
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"stub_oracle\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(root.join("src/lib.rs"), fixture).map_err(|error| error.to_string())?;

        let stub = ripr_stub_write(&root, &format!("src/lib.rs:{line}"));
        let output = run_bounded(stub, &root, "stub", Duration::from_mins(2))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        match expect {
            Expect::Refused(kind) => {
                assert_eq!(output.status.code(), Some(3), "{needle}: {stdout}{stderr}");
                assert!(
                    stderr.contains(&format!("\"kind\": \"{kind}\"")),
                    "{needle}: {stderr}"
                );
                assert_eq!(
                    std::fs::read_to_string(root.join("src/lib.rs"))
                        .map_err(|error| error.to_string())?,
                    fixture,
                    "{needle}: a refusal must not touch the file"
                );
            }
            Expect::StopsAtRiprTodo => {
                assert!(output.status.success(), "{needle}: {stdout}{stderr}");
                let document: serde_json::Value = serde_json::from_str(&stdout)
                    .map_err(|error| format!("{needle}: stub JSON: {error}: {stdout}"))?;
                assert_eq!(document["written"], true, "{needle}");
                if fixture == FIXTURE_5471 {
                    // Two inline test modules: one is picked, not refused.
                    assert_eq!(
                        document["placement"]["kind"], "existing_inline_module",
                        "{needle}"
                    );
                }
                let test_name = document["test_name"]
                    .as_str()
                    .ok_or_else(|| format!("{needle}: test_name"))?
                    .to_string();
                if needle == "x > 40" {
                    // The stub lands in `tests`, the nearest plain module
                    // after `early`, not the gated one or `more_tests`.
                    let written = std::fs::read_to_string(root.join("src/lib.rs"))
                        .map_err(|error| error.to_string())?;
                    let module = |name: &str| {
                        written
                            .find(&format!("mod {name} {{"))
                            .ok_or_else(|| format!("{needle}: `mod {name}` is kept"))
                    };
                    let stub = written
                        .find(&format!("fn {test_name}("))
                        .ok_or_else(|| format!("{needle}: the stub is written"))?;
                    // The closing brace of `mod tests`, so a top-level test
                    // written between the two modules does not pass.
                    let open = module("tests")? + "mod tests ".len();
                    let mut depth = 0usize;
                    let close = written[open..]
                        .char_indices()
                        .find_map(|(offset, ch)| {
                            match ch {
                                '{' => depth += 1,
                                '}' => {
                                    depth -= 1;
                                    if depth == 0 {
                                        return Some(open + offset);
                                    }
                                }
                                _ => {}
                            }
                            None
                        })
                        .ok_or_else(|| format!("{needle}: `mod tests` closes"))?;
                    assert!(
                        open < stub && stub < close && close < module("more_tests")?,
                        "{needle}: the stub goes in `tests`:\n{written}"
                    );
                }
                let binary = root.join(format!("stub_oracle{}", std::env::consts::EXE_SUFFIX));
                let mut rustc = Command::new("rustc");
                rustc
                    .args(["--edition=2024", "--test", "--crate-name", "stub_oracle"])
                    .arg(root.join("src/lib.rs"))
                    .arg("-o")
                    .arg(&binary);
                let build = run_bounded(rustc, &root, "rustc", Duration::from_mins(2))?;
                assert!(
                    build.status.success(),
                    "{needle}: the written stub must compile:\n{}\n{}",
                    String::from_utf8_lossy(&build.stderr),
                    std::fs::read_to_string(root.join("src/lib.rs")).unwrap_or_default()
                );
                let mut test = Command::new(&binary);
                test.arg(&test_name);
                let run = run_bounded(test, &root, "test", Duration::from_secs(30))?;
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&run.stdout),
                    String::from_utf8_lossy(&run.stderr)
                );
                assert!(
                    !run.status.success(),
                    "{needle}: the stub must fail: {text}"
                );
                assert!(text.contains("1 failed"), "{needle}: one test ran: {text}");
                assert!(
                    text.contains("not yet implemented: ripr:"),
                    "{needle}: the stub must stop at its own todo: {text}"
                );
            }
        }
        scratch.cleanup()?;
    }
    Ok(())
}

/// #5453: a producer-admitted `new_integration_file` stub must compile under
/// the cargo command the CLI prints, including a `crate::` parameter type and
/// a crate-root `pub const` boundary. rustc-on-lib.rs cannot see this path.
#[test]
fn written_integration_stub_compiles_under_cargo_test_and_stops_at_its_own_todo()
-> Result<(), String> {
    const SOURCE: &str = r#"pub const LIMIT: u32 = 10;

pub struct Tag;

pub fn price(amount: u32, tag: crate::Tag) -> u32 {
    let _ = tag;
    if amount >= LIMIT { amount - 10 } else { amount }
}

pub mod extra {
    pub struct Unused;
}
"#;
    let scratch = Scratch::new()?;
    let root = scratch.directory.clone();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"stub_integration\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n# Keep this scratch crate out of a parent Cargo workspace.\n[workspace]\n",
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join("src/lib.rs"), SOURCE).map_err(|error| error.to_string())?;
    std::fs::create_dir(root.join("tests")).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("tests/smoke.rs"),
        "#[test]\nfn crate_compiles() {}\n",
    )
    .map_err(|error| error.to_string())?;

    let line = SOURCE
        .lines()
        .position(|text| text.contains("amount >= LIMIT"))
        .map(|index| index + 1)
        .ok_or_else(|| "fixture has `amount >= LIMIT`".to_string())?;
    let stub = ripr_stub_write(&root, &format!("src/lib.rs:{line}"));
    let output = run_bounded(stub, &root, "stub", Duration::from_mins(2))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(output.status.success(), "stub: {stdout}{stderr}");
    let document: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|error| format!("stub JSON: {error}: {stdout}"))?;
    assert_eq!(
        document["placement"]["kind"], "new_integration_file",
        "precondition: established tests/ layout must route to an integration file: {stdout}"
    );
    assert_eq!(document["written"], true, "{stdout}");
    let derived = document["derived_inputs"]
        .as_array()
        .ok_or_else(|| format!("derived_inputs: {stdout}"))?;
    assert!(
        derived
            .iter()
            .any(|value| value.as_str() == Some("amount = LIMIT")),
        "public const boundary must be a derived input: {stdout}"
    );
    let test_name = document["test_name"]
        .as_str()
        .ok_or_else(|| "test_name".to_string())?;
    let file = document["placement"]["file"]
        .as_str()
        .ok_or_else(|| "placement.file".to_string())?;
    assert_eq!(file, "tests/price.rs", "{stdout}");
    let written = std::fs::read_to_string(root.join(file)).map_err(|error| error.to_string())?;
    assert!(written.contains("use stub_integration::*;"), "{written}");
    assert!(
        written.contains("let amount: u32 = LIMIT;"),
        "public const boundary must remain a derived input: {written}"
    );
    assert!(
        written.contains("let tag: stub_integration::Tag = todo!("),
        "crate:: parameter types must rebase to the crate name: {written}"
    );
    assert!(
        !written.contains("crate::"),
        "leftover crate:: would name the test crate, not the library: {written}"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("src/lib.rs")).map_err(|error| error.to_string())?,
        SOURCE,
        "an integration write must not touch the owner file"
    );

    let run = document["run_command"]
        .as_str()
        .ok_or_else(|| format!("run_command: {stdout}"))?;
    assert!(
        run.contains("--test price") && run.contains(test_name),
        "{run}"
    );
    let cargo = cargo_from_run_command(run, &root)?;
    let cargo_run = run_bounded(cargo, &root, "cargo-test", Duration::from_mins(2))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&cargo_run.stdout),
        String::from_utf8_lossy(&cargo_run.stderr)
    );
    if cargo_run.status.success() {
        return Err(format!(
            "the stub must fail at its labelled todo, but cargo test passed:\n{text}\n{written}"
        ));
    }
    if (text.contains("could not compile") || text.contains("error: "))
        && !text.contains("not yet implemented: ripr:")
    {
        return Err(format!(
            "the written stub must compile under cargo test:\n{text}\n{written}"
        ));
    }
    if !text.contains("not yet implemented: ripr:") {
        return Err(format!(
            "the stub must stop at its own todo: {text}\n{written}"
        ));
    }
    scratch.cleanup()?;
    Ok(())
}

fn cargo_from_run_command(run: &str, root: &Path) -> Result<Command, String> {
    let mut parts = run.split_whitespace();
    let program = parts
        .next()
        .ok_or_else(|| format!("empty run_command: {run}"))?;
    if program != "cargo" {
        return Err(format!("run_command must start with cargo, got {run}"));
    }
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .args(parts)
        .current_dir(root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TERM_COLOR", "never");
    Ok(command)
}

fn ripr_stub_write(root: &Path, at: &str) -> Command {
    let mut stub = Command::new(env!("CARGO_BIN_EXE_ripr"));
    stub.args(["agent", "stub", "--root"])
        .arg(root)
        .args(["--at", at, "--write", "--json"]);
    stub
}

struct Scratch {
    directory: PathBuf,
    cleanup_attempted: bool,
}

impl Scratch {
    fn new() -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "ripr-agent-stub-compiles-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        // Own the directory exclusively; never adopt a preexisting one.
        std::fs::create_dir(&directory).map_err(|error| error.to_string())?;
        let scratch = Self {
            directory,
            cleanup_attempted: false,
        };
        std::fs::create_dir(scratch.directory.join("src")).map_err(|error| error.to_string())?;
        Ok(scratch)
    }

    fn cleanup(mut self) -> Result<(), String> {
        self.cleanup_attempted = true;
        std::fs::remove_dir_all(&self.directory)
            .map_err(|error| format!("cleanup failed at {}: {error}", self.directory.display()))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if self.cleanup_attempted {
            return;
        }
        if std::thread::panicking() {
            eprintln!(
                "retained failed stub fixture at {}",
                self.directory.display()
            );
            return;
        }
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

const STREAM_LIMIT: u64 = 256 * 1024;

fn read_bounded_stream(path: &Path) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.take(STREAM_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > STREAM_LIMIT {
        return Err(format!(
            "stream {} exceeds {STREAM_LIMIT} bytes",
            path.display()
        ));
    }
    Ok(bytes)
}

/// File-backed streams avoid pipe backpressure; the shared process owner
/// terminates and reaps on timeout before the scratch directory is removed.
fn run_bounded(
    mut command: Command,
    root: &Path,
    label: &str,
    budget: Duration,
) -> Result<Output, String> {
    let stdout = root.join(format!("{label}.stdout"));
    let stderr = root.join(format!("{label}.stderr"));
    command
        .stdin(Stdio::null())
        .stdout(std::fs::File::create(&stdout).map_err(|error| error.to_string())?)
        .stderr(std::fs::File::create(&stderr).map_err(|error| error.to_string())?);
    let mut child =
        ripr::process_owner::OwnedProcess::spawn(command).map_err(|error| error.to_string())?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if started.elapsed() >= budget {
            child.terminate_tree()?;
            return Err(format!(
                "{label} exceeded its {budget:?} budget; owned process terminated and reaped"
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    Ok(Output {
        status,
        stdout: read_bounded_stream(&stdout)?,
        stderr: read_bounded_stream(&stderr)?,
    })
}
