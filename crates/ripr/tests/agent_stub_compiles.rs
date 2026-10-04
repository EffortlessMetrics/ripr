//! #5355: `ripr agent stub --write` must leave the user's crate compiling.
//! Each ready stub is written into a fresh copy of one fixture crate, the
//! file is compiled with rustc as a test crate, and the stub's test must run
//! and stop at its own `ripr:` `todo!()`. Cases the producer cannot make
//! compile must be refused with a named reason instead.
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
        ("self.max > 3", Expect::Refused("owner_trait_method")),
    ];
    for (needle, expect) in cases {
        let line = FIXTURE
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
        std::fs::write(root.join("src/lib.rs"), FIXTURE).map_err(|error| error.to_string())?;

        let mut stub = Command::new(env!("CARGO_BIN_EXE_ripr"));
        stub.args(["agent", "stub", "--root"]).arg(&root).args([
            "--at",
            &format!("src/lib.rs:{line}"),
            "--write",
            "--json",
        ]);
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
                    FIXTURE,
                    "{needle}: a refusal must not touch the file"
                );
            }
            Expect::StopsAtRiprTodo => {
                assert!(output.status.success(), "{needle}: {stdout}{stderr}");
                let document: serde_json::Value = serde_json::from_str(&stdout)
                    .map_err(|error| format!("{needle}: stub JSON: {error}: {stdout}"))?;
                assert_eq!(document["written"], true, "{needle}");
                let test_name = document["test_name"]
                    .as_str()
                    .ok_or_else(|| format!("{needle}: test_name"))?
                    .to_string();
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
