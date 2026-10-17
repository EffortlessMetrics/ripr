use super::{capture_output_in_dir_with_timeout_bounded, capture_output_with_timeout};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CASE: &str = "RIPR_NESTED_RUNNER_TEST_CASE";
const ROOT: &str = "RIPR_NESTED_RUNNER_TEST_ROOT";
const TEST: &str = "run::nested_process_tests::nested_deadlines_settle_only_owned_work";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("ripr-nested-runner-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).map_err(|error| error.to_string())?;
        Ok(Self(path))
    }
}

fn identity(pid: u32) -> Result<Option<(u64, u32, char)>, String> {
    let text = match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let (_, tail) = text.rsplit_once(')').ok_or("invalid child process stat")?;
    let fields = tail.split_whitespace().collect::<Vec<_>>();
    let born = fields
        .get(19)
        .ok_or("missing child start")?
        .parse::<u64>()
        .map_err(|error| error.to_string())?;
    let group = fields
        .get(2)
        .ok_or("missing child group")?
        .parse::<u32>()
        .map_err(|error| error.to_string())?;
    let state = fields
        .first()
        .and_then(|field| field.chars().next())
        .ok_or("missing child state")?;
    Ok(Some((born, group, state)))
}

fn record_child(root: &Path) -> Result<(), String> {
    let pid = std::process::id();
    let (born, _, _) = identity(pid)?.ok_or("child metadata unavailable")?;
    fs::write(root.join("child.pid"), format!("{pid} {born}")).map_err(|error| error.to_string())
}

fn child_record(path: &Path) -> Result<(u32, u64), String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut words = text.split_whitespace();
    let pid = words
        .next()
        .ok_or("missing child PID")?
        .parse::<u32>()
        .map_err(|error| error.to_string())?;
    let born = words
        .next()
        .ok_or("missing child start")?
        .parse::<u64>()
        .map_err(|error| error.to_string())?;
    Ok((pid, born))
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // A failed regression must not leave its own deliberately slow child.
        // PID/start/group identity protects unrelated or reused process IDs.
        if let Ok(entries) = fs::read_dir(&self.0) {
            for entry in entries.flatten() {
                if let Ok((pid, born)) = child_record(&entry.path().join("child.pid"))
                    && let Ok(Some((current, group, state))) = identity(pid)
                    && current == born
                    && group == pid
                    && !matches!(state, 'Z' | 'X' | 'x')
                {
                    let _ = super::signal_timed_process("-KILL", &format!("-{pid}"));
                }
            }
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn child_case(case: &str, root: &Path) -> Result<(), String> {
    if case == "leaf" {
        record_child(root)?;
        println!("owned child stdout");
        eprintln!("owned child stderr");
        thread::sleep(Duration::from_secs(4));
        fs::write(root.join("late.marker"), b"continued").map_err(|error| error.to_string())?;
        return Ok(());
    }
    if case == "sentinel" {
        record_child(root)?;
        let started = Instant::now();
        while !root.join("release").exists() {
            if started.elapsed() > Duration::from_secs(15) {
                return Err("unrelated sentinel was not released".to_string());
            }
            thread::sleep(Duration::from_millis(10));
        }
        fs::write(root.join("remained.marker"), b"remained").map_err(|error| error.to_string())?;
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let args = vec![
        "--exact".to_string(),
        TEST.to_string(),
        "--nocapture".to_string(),
    ];
    let root_text = root.to_string_lossy().into_owned();
    let envs = [(CASE, "leaf"), (ROOT, root_text.as_str())];
    if matches!(case, "same-group-supervisor" | "exited-group-leader") {
        let mut child =
            super::spawn_timeout_test_child(&exe.to_string_lossy(), &args, &envs, false)?;
        if case == "same-group-supervisor" {
            let _status = child.wait().map_err(|error| error.to_string())?;
        }
        return Ok(());
    }
    let inner_timeout = if case == "inner-short" {
        Duration::from_millis(600)
    } else {
        Duration::from_secs(8)
    };
    // Forking from a non-main thread must retain the same rooted ownership.
    let exe_text = exe.to_string_lossy().into_owned();
    let args_owned = args.clone();
    let env_owned = envs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<Vec<_>>();
    let output = thread::spawn(move || {
        let refs = env_owned
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        capture_output_with_timeout(
            &exe_text,
            &args_owned,
            &refs,
            inner_timeout,
            "thread-owned nested child",
        )
    })
    .join()
    .map_err(|panic_payload| {
        format!("nested child adapter thread panicked: {panic_payload:?}")
    })??;
    if !output.timed_out
        || output
            .status
            .as_ref()
            .and_then(|status| status.code())
            .is_some()
    {
        return Err(format!(
            "inner deadline lost timeout/exit identity: {:?}",
            output.status
        ));
    }
    if !output.stdout.contains("owned child stdout")
        || !output.stderr.contains("owned child stderr")
    {
        return Err("inner timeout lost child diagnostics".to_string());
    }
    println!("inner timeout settled with captured diagnostics");
    Ok(())
}

#[test]
fn nested_deadlines_settle_only_owned_work() -> Result<(), String> {
    if let Ok(case) = std::env::var(CASE) {
        return child_case(
            &case,
            &PathBuf::from(std::env::var(ROOT).map_err(|error| error.to_string())?),
        );
    }
    let _cwd = crate::acquire_test_cwd_read_guard();
    let fixture = Fixture::new()?;
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let args = vec![
        "--exact".to_string(),
        TEST.to_string(),
        "--nocapture".to_string(),
    ];
    let sentinel_root = fixture.0.join("sentinel");
    fs::create_dir(&sentinel_root).map_err(|error| error.to_string())?;
    let sentinel_exe = exe.clone();
    let sentinel_args = args.clone();
    let sentinel_text = sentinel_root.to_string_lossy().into_owned();
    let sentinel = thread::spawn(move || {
        capture_output_with_timeout(
            &sentinel_exe.to_string_lossy(),
            &sentinel_args,
            &[(CASE, "sentinel"), (ROOT, &sentinel_text)],
            Duration::from_secs(20),
            "independent sentinel",
        )
    });
    let result = (|| {
        let ready = Instant::now();
        while !sentinel_root.join("child.pid").exists() {
            if ready.elapsed() > Duration::from_secs(2) {
                return Err("sentinel did not start".to_string());
            }
            thread::sleep(Duration::from_millis(10));
        }
        for (name, bound) in [
            ("inner-long", Duration::from_secs(2)),
            ("inner-short", Duration::from_secs(10)),
        ] {
            let root = fixture.0.join(name);
            fs::create_dir(&root).map_err(|error| error.to_string())?;
            let root_text = root.to_string_lossy().into_owned();
            let output = capture_output_in_dir_with_timeout_bounded(
                &exe,
                &args,
                &[(CASE, name), (ROOT, &root_text)],
                &root,
                bound,
                16384,
                "nested outer command",
            )?;
            let (pid, born) = child_record(&root.join("child.pid"))?;
            if identity(pid)?.is_some_and(|(current, _, state)| {
                current == born && !matches!(state, 'Z' | 'X' | 'x')
            }) {
                return Err(format!(
                    "owned nested child {pid} still runs after outer return"
                ));
            }
            if root.join("late.marker").exists() {
                return Err("nested child continued after deadline".to_string());
            }
            if name == "inner-long" {
                if !output.timed_out
                    || output
                        .status
                        .as_ref()
                        .and_then(|status| status.code())
                        .is_some()
                {
                    return Err("outer timeout lost signal exit identity".to_string());
                }
            } else if output.timed_out
                || !output.status.is_some_and(|status| status.success())
                || !output
                    .stdout
                    .contains("inner timeout settled with captured diagnostics")
            {
                return Err(format!(
                    "inner timeout terminated supervisor or lost diagnostics: {}",
                    output.stderr
                ));
            }
            let (sentinel_pid, sentinel_born) = child_record(&sentinel_root.join("child.pid"))?;
            if !identity(sentinel_pid)?.is_some_and(|(current, _, state)| {
                current == sentinel_born && !matches!(state, 'Z' | 'X' | 'x')
            }) {
                return Err("unrelated sentinel was signalled".to_string());
            }
        }
        Ok(())
    })();
    fs::write(sentinel_root.join("release"), b"release").map_err(|error| error.to_string())?;
    let sentinel_output = sentinel
        .join()
        .map_err(|panic_payload| format!("sentinel adapter panicked: {panic_payload:?}"))??;
    if !sentinel_output
        .status
        .is_some_and(|status| status.success())
        || !sentinel_root.join("remained.marker").is_file()
    {
        return Err("sentinel did not finish successfully".to_string());
    }
    result
}

#[test]
fn discovery_errors_and_exited_leaders_settle_same_group_children() -> Result<(), String> {
    let _cwd = crate::acquire_test_cwd_read_guard();
    let fixture = Fixture::new()?;
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let args = vec![
        "--exact".to_string(),
        TEST.to_string(),
        "--nocapture".to_string(),
    ];
    for case in ["same-group-supervisor", "exited-group-leader"] {
        let root = fixture.0.join(case);
        fs::create_dir(&root).map_err(|error| error.to_string())?;
        let root_text = root.to_string_lossy().into_owned();
        let mut child = super::spawn_timeout_test_child(
            &exe.to_string_lossy(),
            &args,
            &[(CASE, case), (ROOT, &root_text)],
            true,
        )?;
        let root_pid = child.id();
        let root_born = identity(root_pid)?.ok_or("missing owned supervisor")?.0;
        let result = (|| {
            let ready = Instant::now();
            while !root.join("child.pid").exists() {
                if ready.elapsed() > Duration::from_secs(2) {
                    return Err("same-group child did not start".to_string());
                }
                thread::sleep(Duration::from_millis(10));
            }
            let (pid, born) = child_record(&root.join("child.pid"))?;
            if !identity(pid)?.is_some_and(|(current, group, state)| {
                current == born && group == root_pid && !matches!(state, 'Z' | 'X' | 'x')
            }) {
                return Err("same-group stimulus was not live and owned".to_string());
            }
            if case == "same-group-supervisor" {
                let outcome = super::terminate_after_timeout_using(
                    &mut child,
                    "discovery failure control",
                    |_| Err("injected incomplete process snapshot".to_string()),
                );
                let error = outcome
                    .err()
                    .ok_or("discovery error was silently accepted")?;
                if !error.contains("injected incomplete process snapshot")
                    || !error.contains("original owned group kill: true")
                    || !error.contains("bounded reap completed: true")
                {
                    return Err(format!("incomplete cleanup diagnostics: {error}"));
                }
                use std::os::unix::process::ExitStatusExt;
                if child
                    .try_wait()
                    .map_err(|error| error.to_string())?
                    .and_then(|status| status.signal())
                    != Some(9)
                {
                    return Err("discovery failure lost actual child exit identity".to_string());
                }
            } else {
                while identity(root_pid)?.is_none_or(|(_, _, state)| state != 'Z') {
                    if ready.elapsed() > Duration::from_secs(2) {
                        return Err("group leader did not exit".to_string());
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                if !super::linux_process_tree::terminate(
                    root_pid,
                    Duration::from_secs(5),
                    super::signal_timed_process,
                )? {
                    return Err("exited leader did not settle its owned group".to_string());
                }
            }
            if identity(pid)?.is_some_and(|(current, _, state)| {
                current == born && !matches!(state, 'Z' | 'X' | 'x')
            }) {
                return Err("same-group child remained settlement".to_string());
            }
            if root.join("late.marker").exists() {
                return Err("same-group child ran its continuation".to_string());
            }
            Ok(())
        })();
        // Cleanup also protects a deliberately broken implementation. The
        // direct Child or this fixture's recorded live descendant anchors the
        // exclusive group; unrelated/reused identities are never signalled.
        let root_matches = identity(root_pid)?
            .is_some_and(|(born, group, _)| born == root_born && group == root_pid);
        let leaf_matches = match child_record(&root.join("child.pid")) {
            Ok((pid, born)) => identity(pid)?.is_some_and(|(current, group, state)| {
                current == born && group == root_pid && !matches!(state, 'Z' | 'X' | 'x')
            }),
            Err(_) => false,
        };
        if root_matches || leaf_matches {
            let _ = super::signal_timed_process("-KILL", &format!("-{root_pid}"));
        }
        if !super::wait_for_child_exit(
            &mut child,
            Duration::from_secs(5),
            "same-group control cleanup",
        )? {
            return Err("same-group control supervisor did not settle".to_string());
        }
        result?;
    }
    Ok(())
}

#[test]
fn normal_cargo_enumeration_preserves_product_failure_and_recovery() -> Result<(), String> {
    let _cwd = crate::acquire_test_cwd_read_guard();
    let fixture = Fixture::new()?;
    fs::create_dir(fixture.0.join("src")).map_err(|error| error.to_string())?;
    fs::write(fixture.0.join("Cargo.toml"), "[package]\nname=\"runner-enumeration-fixture\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n").map_err(|error| error.to_string())?;
    let executed = fixture.0.join("test-body-ran");
    fs::write(fixture.0.join("src/lib.rs"), format!("#[test]\nfn selected() -> std::io::Result<()> {{ std::fs::write({:?}, b\"unexpected execution\") }}\n", executed)).map_err(|error| error.to_string())?;
    let manifest = fixture.0.join("Cargo.toml").to_string_lossy().into_owned();
    let target = fixture.0.join("target").to_string_lossy().into_owned();
    for (package, success) in [
        ("runner-enumeration-missing", false),
        ("runner-enumeration-fixture", true),
    ] {
        let args = [
            "test",
            "--manifest-path",
            &manifest,
            "-p",
            package,
            "selected",
            "--",
            "--list",
            "--format",
            "terse",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        let output = capture_output_with_timeout(
            env!("CARGO"),
            &args,
            &[("CARGO_TARGET_DIR", &target)],
            Duration::from_mins(3),
            "normal Cargo test enumeration",
        )?;
        if output.timed_out
            || output
                .status
                .as_ref()
                .is_none_or(|status| status.success() != success)
        {
            return Err(format!(
                "Cargo enumeration lost product disposition: {:?}; {}",
                output.status, output.stderr
            ));
        }
        if success {
            let selected = output
                .stdout
                .lines()
                .filter(|line| line.ends_with(": test"))
                .collect::<Vec<_>>();
            if selected != ["selected: test"] {
                return Err(format!("wrong real Cargo inventory: {selected:?}"));
            }
        } else if output.status.as_ref().and_then(|status| status.code()) != Some(101)
            || !output.stderr.contains("did not match any packages")
        {
            return Err(format!(
                "Cargo product failure lost exit/diagnostics: {}",
                output.stderr
            ));
        }
    }
    if executed.exists() {
        return Err("enumeration executed a test body".to_string());
    }
    Ok(())
}
