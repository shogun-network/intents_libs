//! Bounded join point for a dynamic set of async tasks.
//!
//! [`TaskTracker`] is a thin wrapper over
//! [`tokio_util::task::TaskTracker`]. It keeps that type's ergonomics —
//! spawn tasks, track external futures, query how many are still in
//! flight — and adds this crate's error contract on the one operation
//! that can fail in practice: waiting for the tracked tasks to drain
//! within a deadline.
//!
//! # Why wrap instead of using `tokio_util` directly
//! The upstream `TaskTracker` is infallible: `wait()` is an unbounded
//! future that resolves only once the tracker is closed *and* every
//! task has finished. In a service, an unbounded join is a hazard — a
//! single stuck task hangs the shutdown path forever. Wrapping it here
//! lets the bounded-wait methods return a
//! [`ModelResult`](crate::error::ModelResult), so a timeout surfaces as
//! [`Error::Concurrency`](crate::error::Error::Concurrency) and is
//! handled with the same error taxonomy as the rest of the crate,
//! instead of every call site re-implementing the timeout dance.
//!
//! # Closing is required to drain
//! [`wait`](TaskTracker::wait) and [`wait_timeout`](TaskTracker::wait_timeout)
//! only resolve once the tracker has been [`close`](TaskTracker::close)d.
//! If you just want to "spawn N tasks and wait for all of them", reach
//! for [`close_and_wait`](TaskTracker::close_and_wait), which closes
//! first and then waits under a deadline.
//!
//! # Example
//! ```no_run
//! use std::time::Duration;
//! use intents_models::utils::TaskTracker;
//!
//! # async fn run() -> intents_models::error::ModelResult<()> {
//! let tracker = TaskTracker::new();
//!
//! for i in 0..4 {
//!     tracker.spawn(async move {
//!         tokio::time::sleep(Duration::from_millis(10 * i)).await;
//!     });
//! }
//!
//! // Close the tracker and wait for every task to finish, bounded.
//! tracker.close_and_wait(Duration::from_secs(5)).await?;
//! # Ok(())
//! # }
//! ```

use std::future::Future;
use std::time::Duration;

use error_stack::Report;
use tokio::task::JoinHandle;
use tokio_util::task::TaskTracker as InnerTracker;

use crate::error::{Error, ModelResult};

/// Tracks a dynamic set of async tasks and offers a bounded join point.
///
/// Cheap to clone — the underlying `tokio_util::task::TaskTracker` is
/// internally refcounted, so every clone observes the same task set and
/// the same open/closed state.
#[derive(Debug, Clone, Default)]
pub struct TaskTracker {
    inner: InnerTracker,
}

impl TaskTracker {
    /// Create a fresh, open tracker with no tracked tasks.
    pub fn new() -> Self {
        Self {
            inner: InnerTracker::new(),
        }
    }

    /// Clone of the underlying [`tokio_util::task::TaskTracker`].
    ///
    /// Useful when a caller needs an upstream method this wrapper does
    /// not re-expose (for example `spawn_blocking` or `spawn_on`).
    pub fn inner(&self) -> InnerTracker {
        self.inner.clone()
    }

    /// Spawn `future` onto the current Tokio runtime and track it.
    ///
    /// The returned [`JoinHandle`] behaves like any other: dropping it
    /// does not cancel the task, which continues to run and is awaited
    /// by [`wait`](Self::wait) / [`close_and_wait`](Self::close_and_wait).
    ///
    /// # Panics
    /// Panics if called outside of a Tokio runtime.
    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.inner.spawn(future)
    }

    /// Track a future without spawning it onto the runtime.
    ///
    /// The returned future drives `future` to completion while counting
    /// it as an in-flight task for as long as it is polled. Use this to
    /// track work that runs on the caller's own task (for example inside
    /// a `select!`) rather than on a freshly spawned one.
    pub fn track_future<F>(&self, future: F) -> impl Future<Output = F::Output>
    where
        F: Future,
    {
        self.inner.track_future(future)
    }

    /// Close the tracker so no further tasks are accepted as "open".
    ///
    /// Closing is the precondition for [`wait`](Self::wait) to resolve.
    /// Returns `true` if this call transitioned the tracker from open to
    /// closed, `false` if it was already closed. Idempotent.
    pub fn close(&self) -> bool {
        self.inner.close()
    }

    /// Reopen a previously closed tracker.
    ///
    /// Returns `true` if this call transitioned the tracker from closed
    /// to open, `false` if it was already open.
    pub fn reopen(&self) -> bool {
        self.inner.reopen()
    }

    /// Whether the tracker is currently closed.
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    /// Number of tasks still being tracked. For metrics and tests, not
    /// for scheduling decisions.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether no tasks are currently being tracked.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Wait until the tracker is closed and every tracked task has
    /// finished.
    ///
    /// This future never resolves while the tracker is open, so callers
    /// must arrange for [`close`](Self::close) to be invoked (here or on
    /// a clone). Prefer [`wait_timeout`](Self::wait_timeout) or
    /// [`close_and_wait`](Self::close_and_wait) in service code, where an
    /// unbounded wait is a hazard.
    pub async fn wait(&self) {
        self.inner.wait().await;
    }

    /// Wait for all tracked tasks to finish, bounded by `timeout`.
    ///
    /// Does **not** close the tracker — the caller is responsible for
    /// having closed it, otherwise the wait can only ever time out. Use
    /// [`close_and_wait`](Self::close_and_wait) for the common
    /// "drain everything now" case.
    ///
    /// # Errors
    /// Returns [`Error::Concurrency`] when the deadline elapses with
    /// tasks still in flight. The count of tasks still running at the
    /// moment of the timeout is attached to the report.
    pub async fn wait_timeout(&self, timeout: Duration) -> ModelResult<()> {
        match tokio::time::timeout(timeout, self.inner.wait()).await {
            Ok(()) => Ok(()),
            Err(_) => {
                let remaining = self.inner.len();
                Err(Report::new(Error::Concurrency).attach_printable(format!(
                    "task tracker wait timed out after {timeout:?} with {remaining} task(s) still running",
                )))
            }
        }
    }

    /// Close the tracker and wait for every tracked task to finish,
    /// bounded by `timeout`.
    ///
    /// This is the convenience path for draining a known set of tasks:
    /// it [`close`](Self::close)s first (so the wait can terminate) and
    /// then delegates to [`wait_timeout`](Self::wait_timeout). Both
    /// steps are idempotent, so it is safe to call from multiple clones.
    ///
    /// # Errors
    /// Returns [`Error::Concurrency`] when the deadline elapses with
    /// tasks still in flight.
    pub async fn close_and_wait(&self, timeout: Duration) -> ModelResult<()> {
        self.inner.close();
        self.wait_timeout(timeout).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// `close_and_wait` drains every tracked task that finishes within
    /// the timeout.
    #[tokio::test]
    async fn close_and_wait_drains_all_tasks() {
        let tracker = TaskTracker::new();
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..5 {
            let counter = Arc::clone(&counter);
            tracker.spawn(async move {
                tokio::time::sleep(Duration::from_millis(20)).await;
                counter.fetch_add(1, Ordering::SeqCst);
            });
        }

        tracker
            .close_and_wait(Duration::from_secs(1))
            .await
            .expect("tasks must drain within the generous timeout");
        assert_eq!(counter.load(Ordering::SeqCst), 5);
        assert!(tracker.is_empty());
    }

    /// A task that outlives the deadline surfaces as a
    /// `Error::Concurrency` report.
    #[tokio::test]
    async fn close_and_wait_times_out_on_stuck_task() {
        let tracker = TaskTracker::new();

        tracker.spawn(async {
            tokio::time::sleep(Duration::from_secs(5)).await;
        });

        let err = tracker
            .close_and_wait(Duration::from_millis(50))
            .await
            .expect_err("timeout must fire on a stuck task");
        assert!(matches!(err.current_context(), Error::Concurrency));
    }

    /// An empty tracker drains immediately once closed.
    #[tokio::test]
    async fn close_and_wait_with_no_tasks_is_instant() {
        let tracker = TaskTracker::new();
        let start = tokio::time::Instant::now();
        tracker
            .close_and_wait(Duration::from_secs(1))
            .await
            .expect("empty wait must succeed");
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    /// `wait_timeout` without a prior `close` can only ever time out,
    /// because the underlying wait does not resolve while open.
    #[tokio::test]
    async fn wait_timeout_without_close_times_out() {
        let tracker = TaskTracker::new();
        tracker.spawn(async {});

        let err = tracker
            .wait_timeout(Duration::from_millis(50))
            .await
            .expect_err("an open tracker must not drain");
        assert!(matches!(err.current_context(), Error::Concurrency));
    }

    /// `close` is reflected by `is_closed` and is idempotent.
    #[tokio::test]
    async fn close_is_observable_and_idempotent() {
        let tracker = TaskTracker::new();
        assert!(!tracker.is_closed());
        assert!(tracker.close());
        assert!(tracker.is_closed());
        assert!(!tracker.close());
    }

    /// Clones share the same task set: a task spawned on one clone is
    /// awaited by `close_and_wait` on another.
    #[tokio::test]
    async fn clones_share_task_set() {
        let a = TaskTracker::new();
        let b = a.clone();
        let counter = Arc::new(AtomicUsize::new(0));

        let counter_clone = Arc::clone(&counter);
        a.spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        b.close_and_wait(Duration::from_secs(1))
            .await
            .expect("clone must drain the sibling's task");
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    /// `track_future` counts an in-place future as a tracked task while
    /// it is being polled.
    #[tokio::test]
    async fn track_future_counts_inline_work() {
        let tracker = TaskTracker::new();
        let tracked = tracker.track_future(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            7_u32
        });

        assert_eq!(tracker.len(), 1);
        assert_eq!(tracked.await, 7);
        assert!(tracker.is_empty());
    }
}
