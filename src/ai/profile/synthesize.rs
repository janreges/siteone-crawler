// SiteOne Crawler - AI profile: chapter synthesis phase (P5b)
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Reads the selected pages' markdown (as <content url="..."> blocks) and writes one chapter's body.
// The tool renders the heading, so a leading duplicate heading is stripped. An empty result means the
// pages held nothing relevant → the caller OMITS the chapter (never fabricated).

use once_cell::sync::Lazy;
use regex::Regex;

use crate::ai::normalize::strip_think;
use crate::ai::provider::{ChatMessage, ChatRequest};

use super::promptpack::{self, ChapterSpec};
use super::registry;

/// Whitespace left immediately before sentence punctuation by a deleted span (e.g. "vzduch-voda .").
static SPACE_BEFORE_PUNCT: Lazy<Regex> = Lazy::new(|| Regex::new(r" +([.,;:!?])").unwrap());

/// Unambiguous meta-commentary about the generation process that must never reach the reader. A line
/// containing any of these (case-insensitive) is dropped entirely — the model was told to omit such
/// chapters, not to narrate the omission, but it sometimes leaks the instruction into the prose.
const META_MARKERS: [&str; 8] = [
    "v souladu s instrukc",
    "je vynechána",
    "byla vynechána",
    "in accordance with the instruction",
    "this chapter is omitted",
    "this chapter has been omitted",
    "based on the provided pages",
    "the provided pages contain",
];

/// Concatenate `<content url="/path">markdown</content>` blocks up to `budget_bytes` (keeps at least
/// the first block even if it alone exceeds the budget).
pub fn build_content(blocks: &[(String, String)], budget_bytes: usize) -> String {
    let mut out = String::new();
    for (path, md) in blocks {
        let block = format!("<content url=\"{}\">\n{}\n</content>\n", path, md.trim());
        if !out.is_empty() && out.len() + block.len() > budget_bytes {
            break;
        }
        out.push_str(&block);
    }
    out
}

pub fn build_request(
    chapter: &ChapterSpec,
    site_description: &str,
    content: &str,
    lang: &str,
    out_tokens: u32,
    temperature: f32,
) -> ChatRequest {
    let system = promptpack::render(
        registry::SYNTHESIZE_CHAPTER,
        &[
            ("language", lang),
            ("site_description", site_description),
            ("chapter_heading", &chapter.heading),
            ("chapter_instructions", &chapter.synthesis_instructions),
            ("target_chars", &chapter.target_chars.to_string()),
        ],
    );
    ChatRequest {
        system: Some(system),
        messages: vec![ChatMessage::user(content.to_string())],
        max_tokens: out_tokens,
        temperature,
        json_mode: false,
        json_schema: None,
        schema_name: None,
    }
}

/// Clean the model output: strip reasoning, trim, drop a leading duplicate heading line, then run the
/// full deterministic cleanup (`clean_markdown`).
pub fn parse(raw: &str, heading: &str) -> String {
    let text = strip_think(raw).trim().to_string();
    clean_markdown(&strip_leading_heading(&text, heading))
}

/// Full deterministic cleanup of a chapter/summary body. Idempotent and safe to run BEFORE and AFTER
/// the correction pass: correction deletes spans and can leave debris (empty `**bold**` markers,
/// blanked table cells, stray " ." fragments, orphaned headings, blank-line runs) plus the model
/// occasionally leaks meta-commentary about the generation process — all removed here.
pub(crate) fn clean_markdown(md: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for raw in md.lines() {
        if is_meta_line(raw) {
            continue; // drop a meta-commentary line entirely
        }
        // Remove empty bold markers left when a correction blanked the span between `**`…`**`.
        let mut line = raw.replace("** **", " ").replace("****", "");
        // Tidy whitespace a deletion left before punctuation ("word ." → "word.") — but never on a
        // table row, where " :" is a legitimate part of an alignment separator (`| :--- |`).
        if !is_table_line(&line) {
            line = SPACE_BEFORE_PUNCT.replace_all(&line, "$1").into_owned();
        }
        let t = line.trim();
        // Drop a stray punctuation-only line (e.g. " ." left by a deletion) — but NEVER a table row
        // (a GFM separator `| :--- |` is all punctuation yet must be kept for the table to render).
        let punct_only =
            !t.is_empty() && !t.starts_with('|') && t.chars().all(|c| c.is_ascii_punctuation() || c.is_whitespace());
        let empty_table_row = t.starts_with('|') && t.trim_matches('|').split('|').all(|c| c.trim().is_empty());
        if punct_only || empty_table_row {
            continue;
        }
        lines.push(line);
    }
    tidy_structure(&insert_table_separators(&lines))
}

/// True if a line is unambiguous meta-commentary about the generation process that must not reach the
/// reader (the model was told to omit such chapters, not to narrate the omission).
fn is_meta_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if META_MARKERS.iter().any(|m| lower.contains(m)) {
        return true;
    }
    // Czech "…kapitola … vynech…" with words allowed between (e.g. "je tato kapitola vynechána").
    lower.contains("kapitol") && lower.contains("vynech")
}

fn is_table_line(l: &str) -> bool {
    l.trim_start().starts_with('|')
}

fn is_table_separator(l: &str) -> bool {
    let t = l.trim();
    t.starts_with('|') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) && t.contains('-')
}

/// Insert a missing GFM header-separator row (`| --- | --- |`) after a table header that is followed
/// directly by a data row — without it neither GitHub/Markdown viewers nor the report's own HTML
/// renderer show the block as a table.
fn insert_table_separators(lines: &[String]) -> String {
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 4);
    let mut prev_was_table = false;
    for (i, line) in lines.iter().enumerate() {
        let cur_is_table = is_table_line(line);
        out.push(line.clone());
        if cur_is_table
            && !prev_was_table
            && lines
                .get(i + 1)
                .is_some_and(|n| is_table_line(n) && !is_table_separator(n))
        {
            let cols = line.trim().trim_matches('|').split('|').count().max(1);
            out.push(format!("|{}", " --- |".repeat(cols)));
        }
        prev_was_table = cur_is_table;
    }
    out.join("\n")
}

/// Structural tidy: drop any `#`-heading OR standalone `**bold**` pseudo-heading that has no body
/// before the next heading/end (an orphaned subsection), and collapse runs of 2+ blank lines to one.
fn tidy_structure(md: &str) -> String {
    let is_hash = |l: &str| {
        let n = l.trim_start().chars().take_while(|c| *c == '#').count();
        (1..=6).contains(&n) && l.trim_start()[n..].starts_with(' ')
    };
    // A whole-line bold span used as a subsection heading, e.g. "**Energetika a elektromobilita**".
    let is_bold_heading = |l: &str| {
        let t = l.trim();
        t.len() >= 5
            && t.starts_with("**")
            && t.ends_with("**")
            && !t[2..t.len() - 2].contains("**")
            && !t[2..t.len() - 2].trim().is_empty()
    };
    let is_marker = |l: &str| is_hash(l) || is_bold_heading(l);
    let lines: Vec<&str> = md.lines().collect();
    let mut kept: Vec<&str> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        if is_marker(line) {
            let has_body = lines[i + 1..]
                .iter()
                .take_while(|l| !is_marker(l))
                .any(|l| !l.trim().is_empty());
            if !has_body {
                continue; // orphaned heading/pseudo-heading with no content — drop it
            }
        }
        kept.push(line);
    }
    let mut out = String::with_capacity(md.len());
    let mut blank_run = 0usize;
    for line in kept {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run >= 2 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.trim().to_string()
}

/// Remove a leading `#`..`######` heading line when it merely repeats the chapter heading (the tool
/// renders the heading itself), so the body never double-prints it.
fn strip_leading_heading(md: &str, heading: &str) -> String {
    let trimmed = md.trim_start();
    if let Some(first_line) = trimmed.lines().next() {
        let hashes = first_line.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) {
            let title = first_line[hashes..].trim();
            if title.eq_ignore_ascii_case(heading.trim()) {
                let rest = &trimmed[first_line.len()..];
                return rest.trim_start().to_string();
            }
        }
    }
    md.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter() -> ChapterSpec {
        ChapterSpec {
            id: "03-services".into(),
            heading: "Services".into(),
            target_chars: 8000,
            max_pages: 20,
            selection_focus: "services pages".into(),
            synthesis_instructions: "Describe the services.".into(),
        }
    }

    #[test]
    fn build_content_wraps_and_budgets() {
        let blocks = vec![
            ("/a".to_string(), "AAA".to_string()),
            ("/b".to_string(), "B".repeat(10_000)),
        ];
        let out = build_content(&blocks, 100);
        assert!(out.contains("<content url=\"/a\">"));
        assert!(!out.contains("/b"), "second block exceeds the budget");
    }

    #[test]
    fn request_injects_chapter_instructions_and_language() {
        let req = build_request(&chapter(), "A shop.", "<content>x</content>", "cs", 4000, 0.0);
        let sys = req.system.unwrap();
        assert!(sys.contains("Describe the services."));
        assert!(sys.contains("'cs'"));
        assert!(sys.contains("A shop."));
    }

    #[test]
    fn parse_strips_reasoning_and_duplicate_heading() {
        let raw = "<think>plan</think>\n## Services\n\nWe offer A and B.";
        assert_eq!(parse(raw, "Services"), "We offer A and B.");
        // A different heading is left intact.
        let raw2 = "## Our team\n\nBody.";
        assert_eq!(parse(raw2, "Services"), "## Our team\n\nBody.");
    }

    #[test]
    fn tidy_drops_orphaned_headings_and_collapses_blanks() {
        // "### Empty" has no body before the next heading → dropped; the 4 blank lines collapse to 1.
        let raw = "### Kept\n\nBody text.\n\n### Empty\n\n\n\n### Also empty at end";
        let out = parse(raw, "Chapter");
        assert!(out.contains("### Kept"));
        assert!(out.contains("Body text."));
        assert!(!out.contains("### Empty"), "orphaned heading must be dropped");
        assert!(
            !out.contains("### Also empty"),
            "trailing orphaned heading must be dropped"
        );
        assert!(!out.contains("\n\n\n"), "3+ blank lines must be collapsed");
    }

    #[test]
    fn clean_removes_correction_debris_and_meta() {
        let raw = "Skutečná věta o produktu.\n\nV souladu s instrukcemi je tato kapitola vynechána.\n\n- Nabídka vzduch-voda .\n\nProdukt **** a další.\n .";
        let out = clean_markdown(raw);
        assert!(out.contains("Skutečná věta o produktu."));
        assert!(
            !out.to_lowercase().contains("v souladu s instrukc"),
            "meta-commentary line must be dropped"
        );
        assert!(!out.contains("****"), "empty bold markers must be removed");
        assert!(out.contains("vzduch-voda."), "space-before-punctuation must be tidied");
        assert!(
            !out.contains("\n .\n") && !out.ends_with(" ."),
            "stray punctuation line removed"
        );
    }

    #[test]
    fn clean_is_idempotent_and_keeps_real_bold() {
        let raw = "## Kept\n\nBody with **bold** kept.\n\nvzduch-voda .";
        let once = clean_markdown(raw);
        assert_eq!(once, clean_markdown(&once), "cleanup must be idempotent");
        assert!(once.contains("**bold**"), "legitimate bold must be preserved");
    }

    #[test]
    fn clean_preserves_and_inserts_table_separators() {
        // An existing separator row (all punctuation) must NOT be stripped.
        let with_sep = "| Plan | Price |\n| :--- | :--- |\n| Free | 0 |";
        let out = clean_markdown(with_sep);
        assert!(out.contains("| :--- | :--- |"), "existing separator row must be kept");
        // A table missing its separator gets one inserted after the header.
        let no_sep = "| Plan | Price |\n| Free | 0 |";
        let out2 = clean_markdown(no_sep);
        let lines: Vec<&str> = out2.lines().collect();
        assert!(
            is_table_separator(lines[1]),
            "a separator row must be inserted after the header: {out2:?}"
        );
        assert!(out2.contains("| Free | 0 |"));
    }

    #[test]
    fn clean_drops_orphaned_bold_pseudo_heading_and_catches_kapitola_meta() {
        let raw = "**Sekce s obsahem**\n\nReálný text sekce.\n\n**Prázdná sekce**\n\n## Konec\n\nText.\n\nJe tato kapitola vynechána.";
        let out = clean_markdown(raw);
        assert!(out.contains("**Sekce s obsahem**"), "bold heading WITH body kept");
        assert!(out.contains("Reálný text sekce."));
        assert!(
            !out.contains("**Prázdná sekce**"),
            "orphaned bold pseudo-heading dropped"
        );
        assert!(
            !out.to_lowercase().contains("kapitola vynech"),
            "'kapitola ... vynechána' meta must be dropped even with words between"
        );
    }
}
