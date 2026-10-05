//! One long-lived thread for every LSP analysis refresh.
//!
//! Refreshes used to run through `tokio::task::spawn_blocking`, which hands
//! each one to whichever blocking-pool thread is free. glibc gives each
//! thread its own malloc arena and keeps freed pages in it, so after a few
//! refreshes landed on different threads the server held one analysis peak
//! per arena: on ripr-swarm's 15-commit diff RSS grew from ~590 MB after the
//! first refresh to ~1.15 GB after the second, while a single-arena run stayed
//! at ~635 MB. Running every refresh on one thread keeps it in one arena.
//!
//! Refreshes are already serialized by the scheduler's execution gate, so a
//! single thread costs no concurrency. A job that panics fails only its own
//! caller, as with `spawn_blocking`: the thread catches the unwind and goes on
//! to the jobs queued behind it. If the thread is ever gone, the next job
//! starts a fresh one.

use std::sync::Mutex;
use std::sync::mpsc;

type Job = Box<dyn FnOnce() + Send + 'static>;

#[cfg(test)]
pub(super) type RecordedThread = (std::thread::ThreadId, Option<String>);

pub(super) struct AnalysisThread {
    sender: Mutex<Option<mpsc::Sender<Job>>>,
    /// Identity and name of the thread that ran each job, so backend tests
    /// can prove refreshes reach this one thread rather than the blocking
    /// pool (#6632).
    #[cfg(test)]
    job_threads: std::sync::Arc<Mutex<Vec<RecordedThread>>>,
}

impl Default for AnalysisThread {
    fn default() -> Self {
        Self {
            sender: Mutex::new(None),
            #[cfg(test)]
            job_threads: std::sync::Arc::default(),
        }
    }
}

impl AnalysisThread {
    /// Runs `job` on the analysis thread and waits for its result without
    /// blocking the async runtime.
    pub(super) async fn run<T, F>(&self, job: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let (result_sender, result) = tokio::sync::oneshot::channel();
        #[cfg(test)]
        let job_threads = std::sync::Arc::clone(&self.job_threads);
        self.submit(Box::new(move || {
            #[cfg(test)]
            {
                let current = std::thread::current();
                lock_ignoring_poison(&job_threads)
                    .push((current.id(), current.name().map(str::to_owned)));
            }
            // The receiver is gone only when the refresh was dropped; there
            // is nobody left to tell.
            let _ = result_sender.send(job());
        }))?;
        result.await.map_err(|stopped| {
            format!(
                "the analysis job ended without a result ({stopped}; it panicked or was \
                 dropped before running); save again to retry, and report it if it repeats"
            )
        })
    }

    /// Threads recorded for every job that has started, in order.
    #[cfg(test)]
    pub(super) fn job_threads_for_test(&self) -> Vec<RecordedThread> {
        lock_ignoring_poison(&self.job_threads).clone()
    }

    fn submit(&self, job: Job) -> Result<(), String> {
        let mut sender = lock_ignoring_poison(&self.sender);
        let job = match sender.as_ref() {
            Some(live) => match live.send(job) {
                Ok(()) => return Ok(()),
                // The previous thread is gone: start another.
                Err(mpsc::SendError(job)) => job,
            },
            None => job,
        };
        let (fresh, jobs) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("ripr-lsp-analysis".to_owned())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    // A panicking job drops its result sender while unwinding,
                    // so its own caller still sees the failure; the jobs queued
                    // behind it keep running on this thread and arena.
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
                }
            })
            .map_err(|err| {
                format!(
                    "could not start the ripr analysis thread ({err}); \
                     free system threads or memory, then save again to retry"
                )
            })?;
        fresh.send(job).map_err(|unsent| {
            format!(
                "the new ripr analysis thread exited before taking work ({unsent}); \
                 save again to retry"
            )
        })?;
        *sender = Some(fresh);
        Ok(())
    }
}

fn lock_ignoring_poison<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use super::{AnalysisThread, Job};
    use std::sync::{Mutex, mpsc};

    const ANALYSIS_THREAD_NAME: &str = "ripr-lsp-analysis";

    fn runtime() -> Result<tokio::runtime::Runtime, String> {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .map_err(|err| err.to_string())
    }

    #[test]
    fn every_job_runs_on_the_same_thread() -> Result<(), String> {
        let worker = AnalysisThread::default();
        runtime()?.block_on(async {
            let first = worker.run(|| std::thread::current().id()).await?;
            let second = worker.run(|| std::thread::current().id()).await?;
            assert_eq!(first, second);
            assert_ne!(first, std::thread::current().id());
            Ok(())
        })
    }

    #[test]
    fn a_panicking_job_fails_only_its_caller_and_the_thread_keeps_running() -> Result<(), String> {
        let worker = AnalysisThread::default();
        runtime()?.block_on(async {
            let before = worker.run(|| std::thread::current().id()).await?;
            // A runtime index failure stands in for an analysis bug; the index
            // is derived at runtime so the compiler cannot const-prove it.
            let out_of_bounds = std::env::args_os().count() + 1;
            let Err(err) = worker
                .run(move || {
                    let slots = [0u8];
                    slots[out_of_bounds]
                })
                .await
            else {
                return Err("a panicking job must report a failure".to_owned());
            };
            // Only the result channel closing produces this message; a failed
            // spawn or a thread that exits before taking the job reads
            // differently. The job also started, so it was not dropped
            // unrun: the panic is what closed the channel.
            assert!(
                err.starts_with("the analysis job ended without a result"),
                "{err}"
            );
            assert_eq!(worker.job_threads_for_test().len(), 2);
            let after = worker.run(|| std::thread::current().id()).await?;
            assert_eq!(before, after, "the thread survives the panic");
            Ok(())
        })
    }

    #[test]
    fn a_job_queued_behind_a_panicking_job_still_runs() -> Result<(), String> {
        let worker = AnalysisThread::default();
        let out_of_bounds = std::env::args_os().count() + 1;
        runtime()?.block_on(async {
            // `join!` submits both jobs before either finishes: the second
            // waits in the channel while the first sleeps and then panics.
            let (failed, queued) = tokio::join!(
                worker.run(move || {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    let slots = [0u8];
                    slots[out_of_bounds]
                }),
                worker.run(|| 7u8),
            );
            assert!(failed.is_err(), "the panicking job reports a failure");
            assert_eq!(queued?, 7);
            Ok(())
        })
    }

    #[test]
    fn a_job_sent_after_the_thread_is_gone_starts_a_fresh_one() -> Result<(), String> {
        // A sender whose receiver is dropped fails every send, exactly as it
        // would after the analysis thread exited.
        let (dead, gone) = mpsc::channel::<Job>();
        drop(gone);
        let worker = AnalysisThread {
            sender: Mutex::new(Some(dead)),
            ..AnalysisThread::default()
        };
        runtime()?.block_on(async {
            let first = worker
                .run(|| {
                    let current = std::thread::current();
                    (current.id(), current.name().map(str::to_owned))
                })
                .await?;
            assert_eq!(first.1.as_deref(), Some(ANALYSIS_THREAD_NAME));
            // The fresh thread replaced the dead sender: the next job reuses
            // it instead of starting a third.
            let second = worker.run(|| std::thread::current().id()).await?;
            assert_eq!(first.0, second);
            assert_eq!(worker.job_threads_for_test().len(), 2);
            Ok(())
        })
    }
}
