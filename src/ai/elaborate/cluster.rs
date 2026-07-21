// SiteOne Crawler - brand elaborate: mass-entity clustering
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Detect "hundreds of pages of one type" (e-shop product details, blog posts, testimonials) so the
// elaborate CHARACTERIZES and SAMPLES them instead of enumerating them. Purely deterministic: group
// candidate URLs by a path template (id/slug/date segments collapsed to `*`), keep only groups at or
// above a size threshold, and never sample deny-listed noise (tags, pagination, feeds).

use std::collections::BTreeMap;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::ai::selection::Candidate;

/// A detected group of same-template pages.
#[derive(Debug, Clone)]
pub struct UrlCluster {
    /// The normalized path template, e.g. `/produkt/*` or `/blog/*/*/*`.
    pub template: String,
    /// uq_ids of the member pages, in the input (score-desc) order.
    pub member_uq_ids: Vec<String>,
    /// A few example URLs (highest-ranked first) for labeling.
    pub sample_urls: Vec<String>,
    /// True when the template matches the noise deny-list (never sampled, 0 reps).
    pub denied: bool,
}

/// A labeled cluster carried into the elaborate: the brand document characterizes it (e.g.
/// "e-shop with ~1,240 products in category X") instead of enumerating every member page.
#[derive(Debug, Clone)]
pub struct ClusterSummary {
    /// Human label (from the LLM IA round, or the template as a fallback).
    pub label: String,
    pub template: String,
    pub count: usize,
    pub sample_urls: Vec<String>,
}

static RE_SLUG_WITH_DIGITS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\d").unwrap());
static RE_ALL_DIGITS: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\d+$").unwrap());
static RE_UUID: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$").unwrap());
static RE_HEXISH: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^[0-9a-f]{16,}$").unwrap());

/// Path templates that are crawl noise — never sampled as brand content.
static DENY_TEMPLATES: Lazy<Vec<Regex>> = Lazy::new(|| {
    [
        r"(?i)/tag/",
        r"(?i)/tags/",
        r"(?i)/page/\*",
        r"(?i)/stranka/\*",
        r"(?i)/author/",
        r"(?i)/feed",
        r"(?i)/rss",
        r"(?i)/wp-json",
        r"(?i)/comment",
        r"(?i)/attachment",
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

/// Collapse a URL path into a template: a segment becomes `*` when it looks like an id/slug/date
/// component (all digits, a slug that contains digits, a long hex/uuid). Short human segments
/// (`/o-nas`, `/kontakt`) are kept verbatim.
pub fn normalize_path_template(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return "/".to_string();
    }
    let mut out = String::new();
    for seg in trimmed.split('/') {
        out.push('/');
        if is_variable_segment(seg) {
            out.push('*');
        } else {
            out.push_str(seg);
        }
    }
    out
}

fn is_variable_segment(seg: &str) -> bool {
    if seg.is_empty() {
        return false;
    }
    RE_ALL_DIGITS.is_match(seg)
        || RE_UUID.is_match(seg)
        || RE_HEXISH.is_match(seg)
        // A hyphenated slug that also carries digits (e.g. "produkt-123-xyz", "2024-06-foo").
        || (seg.contains('-') && RE_SLUG_WITH_DIGITS.is_match(seg))
}

fn path_of(url: &str) -> String {
    url::Url::parse(url)
        .map(|u| u.path().to_string())
        .unwrap_or_else(|_| url.to_string())
}

fn is_denied(template: &str) -> bool {
    DENY_TEMPLATES.iter().any(|re| re.is_match(template))
}

/// Group candidates by path template; a group with `>= min_size` members becomes a cluster. Returns
/// (clusters, unclustered_uq_ids). Deny-listed templates still form a cluster (so their members are
/// removed from the unclustered set) but are flagged `denied` so no reps are ever sampled from them.
pub fn cluster_urls(candidates: &[Candidate], min_size: usize) -> (Vec<UrlCluster>, Vec<String>) {
    let min_size = min_size.max(2);
    // Preserve input order (candidates arrive score-desc) inside each template bucket.
    let mut groups: BTreeMap<String, Vec<&Candidate>> = BTreeMap::new();
    for c in candidates {
        groups
            .entry(normalize_path_template(&path_of(&c.url)))
            .or_default()
            .push(c);
    }

    let mut clusters = Vec::new();
    let mut unclustered = Vec::new();
    for (template, members) in groups {
        // A template with no `*` is a single concrete page, never a mass-entity cluster.
        let is_templated = template.contains('*');
        if is_templated && members.len() >= min_size {
            clusters.push(UrlCluster {
                denied: is_denied(&template),
                member_uq_ids: members.iter().map(|c| c.uq_id.clone()).collect(),
                sample_urls: members.iter().take(3).map(|c| c.url.clone()).collect(),
                template,
            });
        } else {
            unclustered.extend(members.iter().map(|c| c.uq_id.clone()));
        }
    }
    // Largest clusters first (more informative for the elaborate).
    clusters.sort_by(|a, b| {
        b.member_uq_ids
            .len()
            .cmp(&a.member_uq_ids.len())
            .then(a.template.cmp(&b.template))
    });
    (clusters, unclustered)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(url: &str) -> Candidate {
        Candidate {
            uq_id: url.to_string(),
            url: format!("https://x{}", url),
            depth: 2,
            in_sitemap: false,
            score: 10.0,
        }
    }

    #[test]
    fn normalizes_ids_slugs_dates_keeps_human_paths() {
        assert_eq!(normalize_path_template("/produkt/1234"), "/produkt/*");
        assert_eq!(
            normalize_path_template("/blog/2024/06/muj-clanek"),
            "/blog/*/*/muj-clanek"
        );
        assert_eq!(normalize_path_template("/produkt/abc-123-xyz"), "/produkt/*");
        assert_eq!(normalize_path_template("/o-nas"), "/o-nas"); // human slug, no digits, kept
        assert_eq!(normalize_path_template("/kontakt"), "/kontakt");
        assert_eq!(normalize_path_template("/"), "/");
        assert_eq!(
            normalize_path_template("/p/1b8dac0958934593be7e5e487006c684"),
            "/p/*" // long hex id
        );
    }

    #[test]
    fn clusters_only_at_min_size() {
        let mut cands: Vec<Candidate> = (0..9).map(|i| cand(&format!("/p/{}", i))).collect();
        cands.extend((0..3).map(|i| cand(&format!("/blog/2024/{}/x-1", i)))); // only 3 → not a cluster
        cands.push(cand("/o-nas"));
        let (clusters, unclustered) = cluster_urls(&cands, 8);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].template, "/p/*");
        assert_eq!(clusters[0].member_uq_ids.len(), 9);
        assert!(!clusters[0].denied);
        // The 3 blog pages + /o-nas remain unclustered.
        assert_eq!(unclustered.len(), 4);
    }

    #[test]
    fn deny_listed_cluster_is_flagged() {
        let cands: Vec<Candidate> = (0..10).map(|i| cand(&format!("/tag/{}", i))).collect();
        let (clusters, unclustered) = cluster_urls(&cands, 8);
        assert_eq!(clusters.len(), 1);
        assert!(clusters[0].denied);
        assert!(unclustered.is_empty()); // members are still removed from the unclustered pool
    }

    #[test]
    fn single_concrete_pages_never_cluster() {
        let cands = vec![cand("/o-nas"), cand("/sluzby"), cand("/kontakt")];
        let (clusters, unclustered) = cluster_urls(&cands, 2);
        assert!(clusters.is_empty());
        assert_eq!(unclustered.len(), 3);
    }
}
