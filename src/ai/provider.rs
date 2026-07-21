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

/// Token usage extracted from a response.
#[derive(Debug, Clone, Copy, Default)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

impl Usage {
    fn from_u64(prompt: Option<u64>, completion: Option<u64>) -> Self {
        Usage {
            prompt_tokens: prompt.unwrap_or(0).min(u32::MAX as u64) as u32,
            completion_tokens: completion.unwrap_or(0).min(u32::MAX as u64) as u32,
        }
    }
}

/// Extract the assistant's text content from a provider-native response.
pub fn parse_content(provider: Provider, resp: &Value) -> Option<String> {
    match provider {
        Provider::OpenAi | Provider::OpenAiCompatible => resp
            .get("choices")?
            .get(0)?
            .get("message")?
            .get("content")?
            .as_str()
            .map(|s| s.to_string()),
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
            let parts = resp
                .get("candidates")?
                .get(0)?
                .get("content")?
                .get("parts")?
                .as_array()?;
            let text: String = parts
                .iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("");
            Some(text)
        }
    }
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
/// of the configured provider). `completion_tokens` always represents OUTPUT INCLUDING any
/// reasoning/thinking tokens. Returns None when no known format is present — the caller then
/// keeps working but simply does not count tokens for that call (never panics).
///
/// Supported shapes:
/// - OpenAI / OpenAI-compatible (vLLM, llama.cpp server, LM Studio, SGLang, MiniMax, Ollama
///   `/v1`, Gemini OpenAI-compat): `usage.prompt_tokens` / `usage.completion_tokens`.
/// - Anthropic: `usage.input_tokens` / `usage.output_tokens`.
/// - Gemini native: `usageMetadata.promptTokenCount` + output as `totalTokenCount - prompt`
///   (correct on both Gemini API and Vertex), falling back to `candidatesTokenCount +
///   thoughtsTokenCount`.
/// - Ollama native (`/api/chat`, `/api/generate`): `prompt_eval_count` / `eval_count`.
/// - llama.cpp native (`/completion`): `tokens_evaluated` / `tokens_predicted`.
pub fn parse_usage(_provider: Provider, resp: &Value) -> Option<Usage> {
    let u64f = |v: &Value, k: &str| v.get(k).and_then(|n| n.as_u64());

    if let Some(u) = resp.get("usage") {
        // OpenAI-compatible.
        let (p, c) = (u64f(u, "prompt_tokens"), u64f(u, "completion_tokens"));
        if p.is_some() || c.is_some() {
            return Some(Usage::from_u64(p, c));
        }
        // Anthropic.
        let (i, o) = (u64f(u, "input_tokens"), u64f(u, "output_tokens"));
        if i.is_some() || o.is_some() {
            return Some(Usage::from_u64(i, o));
        }
    }

    // Gemini native.
    if let Some(m) = resp.get("usageMetadata") {
        let prompt = u64f(m, "promptTokenCount");
        let total = u64f(m, "totalTokenCount");
        let cand = u64f(m, "candidatesTokenCount");
        let thoughts = u64f(m, "thoughtsTokenCount");
        if prompt.is_some() || cand.is_some() || total.is_some() {
            let p = prompt.unwrap_or(0);
            let out = match total {
                Some(t) if t >= p => t - p, // captures candidates + thoughts on both API and Vertex
                _ => cand.unwrap_or(0) + thoughts.unwrap_or(0),
            };
            return Some(Usage::from_u64(Some(p), Some(out)));
        }
    }

    // Ollama native.
    let (pe, ec) = (u64f(resp, "prompt_eval_count"), u64f(resp, "eval_count"));
    if pe.is_some() || ec.is_some() {
        return Some(Usage::from_u64(pe, ec));
    }

    // llama.cpp native /completion.
    let (te, tp) = (u64f(resp, "tokens_evaluated"), u64f(resp, "tokens_predicted"));
    if te.is_some() || tp.is_some() {
        return Some(Usage::from_u64(te, tp));
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
        assert_eq!(u.prompt_tokens, 10);
        assert_eq!(u.completion_tokens, 3);
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
        assert_eq!(u.prompt_tokens, 100);
        assert_eq!(u.completion_tokens, 40);
    }

    #[test]
    fn parse_usage_gemini_output_includes_thoughts_via_total() {
        // Gemini: output incl reasoning = total - prompt (works for both API and Vertex).
        let resp = json!({"usageMetadata": {"promptTokenCount": 50, "candidatesTokenCount": 30, "thoughtsTokenCount": 20, "totalTokenCount": 100}});
        let u = parse_usage(Provider::Gemini, &resp).unwrap();
        assert_eq!(u.prompt_tokens, 50);
        assert_eq!(u.completion_tokens, 50); // 100 - 50 = candidates + thoughts
    }

    #[test]
    fn parse_usage_ollama_native() {
        let resp = json!({"prompt_eval_count": 26, "eval_count": 259, "done": true});
        let u = parse_usage(Provider::OpenAiCompatible, &resp).unwrap();
        assert_eq!(u.prompt_tokens, 26);
        assert_eq!(u.completion_tokens, 259);
    }

    #[test]
    fn parse_usage_llamacpp_native() {
        let resp = json!({"tokens_evaluated": 6, "tokens_predicted": 17});
        let u = parse_usage(Provider::OpenAiCompatible, &resp).unwrap();
        assert_eq!(u.prompt_tokens, 6);
        assert_eq!(u.completion_tokens, 17);
    }

    #[test]
    fn parse_usage_none_when_unknown() {
        let resp = json!({"something_else": 1, "choices": []});
        assert!(parse_usage(Provider::OpenAiCompatible, &resp).is_none());
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
