// SiteOne Crawler - AI usage accounting
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Process-global accumulator for ALL LLM calls made during a run (per-page actions + the
// report summary), across every provider. Tokens come from each provider's usage block
// (OpenAI/compatible: prompt_tokens/completion_tokens; Anthropic: input_tokens/output_tokens;
// Gemini: promptTokenCount/candidatesTokenCount) — completion/output already includes any
// reasoning/thinking tokens.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use once_cell::sync::Lazy;

static CALLS: AtomicU64 = AtomicU64::new(0);
static CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static PROMPT_TOKENS: AtomicU64 = AtomicU64::new(0);
static COMPLETION_TOKENS: AtomicU64 = AtomicU64::new(0);
static NETWORK_TIME_MS: AtomicU64 = AtomicU64::new(0);
static CALLS_WITHOUT_USAGE: AtomicU64 = AtomicU64::new(0);

/// Per-analysis-type accounting (keyed by a human-readable category label).
static BY_CATEGORY: Lazy<Mutex<BTreeMap<String, CategoryUsage>>> = Lazy::new(|| Mutex::new(BTreeMap::new()));
/// The LLM model name used (first non-empty wins; both clients use the same one).
static MODEL: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

/// Token/request accounting for one analysis type.
#[derive(Debug, Clone, Default)]
pub struct CategoryUsage {
    pub calls: u64,
    pub cache_hits: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub network_time_ms: u64,
}

/// Remember the model name (called once per client; the first non-empty value sticks).
pub fn note_model(model: &str) {
    if model.trim().is_empty() {
        return;
    }
    if let Ok(mut m) = MODEL.lock()
        && m.is_none()
    {
        *m = Some(model.to_string());
    }
}

/// The model name used for the AI calls, if recorded.
pub fn model_name() -> Option<String> {
    MODEL.lock().ok().and_then(|m| m.clone())
}

/// Record one completed LLM call under `category` (a human-readable analysis-type label).
/// `from_cache` calls contribute their (originally spent) tokens but no network time.
/// `tokens_reported` is false when the provider's response did not contain a recognizable usage
/// block (the call still counts; its tokens are unknown).
pub fn record(
    category: &str,
    prompt_tokens: u64,
    completion_tokens: u64,
    elapsed_ms: u64,
    from_cache: bool,
    tokens_reported: bool,
) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    PROMPT_TOKENS.fetch_add(prompt_tokens, Ordering::Relaxed);
    COMPLETION_TOKENS.fetch_add(completion_tokens, Ordering::Relaxed);
    if from_cache {
        CACHE_HITS.fetch_add(1, Ordering::Relaxed);
    } else {
        NETWORK_TIME_MS.fetch_add(elapsed_ms, Ordering::Relaxed);
    }
    if !tokens_reported {
        CALLS_WITHOUT_USAGE.fetch_add(1, Ordering::Relaxed);
    }
    if let Ok(mut map) = BY_CATEGORY.lock() {
        let e = map.entry(category.to_string()).or_default();
        e.calls += 1;
        e.prompt_tokens += prompt_tokens;
        e.completion_tokens += completion_tokens;
        if from_cache {
            e.cache_hits += 1;
        } else {
            e.network_time_ms += elapsed_ms;
        }
    }
}

/// Per-category usage, ordered by total tokens consumed (biggest first).
pub fn categories() -> Vec<(String, CategoryUsage)> {
    let mut v: Vec<(String, CategoryUsage)> = BY_CATEGORY
        .lock()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    v.sort_by(|a, b| {
        let (ta, tb) = (
            a.1.prompt_tokens + a.1.completion_tokens,
            b.1.prompt_tokens + b.1.completion_tokens,
        );
        tb.cmp(&ta).then_with(|| a.0.cmp(&b.0))
    });
    v
}

/// One human-readable line per analysis type (for the Summary), ordered by tokens consumed.
/// Empty when no categorized calls were recorded.
pub fn breakdown_lines() -> Vec<String> {
    categories()
        .into_iter()
        .filter(|(_, u)| u.calls > 0)
        .map(|(name, u)| format_category_line(&name, &u))
        .collect()
}

fn format_category_line(name: &str, u: &CategoryUsage) -> String {
    let cache = if u.cache_hits > 0 {
        format!(", {} from cache", u.cache_hits)
    } else {
        String::new()
    };
    format!(
        "AI tokens — {}: {} request(s){}, input {} tokens, output {} tokens",
        name,
        u.calls,
        cache,
        format_count(u.prompt_tokens),
        format_count(u.completion_tokens),
    )
}

#[derive(Debug, Clone, Copy)]
pub struct UsageSnapshot {
    pub calls: u64,
    pub cache_hits: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub network_time_s: f64,
    pub calls_without_usage: u64,
}

pub fn snapshot() -> UsageSnapshot {
    UsageSnapshot {
        calls: CALLS.load(Ordering::Relaxed),
        cache_hits: CACHE_HITS.load(Ordering::Relaxed),
        prompt_tokens: PROMPT_TOKENS.load(Ordering::Relaxed),
        completion_tokens: COMPLETION_TOKENS.load(Ordering::Relaxed),
        network_time_s: NETWORK_TIME_MS.load(Ordering::Relaxed) as f64 / 1000.0,
        calls_without_usage: CALLS_WITHOUT_USAGE.load(Ordering::Relaxed),
    }
}

/// Human-readable count: `1.245M (1245678)` for millions, `12.3k (12345)` for thousands,
/// the plain number below 1000.
pub fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.3}M ({})", n as f64 / 1_000_000.0, n)
    } else if n >= 1_000 {
        format!("{:.1}k ({})", n as f64 / 1_000.0, n)
    } else {
        n.to_string()
    }
}

/// A one-line usage summary, or None if no LLM calls were made.
pub fn summary_line() -> Option<String> {
    let s = snapshot();
    if s.calls == 0 {
        return None;
    }
    let cache = if s.cache_hits > 0 {
        format!(" ({} served from cache)", s.cache_hits)
    } else {
        String::new()
    };
    let model = model_name().map(|m| format!(" using {}", m)).unwrap_or_default();

    // No provider reported token usage in any recognizable format → report calls + time only.
    if s.prompt_tokens == 0 && s.completion_tokens == 0 {
        return Some(format!(
            "AI usage: {} LLM call(s){} in {:.1}s{} — token usage not reported by the provider.",
            s.calls, cache, s.network_time_s, model
        ));
    }

    let partial = if s.calls_without_usage > 0 {
        format!(" ({} call(s) without token data)", s.calls_without_usage)
    } else {
        String::new()
    };
    Some(format!(
        "AI usage: {} LLM call(s){} in {:.1}s{} — input {} tokens, output {} tokens (incl. reasoning){}",
        s.calls,
        cache,
        s.network_time_s,
        model,
        format_count(s.prompt_tokens),
        format_count(s.completion_tokens),
        partial,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_count_buckets() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(12_345), "12.3k (12345)");
        assert_eq!(format_count(1_245_000), "1.245M (1245000)");
        assert_eq!(format_count(2_500_000), "2.500M (2500000)");
    }

    #[test]
    fn format_category_line_shape() {
        let u = CategoryUsage {
            calls: 12,
            cache_hits: 0,
            prompt_tokens: 40_079,
            completion_tokens: 4_609,
            network_time_ms: 0,
        };
        let line = format_category_line("SEO analysis", &u);
        assert!(line.contains("SEO analysis"));
        assert!(line.contains("12 request(s)"));
        assert!(line.contains("input 40.1k"));
        assert!(line.contains("output 4.6k"));
        assert!(!line.contains("from cache"));
    }

    #[test]
    fn format_category_line_with_cache() {
        let u = CategoryUsage {
            calls: 5,
            cache_hits: 2,
            prompt_tokens: 100,
            completion_tokens: 50,
            network_time_ms: 0,
        };
        let line = format_category_line("Custom check", &u);
        assert!(line.contains("2 from cache"));
    }
}
