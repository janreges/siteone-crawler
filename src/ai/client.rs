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
use super::provider::{self, ChatRequest, Provider, Usage};
use crate::error::{CrawlerError, CrawlerResult};

const MAX_ATTEMPTS: u32 = 3;

/// Delay before the single end-to-end retry in `complete_parsed`.
const PARSE_RETRY_DELAY_SECS: u64 = 5;

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

/// On-disk cache record (content-addressed; never contains the API key). `prompt_tokens` and
/// `completion_tokens` keep their original meaning and number type, 0/0 meaning "not reported",
/// so records stay readable in both directions between builds; the newer counts are optional.
#[derive(Serialize, Deserialize)]
struct CachedCompletion {
    text: String,
    prompt_tokens: u64,
    completion_tokens: u64,
    #[serde(default)]
    reasoning_tokens: Option<u64>,
    #[serde(default)]
    cached_input_tokens: Option<u64>,
    #[serde(default)]
    finish_reason: Option<String>,
}

pub struct AiClient {
    client: reqwest::Client,
    config: AiConfig,
    request_interval: Option<Duration>,
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
        Self {
            client,
            config,
            request_interval,
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
                    last_err = Some(CrawlerError::Other(format!(
                        "invalid response: generation stopped at token limit ({})",
                        completion.finish_reason.as_deref().unwrap_or("unknown")
                    )));
                }
                Ok(completion) if completion.was_interrupted(self.provider()) => {
                    self.evict_cached(&active_req, None);
                    last_err = Some(CrawlerError::Other(format!(
                        "invalid response: provider stopped generation abnormally ({})",
                        completion.finish_reason.as_deref().unwrap_or("unknown")
                    )));
                }
                Ok(completion) => match parse(&completion.text) {
                    Ok(value) => return Ok((value, completion)),
                    Err(e) => {
                        self.evict_cached(&active_req, None);
                        last_err = Some(CrawlerError::Other(format!("invalid response: {}", e)));
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
            return Err(CrawlerError::Other(format!(
                "AI provider stopped generation abnormally ({})",
                completion.finish_reason.as_deref().unwrap_or("unknown")
            )));
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
        if use_cache && let Some(hit) = self.get_cached(&cache_key) {
            super::usage::record(category, &hit.usage, 0, true);
            return Ok(hit);
        }

        let call_start = std::time::Instant::now();

        // Build headers.
        let mut headers = HeaderMap::new();
        for (k, v) in &shaped.headers {
            if let (Ok(name), Ok(val)) = (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(v)) {
                headers.insert(name, val);
            }
        }

        let timeout = Duration::from_secs(self.config.timeout_secs.max(1));
        let body_string = serde_json::to_string(&shaped.body)
            .map_err(|e| CrawlerError::Other(format!("AI request serialization error: {}", e)))?;
        let mut last_err = String::from("unknown error");

        for attempt in 0..MAX_ATTEMPTS {
            self.wait_for_rate_slot().await;
            super::usage::record_http_attempt(retry_context || attempt > 0);
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

                    if code == 429 || (500..=599).contains(&code) {
                        last_err = format!("HTTP {}", code);
                        if attempt + 1 < MAX_ATTEMPTS {
                            self.backoff(attempt, retry_after).await;
                            continue;
                        }
                        return Err(CrawlerError::Other(format!(
                            "AI request failed after retries: {}",
                            last_err
                        )));
                    }

                    let body_text = r
                        .text()
                        .await
                        .map_err(|e| CrawlerError::Other(format!("AI response read error: {}", e)))?;

                    let json: serde_json::Value = serde_json::from_str(&body_text).map_err(|e| {
                        CrawlerError::Other(format!(
                            "AI response is not valid JSON: {} (body starts: {})",
                            e,
                            snippet(&body_text)
                        ))
                    })?;

                    let usage = provider::parse_usage(self.config.provider, &json).unwrap_or_default();
                    let finish_reason = provider::parse_finish_reason(self.config.provider, &json);

                    // A successful HTTP response may still be a refusal/safety response with no
                    // content. Account for any provider-reported tokens before validating content.
                    if status.is_success() {
                        super::usage::record(category, &usage, call_start.elapsed().as_millis() as u64, false);
                    }

                    // Non-2xx with a parseable body, or a 200 body carrying a provider error.
                    if let Some(msg) = provider::extract_error(self.config.provider, &json) {
                        return Err(CrawlerError::Other(format!("AI provider error: {}", msg)));
                    }
                    if !status.is_success() {
                        return Err(CrawlerError::Other(format!(
                            "AI HTTP {}: {}",
                            code,
                            snippet(&body_text)
                        )));
                    }

                    let text = provider::parse_content(self.config.provider, &json)
                        .ok_or_else(|| CrawlerError::Other(no_content_message(self.config.provider, &json)))?;

                    let completion = AiCompletion {
                        text,
                        usage,
                        from_cache: false,
                        finish_reason,
                    };
                    self.store_cached(&cache_key, &completion);
                    return Ok(completion);
                }
                Err(e) => {
                    last_err = e.to_string();
                    // Do NOT retry on timeout: a paid completion may have been processed
                    // server-side, so retrying could double-charge. Only retry when we know
                    // the request never reached/processed (connect/build errors).
                    let retriable = e.is_connect() || e.is_request();
                    if retriable && attempt + 1 < MAX_ATTEMPTS {
                        self.backoff(attempt, None).await;
                        continue;
                    }
                    return Err(CrawlerError::Other(format!("AI request error: {}", last_err)));
                }
            }
        }

        Err(CrawlerError::Other(format!("AI request failed: {}", last_err)))
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
        let reported = cached.prompt_tokens > 0 || cached.completion_tokens > 0;
        let usage = if reported {
            Usage {
                input_tokens: Some(cached.prompt_tokens),
                output_tokens: Some(cached.completion_tokens),
                reasoning_tokens: cached.reasoning_tokens,
                cached_input_tokens: cached.cached_input_tokens,
            }
        } else {
            Usage::default()
        };
        Some(AiCompletion {
            text: cached.text,
            usage,
            from_cache: true,
            finish_reason: cached.finish_reason,
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
            reasoning_tokens: completion.usage.reasoning_tokens,
            cached_input_tokens: completion.usage.cached_input_tokens,
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

/// The error text for a response without content, quoting the model's refusal when it gave one.
fn no_content_message(provider: Provider, json: &serde_json::Value) -> String {
    match provider::parse_refusal(provider, json) {
        Some(refusal) => format!("AI response had no content (refusal: {})", snippet(&refusal)),
        None => "AI response had no content".to_string(),
    }
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
            };
            assert!(completion.was_truncated());
        }
        let completion = AiCompletion {
            text: "{}".to_string(),
            usage: Usage::default(),
            from_cache: false,
            finish_reason: Some("stop".to_string()),
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
        };
        client.store_cached("abcdef", &completion);
        let hit = client.get_cached("abcdef").expect("a cache hit");
        assert_eq!(hit.usage, usage);
        assert!(hit.from_cache);
        assert_eq!(hit.text, "OK");
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

    #[test]
    fn missing_content_error_quotes_the_refusal() {
        let refused =
            serde_json::json!({"choices": [{"message": {"content": null, "refusal": "I can't help with that."}}]});
        assert_eq!(
            no_content_message(Provider::OpenAi, &refused),
            "AI response had no content (refusal: I can't help with that.)"
        );
        let empty = serde_json::json!({"choices": [{"message": {}}]});
        assert_eq!(
            no_content_message(Provider::OpenAi, &empty),
            "AI response had no content"
        );
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
