// SiteOne Crawler - AI profile: correction pass
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Applies model-proposed {from,to} edits to a produced chapter/summary MECHANICALLY and safely.
// `from` has three forms: an exact unique substring, "regex:PATTERN", or an ellipsis "A...B" span.
// Guards prevent a single edit or the cumulative deletions from wiping most of the text; overlapping
// edits are dropped; splices are applied by descending offset so earlier offsets never drift.

use regex::RegexBuilder;
use serde::Deserialize;

use crate::ai::normalize::{normalize_json_array, repair_json};

const MIN_FROM_LEN: usize = 12;
const MAX_EDITS: usize = 200;
const REGEX_SIZE_LIMIT: usize = 1 << 20;
/// Reject a single edit whose span covers more than this fraction of the text.
const MAX_SPAN_FRACTION: f64 = 0.75;
/// Reject deletions once cumulative deleted bytes would exceed this fraction of the text.
const MAX_CUMULATIVE_DELETE_FRACTION: f64 = 0.70;
/// Each side of an "A...B" ellipsis anchor must be at least this many characters.
const MIN_ELLIPSIS_ANCHOR: usize = 10;

#[derive(Debug, Clone, Deserialize)]
pub struct Edit {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CorrectionReport {
    pub proposed: usize,
    pub applied: usize,
    pub skipped_no_match: usize,
    pub skipped_ambiguous: usize,
    pub skipped_overlap: usize,
    pub skipped_guard: usize,
}

#[derive(Debug, Deserialize)]
struct EditsWrapper {
    #[serde(default)]
    edits: Vec<Edit>,
}

pub fn parse_edits(raw: &str) -> Result<Vec<Edit>, String> {
    let normalized = normalize_json_array(raw);
    let mut list = parse_any(&normalized)
        .or_else(|| parse_any(&repair_json(raw)))
        .ok_or_else(|| "correction response is not valid JSON".to_string())?;
    // A regex/ellipsis `from` can legitimately be short; only exact spans need the length floor.
    list.retain(|e| is_regex(&e.from) || is_ellipsis(&e.from) || e.from.trim().len() >= MIN_FROM_LEN);
    list.truncate(MAX_EDITS);
    Ok(list)
}

fn parse_any(text: &str) -> Option<Vec<Edit>> {
    if let Ok(list) = serde_json::from_str::<Vec<Edit>>(text) {
        return Some(list);
    }
    if text.trim_start().starts_with('{')
        && text.contains("\"edits\"")
        && let Ok(w) = serde_json::from_str::<EditsWrapper>(text)
    {
        return Some(w.edits);
    }
    None
}

fn is_regex(from: &str) -> bool {
    from.starts_with("regex:")
}

fn is_ellipsis(from: &str) -> bool {
    if let Some((a, b)) = from.split_once("...") {
        a.chars().count() >= MIN_ELLIPSIS_ANCHOR && b.chars().count() >= MIN_ELLIPSIS_ANCHOR
    } else {
        false
    }
}

enum SpanErr {
    NoMatch,
    Ambiguous,
    Invalid,
}

/// Resolve a `from` to a unique byte span [start,end) in `text`, honoring the three forms.
fn resolve_span(text: &str, from: &str) -> Result<(usize, usize), SpanErr> {
    if let Some(pattern) = from.strip_prefix("regex:") {
        let re = RegexBuilder::new(pattern)
            .size_limit(REGEX_SIZE_LIMIT)
            .build()
            .map_err(|_| SpanErr::Invalid)?;
        let mut it = re.find_iter(text);
        let first = it.next().ok_or(SpanErr::NoMatch)?;
        if it.next().is_some() {
            return Err(SpanErr::Ambiguous);
        }
        return Ok((first.start(), first.end()));
    }
    if let Some((a, b)) = from.split_once("...")
        && a.chars().count() >= MIN_ELLIPSIS_ANCHOR
        && b.chars().count() >= MIN_ELLIPSIS_ANCHOR
    {
        let a_hits: Vec<usize> = text.match_indices(a).map(|(i, _)| i).collect();
        if a_hits.is_empty() {
            return Err(SpanErr::NoMatch);
        }
        if a_hits.len() > 1 {
            return Err(SpanErr::Ambiguous);
        }
        let start = a_hits[0];
        let after = start + a.len();
        let b_rel = text[after..].find(b).ok_or(SpanErr::NoMatch)?;
        return Ok((start, after + b_rel + b.len()));
    }
    // Exact substring, must be unique.
    let hits: Vec<usize> = text.match_indices(from).map(|(i, _)| i).collect();
    match hits.len() {
        0 => Err(SpanErr::NoMatch),
        1 => Ok((hits[0], hits[0] + from.len())),
        _ => Err(SpanErr::Ambiguous),
    }
}

pub fn apply(text: &str, edits: &[Edit]) -> (String, CorrectionReport) {
    let mut report = CorrectionReport {
        proposed: edits.len(),
        ..Default::default()
    };
    let max_span = (text.len() as f64 * MAX_SPAN_FRACTION) as usize;

    // Resolve each edit to a span, applying the single-span guard.
    let mut spans: Vec<(usize, usize, &str)> = Vec::new();
    for e in edits {
        if e.from.trim().is_empty() {
            report.skipped_no_match += 1;
            continue;
        }
        match resolve_span(text, &e.from) {
            Ok((s, en)) if en - s > max_span => report.skipped_guard += 1,
            Ok((s, en)) => spans.push((s, en, e.to.as_str())),
            Err(SpanErr::NoMatch) | Err(SpanErr::Invalid) => report.skipped_no_match += 1,
            Err(SpanErr::Ambiguous) => report.skipped_ambiguous += 1,
        }
    }

    // Drop overlaps (ascending start, first wins).
    spans.sort_by_key(|(start, _, _)| *start);
    let mut accepted: Vec<(usize, usize, &str)> = Vec::new();
    let mut last_end = 0usize;
    let mut first = true;
    for span in spans {
        if !first && span.0 < last_end {
            report.skipped_overlap += 1;
            continue;
        }
        last_end = span.1;
        first = false;
        accepted.push(span);
    }

    // Cumulative deletion guard: skip deletions once total deleted bytes would exceed the cap.
    let delete_cap = (text.len() as f64 * MAX_CUMULATIVE_DELETE_FRACTION) as usize;
    let mut deleted = 0usize;
    let mut final_spans: Vec<(usize, usize, &str)> = Vec::new();
    for (s, en, to) in accepted {
        if to.is_empty() {
            if deleted + (en - s) > delete_cap {
                report.skipped_guard += 1;
                continue;
            }
            deleted += en - s;
        }
        final_spans.push((s, en, to));
    }

    // Splice descending so earlier offsets stay valid.
    report.applied = final_spans.len();
    final_spans.sort_by_key(|b| std::cmp::Reverse(b.0));
    let mut out = text.to_string();
    for (s, en, to) in final_spans {
        out.replace_range(s..en, to);
    }
    (out, report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(from: &str, to: &str) -> Edit {
        Edit {
            from: from.into(),
            to: to.into(),
            note: String::new(),
        }
    }

    #[test]
    fn exact_unique_replace() {
        let (out, r) = apply(
            "Company founded in 2010 exactly.",
            &[edit("founded in 2010", "founded in 2011")],
        );
        assert_eq!(out, "Company founded in 2011 exactly.");
        assert_eq!(r.applied, 1);
    }

    #[test]
    fn exact_ambiguous_is_skipped() {
        let (out, r) = apply(
            "repeated phrase and repeated phrase here",
            &[edit("repeated phrase", "X")],
        );
        assert_eq!(out, "repeated phrase and repeated phrase here");
        assert_eq!(r.skipped_ambiguous, 1);
    }

    #[test]
    fn regex_form_replaces_single_match() {
        let (out, r) = apply("Order 12345 shipped.", &[edit(r"regex:\d{5}", "REDACTED")]);
        assert_eq!(out, "Order REDACTED shipped.");
        assert_eq!(r.applied, 1);
    }

    #[test]
    fn regex_invalid_and_multi_match_are_skipped() {
        let (_o, r1) = apply("aaa", &[edit(r"regex:(", "x")]);
        assert_eq!(r1.skipped_no_match, 1);
        let (_o2, r2) = apply("a1 a2 a3", &[edit(r"regex:a\d", "x")]);
        assert_eq!(r2.skipped_ambiguous, 1);
    }

    #[test]
    fn ellipsis_deletes_whole_span() {
        let text = "Intro paragraph. START_MARKER middle junk to remove END_MARKER. Outro paragraph.";
        let (out, r) = apply(text, &[edit("START_MARKER...END_MARKER", "")]);
        assert_eq!(out, "Intro paragraph. . Outro paragraph.");
        assert_eq!(r.applied, 1);
    }

    #[test]
    fn single_span_guard_blocks_huge_edit() {
        let text = "small text body here now"; // len 24; 75% ~= 18
        let (out, r) = apply(text, &[edit("regex:.*", "")]);
        assert_eq!(out, text);
        assert_eq!(r.skipped_guard, 1);
    }

    #[test]
    fn cumulative_delete_guard_stops_wipeout() {
        // Three deletions each ~30% of the text; the third pushes past the 70% cap and is skipped.
        let text = "AAAAAAAAAA BBBBBBBBBB CCCCCCCCCC"; // 31 chars
        let edits = vec![edit("AAAAAAAAAA", ""), edit("BBBBBBBBBB", ""), edit("CCCCCCCCCC", "")];
        let (_out, r) = apply(text, &edits);
        assert_eq!(r.applied, 2);
        assert_eq!(r.skipped_guard, 1);
    }

    #[test]
    fn overlapping_edits_first_wins() {
        let (_out, r) = apply(
            "alpha beta gamma delta",
            &[edit("alpha beta gamma", "X"), edit("beta gamma delta", "Y")],
        );
        assert_eq!(r.applied, 1);
        assert_eq!(r.skipped_overlap, 1);
    }

    #[test]
    fn parse_accepts_wrapper_and_forms() {
        let raw = r#"{"edits":[{"from":"regex:\\d+","to":"N"},{"from":"exact long span here","to":""},{"from":"short","to":"x"}]}"#;
        let edits = parse_edits(raw).unwrap();
        // "short" (<12, not regex/ellipsis) dropped; the regex and the exact span kept.
        assert_eq!(edits.len(), 2);
    }
}
