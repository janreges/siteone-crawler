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

/// Best-effort repair of the mechanical JSON mistakes LLMs commonly make, so `serde_json` can
/// parse the result. This is NOT a full JSON parser — it fixes the frequent, well-documented
/// failure modes (studied across many models) in a single string-aware pass: markdown/```json
/// fences and `<think>` reasoning stripped + outer JSON value extracted; Python/JS literals
/// `True`/`False`/`None` → `true`/`false`/`null` (outside strings); single-quoted strings →
/// double-quoted (inner `"` escaped); trailing commas before `}`/`]` removed; and truncated output
/// completed (an unterminated string is closed and any still-open `{`/`[` are closed in order at
/// end-of-input).
///
/// It deliberately makes NO attempt to invent missing content — genuinely lost data stays lost; the
/// goal is only to make a recoverable structure parseable. Callers must still validate the parsed
/// value (and never fabricate defaults for a response this cannot rescue).
pub struct JsonRepair {
    pub json: String,
    /// True when repair had to close an unterminated string/container. That operation can make a
    /// token-truncated response parseable, but cannot recover its missing values.
    pub completed_truncation: bool,
}

pub fn repair_json_with_status(raw: &str) -> JsonRepair {
    // Start from the already-unwrapped candidate (fences + think removed), then the outermost
    // JSON value so leading/trailing prose does not confuse the pass.
    let unfenced = strip_code_fences(&strip_think(raw));
    let candidate = extract_json(&unfenced);

    let mut out = String::with_capacity(candidate.len() + 16);
    // Stack of open containers, storing the closing char we owe ('}' or ']').
    let mut stack: Vec<char> = Vec::new();
    let mut in_str = false;
    // The delimiter the model used for the current string ('"' or '\''); we always EMIT '"'.
    let mut str_delim = '"';
    let mut escaped = false;

    let bytes: Vec<char> = candidate.chars().collect();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if escaped {
                out.push(c);
                escaped = false;
            } else if c == '\\' {
                // `\'` is not a valid JSON escape — the model meant a literal apostrophe. Drop the
                // backslash so the string stays parseable; keep every other escape intact.
                if bytes.get(i + 1) == Some(&'\'') {
                    out.push('\'');
                    i += 2;
                    continue;
                }
                out.push(c);
                escaped = true;
            } else if c == str_delim {
                if str_delim == '\'' {
                    // A single-quoted string may contain an UNescaped apostrophe ("it's fine"). Only
                    // treat this `'` as the closing delimiter when the next non-space char is a
                    // structural token (`, : } ]`) or end-of-input; otherwise it is a literal
                    // apostrophe inside the (now double-quoted) string.
                    let mut j = i + 1;
                    while bytes.get(j).is_some_and(|c| c.is_whitespace()) {
                        j += 1;
                    }
                    let closes = match bytes.get(j) {
                        None => true,
                        Some(c) => matches!(c, ',' | ':' | '}' | ']'),
                    };
                    if closes {
                        out.push('"');
                        in_str = false;
                    } else {
                        out.push('\''); // literal apostrophe (valid inside a JSON double-quoted string)
                    }
                } else {
                    // End of a double-quoted string → emit a double quote.
                    out.push('"');
                    in_str = false;
                }
            } else if c == '"' && str_delim == '\'' {
                // A literal double quote inside a single-quoted string must be escaped.
                out.push_str("\\\"");
            } else {
                out.push(c);
            }
            i += 1;
            continue;
        }
        match c {
            '"' | '\'' => {
                in_str = true;
                str_delim = c;
                out.push('"');
            }
            '{' => {
                stack.push('}');
                out.push(c);
            }
            '[' => {
                stack.push(']');
                out.push(c);
            }
            '}' | ']' => {
                trim_trailing_comma(&mut out);
                if stack.last() == Some(&c) {
                    stack.pop();
                }
                out.push(c);
            }
            't' | 'f' | 'n' | 'T' | 'F' | 'N' => {
                // Python/JS literal normalization on a word boundary.
                if let Some((lit, len)) = match_literal(&bytes, i) {
                    out.push_str(lit);
                    i += len;
                    continue;
                }
                out.push(c);
            }
            _ => out.push(c),
        }
        i += 1;
    }

    let completed_truncation = in_str || !stack.is_empty();
    // Close an unterminated string.
    if in_str {
        out.push('"');
    }
    // Remove a trailing comma dangling at the very end, then close open containers.
    trim_trailing_comma(&mut out);
    while let Some(close) = stack.pop() {
        out.push(close);
    }
    JsonRepair {
        json: out,
        completed_truncation,
    }
}

pub fn repair_json(raw: &str) -> String {
    repair_json_with_status(raw).json
}

/// Drop a trailing comma (and following whitespace) already written to `out`.
fn trim_trailing_comma(out: &mut String) {
    let trimmed = out.trim_end();
    if trimmed.ends_with(',') {
        let new_len = trimmed.len() - 1;
        out.truncate(new_len);
    } else if trimmed.len() != out.len() {
        // Normalize trailing whitespace we may re-emit before a closer.
        out.truncate(trimmed.len());
    }
}

/// Match a bare `true`/`false`/`null` (any case) at `pos`, returning the canonical form + the
/// number of source chars consumed, IF it is a standalone word (not part of an identifier/string).
fn match_literal(bytes: &[char], pos: usize) -> Option<(&'static str, usize)> {
    let ends_word = |idx: usize| -> bool {
        match bytes.get(idx) {
            None => true,
            Some(c) => !(c.is_ascii_alphanumeric() || *c == '_'),
        }
    };
    let try_word = |word: &str, canon: &'static str| -> Option<(&'static str, usize)> {
        let wl = word.len();
        if pos + wl <= bytes.len() {
            let slice: String = bytes[pos..pos + wl].iter().collect();
            if slice.eq_ignore_ascii_case(word) && ends_word(pos + wl) {
                return Some((canon, wl));
            }
        }
        None
    };
    try_word("true", "true")
        .or_else(|| try_word("false", "false"))
        .or_else(|| try_word("null", "null"))
        .or_else(|| try_word("none", "null"))
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

    // ---- repair_json ----
    fn repaired_value(raw: &str) -> serde_json::Value {
        let fixed = repair_json(raw);
        serde_json::from_str(&fixed).unwrap_or_else(|e| panic!("repair failed to produce valid JSON: {}\n{}", e, fixed))
    }

    #[test]
    fn repair_removes_trailing_comma() {
        let v = repaired_value("{\"a\":1,\"b\":2,}");
        assert_eq!(v["b"], 2);
    }

    #[test]
    fn repair_trailing_comma_in_array() {
        let v = repaired_value("{\"xs\":[1,2,3,]}");
        assert_eq!(v["xs"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn repair_single_quotes_to_double() {
        let v = repaired_value("{'a':'hello','b':'world'}");
        assert_eq!(v["a"], "hello");
        assert_eq!(v["b"], "world");
    }

    #[test]
    fn repair_escapes_inner_double_quote_in_single_quoted_string() {
        let v = repaired_value("{'msg':'he said \"hi\"'}");
        assert_eq!(v["msg"], "he said \"hi\"");
    }

    #[test]
    fn repair_python_literals() {
        let v = repaired_value("{\"a\":True,\"b\":False,\"c\":None}");
        assert_eq!(v["a"], true);
        assert_eq!(v["b"], false);
        assert_eq!(v["c"], serde_json::Value::Null);
    }

    #[test]
    fn repair_closes_truncated_object_and_string() {
        // Truncated mid-string mid-object (token limit) → structurally completed.
        let v = repaired_value("{\"a\":1,\"b\":\"un終");
        assert_eq!(v["a"], 1);
        assert!(v["b"].is_string());
    }

    #[test]
    fn repair_closes_truncated_nested_array() {
        let v = repaired_value("{\"findings\":[{\"severity\":\"high\"");
        assert!(v["findings"].is_array());
        assert_eq!(v["findings"][0]["severity"], "high");
    }

    #[test]
    fn repair_strips_fences_and_prose() {
        let v = repaired_value("Sure! ```json\n{\"a\":1,}\n``` done");
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn repair_leaves_valid_json_semantically_intact() {
        let v = repaired_value("{\"a\":[1,2],\"b\":\"x\",\"c\":true}");
        assert_eq!(v["a"][1], 2);
        assert_eq!(v["c"], true);
    }

    #[test]
    fn repair_does_not_touch_literals_inside_strings() {
        let v = repaired_value("{\"a\":\"True story, None taken\"}");
        assert_eq!(v["a"], "True story, None taken");
    }

    #[test]
    fn repair_invalid_backslash_apostrophe_escape() {
        // `\'` is not a valid JSON escape — must become a literal apostrophe, not stay as `\'`.
        let v = repaired_value("{\"a\":\"it\\'s fine\"}");
        assert_eq!(v["a"], "it's fine");
        let v2 = repaired_value("{'msg':'it\\'s ok'}");
        assert_eq!(v2["msg"], "it's ok");
    }

    #[test]
    fn repair_unescaped_apostrophe_in_single_quoted_string() {
        // The `'` inside "it's" must NOT close the string early.
        let v = repaired_value("{'name':'John','note':'it's fine'}");
        assert_eq!(v["name"], "John");
        assert_eq!(v["note"], "it's fine");
    }
}
