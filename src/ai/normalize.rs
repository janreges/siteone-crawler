// SiteOne Crawler - AI response normalization
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Deterministic normalization of raw LLM responses before parsing/display.
//
// Real-world LLM endpoints are messy: some models (e.g. MiniMax M3) emit inline
// `<think>...</think>` reasoning even in non-thinking mode, and JSON answers arrive
// raw, in single backticks, in triple backticks, or in ```json fenced blocks. This
// module makes all of that robust and is covered by extensive unit tests.

use once_cell::sync::Lazy;
use regex::Regex;

// Well-formed reasoning block: <think> ... </think> (DOTALL, non-greedy).
static RE_THINK_PAIR: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)<think>.*?</think>").unwrap());
// Unterminated trailing reasoning block: <think> ... (no closing tag).
static RE_THINK_OPEN: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)<think>.*$").unwrap());

/// Remove inline `<think>...</think>` reasoning blocks from a model response.
///
/// Handles both well-formed pairs and an unterminated trailing `<think>` (which some
/// models emit when truncated mid-reasoning).
pub fn strip_think(text: &str) -> String {
    let without_pairs = RE_THINK_PAIR.replace_all(text, "");
    let without_open = RE_THINK_OPEN.replace_all(&without_pairs, "");
    without_open.trim().to_string()
}

/// Unwrap a value from surrounding code fences, handling all common variants:
/// raw (no fence), single backticks, triple backticks, and language-tagged
/// ```json fences.
pub fn strip_code_fences(text: &str) -> String {
    let trimmed = text.trim();

    // Triple-backtick block, optionally tagged with a language (```json / ```JSON / ```).
    if let Some(rest) = trimmed.strip_prefix("```") {
        // Drop an optional language tag up to the first newline.
        let after_tag = match rest.find('\n') {
            Some(nl) => &rest[nl + 1..],
            None => rest, // single-line ```...``` with no newline
        };
        let inner = after_tag.strip_suffix("```").unwrap_or(after_tag);
        // Also handle a trailing fence preceded by a newline.
        let inner = inner.trim_end().strip_suffix("```").unwrap_or(inner);
        return inner.trim().to_string();
    }

    // Single-backtick wrap: `...`
    if trimmed.len() >= 2 && trimmed.starts_with('`') && trimmed.ends_with('`') && !trimmed[1..].starts_with('`') {
        return trimmed[1..trimmed.len() - 1].trim().to_string();
    }

    trimmed.to_string()
}

/// Extract the outermost JSON value (object or array) from a string that may contain
/// leading/trailing prose. Returns the substring from the first `{`/`[` to its matching
/// last `}`/`]`. Returns the trimmed input unchanged if no JSON delimiters are found.
pub fn extract_json(text: &str) -> String {
    let obj_start = text.find('{');
    let arr_start = text.find('[');

    let (open, close) = match (obj_start, arr_start) {
        (Some(o), Some(a)) => {
            if o < a {
                ('{', '}')
            } else {
                ('[', ']')
            }
        }
        (Some(_), None) => ('{', '}'),
        (None, Some(_)) => ('[', ']'),
        (None, None) => return text.trim().to_string(),
    };

    let start = text.find(open);
    let end = text.rfind(close);
    match (start, end) {
        (Some(s), Some(e)) if e > s => text[s..=e].trim().to_string(),
        _ => text.trim().to_string(),
    }
}

/// Extract a balanced JSON value starting at the first `open` delimiter, honoring string
/// literals (so braces/brackets inside `"..."` don't affect nesting) and escapes. Returns
/// None if no balanced value is found (e.g. truncated output).
pub fn extract_balanced(text: &str, open: char, close: char) -> Option<String> {
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escaped = false;
    let mut started = false;
    let mut start = 0usize;

    for (i, c) in text.char_indices() {
        if !started {
            if c == open {
                started = true;
                start = i;
                depth = 1;
            }
            continue;
        }
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            _ if c == open => depth += 1,
            _ if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(text[start..i + c.len_utf8()].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// Full normalization pipeline for a response expected to contain a JSON OBJECT
/// (seo/llms/typos): strip reasoning blocks, unwrap fences, then extract a balanced,
/// valid `{...}` (preferred), falling back to a balanced `[...]`, then a naive scan. This
/// avoids picking up a markdown `[link]` in prose before the real JSON object.
pub fn normalize_json_response(raw: &str) -> String {
    let no_think = strip_think(raw);
    let unfenced = strip_code_fences(&no_think);

    if let Some(obj) = extract_balanced(&unfenced, '{', '}')
        && serde_json::from_str::<serde_json::Value>(&obj).is_ok()
    {
        return obj;
    }
    if let Some(arr) = extract_balanced(&unfenced, '[', ']')
        && serde_json::from_str::<serde_json::Value>(&arr).is_ok()
    {
        return arr;
    }
    extract_json(&unfenced)
}

/// Normalization for a response expected to contain a JSON ARRAY (custom action). Prefers a
/// balanced `[...]` (e.g. an array of findings, including the inner array of a
/// `{"findings":[...]}` wrapper), falling back to a balanced `{...}`.
pub fn normalize_json_array(raw: &str) -> String {
    let no_think = strip_think(raw);
    let unfenced = strip_code_fences(&no_think);

    if let Some(arr) = extract_balanced(&unfenced, '[', ']')
        && serde_json::from_str::<serde_json::Value>(&arr).is_ok()
    {
        return arr;
    }
    if let Some(obj) = extract_balanced(&unfenced, '{', '}')
        && serde_json::from_str::<serde_json::Value>(&obj).is_ok()
    {
        return obj;
    }
    extract_json(&unfenced)
}

/// Normalization for a response expected to contain free-form text/markdown (e.g.
/// llms.txt): only strip reasoning blocks and unwrap an outer fence if the whole
/// answer was fenced.
pub fn normalize_text_response(raw: &str) -> String {
    let no_think = strip_think(raw);
    strip_code_fences(&no_think)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_well_formed_think_block() {
        let input = "<think>\nLet me reason about this.\n</think>\n{\"ok\":true}";
        assert_eq!(strip_think(input), "{\"ok\":true}");
    }

    #[test]
    fn strips_minimax_m3_style_think() {
        // Exact shape observed from MiniMax-M3 live probe.
        let input = "<think>\nThe user is asking me to reply. My instructions say I must always output a thinking block.\n</think>\nOK";
        assert_eq!(strip_think(input), "OK");
    }

    #[test]
    fn strips_unterminated_think() {
        let input = "<think>\nreasoning that got cut off mid-thought";
        assert_eq!(strip_think(input), "");
    }

    #[test]
    fn strips_multiple_think_blocks() {
        let input = "<think>a</think>X<think>b</think>Y";
        assert_eq!(strip_think(input), "XY");
    }

    #[test]
    fn leaves_text_without_think_untouched() {
        assert_eq!(strip_think("plain answer"), "plain answer");
    }

    #[test]
    fn unwraps_triple_backtick_json_tag() {
        let input = "```json\n{\"a\":1}\n```";
        assert_eq!(strip_code_fences(input), "{\"a\":1}");
    }

    #[test]
    fn unwraps_triple_backtick_uppercase_tag() {
        let input = "```JSON\n{\"a\":1}\n```";
        assert_eq!(strip_code_fences(input), "{\"a\":1}");
    }

    #[test]
    fn unwraps_triple_backtick_no_tag() {
        let input = "```\n{\"a\":1}\n```";
        assert_eq!(strip_code_fences(input), "{\"a\":1}");
    }

    #[test]
    fn unwraps_single_backtick() {
        let input = "`{\"a\":1}`";
        assert_eq!(strip_code_fences(input), "{\"a\":1}");
    }

    #[test]
    fn leaves_raw_json_untouched() {
        let input = "{\"a\":1}";
        assert_eq!(strip_code_fences(input), "{\"a\":1}");
    }

    #[test]
    fn extracts_json_with_surrounding_prose() {
        let input = "Here is the result: {\"a\":1} hope it helps";
        assert_eq!(extract_json(input), "{\"a\":1}");
    }

    #[test]
    fn extracts_json_array() {
        let input = "prefix [1,2,3] suffix";
        assert_eq!(extract_json(input), "[1,2,3]");
    }

    #[test]
    fn extracts_object_when_object_comes_first() {
        let input = "{\"items\":[1,2]}";
        assert_eq!(extract_json(input), "{\"items\":[1,2]}");
    }

    #[test]
    fn full_pipeline_minimax_think_plus_fence() {
        let input = "<think>\nI should produce JSON.\n</think>\n```json\n{\"score\":80}\n```";
        let out = normalize_json_response(input);
        assert_eq!(out, "{\"score\":80}");
        // And it must parse.
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["score"], 80);
    }

    #[test]
    fn full_pipeline_plain_json() {
        assert_eq!(normalize_json_response("{\"x\":1}"), "{\"x\":1}");
    }

    #[test]
    fn full_pipeline_prose_then_json() {
        let input = "Sure! Here you go:\n{\"x\":1}\nLet me know if you need more.";
        let out = normalize_json_response(input);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["x"], 1);
    }

    #[test]
    fn text_pipeline_unwraps_markdown_fence() {
        let input = "<think>plan</think>\n```\n# Title\n> summary\n```";
        assert_eq!(normalize_text_response(input), "# Title\n> summary");
    }

    #[test]
    fn object_first_ignores_markdown_link_in_prose() {
        // B2: prose containing a [link] before the real JSON object.
        let out = normalize_json_response("see [1] then {\"a\":1}");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn balanced_scan_respects_braces_in_strings() {
        let out = normalize_json_response("{\"text\":\"a } b { c\"}");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["text"], "a } b { c");
    }

    #[test]
    fn balanced_scan_handles_nested_objects() {
        let out = normalize_json_response("prefix {\"a\":{\"b\":1}} suffix");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["a"]["b"], 1);
    }

    #[test]
    fn array_first_for_custom() {
        let out = normalize_json_array("Here: [{\"x\":1},{\"y\":2}] done");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.is_array());
        assert_eq!(v.as_array().unwrap().len(), 2);
    }

    #[test]
    fn array_first_extracts_findings_inner_array() {
        let out = normalize_json_array("{\"findings\":[{\"a\":1}]}");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.is_array());
    }

    #[test]
    fn extract_balanced_none_on_truncated() {
        assert!(extract_balanced("{\"a\":1", '{', '}').is_none());
    }
}
