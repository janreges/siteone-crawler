// SiteOne Crawler - brand elaborate: correction pass
// (c) Jan Reges <jan.reges@siteone.cz>
//
// The optional correction pass asks the model to return a JSON array of `{from,to}` edits against
// the finished elaborate (fix typos/artifacts and delete statements not supported by the sources).
// The crawler applies them MECHANICALLY and safely: each `from` must match EXACTLY ONCE (0 → skip;
// >1 → skip, never replace-all), overlapping edits are dropped, and edits are spliced by start
// offset DESCENDING so earlier offsets never drift. Nothing is ever fabricated — only spans the
// model quoted verbatim from the elaborate can be changed or deleted.

use serde::Deserialize;

use crate::ai::normalize::{normalize_json_array, repair_json};

/// Minimum length of a `from` span — short spans are ambiguous and rejected outright.
const MIN_FROM_LEN: usize = 12;
/// Hard cap on the number of edits considered.
const MAX_EDITS: usize = 200;

#[derive(Debug, Clone, Deserialize)]
pub struct Edit {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub reason: String,
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
}

#[derive(Debug, Deserialize)]
struct EditsWrapper {
    #[serde(default)]
    edits: Vec<Edit>,
}

/// Parse the correction response. Accepts a bare array `[...]` or a `{"edits":[...]}` wrapper, with a
/// `repair_json` fallback. Drops edits whose `from` is shorter than `MIN_FROM_LEN`; caps to `MAX_EDITS`.
pub fn parse_edits(raw: &str) -> Result<Vec<Edit>, String> {
    // Array-first: the edits are a JSON array (possibly inside a `{"edits":[...]}` wrapper, whose
    // inner array `normalize_json_array` extracts). Object-first normalization would wrongly grab
    // the first inner edit object.
    let normalized = normalize_json_array(raw);
    let mut list = parse_any(&normalized)
        .or_else(|| parse_any(&repair_json(raw)))
        .ok_or_else(|| "correction response is not valid JSON".to_string())?;
    list.retain(|e| e.from.trim().len() >= MIN_FROM_LEN);
    list.truncate(MAX_EDITS);
    Ok(list)
}

fn parse_any(text: &str) -> Option<Vec<Edit>> {
    if let Ok(list) = serde_json::from_str::<Vec<Edit>>(text) {
        return Some(list);
    }
    // Only accept an object wrapper that actually carries an "edits" array (never treat an arbitrary
    // object as an empty edit list).
    if text.trim_start().starts_with('{')
        && text.contains("\"edits\"")
        && let Ok(w) = serde_json::from_str::<EditsWrapper>(text)
    {
        return Some(w.edits);
    }
    None
}

/// Apply edits to `text`, deterministically and safely. Returns the corrected text and a report.
pub fn apply_edits(text: &str, edits: &[Edit]) -> (String, CorrectionReport) {
    let mut report = CorrectionReport {
        proposed: edits.len(),
        ..Default::default()
    };

    // Resolve each edit to a unique byte span in the ORIGINAL text.
    let mut spans: Vec<(usize, usize, &str)> = Vec::new();
    for e in edits {
        let from = e.from.as_str();
        if from.is_empty() {
            report.skipped_no_match += 1;
            continue;
        }
        let matches: Vec<usize> = text.match_indices(from).map(|(i, _)| i).collect();
        match matches.len() {
            0 => report.skipped_no_match += 1,
            1 => spans.push((matches[0], matches[0] + from.len(), e.to.as_str())),
            _ => report.skipped_ambiguous += 1,
        }
    }

    // Drop overlaps: sort by start ascending, keep the first, skip any that overlaps an accepted one.
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

    // Splice DESCENDING so earlier offsets remain valid as we mutate.
    let mut out = text.to_string();
    report.applied = accepted.len();
    accepted.sort_by_key(|b| std::cmp::Reverse(b.0));
    for (start, end, to) in accepted {
        out.replace_range(start..end, to);
    }
    (out, report)
}

/// Whether the support-vs-source check fits one call or must be split per section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrectionMode {
    Combined,
    SplitTyposPlusSectionedSupport,
}

/// Source material + the elaborate must both fit one window for a combined pass.
pub fn plan_correction(material_kb: usize, elaborate_kb: usize) -> CorrectionMode {
    if material_kb + elaborate_kb <= 120 {
        CorrectionMode::Combined
    } else {
        CorrectionMode::SplitTyposPlusSectionedSupport
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(from: &str, to: &str) -> Edit {
        Edit {
            from: from.into(),
            to: to.into(),
            reason: "typo".into(),
            note: String::new(),
        }
    }

    #[test]
    fn applies_unique_match() {
        let (out, r) = apply_edits(
            "Firma byla zalozena v roce 2010.",
            &[edit("zalozena v roce 2010", "založena v roce 2010")],
        );
        assert_eq!(out, "Firma byla založena v roce 2010.");
        assert_eq!(r.applied, 1);
    }

    #[test]
    fn skips_zero_and_multi_match() {
        // Absent from → no match.
        let (out, r) = apply_edits("nic tady není k opravě", &[edit("neexistující fráze zde", "x")]);
        assert_eq!(out, "nic tady není k opravě");
        assert_eq!(r.skipped_no_match, 1);
        // Duplicated from → ambiguous, unchanged (never replace-all).
        let (out2, r2) = apply_edits("opakovaná fráze a opakovaná fráze zde", &[edit("opakovaná fráze", "X")]);
        assert_eq!(out2, "opakovaná fráze a opakovaná fráze zde");
        assert_eq!(r2.skipped_ambiguous, 1);
    }

    #[test]
    fn empty_to_deletes_span() {
        let (out, r) = apply_edits(
            "Dobrá věta. Halucinovaná věta zde. Konec.",
            &[edit("Halucinovaná věta zde. ", "")],
        );
        assert_eq!(out, "Dobrá věta. Konec.");
        assert_eq!(r.applied, 1);
    }

    #[test]
    fn overlapping_edits_first_wins() {
        let text = "alpha beta gamma delta";
        let edits = vec![edit("alpha beta gamma", "X"), edit("beta gamma delta", "Y")];
        let (_out, r) = apply_edits(text, &edits);
        assert_eq!(r.applied, 1);
        assert_eq!(r.skipped_overlap, 1);
    }

    #[test]
    fn descending_splice_no_offset_drift() {
        // Two non-overlapping edits; the later-in-text one is applied without shifting the earlier.
        let text = "prvni chybna cast a druha chybna cast";
        let edits = vec![
            edit("prvni chybna cast", "PRVNI OK"),
            edit("druha chybna cast", "DRUHA OK"),
        ];
        let (out, r) = apply_edits(text, &edits);
        assert_eq!(out, "PRVNI OK a DRUHA OK");
        assert_eq!(r.applied, 2);
    }

    #[test]
    fn utf8_boundaries_safe() {
        let text = "Příliš žluťoučký kůň úpěl ďábelské ódy dokola.";
        let edits = vec![edit("žluťoučký kůň úpěl", "žlutý kůň zpíval")];
        let (out, r) = apply_edits(text, &edits);
        assert!(out.contains("žlutý kůň zpíval"));
        assert_eq!(r.applied, 1);
    }

    #[test]
    fn parse_accepts_bare_array_wrapper_and_dirty() {
        let bare = r#"[{"from":"nejaka dlouha fraze","to":"opravena","reason":"typo"}]"#;
        assert_eq!(parse_edits(bare).unwrap().len(), 1);
        let wrapper = r#"{"edits":[{"from":"jina dlouha fraze zde","to":""}]}"#;
        assert_eq!(parse_edits(wrapper).unwrap().len(), 1);
        // Trailing comma → repair; short `from` dropped.
        let dirty =
            "```json\n[{\"from\":\"kratke\",\"to\":\"x\"},{\"from\":\"dostatecne dlouha fraze\",\"to\":\"y\"},]\n```";
        let edits = parse_edits(dirty).unwrap();
        assert_eq!(edits.len(), 1); // "kratke" (<12) dropped
        assert_eq!(edits[0].to, "y");
    }

    #[test]
    fn plan_correction_thresholds() {
        assert_eq!(plan_correction(40, 40), CorrectionMode::Combined);
        assert_eq!(plan_correction(120, 40), CorrectionMode::SplitTyposPlusSectionedSupport);
    }
}
