// SiteOne Crawler - AI task progress
// (c) Jan Reges <jan.reges@siteone.cz>
//
// How far each AI task (a per-page action, a report, an elaborate or profile stage) has got, one
// unit of work (page, area, section, chapter, call) at a time. Failed units count as done too, so a
// task that runs to its end reaches `done == total`; one that ends early is finished below it.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Mutex;

use super::telemetry::{self, Subject};

struct TaskState {
    label: String,
    done: u64,
    total: u64,
    finished: bool,
}

/// Every task of the run by its stable key (`seo`, `report:ia`, `profile:chapters`, …).
static TASKS: Mutex<BTreeMap<String, TaskState>> = Mutex::new(BTreeMap::new());

/// Start `task` (again) with `total` units; `label` is its human-readable name.
pub fn start(task: &str, label: &str, total: u64) {
    if let Ok(mut tasks) = TASKS.lock() {
        tasks.insert(
            task.to_string(),
            TaskState {
                label: label.to_string(),
                done: 0,
                total,
                finished: false,
            },
        );
    }
}

/// One unit of `task` finished, successfully or not. Never counts past the total.
pub fn advance(task: &str) {
    if let Ok(mut tasks) = TASKS.lock()
        && let Some(state) = tasks.get_mut(task)
        && !state.finished
    {
        state.done = (state.done + 1).min(state.total);
    }
}

/// `task` ended, with every unit counted or early (then `done < total`).
pub fn finish(task: &str) {
    if let Ok(mut tasks) = TASKS.lock()
        && let Some(state) = tasks.get_mut(task)
    {
        state.finished = true;
    }
}

/// `(done, total)` of `task`, None when it was never started.
pub fn current(task: &str) -> Option<(u64, u64)> {
    let tasks = TASKS.lock().ok()?;
    tasks.get(task).map(|state| (state.done, state.total))
}

/// The label `task` was started with.
pub fn label(task: &str) -> Option<String> {
    let tasks = TASKS.lock().ok()?;
    tasks.get(task).map(|state| state.label.clone())
}

/// Advances its task when dropped, so a unit counts on every way out of it: success, failure,
/// `continue`, a panic or a cancellation.
pub struct AdvanceOnDrop(String);

impl Drop for AdvanceOnDrop {
    fn drop(&mut self) {
        advance(&self.0);
    }
}

/// Count the current unit of `task` when the returned guard goes out of scope.
pub fn advance_on_drop(task: &str) -> AdvanceOnDrop {
    AdvanceOnDrop(task.to_string())
}

/// Run one unit of `task`: the AI requests of `fut` carry the task and `subject` (a page path, an
/// area, a section or chapter), and the unit counts once `fut` ends, however it ends.
pub async fn unit<F: Future>(task: impl Into<String>, subject: impl Into<String>, fut: F) -> F::Output {
    let task = task.into();
    let _counted = advance_on_drop(&task);
    let subject = Subject {
        task: Some(task),
        subject: Some(subject.into()),
    };
    telemetry::scope(subject, fut).await
}

/// Run `fut` as a task of its own with a single unit (a stage that makes one call).
pub async fn single_unit<F: Future>(task: &str, label: &str, subject: impl Into<String>, fut: F) -> F::Output {
    start(task, label, 1);
    let output = unit(task, subject, fut).await;
    finish(task);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::telemetry::{Subject, current_subject};

    #[test]
    fn a_task_counts_its_units_up_to_the_total() {
        start("test:count", "Counting", 2);
        assert_eq!(current("test:count"), Some((0, 2)));
        assert_eq!(label("test:count").as_deref(), Some("Counting"));
        advance("test:count");
        assert_eq!(current("test:count"), Some((1, 2)));
        advance("test:count");
        advance("test:count");
        assert_eq!(current("test:count"), Some((2, 2)), "clamped to the total");
    }

    #[test]
    fn unknown_tasks_are_ignored() {
        advance("test:never-started");
        finish("test:never-started");
        assert_eq!(current("test:never-started"), None);
        assert_eq!(label("test:never-started"), None);
    }

    #[test]
    fn a_finished_task_keeps_its_count() {
        start("test:finish", "Finishing", 3);
        advance("test:finish");
        finish("test:finish");
        advance("test:finish");
        assert_eq!(current("test:finish"), Some((1, 3)), "ended early, below the total");
    }

    #[test]
    fn starting_a_task_again_resets_it() {
        start("test:restart", "First", 2);
        advance("test:restart");
        finish("test:restart");
        start("test:restart", "Second", 5);
        assert_eq!(current("test:restart"), Some((0, 5)));
        assert_eq!(label("test:restart").as_deref(), Some("Second"));
        advance("test:restart");
        assert_eq!(current("test:restart"), Some((1, 5)));
    }

    #[tokio::test]
    async fn a_unit_attaches_its_task_and_subject_and_counts_even_when_it_fails() {
        start("test:unit", "Unit", 2);
        let (seen, result) = unit("test:unit", "/about", async {
            (current_subject(), Err::<(), &str>("the request failed"))
        })
        .await;
        assert_eq!(
            seen,
            Some(Subject {
                task: Some("test:unit".to_string()),
                subject: Some("/about".to_string()),
            })
        );
        assert!(result.is_err());
        assert_eq!(current("test:unit"), Some((1, 2)));
    }

    #[tokio::test]
    async fn a_unit_counts_when_its_task_panics() {
        start("test:panic", "Panic", 1);
        let handle = tokio::spawn(unit("test:panic", "/boom", async {
            if current("test:panic").is_some() {
                panic!("a unit that panics");
            }
        }));
        assert!(handle.await.is_err());
        assert_eq!(current("test:panic"), Some((1, 1)));
    }

    #[tokio::test]
    async fn a_single_unit_task_starts_counts_and_finishes() {
        let seen = single_unit("test:single", "Single", "example.com", async { current_subject() }).await;
        assert_eq!(seen.and_then(|s| s.subject).as_deref(), Some("example.com"));
        assert_eq!(current("test:single"), Some((1, 1)));
        assert_eq!(label("test:single").as_deref(), Some("Single"));
    }
}
