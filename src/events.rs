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
    ("ai-llms", "llms", "AI text export"),
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
    }
}
