//! Settle the synchronous descendants of one timed command, including nested
//! command adapters that created their own process groups. This is local to the
//! existing runner: no unrelated process tree or caller group is adopted.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
struct Process {
    pid: u32,
    parent: u32,
    group: u32,
    started: u64,
    state: char,
}

impl Process {
    fn exited(&self) -> bool {
        matches!(self.state, 'Z' | 'X' | 'x')
    }

    fn stopped(&self) -> bool {
        self.exited() || matches!(self.state, 'T' | 't')
    }
}

fn process(pid: u32) -> Result<Option<Process>, String> {
    let text = match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("read owned process {pid}: {error}")),
    };
    let (head, tail) = text
        .rsplit_once(')')
        .ok_or_else(|| format!("owned process {pid} has malformed stat"))?;
    if head.split_whitespace().next() != Some(pid.to_string().as_str()) {
        return Err(format!("process metadata identity mismatch for {pid}"));
    }
    let fields = tail.split_whitespace().collect::<Vec<_>>();
    let field = |index: usize| {
        fields
            .get(index)
            .copied()
            .ok_or_else(|| format!("owned process {pid} lacks stat field {index}"))
    };
    Ok(Some(Process {
        pid,
        parent: field(1)?
            .parse()
            .map_err(|error| format!("process {pid} parent: {error}"))?,
        group: field(2)?
            .parse()
            .map_err(|error| format!("process {pid} group: {error}"))?,
        started: field(19)?
            .parse()
            .map_err(|error| format!("process {pid} start: {error}"))?,
        state: field(0)?
            .chars()
            .next()
            .ok_or_else(|| format!("process {pid} state is empty"))?,
    }))
}

fn same_process(owned: &Process) -> Result<Option<Process>, String> {
    Ok(process(owned.pid)?.filter(|current| current.started == owned.started))
}

fn thread_states(owned: &Process, deadline: Instant) -> Result<Option<Vec<char>>, String> {
    let tasks = match fs::read_dir(format!("/proc/{}/task", owned.pid)) {
        Ok(tasks) => tasks,
        Err(error) if error.kind() == ErrorKind::NotFound && same_process(owned)?.is_none() => {
            return Ok(None);
        }
        Err(error) => return Err(format!("read owned process {} threads: {error}", owned.pid)),
    };
    let mut states = Vec::new();
    let mut leader_present = false;
    for entry in tasks {
        if Instant::now() >= deadline {
            return Err("owned thread snapshot exceeded settlement grace".to_string());
        }
        let entry = entry.map_err(|error| format!("read owned thread: {error}"))?;
        let tid = entry
            .file_name()
            .to_string_lossy()
            .parse::<u32>()
            .map_err(|error| format!("owned thread identity: {error}"))?;
        if let Some(thread) = process(tid)? {
            leader_present |= tid == owned.pid;
            states.push(thread.state);
        }
    }
    if same_process(owned)?.is_none() {
        return Ok(None);
    }
    if !leader_present || states.is_empty() {
        return Err(format!(
            "incomplete task snapshot for owned process {}",
            owned.pid
        ));
    }
    Ok(Some(states))
}

fn task_states_exited(states: &[char]) -> Result<bool, String> {
    if states.is_empty() {
        return Err("empty task snapshot cannot establish exit".to_string());
    }
    Ok(states.iter().all(|state| matches!(state, 'Z' | 'X' | 'x')))
}

fn threads_stopped(owned: &Process, deadline: Instant) -> Result<bool, String> {
    Ok(thread_states(owned, deadline)?.is_none_or(|states| {
        states
            .iter()
            .all(|state| matches!(state, 'Z' | 'X' | 'x' | 'T' | 't'))
    }))
}

fn threads_exited(owned: &Process, deadline: Instant) -> Result<bool, String> {
    match thread_states(owned, deadline)? {
        None => Ok(true),
        Some(states) => task_states_exited(&states),
    }
}

fn validate_snapshot_anchors(
    snapshot: &BTreeMap<u32, Process>,
    owned: &[Process],
) -> Result<(), String> {
    for expected in owned {
        let current = snapshot.get(&expected.pid).ok_or_else(|| {
            format!(
                "incomplete process snapshot: owned identity {} is absent",
                expected.pid
            )
        })?;
        if current.started != expected.started || current.group != expected.group {
            return Err(format!(
                "process snapshot changed owned identity {}",
                expected.pid
            ));
        }
    }
    Ok(())
}

fn process_index(deadline: Instant) -> Result<BTreeMap<u32, Process>, String> {
    // Some supported kernels omit /proc/PID/task/TID/children. PPID in stat
    // identifies the parent process even when another thread spawned the child.
    // This ephemeral metadata snapshot grants no ownership by itself.
    let mut entries = BTreeMap::new();
    for entry in fs::read_dir("/proc").map_err(|error| format!("read process metadata: {error}"))? {
        if Instant::now() >= deadline {
            return Err("process metadata snapshot exceeded settlement grace".to_string());
        }
        let entry = entry.map_err(|error| format!("read process metadata entry: {error}"))?;
        if let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>()
            && let Some(current) = process(pid)?
        {
            entries.insert(pid, current);
        }
    }
    Ok(entries)
}

/// The direct child remains unreaped while this runs. Stop each owned group
/// before walking its children: synchronous supervisors cannot then
/// reap a discovered subgroup leader or spawn more children during discovery.
/// Already daemonized/session-escaped processes are not claimed by this rooted
/// synchronous-child contract.
pub(super) fn terminate(
    root: u32,
    grace: Duration,
    mut signal: impl FnMut(&str, &str) -> bool,
) -> Result<bool, String> {
    let _self_identity =
        process(std::process::id())?.ok_or("runner process metadata is unavailable")?;
    let Some(root_process) = process(root)? else {
        return Ok(false);
    };
    if root_process.group != root || root == std::process::id() {
        return Err(format!(
            "process {root} is not an exclusive runner-owned group"
        ));
    }
    let deadline = Instant::now() + grace;
    let mut owned = vec![root_process];
    let mut groups = BTreeSet::from([root]);
    let mut snapshot = BTreeMap::new();
    let discovery = (|| {
        let mut index = 0;
        while index < owned.len() {
            if Instant::now() >= deadline {
                return Err("owned process discovery exceeded settlement grace".to_string());
            }
            let original = &owned[index];
            let Some(mut current) = same_process(original)? else {
                return Err(format!(
                    "owned process {} vanished during discovery",
                    original.pid
                ));
            };
            let target = if current.group == current.pid {
                groups.insert(current.group);
                format!("-{}", current.group)
            } else {
                current.pid.to_string()
            };
            if (current.group == current.pid || !current.stopped()) && !signal("-STOP", &target) {
                return Err(format!("failed to stop owned process {}", current.pid));
            }
            while !current.stopped() {
                if Instant::now() >= deadline {
                    return Err(format!("owned process {} did not stop", current.pid));
                }
                thread::sleep(Duration::from_millis(5));
                current = same_process(original)?.ok_or_else(|| {
                    format!(
                        "owned process {} vanished before stop confirmation",
                        original.pid
                    )
                })?;
            }
            if current.group == current.pid || !groups.contains(&current.group) {
                // Confirm a stable stopped group before using its child index.
                // A process may have forked while the stop was being delivered.
                let mut previous = BTreeSet::new();
                loop {
                    snapshot = process_index(deadline)?;
                    validate_snapshot_anchors(&snapshot, &owned)?;
                    let members = snapshot
                        .values()
                        .filter(|entry| {
                            if groups.contains(&current.group) {
                                entry.group == current.group
                            } else {
                                entry.pid == current.pid
                            }
                        })
                        .collect::<Vec<_>>();
                    let identities = members
                        .iter()
                        .map(|entry| (entry.pid, entry.started))
                        .collect::<BTreeSet<_>>();
                    let mut quiescent = members.iter().all(|entry| entry.stopped());
                    for entry in &members {
                        if !threads_stopped(entry, deadline)? {
                            quiescent = false;
                        }
                    }
                    if identities == previous && quiescent {
                        break;
                    }
                    previous = identities;
                }
            }
            {
                let children = snapshot
                    .values()
                    .filter(|entry| entry.parent == current.pid || groups.contains(&entry.group))
                    .cloned()
                    .collect::<Vec<_>>();
                for child in children {
                    let pid = child.pid;
                    if owned.iter().any(|entry| entry.pid == pid) {
                        continue;
                    }
                    let observed = same_process(&child)?
                        .ok_or_else(|| format!("owned child {pid} vanished"))?;
                    if (observed.parent != current.pid && !groups.contains(&observed.group))
                        || observed.group != child.group
                    {
                        return Err(format!(
                            "owned child {pid} changed ownership before discovery"
                        ));
                    }
                    owned.push(child);
                }
            }
            index += 1;
        }
        Ok(())
    })();

    // This cleanup also runs after partial discovery failure. Never leave an
    // owned group stopped, and never target an inherited/unrelated group.
    for entry in owned.iter().rev() {
        // Frozen ancestors cannot reap these identities. Kill leaves first,
        // keeping each ancestor alive until its children's signals are sent.
        // In particular, a procfs error must not skip cleanup of stopped work.
        let current = match same_process(entry) {
            Ok(Some(current)) => Some(current),
            Ok(None) => continue,
            // Its frozen parent still holds the unreaped child identity. A
            // metadata read error must not strand that already-owned process.
            Err(_) => None,
        };
        let target = if groups.contains(&entry.pid)
            && current
                .as_ref()
                .is_none_or(|current| current.group == entry.pid)
        {
            format!("-{}", entry.pid)
        } else {
            entry.pid.to_string()
        };
        let _ = signal("-KILL", &target);
    }
    loop {
        let mut live = Vec::new();
        for entry in &owned {
            if let Some(current) = same_process(entry)?
                && (!current.exited() || !threads_exited(&current, deadline)?)
            {
                live.push(current.pid);
            }
        }
        if live.is_empty() {
            return discovery.map(|()| true);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "owned processes did not settle: {live:?}; discovery: {discovery:?}"
            ));
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::{Process, task_states_exited, validate_snapshot_anchors};
    use std::collections::BTreeMap;

    #[test]
    fn empty_or_changed_snapshot_cannot_claim_owned_tree_settlement() -> Result<(), String> {
        let owned = Process {
            pid: 41,
            parent: 40,
            group: 41,
            started: 99,
            state: 'T',
        };
        if validate_snapshot_anchors(&BTreeMap::new(), std::slice::from_ref(&owned)).is_ok() {
            return Err("empty snapshot accepted a known owned process".to_string());
        }
        let mut changed = owned.clone();
        changed.started += 1;
        if validate_snapshot_anchors(
            &BTreeMap::from([(41, changed)]),
            std::slice::from_ref(&owned),
        )
        .is_ok()
        {
            return Err("reused PID accepted as the owned identity".to_string());
        }
        validate_snapshot_anchors(&BTreeMap::from([(41, owned.clone())]), &[owned])
    }

    #[test]
    fn zombie_leader_cannot_hide_live_or_stopped_worker_threads() -> Result<(), String> {
        for states in [&['Z', 'R'][..], &['Z', 'T'][..], &['Z', 'D'][..]] {
            if task_states_exited(states)? {
                return Err(format!("live task set claimed exited: {states:?}"));
            }
        }
        if task_states_exited(&[]).is_ok() {
            return Err("missing tasks claimed exited".to_string());
        }
        if !task_states_exited(&['Z', 'X'])? {
            return Err("exited tasks were not recognized".to_string());
        }
        Ok(())
    }
}
