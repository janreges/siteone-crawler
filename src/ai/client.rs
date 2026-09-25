// SiteOne Crawler - AI HTTP client
// (c) Jan Reges <jan.reges@siteone.cz>
//
// A thin client over the existing `reqwest` dependency (no new LLM crate). Mirrors the
// patterns of `engine/http_client.rs`: shared client, per-request timeout, on-disk cache.
// Adds retry/backoff on 429/5xx and provider-native request shaping + response parsing.

use std::path::Path;
use std::time::{Duration, Instant};

use md5::{Digest, Md5};
use once_cell::sync::Lazy;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};

use super::config::AiConfig;
use super::provider::{self, ChatRequest, ModelInfo, Provider, Usage};
use super::secret::Redactor;
use super::telemetry::{self, RequestOutcome, RequestRecord};
use crate::error::{CrawlerError, CrawlerResult};

const MAX_ATTEMPTS: u32 = 3;

/// Delay before the single end-to-end retry in `complete_parsed`.
const PARSE_RETRY_DELAY_SECS: u64 = 5;

/// A configured API key shorter than this is not blanked out of an answer: placeholder keys such
/// as `EMPTY`, `none` or `sk-no-key-required` are words an answer may say itself.
const MIN_KEY_CHARS_BLANKED_IN_ANSWERS: usize = 20;

/// One process-wide gate is intentional: usage accounting already assumes one crawl per process,
/// and report extraction plus the later executive summary build separate `AiClient` instances.
/// Sharing the gate keeps their actual HTTP sends under the same configured rate limit.
static NEXT_REQUEST_AT: Lazy<tokio::sync::Mutex<Instant>> = Lazy::new(|| tokio::sync::Mutex::new(Instant::now()));

/// Result of a successful completion. `text` is the raw model output (callers run it
/// through `normalize::*` before parsing).
pub struct AiCompletion {
    pub text: String,
    pub usage: Usage,
    pub from_cache: bool,
    pub finish_reason: Option<String>,
    /// The answering HTTP attempt's time from send to body read (None for a cache hit).
    pub duration_ms: Option<u64>,
}

impl AiCompletion {
    pub fn was_truncated(&self) -> bool {
        self.finish_reason.as_deref().is_some_and(|reason| {
            matches!(
                reason.trim().to_ascii_lowercase().as_str(),
                "length" | "max_tokens" | "max_output_tokens" | "token_limit"
            )
        })
    }

    pub fn was_interrupted(&self, provider: Provider) -> bool {
        self.finish_reason
            .as_deref()
            .is_some_and(|reason| !successful_finish_reason(provider, reason))
    }
}

/// On-disk cache record (content-addressed; never contains the API key). `usage` is the usage as
/// reported, an unknown count as null. `prompt_tokens` and `completion_tokens` keep their original
/// meaning for older builds, 0/0 meaning "not reported"; a record without `usage`, written by an
/// older build, is read that way.
#[derive(Serialize, Deserialize)]
struct CachedCompletion {
    text: String,
    prompt_tokens: u64,
    completion_tokens: u64,
    #[serde(default)]
    usage: Option<Usage>,
    #[serde(default)]
    finish_reason: Option<String>,
}

pub struct AiClient {
    client: reqwest::Client,
    config: AiConfig,
    request_interval: Option<Duration>,
    redactor: Redactor,
}

impl AiClient {
    pub fn new(config: AiConfig) -> Self {
        let client = reqwest::Client::builder()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let request_interval = config
            .max_reqs_per_sec
            .filter(|rate| rate.is_finite() && *rate > 0.0)
            .map(|rate| Duration::from_secs_f64(1.0 / rate));
        let redactor = Redactor::new(config.api_key.as_deref(), &config.endpoint);
        Self {
            client,
            config,
            request_interval,
            redactor,
        }
    }

    pub fn config(&self) -> &AiConfig {
        &self.config
    }

    /// Build the neutral request the client shapes per provider.
    pub fn provider(&self) -> Provider {
        self.config.provider
    }

    /// Perform a chat completion using the client's default `extra_body`. `category` is a
    /// human-readable analysis-type label used for the per-type token accounting in the summary.
    pub async fn complete(&self, req: &ChatRequest, category: &str) -> CrawlerResult<AiCompletion> {
        self.complete_with(req, None, category).await
    }

    /// `complete` + the caller's `parse`, retried once when the model returns output that `parse`
    /// rejects. Transport retries are handled inside `complete_cached`; they are not repeated again
    /// here, especially after a timeout that might already have incurred provider cost.
    pub async fn complete_parsed<T>(
        &self,
        req: &ChatRequest,
        category: &str,
        parse: impl Fn(&str) -> Result<T, String>,
    ) -> CrawlerResult<(T, AiCompletion)> {
        self.complete_parsed_n(req, category, 2, parse).await
    }

    /// Like `complete_parsed`, but with a caller-chosen number of TOTAL attempts (`max_attempts`,
    /// clamped to >=1). Attempt 0 may read the cache; every retry bypasses the cache READ (so it
    /// re-calls the model rather than re-reading the same malformed completion) and pauses first.
    /// Returns Err with the last failure only after ALL attempts fail — the caller decides how to
    /// record that (e.g. mark the page as an error; NEVER fabricate a value). Provider/transport
    /// failures return after the HTTP layer's own retry policy; this loop is for invalid model output.
    pub async fn complete_parsed_n<T>(
        &self,
        req: &ChatRequest,
        category: &str,
        max_attempts: u32,
        parse: impl Fn(&str) -> Result<T, String>,
    ) -> CrawlerResult<(T, AiCompletion)> {
        let attempts = max_attempts.max(1);
        let mut last_err: Option<CrawlerError> = None;
        let mut attempt = 0;
        let mut active_req = req.clone();
        let mut used_output_format_fallback = false;
        while attempt < attempts {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_secs(PARSE_RETRY_DELAY_SECS)).await;
            }
            match self
                .complete_cached(
                    &active_req,
                    None,
                    category,
                    attempt == 0,
                    attempt > 0 || used_output_format_fallback,
                )
                .await
            {
                Ok(completion) if completion.was_truncated() => {
                    self.evict_cached(&active_req, None);
                    last_err = Some(CrawlerError::Other(self.redact(&format!(
                        "invalid response: generation stopped at token limit ({})",
                        completion.finish_reason.as_deref().unwrap_or("unknown")
                    ))));
                }
                Ok(completion) if completion.was_interrupted(self.provider()) => {
                    self.evict_cached(&active_req, None);
                    last_err = Some(CrawlerError::Other(self.redact(&format!(
                        "invalid response: provider stopped generation abnormally ({})",
                        completion.finish_reason.as_deref().unwrap_or("unknown")
                    ))));
                }
                Ok(completion) => match parse(&completion.text) {
                    Ok(value) => return Ok((value, completion)),
                    Err(e) => {
                        self.evict_cached(&active_req, None);
                        last_err = Some(CrawlerError::Other(self.redact(&format!("invalid response: {}", e))));
                    }
                },
                Err(e)
                    if !used_output_format_fallback
                        && (active_req.json_schema.is_some() || active_req.json_mode)
                        && is_structured_output_unsupported(&e.to_string()) =>
                {
                    active_req.json_schema = None;
                    active_req.schema_name = None;
                    // A true prompt-mode fallback must also remove JSON-object mode. Several local
                    // OpenAI-compatible endpoints reject `response_format` altogether, not only
                    // its `json_schema` variant.
                    active_req.json_mode = false;
                    used_output_format_fallback = true;
                    last_err = Some(e);
                    continue;
                }
                Err(e) => return Err(e),
            }
            attempt += 1;
        }
        Err(last_err.unwrap_or_else(|| CrawlerError::Other("AI request failed".to_string())))
    }

    /// Perform a chat completion, optionally overriding `extra_body` for this single request
    /// (used by the report synthesis, e.g. to enable thinking only for that call). `category`
    /// labels the call for per-analysis-type token accounting.
    pub async fn complete_with(
        &self,
        req: &ChatRequest,
        extra_body_override: Option<&serde_json::Value>,
        category: &str,
    ) -> CrawlerResult<AiCompletion> {
        let completion = self
            .complete_cached(req, extra_body_override, category, true, false)
            .await?;
        if completion.was_interrupted(self.provider()) {
            self.evict_cached(req, extra_body_override);
            return Err(CrawlerError::Other(self.redact(&format!(
                "AI provider stopped generation abnormally ({})",
                completion.finish_reason.as_deref().unwrap_or("unknown")
            ))));
        }
        Ok(completion)
    }

    /// Like `complete_with`, but `use_cache = false` bypasses the cache READ so a fresh API call
    /// is made. Used by the parse-retry — otherwise the retry would re-read the same malformed
    /// cached completion and the second attempt would be pointless. A fresh result still overwrites
    /// the cache entry.
    async fn complete_cached(
        &self,
        req: &ChatRequest,
        extra_body_override: Option<&serde_json::Value>,
        category: &str,
        use_cache: bool,
        retry_context: bool,
    ) -> CrawlerResult<AiCompletion> {
        let extra_body = extra_body_override.or(self.config.extra_body.as_ref());
        let shaped = provider::shape_request(
            self.config.provider,
            &self.config.model,
            &self.config.endpoint,
            self.config.api_key.as_deref(),
            req,
            self.config.force_completion_tokens,
            extra_body,
        );

        // Cache key from URL + body only (no auth headers).
        let cache_key = self.cache_key(&shaped.url, &shaped.body);
        // One telemetry record per cache hit and per HTTP attempt.
        let base = self.base_record(category);
        if use_cache && let Some(hit) = self.get_cached(&cache_key) {
            super::usage::record(category, &hit.usage, true);
            telemetry::report(RequestRecord {
                outcome: RequestOutcome::CacheHit,
                usage: hit.usage.has_tokens().then_some(hit.usage),
                finish_reason: hit.finish_reason.as_deref().map(|reason| self.redact(reason)),
                ..base
            });
            return Ok(hit);
        }

        let headers = header_map(&shaped.headers);

        let timeout = Duration::from_secs(self.config.timeout_secs.max(1));
        let body_string = serde_json::to_string(&shaped.body)
            .map_err(|e| CrawlerError::Other(format!("AI request serialization error: {}", e)))?;
        let mut last_err = String::from("unknown error");
        // Reports a failed attempt and returns the error for the caller.
        let fail = |record: RequestRecord, message: String| {
            let message = self.redact(&message);
            telemetry::report(RequestRecord {
                outcome: RequestOutcome::Error,
                error: Some(message.clone()),
                ..record
            });
            CrawlerError::Other(message)
        };

        let _call_time = CallTime {
            category,
            started: Instant::now(),
        };
        for attempt in 0..MAX_ATTEMPTS {
            self.wait_for_rate_slot().await;
            super::usage::record_http_attempt(retry_context || attempt > 0);
            let will_retry = attempt + 1 < MAX_ATTEMPTS;
            // The attempt's duration excludes the rate-limit wait above and the backoff below.
            let sent_at = Instant::now();
            let record = RequestRecord {
                attempt: attempt + 1,
                ..base.clone()
            };
            let resp = self
                .client
                .post(&shaped.url)
                .headers(headers.clone())
                .body(body_string.clone())
                .timeout(timeout)
                .send()
                .await;

            match resp {
                Ok(r) => {
                    let status = r.status();
                    let retry_after = r
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.trim().parse::<u64>().ok());
                    let code = status.as_u16();
                    let record = RequestRecord {
                        http_status: Some(code),
                        duration_ms: Some(elapsed_ms(sent_at)),
                        ..record
                    };

                    if code == 429 || (500..=599).contains(&code) {
                        last_err = format!("HTTP {}", code);
                        if will_retry {
                            telemetry::report(RequestRecord {
                                outcome: RequestOutcome::Retry,
                                error: Some(last_err.clone()),
                                ..record
                            });
                            self.backoff(attempt, retry_after).await;
                            continue;
                        }
                        return Err(fail(record, format!("AI request failed after retries: {}", last_err)));
                    }

                    let body_text = match r.text().await {
                        Ok(text) => text,
                        Err(e) => {
                            // A 2xx answer all the same: a completed call without token usage.
                            if status.is_success() {
                                super::usage::record(category, &Usage::default(), false);
                            }
                            let record = RequestRecord {
                                duration_ms: Some(elapsed_ms(sent_at)),
                                ..record
                            };
                            return Err(fail(
                                record,
                                format!("AI response read error: {}", transport_error_text(e)),
                            ));
                        }
                    };
                    let record = RequestRecord {
                        duration_ms: Some(elapsed_ms(sent_at)),
                        ..record
                    };

                    let json: serde_json::Value = match serde_json::from_str(&body_text) {
                        Ok(json) => json,
                        Err(e) => {
                            // A 2xx answer all the same: a completed call without token usage.
                            if status.is_success() {
                                super::usage::record(category, &Usage::default(), false);
                            }
                            return Err(fail(
                                record,
                                format!(
                                    "AI response is not valid JSON: {} (body starts: {})",
                                    e,
                                    snippet(&self.redact(&body_text))
                                ),
                            ));
                        }
                    };

                    let parsed_usage = provider::parse_usage(self.config.provider, &json);
                    let usage = parsed_usage.unwrap_or_default();
                    let finish_reason = provider::parse_finish_reason(self.config.provider, &json);
                    let record = RequestRecord {
                        usage: parsed_usage,
                        reasoning_chars: provider::parse_reasoning(self.config.provider, &json),
                        finish_reason: finish_reason.as_deref().map(|reason| self.redact(reason)),
                        ..record
                    };

                    // A successful HTTP response may still be a refusal/safety response with no
                    // content. Account for any provider-reported tokens before validating content.
                    if status.is_success() {
                        super::usage::record(category, &usage, false);
                    }

                    // Non-2xx with a parseable body, or a 200 body carrying a provider error.
                    if let Some(msg) = provider::extract_error(self.config.provider, &json) {
                        return Err(fail(record, format!("AI provider error: {}", msg)));
                    }
                    if !status.is_success() {
                        return Err(fail(
                            record,
                            format!("AI HTTP {}: {}", code, snippet(&self.redact(&body_text))),
                        ));
                    }

                    let Some(text) = provider::parse_content(self.config.provider, &json) else {
                        return Err(fail(record, self.no_content_message(&json)));
                    };
                    let text = self.without_api_key(text);
                    let duration_ms = record.duration_ms;
                    telemetry::report(record);

                    let completion = AiCompletion {
                        text,
                        usage,
                        from_cache: false,
                        finish_reason,
                        duration_ms,
                    };
                    self.store_cached(&cache_key, &completion);
                    return Ok(completion);
                }
                Err(e) => {
                    // Do NOT retry on timeout: a paid completion may have been processed
                    // server-side, so retrying could double-charge. Only retry when we know
                    // the request never reached/processed (connect/build errors). reqwest reports
                    // a timeout as a request error too, hence the explicit exclusion.
                    let retriable = !e.is_timeout() && (e.is_connect() || e.is_request());
                    let record = RequestRecord {
                        duration_ms: Some(elapsed_ms(sent_at)),
                        ..record
                    };
                    let error = self.redact(&transport_error_text(e));
                    last_err = error.clone();
                    if retriable && will_retry {
                        telemetry::report(RequestRecord {
                            outcome: RequestOutcome::Retry,
                            error: Some(error),
                            ..record
                        });
                        self.backoff(attempt, None).await;
                        continue;
                    }
                    telemetry::report(RequestRecord {
                        outcome: RequestOutcome::Error,
                        error: Some(format!("AI request error: {}", error)),
                        ..record
                    });
                    return Err(CrawlerError::Other(format!("AI request error: {}", last_err)));
                }
            }
        }

        Err(CrawlerError::Other(format!("AI request failed: {}", last_err)))
    }

    /// The models the endpoint offers, sorted by id (one GET, no retries). The error is a
    /// credential-free sentence.
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, String> {
        let shaped = provider::models_request(
            self.config.provider,
            &self.config.endpoint,
            self.config.api_key.as_deref(),
        );
        let response = self
            .client
            .get(&shaped.url)
            .headers(header_map(&shaped.headers))
            .timeout(Duration::from_secs(self.config.timeout_secs.max(1)))
            .send()
            .await
            .map_err(|e| self.redact(&format!("AI request error: {}", transport_error_text(e))))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| self.redact(&format!("AI response read error: {}", transport_error_text(e))))?;
        let json = serde_json::from_str::<serde_json::Value>(&body).ok();
        if !status.is_success() {
            let detail = json
                .as_ref()
                .and_then(|json| provider::extract_error(self.config.provider, json))
                .unwrap_or_else(|| snippet(&self.redact(&body)));
            return Err(self.redact(&match detail.is_empty() {
                true => format!("HTTP {}", status.as_u16()),
                false => format!("HTTP {}: {}", status.as_u16(), detail),
            }));
        }
        let Some(json) = json else {
            return Err(self.redact(&format!(
                "The endpoint's answer is not JSON (body starts: {})",
                snippet(&self.redact(&body))
            )));
        };
        if let Some(message) = provider::extract_error(self.config.provider, &json) {
            return Err(self.redact(&format!("AI provider error: {}", message)));
        }
        provider::parse_model_list(&json).ok_or_else(|| {
            self.redact(&format!(
                "The endpoint's answer is not a model list (body starts: {})",
                snippet(&self.redact(&body))
            ))
        })
    }

    /// The error text for a response without content, quoting the model's refusal when it gave one.
    fn no_content_message(&self, json: &serde_json::Value) -> String {
        match provider::parse_refusal(self.config.provider, json) {
            // Redacted before it is cut: a cut through a credential would leave a part of it.
            Some(refusal) => format!(
                "AI response had no content (refusal: {})",
                snippet(&self.redact(&refusal))
            ),
            None => "AI response had no content".to_string(),
        }
    }

    /// `text` without the credentials of the connection, should a provider or proxy echo them
    /// (see `Redactor`). Applied to whatever the client reports; a body is redacted before it is cut.
    pub fn redact(&self, text: &str) -> String {
        self.redactor.redact(text)
    }

    /// A successful answer without the configured API key, should the provider or a proxy echo it:
    /// the answer is cached, parsed and published. Only a key of 20+ characters is blanked out.
    fn without_api_key(&self, text: String) -> String {
        match self.config.api_key.as_deref() {
            Some(key) if key.chars().count() >= MIN_KEY_CHARS_BLANKED_IN_ANSWERS && text.contains(key) => {
                text.replace(key, "[redacted]")
            }
            _ => text,
        }
    }

    /// The telemetry record of a request made now, carrying the subject of the enclosing
    /// `telemetry::scope`.
    fn base_record(&self, category: &str) -> RequestRecord {
        let subject = telemetry::current_subject().unwrap_or_default();
        RequestRecord {
            seq: 0,
            task: subject.task.as_deref().map(telemetry::task_ref),
            category: category.to_string(),
            subject: subject.subject,
            provider: self.config.provider.as_str(),
            model: self.config.model.clone(),
            attempt: 1,
            max_attempts: MAX_ATTEMPTS,
            outcome: RequestOutcome::Ok,
            http_status: None,
            error: None,
            usage: None,
            reasoning_chars: None,
            duration_ms: None,
            finish_reason: None,
        }
    }

    async fn backoff(&self, attempt: u32, retry_after: Option<u64>) {
        let secs = retry_after.unwrap_or_else(|| 1u64 << attempt); // 1s, 2s, 4s
        // Bound hostile or accidental multi-hour values, while still honoring normal provider
        // windows such as Retry-After: 60/120 instead of retrying prematurely after 30 seconds.
        tokio::time::sleep(Duration::from_secs(secs.min(300))).await;
    }

    fn cache_key(&self, url: &str, body: &serde_json::Value) -> String {
        let mut hasher = Md5::new();
        hasher.update(self.config.provider.as_str().as_bytes());
        hasher.update(url.as_bytes());
        hasher.update(self.config.model.as_bytes());
        hasher.update(body.to_string().as_bytes());
        crate::utils::to_lower_hex(hasher.finalize())
    }

    fn cache_file_path(&self, key: &str) -> Option<String> {
        let dir = self.config.cache_dir.as_ref()?;
        Some(format!("{}/{}/{}.json", dir, &key[..2], key))
    }

    fn get_cached(&self, key: &str) -> Option<AiCompletion> {
        let path = self.cache_file_path(key)?;
        if !Path::new(&path).is_file() {
            return None;
        }
        let data = std::fs::read_to_string(&path).ok()?;
        let cached: CachedCompletion = serde_json::from_str(&data).ok()?;
        let usage = match cached.usage {
            Some(usage) => usage,
            None if cached.prompt_tokens > 0 || cached.completion_tokens > 0 => Usage {
                input_tokens: Some(cached.prompt_tokens),
                output_tokens: Some(cached.completion_tokens),
                ..Usage::default()
            },
            None => Usage::default(),
        };
        Some(AiCompletion {
            // A file an older build wrote may still hold the key.
            text: self.without_api_key(cached.text),
            usage,
            from_cache: true,
            finish_reason: cached.finish_reason,
            duration_ms: None,
        })
    }

    fn store_cached(&self, key: &str, completion: &AiCompletion) {
        let path = match self.cache_file_path(key) {
            Some(p) => p,
            None => return,
        };
        if let Some(parent) = Path::new(&path).parent()
            && !parent.is_dir()
        {
            let _ = std::fs::create_dir_all(parent);
        }
        let cached = CachedCompletion {
            text: completion.text.clone(),
            prompt_tokens: completion.usage.input(),
            completion_tokens: completion.usage.output(),
            usage: Some(completion.usage),
            finish_reason: completion.finish_reason.clone(),
        };
        if let Ok(json) = serde_json::to_string(&cached) {
            let _ = std::fs::write(&path, json);
        }
    }

    fn evict_cached(&self, req: &ChatRequest, extra_body_override: Option<&serde_json::Value>) {
        let extra_body = extra_body_override.or(self.config.extra_body.as_ref());
        let shaped = provider::shape_request(
            self.config.provider,
            &self.config.model,
            &self.config.endpoint,
            self.config.api_key.as_deref(),
            req,
            self.config.force_completion_tokens,
            extra_body,
        );
        let key = self.cache_key(&shaped.url, &shaped.body);
        if let Some(path) = self.cache_file_path(&key) {
            let _ = std::fs::remove_file(path);
        }
    }

    async fn wait_for_rate_slot(&self) {
        let Some(interval) = self.request_interval else {
            return;
        };
        let mut next = NEXT_REQUEST_AT.lock().await;
        let now = Instant::now();
        if *next > now {
            tokio::time::sleep(*next - now).await;
        }
        *next = Instant::now() + interval;
    }
}

/// Reset the shared send gate at the start of a crawler run.
pub async fn reset_rate_limiter() {
    *NEXT_REQUEST_AT.lock().await = Instant::now();
}

fn successful_finish_reason(provider: Provider, reason: &str) -> bool {
    let reason = reason.trim().to_ascii_lowercase();
    match provider {
        Provider::OpenAi => reason == "stop",
        Provider::OpenAiCompatible => matches!(
            reason.as_str(),
            "stop" | "eos" | "eos_token" | "end_turn" | "stop_sequence" | "complete" | "completed"
        ),
        Provider::Anthropic => matches!(reason.as_str(), "end_turn" | "stop_sequence"),
        Provider::Gemini => reason == "stop",
    }
}

fn is_structured_output_unsupported(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    let mentions_schema = [
        "response_format",
        "json_schema",
        "guided_json",
        "structured output",
        "responseschema",
        "responsejsonschema",
        "response_json_schema",
    ]
    .iter()
    .any(|needle| message.contains(needle));
    let rejects_capability = [
        "not supported",
        "unsupported",
        "does not support",
        "unknown parameter",
        "unknown name",
        "unrecognized",
        "invalid parameter",
        "not permitted",
        "not allowed",
        "extra inputs are not permitted",
        "extra_forbidden",
    ]
    .iter()
    .any(|needle| message.contains(needle));
    mentions_schema && rejects_capability
}

/// The shaped headers as a `HeaderMap`; a name or value HTTP cannot carry is left out.
fn header_map(headers: &[(String, String)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (k, v) in headers {
        if let (Ok(name), Ok(val)) = (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(v)) {
            map.insert(name, val);
        }
    }
    map
}

fn elapsed_ms(since: Instant) -> u64 {
    since.elapsed().as_millis() as u64
}

/// Adds the time of one AI call to the usage totals when dropped, i.e. however the call ends.
struct CallTime<'a> {
    category: &'a str,
    started: Instant,
}

impl Drop for CallTime<'_> {
    fn drop(&mut self) {
        super::usage::record_call_time(self.category, elapsed_ms(self.started));
    }
}

/// A transport error with its causes but without its URL, which may carry credentials.
fn transport_error_text(e: reqwest::Error) -> String {
    let e = e.without_url();
    let mut text = e.to_string();
    let mut source = std::error::Error::source(&e);
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

fn snippet(s: &str) -> String {
    let t = s.trim();
    // Truncate by characters, not bytes, so a multibyte char (e.g. non-ASCII error
    // messages) at the boundary never panics.
    if t.chars().count() > 200 {
        let truncated: String = t.chars().take(200).collect();
        format!("{}…", truncated)
    } else {
        t.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_limit_finish_reason_is_never_parseable_success() {
        for reason in ["length", "max_tokens", "MAX_TOKENS"] {
            let completion = AiCompletion {
                text: "{}".to_string(),
                usage: Usage::default(),
                from_cache: false,
                finish_reason: Some(reason.to_string()),
                duration_ms: None,
            };
            assert!(completion.was_truncated());
        }
        let completion = AiCompletion {
            text: "{}".to_string(),
            usage: Usage::default(),
            from_cache: false,
            finish_reason: Some("stop".to_string()),
            duration_ms: None,
        };
        assert!(!completion.was_truncated());
        assert!(!completion.was_interrupted(Provider::OpenAi));
    }

    #[test]
    fn safety_and_filter_finish_reasons_are_never_success() {
        for (provider, reason) in [
            (Provider::OpenAi, "content_filter"),
            (Provider::Anthropic, "refusal"),
            (Provider::Gemini, "SAFETY"),
            (Provider::Gemini, "RECITATION"),
        ] {
            let completion = AiCompletion {
                text: "{}".to_string(),
                usage: Usage::default(),
                from_cache: false,
                finish_reason: Some(reason.to_string()),
                duration_ms: None,
            };
            assert!(completion.was_interrupted(provider), "{provider:?} {reason}");
        }
    }

    fn client_with_cache(dir: &std::path::Path) -> AiClient {
        AiClient::new(AiConfig {
            provider: Provider::OpenAiCompatible,
            endpoint: "http://127.0.0.1:9/v1".to_string(),
            model: "m".to_string(),
            api_key: None,
            max_tokens: 100,
            temperature: 0.0,
            force_completion_tokens: false,
            extra_body: None,
            timeout_secs: 1,
            cache_dir: Some(dir.display().to_string()),
            max_reqs_per_sec: None,
        })
    }

    #[test]
    fn cache_keeps_reasoning_and_cached_tokens() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let client = client_with_cache(dir.path());
        let usage = Usage {
            input_tokens: Some(17),
            output_tokens: Some(37),
            reasoning_tokens: Some(33),
            cached_input_tokens: Some(4),
        };
        let completion = AiCompletion {
            text: "OK".to_string(),
            usage,
            from_cache: false,
            finish_reason: Some("stop".to_string()),
            duration_ms: None,
        };
        client.store_cached("abcdef", &completion);
        let hit = client.get_cached("abcdef").expect("a cache hit");
        assert_eq!(hit.usage, usage);
        assert!(hit.from_cache);
        assert_eq!(hit.text, "OK");
    }

    #[test]
    fn cache_keeps_unknown_and_reported_zero_counts_apart() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let client = client_with_cache(dir.path());
        for usage in [
            usage(Some(42), None, None, None),
            usage(None, Some(19), Some(0), None),
            usage(Some(0), Some(0), None, Some(0)),
            Usage::default(),
        ] {
            let completion = AiCompletion {
                text: "OK".to_string(),
                usage,
                from_cache: false,
                finish_reason: None,
                duration_ms: Some(80),
            };
            client.store_cached("abcdef", &completion);
            let hit = client.get_cached("abcdef").expect("a cache hit");
            assert_eq!(hit.usage, usage);
        }
    }

    fn usage(input: Option<u64>, output: Option<u64>, reasoning: Option<u64>, cached: Option<u64>) -> Usage {
        Usage {
            input_tokens: input,
            output_tokens: output,
            reasoning_tokens: reasoning,
            cached_input_tokens: cached,
        }
    }

    #[test]
    fn cache_files_of_older_builds_still_load() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let client = client_with_cache(dir.path());
        std::fs::create_dir_all(dir.path().join("ab")).expect("the cache subdir");
        std::fs::write(
            dir.path().join("ab/abcdef.json"),
            r#"{"text":"OK","prompt_tokens":17,"completion_tokens":37,"finish_reason":"stop"}"#,
        )
        .expect("an old cache file");
        std::fs::write(
            dir.path().join("ab/ab0000.json"),
            r#"{"text":"OK","prompt_tokens":0,"completion_tokens":0}"#,
        )
        .expect("an old cache file without usage");

        let hit = client.get_cached("abcdef").expect("an old record loads");
        assert_eq!(
            hit.usage,
            Usage {
                input_tokens: Some(17),
                output_tokens: Some(37),
                reasoning_tokens: None,
                cached_input_tokens: None,
            }
        );
        // Older builds stored "not reported" as 0/0.
        let hit = client.get_cached("ab0000").expect("an old record loads");
        assert_eq!(hit.usage, Usage::default());
    }

    #[tokio::test]
    async fn requests_carry_the_subject_of_their_scope() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let client = client_with_cache(dir.path());
        let subject = telemetry::Subject {
            task: Some("seo".to_string()),
            subject: Some("/about".to_string()),
        };
        let record = telemetry::scope(subject, async { client.base_record("SEO analysis") }).await;
        assert_eq!(record.task.map(|task| task.key).as_deref(), Some("seo"));
        assert_eq!(record.subject.as_deref(), Some("/about"));
        assert_eq!(record.category, "SEO analysis");
        assert_eq!((record.provider, record.model.as_str()), ("openai-compatible", "m"));
        assert_eq!((record.attempt, record.max_attempts), (1, MAX_ATTEMPTS));

        let outside = client.base_record("SEO analysis");
        assert!(outside.task.is_none() && outside.subject.is_none());
    }

    #[tokio::test]
    async fn transport_errors_are_reported_without_their_url() {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .expect("a free port")
            .port();
        let error = reqwest::Client::new()
            .post(format!("http://user:secret-xyz@127.0.0.1:{port}/v1/chat/completions"))
            .send()
            .await
            .expect_err("nothing listens there");
        let text = transport_error_text(error);
        assert!(!text.contains("secret-xyz") && !text.contains("127.0.0.1"), "{text}");
        assert!(text.starts_with("error sending request: "), "the causes follow: {text}");
    }

    #[test]
    fn errors_never_echo_the_api_key() {
        let client = client_with_connection("http://127.0.0.1:9/v1", "secret-xyz");
        assert_eq!(
            client.redact("AI provider error: invalid key secret-xyz"),
            "AI provider error: invalid key [redacted]"
        );
    }

    #[test]
    fn answers_lose_only_a_key_of_20_or_more_characters() {
        let key = "sk-key-of-20-chars-x";
        let client = client_with_connection("http://127.0.0.1:9/v1", key);
        assert_eq!(
            client.without_api_key(format!(r#"{{"title":"Key {key}"}}"#)),
            r#"{"title":"Key [redacted]"}"#
        );
        // Placeholder keys and other short values are words an answer may say itself.
        for key in ["EMPTY", "none", "sk-no-key-required", "2026-09-25", &key[..19]] {
            let client = client_with_connection("http://127.0.0.1:9/v1", key);
            let answer = format!(r#"{{"title":"{key}"}}"#);
            assert_eq!(client.without_api_key(answer.clone()), answer);
        }
    }

    fn client_with_connection(endpoint: &str, api_key: &str) -> AiClient {
        AiClient::new(AiConfig {
            endpoint: endpoint.to_string(),
            api_key: Some(api_key.to_string()),
            ..client_with_cache(std::path::Path::new("/nonexistent")).config
        })
    }

    #[test]
    fn errors_never_echo_the_credentials_of_the_endpoint() {
        let client = client_with_connection(
            "http://review:PW_SENTINEL_0002@proxy.test:8000/v1?token=QUERY_SENTINEL_0003",
            "sk-SENTINEL-key-0001",
        );
        // A proxy that quotes the endpoint it rejected, and the key and token on their own.
        let echo = "Proxy rejected http://review:PW_SENTINEL_0002@proxy.test:8000/v1?token=QUERY_SENTINEL_0003 \
                    (key sk-SENTINEL-key-0001, token QUERY_SENTINEL_0003, password PW_SENTINEL_0002)";
        let redacted = client.redact(echo);
        for secret in [
            "PW_SENTINEL_0002",
            "QUERY_SENTINEL_0003",
            "sk-SENTINEL-key-0001",
            "review:",
        ] {
            assert!(!redacted.contains(secret), "{secret} in {redacted}");
        }
        assert!(
            redacted.starts_with("Proxy rejected http://proxy.test:8000/v1"),
            "{redacted}"
        );

        // Userinfo as written in the endpoint (percent-encoded) and as a server decodes it.
        let client = client_with_connection("http://user:P%40ss_SENTINEL_0004@proxy.test/v1", "sk-SENTINEL-key-0001");
        for echo in ["bad password P%40ss_SENTINEL_0004", "bad password P@ss_SENTINEL_0004"] {
            let redacted = client.redact(echo);
            assert!(!redacted.contains("ss_SENTINEL_0004"), "{redacted}");
        }
        // Userinfo that does not decode, in any URL the message quotes.
        let redacted = client.redact("at http://%FFuser:PW_SENTINEL_0005@other.test/v1: refused");
        assert_eq!(redacted, "at http://other.test/v1: refused");
    }

    #[test]
    fn a_short_key_withholds_the_message_rather_than_spelling_itself_out() {
        // Blanking `a` out of "invalid API key" leaves "inv[redacted]lid": the blanks spell the key.
        let client = client_with_connection("http://127.0.0.1:9/v1", "a");
        let redacted = client.redact("invalid API key");
        assert!(!redacted.contains("inv"), "{redacted}");
        assert!(redacted.contains("withheld"), "{redacted}");
        assert_eq!(
            client.redact("HTTP 500"),
            "HTTP 500",
            "a message without it is left alone"
        );
    }

    #[test]
    fn a_refusal_is_redacted_before_it_is_cut() {
        let key = "sk-SENTINEL-key-0001-abcdefghijklmnopqrstuvwxyz";
        let client = client_with_connection("http://127.0.0.1:9/v1", key);
        // The key straddles the 200-character cut: cutting first would leave its prefix.
        let refusal = format!("{}{}", "x".repeat(180), key);
        let refused = serde_json::json!({"choices": [{"message": {"content": null, "refusal": refusal}}]});
        let message = client.no_content_message(&refused);
        assert!(!message.contains(&key[..12]), "{message}");
    }

    #[test]
    fn missing_content_error_quotes_the_refusal() {
        let client = client_with_cache(std::path::Path::new("/nonexistent"));
        let refused =
            serde_json::json!({"choices": [{"message": {"content": null, "refusal": "I can't help with that."}}]});
        assert_eq!(
            client.no_content_message(&refused),
            "AI response had no content (refusal: I can't help with that.)"
        );
        let empty = serde_json::json!({"choices": [{"message": {}}]});
        assert_eq!(client.no_content_message(&empty), "AI response had no content");
    }

    #[test]
    fn structured_output_fallback_only_matches_schema_capability_errors() {
        assert!(is_structured_output_unsupported(
            "AI provider error: response_format json_schema is not supported by this model"
        ));
        assert!(!is_structured_output_unsupported(
            "AI request failed after retries: HTTP 500"
        ));
        assert!(is_structured_output_unsupported(
            "guided_json: extra inputs are not permitted (extra_forbidden)"
        ));
        assert!(is_structured_output_unsupported(
            "Invalid schema for response_format: keyword 'format' is not permitted"
        ));
        assert!(is_structured_output_unsupported(
            "generationConfig.responseJsonSchema is not supported"
        ));
    }
}
