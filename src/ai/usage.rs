// SiteOne Crawler - AI usage accounting
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Process-global accumulator for ALL LLM calls made during a run (per-page actions + the
// report summary), across every provider. Tokens come from each provider's usage block, parsed
// by `provider::parse_usage` — completion/output already includes any reasoning/thinking tokens.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use once_cell::sync::Lazy;

use super::provider::Usage;

static CALLS: AtomicU64 = AtomicU64::new(0);
static CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static PROMPT_TOKENS: AtomicU64 = AtomicU64::new(0);
static COMPLETION_TOKENS: AtomicU64 = AtomicU64::new(0);
static REASONING_TOKENS: AtomicU64 = AtomicU64::new(0);
static CACHED_INPUT_TOKENS: AtomicU64 = AtomicU64::new(0);
static CALLS_WITH_UNKNOWN_REASONING: AtomicU64 = AtomicU64::new(0);
static NETWORK_TIME_MS: AtomicU64 = AtomicU64::new(0);
static CALLS_WITHOUT_USAGE: AtomicU64 = AtomicU64::new(0);
static HTTP_ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static RETRIES: AtomicU64 = AtomicU64::new(0);

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
    pub reasoning_tokens: u64,
    pub cached_input_tokens: u64,
    pub network_time_ms: u64,
    /// Output tokens of the requests whose generation time is known (see `record_generation`).
    pub timed_output_tokens: u64,
    /// Send-to-body time of those requests, without rate-limit waits and retry backoff.
    pub generation_ms: u64,
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

/// Adds `n` to `total`, stopping at `u64::MAX`: absurd reported counts must not wrap a total
/// around or panic.
fn add(total: &AtomicU64, n: u64) {
    let _ = total.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |sum| Some(sum.saturating_add(n)));
}

/// Record one completed LLM call under `category` (a human-readable analysis-type label): a 2xx
/// response, or a cache hit. `from_cache` calls count as logical completions but contribute no
/// tokens to the current run: their provider cost was paid in an earlier run.
/// A call whose `usage` reports neither input nor output tokens is counted as a call without
/// token data; one without a reasoning count adds nothing to the reasoning total but is counted in
/// `calls_with_unknown_reasoning`. The call's time is added by `record_call_time`.
pub fn record(category: &str, usage: &Usage, from_cache: bool) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    if from_cache {
        CACHE_HITS.fetch_add(1, Ordering::Relaxed);
    } else {
        add(&PROMPT_TOKENS, usage.input());
        add(&COMPLETION_TOKENS, usage.output());
        add(&REASONING_TOKENS, usage.reasoning_tokens.unwrap_or(0));
        add(&CACHED_INPUT_TOKENS, usage.cached_input_tokens.unwrap_or(0));
        if !usage.has_tokens() {
            CALLS_WITHOUT_USAGE.fetch_add(1, Ordering::Relaxed);
        }
        if usage.reasoning_tokens.is_none() {
            CALLS_WITH_UNKNOWN_REASONING.fetch_add(1, Ordering::Relaxed);
        }
    }
    if let Ok(mut map) = BY_CATEGORY.lock() {
        let e = map.entry(category.to_string()).or_default();
        e.calls += 1;
        if from_cache {
            e.cache_hits += 1;
        } else {
            e.prompt_tokens = e.prompt_tokens.saturating_add(usage.input());
            e.completion_tokens = e.completion_tokens.saturating_add(usage.output());
            e.reasoning_tokens = e.reasoning_tokens.saturating_add(usage.reasoning_tokens.unwrap_or(0));
            e.cached_input_tokens = e
                .cached_input_tokens
                .saturating_add(usage.cached_input_tokens.unwrap_or(0));
        }
    }
}

/// Record the time of one LLM call that went to the network, however it ended (answer, HTTP error,
/// timeout, unusable body): from before its first rate-limit wait to its end, retries included.
pub fn record_call_time(category: &str, elapsed_ms: u64) {
    add(&NETWORK_TIME_MS, elapsed_ms);
    if let Ok(mut map) = BY_CATEGORY.lock() {
        let e = map.entry(category.to_string()).or_default();
        e.network_time_ms = e.network_time_ms.saturating_add(elapsed_ms);
    }
}

/// Record the output tokens and the send-to-body time of one successful request, for the average
/// generation speed of its category.
pub fn record_generation(category: &str, output_tokens: u64, duration_ms: u64) {
    if let Ok(mut map) = BY_CATEGORY.lock() {
        let e = map.entry(category.to_string()).or_default();
        e.timed_output_tokens = e.timed_output_tokens.saturating_add(output_tokens);
        e.generation_ms = e.generation_ms.saturating_add(duration_ms);
    }
}

pub fn record_http_attempt(is_retry: bool) {
    HTTP_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
    if is_retry {
        RETRIES.fetch_add(1, Ordering::Relaxed);
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
            a.1.prompt_tokens.saturating_add(a.1.completion_tokens),
            b.1.prompt_tokens.saturating_add(b.1.completion_tokens),
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
    let reasoning = if u.reasoning_tokens > 0 {
        format!(", reasoning {} tokens", format_count(u.reasoning_tokens))
    } else {
        String::new()
    };
    let speed = if u.generation_ms > 0 {
        format!(
            ", avg {} tok/s",
            (u.timed_output_tokens as f64 * 1000.0 / u.generation_ms as f64).round() as u64
        )
    } else {
        String::new()
    };
    format!(
        "AI tokens — {}: {} request(s){}, input {} tokens, output {} tokens{}{}",
        name,
        u.calls,
        cache,
        format_count(u.prompt_tokens),
        format_count(u.completion_tokens),
        reasoning,
        speed,
    )
}

#[derive(Debug, Clone, Copy)]
pub struct UsageSnapshot {
    pub calls: u64,
    pub cache_hits: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub reasoning_tokens: u64,
    pub cached_input_tokens: u64,
    pub network_time_s: f64,
    pub calls_without_usage: u64,
    pub calls_with_unknown_reasoning: u64,
    pub http_attempts: u64,
    pub retries: u64,
}

impl UsageSnapshot {
    pub fn delta_since(self, earlier: Self) -> Self {
        Self {
            calls: self.calls.saturating_sub(earlier.calls),
            cache_hits: self.cache_hits.saturating_sub(earlier.cache_hits),
            prompt_tokens: self.prompt_tokens.saturating_sub(earlier.prompt_tokens),
            completion_tokens: self.completion_tokens.saturating_sub(earlier.completion_tokens),
            reasoning_tokens: self.reasoning_tokens.saturating_sub(earlier.reasoning_tokens),
            cached_input_tokens: self.cached_input_tokens.saturating_sub(earlier.cached_input_tokens),
            network_time_s: (self.network_time_s - earlier.network_time_s).max(0.0),
            calls_without_usage: self.calls_without_usage.saturating_sub(earlier.calls_without_usage),
            calls_with_unknown_reasoning: self
                .calls_with_unknown_reasoning
                .saturating_sub(earlier.calls_with_unknown_reasoning),
            http_attempts: self.http_attempts.saturating_sub(earlier.http_attempts),
            retries: self.retries.saturating_sub(earlier.retries),
        }
    }
}

pub fn snapshot() -> UsageSnapshot {
    UsageSnapshot {
        calls: CALLS.load(Ordering::Relaxed),
        cache_hits: CACHE_HITS.load(Ordering::Relaxed),
        prompt_tokens: PROMPT_TOKENS.load(Ordering::Relaxed),
        completion_tokens: COMPLETION_TOKENS.load(Ordering::Relaxed),
        reasoning_tokens: REASONING_TOKENS.load(Ordering::Relaxed),
        cached_input_tokens: CACHED_INPUT_TOKENS.load(Ordering::Relaxed),
        network_time_s: NETWORK_TIME_MS.load(Ordering::Relaxed) as f64 / 1000.0,
        calls_without_usage: CALLS_WITHOUT_USAGE.load(Ordering::Relaxed),
        calls_with_unknown_reasoning: CALLS_WITH_UNKNOWN_REASONING.load(Ordering::Relaxed),
        http_attempts: HTTP_ATTEMPTS.load(Ordering::Relaxed),
        retries: RETRIES.load(Ordering::Relaxed),
    }
}

/// Start a new crawler run with isolated accounting. The CLI executes one crawl at a time; without
/// this reset, embedding the crawler and running it repeatedly in one process would leak prior-run
/// tokens and costs into later AI artifacts.
pub fn reset() {
    CALLS.store(0, Ordering::Relaxed);
    CACHE_HITS.store(0, Ordering::Relaxed);
    PROMPT_TOKENS.store(0, Ordering::Relaxed);
    COMPLETION_TOKENS.store(0, Ordering::Relaxed);
    REASONING_TOKENS.store(0, Ordering::Relaxed);
    CACHED_INPUT_TOKENS.store(0, Ordering::Relaxed);
    NETWORK_TIME_MS.store(0, Ordering::Relaxed);
    CALLS_WITHOUT_USAGE.store(0, Ordering::Relaxed);
    CALLS_WITH_UNKNOWN_REASONING.store(0, Ordering::Relaxed);
    HTTP_ATTEMPTS.store(0, Ordering::Relaxed);
    RETRIES.store(0, Ordering::Relaxed);
    if let Ok(mut categories) = BY_CATEGORY.lock() {
        categories.clear();
    }
    if let Ok(mut model) = MODEL.lock() {
        *model = None;
    }
}

/// Emit the `aiUsage` event with the totals of the run (the model as recorded, else `model`).
pub fn emit_event(provider: &str, model: &str) {
    if !crate::events::is_enabled() {
        return;
    }
    let s = snapshot();
    crate::events::emit(crate::events::Event::AiUsage {
        provider: provider.to_string(),
        model: model_name().unwrap_or_else(|| model.to_string()),
        calls: s.calls,
        cache_hits: s.cache_hits,
        http_attempts: s.http_attempts,
        retries: s.retries,
        input_tokens: s.prompt_tokens,
        output_tokens: s.completion_tokens,
        reasoning_tokens: s.reasoning_tokens,
        cached_input_tokens: s.cached_input_tokens,
        calls_without_usage: s.calls_without_usage,
        network_ms: (s.network_time_s * 1000.0).round() as u64,
    });
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
            ..Default::default()
        };
        let line = format_category_line("SEO analysis", &u);
        assert!(line.contains("SEO analysis"));
        assert!(line.contains("12 request(s)"));
        assert!(line.contains("input 40.1k"));
        assert!(line.contains("output 4.6k"));
        assert!(!line.contains("from cache"));
    }

    /// Serializes the tests that record into the process-wide totals.
    static GLOBALS: Mutex<()> = Mutex::new(());

    fn usage(input: Option<u64>, output: Option<u64>, reasoning: Option<u64>, cached: Option<u64>) -> Usage {
        Usage {
            input_tokens: input,
            output_tokens: output,
            reasoning_tokens: reasoning,
            cached_input_tokens: cached,
        }
    }

    #[test]
    fn record_sums_reasoning_and_cached_tokens() {
        let _globals = GLOBALS.lock().unwrap_or_else(|e| e.into_inner());
        let category = "test: reasoning and cached";
        let before = snapshot();
        record(category, &usage(Some(17), Some(37), Some(33), Some(4)), false);
        record_call_time(category, 1200);
        record(category, &usage(Some(19), Some(2), None, Some(0)), false);
        record_call_time(category, 300);
        record(category, &Usage::default(), false);
        record_call_time(category, 100);
        // A cache hit adds no tokens: they were paid for in an earlier run.
        record(category, &usage(Some(1000), Some(500), Some(400), Some(900)), true);

        let d = snapshot().delta_since(before);
        assert_eq!((d.calls, d.cache_hits), (4, 1));
        assert_eq!((d.prompt_tokens, d.completion_tokens), (36, 39));
        assert_eq!((d.reasoning_tokens, d.cached_input_tokens), (33, 4));
        assert_eq!(d.calls_without_usage, 1);
        assert_eq!(
            d.calls_with_unknown_reasoning, 2,
            "the call without reasoning and the one without usage"
        );

        let (_, c) = categories()
            .into_iter()
            .find(|(name, _)| name == category)
            .expect("the category");
        assert_eq!((c.calls, c.cache_hits, c.network_time_ms), (4, 1, 1600));
        assert_eq!((c.prompt_tokens, c.completion_tokens), (36, 39));
        assert_eq!((c.reasoning_tokens, c.cached_input_tokens), (33, 4));
    }

    #[test]
    fn category_line_without_reasoning_reads_as_before() {
        let _globals = GLOBALS.lock().unwrap_or_else(|e| e.into_inner());
        record(
            "test: no reasoning",
            &usage(Some(40_079), Some(4_609), None, None),
            false,
        );
        let line = breakdown_lines()
            .into_iter()
            .find(|line| line.contains("test: no reasoning"))
            .expect("the category line");
        assert_eq!(
            line,
            "AI tokens — test: no reasoning: 1 request(s), input 40.1k (40079) tokens, output 4.6k (4609) tokens"
        );
    }

    #[test]
    fn category_line_with_reasoning_and_speed() {
        let _globals = GLOBALS.lock().unwrap_or_else(|e| e.into_inner());
        let category = "test: reasoning and speed";
        record(category, &usage(Some(17), Some(37), Some(33), None), false);
        record(category, &usage(Some(19), Some(2), None, None), false);
        // Generation throughput counts only the timed attempts: 39 tokens in 1.3 s = 30 tok/s.
        record_generation(category, 37, 1000);
        record_generation(category, 2, 300);
        let line = breakdown_lines()
            .into_iter()
            .find(|line| line.contains(category))
            .expect("the category line");
        assert_eq!(
            line,
            "AI tokens — test: reasoning and speed: 2 request(s), input 36 tokens, output 39 tokens, reasoning 33 tokens, avg 30 tok/s"
        );
    }

    #[test]
    fn totals_stop_at_the_largest_count_instead_of_overflowing() {
        let _globals = GLOBALS.lock().unwrap_or_else(|e| e.into_inner());
        // Every count at the largest value a response may report (2^53): 2^11 such responses
        // reach 2^64 in each total.
        let huge = usage(Some(1 << 53), Some(1 << 53), Some(1 << 53), Some(1 << 53));
        for _ in 0..(1 << 11) + 1 {
            record("test: huge", &huge, false);
            record_call_time("test: huge", 1 << 53);
            record_generation("test: huge", 1 << 53, 1 << 53);
        }
        record("test: small", &usage(Some(1), None, None, None), false);

        let s = snapshot();
        let totals = (
            s.prompt_tokens,
            s.completion_tokens,
            s.reasoning_tokens,
            s.cached_input_tokens,
        );
        assert_eq!(totals, (u64::MAX, u64::MAX, u64::MAX, u64::MAX));
        let categories = categories();
        let (_, huge) = categories
            .iter()
            .find(|(name, _)| name == "test: huge")
            .expect("the category");
        let times = (huge.network_time_ms, huge.timed_output_tokens, huge.generation_ms);
        assert_eq!(times, (u64::MAX, u64::MAX, u64::MAX));
        let names: Vec<&String> = categories.iter().map(|(name, _)| name).collect();
        let (huge_at, small_at) = (
            names.iter().position(|name| *name == "test: huge"),
            names.iter().position(|name| *name == "test: small"),
        );
        assert!(
            matches!((huge_at, small_at), (Some(huge), Some(small)) if huge < small),
            "the biggest first: {names:?}"
        );
        let line = breakdown_lines()
            .into_iter()
            .find(|line| line.contains("test: huge"))
            .expect("the category line");
        assert!(line.contains(&format!("input {}", format_count(u64::MAX))), "{line}");
        reset();
    }

    #[test]
    fn format_category_line_with_cache() {
        let u = CategoryUsage {
            calls: 5,
            cache_hits: 2,
            prompt_tokens: 100,
            completion_tokens: 50,
            network_time_ms: 0,
            ..Default::default()
        };
        let line = format_category_line("Custom check", &u);
        assert!(line.contains("2 from cache"));
    }
}
