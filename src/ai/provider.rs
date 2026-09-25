// SiteOne Crawler - AI provider request/response shaping
// (c) Jan Reges <jan.reges@siteone.cz>
//
// A neutral `ChatRequest` is shaped into each provider's native HTTP request, and each
// provider's native response is parsed back into plain text + token usage. Provider
// quirks (max_tokens vs max_completion_tokens, Anthropic headers, Gemini request shape,
// MiniMax base_resp errors) live here so the rest of the AI subsystem stays provider-agnostic.

use serde_json::{Value, json};

/// Supported LLM provider modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    /// api.openai.com (applies OpenAI-specific reasoning-model quirks)
    OpenAi,
    /// Any OpenAI-compatible endpoint (vLLM, LiteLLM, MiniMax, LocalAI, Ollama, ...)
    OpenAiCompatible,
    Anthropic,
    Gemini,
}

impl Provider {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "openai" => Some(Provider::OpenAi),
            "openai-compatible" | "openai_compatible" | "compatible" => Some(Provider::OpenAiCompatible),
            "anthropic" | "claude" => Some(Provider::Anthropic),
            "gemini" | "google" => Some(Provider::Gemini),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::OpenAi => "openai",
            Provider::OpenAiCompatible => "openai-compatible",
            Provider::Anthropic => "anthropic",
            Provider::Gemini => "gemini",
        }
    }

    /// Default base endpoint when `--ai-endpoint` is not provided. None for
    /// openai-compatible (endpoint is required there).
    pub fn default_endpoint(&self) -> Option<&'static str> {
        match self {
            Provider::OpenAi => Some("https://api.openai.com/v1"),
            Provider::OpenAiCompatible => None,
            Provider::Anthropic => Some("https://api.anthropic.com"),
            Provider::Gemini => Some("https://generativelanguage.googleapis.com/v1beta"),
        }
    }

    /// Conventional environment variable read by default for this provider.
    pub fn default_key_env(&self) -> &'static str {
        match self {
            Provider::OpenAi | Provider::OpenAiCompatible => "OPENAI_API_KEY",
            Provider::Anthropic => "ANTHROPIC_API_KEY",
            Provider::Gemini => "GEMINI_API_KEY",
        }
    }
}

/// One chat turn.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }
}

/// A provider-neutral chat request.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub system: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    pub temperature: f32,
    /// Hint that a JSON answer is expected (used for Gemini responseMimeType).
    pub json_mode: bool,
    /// Optional JSON Schema (a schema object) the model output must match. When set AND the
    /// provider supports native structured output, shaping emits `response_format:json_schema`
    /// (OpenAI/-compatible, plus a `guided_json` mirror for vLLM/SGLang) or Gemini
    /// `responseJsonSchema`. When None, or for providers without native support, callers rely on
    /// the prompt-embedded schema + robust `normalize` parsing instead.
    pub json_schema: Option<Value>,
    /// Human-friendly name for the enforced schema (OpenAI `json_schema.name`).
    pub schema_name: Option<String>,
}

impl ChatRequest {
    /// Attach an enforced JSON schema (fluent helper for the report/extract action).
    pub fn with_schema(mut self, schema: Value, name: impl Into<String>) -> Self {
        self.json_schema = Some(schema);
        self.schema_name = Some(name.into());
        self
    }
}

/// A fully shaped HTTP request ready to send.
pub struct ShapedRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

/// True for OpenAI reasoning models that reject `max_tokens` and a non-default `temperature`.
/// Note: gpt-4o is intentionally NOT here — it accepts both `max_tokens` and `temperature`.
pub fn openai_model_needs_completion_tokens(model: &str) -> bool {
    let m = model.to_lowercase();
    m.starts_with("o1") || m.starts_with("o3") || m.starts_with("o4") || m.contains("gpt-5")
}

/// Conservative capability check used by `--ai-schema-enforce=auto`. Unknown hosted OpenAI
/// models stay on the embedded-contract path; explicit `on` still allows users to opt in, and the
/// client has a one-time pure-prompt fallback for APIs that reject output-format controls.
pub fn supports_native_schema_auto(provider: Provider, model: &str) -> bool {
    match provider {
        Provider::Gemini => {
            let model = model.to_ascii_lowercase();
            ["gemini-2.0", "gemini-2.5", "gemini-3"]
                .iter()
                .any(|prefix| model.starts_with(prefix))
        }
        Provider::OpenAi => {
            let model = model.to_ascii_lowercase();
            // Some hosted `-pro` variants are Responses-only or explicitly do not support
            // Structured Outputs. Keep auto mode conservative; explicit `on` still has the
            // client's one-time prompt-mode fallback.
            if model.contains("-pro") || model == "gpt-4o-2024-05-13" {
                return false;
            }
            ["gpt-4o", "gpt-4.1", "gpt-5", "o3", "o4"]
                .iter()
                .any(|prefix| model.starts_with(prefix))
        }
        Provider::OpenAiCompatible | Provider::Anthropic => false,
    }
}

/// Recursively merge `overlay` into `base` (used for `--ai-extra-body`).
pub fn deep_merge(base: &mut Value, overlay: &Value) {
    match (base, overlay) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                deep_merge(b.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (b, o) => *b = o.clone(),
    }
}

fn trim_endpoint(endpoint: &str) -> &str {
    endpoint.trim_end_matches('/')
}

/// Shape a neutral request into a provider-native HTTP request.
///
/// `force_completion_tokens` is the `--ai-use-max-completion-tokens` override.
/// `extra_body` is deep-merged into the body last so the user can override anything.
pub fn shape_request(
    provider: Provider,
    model: &str,
    endpoint: &str,
    api_key: Option<&str>,
    req: &ChatRequest,
    force_completion_tokens: bool,
    extra_body: Option<&Value>,
) -> ShapedRequest {
    let mut shaped = match provider {
        Provider::OpenAi | Provider::OpenAiCompatible => {
            shape_openai(provider, model, endpoint, api_key, req, force_completion_tokens)
        }
        Provider::Anthropic => shape_anthropic(model, endpoint, api_key, req),
        Provider::Gemini => shape_gemini(model, endpoint, api_key, req),
    };

    if let Some(extra) = extra_body {
        deep_merge(&mut shaped.body, extra);
    }
    shaped
}

fn shape_openai(
    provider: Provider,
    model: &str,
    endpoint: &str,
    api_key: Option<&str>,
    req: &ChatRequest,
    force_completion_tokens: bool,
) -> ShapedRequest {
    let url = format!("{}/chat/completions", trim_endpoint(endpoint));

    let mut messages: Vec<Value> = Vec::new();
    if let Some(ref sys) = req.system {
        messages.push(json!({"role": "system", "content": sys}));
    }
    for m in &req.messages {
        messages.push(json!({"role": m.role, "content": m.content}));
    }

    let mut body = json!({
        "model": model,
        "messages": messages,
    });

    let use_completion =
        force_completion_tokens || (provider == Provider::OpenAi && openai_model_needs_completion_tokens(model));
    if use_completion {
        body["max_completion_tokens"] = json!(req.max_tokens);
    } else {
        body["max_tokens"] = json!(req.max_tokens);
    }

    // OpenAI reasoning models reject a non-default temperature; omit it for them.
    let omit_temp = provider == Provider::OpenAi && openai_model_needs_completion_tokens(model);
    if !omit_temp {
        body["temperature"] = json!(req.temperature);
    }

    // Ask OpenAI-compatible endpoints for a JSON object when we expect JSON. OpenAI, vLLM and
    // MiniMax all honor this; `--ai-extra-body` is deep-merged afterwards so a user can override
    // (e.g. to `{"type":"text"}`) for an endpoint that rejects it.
    if req.json_mode {
        body["response_format"] = json!({"type": "json_object"});
    }

    // Native structured output: hosted OpenAI receives only its documented response_format.
    // Compatible endpoints additionally receive the vLLM/SGLang `guided_json` extension.
    if let Some(schema) = &req.json_schema {
        let name = req.schema_name.clone().unwrap_or_else(|| "ai_response".to_string());
        let schema = if provider == Provider::OpenAi {
            openai_strict_schema(schema)
        } else {
            schema.clone()
        };
        body["response_format"] = json!({
            "type": "json_schema",
            "json_schema": { "name": name, "schema": schema, "strict": true }
        });
        if provider == Provider::OpenAiCompatible {
            body["guided_json"] = schema.clone();
        }
    }

    let mut headers = vec![("content-type".to_string(), "application/json".to_string())];
    if let Some(key) = api_key {
        headers.push(("authorization".to_string(), format!("Bearer {}", key)));
    }

    ShapedRequest { url, headers, body }
}

fn shape_anthropic(model: &str, endpoint: &str, api_key: Option<&str>, req: &ChatRequest) -> ShapedRequest {
    let url = format!("{}/v1/messages", trim_endpoint(endpoint));

    let messages: Vec<Value> = req
        .messages
        .iter()
        .map(|m| json!({"role": m.role, "content": m.content}))
        .collect();

    let mut body = json!({
        "model": model,
        "max_tokens": req.max_tokens,
        "temperature": req.temperature,
        "messages": messages,
    });
    if let Some(ref sys) = req.system {
        body["system"] = json!(sys);
    }

    let mut headers = vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("anthropic-version".to_string(), "2023-06-01".to_string()),
    ];
    if let Some(key) = api_key {
        headers.push(("x-api-key".to_string(), key.to_string()));
    }

    ShapedRequest { url, headers, body }
}

fn shape_gemini(model: &str, endpoint: &str, api_key: Option<&str>, req: &ChatRequest) -> ShapedRequest {
    // Key goes in a header, never in the URL (keeps it out of logs).
    let url = format!("{}/models/{}:generateContent", trim_endpoint(endpoint), model);

    let user_text = req
        .messages
        .iter()
        .map(|m| m.content.clone())
        .collect::<Vec<_>>()
        .join("\n\n");

    let mut generation_config = json!({
        "maxOutputTokens": req.max_tokens,
        "temperature": req.temperature,
    });
    if req.json_mode {
        generation_config["responseMimeType"] = json!("application/json");
    }
    if let Some(schema) = &req.json_schema {
        generation_config["responseMimeType"] = json!("application/json");
        // `responseJsonSchema` accepts the JSON Schema dialect generated by the report engine.
        // The older `responseSchema` field expects Google's restricted OpenAPI `Schema` shape.
        generation_config["responseJsonSchema"] = schema.clone();
    }

    let mut body = json!({
        "contents": [ { "role": "user", "parts": [ { "text": user_text } ] } ],
        "generationConfig": generation_config,
    });
    if let Some(ref sys) = req.system {
        body["systemInstruction"] = json!({ "parts": [ { "text": sys } ] });
    }

    let mut headers = vec![("content-type".to_string(), "application/json".to_string())];
    if let Some(key) = api_key {
        headers.push(("x-goog-api-key".to_string(), key.to_string()));
    }

    ShapedRequest { url, headers, body }
}

/// The GET request that lists the models an endpoint offers (`body` is unused). Anthropic and
/// Gemini page their lists; one page of the most they return at once holds every model.
pub fn models_request(provider: Provider, endpoint: &str, api_key: Option<&str>) -> ShapedRequest {
    let endpoint = trim_endpoint(endpoint);
    let mut headers = Vec::new();
    let url = match provider {
        Provider::OpenAi | Provider::OpenAiCompatible => {
            if let Some(key) = api_key {
                headers.push(("authorization".to_string(), format!("Bearer {}", key)));
            }
            format!("{}/models", endpoint)
        }
        Provider::Anthropic => {
            headers.push(("anthropic-version".to_string(), "2023-06-01".to_string()));
            if let Some(key) = api_key {
                headers.push(("x-api-key".to_string(), key.to_string()));
            }
            format!("{}/v1/models?limit=1000", endpoint)
        }
        Provider::Gemini => {
            // Key goes in a header, never in the URL (keeps it out of logs).
            if let Some(key) = api_key {
                headers.push(("x-goog-api-key".to_string(), key.to_string()));
            }
            format!("{}/models?pageSize=1000", endpoint)
        }
    };
    ShapedRequest {
        url,
        headers,
        body: Value::Null,
    }
}

/// Hosted OpenAI strict mode supports only a subset of JSON Schema string formats. Keep the
/// crawler's semantic URL validation, but remove the unsupported `uri` annotation before sending
/// the schema so a valid custom URL field does not force a prompt-mode downgrade.
fn openai_strict_schema(schema: &Value) -> Value {
    let mut schema = schema.clone();
    fn visit(value: &mut Value) {
        match value {
            Value::Object(object) => {
                if object.get("format").and_then(Value::as_str) == Some("uri") {
                    object.remove("format");
                }
                for child in object.values_mut() {
                    visit(child);
                }
            }
            Value::Array(items) => items.iter_mut().for_each(visit),
            _ => {}
        }
    }
    visit(&mut schema);
    schema
}

/// Token usage reported by the provider for one response. Every field is None when the response
/// did not say — "unknown" is never turned into 0 (also not in the AI cache, which stores it as is).
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Usage {
    /// All prompt tokens the provider processed (for Anthropic: input + cache creation + cache read).
    pub input_tokens: Option<u64>,
    /// Generated tokens, INCLUDING reasoning/thinking tokens.
    pub output_tokens: Option<u64>,
    /// The part of `output_tokens` that was reasoning/thinking, when the provider reports it.
    pub reasoning_tokens: Option<u64>,
    /// The part of `input_tokens` served from the provider's prompt cache.
    pub cached_input_tokens: Option<u64>,
}

impl Usage {
    pub fn input(&self) -> u64 {
        self.input_tokens.unwrap_or(0)
    }

    pub fn output(&self) -> u64 {
        self.output_tokens.unwrap_or(0)
    }

    /// True when the provider reported at least the input or the output token count.
    pub fn has_tokens(&self) -> bool {
        self.input_tokens.is_some() || self.output_tokens.is_some()
    }
}

/// Largest token count accepted from a response (2^53, the largest integer a JSON number is
/// guaranteed to carry exactly).
const MAX_COUNT: u64 = 1 << 53;

/// A token count at `key` of `v`: a JSON integer, or a finite non-negative float with no
/// fractional part, up to 2^53. Anything else (negative, fractional, huge, string, ...) is None.
fn count(v: &Value, key: &str) -> Option<u64> {
    let n = v.get(key)?;
    let count = match n.as_u64() {
        Some(i) => i,
        None => {
            let f = n.as_f64()?;
            if !f.is_finite() || f < 0.0 || f.fract() != 0.0 {
                return None;
            }
            // Saturates for huge values, which the range check below then rejects.
            f as u64
        }
    };
    (count <= MAX_COUNT).then_some(count)
}

/// Extract the assistant's text content from a provider-native response. OpenAI-compatible
/// content may be a string or an array of parts (the text parts are joined); Gemini thought parts
/// are reasoning, not content.
pub fn parse_content(provider: Provider, resp: &Value) -> Option<String> {
    match provider {
        Provider::OpenAi | Provider::OpenAiCompatible => {
            match resp.get("choices")?.get(0)?.get("message")?.get("content")? {
                Value::String(text) => Some(text.clone()),
                Value::Array(parts) => {
                    let texts: Vec<&str> = parts
                        .iter()
                        .filter(|p| matches!(p.get("type").and_then(Value::as_str), None | Some("text")))
                        .filter_map(|p| p.get("text").and_then(Value::as_str))
                        .collect();
                    (!texts.is_empty()).then(|| texts.concat())
                }
                _ => None,
            }
        }
        Provider::Anthropic => {
            let blocks = resp.get("content")?.as_array()?;
            let text: String = blocks
                .iter()
                .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("");
            Some(text)
        }
        Provider::Gemini => {
            let text: String = gemini_parts(resp)?
                .iter()
                .filter(|p| !is_gemini_thought(p))
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("");
            Some(text)
        }
    }
}

fn gemini_parts(resp: &Value) -> Option<&Vec<Value>> {
    resp.get("candidates")?.get(0)?.get("content")?.get("parts")?.as_array()
}

fn is_gemini_thought(part: &Value) -> bool {
    part.get("thought").and_then(Value::as_bool) == Some(true)
}

/// The refusal an OpenAI(-compatible) model returned instead of content, if any.
pub fn parse_refusal(provider: Provider, resp: &Value) -> Option<String> {
    match provider {
        Provider::OpenAi | Provider::OpenAiCompatible => resp
            .get("choices")?
            .get(0)?
            .get("message")?
            .get("refusal")?
            .as_str()
            .map(str::trim)
            .filter(|refusal| !refusal.is_empty())
            .map(str::to_string),
        Provider::Anthropic | Provider::Gemini => None,
    }
}

/// Character count of the reasoning/thinking text a response carries, None when it has none:
/// `message.reasoning` (vLLM >= 0.10), `message.reasoning_content` (DeepSeek, older vLLM, SGLang)
/// or inline `<think>...</think>` in the content (MiniMax, Qwen without a reasoning parser);
/// Anthropic `thinking` blocks; Gemini thought parts. Only the count is used (the telemetry shows
/// that reasoning happened even when the provider does not count its tokens); the text is never
/// kept.
pub fn parse_reasoning(provider: Provider, resp: &Value) -> Option<u64> {
    let chars = match provider {
        Provider::OpenAi | Provider::OpenAiCompatible => {
            let message = resp.get("choices")?.get(0)?.get("message")?;
            ["reasoning", "reasoning_content"]
                .iter()
                .filter_map(|key| message.get(key).and_then(Value::as_str))
                .map(text_chars)
                .find(|&chars| chars > 0)
                .or_else(|| parse_content(provider, resp).map(|content| inline_think_chars(&content)))
                .unwrap_or(0)
        }
        Provider::Anthropic => resp
            .get("content")?
            .as_array()?
            .iter()
            .filter(|b| b.get("type").and_then(Value::as_str) == Some("thinking"))
            .filter_map(|b| b.get("thinking").and_then(Value::as_str))
            .map(text_chars)
            .sum(),
        Provider::Gemini => gemini_parts(resp)?
            .iter()
            .filter(|p| is_gemini_thought(p))
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .map(text_chars)
            .sum(),
    };
    (chars > 0).then_some(chars)
}

fn text_chars(text: &str) -> u64 {
    text.trim().chars().count() as u64
}

/// Characters inside `<think>...</think>` blocks, including an unterminated trailing `<think>`
/// (the same blocks `normalize::strip_think` removes).
fn inline_think_chars(content: &str) -> u64 {
    const OPEN: &str = "<think>";
    const CLOSE: &str = "</think>";
    let mut chars = 0;
    let mut rest = content;
    while let Some(start) = rest.find(OPEN) {
        let inner = &rest[start + OPEN.len()..];
        match inner.find(CLOSE) {
            Some(end) => {
                chars += text_chars(&inner[..end]);
                rest = &inner[end + CLOSE.len()..];
            }
            None => {
                chars += text_chars(inner);
                break;
            }
        }
    }
    chars
}

/// Extract why generation stopped. Callers use length/token-limit reasons as a hard parse failure:
/// closing brackets can repair syntax, but cannot recover output the provider never returned.
pub fn parse_finish_reason(provider: Provider, resp: &Value) -> Option<String> {
    match provider {
        Provider::OpenAi | Provider::OpenAiCompatible => resp
            .get("choices")?
            .get(0)?
            .get("finish_reason")?
            .as_str()
            .map(str::to_string),
        Provider::Anthropic => resp.get("stop_reason")?.as_str().map(str::to_string),
        Provider::Gemini => resp
            .get("candidates")?
            .get(0)?
            .get("finishReason")?
            .as_str()
            .map(str::to_string),
    }
}

/// Extract token usage from a response, trying every known runtime format in turn (regardless
/// of the configured provider). `output_tokens` always represents OUTPUT INCLUDING any
/// reasoning/thinking tokens. Returns None when no known format is present — the caller then
/// keeps working but simply does not count tokens for that call (never panics).
///
/// Supported shapes:
/// - OpenAI / OpenAI-compatible (vLLM, llama.cpp server, LM Studio, SGLang, MiniMax, DeepSeek,
///   Ollama `/v1`, Gemini OpenAI-compat): `usage.prompt_tokens` / `usage.completion_tokens`,
///   reasoning in `completion_tokens_details.reasoning_tokens`, cached input in
///   `prompt_tokens_details.cached_tokens` (DeepSeek: `prompt_cache_hit_tokens`).
/// - Anthropic and the OpenAI Responses API: `usage.input_tokens` / `usage.output_tokens`; the
///   Anthropic prompt-cache parts (`cache_creation_input_tokens`, `cache_read_input_tokens`) are
///   added to the input; reasoning in `output_tokens_details.thinking_tokens` or
///   `…reasoning_tokens`; cached input in `cache_read_input_tokens` or
///   `input_tokens_details.cached_tokens`.
/// - Gemini native: `usageMetadata.promptTokenCount` + output as `totalTokenCount - prompt`
///   (correct on both Gemini API and Vertex), falling back to `candidatesTokenCount +
///   thoughtsTokenCount`; reasoning = `thoughtsTokenCount`, cached = `cachedContentTokenCount`.
/// - Ollama native (`/api/chat`, `/api/generate`): `prompt_eval_count` / `eval_count`.
/// - llama.cpp native (`/completion`): `tokens_evaluated` / `tokens_predicted`.
///
/// A reasoning count larger than the output count is inconsistent and reported as unknown.
pub fn parse_usage(_provider: Provider, resp: &Value) -> Option<Usage> {
    let usage = parse_usage_shape(resp)?;
    if !usage.has_tokens() {
        return None;
    }
    let reasoning_fits = match (usage.reasoning_tokens, usage.output_tokens) {
        (Some(reasoning), Some(output)) => reasoning <= output,
        _ => true,
    };
    Some(Usage {
        reasoning_tokens: usage.reasoning_tokens.filter(|_| reasoning_fits),
        ..usage
    })
}

fn parse_usage_shape(resp: &Value) -> Option<Usage> {
    let nested = |v: &Value, object: &str, key: &str| v.get(object).and_then(|o| count(o, key));

    if let Some(u) = resp.get("usage") {
        // OpenAI-compatible.
        let (p, c) = (count(u, "prompt_tokens"), count(u, "completion_tokens"));
        if p.is_some() || c.is_some() {
            return Some(Usage {
                input_tokens: p,
                output_tokens: c,
                reasoning_tokens: nested(u, "completion_tokens_details", "reasoning_tokens"),
                cached_input_tokens: nested(u, "prompt_tokens_details", "cached_tokens")
                    .or_else(|| count(u, "prompt_cache_hit_tokens")),
            });
        }
        // Anthropic, OpenAI Responses.
        let (i, o) = (count(u, "input_tokens"), count(u, "output_tokens"));
        if i.is_some() || o.is_some() {
            let cache_read = count(u, "cache_read_input_tokens");
            let input = i.map(|i| i + count(u, "cache_creation_input_tokens").unwrap_or(0) + cache_read.unwrap_or(0));
            return Some(Usage {
                input_tokens: input,
                output_tokens: o,
                reasoning_tokens: nested(u, "output_tokens_details", "thinking_tokens")
                    .or_else(|| nested(u, "output_tokens_details", "reasoning_tokens")),
                cached_input_tokens: cache_read.or_else(|| nested(u, "input_tokens_details", "cached_tokens")),
            });
        }
    }

    // Gemini native.
    if let Some(m) = resp.get("usageMetadata") {
        let prompt = count(m, "promptTokenCount");
        let total = count(m, "totalTokenCount");
        let cand = count(m, "candidatesTokenCount");
        let thoughts = count(m, "thoughtsTokenCount");
        if prompt.is_some() || cand.is_some() || total.is_some() {
            let output = match (total, prompt) {
                // Captures candidates + thoughts on both API and Vertex.
                (Some(t), Some(p)) if t >= p => Some(t - p),
                _ if cand.is_some() || thoughts.is_some() => Some(cand.unwrap_or(0) + thoughts.unwrap_or(0)),
                _ => None,
            };
            return Some(Usage {
                input_tokens: prompt,
                output_tokens: output,
                reasoning_tokens: thoughts,
                cached_input_tokens: count(m, "cachedContentTokenCount"),
            });
        }
    }

    // Ollama native.
    let (pe, ec) = (count(resp, "prompt_eval_count"), count(resp, "eval_count"));
    if pe.is_some() || ec.is_some() {
        return Some(Usage {
            input_tokens: pe,
            output_tokens: ec,
            ..Usage::default()
        });
    }

    // llama.cpp native /completion.
    let (te, tp) = (count(resp, "tokens_evaluated"), count(resp, "tokens_predicted"));
    if te.is_some() || tp.is_some() {
        return Some(Usage {
            input_tokens: te,
            output_tokens: tp,
            ..Usage::default()
        });
    }

    None
}

/// Detect a provider-level error embedded in an otherwise-200 response body.
pub fn extract_error(provider: Provider, resp: &Value) -> Option<String> {
    // Generic { "error": { "message": ... } } shape used by OpenAI/Anthropic/Gemini.
    if let Some(msg) = resp
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
    {
        return Some(msg.to_string());
    }
    if let Some(err) = resp.get("error").and_then(|e| e.as_str()) {
        return Some(err.to_string());
    }
    // MiniMax (openai-compatible) wraps status in base_resp.
    if provider == Provider::OpenAiCompatible
        && let Some(base) = resp.get("base_resp")
    {
        let code = base.get("status_code").and_then(|c| c.as_i64()).unwrap_or(0);
        if code != 0 {
            let msg = base
                .get("status_msg")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            return Some(format!("provider error {}: {}", code, msg));
        }
    }
    None
}

/// One model an endpoint offers, as `--ai-list-models` prints it. A size the endpoint did not
/// state is None, never 0.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub display_name: Option<String>,
    /// Tokens the model accepts.
    pub context_window: Option<u64>,
    pub max_output_tokens: Option<u64>,
}

/// The models of a model-list response, sorted by id: `data[]` (OpenAI, every OpenAI-compatible
/// runtime, Anthropic) or `models[]` (Gemini). Entries without a usable id and models that list
/// their generation methods without `generateContent` are skipped, an odd size is left out, a
/// repeated id keeps its first entry. None when the response holds no model list at all.
pub fn parse_model_list(resp: &Value) -> Option<Vec<ModelInfo>> {
    let entries = resp
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| resp.get("models").and_then(Value::as_array))?;
    let mut models: Vec<ModelInfo> = entries.iter().filter_map(model_info).collect();
    models.sort_by(|a, b| a.id.cmp(&b.id));
    models.dedup_by(|later, earlier| later.id == earlier.id);
    Some(models)
}

fn model_info(entry: &Value) -> Option<ModelInfo> {
    if let Some(methods) = entry.get("supportedGenerationMethods").and_then(Value::as_array)
        && !methods.iter().any(|method| method.as_str() == Some("generateContent"))
    {
        return None;
    }
    let text = |key: &str| {
        entry
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
    };
    // Gemini names its models `models/<id>`.
    let id = text("id").or_else(|| {
        text("name")
            .map(|name| name.strip_prefix("models/").unwrap_or(name))
            .filter(|id| !id.is_empty())
    })?;
    let size = |keys: &[&str]| keys.iter().find_map(|key| count(entry, key)).filter(|&n| n > 0);
    Some(ModelInfo {
        id: id.to_string(),
        display_name: text("display_name").or_else(|| text("displayName")).map(str::to_string),
        context_window: size(&[
            "max_model_len",
            "context_length",
            "context_window",
            "max_input_tokens",
            "inputTokenLimit",
        ]),
        max_output_tokens: size(&["max_tokens", "outputTokenLimit"]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_req() -> ChatRequest {
        ChatRequest {
            system: Some("You are a tester.".to_string()),
            messages: vec![ChatMessage::user("hello")],
            max_tokens: 100,
            temperature: 0.0,
            json_mode: false,
            json_schema: None,
            schema_name: None,
        }
    }

    #[test]
    fn provider_parse_roundtrip() {
        assert_eq!(Provider::parse("openai"), Some(Provider::OpenAi));
        assert_eq!(Provider::parse("openai-compatible"), Some(Provider::OpenAiCompatible));
        assert_eq!(Provider::parse("anthropic"), Some(Provider::Anthropic));
        assert_eq!(Provider::parse("gemini"), Some(Provider::Gemini));
        assert_eq!(Provider::parse("nope"), None);
    }

    #[test]
    fn openai_uses_max_tokens_for_normal_model() {
        let r = shape_request(
            Provider::OpenAiCompatible,
            "Qwen/Qwen3.6-27B-FP8",
            "http://h/v1",
            Some("k"),
            &base_req(),
            false,
            None,
        );
        assert_eq!(r.url, "http://h/v1/chat/completions");
        assert!(r.body.get("max_tokens").is_some());
        assert!(r.body.get("max_completion_tokens").is_none());
        assert!(r.body.get("temperature").is_some());
        assert!(r.headers.iter().any(|(k, v)| k == "authorization" && v == "Bearer k"));
    }

    #[test]
    fn openai_reasoning_model_uses_completion_tokens_and_omits_temp() {
        let r = shape_request(
            Provider::OpenAi,
            "gpt-5-mini",
            "https://api.openai.com/v1",
            Some("k"),
            &base_req(),
            false,
            None,
        );
        assert!(r.body.get("max_completion_tokens").is_some());
        assert!(r.body.get("max_tokens").is_none());
        assert!(r.body.get("temperature").is_none());
    }

    #[test]
    fn force_completion_tokens_override() {
        let r = shape_request(
            Provider::OpenAiCompatible,
            "some-model",
            "http://h/v1",
            None,
            &base_req(),
            true,
            None,
        );
        assert!(r.body.get("max_completion_tokens").is_some());
        assert!(r.body.get("max_tokens").is_none());
    }

    #[test]
    fn anthropic_shape() {
        let r = shape_request(
            Provider::Anthropic,
            "claude-x",
            "https://api.anthropic.com",
            Some("k"),
            &base_req(),
            false,
            None,
        );
        assert_eq!(r.url, "https://api.anthropic.com/v1/messages");
        assert_eq!(r.body["system"], json!("You are a tester."));
        assert!(r.body.get("max_tokens").is_some());
        assert!(r.headers.iter().any(|(k, v)| k == "x-api-key" && v == "k"));
        assert!(r.headers.iter().any(|(k, _)| k == "anthropic-version"));
    }

    #[test]
    fn gemini_shape_key_in_header_not_url() {
        let mut req = base_req();
        req.json_mode = true;
        let r = shape_request(
            Provider::Gemini,
            "gemini-2.5",
            "https://g/v1beta",
            Some("secret"),
            &req,
            false,
            None,
        );
        assert_eq!(r.url, "https://g/v1beta/models/gemini-2.5:generateContent");
        assert!(!r.url.contains("secret"));
        assert!(r.headers.iter().any(|(k, v)| k == "x-goog-api-key" && v == "secret"));
        assert_eq!(
            r.body["generationConfig"]["responseMimeType"],
            json!("application/json")
        );
    }

    #[test]
    fn openai_json_mode_sets_response_format() {
        let mut req = base_req();
        req.json_mode = true;
        let r = shape_request(Provider::OpenAiCompatible, "m", "http://h/v1", None, &req, false, None);
        assert_eq!(r.body["response_format"], json!({"type": "json_object"}));
    }

    #[test]
    fn openai_json_schema_sets_response_format_and_guided_json() {
        let mut req = base_req();
        req.json_mode = true;
        req.json_schema = Some(json!({"type":"object","properties":{"a":{"type":"string"}},"required":["a"]}));
        req.schema_name = Some("ai_extract".into());
        let r = shape_request(Provider::OpenAiCompatible, "m", "http://h/v1", None, &req, false, None);
        assert_eq!(r.body["response_format"]["type"], "json_schema");
        assert_eq!(r.body["response_format"]["json_schema"]["name"], "ai_extract");
        assert!(r.body["response_format"]["json_schema"]["schema"].is_object());
        // vLLM/SGLang-friendly mirror.
        assert!(r.body["guided_json"].is_object());
    }

    #[test]
    fn hosted_openai_schema_never_sends_compatible_guided_json_extension() {
        let mut req = base_req();
        req.json_mode = true;
        req.json_schema = Some(json!({
            "type":"object",
            "properties":{
                "a":{"type":"string"},
                "url":{"type":"string","format":"uri"}
            },
            "required":["a", "url"],
            "additionalProperties":false
        }));
        req.schema_name = Some("ai_extract".into());
        let r = shape_request(
            Provider::OpenAi,
            "gpt-4.1-mini",
            "https://api.openai.com/v1",
            Some("k"),
            &req,
            false,
            None,
        );
        assert_eq!(r.body["response_format"]["type"], "json_schema");
        assert!(r.body.get("guided_json").is_none());
        assert!(
            r.body["response_format"]["json_schema"]["schema"]["properties"]["url"]
                .get("format")
                .is_none()
        );
    }

    #[test]
    fn gemini_json_schema_sets_response_json_schema() {
        let mut req = base_req();
        req.json_mode = true;
        req.json_schema = Some(json!({"type":"object","properties":{"a":{"type":"string"}}}));
        let r = shape_request(Provider::Gemini, "m", "http://x", None, &req, false, None);
        assert_eq!(
            r.body["generationConfig"]["responseMimeType"],
            json!("application/json")
        );
        assert!(r.body["generationConfig"]["responseJsonSchema"].is_object());
        assert!(r.body["generationConfig"].get("responseSchema").is_none());
    }

    #[test]
    fn openai_non_json_mode_omits_response_format() {
        let r = shape_request(
            Provider::OpenAiCompatible,
            "m",
            "http://h/v1",
            None,
            &base_req(),
            false,
            None,
        );
        assert!(r.body.get("response_format").is_none());
    }

    #[test]
    fn extra_body_can_override_response_format() {
        let mut req = base_req();
        req.json_mode = true;
        let extra = json!({"response_format": {"type": "text"}});
        let r = shape_request(
            Provider::OpenAiCompatible,
            "m",
            "http://h/v1",
            None,
            &req,
            false,
            Some(&extra),
        );
        assert_eq!(r.body["response_format"], json!({"type": "text"}));
    }

    #[test]
    fn extra_body_deep_merges_and_overrides() {
        let extra = json!({"chat_template_kwargs": {"enable_thinking": false}, "temperature": 0.7});
        let r = shape_request(
            Provider::OpenAiCompatible,
            "m",
            "http://h/v1",
            None,
            &base_req(),
            false,
            Some(&extra),
        );
        assert_eq!(r.body["chat_template_kwargs"]["enable_thinking"], json!(false));
        assert_eq!(r.body["temperature"], json!(0.7)); // overrode our 0.0
    }

    #[test]
    fn parse_openai_content_and_usage() {
        let resp = json!({
            "choices": [ { "message": { "content": "hi there" } } ],
            "usage": { "prompt_tokens": 10, "completion_tokens": 3 }
        });
        assert_eq!(
            parse_content(Provider::OpenAiCompatible, &resp).as_deref(),
            Some("hi there")
        );
        let u = parse_usage(Provider::OpenAiCompatible, &resp).unwrap();
        assert_eq!(u, usage(Some(10), Some(3), None, None));
    }

    #[test]
    fn parses_provider_finish_reasons() {
        assert_eq!(
            parse_finish_reason(Provider::OpenAi, &json!({"choices":[{"finish_reason":"length"}]})),
            Some("length".to_string())
        );
        assert_eq!(
            parse_finish_reason(Provider::Anthropic, &json!({"stop_reason":"max_tokens"})),
            Some("max_tokens".to_string())
        );
        assert_eq!(
            parse_finish_reason(Provider::Gemini, &json!({"candidates":[{"finishReason":"MAX_TOKENS"}]})),
            Some("MAX_TOKENS".to_string())
        );
    }

    #[test]
    fn auto_schema_capability_is_model_and_provider_aware() {
        assert!(supports_native_schema_auto(Provider::OpenAi, "gpt-4.1-mini"));
        assert!(!supports_native_schema_auto(Provider::OpenAi, "gpt-3.5-turbo"));
        assert!(!supports_native_schema_auto(Provider::OpenAi, "gpt-5.2-pro"));
        assert!(!supports_native_schema_auto(Provider::OpenAi, "gpt-4o-2024-05-13"));
        assert!(supports_native_schema_auto(Provider::Gemini, "gemini-2.5-pro"));
        assert!(!supports_native_schema_auto(Provider::Gemini, "gemini-1.5-pro"));
        assert!(!supports_native_schema_auto(Provider::Gemini, "custom-gemini-proxy"));
        assert!(!supports_native_schema_auto(Provider::OpenAiCompatible, "anything"));
        assert!(!supports_native_schema_auto(Provider::Anthropic, "claude-sonnet-4"));
    }

    #[test]
    fn parse_usage_anthropic() {
        let resp = json!({"usage": {"input_tokens": 100, "output_tokens": 40}});
        let u = parse_usage(Provider::Anthropic, &resp).unwrap();
        assert_eq!(u, usage(Some(100), Some(40), None, None));
    }

    #[test]
    fn parse_usage_gemini_output_includes_thoughts_via_total() {
        // Gemini: output incl reasoning = total - prompt (works for both API and Vertex).
        let resp = json!({"usageMetadata": {"promptTokenCount": 50, "candidatesTokenCount": 30, "thoughtsTokenCount": 20, "totalTokenCount": 100}});
        let u = parse_usage(Provider::Gemini, &resp).unwrap();
        // 100 - 50 = candidates + thoughts
        assert_eq!(u, usage(Some(50), Some(50), Some(20), None));
    }

    #[test]
    fn parse_usage_ollama_native() {
        let resp = json!({"prompt_eval_count": 26, "eval_count": 259, "done": true});
        let u = parse_usage(Provider::OpenAiCompatible, &resp).unwrap();
        assert_eq!(u, usage(Some(26), Some(259), None, None));
    }

    #[test]
    fn parse_usage_llamacpp_native() {
        let resp = json!({"tokens_evaluated": 6, "tokens_predicted": 17});
        let u = parse_usage(Provider::OpenAiCompatible, &resp).unwrap();
        assert_eq!(u, usage(Some(6), Some(17), None, None));
    }

    #[test]
    fn parse_usage_none_when_unknown() {
        for resp in [
            json!({"something_else": 1, "choices": []}),
            json!({"usage": {}}),
            json!({"usage": {"total_tokens": 5}}),
            json!({"usage": null}),
            json!({"usage": "12"}),
            json!({"usageMetadata": {}}),
            json!({"usageMetadata": {"totalTokenCount": 29}}),
            json!({"usage": {"prompt_tokens": "12", "completion_tokens": -1}}),
            json!([1, 2]),
            json!("text"),
            json!(null),
        ] {
            assert_eq!(parse_usage(Provider::OpenAiCompatible, &resp), None, "{resp}");
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

    /// A captured real response from `tests/fixtures/ai-responses/`.
    fn fixture(name: &str) -> Value {
        let text = match name {
            "vllm-qwen-think" => include_str!("../../tests/fixtures/ai-responses/vllm-qwen-think.json"),
            "vllm-qwen-nothink" => include_str!("../../tests/fixtures/ai-responses/vllm-qwen-nothink.json"),
            "openai" => include_str!("../../tests/fixtures/ai-responses/openai.json"),
            "anthropic" => include_str!("../../tests/fixtures/ai-responses/anthropic.json"),
            "gemini" => include_str!("../../tests/fixtures/ai-responses/gemini.json"),
            "minimax" => include_str!("../../tests/fixtures/ai-responses/minimax.json"),
            "deepseek" => include_str!("../../tests/fixtures/ai-responses/deepseek.json"),
            other => panic!("no fixture {other}"),
        };
        serde_json::from_str(text).expect("the fixture is JSON")
    }

    #[test]
    fn usage_of_every_captured_runtime() {
        // Numbers read from the fixture files. Anthropic input = input 43 + cache creation 0 +
        // cache read 0; Gemini output = totalTokenCount 29 - promptTokenCount 8 = 21 and it reports
        // no cachedContentTokenCount; MiniMax reports no reasoning count; DeepSeek's cached count
        // comes from prompt_tokens_details (0), not prompt_cache_hit_tokens.
        let cases = [
            ("vllm-qwen-think", usage(Some(17), Some(37), Some(33), Some(0))),
            ("vllm-qwen-nothink", usage(Some(19), Some(2), Some(0), Some(0))),
            ("openai", usage(Some(13), Some(10), Some(0), Some(0))),
            ("anthropic", usage(Some(43), Some(39), Some(32), Some(0))),
            ("gemini", usage(Some(8), Some(21), Some(20), None)),
            ("minimax", usage(Some(183), Some(30), None, Some(128))),
            ("deepseek", usage(Some(37), Some(19), Some(17), Some(0))),
        ];
        // The shape decides, not the configured provider.
        for provider in [
            Provider::OpenAi,
            Provider::OpenAiCompatible,
            Provider::Anthropic,
            Provider::Gemini,
        ] {
            for (name, expected) in cases {
                assert_eq!(
                    parse_usage(provider, &fixture(name)),
                    Some(expected),
                    "{name} as {provider:?}"
                );
            }
        }
    }

    #[test]
    fn usage_counts_accept_only_whole_non_negative_numbers() {
        let input = |count: Value| {
            parse_usage(
                Provider::OpenAiCompatible,
                &json!({"usage": {"prompt_tokens": count, "completion_tokens": 5}}),
            )
            .expect("completion_tokens alone is a known shape")
            .input_tokens
        };
        assert_eq!(input(json!(12)), Some(12));
        assert_eq!(input(json!(12.0)), Some(12));
        assert_eq!(input(json!(0)), Some(0));
        assert_eq!(input(json!(9_007_199_254_740_992u64)), Some(9_007_199_254_740_992));
        for bad in [
            json!(12.5),
            json!(-1),
            json!(-0.5),
            json!("12"),
            json!(1e300),
            json!(9_007_199_254_740_993u64),
            json!(u64::MAX),
            json!(null),
            json!(true),
            json!([12]),
            json!({"n": 12}),
        ] {
            assert_eq!(input(bad.clone()), None, "{bad}");
        }
    }

    #[test]
    fn reasoning_larger_than_output_is_unknown() {
        let with_reasoning = |reasoning: u64| {
            parse_usage(
                Provider::OpenAiCompatible,
                &json!({"usage": {"prompt_tokens": 10, "completion_tokens": 5,
                    "completion_tokens_details": {"reasoning_tokens": reasoning}}}),
            )
        };
        assert_eq!(with_reasoning(6), Some(usage(Some(10), Some(5), None, None)));
        assert_eq!(with_reasoning(5), Some(usage(Some(10), Some(5), Some(5), None)));
    }

    #[test]
    fn anthropic_input_includes_prompt_cache_tokens() {
        let resp = json!({"usage": {"input_tokens": 10, "cache_creation_input_tokens": 5,
            "cache_read_input_tokens": 7, "output_tokens": 3}});
        assert_eq!(
            parse_usage(Provider::Anthropic, &resp),
            Some(usage(Some(22), Some(3), None, Some(7)))
        );
        // Cache parts alone do not make an input count up.
        let resp = json!({"usage": {"cache_read_input_tokens": 7, "output_tokens": 3}});
        assert_eq!(
            parse_usage(Provider::Anthropic, &resp),
            Some(usage(None, Some(3), None, Some(7)))
        );
    }

    #[test]
    fn openai_responses_usage_shape() {
        let resp = json!({"usage": {"input_tokens": 20, "input_tokens_details": {"cached_tokens": 4},
            "output_tokens": 9, "output_tokens_details": {"reasoning_tokens": 6}}});
        assert_eq!(
            parse_usage(Provider::OpenAi, &resp),
            Some(usage(Some(20), Some(9), Some(6), Some(4)))
        );
    }

    #[test]
    fn deepseek_cache_hit_tokens_when_no_prompt_details() {
        let resp = json!({"usage": {"prompt_tokens": 37, "completion_tokens": 19, "prompt_cache_hit_tokens": 30}});
        assert_eq!(
            parse_usage(Provider::OpenAiCompatible, &resp),
            Some(usage(Some(37), Some(19), None, Some(30)))
        );
    }

    #[test]
    fn gemini_output_from_parts_when_total_is_inconsistent() {
        let resp = json!({"usageMetadata": {"promptTokenCount": 50, "totalTokenCount": 40,
            "candidatesTokenCount": 7, "thoughtsTokenCount": 3, "cachedContentTokenCount": 12}});
        assert_eq!(
            parse_usage(Provider::Gemini, &resp),
            Some(usage(Some(50), Some(10), Some(3), Some(12)))
        );
    }

    #[test]
    fn parse_anthropic_content() {
        let resp = json!({"content": [ {"type":"text","text":"a"}, {"type":"text","text":"b"} ]});
        assert_eq!(parse_content(Provider::Anthropic, &resp).as_deref(), Some("ab"));
    }

    #[test]
    fn parse_gemini_content() {
        let resp = json!({"candidates":[{"content":{"parts":[{"text":"x"},{"text":"y"}]}}]});
        assert_eq!(parse_content(Provider::Gemini, &resp).as_deref(), Some("xy"));
    }

    #[test]
    fn openai_content_parts_are_joined() {
        let resp = json!({"choices": [{"message": {"content": [
            {"type": "text", "text": "{\"a\":"},
            {"type": "image_url", "image_url": {"url": "http://x"}},
            {"type": "text", "text": "1}"}
        ]}}]});
        assert_eq!(
            parse_content(Provider::OpenAiCompatible, &resp).as_deref(),
            Some("{\"a\":1}")
        );
        let no_text = json!({"choices": [{"message": {"content": [{"type": "image_url"}]}}]});
        assert_eq!(parse_content(Provider::OpenAi, &no_text), None);
    }

    #[test]
    fn refusal_without_content() {
        let resp = json!({"choices": [{"message": {"content": null, "refusal": "I can't help with that."}}]});
        assert_eq!(parse_content(Provider::OpenAi, &resp), None);
        assert_eq!(
            parse_refusal(Provider::OpenAi, &resp).as_deref(),
            Some("I can't help with that.")
        );
        assert_eq!(
            parse_refusal(Provider::OpenAi, &fixture("openai")),
            None,
            "refusal: null"
        );
    }

    #[test]
    fn reasoning_text_of_every_captured_runtime() {
        // Character counts of the trimmed reasoning text in the fixture files.
        let cases = [
            ("vllm-qwen-think", Provider::OpenAiCompatible, Some(148)), // message.reasoning
            ("vllm-qwen-nothink", Provider::OpenAiCompatible, None),    // reasoning: null
            ("deepseek", Provider::OpenAiCompatible, Some(77)),         // message.reasoning_content
            ("anthropic", Provider::Anthropic, Some(123)),              // "thinking" block
            ("minimax", Provider::OpenAiCompatible, Some(108)),         // inline <think> in the content
            ("openai", Provider::OpenAi, None),
            ("gemini", Provider::Gemini, None),
        ];
        for (name, provider, expected) in cases {
            assert_eq!(parse_reasoning(provider, &fixture(name)), expected, "{name}");
        }
    }

    #[test]
    fn inline_think_blocks_count_their_inner_text() {
        let content = |text: &str| json!({"choices": [{"message": {"content": text}}]});
        let reasoning = |text: &str| parse_reasoning(Provider::OpenAiCompatible, &content(text));
        assert_eq!(reasoning("<think>ab</think>X<think>čd</think>Y"), Some(4));
        assert_eq!(reasoning("<think>cut off mid-thou"), Some(16), "unterminated");
        assert_eq!(reasoning("<think> \n </think>{}"), None, "blank");
        assert_eq!(reasoning("{\"ok\":true}"), None);
    }

    #[test]
    fn gemini_thought_parts_are_reasoning_not_content() {
        let resp = json!({"candidates": [{"content": {"parts": [
            {"text": "Let me think.", "thought": true},
            {"text": "OK"}
        ]}}]});
        assert_eq!(parse_content(Provider::Gemini, &resp).as_deref(), Some("OK"));
        assert_eq!(parse_reasoning(Provider::Gemini, &resp), Some(13));
    }

    /// xorshift64: a deterministic pseudo-random sequence without a new dependency.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
            &items[(self.next() % items.len() as u64) as usize]
        }
    }

    /// Keys of every response shape we parse, so random values often hit the parsed paths.
    const KEYS: &[&str] = &[
        "usage",
        "choices",
        "message",
        "content",
        "reasoning",
        "reasoning_content",
        "refusal",
        "text",
        "type",
        "thinking",
        "thought",
        "parts",
        "candidates",
        "usageMetadata",
        "prompt_tokens",
        "completion_tokens",
        "prompt_tokens_details",
        "completion_tokens_details",
        "reasoning_tokens",
        "cached_tokens",
        "prompt_cache_hit_tokens",
        "input_tokens",
        "output_tokens",
        "cache_creation_input_tokens",
        "cache_read_input_tokens",
        "output_tokens_details",
        "input_tokens_details",
        "thinking_tokens",
        "promptTokenCount",
        "candidatesTokenCount",
        "totalTokenCount",
        "thoughtsTokenCount",
        "cachedContentTokenCount",
        "prompt_eval_count",
        "eval_count",
        "tokens_evaluated",
        "tokens_predicted",
        "finish_reason",
        "stop_reason",
        "finishReason",
        "error",
        "base_resp",
        "status_code",
        "status_msg",
        "x",
    ];

    fn random_json(rng: &mut Rng, depth: u32) -> Value {
        let kind = if depth >= 5 { rng.next() % 4 } else { rng.next() % 6 };
        match kind {
            0 => rng
                .pick(&[
                    json!(null),
                    json!(true),
                    json!(0),
                    json!(-1),
                    json!(12.5),
                    json!(-0.0),
                    json!(1e300),
                    json!(u64::MAX),
                    json!(i64::MIN),
                    json!(9_007_199_254_740_993u64),
                ])
                .clone(),
            1 => json!(rng.next() >> (rng.next() % 64)),
            2 | 3 => json!(*rng.pick(&[
                "",
                "text",
                "<think>",
                "</think>",
                "<think>ž</think>{}",
                "<think>unterminated ž",
                "stop",
                "length",
                "thinking",
                "12",
                "\u{1F600}",
            ])),
            4 => Value::Array((0..rng.next() % 4).map(|_| random_json(rng, depth + 1)).collect()),
            _ => Value::Object(
                (0..rng.next() % 5)
                    .map(|_| (rng.pick(KEYS).to_string(), random_json(rng, depth + 1)))
                    .collect(),
            ),
        }
    }

    fn parse_everything(resp: &Value) {
        for provider in [
            Provider::OpenAi,
            Provider::OpenAiCompatible,
            Provider::Anthropic,
            Provider::Gemini,
        ] {
            let _ = parse_usage(provider, resp);
            let _ = parse_content(provider, resp);
            let _ = parse_reasoning(provider, resp);
            let _ = parse_refusal(provider, resp);
            let _ = parse_finish_reason(provider, resp);
            let _ = extract_error(provider, resp);
        }
        let _ = parse_model_list(resp);
    }

    /// Nothing in response parsing may panic, whatever a provider sends.
    #[test]
    fn response_parsing_never_panics() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for _ in 0..20_000 {
            parse_everything(&random_json(&mut rng, 0));
        }

        let fixtures = [
            fixture("vllm-qwen-think"),
            fixture("vllm-qwen-nothink"),
            fixture("openai"),
            fixture("anthropic"),
            fixture("gemini"),
            fixture("minimax"),
            fixture("deepseek"),
            serde_json::from_str(include_str!(
                "../../tests/fixtures/ai-responses/error-anthropic-unknown-model.json"
            ))
            .expect("JSON"),
            serde_json::from_str(include_str!(
                "../../tests/fixtures/ai-responses/error-openai-unknown-model.json"
            ))
            .expect("JSON"),
            serde_json::from_str(include_str!(
                "../../tests/fixtures/ai-responses/error-vllm-unknown-model.json"
            ))
            .expect("JSON"),
            model_list("vllm"),
            model_list("anthropic"),
            model_list("gemini"),
            model_list("openai"),
        ];
        for resp in fixtures {
            parse_everything(&resp);
            // The fixture with one key removed in turn: top level, `usage`, `usageMetadata`,
            // `choices[0]`, `choices[0].message`, `candidates[0]` and the first model entry.
            let paths: [&[&str]; 8] = [
                &[],
                &["usage"],
                &["usageMetadata"],
                &["choices", "0"],
                &["choices", "0", "message"],
                &["candidates", "0"],
                &["data", "0"],
                &["models", "0"],
            ];
            for path in paths {
                let pointer: String = path.iter().map(|segment| format!("/{segment}")).collect();
                let keys: Vec<String> = match resp.pointer(&pointer).and_then(Value::as_object) {
                    Some(object) => object.keys().cloned().collect(),
                    None => continue,
                };
                for key in keys {
                    let mut pruned = resp.clone();
                    if let Some(object) = pruned.pointer_mut(&pointer).and_then(Value::as_object_mut) {
                        object.remove(&key);
                    }
                    parse_everything(&pruned);
                }
            }
        }
    }

    /// A captured model list (`models-<name>.json`) from `tests/fixtures/ai-responses/`.
    fn model_list(name: &str) -> Value {
        let text = match name {
            "vllm" => include_str!("../../tests/fixtures/ai-responses/models-vllm.json"),
            "anthropic" => include_str!("../../tests/fixtures/ai-responses/models-anthropic.json"),
            "gemini" => include_str!("../../tests/fixtures/ai-responses/models-gemini.json"),
            "openai" => include_str!("../../tests/fixtures/ai-responses/models-openai.json"),
            other => panic!("no model list {other}"),
        };
        serde_json::from_str(text).expect("the fixture is JSON")
    }

    fn model(
        id: &str,
        display_name: Option<&str>,
        context_window: Option<u64>,
        max_output_tokens: Option<u64>,
    ) -> ModelInfo {
        ModelInfo {
            id: id.to_string(),
            display_name: display_name.map(str::to_string),
            context_window,
            max_output_tokens,
        }
    }

    #[test]
    fn model_lists_of_every_captured_runtime() {
        assert_eq!(
            parse_model_list(&model_list("vllm")),
            Some(vec![
                model("deepseek-ai/DeepSeek-V4-Flash-0731", None, Some(262_144), None),
                model("deepseek-ai/DeepSeek-V4-Flash-Vision-Exp", None, Some(262_144), None),
                model("nvidia/Qwen3.8-Flash-Next-NVFP4", None, Some(262_144), None),
            ])
        );
        assert_eq!(
            parse_model_list(&model_list("anthropic")),
            Some(vec![
                model(
                    "claude-fable-5-1",
                    Some("Claude Fable 5.1"),
                    Some(1_000_000),
                    Some(128_000)
                ),
                model("claude-opus-5", Some("Claude Opus 5"), Some(1_000_000), Some(128_000)),
                model(
                    "claude-opus-5-5",
                    Some("Claude Opus 5.5"),
                    Some(1_000_000),
                    Some(128_000)
                ),
            ])
        );
        assert_eq!(
            parse_model_list(&model_list("gemini")),
            Some(vec![
                model(
                    "gemini-2.5-flash",
                    Some("Gemini 2.5 Flash"),
                    Some(1_048_576),
                    Some(65_536)
                ),
                model(
                    "gemini-2.5-flash-preview-tts",
                    Some("Gemini 2.5 Flash Preview TTS"),
                    Some(8192),
                    Some(16_384)
                ),
                model("gemini-2.5-pro", Some("Gemini 2.5 Pro"), Some(1_048_576), Some(65_536)),
            ])
        );
        assert_eq!(
            parse_model_list(&model_list("openai")),
            Some(vec![
                model("gpt-5-2025-08-07", None, None, None),
                model("gpt-5-mini", None, None, None),
                model("gpt-audio-2025-08-28", None, None, None),
            ])
        );
    }

    #[test]
    fn odd_model_entries_are_skipped_or_lose_only_the_odd_field() {
        let resp = json!({"data": [
            {"id": "b", "context_length": 32768, "max_tokens": 4096.0},
            {"id": "a", "context_window": -5, "display_name": 7},
            {"id": "c", "max_model_len": 1.5, "max_tokens": "lots", "displayName": "  "},
            {"id": "  d  ", "max_model_len": 0, "display_name": " Model D "},
            {"id": "b", "max_model_len": 1},
            {"id": "e", "max_input_tokens": 1e300},
            {"id": ""},
            {"id": 42},
            {"display_name": "no id"},
            "not an object",
            null
        ]});
        assert_eq!(
            parse_model_list(&resp),
            Some(vec![
                model("a", None, None, None),
                // A duplicate id keeps its first entry.
                model("b", None, Some(32_768), Some(4096)),
                model("c", None, None, None),
                model("d", Some("Model D"), None, None),
                model("e", None, None, None),
            ])
        );
    }

    #[test]
    fn models_that_cannot_generate_content_are_left_out() {
        let resp = json!({"models": [
            {"name": "models/text-embedding-004", "supportedGenerationMethods": ["embedContent"]},
            {"name": "models/gemini-x", "supportedGenerationMethods": ["generateContent"], "inputTokenLimit": 100},
            {"name": "models/no-methods-listed"}
        ]});
        assert_eq!(
            parse_model_list(&resp),
            Some(vec![
                model("gemini-x", None, Some(100), None),
                model("no-methods-listed", None, None, None),
            ])
        );
    }

    #[test]
    fn an_answer_without_a_model_list_is_not_one() {
        assert_eq!(parse_model_list(&json!({"object": "list", "data": []})), Some(vec![]));
        for resp in [
            json!(null),
            json!([]),
            json!("models"),
            json!({"data": "soon"}),
            json!({"models": {"id": "x"}}),
            json!({"error": {"message": "invalid API key"}}),
        ] {
            assert_eq!(parse_model_list(&resp), None, "{resp}");
        }
    }

    #[test]
    fn model_list_requests_of_every_provider() {
        let has = |r: &ShapedRequest, name: &str, value: &str| r.headers.iter().any(|(n, v)| n == name && v == value);
        let r = models_request(Provider::OpenAiCompatible, "http://gpu:7999/v1/", Some("sk-x"));
        assert_eq!(r.url, "http://gpu:7999/v1/models");
        assert!(has(&r, "authorization", "Bearer sk-x"));
        let r = models_request(Provider::OpenAiCompatible, "http://gpu:7999/v1", None);
        assert_eq!(r.url, "http://gpu:7999/v1/models");
        assert!(
            r.headers.iter().all(|(name, _)| name != "authorization"),
            "no key, no header"
        );
        let r = models_request(Provider::OpenAi, "https://api.openai.com/v1", Some("sk-x"));
        assert_eq!(r.url, "https://api.openai.com/v1/models");
        assert!(has(&r, "authorization", "Bearer sk-x"));
        // One page of the most either API returns at once.
        let r = models_request(Provider::Anthropic, "https://api.anthropic.com", Some("sk-ant"));
        assert_eq!(r.url, "https://api.anthropic.com/v1/models?limit=1000");
        assert!(has(&r, "x-api-key", "sk-ant") && has(&r, "anthropic-version", "2023-06-01"));
        let r = models_request(
            Provider::Gemini,
            "https://generativelanguage.googleapis.com/v1beta",
            Some("AIza-x"),
        );
        assert_eq!(
            r.url,
            "https://generativelanguage.googleapis.com/v1beta/models?pageSize=1000"
        );
        // The key goes in a header, never in the URL (which errors and logs may quote).
        assert!(has(&r, "x-goog-api-key", "AIza-x"));
        assert!(!r.url.contains("AIza-x"));
    }

    #[test]
    fn error_bodies_of_captured_runtimes() {
        let cases = [
            (
                include_str!("../../tests/fixtures/ai-responses/error-anthropic-unknown-model.json"),
                Provider::Anthropic,
                "model: no-such-model-xyz",
            ),
            (
                include_str!("../../tests/fixtures/ai-responses/error-openai-unknown-model.json"),
                Provider::OpenAi,
                "The model `no-such-model-xyz` does not exist or you do not have access to it.",
            ),
            (
                include_str!("../../tests/fixtures/ai-responses/error-vllm-unknown-model.json"),
                Provider::OpenAiCompatible,
                "The model `no-such-model` does not exist.",
            ),
        ];
        for (body, provider, message) in cases {
            let resp: Value = serde_json::from_str(body).expect("JSON");
            assert_eq!(extract_error(provider, &resp).as_deref(), Some(message));
        }
    }

    #[test]
    fn minimax_base_resp_error_detected() {
        let resp = json!({"base_resp": {"status_code": 1004, "status_msg": "auth failed"}});
        let err = extract_error(Provider::OpenAiCompatible, &resp);
        assert!(err.unwrap().contains("auth failed"));
    }

    #[test]
    fn minimax_base_resp_ok_not_error() {
        let resp = json!({"base_resp": {"status_code": 0, "status_msg": ""}, "choices":[]});
        assert!(extract_error(Provider::OpenAiCompatible, &resp).is_none());
    }
}
