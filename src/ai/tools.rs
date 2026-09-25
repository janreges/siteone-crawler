// SiteOne Crawler - AI utility modes
// (c) Jan Reges <jan.reges@siteone.cz>
//
// `--ai-list-models` and `--ai-check`: one-shot runs against the configured AI endpoint without
// a crawl, for hosts such as the desktop GUI that let a person pick and test a model. Each prints
// exactly one JSON object on stdout — also on failure, as `{"ok":false,"error":…}` — and never a
// credential. Human-readable details go to stderr.

use std::io::Write;

use serde::Serialize;

use super::client::AiClient;
use super::config::{self, AiConfig};
use super::provider::{ChatMessage, ChatRequest, ModelInfo};
use crate::options::core_options::CoreOptions;
use crate::utils;

/// What `--ai-check` asks the model.
const CHECK_PROMPT: &str = "Reply with the single word OK.";

/// The accounting label of the `--ai-check` request, shown in its console line.
const CHECK_CATEGORY: &str = "Connection check";

/// The longest reply `--ai-check` prints.
const MAX_REPLY_CHARS: usize = 200;

/// The answer of `--ai-list-models`. Sizes an endpoint does not state are `null`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelList {
    pub provider: &'static str,
    /// The endpoint asked, without credentials.
    pub endpoint: String,
    pub models: Vec<ModelInfo>,
}

/// The answer of `--ai-check`. What the provider did not report is left out, never 0.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub provider: &'static str,
    pub model: String,
    /// The request's time from send to body read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_input_tokens: Option<u64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "crate::events::one_decimal"
    )]
    pub output_tokens_per_second: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    /// The model's answer without inline reasoning, cut to 200 characters.
    pub reply: String,
}

/// One JSON answer: `ok` first, then the fields of `body`.
#[derive(Serialize)]
struct Answer<T: Serialize> {
    ok: bool,
    #[serde(flatten)]
    body: T,
}

#[derive(Serialize)]
struct Failure<'a> {
    error: &'a str,
}

/// Whether `argv` asks for a utility mode, read from the raw arguments so that a configuration
/// error can still be answered in JSON.
pub fn requested(argv: &[String]) -> bool {
    argv.iter().skip(1).any(|arg| {
        ["--ai-list-models", "--ai-check"].iter().any(|flag| {
            arg == flag
                || arg
                    .strip_prefix(flag)
                    .and_then(|rest| rest.strip_prefix('='))
                    .is_some_and(|value| ["1", "yes", "true"].contains(&value))
        })
    })
}

/// Runs the utility mode `options` asks for and prints its answer. Returns the exit code:
/// 0 on success, 1 on failure.
pub async fn run(options: &CoreOptions) -> i32 {
    if options.no_color {
        utils::disable_colors();
    } else if options.force_color {
        utils::force_enabled_colors();
    }
    let answer = match config::build_config(options) {
        Err(error) => Err(error),
        Ok(config) if options.ai_list_models => list_models(config).await.map(|list| {
            print_info(&format!(
                "AI models: {} model(s) at {}.",
                list.models.len(),
                list.endpoint
            ));
            to_json(&list)
        }),
        Ok(config) => check(config).await.map(|result| to_json(&result)),
    };
    match answer {
        Ok(json) => {
            print_answer(&json);
            0
        }
        Err(error) => {
            print_failure(&error);
            1
        }
    }
}

/// Lists the models of the configured endpoint.
pub async fn list_models(config: AiConfig) -> Result<ModelList, String> {
    let client = AiClient::new(config);
    let models = client.list_models().await?;
    Ok(ModelList {
        provider: client.provider().as_str(),
        endpoint: utils::redact_url_userinfo(&client.config().endpoint),
        models,
    })
}

/// Sends one short request to the configured model through the normal client, which prints its
/// console line on stderr.
pub async fn check(mut config: AiConfig) -> Result<CheckResult, String> {
    // A check measures the endpoint as it is now: nothing is read from or written to the cache.
    config.cache_dir = None;
    let client = AiClient::new(config);
    let req = ChatRequest {
        system: None,
        messages: vec![ChatMessage::user(CHECK_PROMPT)],
        max_tokens: client.config().max_tokens,
        temperature: client.config().temperature,
        json_mode: false,
        json_schema: None,
        schema_name: None,
    };
    let completion = client.complete(&req, CHECK_CATEGORY).await.map_err(|e| e.to_string())?;
    let usage = completion.usage;
    Ok(CheckResult {
        provider: client.provider().as_str(),
        model: client.config().model.clone(),
        ms: completion.duration_ms,
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        reasoning_tokens: usage.reasoning_tokens,
        cached_input_tokens: usage.cached_input_tokens,
        output_tokens_per_second: usage
            .output_tokens
            .zip(completion.duration_ms)
            .and_then(|(output, ms)| super::telemetry::per_second(output, ms)),
        finish_reason: completion.finish_reason,
        reply: reply_text(&completion.text),
    })
}

/// The model's answer as `--ai-check` prints it: without inline reasoning, trimmed, cut to size.
fn reply_text(text: &str) -> String {
    super::normalize::strip_think(text)
        .chars()
        .take(MAX_REPLY_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

fn to_json<T: Serialize>(body: &T) -> String {
    serde_json::to_string(&Answer { ok: true, body }).unwrap_or_else(|e| {
        let error = format!("the answer could not be written as JSON: {}", e);
        serde_json::to_string(&Answer {
            ok: false,
            body: Failure { error: &error },
        })
        .unwrap_or_default()
    })
}

/// Prints `error` in red on stderr and the `{"ok":false,…}` answer on stdout.
pub fn print_failure(error: &str) {
    let _ = writeln!(
        std::io::stderr(),
        "{}",
        utils::get_color_text(&format!("ERROR: {}", error), "red", false)
    );
    let json = serde_json::to_string(&Answer {
        ok: false,
        body: Failure { error },
    })
    .unwrap_or_default();
    print_answer(&json);
}

fn print_info(message: &str) {
    let _ = writeln!(std::io::stderr(), "{}", utils::get_color_text(message, "gray", false));
}

/// The one line on stdout. Never `println!`: it panics when stdout is gone.
fn print_answer(json: &str) {
    let mut stdout = std::io::stdout();
    let _ = writeln!(stdout, "{}", json);
    let _ = stdout.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Vec<String> {
        std::iter::once("siteone-crawler")
            .chain(args.iter().copied())
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn a_utility_mode_is_recognized_before_the_options_parse() {
        assert!(requested(&argv(&["--ai-list-models"])));
        assert!(requested(&argv(&["--ai-provider=nope", "--ai-check"])));
        assert!(requested(&argv(&["--ai-check=true"])));
        assert!(!requested(&argv(&["--ai-check=false"])));
        assert!(!requested(&argv(&["--url=https://example.com/", "--ai-dry-run"])));
        assert!(!requested(&argv(&["--ai-checkpoint"])));
    }

    #[test]
    fn the_reply_leaves_out_reasoning_and_is_cut_to_size() {
        assert_eq!(reply_text("<think>The user wants OK.</think>\n\nOK"), "OK");
        assert_eq!(reply_text("\n\nOK\n"), "OK");
        assert_eq!(reply_text(&"é".repeat(500)).chars().count(), MAX_REPLY_CHARS);
    }

    #[test]
    fn an_answer_starts_with_ok_and_leaves_out_what_is_unknown() {
        let result = CheckResult {
            provider: "openai-compatible",
            model: "m".to_string(),
            ms: Some(401),
            input_tokens: Some(17),
            output_tokens: Some(37),
            reasoning_tokens: None,
            cached_input_tokens: None,
            output_tokens_per_second: Some(92.269),
            finish_reason: None,
            reply: "OK".to_string(),
        };
        assert_eq!(
            to_json(&result),
            r#"{"ok":true,"provider":"openai-compatible","model":"m","ms":401,"inputTokens":17,"outputTokens":37,"outputTokensPerSecond":92.3,"reply":"OK"}"#
        );
        let list = ModelList {
            provider: "openai-compatible",
            endpoint: "http://gpu:7999/v1".to_string(),
            models: vec![ModelInfo {
                id: "m".to_string(),
                display_name: None,
                context_window: Some(262_144),
                max_output_tokens: None,
            }],
        };
        assert_eq!(
            to_json(&list),
            r#"{"ok":true,"provider":"openai-compatible","endpoint":"http://gpu:7999/v1","models":[{"id":"m","displayName":null,"contextWindow":262144,"maxOutputTokens":null}]}"#
        );
    }
}
