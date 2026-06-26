// Live AI client integration tests (network, #[ignore] by default).
// Run with: cargo test --test ai_live -- --ignored --nocapture
// Requires MINIMAX_API_KEY in env for the MiniMax test; the vLLM test reads its endpoint from
// VLLM_ENDPOINT (and an optional VLLM_API_KEY), defaulting to a local server.

use siteone_crawler::ai::client::AiClient;
use siteone_crawler::ai::config::AiConfig;
use siteone_crawler::ai::normalize::normalize_json_response;
use siteone_crawler::ai::provider::{ChatMessage, ChatRequest, Provider};

fn json_req() -> ChatRequest {
    ChatRequest {
        system: Some(
            "You are a strict JSON generator. Output ONLY a single JSON object, no prose, no code fences.".to_string(),
        ),
        messages: vec![ChatMessage::user(
            "Return a JSON object with the 7 rainbow colors in the form {\"colors\":[\"red\",...]}. Output ONLY the JSON object.",
        )],
        max_tokens: 4000,
        temperature: 0.0,
        json_mode: true,
    }
}

#[tokio::test]
#[ignore]
async fn minimax_m3_json_roundtrip() {
    let key = std::env::var("MINIMAX_API_KEY").expect("set MINIMAX_API_KEY");
    let cfg = AiConfig {
        provider: Provider::OpenAiCompatible,
        endpoint: "https://api.minimax.io/v1".to_string(),
        model: "MiniMax-M3".to_string(),
        api_key: Some(key),
        max_tokens: 4000,
        temperature: 0.0,
        force_completion_tokens: false,
        extra_body: None,
        timeout_secs: 180,
        cache_dir: None,
    };
    let client = AiClient::new(cfg);
    let completion = client
        .complete(&json_req(), "live-test")
        .await
        .expect("minimax call failed");
    eprintln!("=== MiniMax-M3 RAW ===\n{}\n=== END ===", completion.text);
    eprintln!(
        "usage: prompt={} completion={}",
        completion.usage.prompt_tokens, completion.usage.completion_tokens
    );
    let norm = normalize_json_response(&completion.text);
    eprintln!("=== normalized ===\n{}", norm);
    let v: serde_json::Value = serde_json::from_str(&norm).expect("normalized output is not valid JSON");
    assert!(
        v.get("colors").and_then(|c| c.as_array()).is_some(),
        "missing colors array"
    );
}

#[tokio::test]
#[ignore]
async fn qwen_vllm_json_roundtrip() {
    let cfg = AiConfig {
        provider: Provider::OpenAiCompatible,
        endpoint: std::env::var("VLLM_ENDPOINT").unwrap_or_else(|_| "http://localhost:8000/v1".to_string()),
        model: std::env::var("VLLM_MODEL").unwrap_or_else(|_| "Qwen/Qwen3-32B".to_string()),
        api_key: std::env::var("VLLM_API_KEY").ok(),
        max_tokens: 4000,
        temperature: 0.0,
        force_completion_tokens: false,
        // Qwen3 supports disabling thinking via chat_template_kwargs for cheaper JSON.
        extra_body: Some(serde_json::json!({"chat_template_kwargs": {"enable_thinking": false}})),
        timeout_secs: 180,
        cache_dir: None,
    };
    let client = AiClient::new(cfg);
    let completion = client
        .complete(&json_req(), "live-test")
        .await
        .expect("qwen call failed");
    eprintln!("=== Qwen RAW ===\n{}\n=== END ===", completion.text);
    let norm = normalize_json_response(&completion.text);
    eprintln!("=== normalized ===\n{}", norm);
    let v: serde_json::Value = serde_json::from_str(&norm).expect("normalized output is not valid JSON");
    assert!(
        v.get("colors").and_then(|c| c.as_array()).is_some(),
        "missing colors array"
    );
}
