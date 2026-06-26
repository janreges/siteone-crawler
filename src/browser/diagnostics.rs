// SiteOne Crawler - Browser diagnostics
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Per-page diagnostics collected while rendering a page in a real browser:
// console messages, uncaught JS exceptions, failed network requests, and security
// violations (CSP/CORS/mixed-content), plus an optional screenshot path.
//
// These plain data types are always compiled so `HttpResponse` can carry an inert
// `Option<BrowserDiagnostics>` (always `None` on the direct-HTTP path). The CDP event
// collection that fills them lives behind the `browser` Cargo feature (Phase 3).

use serde::Serialize;

/// Severity classification shared by all diagnostic kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// A `console.*` message captured from the page.
#[derive(Debug, Clone, Serialize)]
pub struct ConsoleMessage {
    pub severity: Severity,
    /// The console method (`log`, `warn`, `error`, ...).
    pub kind: String,
    pub text: String,
    pub url: Option<String>,
    pub line: Option<u32>,
}

/// An uncaught JavaScript exception thrown on the page.
#[derive(Debug, Clone, Serialize)]
pub struct JsException {
    pub text: String,
    pub url: Option<String>,
    pub line: Option<u32>,
    pub col: Option<u32>,
}

/// A failed or error network request (404/5xx, DNS/connection failures, blocked).
#[derive(Debug, Clone, Serialize)]
pub struct NetworkError {
    pub url: String,
    pub status: Option<i32>,
    pub error_text: Option<String>,
    /// Short kind, e.g. `http-error`, `loading-failed`, `blocked`.
    pub kind: String,
}

/// A security violation surfaced by the browser (CSP, CORS, mixed content).
#[derive(Debug, Clone, Serialize)]
pub struct SecurityIssue {
    /// `csp`, `cors`, `mixed-content`, ...
    pub kind: String,
    pub text: String,
    pub url: Option<String>,
}

/// All diagnostics collected for a single rendered page.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BrowserDiagnostics {
    pub console: Vec<ConsoleMessage>,
    pub exceptions: Vec<JsException>,
    pub network_errors: Vec<NetworkError>,
    pub violations: Vec<SecurityIssue>,
    pub screenshot_path: Option<String>,
    /// Set when rendering failed and the crawler fell back to the plain HTTP response.
    pub render_error: Option<String>,
    /// Set when the screenshot capture failed (carries the reason).
    pub screenshot_error: Option<String>,
    /// Total wall time of navigate + wait, in milliseconds.
    pub render_total_ms: u64,
    /// Severity counts, for fast table/summary rendering.
    pub errors: u32,
    pub warnings: u32,
    pub infos: u32,
}

impl BrowserDiagnostics {
    /// Recompute the severity counters from the collected items.
    pub fn recount(&mut self) {
        let mut errors = 0u32;
        let mut warnings = 0u32;
        let mut infos = 0u32;
        for c in &self.console {
            match c.severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
                Severity::Info => infos += 1,
            }
        }
        errors += self.exceptions.len() as u32;
        for n in &self.network_errors {
            match n.status {
                Some(s) if s >= 500 => errors += 1,
                Some(_) => warnings += 1,
                None => errors += 1,
            }
        }
        errors += self.violations.len() as u32;
        if self.render_error.is_some() {
            errors += 1;
        }
        if self.screenshot_error.is_some() {
            warnings += 1;
        }
        self.errors = errors;
        self.warnings = warnings;
        self.infos = infos;
    }

    /// Total number of error+warning findings (used by the analyzer/scoring).
    pub fn issue_count(&self) -> u32 {
        self.errors + self.warnings
    }

    /// Produce a compact, size-bounded text payload of the diagnostics, suitable as input
    /// to an AI analysis step. Items are ordered by importance (exceptions and errors first,
    /// then warnings, network failures, security violations, info last). Each message is
    /// truncated to `msg_max_chars` characters, at most `max_messages` lines are kept, and the
    /// whole payload is capped at `total_max_kb` kilobytes. Returns an empty string if there
    /// is nothing to report.
    pub fn to_ai_payload(&self, max_messages: usize, msg_max_chars: usize, total_max_kb: usize) -> String {
        let mut lines: Vec<String> = Vec::new();

        if let Some(err) = &self.render_error {
            lines.push(format!("[error][render-failed] {}", truncate_msg(err, msg_max_chars)));
        }
        if let Some(err) = &self.screenshot_error {
            lines.push(format!(
                "[warning][screenshot-failed] {}",
                truncate_msg(err, msg_max_chars)
            ));
        }
        for e in &self.exceptions {
            lines.push(format!(
                "[error][js-exception] {}",
                truncate_msg(&e.text, msg_max_chars)
            ));
        }
        for c in self.console.iter().filter(|c| matches!(c.severity, Severity::Error)) {
            lines.push(format!(
                "[error][console.{}] {}",
                c.kind,
                truncate_msg(&c.text, msg_max_chars)
            ));
        }
        for c in self.console.iter().filter(|c| matches!(c.severity, Severity::Warning)) {
            lines.push(format!(
                "[warning][console.{}] {}",
                c.kind,
                truncate_msg(&c.text, msg_max_chars)
            ));
        }
        for n in &self.network_errors {
            let label = n
                .status
                .map(|s| s.to_string())
                .or_else(|| n.error_text.clone())
                .unwrap_or_else(|| n.kind.clone());
            // Match recount(): 5xx (and connection-level failures with no status) are errors.
            let severity = match n.status {
                Some(s) if s >= 500 => "error",
                Some(_) => "warning",
                None => "error",
            };
            lines.push(format!(
                "[{}][network:{}] {}",
                severity,
                label,
                truncate_msg(&n.url, msg_max_chars)
            ));
        }
        for v in &self.violations {
            lines.push(format!("[error][{}] {}", v.kind, truncate_msg(&v.text, msg_max_chars)));
        }
        for c in self.console.iter().filter(|c| matches!(c.severity, Severity::Info)) {
            lines.push(format!(
                "[info][console.{}] {}",
                c.kind,
                truncate_msg(&c.text, msg_max_chars)
            ));
        }

        if lines.len() > max_messages {
            let dropped = lines.len() - max_messages;
            lines.truncate(max_messages);
            lines.push(format!("... ({} more message(s) omitted)", dropped));
        }

        let total_max = total_max_kb.saturating_mul(1024).max(1);
        let mut out = String::new();
        for line in lines {
            if out.len() + line.len() + 1 > total_max {
                out.push_str("... (truncated to size limit)\n");
                break;
            }
            out.push_str(&line);
            out.push('\n');
        }
        out
    }
}

/// Collapse newlines and truncate a message to at most `max` characters (char-safe).
fn truncate_msg(s: &str, max: usize) -> String {
    let cleaned = s.replace(['\n', '\r', '\t'], " ");
    if cleaned.chars().count() <= max {
        cleaned
    } else {
        let truncated: String = cleaned.chars().take(max).collect();
        format!("{}…", truncated)
    }
}

/// CDP event collection. Compiled only with the `browser` feature.
#[cfg(feature = "browser")]
pub mod collect {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use chromiumoxide::Page;
    use chromiumoxide::cdp::browser_protocol::log::{
        EnableParams as LogEnable, EventEntryAdded, LogEntryLevel, LogEntrySource,
    };
    use chromiumoxide::cdp::browser_protocol::network::{
        EnableParams as NetworkEnable, EventLoadingFailed, EventRequestWillBeSent, EventResponseReceived, RequestId,
    };
    use chromiumoxide::cdp::js_protocol::runtime::{
        ConsoleApiCalledType, EnableParams as RuntimeEnable, EventConsoleApiCalled, EventExceptionThrown, RemoteObject,
    };
    use futures::StreamExt;
    use tokio::task::JoinHandle;

    use super::{BrowserDiagnostics, ConsoleMessage, JsException, NetworkError, SecurityIssue, Severity};

    /// Per-kind cap on collected items, to bound memory against chatty/hostile pages.
    const MAX_ITEMS_PER_KIND: usize = 500;
    /// Max characters stored per captured message/URL at collection time.
    const MAX_TEXT_LEN: usize = 4096;
    /// Cap on the requestId→URL correlation map.
    const MAX_TRACKED_REQUESTS: usize = 10_000;

    /// Truncate a captured string to `MAX_TEXT_LEN` characters (char-safe).
    fn clip(s: &str) -> String {
        if s.chars().count() <= MAX_TEXT_LEN {
            s.to_string()
        } else {
            s.chars().take(MAX_TEXT_LEN).collect()
        }
    }

    /// Collects diagnostics from a page's CDP event streams until `finish()` is called.
    pub struct Collector {
        diag: Arc<Mutex<BrowserDiagnostics>>,
        tasks: Vec<JoinHandle<()>>,
    }

    impl Collector {
        /// Enable the Network and Log domains (Runtime is on by default) and subscribe to
        /// the diagnostic event streams. Call this on a blank page BEFORE navigating.
        pub async fn attach(page: &Page, main_url: &str) -> Collector {
            let diag = Arc::new(Mutex::new(BrowserDiagnostics::default()));
            let requests: Arc<Mutex<HashMap<RequestId, String>>> = Arc::new(Mutex::new(HashMap::new()));
            let main_url = main_url.to_string();
            // Explicitly enable the domains whose events we consume (console/exceptions need
            // Runtime; failed requests need Network; CSP/CORS/mixed-content need Log).
            let _ = page.execute(RuntimeEnable::default()).await;
            let _ = page.execute(NetworkEnable::default()).await;
            let _ = page.execute(LogEnable::default()).await;

            let mut tasks: Vec<JoinHandle<()>> = Vec::new();

            // requestId → URL map, so loadingFailed (which carries no URL) can be correlated.
            if let Ok(mut stream) = page.event_listener::<EventRequestWillBeSent>().await {
                let r = requests.clone();
                tasks.push(tokio::spawn(async move {
                    while let Some(ev) = stream.next().await {
                        if let Ok(mut m) = r.lock()
                            && m.len() < MAX_TRACKED_REQUESTS
                        {
                            m.insert(ev.request_id.clone(), ev.request.url.clone());
                        }
                    }
                }));
            }

            if let Ok(mut stream) = page.event_listener::<EventConsoleApiCalled>().await {
                let d = diag.clone();
                tasks.push(tokio::spawn(async move {
                    while let Some(ev) = stream.next().await {
                        let severity = console_severity(&ev.r#type);
                        let text = clip(&ev.args.iter().map(remote_object_to_text).collect::<Vec<_>>().join(" "));
                        if let Ok(mut g) = d.lock()
                            && g.console.len() < MAX_ITEMS_PER_KIND
                        {
                            g.console.push(ConsoleMessage {
                                severity,
                                kind: format!("{:?}", ev.r#type).to_lowercase(),
                                text,
                                url: None,
                                line: None,
                            });
                        }
                    }
                }));
            }

            if let Ok(mut stream) = page.event_listener::<EventExceptionThrown>().await {
                let d = diag.clone();
                tasks.push(tokio::spawn(async move {
                    while let Some(ev) = stream.next().await {
                        let det = &ev.exception_details;
                        if let Ok(mut g) = d.lock()
                            && g.exceptions.len() < MAX_ITEMS_PER_KIND
                        {
                            g.exceptions.push(JsException {
                                text: clip(&det.text),
                                url: det.url.clone(),
                                line: Some(det.line_number as u32),
                                col: Some(det.column_number as u32),
                            });
                        }
                    }
                }));
            }

            if let Ok(mut stream) = page.event_listener::<EventResponseReceived>().await {
                let d = diag.clone();
                let main = main_url.clone();
                tasks.push(tokio::spawn(async move {
                    while let Some(ev) = stream.next().await {
                        let status = ev.response.status;
                        // Skip the main document — its status is already reported by the crawler.
                        if status >= 400
                            && ev.response.url != main
                            && let Ok(mut g) = d.lock()
                            && g.network_errors.len() < MAX_ITEMS_PER_KIND
                        {
                            g.network_errors.push(NetworkError {
                                url: clip(&ev.response.url),
                                status: Some(status as i32),
                                error_text: None,
                                kind: "http-error".to_string(),
                            });
                        }
                    }
                }));
            }

            if let Ok(mut stream) = page.event_listener::<EventLoadingFailed>().await {
                let d = diag.clone();
                let r = requests.clone();
                tasks.push(tokio::spawn(async move {
                    while let Some(ev) = stream.next().await {
                        if ev.canceled.unwrap_or(false) {
                            continue;
                        }
                        let url = r
                            .lock()
                            .ok()
                            .and_then(|m| m.get(&ev.request_id).cloned())
                            .unwrap_or_default();
                        if let Ok(mut g) = d.lock()
                            && g.network_errors.len() < MAX_ITEMS_PER_KIND
                        {
                            g.network_errors.push(NetworkError {
                                url: clip(&url),
                                status: None,
                                error_text: Some(clip(&ev.error_text)),
                                kind: "loading-failed".to_string(),
                            });
                        }
                    }
                }));
            }

            if let Ok(mut stream) = page.event_listener::<EventEntryAdded>().await {
                let d = diag.clone();
                tasks.push(tokio::spawn(async move {
                    while let Some(ev) = stream.next().await {
                        let entry = &ev.entry;
                        let severity = log_severity(&entry.level);
                        let is_security = matches!(entry.source, LogEntrySource::Security);
                        if let Ok(mut g) = d.lock() {
                            if is_security && g.violations.len() < MAX_ITEMS_PER_KIND {
                                g.violations.push(SecurityIssue {
                                    kind: "security".to_string(),
                                    text: clip(&entry.text),
                                    url: entry.url.clone(),
                                });
                            } else if !is_security && g.console.len() < MAX_ITEMS_PER_KIND {
                                g.console.push(ConsoleMessage {
                                    severity,
                                    kind: format!("{:?}", entry.source).to_lowercase(),
                                    text: clip(&entry.text),
                                    url: entry.url.clone(),
                                    line: None,
                                });
                            }
                        }
                    }
                }));
            }

            Collector { diag, tasks }
        }

        /// Stop collecting and return the diagnostics with recomputed severity counts.
        pub fn finish(self) -> BrowserDiagnostics {
            for t in &self.tasks {
                t.abort();
            }
            let mut diag = self.diag.lock().map(|g| g.clone()).unwrap_or_default();
            diag.recount();
            diag
        }
    }

    fn console_severity(t: &ConsoleApiCalledType) -> Severity {
        match t {
            ConsoleApiCalledType::Error => Severity::Error,
            ConsoleApiCalledType::Warning => Severity::Warning,
            _ => Severity::Info,
        }
    }

    fn log_severity(l: &LogEntryLevel) -> Severity {
        match l {
            LogEntryLevel::Error => Severity::Error,
            LogEntryLevel::Warning => Severity::Warning,
            _ => Severity::Info,
        }
    }

    fn remote_object_to_text(obj: &RemoteObject) -> String {
        if let Some(desc) = &obj.description {
            return desc.clone();
        }
        match &obj.value {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn console(severity: Severity, text: &str) -> ConsoleMessage {
        ConsoleMessage {
            severity,
            kind: "log".to_string(),
            text: text.to_string(),
            url: None,
            line: None,
        }
    }

    #[test]
    fn empty_diagnostics_produce_empty_payload() {
        let d = BrowserDiagnostics::default();
        assert_eq!(d.to_ai_payload(100, 200, 128), "");
    }

    #[test]
    fn each_message_is_truncated_to_char_limit() {
        let mut d = BrowserDiagnostics::default();
        d.console.push(console(Severity::Error, &"x".repeat(500)));
        let payload = d.to_ai_payload(100, 50, 128);
        assert!(payload.contains('…'));
        let line = payload.lines().next().unwrap();
        assert_eq!(line.chars().filter(|&c| c == 'x').count(), 50);
    }

    #[test]
    fn message_count_is_capped() {
        let mut d = BrowserDiagnostics::default();
        for i in 0..10 {
            d.console.push(console(Severity::Error, &format!("err {}", i)));
        }
        let payload = d.to_ai_payload(3, 200, 128);
        assert!(payload.contains("more message(s) omitted"));
        assert_eq!(payload.lines().count(), 4); // 3 kept + 1 omitted-notice
    }

    #[test]
    fn total_size_is_capped() {
        let mut d = BrowserDiagnostics::default();
        for i in 0..50 {
            d.console.push(console(Severity::Error, &format!("error number {}", i)));
        }
        let payload = d.to_ai_payload(100, 200, 0);
        assert!(payload.contains("truncated to size limit"));
    }

    #[test]
    fn errors_precede_warnings_precede_info() {
        let mut d = BrowserDiagnostics::default();
        d.console.push(console(Severity::Info, "info-msg"));
        d.console.push(console(Severity::Warning, "warn-msg"));
        d.console.push(console(Severity::Error, "error-msg"));
        let payload = d.to_ai_payload(100, 200, 128);
        let err = payload.find("error-msg").unwrap();
        let warn = payload.find("warn-msg").unwrap();
        let info = payload.find("info-msg").unwrap();
        assert!(err < warn && warn < info);
    }
}
