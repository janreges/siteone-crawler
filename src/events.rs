// SiteOne Crawler - NDJSON event stream for host applications
// (c) Jan Reges <jan.reges@siteone.cz>
//
//! A machine-readable account of a run, for tools that drive the crawler.
//!
//! The GUI used to reconstruct progress by parsing the human-readable output with regular
//! expressions, which breaks whenever the wording or the table layout changes. With
//! `--events-file` the same information arrives as one JSON object per line, on a channel of
//! its own so it cannot collide with stdout or stderr.
//!
//! The stream is opt-in and best-effort: with no `--events-file` nothing here runs, and a
//! write failure is reported once and then ignored rather than disturbing the crawl.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

/// Bumped when the shape of an event changes incompatibly. Consumers should accept anything
/// with a protocol they know and ignore fields and event types they do not.
pub const PROTOCOL: u32 = 1;

static WRITER: OnceLock<Mutex<BufWriter<File>>> = OnceLock::new();
static FAILED: AtomicBool = AtomicBool::new(false);

/// One step of a run. Phases bracket the work the user waits for.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PhaseState {
    Started,
    Finished,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    #[serde(rename_all = "camelCase")]
    RunStarted {
        protocol: u32,
        version: &'static str,
        executed_at: String,
        url: String,
        working_dir: String,
    },
    /// One crawled URL, as it is added to the progress table.
    #[serde(rename_all = "camelCase")]
    Url {
        url: String,
        /// Negative values are connection failures and timeouts, as elsewhere in the crawler.
        status: i32,
        content_type: &'static str,
        time_ms: u64,
        size: i64,
        cached: bool,
        done: usize,
        total: usize,
    },
    #[serde(rename_all = "camelCase")]
    Phase {
        name: &'static str,
        state: PhaseState,
        #[serde(skip_serializing_if = "Option::is_none")]
        ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// A file or URL the run produced.
    #[serde(rename_all = "camelCase")]
    Artifact {
        kind: &'static str,
        label: &'static str,
        path: String,
    },
    /// A step that failed without failing the run.
    #[serde(rename_all = "camelCase")]
    Issue {
        kind: &'static str,
        label: &'static str,
        detail: String,
    },
    #[serde(rename_all = "camelCase")]
    RunFinished {
        outcome: &'static str,
        exit_code: i32,
        ms: u64,
        /// Whether the crawl was cut short by Ctrl+C or a `stop` on stdin.
        interrupted: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// One LLM HTTP attempt or AI cache hit, as its response arrives.
    AiRequest(Box<AiRequest>),
    /// An AI task started, finished one more unit, or ended.
    #[serde(rename_all = "camelCase")]
    AiProgress {
        task: String,
        label: String,
        done: u64,
        total: u64,
        /// `started`, `progress` or `finished`.
        state: &'static str,
    },
    /// The AI totals of the run, once all AI work is done.
    #[serde(rename_all = "camelCase")]
    AiUsage {
        provider: String,
        model: String,
        calls: u64,
        cache_hits: u64,
        http_attempts: u64,
        retries: u64,
        input_tokens: u64,
        output_tokens: u64,
        reasoning_tokens: u64,
        cached_input_tokens: u64,
        calls_without_usage: u64,
        network_ms: u64,
    },
}

/// The fields of an `aiRequest` event (boxed: far larger than the other events).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRequest {
    pub seq: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Units of the task done when the response arrived (this request's own unit not yet).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    pub provider: &'static str,
    pub model: String,
    pub attempt: u32,
    pub max_attempts: u32,
    /// `ok`, `retry`, `error` or `cacheHit`.
    pub outcome: &'static str,
    /// The HTTP status of the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_chars: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "one_decimal")]
    pub output_tokens_per_second: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "one_decimal")]
    pub total_tokens_per_second: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

/// Rates are written with one decimal, e.g. `119.4`.
fn one_decimal<S: serde::Serializer>(value: &Option<f64>, serializer: S) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => serializer.serialize_f64((value * 10.0).round() / 10.0),
        None => serializer.serialize_none(),
    }
}

/// Opens the event file. Called once, before the crawl starts.
///
/// Failing here is fatal on purpose: a host that asked for events is relying on them, and
/// silently running without would leave it watching an empty file forever.
pub fn init(path: &Path) -> std::io::Result<()> {
    let file = File::create(path)?;
    let _ = WRITER.set(Mutex::new(BufWriter::new(file)));
    Ok(())
}

pub fn is_enabled() -> bool {
    WRITER.get().is_some()
}

/// Writes one event. Does nothing when the stream was never opened.
///
/// Each line is flushed immediately: a host tailing the file needs to see progress while the
/// crawl runs, not when it ends.
pub fn emit(event: Event) {
    let Some(writer) = WRITER.get() else { return };
    if FAILED.load(Ordering::Relaxed) {
        return;
    }
    let Ok(mut writer) = writer.lock() else { return };

    let result = serde_json::to_string(&event)
        .map_err(std::io::Error::other)
        .and_then(|line| writeln!(writer, "{}", line))
        .and_then(|()| writer.flush());

    if let Err(error) = result {
        // Say so once, then stay quiet: a full disk should not fill the log as well.
        FAILED.store(true, Ordering::Relaxed);
        eprintln!("Event stream stopped: {}", error);
    }
}

/// Convenience for the common `started` / `finished` bracket around a phase.
pub fn phase(name: &'static str, state: PhaseState) {
    emit(Event::Phase {
        name,
        state,
        ms: None,
        detail: None,
    });
}

pub fn phase_finished(name: &'static str, ms: u64) {
    emit(Event::Phase {
        name,
        state: PhaseState::Finished,
        ms: Some(ms),
        detail: None,
    });
}

/// Summary codes that name a file or URL the run produced.
///
/// Emitting from one place keeps the exporters untouched, and the code already says exactly
/// what was written — far better than matching on the human-readable wording afterwards.
const ARTIFACTS: &[(&str, &str, &str)] = &[
    ("export-to-html", "html", "HTML report"),
    ("export-to-json", "json", "JSON data"),
    ("export-to-text", "text", "Text output"),
    ("upload-done", "online", "Online HTML report"),
    ("offline-website-generated", "offline", "Website clone"),
    ("markdown-generated", "markdown", "Markdown export"),
    ("markdown-combined", "markdown-single", "Single-file Markdown"),
    ("sitemap-xml", "sitemap-xml", "XML sitemap"),
    ("sitemap-txt", "sitemap-txt", "Text sitemap"),
    ("screenshots", "screenshots", "Screenshots"),
    ("screenshots-animation", "animation", "Animation"),
];

/// Summary codes for steps that failed without failing the run.
const ISSUES: &[(&str, &str)] = &[
    ("upload-failed", "Online report upload failed"),
    ("mail-report-failed", "Email delivery failed"),
    (
        "offline-exporter-store-file-error",
        "Website clone could not store a file",
    ),
    (
        "markdown-exporter-store-file-error",
        "Markdown export could not store a file",
    ),
    ("markdown-combine-error", "Single-file Markdown failed"),
];

/// Pulls the path out of a summary message.
///
/// Every exporter quotes it, which is the one part of the wording that is stable.
fn quoted_path(text: &str) -> Option<String> {
    let start = text.find('\'')?;
    let rest = &text[start + 1..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

/// Reports an artifact if this summary code names one. Called for every info item.
pub fn emit_artifact(code: &str, text: &str) {
    if !is_enabled() {
        return;
    }
    let Some((_, kind, label)) = ARTIFACTS.iter().find(|(c, _, _)| *c == code) else {
        return;
    };
    // Without a path there is nothing a host could open.
    let Some(path) = quoted_path(text) else { return };
    emit(Event::Artifact { kind, label, path });
}

/// Reports a step that failed without failing the run.
pub fn emit_issue(code: &str, text: &str) {
    if !is_enabled() {
        return;
    }
    let Some((_, label)) = ISSUES.iter().find(|(c, _)| *c == code) else {
        return;
    };
    emit(Event::Issue {
        kind: code_to_kind(code),
        label,
        detail: text.to_string(),
    });
}

/// Reports a file an AI exporter wrote. The AI exporters announce their files themselves, one
/// event per file, because their summary lines name several files at once.
pub fn emit_ai_artifact(kind: &'static str, label: &'static str, path: &Path) {
    if !is_enabled() {
        return;
    }
    emit(Event::Artifact {
        kind,
        label,
        path: crate::utils::get_absolute_path(&path.to_string_lossy()),
    });
}

/// Reports an AI step that failed without failing the run (kind `ai`).
pub fn emit_ai_issue(label: &'static str, detail: &str) {
    if !is_enabled() {
        return;
    }
    emit(ai_issue(label, detail));
}

fn ai_issue(label: &'static str, detail: &str) -> Event {
    Event::Issue {
        kind: "ai",
        label,
        detail: detail.to_string(),
    }
}

fn code_to_kind(code: &str) -> &'static str {
    match code {
        c if c.starts_with("upload") => "upload",
        c if c.starts_with("mail") => "mail",
        c if c.starts_with("offline") => "offline",
        c if c.starts_with("markdown") => "markdown",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_serialise_as_one_tagged_object_per_line() {
        let event = Event::Url {
            url: "/about".to_string(),
            status: 200,
            content_type: "html",
            time_ms: 13,
            size: 559,
            cached: false,
            done: 1,
            total: 9,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.starts_with(r#"{"type":"url","#), "got {json}");
        assert!(json.contains(r#""contentType":"html""#), "fields are camelCase: {json}");
        assert!(!json.contains('\n'), "an event must stay on one line");
    }

    #[test]
    fn optional_fields_are_left_out_rather_than_nulled() {
        let json = serde_json::to_string(&Event::Phase {
            name: "crawl",
            state: PhaseState::Started,
            ms: None,
            detail: None,
        })
        .unwrap();
        assert_eq!(json, r#"{"type":"phase","name":"crawl","state":"started"}"#);
    }

    #[test]
    fn a_finished_run_reports_how_it_ended() {
        let json = serde_json::to_string(&Event::RunFinished {
            outcome: "cancelled",
            exit_code: 0,
            ms: 4210,
            interrupted: true,
            error: None,
        })
        .unwrap();
        assert!(json.contains(r#""outcome":"cancelled""#));
        assert!(json.contains(r#""interrupted":true"#));
        assert!(!json.contains("error"));
    }

    #[test]
    fn the_quoted_path_is_what_gets_reported() {
        assert_eq!(
            quoted_path("HTML report saved to '/tmp/report.html' and took 0 ms."),
            Some("/tmp/report.html".to_string())
        );
        assert_eq!(
            quoted_path("XML sitemap generated to '/tmp/s.xml'."),
            Some("/tmp/s.xml".to_string())
        );
        assert_eq!(quoted_path("nothing quoted here"), None);
    }

    #[test]
    fn only_codes_that_name_a_file_are_artifacts() {
        assert!(ARTIFACTS.iter().any(|(c, _, _)| *c == "export-to-html"));
        assert!(!ARTIFACTS.iter().any(|(c, _, _)| *c == "ai-seo-low"));
    }

    #[test]
    fn emitting_without_a_stream_is_a_no_op() {
        // No init() in this test binary path; emit must not panic or block.
        assert!(!is_enabled());
        phase("crawl", PhaseState::Started);
        emit_ai_artifact("ai-report-json", "AI report (JSON)", Path::new("/tmp/r.json"));
        emit_ai_issue("AI phase skipped", "no API key");
    }

    fn ai_request() -> AiRequest {
        AiRequest {
            seq: 12,
            task: Some("seo".to_string()),
            label: Some("SEO".to_string()),
            done: Some(11),
            total: Some(40),
            category: "SEO analysis".to_string(),
            subject: Some("/blog/post".to_string()),
            provider: "openai-compatible",
            model: "qwen".to_string(),
            attempt: 1,
            max_attempts: 3,
            outcome: "ok",
            status: Some(200),
            error: None,
            input_tokens: Some(3412),
            output_tokens: Some(812),
            reasoning_tokens: Some(540),
            cached_input_tokens: Some(0),
            reasoning_chars: Some(1480),
            ms: Some(6800),
            output_tokens_per_second: Some(119.411_764),
            total_tokens_per_second: Some(621.0),
            finish_reason: Some("stop".to_string()),
        }
    }

    #[test]
    fn an_ai_request_is_one_camel_case_line_with_speeds_to_one_decimal() {
        let json = serde_json::to_string(&Event::AiRequest(Box::new(ai_request()))).unwrap();
        assert_eq!(
            json,
            concat!(
                r#"{"type":"aiRequest","seq":12,"task":"seo","label":"SEO","done":11,"total":40,"#,
                r#""category":"SEO analysis","subject":"/blog/post","provider":"openai-compatible","#,
                r#""model":"qwen","attempt":1,"maxAttempts":3,"outcome":"ok","status":200,"#,
                r#""inputTokens":3412,"outputTokens":812,"reasoningTokens":540,"cachedInputTokens":0,"#,
                r#""reasoningChars":1480,"ms":6800,"outputTokensPerSecond":119.4,"#,
                r#""totalTokensPerSecond":621.0,"finishReason":"stop"}"#
            )
        );
    }

    #[test]
    fn an_ai_request_leaves_out_what_is_unknown() {
        let retry = AiRequest {
            seq: 3,
            task: None,
            label: None,
            done: None,
            total: None,
            subject: None,
            outcome: "retry",
            status: Some(429),
            error: Some("HTTP 429".to_string()),
            input_tokens: None,
            output_tokens: None,
            reasoning_tokens: None,
            cached_input_tokens: None,
            reasoning_chars: None,
            ms: Some(400),
            output_tokens_per_second: None,
            total_tokens_per_second: None,
            finish_reason: None,
            ..ai_request()
        };
        let json = serde_json::to_string(&Event::AiRequest(Box::new(retry))).unwrap();
        assert_eq!(
            json,
            concat!(
                r#"{"type":"aiRequest","seq":3,"category":"SEO analysis","provider":"openai-compatible","#,
                r#""model":"qwen","attempt":1,"maxAttempts":3,"outcome":"retry","status":429,"#,
                r#""error":"HTTP 429","ms":400}"#
            )
        );
    }

    #[test]
    fn ai_progress_and_usage_are_camel_case() {
        let json = serde_json::to_string(&Event::AiProgress {
            task: "profile:chapters".to_string(),
            label: "Profile: chapters".to_string(),
            done: 3,
            total: 12,
            state: "progress",
        })
        .unwrap();
        assert_eq!(
            json,
            r#"{"type":"aiProgress","task":"profile:chapters","label":"Profile: chapters","done":3,"total":12,"state":"progress"}"#
        );

        let json = serde_json::to_string(&Event::AiUsage {
            provider: "openai-compatible".to_string(),
            model: "qwen".to_string(),
            calls: 6,
            cache_hits: 1,
            http_attempts: 7,
            retries: 2,
            input_tokens: 100,
            output_tokens: 50,
            reasoning_tokens: 30,
            cached_input_tokens: 10,
            calls_without_usage: 0,
            network_ms: 1234,
        })
        .unwrap();
        assert_eq!(
            json,
            concat!(
                r#"{"type":"aiUsage","provider":"openai-compatible","model":"qwen","calls":6,"cacheHits":1,"#,
                r#""httpAttempts":7,"retries":2,"inputTokens":100,"outputTokens":50,"reasoningTokens":30,"#,
                r#""cachedInputTokens":10,"callsWithoutUsage":0,"networkMs":1234}"#
            )
        );
    }

    #[test]
    fn ai_issues_have_their_own_kind() {
        let json = serde_json::to_string(&ai_issue("AI phase skipped", "no API key")).unwrap();
        assert_eq!(
            json,
            r#"{"type":"issue","kind":"ai","label":"AI phase skipped","detail":"no API key"}"#
        );
    }
}
