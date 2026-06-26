// SiteOne Crawler - AI configuration & API-key resolution
// (c) Jan Reges <jan.reges@siteone.cz>

use serde_json::Value;

use super::provider::Provider;
use crate::error::{CrawlerError, CrawlerResult};
use crate::options::core_options::CoreOptions;

/// Build an `AiConfig` from CLI options, resolving the API key. Returns an error string when
/// the provider/key configuration is unusable.
pub fn build_config(options: &CoreOptions) -> Result<AiConfig, String> {
    let provider = Provider::parse(&options.ai_provider)
        .ok_or_else(|| format!("invalid --ai-provider '{}'", options.ai_provider))?;
    let api_key = resolve_api_key(
        provider,
        options.ai_api_key.as_ref().map(|s| s.expose()),
        options.ai_api_key_env.as_deref(),
        options.ai_api_key_file.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    if api_key.is_none() && provider != Provider::OpenAiCompatible {
        return Err(format!(
            "AI is enabled but no API key resolved for provider '{}'. Set {} or use --ai-api-key-file.",
            provider.as_str(),
            provider.default_key_env()
        ));
    }
    let endpoint = options
        .ai_endpoint
        .clone()
        .or_else(|| provider.default_endpoint().map(|s| s.to_string()))
        .unwrap_or_default();
    let extra_body = options
        .ai_extra_body
        .as_ref()
        .and_then(|s| serde_json::from_str::<Value>(s).ok());
    let cache_dir = match options.ai_cache_dir.as_deref() {
        None | Some("") | Some("off") => None,
        Some(d) => Some(crate::utils::get_absolute_path(d)),
    };
    Ok(AiConfig {
        provider,
        endpoint,
        model: options.ai_model.clone().unwrap_or_default(),
        api_key,
        max_tokens: options.ai_max_tokens.clamp(1, 1_000_000) as u32,
        temperature: options.ai_temperature as f32,
        force_completion_tokens: options.ai_use_max_completion_tokens,
        extra_body,
        timeout_secs: options.ai_timeout.clamp(1, 3600) as u64,
        cache_dir,
    })
}

/// Runtime configuration for the AI client. The resolved `api_key` lives only here and
/// in the client — never in the serialized `CoreOptions`.
#[derive(Clone)]
pub struct AiConfig {
    pub provider: Provider,
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub force_completion_tokens: bool,
    pub extra_body: Option<Value>,
    pub timeout_secs: u64,
    pub cache_dir: Option<String>,
}

/// Resolve the API key using the documented precedence (first match wins):
/// 1. `--ai-api-key-file=PATH`        (read first line, trim)
/// 2. `--ai-api-key=env:VARNAME`      (indirection)
/// 3. `--ai-api-key=VALUE`            (raw, discouraged)
/// 4. `--ai-api-key-env=NAME`         (named env var)
/// 5. default conventional env var per provider (OPENAI/ANTHROPIC/GEMINI_API_KEY)
///
/// Returns `Ok(None)` when nothing resolves (caller decides whether that is fatal).
pub fn resolve_api_key(
    provider: Provider,
    raw_key: Option<&str>,
    key_env: Option<&str>,
    key_file: Option<&str>,
) -> CrawlerResult<Option<String>> {
    // 1. key file
    if let Some(path) = key_file {
        let content = std::fs::read_to_string(path)
            .map_err(|e| CrawlerError::Config(format!("Cannot read --ai-api-key-file '{}': {}", path, e)))?;
        let key = content.lines().next().unwrap_or("").trim().to_string();
        if key.is_empty() {
            return Err(CrawlerError::Config(format!("--ai-api-key-file '{}' is empty", path)));
        }
        return Ok(Some(key));
    }

    // 2 & 3. raw key (possibly env: indirection)
    if let Some(raw) = raw_key
        && !raw.is_empty()
    {
        if let Some(var) = raw.strip_prefix("env:") {
            return Ok(read_env(var));
        }
        return Ok(Some(raw.to_string()));
    }

    // 4. named env var
    if let Some(var) = key_env
        && !var.is_empty()
    {
        return Ok(read_env(var));
    }

    // 5. default conventional env var
    Ok(read_env(provider.default_key_env()))
}

fn read_env(var: &str) -> Option<String> {
    match std::env::var(var) {
        Ok(v) if !v.trim().is_empty() => Some(v.trim().to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Env-var tests use a unique var name to avoid cross-test interference.
    #[test]
    fn raw_key_wins_over_env() {
        let r = resolve_api_key(Provider::OpenAiCompatible, Some("sk-raw"), None, None).unwrap();
        assert_eq!(r.as_deref(), Some("sk-raw"));
    }

    #[test]
    fn env_indirection_reads_named_var() {
        unsafe { std::env::set_var("SITEONE_TEST_AI_KEY_A", "sk-from-env") };
        let r = resolve_api_key(
            Provider::OpenAiCompatible,
            Some("env:SITEONE_TEST_AI_KEY_A"),
            None,
            None,
        )
        .unwrap();
        assert_eq!(r.as_deref(), Some("sk-from-env"));
    }

    #[test]
    fn key_env_option_reads_named_var() {
        unsafe { std::env::set_var("SITEONE_TEST_AI_KEY_B", "sk-named") };
        let r = resolve_api_key(Provider::OpenAiCompatible, None, Some("SITEONE_TEST_AI_KEY_B"), None).unwrap();
        assert_eq!(r.as_deref(), Some("sk-named"));
    }

    #[test]
    fn missing_resolves_to_none() {
        let r = resolve_api_key(Provider::Anthropic, None, Some("SITEONE_DOES_NOT_EXIST_XYZ"), None).unwrap();
        assert_eq!(r, None);
    }
}
