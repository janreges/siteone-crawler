// SiteOne Crawler - AI profile: chapter synthesis phase (P5b)
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Reads the selected pages' markdown (as <content url="..."> blocks) and writes one chapter's body.
// The tool renders the heading, so a leading duplicate heading is stripped. An empty result means the
// pages held nothing relevant → the caller OMITS the chapter (never fabricated).

use crate::ai::normalize::strip_think;
use crate::ai::provider::{ChatMessage, ChatRequest};

use super::promptpack::{self, ChapterSpec};
use super::registry;

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

/// Clean the model output: strip reasoning, trim, drop a leading duplicate heading line, then tidy
/// orphaned empty subsections and excess blank lines.
pub fn parse(raw: &str, heading: &str) -> String {
    let text = strip_think(raw).trim().to_string();
    tidy_markdown(&strip_leading_heading(&text, heading))
}

/// Deterministic markdown tidy: drop any heading line that has no body before the next heading or the
/// end (an "orphaned" subsection the model emitted but never filled), and collapse runs of 3+ blank
/// lines to a single blank line. Keeps everything else byte-for-byte.
fn tidy_markdown(md: &str) -> String {
    let is_heading = |l: &str| {
        let n = l.trim_start().chars().take_while(|c| *c == '#').count();
        (1..=6).contains(&n) && l.trim_start()[n..].starts_with(' ')
    };
    let lines: Vec<&str> = md.lines().collect();
    let mut kept: Vec<&str> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        if is_heading(line) {
            // Look ahead: is there any non-blank, non-heading content before the next heading/end?
            let has_body = lines[i + 1..]
                .iter()
                .take_while(|l| !is_heading(l))
                .any(|l| !l.trim().is_empty());
            if !has_body {
                continue; // orphaned heading with no content — drop it
            }
        }
        kept.push(line);
    }
    // Collapse 3+ consecutive blank lines to one blank line.
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
}
