// SiteOne Crawler - AI request telemetry
// (c) Jan Reges <jan.reges@siteone.cz>
//
// One record per LLM HTTP attempt and per cache hit, reported as the response arrives: a compact
// line on stderr (unless hidden) plus the throughput totals. Everything here is fire-and-forget —
// a failure to print never affects the AI result.

use std::future::Future;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::provider::Usage;
use crate::utils::get_color_text;

/// The AI task and the page, chapter or stage the requests of a future belong to (see `scope`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Subject {
    pub task: Option<String>,
    pub subject: Option<String>,
}

tokio::task_local! {
    static SUBJECT: Subject;
}

/// Run `fut` with `subject` attached to every AI request it makes. Task-locals do not cross
/// `tokio::spawn`, so a spawned per-page future must be wrapped inside the spawn.
pub async fn scope<F: Future>(subject: Subject, fut: F) -> F::Output {
    SUBJECT.scope(subject, fut).await
}

/// The subject of the enclosing `scope`, None outside of one.
pub fn current_subject() -> Option<Subject> {
    SUBJECT.try_with(Subject::clone).ok()
}

/// The AI task a request belongs to: its stable key, human label and progress (done, total).
#[derive(Debug, Clone, PartialEq)]
pub struct TaskRef {
    pub key: String,
    pub label: String,
    pub progress: Option<(u64, u64)>,
}

/// The task of `key` as known when a request is reported.
pub fn task_ref(key: &str) -> TaskRef {
    TaskRef {
        key: key.to_string(),
        label: key.to_string(),
        progress: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestOutcome {
    Ok,
    Retry,
    Error,
    CacheHit,
}

#[derive(Debug, Clone)]
pub struct RequestRecord {
    /// 1-based, process-wide, in the order responses arrive (assigned by `report`).
    pub seq: u64,
    pub task: Option<TaskRef>,
    /// The accounting label of the request ("SEO analysis", "AI report (extract)", …).
    pub category: String,
    /// Page path, chapter or stage the request belongs to.
    pub subject: Option<String>,
    pub provider: &'static str,
    pub model: String,
    /// 1-based transport attempt.
    pub attempt: u32,
    pub max_attempts: u32,
    pub outcome: RequestOutcome,
    pub http_status: Option<u16>,
    /// Short and credential-free; `report` cuts it to 200 characters.
    pub error: Option<String>,
    pub usage: Option<Usage>,
    /// Characters of reasoning text the response carried (reasoning present but maybe not counted).
    pub reasoning_chars: Option<u64>,
    /// From just before the send to the body being read, this attempt only (None for cache hits).
    pub duration_ms: Option<u64>,
    pub finish_reason: Option<String>,
}

impl RequestRecord {
    /// Output tokens (incl. reasoning) per second of this attempt's wall time.
    pub fn output_tokens_per_second(&self) -> Option<f64> {
        per_second(self.usage?.output_tokens?, self.duration_ms?)
    }

    /// (input + output) tokens per second — the end-to-end throughput.
    pub fn total_tokens_per_second(&self) -> Option<f64> {
        let usage = self.usage?;
        per_second(
            usage.input_tokens?.saturating_add(usage.output_tokens?),
            self.duration_ms?,
        )
    }

    /// The uncoloured console line of this record.
    pub fn console_line(&self) -> String {
        let (prefix, details) = self.console_parts();
        format!("  {} {}", prefix, details)
    }

    /// The outcome prefix (`AI ✓`) and the details after it.
    fn console_parts(&self) -> (&'static str, String) {
        let prefix = match self.outcome {
            RequestOutcome::Ok => "AI ✓",
            RequestOutcome::Retry => "AI ↻",
            RequestOutcome::Error => "AI ✗",
            RequestOutcome::CacheHit => "AI ⇢",
        };
        let mut head = format!("#{} ", self.seq);
        match &self.task {
            Some(task) => {
                head.push_str(&task.label);
                if let Some((done, total)) = task.progress {
                    head.push_str(&format!(" {}/{}", done, total));
                }
            }
            None => head.push_str(&self.category),
        }
        let mut parts = vec![head];
        parts.extend(self.subject.clone());
        let error = || self.error.clone().unwrap_or_else(|| "request failed".to_string());
        match self.outcome {
            RequestOutcome::Ok => {
                parts.extend(self.token_parts());
                parts.extend(self.duration_ms.map(seconds));
                parts.extend(
                    self.output_tokens_per_second()
                        .map(|speed| format!("{} tok/s", speed.round() as u64)),
                );
            }
            RequestOutcome::Retry => {
                parts.push(error());
                parts.extend(self.duration_ms.map(seconds));
                parts.push(format!("retrying (attempt {}/{})", self.attempt + 1, self.max_attempts));
            }
            RequestOutcome::Error => {
                parts.push(error());
                parts.extend(self.duration_ms.map(seconds));
            }
            RequestOutcome::CacheHit => {
                parts.push("cache hit".to_string());
                parts.extend(self.token_parts());
            }
        }
        (prefix, parts.join(" · "))
    }

    /// `3,412 in` and `812 out (540 reasoning)`, or `tokens not reported`.
    fn token_parts(&self) -> Vec<String> {
        let Some(usage) = self.usage.filter(Usage::has_tokens) else {
            return vec!["tokens not reported".to_string()];
        };
        let mut parts = Vec::new();
        if let Some(input) = usage.input_tokens {
            parts.push(format!("{} in", thousands(input)));
        }
        if let Some(output) = usage.output_tokens {
            let reasoning = match (usage.reasoning_tokens, self.reasoning_chars) {
                (Some(reasoning), _) => format!(" ({} reasoning)", thousands(reasoning)),
                (None, Some(_)) => " (reasoning n/a)".to_string(),
                (None, None) => String::new(),
            };
            parts.push(format!("{} out{}", thousands(output), reasoning));
        }
        parts
    }
}

fn per_second(tokens: u64, duration_ms: u64) -> Option<f64> {
    (duration_ms > 0).then(|| tokens as f64 * 1000.0 / duration_ms as f64)
}

/// `6.8 s`, rounded half up to a tenth.
fn seconds(duration_ms: u64) -> String {
    let tenths = duration_ms.saturating_add(50) / 100;
    format!("{}.{} s", tenths / 10, tenths % 10)
}

/// `1,234,567`.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

static NEXT_SEQ: AtomicU64 = AtomicU64::new(1);
static CONSOLE_ENABLED: AtomicBool = AtomicBool::new(true);

/// Show or hide the per-request console lines (hidden with `--hide-progress-bar`).
pub fn set_console_enabled(enabled: bool) {
    CONSOLE_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Report one request: assign its sequence number, print its line and add it to the totals.
pub fn report(mut record: RequestRecord) {
    record.seq = NEXT_SEQ.fetch_add(1, Ordering::Relaxed);
    record.error = record.error.as_deref().map(error_text);
    if record.outcome == RequestOutcome::Ok
        && let (Some(output), Some(duration_ms)) = (record.usage.and_then(|u| u.output_tokens), record.duration_ms)
        && duration_ms > 0
    {
        super::usage::record_generation(&record.category, output, duration_ms);
    }
    if CONSOLE_ENABLED.load(Ordering::Relaxed) {
        let (prefix, details) = record.console_parts();
        let color = match record.outcome {
            RequestOutcome::Ok => "green",
            RequestOutcome::Retry | RequestOutcome::CacheHit => "yellow",
            RequestOutcome::Error => "red",
        };
        // Never `eprintln!`: it panics when stderr is gone, and printing must not affect the AI result.
        let _ = writeln!(
            std::io::stderr(),
            "  {} {}",
            get_color_text(prefix, color, false),
            get_color_text(&details, "gray", false)
        );
    }
}

const MAX_ERROR_CHARS: usize = 200;

/// `message` cut to `MAX_ERROR_CHARS` characters (an ellipsis marks the cut).
fn error_text(message: &str) -> String {
    if message.chars().count() <= MAX_ERROR_CHARS {
        return message.to_string();
    }
    let mut cut: String = message.chars().take(MAX_ERROR_CHARS - 1).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(outcome: RequestOutcome) -> RequestRecord {
        RequestRecord {
            seq: 12,
            task: Some(TaskRef {
                key: "seo".to_string(),
                label: "SEO".to_string(),
                progress: Some((12, 40)),
            }),
            category: "SEO analysis".to_string(),
            subject: Some("/blog/post".to_string()),
            provider: "openai-compatible",
            model: "m".to_string(),
            attempt: 1,
            max_attempts: 3,
            outcome,
            http_status: Some(200),
            error: None,
            usage: None,
            reasoning_chars: None,
            duration_ms: Some(6800),
            finish_reason: Some("stop".to_string()),
        }
    }

    fn usage(input: Option<u64>, output: Option<u64>, reasoning: Option<u64>) -> Option<Usage> {
        Some(Usage {
            input_tokens: input,
            output_tokens: output,
            reasoning_tokens: reasoning,
            cached_input_tokens: None,
        })
    }

    #[test]
    fn ok_line_with_reasoning_tokens() {
        let r = RequestRecord {
            usage: usage(Some(3412), Some(812), Some(540)),
            ..record(RequestOutcome::Ok)
        };
        assert_eq!(
            r.console_line(),
            "  AI ✓ #12 SEO 12/40 · /blog/post · 3,412 in · 812 out (540 reasoning) · 6.8 s · 119 tok/s"
        );
    }

    #[test]
    fn ok_line_with_uncounted_reasoning() {
        let r = RequestRecord {
            seq: 13,
            task: Some(TaskRef {
                key: "profile:chapters".to_string(),
                label: "Profile: chapters".to_string(),
                progress: Some((3, 12)),
            }),
            subject: Some("Services".to_string()),
            usage: usage(Some(9870), Some(1944), None),
            reasoning_chars: Some(500),
            duration_ms: Some(21_400),
            ..record(RequestOutcome::Ok)
        };
        assert_eq!(
            r.console_line(),
            "  AI ✓ #13 Profile: chapters 3/12 · Services · 9,870 in · 1,944 out (reasoning n/a) · 21.4 s · 91 tok/s"
        );
    }

    #[test]
    fn ok_line_without_usage() {
        let r = RequestRecord {
            seq: 14,
            task: Some(TaskRef {
                key: "typos".to_string(),
                label: "Typos".to_string(),
                progress: Some((3, 40)),
            }),
            subject: Some("/about".to_string()),
            duration_ms: Some(2100),
            ..record(RequestOutcome::Ok)
        };
        assert_eq!(
            r.console_line(),
            "  AI ✓ #14 Typos 3/40 · /about · tokens not reported · 2.1 s"
        );
    }

    #[test]
    fn retry_line() {
        let r = RequestRecord {
            seq: 15,
            subject: Some("/blog/x".to_string()),
            http_status: Some(429),
            error: Some("HTTP 429".to_string()),
            duration_ms: Some(400),
            finish_reason: None,
            ..record(RequestOutcome::Retry)
        };
        let r = RequestRecord {
            task: Some(TaskRef {
                progress: Some((13, 40)),
                ..r.task.clone().expect("a task")
            }),
            ..r
        };
        assert_eq!(
            r.console_line(),
            "  AI ↻ #15 SEO 13/40 · /blog/x · HTTP 429 · 0.4 s · retrying (attempt 2/3)"
        );
    }

    #[test]
    fn error_line() {
        let r = RequestRecord {
            seq: 16,
            subject: Some("/blog/x".to_string()),
            error: Some("AI provider error: model not found".to_string()),
            duration_ms: Some(200),
            ..record(RequestOutcome::Error)
        };
        let r = RequestRecord {
            task: Some(TaskRef {
                progress: Some((13, 40)),
                ..r.task.clone().expect("a task")
            }),
            ..r
        };
        assert_eq!(
            r.console_line(),
            "  AI ✗ #16 SEO 13/40 · /blog/x · AI provider error: model not found · 0.2 s"
        );
    }

    #[test]
    fn cache_hit_line() {
        let r = RequestRecord {
            seq: 17,
            subject: Some("/contact".to_string()),
            usage: usage(Some(1020), Some(88), None),
            duration_ms: None,
            http_status: None,
            ..record(RequestOutcome::CacheHit)
        };
        let r = RequestRecord {
            task: Some(TaskRef {
                progress: Some((14, 40)),
                ..r.task.clone().expect("a task")
            }),
            ..r
        };
        assert_eq!(
            r.console_line(),
            "  AI ⇢ #17 SEO 14/40 · /contact · cache hit · 1,020 in · 88 out"
        );
    }

    #[test]
    fn line_without_task_or_subject_names_the_category() {
        let r = RequestRecord {
            seq: 1,
            task: None,
            subject: None,
            usage: usage(Some(17), Some(37), Some(33)),
            duration_ms: Some(100),
            ..record(RequestOutcome::Ok)
        };
        assert_eq!(
            r.console_line(),
            "  AI ✓ #1 SEO analysis · 17 in · 37 out (33 reasoning) · 0.1 s · 370 tok/s"
        );
        let r = RequestRecord {
            task: Some(TaskRef {
                progress: None,
                ..record(RequestOutcome::Ok).task.expect("a task")
            }),
            ..r
        };
        assert_eq!(
            r.console_line(),
            "  AI ✓ #1 SEO · 17 in · 37 out (33 reasoning) · 0.1 s · 370 tok/s"
        );
    }

    #[test]
    fn line_numbers_and_rounding() {
        let r = RequestRecord {
            task: None,
            subject: None,
            usage: usage(Some(1_234_567), Some(1000), Some(0)),
            // 6.85 s rounds half up to 6.9 s; 1000 / 6.85 = 145.99 tok/s.
            duration_ms: Some(6850),
            ..record(RequestOutcome::Ok)
        };
        assert_eq!(
            r.console_line(),
            "  AI ✓ #12 SEO analysis · 1,234,567 in · 1,000 out (0 reasoning) · 6.9 s · 146 tok/s"
        );
        let r = RequestRecord {
            usage: usage(None, Some(5), None),
            duration_ms: Some(0),
            ..r
        };
        assert_eq!(r.console_line(), "  AI ✓ #12 SEO analysis · 5 out · 0.0 s");
    }

    #[test]
    fn tokens_per_second() {
        let r = RequestRecord {
            usage: usage(Some(3412), Some(812), None),
            ..record(RequestOutcome::Ok)
        };
        let rounded = |v: Option<f64>| v.map(|v| (v * 10.0).round() / 10.0);
        assert_eq!(rounded(r.output_tokens_per_second()), Some(119.4));
        assert_eq!(rounded(r.total_tokens_per_second()), Some(621.2));

        let instant = RequestRecord {
            duration_ms: Some(0),
            ..r.clone()
        };
        assert_eq!(instant.output_tokens_per_second(), None);
        assert_eq!(instant.total_tokens_per_second(), None);
        let cached = RequestRecord {
            duration_ms: None,
            ..r.clone()
        };
        assert_eq!(cached.output_tokens_per_second(), None);
        let no_input = RequestRecord {
            usage: usage(None, Some(812), None),
            ..r.clone()
        };
        assert_eq!(rounded(no_input.output_tokens_per_second()), Some(119.4));
        assert_eq!(no_input.total_tokens_per_second(), None);
        let no_usage = RequestRecord {
            usage: None,
            ..r.clone()
        };
        assert_eq!(no_usage.output_tokens_per_second(), None);
        // Counts read back from a cache file are not bounded like parsed ones.
        let huge = RequestRecord {
            usage: usage(Some(u64::MAX), Some(1), None),
            duration_ms: Some(1000),
            ..r
        };
        assert_eq!(huge.total_tokens_per_second(), Some(u64::MAX as f64));
    }

    #[test]
    fn error_text_is_cut_to_200_characters() {
        assert_eq!(error_text("HTTP 429"), "HTTP 429");
        let long = "é".repeat(300);
        let cut = error_text(&long);
        assert_eq!(cut.chars().count(), 200);
        assert!(cut.ends_with('…'));
        assert_eq!(error_text(&"x".repeat(200)), "x".repeat(200));
    }

    #[tokio::test]
    async fn scope_attaches_the_subject() {
        assert_eq!(current_subject(), None);
        let subject = Subject {
            task: Some("seo".to_string()),
            subject: Some("/about".to_string()),
        };
        let seen = scope(subject.clone(), async { current_subject() }).await;
        assert_eq!(seen, Some(subject));
        assert_eq!(current_subject(), None);
    }
}
