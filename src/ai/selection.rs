// SiteOne Crawler - AI page selection & importance ranking
// (c) Jan Reges <jan.reges@siteone.cz>
//
// The "spend firewall": cheap filters (HTML 200, internal, include/exclude masks) run
// before ranking, then the highest-ranked pages up to --ai-max-pages are kept.

use std::collections::HashMap;

// Use the SAME engine the CLI validates `--ai-include`/`--ai-exclude` with (fancy_regex /
// PCRE). Compiling here with a different engine (e.g. the `regex` crate) would silently drop a
// pattern that uses PCRE-only features but passed CLI validation — making the privacy/cost
// filter fail OPEN and potentially sending excluded pages to the LLM.
use fancy_regex::Regex;

use crate::result::status::Status;
use crate::result::visited_url::{SOURCE_INIT_URL, SOURCE_SITEMAP, VisitedUrl};
use crate::types::ContentTypeId;

/// A page selected for AI analysis, with its importance score (descending = more important).
#[derive(Debug, Clone)]
pub struct RankedPage {
    pub uq_id: String,
    pub url: String,
    pub score: f64,
}

/// Outcome of the selection step (used for the dry-run preview too).
pub struct Selection {
    pub selected: Vec<RankedPage>,
    pub total_candidates_before_cap: usize,
    pub total_html_pages: usize,
    pub total_eligible_before_masks: usize,
    pub excluded_by_mask: usize,
}

/// Filter + rank crawled pages for AI analysis.
pub fn select_pages(status: &Status, include: &[String], exclude: &[String], max_pages: usize) -> Selection {
    let visited = status.get_visited_urls();

    let include_res = compile(include, "include");
    let exclude_res = compile(exclude, "exclude");

    let total_html_pages = visited
        .iter()
        .filter(|url| url.content_type == ContentTypeId::Html)
        .count();

    // Eligible pages: internal/allowed HTML with HTTP 200, before user include/exclude masks.
    let eligible_pages: Vec<&VisitedUrl> = visited.iter().filter(|u| eligible_page(u)).collect();
    let total_eligible_before_masks = eligible_pages.len();

    let mut excluded_by_mask = 0usize;
    let candidates: Vec<&VisitedUrl> = eligible_pages
        .into_iter()
        .filter(|u| {
            // Fail CLOSED on a match error (catastrophic backtracking etc.): an un-evaluatable
            // include drops the page (not "included"); an un-evaluatable exclude drops it too
            // ("excluded"). Either way we never send a page we could not confidently clear.
            if !include_res.is_empty() && !include_res.iter().any(|re| re.is_match(&u.url).unwrap_or(false)) {
                excluded_by_mask += 1;
                return false;
            }
            if exclude_res.iter().any(|re| re.is_match(&u.url).unwrap_or(true)) {
                excluded_by_mask += 1;
                return false;
            }
            true
        })
        .collect();

    let total_candidates_before_cap = candidates.len();

    // Build first-discovery tree structures for ranking.
    let init_uq = visited
        .iter()
        .find(|u| u.source_attr == SOURCE_INIT_URL)
        .map(|u| u.uq_id.clone());

    // depth via BFS over first-discovery edges (child.source_uq_id -> parent).
    let depths = compute_depths(&visited, init_uq.as_deref());

    // fanout(P) = how many pages were first discovered from P (hub/nav proxy).
    let mut fanout: HashMap<String, u32> = HashMap::new();
    for u in &visited {
        *fanout.entry(u.source_uq_id.clone()).or_insert(0) += 1;
    }

    let mut ranked: Vec<RankedPage> = candidates
        .iter()
        .map(|u| {
            let score = score_page(u, init_uq.as_deref(), &depths, &fanout);
            RankedPage {
                uq_id: u.uq_id.clone(),
                url: u.url.clone(),
                score,
            }
        })
        .collect();

    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(max_pages);

    Selection {
        selected: ranked,
        total_candidates_before_cap,
        total_html_pages,
        total_eligible_before_masks,
        excluded_by_mask,
    }
}

fn eligible_page(url: &VisitedUrl) -> bool {
    !url.is_external && url.status_code == 200 && url.is_allowed_for_crawling && url.content_type == ContentTypeId::Html
}

/// Compile include/exclude patterns. A pattern that validated at the CLI (same engine) always
/// compiles here; if one somehow does not, warn LOUDLY rather than silently dropping it — a
/// silently ignored exclude would let pages through to the LLM.
fn compile(patterns: &[String], kind: &str) -> Vec<Regex> {
    patterns
        .iter()
        .map(|p| match Regex::new(p) {
            Ok(re) => re,
            Err(e) => {
                eprintln!(
                    "{}",
                    crate::utils::get_color_text(
                        &format!(
                            "AI --ai-{} pattern '{}' could not be compiled; selection FAILED CLOSED: {}",
                            kind, p, e
                        ),
                        "yellow",
                        true,
                    )
                );
                // Programmatic callers can bypass CLI validation. Preserve the privacy/cost
                // invariant there too: an invalid include matches nothing; an invalid exclude
                // matches everything.
                let sentinel = if kind == "include" { r"(?!)" } else { r"(?s:.*)" };
                Regex::new(sentinel).expect("internal fail-closed regex must compile")
            }
        })
        .collect()
}

fn compute_depths(visited: &[VisitedUrl], init_uq: Option<&str>) -> HashMap<String, u32> {
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    for u in visited {
        children
            .entry(u.source_uq_id.clone())
            .or_default()
            .push(u.uq_id.clone());
    }
    let mut depths: HashMap<String, u32> = HashMap::new();
    if let Some(root) = init_uq {
        let mut queue = std::collections::VecDeque::new();
        depths.insert(root.to_string(), 0);
        queue.push_back(root.to_string());
        while let Some(node) = queue.pop_front() {
            let d = *depths.get(&node).unwrap_or(&0);
            if let Some(kids) = children.get(&node) {
                for kid in kids {
                    if !depths.contains_key(kid) {
                        depths.insert(kid.clone(), d + 1);
                        queue.push_back(kid.clone());
                    }
                }
            }
        }
    }
    depths
}

fn score_page(
    u: &VisitedUrl,
    init_uq: Option<&str>,
    depths: &HashMap<String, u32>,
    fanout: &HashMap<String, u32>,
) -> f64 {
    let depth = *depths.get(&u.uq_id).unwrap_or(&99);

    // Homepage itself, or linked directly from homepage.
    let homepage_linked = Some(u.uq_id.as_str()) == init_uq || Some(u.source_uq_id.as_str()) == init_uq || depth <= 1;
    let homepage_score = if homepage_linked { 40.0 } else { 0.0 };

    // Click-depth: 40 at depth 0, 20 at 1, 13 at 2, ...
    let depth_score = 40.0 / (1.0 + depth as f64);

    // Fanout proxy for hub/nav importance.
    let fo = *fanout.get(&u.uq_id).unwrap_or(&0) as f64;
    let fanout_score = (5.0 * (1.0 + fo).log2()).min(25.0);

    // Presence in sitemap.
    let sitemap_score = if u.source_attr == SOURCE_SITEMAP { 15.0 } else { 0.0 };

    // URL path shallowness.
    let segments = url::Url::parse(&u.url)
        .ok()
        .map(|p| p.path().trim_matches('/').split('/').filter(|s| !s.is_empty()).count())
        .unwrap_or(3) as f64;
    let shallow_score = (10.0 - 2.0 * segments).max(0.0);

    homepage_score + depth_score + fanout_score + sitemap_score + shallow_score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_accepts_pcre_lookahead_like_the_cli() {
        // A PCRE negative-lookahead validates at the CLI (fancy_regex) but the `regex` crate
        // cannot compile it — before the fix it was silently dropped, failing the filter OPEN.
        let res = compile(&[r"^(?!.*/press/).*$".to_string()], "exclude");
        assert_eq!(
            res.len(),
            1,
            "PCRE lookahead pattern must compile with the same engine as the CLI"
        );
        let re = &res[0];
        assert!(re.is_match("https://x/products/").unwrap()); // not /press/ → matches
        assert!(!re.is_match("https://x/press/release").unwrap()); // /press/ → excluded by lookahead
    }

    #[test]
    fn invalid_pattern_fails_closed() {
        // An unbalanced include is invalid in every engine; the sentinel matches nothing so a
        // programmatic caller cannot accidentally send every page to the LLM.
        let res = compile(&["(unclosed".to_string()], "include");
        assert_eq!(res.len(), 1);
        assert!(!res[0].is_match("https://example.test/private").unwrap());
    }

    #[test]
    fn external_html_is_not_eligible_even_when_external_crawling_was_allowed() {
        let page = VisitedUrl::new(
            "id".to_string(),
            String::new(),
            SOURCE_INIT_URL,
            "https://external.example/page".to_string(),
            200,
            0.0,
            None,
            ContentTypeId::Html,
            Some("text/html".to_string()),
            None,
            None,
            true,
            true,
            0,
            None,
        );

        assert!(!eligible_page(&page));
    }
}
