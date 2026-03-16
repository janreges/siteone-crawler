// SiteOne Crawler - Quality Scorer
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Computes quality scores (0.0-10.0) across 5 categories based on
// data already collected by existing analyzers.

use regex::Regex;

use crate::components::summary::item_status::ItemStatus;
use crate::components::summary::summary::Summary;
use crate::output::output::BasicStats;
use crate::scoring::quality_score::{CategoryScore, Deduction, QualityScores, score_label};

/// Maximum total deduction from "per URL" rules within a single category.
const MAX_PER_URL_DEDUCTION: f64 = 5.0;

/// Maximum deduction from a single per-URL deduction type (prevents one issue from eating entire budget).
const MAX_PER_TYPE_DEDUCTION: f64 = 2.5;

/// Calculate quality scores from analysis results.
pub fn calculate_scores(summary: &Summary, basic_stats: &BasicStats) -> QualityScores {
    let categories = vec![
        score_performance(summary, basic_stats),
        score_seo(summary, basic_stats),
        score_security(summary),
        score_accessibility(summary),
        score_best_practices(summary),
    ];

    let overall_score = categories.iter().map(|c| c.score * c.weight).sum::<f64>();
    let overall_score = round1(overall_score);

    let overall = CategoryScore {
        name: "Overall".to_string(),
        code: "overall".to_string(),
        score: overall_score,
        label: score_label(overall_score).to_string(),
        weight: 1.0,
        deductions: Vec::new(),
    };

    QualityScores { overall, categories }
}

// ---- Category scorers ----

fn score_performance(summary: &Summary, stats: &BasicStats) -> CategoryScore {
    let mut deductions = Vec::new();
    let mut per_url_total = 0.0;

    // Average response time
    if stats.total_requests_times_avg > 1.0 {
        deductions.push(Deduction {
            reason: format!(
                "Average response time {:.0}ms > 1000ms",
                stats.total_requests_times_avg * 1000.0
            ),
            points: 1.0,
        });
    } else if stats.total_requests_times_avg > 0.5 {
        deductions.push(Deduction {
            reason: format!(
                "Average response time {:.0}ms > 500ms",
                stats.total_requests_times_avg * 1000.0
            ),
            points: 0.5,
        });
    }

    // Slowest single response (from BasicStats — covers all resource types)
    if stats.total_requests_times_max > 5.0 {
        deductions.push(Deduction {
            reason: format!("Slowest response {:.1}s > 5.0s", stats.total_requests_times_max),
            points: 1.0,
        });
    } else if stats.total_requests_times_max > 3.0 {
        deductions.push(Deduction {
            reason: format!("Slowest response {:.1}s > 3.0s", stats.total_requests_times_max),
            points: 0.5,
        });
    }

    // Slow URLs count (from slowest analyzer summary)
    if is_not_ok(summary, "slowUrls") {
        let count = get_item_count(summary, "slowUrls").unwrap_or(1);
        if count > 0 {
            let pts = (count as f64 * 0.3).min(MAX_PER_URL_DEDUCTION);
            per_url_total += pts;
            deductions.push(Deduction {
                reason: format!("{} slow URL(s) detected", count),
                points: round1(pts),
            });
        }
    }

    build_category("Performance", "performance", 0.20, deductions, per_url_total)
}

fn score_seo(summary: &Summary, stats: &BasicStats) -> CategoryScore {
    let mut deductions = Vec::new();
    let mut per_url_total = 0.0;

    // Missing H1
    per_url_deduct(
        summary,
        "pages-without-h1",
        0.3,
        "page(s) without <h1>",
        &mut deductions,
        &mut per_url_total,
    );

    // Multiple H1
    per_url_deduct(
        summary,
        "pages-with-multiple-h1",
        0.2,
        "page(s) with multiple <h1>",
        &mut deductions,
        &mut per_url_total,
    );

    // Title uniqueness issues
    if is_not_ok(summary, "title-uniqueness") {
        let count = get_item_count_for_code(summary, "title-uniqueness").unwrap_or(1);
        let pts = (count as f64 * 0.3).min(MAX_PER_TYPE_DEDUCTION);
        let remaining = MAX_PER_URL_DEDUCTION - per_url_total;
        let pts = pts.min(remaining).max(0.0);
        per_url_total += pts;
        deductions.push(Deduction {
            reason: "Non-unique page titles detected".to_string(),
            points: round1(pts),
        });
    }

    // Meta description uniqueness
    if is_not_ok(summary, "meta-description-uniqueness") {
        let count = get_item_count_for_code(summary, "meta-description-uniqueness").unwrap_or(1);
        let pts = (count as f64 * 0.3).min(MAX_PER_TYPE_DEDUCTION);
        let remaining = MAX_PER_URL_DEDUCTION - per_url_total;
        let pts = pts.min(remaining).max(0.0);
        per_url_total += pts;
        deductions.push(Deduction {
            reason: "Non-unique meta descriptions detected".to_string(),
            points: round1(pts),
        });
    }

    // 404 pages — use status code count from BasicStats for accuracy
    let count_404 = stats.count_by_status.get(&404).copied().unwrap_or(0);
    if count_404 > 0 {
        let pts = match count_404 {
            1 => 0.5,
            2..=5 => 1.0,
            6..=20 => 1.5,
            _ => 2.0,
        };
        deductions.push(Deduction {
            reason: format!("{} page(s) returned 404", count_404),
            points: pts,
        });
    }

    // Redirects
    if is_not_ok(summary, "redirects") {
        let count = get_item_count(summary, "redirects").unwrap_or(1);
        if count > 0 {
            let pts = (count as f64 * 0.15).min(MAX_PER_TYPE_DEDUCTION);
            let remaining = MAX_PER_URL_DEDUCTION - per_url_total;
            let pts = pts.min(remaining).max(0.0);
            per_url_total += pts;
            deductions.push(Deduction {
                reason: format!("{} redirect(s) found", count),
                points: round1(pts),
            });
        }
    }

    build_category("SEO", "seo", 0.20, deductions, per_url_total)
}

fn score_security(summary: &Summary) -> CategoryScore {
    let mut deductions = Vec::new();

    // SSL certificate issues
    for code in &[
        "ssl-certificate-connect",
        "ssl-certificate-missing",
        "ssl-certificate-parse",
        "ssl-certificate-valid",
    ] {
        if is_critical(summary, code) {
            deductions.push(Deduction {
                reason: "SSL/TLS certificate issue".to_string(),
                points: 3.0,
            });
            break;
        }
    }

    // SSL certificate validity period
    if is_critical(summary, "ssl-certificate-valid-to") {
        deductions.push(Deduction {
            reason: "SSL certificate expired or expiring soon".to_string(),
            points: 0.5,
        });
    }

    // Unsafe SSL protocols
    if is_critical(summary, "ssl-protocol-unsafe") || is_warning(summary, "ssl-protocol-unsafe") {
        deductions.push(Deduction {
            reason: "Insecure TLS protocol versions supported".to_string(),
            points: 1.0,
        });
    }

    // Security headers — graduated scale based on affected page count
    if is_critical(summary, "security") {
        let count = get_item_count(summary, "security").unwrap_or(1);
        let pts = match count {
            0 => 0.0,
            1 => 1.0,
            2 => 1.5,
            3 => 2.0,
            4..=10 => 2.5,
            11..=50 => 3.0,
            _ => 3.5,
        };
        deductions.push(Deduction {
            reason: format!("{} page(s) with critical security findings", count),
            points: pts,
        });
    } else if is_warning_or_above(summary, "security") {
        let count = get_item_count(summary, "security").unwrap_or(1);
        let pts = match count {
            0 => 0.0,
            1 => 0.5,
            2 => 0.75,
            3 => 1.0,
            4..=10 => 1.25,
            _ => 1.5,
        };
        deductions.push(Deduction {
            reason: format!("{} page(s) with security warnings", count),
            points: pts,
        });
    }

    build_category("Security", "security", 0.25, deductions, 0.0)
}

fn score_accessibility(summary: &Summary) -> CategoryScore {
    let mut deductions = Vec::new();
    let mut per_url_total = 0.0;

    // Missing lang attribute (flat deduction — affects entire site)
    if is_not_ok(summary, "pages-without-lang") {
        let count = get_item_count(summary, "pages-without-lang").unwrap_or(1);
        let pts = if count > 0 { 1.5 } else { 0.0 };
        deductions.push(Deduction {
            reason: format!("{} page(s) without lang attribute", count),
            points: pts,
        });
    }

    // Missing image alt attributes
    per_url_deduct(
        summary,
        "pages-without-image-alt-attributes",
        0.5,
        "page(s) without image alt attributes",
        &mut deductions,
        &mut per_url_total,
    );

    // Missing form labels
    per_url_deduct(
        summary,
        "pages-without-form-labels",
        0.5,
        "page(s) without form labels",
        &mut deductions,
        &mut per_url_total,
    );

    // Skipped heading levels (accessibility concern, not SEO)
    per_url_deduct(
        summary,
        "pages-with-skipped-heading-levels",
        0.1,
        "page(s) with skipped heading levels",
        &mut deductions,
        &mut per_url_total,
    );

    // Missing ARIA labels
    per_url_deduct(
        summary,
        "pages-without-aria-labels",
        0.3,
        "page(s) without aria labels",
        &mut deductions,
        &mut per_url_total,
    );

    // Missing roles (lower weight — semantic HTML provides implicit roles)
    per_url_deduct(
        summary,
        "pages-without-roles",
        0.15,
        "page(s) without role attributes",
        &mut deductions,
        &mut per_url_total,
    );

    // Invalid HTML
    per_url_deduct(
        summary,
        "pages-with-invalid-html",
        0.3,
        "page(s) with invalid HTML",
        &mut deductions,
        &mut per_url_total,
    );

    build_category("Accessibility", "accessibility", 0.20, deductions, per_url_total)
}

fn score_best_practices(summary: &Summary) -> CategoryScore {
    let mut deductions = Vec::new();
    let mut per_url_total = 0.0;

    // Duplicate SVGs
    per_url_deduct(
        summary,
        "pages-with-duplicated-svgs",
        0.3,
        "page(s) with duplicated inline SVGs",
        &mut deductions,
        &mut per_url_total,
    );

    // Large SVGs
    per_url_deduct(
        summary,
        "pages-with-large-svgs",
        0.2,
        "page(s) with large inline SVGs",
        &mut deductions,
        &mut per_url_total,
    );

    // Invalid SVGs
    per_url_deduct(
        summary,
        "pages-with-invalid-svgs",
        0.2,
        "page(s) with invalid inline SVGs",
        &mut deductions,
        &mut per_url_total,
    );

    // Missing quotes
    per_url_deduct(
        summary,
        "pages-with-missing-quotes",
        0.2,
        "page(s) with missing quotes",
        &mut deductions,
        &mut per_url_total,
    );

    // Deep DOM
    per_url_deduct(
        summary,
        "pages-with-deep-dom",
        0.5,
        "page(s) with deep DOM",
        &mut deductions,
        &mut per_url_total,
    );

    // Non-clickable phone numbers
    per_url_deduct(
        summary,
        "pages-with-non-clickable-phone-numbers",
        0.3,
        "page(s) with non-clickable phone numbers",
        &mut deductions,
        &mut per_url_total,
    );

    // Brotli support
    if is_not_ok(summary, "brotli-support") {
        deductions.push(Deduction {
            reason: "No Brotli compression support".to_string(),
            points: 0.5,
        });
    }

    // WebP support
    if is_not_ok(summary, "webp-support") {
        deductions.push(Deduction {
            reason: "No WebP image support".to_string(),
            points: 0.3,
        });
    }

    build_category("Best Practices", "best-practices", 0.15, deductions, per_url_total)
}

// ---- Helpers ----

fn build_category(
    name: &str,
    code: &str,
    weight: f64,
    deductions: Vec<Deduction>,
    _per_url_total: f64,
) -> CategoryScore {
    let fixed_total: f64 = deductions.iter().map(|d| d.points).sum();
    let score = round1((10.0 - fixed_total).clamp(0.0, 10.0));

    CategoryScore {
        name: name.to_string(),
        code: code.to_string(),
        score,
        label: score_label(score).to_string(),
        weight,
        deductions,
    }
}

/// Apply a per-URL deduction with per-type sub-cap and total cap.
fn per_url_deduct(
    summary: &Summary,
    apl_code: &str,
    points_per_url: f64,
    description: &str,
    deductions: &mut Vec<Deduction>,
    per_url_total: &mut f64,
) {
    if is_not_ok(summary, apl_code) {
        let count = get_item_count(summary, apl_code).unwrap_or(1);
        if count > 0 {
            let remaining = MAX_PER_URL_DEDUCTION - *per_url_total;
            if remaining <= 0.0 {
                return;
            }
            // Apply per-type sub-cap, then total cap
            let pts = (count as f64 * points_per_url)
                .min(MAX_PER_TYPE_DEDUCTION)
                .min(remaining);
            *per_url_total += pts;
            deductions.push(Deduction {
                reason: format!("{} {}", count, description),
                points: round1(pts),
            });
        }
    }
}

/// Check if a summary item is not OK (Warning, Critical, or Notice).
fn is_not_ok(summary: &Summary, apl_code: &str) -> bool {
    summary
        .get_items()
        .iter()
        .any(|item| item.apl_code == apl_code && !matches!(item.status, ItemStatus::Ok | ItemStatus::Info))
}

/// Check if a summary item is Critical.
fn is_critical(summary: &Summary, apl_code: &str) -> bool {
    summary
        .get_items()
        .iter()
        .any(|item| item.apl_code == apl_code && item.status == ItemStatus::Critical)
}

/// Check if a summary item is Warning or above.
fn is_warning_or_above(summary: &Summary, apl_code: &str) -> bool {
    summary
        .get_items()
        .iter()
        .any(|item| item.apl_code == apl_code && matches!(item.status, ItemStatus::Warning | ItemStatus::Critical))
}

/// Check if a summary item is Warning.
fn is_warning(summary: &Summary, apl_code: &str) -> bool {
    summary
        .get_items()
        .iter()
        .any(|item| item.apl_code == apl_code && item.status == ItemStatus::Warning)
}

/// Extract a count (first number found) from a non-OK summary item's text.
fn get_item_count(summary: &Summary, apl_code: &str) -> Option<usize> {
    let item = summary
        .get_items()
        .iter()
        .find(|i| i.apl_code == apl_code && !matches!(i.status, ItemStatus::Ok | ItemStatus::Info))?;
    extract_first_number(&item.text)
}

/// Get count for items that may have multiple entries with the same apl_code (e.g. title-uniqueness).
fn get_item_count_for_code(summary: &Summary, apl_code: &str) -> Option<usize> {
    let count = summary
        .get_items()
        .iter()
        .filter(|i| i.apl_code == apl_code && !matches!(i.status, ItemStatus::Ok | ItemStatus::Info))
        .count();
    if count > 0 { Some(count) } else { None }
}

/// Extract the first number from a string (e.g., "Security - 89 pages(s) with..." -> 89).
fn extract_first_number(text: &str) -> Option<usize> {
    number_regex().find(text).and_then(|m| m.as_str().parse().ok())
}

fn number_regex() -> &'static Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\d+").unwrap())
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::summary::item::Item;
    use crate::scoring::quality_score::score_label;

    fn make_empty_summary() -> Summary {
        Summary::new()
    }

    fn make_summary_with_items(items: Vec<(&str, ItemStatus)>) -> Summary {
        let mut s = Summary::new();
        for (code, status) in items {
            s.add_item(Item::new(code.to_string(), "1 test issue".to_string(), status));
        }
        s
    }

    fn make_basic_stats() -> BasicStats {
        BasicStats {
            total_urls: 100,
            total_requests_times_avg: 0.3,
            ..Default::default()
        }
    }

    #[test]
    fn perfect_score_for_clean_site() {
        let summary = make_empty_summary();
        let stats = make_basic_stats();
        let scores = calculate_scores(&summary, &stats);
        assert_eq!(scores.overall.score, 10.0);
    }

    #[test]
    fn score_label_thresholds() {
        assert_eq!(score_label(9.5), "Excellent");
        assert_eq!(score_label(8.0), "Good");
        assert_eq!(score_label(5.5), "Fair");
        assert_eq!(score_label(3.5), "Poor");
        assert_eq!(score_label(1.0), "Critical");
    }

    #[test]
    fn slow_response_deduction() {
        let summary = make_empty_summary();
        let mut stats = make_basic_stats();
        stats.total_requests_times_avg = 1.5;
        let scores = calculate_scores(&summary, &stats);
        let perf = scores.categories.iter().find(|c| c.code == "performance").unwrap();
        assert!(perf.score < 10.0);
    }

    #[test]
    fn categories_have_correct_weights() {
        let summary = make_empty_summary();
        let stats = make_basic_stats();
        let scores = calculate_scores(&summary, &stats);
        let total_weight: f64 = scores.categories.iter().map(|c| c.weight).sum();
        assert!((total_weight - 1.0).abs() < 0.001);
    }

    #[test]
    fn overall_is_weighted_average() {
        let summary = make_empty_summary();
        let stats = make_basic_stats();
        let scores = calculate_scores(&summary, &stats);
        let expected: f64 = scores.categories.iter().map(|c| c.score * c.weight).sum();
        let expected = round1(expected);
        assert!((scores.overall.score - expected).abs() < 0.01);
    }

    #[test]
    fn errors_404_deduct_from_seo() {
        let summary = make_empty_summary();
        let mut stats = make_basic_stats();
        stats.count_by_status.insert(404, 5);
        let scores = calculate_scores(&summary, &stats);
        let seo = scores.categories.iter().find(|c| c.code == "seo").unwrap();
        assert!(seo.score < 10.0);
    }

    #[test]
    fn warnings_reduce_score() {
        let summary = make_summary_with_items(vec![
            ("pages-without-h1", ItemStatus::Warning),
            ("pages-without-lang", ItemStatus::Warning),
        ]);
        let stats = make_basic_stats();
        let scores = calculate_scores(&summary, &stats);
        assert!(scores.overall.score < 10.0);
    }
}
