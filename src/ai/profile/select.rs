// SiteOne Crawler - AI profile: per-chapter page selection
// (c) Jan Reges <jan.reges@siteone.cz>
//
// The LLM only ever sees a NUMBERED list and returns integer ids — hallucinated URLs are impossible.
// `enforce` then deterministically drops unknown ids, restores rank (universe) order, caps the count,
// and greedily fits the char budget from the tail.

use crate::ai::normalize::{normalize_json_array, repair_json};
use crate::ai::provider::{ChatMessage, ChatRequest};

use super::ProfilePage;

/// Render the universe as "id. path — description (N chars)" with 1-based ids.
pub fn render_numbered(pages: &[ProfilePage]) -> String {
    let mut out = String::with_capacity(pages.len() * 120);
    for (i, p) in pages.iter().enumerate() {
        let desc = if p.description.is_empty() {
            "(no description)"
        } else {
            p.description.as_str()
        };
        out.push_str(&format!("{}. {} — {} ({} chars)\n", i + 1, p.path, desc, p.size_chars));
    }
    out
}

/// Parse a JSON array of integers (tolerating code fences / wrappers via normalize + repair).
pub fn parse_ids(raw: &str) -> Result<Vec<usize>, String> {
    let normalized = normalize_json_array(raw);
    let parsed = serde_json::from_str::<Vec<i64>>(&normalized)
        .or_else(|_| serde_json::from_str::<Vec<i64>>(&repair_json(raw)))
        .map_err(|e| format!("selection response is not a JSON array of integers: {e}"))?;
    Ok(parsed.into_iter().filter(|n| *n >= 1).map(|n| n as usize).collect())
}

/// Convert 1-based model ids to 0-based indices, drop unknowns, dedup, restore rank order, cap the
/// count, and greedily fit the char budget by dropping from the tail (least-important first).
pub fn enforce(ids: &[usize], pages: &[ProfilePage], max_pages: usize, budget_chars: usize) -> Vec<usize> {
    let mut seen = std::collections::HashSet::new();
    let mut idx: Vec<usize> = ids
        .iter()
        .filter_map(|&id| id.checked_sub(1))
        .filter(|&i| i < pages.len())
        .filter(|&i| seen.insert(i))
        .collect();
    // Restore rank (universe) order so we keep the more important pages when trimming.
    idx.sort_unstable();
    idx.truncate(max_pages.max(1));
    // Greedily fit the char budget: drop from the tail until the sum fits (keep at least one).
    while idx.len() > 1 {
        let total: usize = idx.iter().map(|&i| pages[i].size_chars).sum();
        if total <= budget_chars {
            break;
        }
        idx.pop();
    }
    idx
}

pub fn build_request(system: &str, numbered: &str, out_tokens: u32, temperature: f32) -> ChatRequest {
    ChatRequest {
        system: Some(system.to_string()),
        messages: vec![ChatMessage::user(format!("<pages>\n{numbered}</pages>"))],
        max_tokens: out_tokens,
        temperature,
        json_mode: true,
        json_schema: None,
        schema_name: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(path: &str, size: usize) -> ProfilePage {
        ProfilePage {
            uq_id: path.to_string(),
            url: format!("https://x{path}"),
            path: path.to_string(),
            description: format!("desc of {path}"),
            size_chars: size,
        }
    }

    fn universe() -> Vec<ProfilePage> {
        vec![
            page("/", 100),
            page("/about", 200),
            page("/services", 300),
            page("/contact", 50),
        ]
    }

    #[test]
    fn render_is_numbered_from_one() {
        let r = render_numbered(&universe());
        assert!(r.starts_with("1. / — desc of / (100 chars)"));
        assert!(r.contains("\n3. /services — desc of /services (300 chars)"));
    }

    #[test]
    fn parse_accepts_array_and_fenced() {
        assert_eq!(parse_ids("[1, 3, 4]").unwrap(), vec![1, 3, 4]);
        assert_eq!(parse_ids("```json\n[2,2,5]\n```").unwrap(), vec![2, 2, 5]);
        assert!(parse_ids("not json").is_err());
    }

    #[test]
    fn enforce_drops_unknown_dedups_and_orders() {
        // ids: 3 (services), 99 (unknown), 1 (home), 3 (dup) → indices {2,0} → ordered [0,2].
        let out = enforce(&[3, 99, 1, 3], &universe(), 10, 1_000_000);
        assert_eq!(out, vec![0, 2]);
    }

    #[test]
    fn enforce_caps_count() {
        let out = enforce(&[1, 2, 3, 4], &universe(), 2, 1_000_000);
        assert_eq!(out, vec![0, 1]);
    }

    #[test]
    fn enforce_fits_char_budget_from_tail() {
        // Sizes 100,200,300 = 600; budget 350 → drop tail (index 2, size 300) leaving [0,1]=300.
        let out = enforce(&[1, 2, 3], &universe(), 10, 350);
        assert_eq!(out, vec![0, 1]);
    }

    #[test]
    fn enforce_keeps_at_least_one_even_over_budget() {
        let out = enforce(&[3], &universe(), 10, 10);
        assert_eq!(out, vec![2]);
    }
}
