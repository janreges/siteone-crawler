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
