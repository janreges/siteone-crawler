// SiteOne Crawler - AI HTTP client
// (c) Jan Reges <jan.reges@siteone.cz>
//
// A thin client over the existing `reqwest` dependency (no new LLM crate). Mirrors the
// patterns of `engine/http_client.rs`: shared client, per-request timeout, on-disk cache.
// Adds retry/backoff on 429/5xx and provider-native request shaping + response parsing.

use std::path::Path;
use std::time::Duration;

use md5::{Digest, Md5};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};

use super::config::AiConfig;
use super::provider::{self, ChatRequest, Provider, Usage};
use crate::error::{CrawlerError, CrawlerResult};

const MAX_ATTEMPTS: u32 = 3;

/// Delay before the single end-to-end retry in `complete_parsed`.
const PARSE_RETRY_DELAY_SECS: u64 = 5;

/// Result of a successful completion. `text` is the raw model output (callers run it
/// through `normalize::*` before parsing).
pub struct AiCompletion {
    pub text: String,
    pub usage: Usage,
    pub from_cache: bool,
}

/// On-disk cache record (content-addressed; never contains the API key).
#[derive(Serialize, Deserialize)]
struct CachedCompletion {
    text: String,
    prompt_tokens: u32,
    completion_tokens: u32,
}

pub struct AiClient {
    client: reqwest::Client,
    config: AiConfig,
}

impl AiClient {
    pub fn new(config: AiConfig) -> Self {
        let client = reqwest::Client::builder()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client, config }
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

    /// `complete` + the caller's `parse`, retried ONCE after a short delay on ANY failure —
    /// not only transport/HTTP errors (already retried inside `complete`), but also a response
    /// the model returns malformed so that `parse` rejects it (common under provider overload).
    /// LLMs are non-deterministic, so a second attempt usually succeeds. Returns the parsed value
    /// plus the (successful) completion so callers can still read token usage.
    pub async fn complete_parsed<T>(
        &self,
        req: &ChatRequest,
        category: &str,
        parse: impl Fn(&str) -> Result<T, String>,
    ) -> CrawlerResult<(T, AiCompletion)> {
        let mut last_err: Option<CrawlerError> = None;
        // attempt 0 = first try; attempt 1 = the one retry (after a 5s pause).
        for attempt in 0..2u32 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_secs(PARSE_RETRY_DELAY_SECS)).await;
            }
            match self.complete(req, category).await {
                Ok(completion) => match parse(&completion.text) {
                    Ok(value) => return Ok((value, completion)),
                    Err(e) => last_err = Some(CrawlerError::Other(format!("invalid response: {}", e))),
                },
                Err(e) => last_err = Some(e),
            }
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
        if let Some(hit) = self.get_cached(&cache_key) {
            let had_tokens = hit.usage.prompt_tokens > 0 || hit.usage.completion_tokens > 0;
            super::usage::record(
                category,
                hit.usage.prompt_tokens as u64,
                hit.usage.completion_tokens as u64,
                0,
                true,
                had_tokens,
            );
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
                        .ok_or_else(|| CrawlerError::Other("AI response had no content".to_string()))?;
                    let parsed_usage = provider::parse_usage(self.config.provider, &json);
                    let tokens_reported = parsed_usage.is_some();
                    let usage = parsed_usage.unwrap_or_default();

                    let completion = AiCompletion {
                        text,
                        usage,
                        from_cache: false,
                    };
                    super::usage::record(
                        category,
                        usage.prompt_tokens as u64,
                        usage.completion_tokens as u64,
                        call_start.elapsed().as_millis() as u64,
                        false,
                        tokens_reported,
                    );
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
        tokio::time::sleep(Duration::from_secs(secs.min(30))).await;
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
        Some(AiCompletion {
            text: cached.text,
            usage: Usage {
                prompt_tokens: cached.prompt_tokens,
                completion_tokens: cached.completion_tokens,
            },
            from_cache: true,
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
            prompt_tokens: completion.usage.prompt_tokens,
            completion_tokens: completion.usage.completion_tokens,
        };
        if let Ok(json) = serde_json::to_string(&cached) {
            let _ = std::fs::write(&path, json);
        }
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
