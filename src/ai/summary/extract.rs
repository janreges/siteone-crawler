// SiteOne Crawler - AI report-summary data extraction
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Builds the compact per-area input JSON fed to the area-evaluation prompts. The crawl may
// have up to 100,000 URLs, so this NEVER emits per-URL lists — only aggregates, distributions,
// and a few hard-capped top-N examples. Each area input targets well under 15 KB.

use std::collections::HashMap;

use serde_json::{Value, json};

use crate::result::status::Status;
use crate::result::visited_url::{
    CACHE_TYPE_HAS_CACHE_CONTROL, CACHE_TYPE_HAS_ETAG, CACHE_TYPE_HAS_IMMUTABLE, CACHE_TYPE_HAS_LAST_MODIFIED,
    CACHE_TYPE_HAS_MAX_AGE, CACHE_TYPE_HAS_NO_STORE, CACHE_TYPE_NO_CACHE_HEADERS, VisitedUrl,
};
use crate::scoring::quality_score::QualityScores;
use crate::types::ContentTypeId;

const MAX_FINDINGS_PER_AREA: usize = 30;
const MAX_TOP_N: usize = 12;
const MAX_URL_LEN: usize = 120;
const MAX_TEXT_LEN: usize = 240;

/// The five evaluation areas. Order is fixed (used for display + synthesis).
pub const AREAS: [&str; 5] = ["security", "accessibility", "seo", "performance", "infrastructure"];

/// One compact per-area input.
pub struct AreaInput {
    pub area: &'static str,
    pub json: Value,
}

/// Build all area inputs from the post-analysis Status + computed quality scores.
pub fn build_area_inputs(status: &Status, scores: &QualityScores) -> Vec<AreaInput> {
    let visited = status.get_visited_urls();
    let stats = status.get_basic_stats();
    let agg = CrawlAggregates::compute(&visited, stats.total_size);

    // Group summary findings by area.
    let summary = status.get_summary();
    let mut findings_by_area: HashMap<&'static str, Vec<Value>> = HashMap::new();
    for item in summary.get_items() {
        if item.text.trim().is_empty() {
            continue;
        }
        let area = area_for_apl_code(&item.apl_code);
        let list = findings_by_area.entry(area).or_default();
        if list.len() < MAX_FINDINGS_PER_AREA {
            list.push(json!({
                "severity": format!("{:?}", item.status).to_uppercase(),
                "code": item.apl_code,
                "text": truncate(&strip_ansi(&item.text), MAX_TEXT_LEN),
            }));
        }
    }

    let scope = json!({
        "total_urls": stats.total_urls,
        "html_pages": agg.html_pages,
        "internal_urls": agg.internal_urls,
        "external_urls": agg.external_urls,
        "https_urls": agg.https_urls,
        "http_urls": agg.http_urls,
        "total_transfer_human": stats.total_size_formatted,
    });

    let mut out = Vec::new();
    for area in AREAS {
        let findings = findings_by_area.remove(area).unwrap_or_default();
        let mut obj = json!({
            "area": area,
            "scope": scope,
            "category_score": category_score_json(scores, area),
            "findings": findings,
            "area_stats": area_stats(area, &agg, status),
        });
        cap_area_json(&mut obj);
        out.push(AreaInput { area, json: obj });
    }
    out
}

/// Map a summary item's apl_code to one of the five areas (keyword heuristic, order matters).
fn area_for_apl_code(code: &str) -> &'static str {
    let c = code.to_lowercase();
    let has = |kws: &[&str]| kws.iter().any(|k| c.contains(k));
    if has(&[
        "security",
        "ssl",
        "tls",
        "cert",
        "cookie",
        "csp",
        "hsts",
        "mixed",
        "x-frame",
        "x-content",
        "referrer",
        "permissions-policy",
    ]) {
        // Note: DNS/IPv6 record findings are routed to infrastructure (taxonomy), not security.
        "security"
    } else if has(&[
        "seo",
        "title",
        "description",
        "canonical",
        "noindex",
        "keyword",
        "open-graph",
        "robots",
        "-og",
    ]) {
        "seo"
    } else if has(&[
        "accessib",
        "image-alt",
        "alt-attr",
        "form-label",
        "aria",
        "main-landmark",
        "lang",
        "invalid-html",
        "h1",
        "heading",
        "deep-dom",
        "phone",
        "svg",
        "quotes",
    ]) {
        "accessibility"
    } else if has(&[
        "slow",
        "fast",
        "cache",
        "static-asset",
        "weight",
        "request",
        "response",
        "brotli",
        "webp",
        "avif",
        "compression",
    ]) {
        // Modern formats (WebP/AVIF) and Brotli are transfer/payload concerns → performance.
        "performance"
    } else {
        "infrastructure"
    }
}

/// Map an area to its quality-score category (by category name substring) and emit
/// score + grade + the scorer's curated deductions (reason + fix).
fn category_score_json(scores: &QualityScores, area: &str) -> Value {
    let names: &[&str] = match area {
        "security" => &["security"],
        "accessibility" => &["accessibility", "best practices"],
        "seo" => &["seo"],
        "performance" => &["performance"],
        _ => &[],
    };
    if names.is_empty() {
        return Value::Null;
    }
    let mut cats = Vec::new();
    for cat in &scores.categories {
        let cn = cat.name.to_lowercase();
        if names.iter().any(|n| cn.contains(n)) {
            let deductions: Vec<Value> = cat
                .deductions
                .iter()
                .take(MAX_TOP_N)
                .map(|d| {
                    json!({
                        "reason": truncate(&d.reason, MAX_TEXT_LEN),
                        "points": d.points,
                        "fix": d.fix.as_deref().map(|f| truncate(f, MAX_TEXT_LEN)),
                    })
                })
                .collect();
            cats.push(json!({
                "name": cat.name,
                "score_0_10": (cat.score * 10.0).round() / 10.0,
                "label": cat.label,
                "weight": cat.weight,
                "deductions": deductions,
            }));
        }
    }
    json!(cats)
}

/// Area-specific numeric aggregates (already capped).
fn area_stats(area: &str, agg: &CrawlAggregates, status: &Status) -> Value {
    match area {
        "security" => json!({
            "https_urls": agg.https_urls,
            "http_urls": agg.http_urls,
            "security_headers": security_headers(status),
            "security_header_severity": capped_table(status, "security", 15),
            "certificate_info": capped_table(status, "certificate-info", 12),
        }),
        "seo" => json!({
            "html_pages": agg.html_pages,
            "pages_with_opengraph": status.get_super_table_rows("open-graph").len(),
            "ai_seo_scores": ai_seo_scores(status),
            "duplicate_titles_top": capped_table(status, "non-unique-titles", 10),
            "duplicate_descriptions_top": capped_table(status, "non-unique-descriptions", 10),
        }),
        "accessibility" => json!({
            "html_pages": agg.html_pages,
            "accessibility_checks": capped_table(status, "accessibility", 10),
            "best_practice_checks": capped_table(status, "best-practices", 14),
        }),
        "performance" => json!({
            "response_time_s": { "avg": agg.rt_avg, "p90": agg.rt_p90, "max": agg.rt_max },
            "slow_over_1s": agg.slow_over_1s,
            "slow_over_2s": agg.slow_over_2s,
            "total_transfer_bytes": agg.total_bytes,
            "bytes_by_content_type": agg.bytes_by_type_json(),
            "images_total": agg.image_count,
            "webp_images": agg.webp_images,
            "avif_images": agg.avif_images,
            "http_caching": agg.caching_json(),
            "text_compression": agg.compression_json(),
            "caching_by_content_type": capped_table(status, "caching-per-content-type", 12),
            "top_slowest": agg.top_slowest_json(),
            "top_largest": agg.top_largest_json(),
        }),
        _ => json!({
            "status_distribution": agg.status_dist_json(),
            "content_types": agg.content_type_json(),
            "image_subtypes": agg.image_subtypes_json(),
            "skipped_urls": skipped_by_reason(status),
            "redirects": agg.redirect_count,
            "broken_404": agg.count_404,
            "server_errors_5xx": agg.count_5xx,
            "connection_errors": agg.error_count,
            "external_domains_total": agg.external_domains.len(),
            "top_external_domains": agg.top_external_domains_json(),
        }),
    }
}

/// Actual security-relevant HTTP response headers + their real values (from the headers-values
/// table), so the security prompt can judge CSP strength (unsafe-inline/eval), HSTS max-age,
/// X-Frame-Options, cookie flags, and tech-stack disclosure — plus which protective headers are
/// MISSING entirely. Values are length-capped; the set of headers is fixed (bounded).
fn security_headers(status: &Status) -> Value {
    // Protective headers (their ABSENCE is a finding) + disclosure headers (their PRESENCE is).
    const PROTECTIVE: &[&str] = &[
        "content-security-policy",
        "strict-transport-security",
        "x-frame-options",
        "x-content-type-options",
        "referrer-policy",
        "permissions-policy",
        "cross-origin-opener-policy",
        "cross-origin-embedder-policy",
        "cross-origin-resource-policy",
    ];
    const ALSO_RELEVANT: &[&str] = &[
        "x-xss-protection",
        "feature-policy",
        "access-control-allow-origin",
        "set-cookie",
        "server",
        "x-powered-by",
    ];

    let is_relevant = |h: &str| PROTECTIVE.contains(&h) || ALSO_RELEVANT.contains(&h);

    let mut present_values = Vec::new();
    let mut present_set = std::collections::BTreeSet::new();
    for row in status.get_super_table_rows("headers-values") {
        let name = row.get("header").map(|h| h.trim().to_lowercase()).unwrap_or_default();
        if !is_relevant(name.as_str()) {
            continue;
        }
        present_set.insert(name.clone());
        if present_values.len() < 30 {
            present_values.push(json!({
                "header": row.get("header").cloned().unwrap_or_default(),
                "pages": row.get("occurrences").cloned().unwrap_or_default(),
                "value": truncate(row.get("value").map(|s| s.as_str()).unwrap_or(""), 600),
            }));
        }
    }

    let missing_protective: Vec<&str> = PROTECTIVE
        .iter()
        .filter(|h| !present_set.contains(**h))
        .copied()
        .collect();

    json!({
        "present": present_values,
        "missing_protective_headers": missing_protective,
    })
}

/// Breakdown of skipped URLs by reason. Crucially distinguishes external/disallowed-host
/// skips (NORMAL — links to other domains the crawler does not follow) from robots.txt blocks
/// and depth limits, so the model does not flag normal external links as a crawlability problem.
fn skipped_by_reason(status: &Status) -> Value {
    use crate::types::SkippedReason;
    let (mut external, mut robots, mut max_depth) = (0i64, 0i64, 0i64);
    for e in status.get_skipped_urls() {
        match e.reason {
            SkippedReason::NotAllowedHost => external += 1,
            SkippedReason::RobotsTxt => robots += 1,
            SkippedReason::ExceedsMaxDepth => max_depth += 1,
        }
    }
    json!({
        "external_or_disallowed_host_normal": external,
        "blocked_by_robots_txt": robots,
        "exceeded_max_crawl_depth": max_depth,
    })
}

/// Compact summary of the per-page AI SEO scores (from the `ai-seo` table, if the seo action
/// ran). Returns Null when there is no data. `overall` cells are stored as "NN%".
fn ai_seo_scores(status: &Status) -> Value {
    let rows = status.get_super_table_rows("ai-seo");
    if rows.is_empty() {
        return Value::Null;
    }
    let parse_pct = |s: &str| s.trim().trim_end_matches('%').trim().parse::<i64>().ok();
    let mut scored: Vec<(String, i64)> = rows
        .iter()
        .filter_map(|r| {
            let overall = r.get("overall").and_then(|v| parse_pct(v))?;
            let path = r.get("urlPathAndQuery").cloned().unwrap_or_default();
            Some((path, overall))
        })
        .collect();
    if scored.is_empty() {
        return Value::Null;
    }
    let n = scored.len() as i64;
    let avg = scored.iter().map(|(_, s)| *s).sum::<i64>() / n;
    scored.sort_by_key(|(_, s)| *s);
    let weakest: Vec<Value> = scored
        .iter()
        .take(8)
        .map(|(p, s)| json!({ "path": truncate(p, MAX_URL_LEN), "overall": s }))
        .collect();
    json!({ "pages_assessed": n, "avg_overall": avg, "weakest_pages": weakest })
}

/// Pull up to `limit` rows from a stored super table.
fn capped_table(status: &Status, apl_code: &str, limit: usize) -> Value {
    let rows: Vec<Value> = status
        .get_super_table_rows(apl_code)
        .into_iter()
        .take(limit)
        .map(|row| {
            let capped: serde_json::Map<String, Value> = row
                .into_iter()
                .map(|(k, v)| (k, Value::String(truncate(&v, MAX_TEXT_LEN))))
                .collect();
            Value::Object(capped)
        })
        .collect();
    json!(rows)
}

const HARD_CAP: usize = 28_000; // bytes; well under any context budget

/// Final safety net guaranteeing the per-area input never exceeds HARD_CAP: trim findings
/// first, then, as a last resort, drop the (already-capped) area_stats block.
fn cap_area_json(obj: &mut Value) {
    let len = |v: &Value| serde_json::to_string(v).map(|s| s.len()).unwrap_or(0);

    let mut guard = 0;
    while len(obj) > HARD_CAP && guard < 60 {
        match obj.get_mut("findings").and_then(|f| f.as_array_mut()) {
            // Halve the findings each step so even a pathologically large array converges fast.
            Some(arr) if !arr.is_empty() => arr.truncate(arr.len() / 2),
            _ => break,
        }
        guard += 1;
    }

    if len(obj) > HARD_CAP
        && let Some(map) = obj.as_object_mut()
    {
        map.insert(
            "area_stats".to_string(),
            json!({ "note": "omitted to fit context budget" }),
        );
    }
}

// ---------------------------------------------------------------------------------------

/// Aggregates computed in a single pass over the visited URLs.
struct CrawlAggregates {
    html_pages: usize,
    internal_urls: usize,
    external_urls: usize,
    https_urls: usize,
    http_urls: usize,
    total_bytes: i64,
    rt_avg: f64,
    rt_p90: f64,
    rt_max: f64,
    slow_over_1s: usize,
    slow_over_2s: usize,
    redirect_count: usize,
    count_404: usize,
    count_5xx: usize,
    error_count: usize,
    image_count: usize,
    webp_images: usize,
    avif_images: usize,
    status_dist: HashMap<i32, usize>,
    type_count: HashMap<i32, usize>,
    type_bytes: HashMap<i32, i64>,
    image_subtypes: HashMap<String, (usize, i64)>,
    external_domains: HashMap<String, usize>,
    top_slowest: Vec<(String, f64, i64)>,
    top_largest: Vec<(String, i64, String)>,
    // HTTP caching aggregates over static assets (200 responses).
    cache_assets: usize,
    cache_with_cache_control: usize,
    cache_with_max_age: usize,
    cache_with_etag: usize,
    cache_with_last_modified: usize,
    cache_immutable: usize,
    cache_no_store: usize,
    cache_missing_headers: usize,
    cache_lifetime_buckets: [usize; 6], // none, <1h, 1h-1d, 1d-30d, 30d-1y, >1y
    // Compression coverage over text-based assets (200 responses).
    text_assets: usize,
    text_compressed: usize,
    text_brotli: usize,
    text_gzip: usize,
}

impl CrawlAggregates {
    fn compute(visited: &[VisitedUrl], total_size: i64) -> Self {
        let mut a = CrawlAggregates {
            html_pages: 0,
            internal_urls: 0,
            external_urls: 0,
            https_urls: 0,
            http_urls: 0,
            total_bytes: total_size,
            rt_avg: 0.0,
            rt_p90: 0.0,
            rt_max: 0.0,
            slow_over_1s: 0,
            slow_over_2s: 0,
            redirect_count: 0,
            count_404: 0,
            count_5xx: 0,
            error_count: 0,
            image_count: 0,
            webp_images: 0,
            avif_images: 0,
            status_dist: HashMap::new(),
            type_count: HashMap::new(),
            type_bytes: HashMap::new(),
            image_subtypes: HashMap::new(),
            external_domains: HashMap::new(),
            top_slowest: Vec::new(),
            top_largest: Vec::new(),
            cache_assets: 0,
            cache_with_cache_control: 0,
            cache_with_max_age: 0,
            cache_with_etag: 0,
            cache_with_last_modified: 0,
            cache_immutable: 0,
            cache_no_store: 0,
            cache_missing_headers: 0,
            cache_lifetime_buckets: [0; 6],
            text_assets: 0,
            text_compressed: 0,
            text_brotli: 0,
            text_gzip: 0,
        };

        let mut rt_sum = 0.0;
        let mut rt_n = 0usize;
        let mut rt_values: Vec<f64> = Vec::new();
        for u in visited {
            if u.url.starts_with("https://") {
                a.https_urls += 1;
            } else if u.url.starts_with("http://") {
                a.http_urls += 1;
            }
            if u.is_external {
                a.external_urls += 1;
                if let Some(host) = url_host(&u.url) {
                    *a.external_domains.entry(host).or_insert(0) += 1;
                }
            } else {
                a.internal_urls += 1;
            }

            *a.status_dist.entry(u.status_code).or_insert(0) += 1;
            match u.status_code {
                404 => a.count_404 += 1,
                500..=599 => a.count_5xx += 1,
                c if c < 0 => a.error_count += 1,
                _ => {}
            }
            if (300..=399).contains(&u.status_code) {
                a.redirect_count += 1;
            }
            if u.content_type == ContentTypeId::Html && u.status_code == 200 {
                a.html_pages += 1;
            }

            let ct = u.content_type as i32;
            *a.type_count.entry(ct).or_insert(0) += 1;
            if let Some(sz) = u.size {
                *a.type_bytes.entry(ct).or_insert(0) += sz;
                let path = url_path(&u.url);
                push_top(
                    &mut a.top_largest,
                    (path, sz, u.content_type.name().to_string()),
                    |x| x.1,
                    MAX_TOP_N,
                );
            }

            if u.content_type == ContentTypeId::Image {
                a.image_count += 1;
                let sub = image_subtype(u.content_type_header.as_deref());
                if sub == "webp" {
                    a.webp_images += 1;
                } else if sub == "avif" {
                    a.avif_images += 1;
                }
                let e = a.image_subtypes.entry(sub).or_insert((0, 0));
                e.0 += 1;
                e.1 += u.size.unwrap_or(0);
            }

            // HTTP caching aggregates over cacheable static assets (200 responses).
            if u.status_code == 200 && u.is_static_file() {
                let f = u.cache_type_flags;
                a.cache_assets += 1;
                if f & CACHE_TYPE_HAS_CACHE_CONTROL != 0 {
                    a.cache_with_cache_control += 1;
                }
                if f & CACHE_TYPE_HAS_MAX_AGE != 0 {
                    a.cache_with_max_age += 1;
                }
                if f & CACHE_TYPE_HAS_ETAG != 0 {
                    a.cache_with_etag += 1;
                }
                if f & CACHE_TYPE_HAS_LAST_MODIFIED != 0 {
                    a.cache_with_last_modified += 1;
                }
                if f & CACHE_TYPE_HAS_IMMUTABLE != 0 {
                    a.cache_immutable += 1;
                }
                if f & CACHE_TYPE_HAS_NO_STORE != 0 {
                    a.cache_no_store += 1;
                }
                if f & CACHE_TYPE_NO_CACHE_HEADERS != 0 {
                    a.cache_missing_headers += 1;
                }
                let bucket = match u.cache_lifetime {
                    None | Some(0) => 0,
                    Some(s) if s < 3_600 => 1,
                    Some(s) if s < 86_400 => 2,
                    Some(s) if s < 2_592_000 => 3,
                    Some(s) if s < 31_536_000 => 4,
                    Some(_) => 5,
                };
                a.cache_lifetime_buckets[bucket] += 1;
            }

            // Compression coverage over text-based (compressible) assets (200 responses).
            if u.status_code == 200
                && matches!(
                    u.content_type,
                    ContentTypeId::Html
                        | ContentTypeId::Script
                        | ContentTypeId::Stylesheet
                        | ContentTypeId::Json
                        | ContentTypeId::Xml
                )
            {
                a.text_assets += 1;
                let enc = u.content_encoding.as_deref().unwrap_or("").to_lowercase();
                let is_br = enc.contains("br");
                let is_gzip = enc.contains("gzip") || enc.contains("deflate") || enc.contains("zstd");
                if is_br {
                    a.text_brotli += 1;
                    a.text_compressed += 1;
                } else if is_gzip {
                    a.text_gzip += 1;
                    a.text_compressed += 1;
                }
            }

            if u.request_time > 0.0 {
                rt_sum += u.request_time;
                rt_n += 1;
                rt_values.push(u.request_time);
                if u.request_time > a.rt_max {
                    a.rt_max = u.request_time;
                }
                if u.request_time > 1.0 {
                    a.slow_over_1s += 1;
                }
                if u.request_time > 2.0 {
                    a.slow_over_2s += 1;
                }
                let path = url_path(&u.url);
                push_top(
                    &mut a.top_slowest,
                    (path, u.request_time, u.size.unwrap_or(0)),
                    |x| (x.1 * 1000.0) as i64,
                    MAX_TOP_N,
                );
            }
        }
        a.rt_avg = if rt_n > 0 {
            (rt_sum / rt_n as f64 * 1000.0).round() / 1000.0
        } else {
            0.0
        };
        a.rt_p90 = if rt_values.is_empty() {
            0.0
        } else {
            rt_values.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
            let idx = (((rt_values.len() as f64) * 0.9).ceil() as usize)
                .saturating_sub(1)
                .min(rt_values.len() - 1);
            (rt_values[idx] * 1000.0).round() / 1000.0
        };
        a.rt_max = (a.rt_max * 1000.0).round() / 1000.0;
        a
    }

    fn status_dist_json(&self) -> Value {
        // Cap to the most frequent status codes so this stays bounded even with pathological
        // status-code cardinality; remaining codes are aggregated into "other".
        const MAX_CODES: usize = 25;
        let mut pairs: Vec<(i32, usize)> = self.status_dist.iter().map(|(k, v)| (*k, *v)).collect();
        pairs.sort_by_key(|p| std::cmp::Reverse(p.1));
        let mut m = serde_json::Map::new();
        let mut other = 0usize;
        for (i, (code, n)) in pairs.into_iter().enumerate() {
            if i < MAX_CODES {
                m.insert(code.to_string(), json!(n));
            } else {
                other += n;
            }
        }
        if other > 0 {
            m.insert("other".to_string(), json!(other));
        }
        Value::Object(m)
    }

    fn content_type_json(&self) -> Value {
        let mut v: Vec<Value> = self
            .type_count
            .iter()
            .map(|(ct, n)| {
                let name = ContentTypeId::from_i32(*ct).map(|c| c.name()).unwrap_or("Other");
                json!({ "type": name, "count": n, "total_bytes": self.type_bytes.get(ct).copied().unwrap_or(0) })
            })
            .collect();
        v.sort_by(|a, b| b["total_bytes"].as_i64().cmp(&a["total_bytes"].as_i64()));
        v.truncate(MAX_TOP_N);
        json!(v)
    }

    fn bytes_by_type_json(&self) -> Value {
        self.content_type_json()
    }

    fn image_subtypes_json(&self) -> Value {
        let mut v: Vec<Value> = self
            .image_subtypes
            .iter()
            .map(|(s, (n, b))| json!({ "subtype": s, "count": n, "total_bytes": b }))
            .collect();
        v.sort_by(|a, b| b["count"].as_i64().cmp(&a["count"].as_i64()));
        v.truncate(10);
        json!(v)
    }

    fn top_slowest_json(&self) -> Value {
        let v: Vec<Value> = self
            .top_slowest
            .iter()
            .map(|(p, t, s)| json!({ "path": p, "time_s": (t * 1000.0).round() / 1000.0, "size_bytes": s }))
            .collect();
        json!(v)
    }

    fn top_largest_json(&self) -> Value {
        let v: Vec<Value> = self
            .top_largest
            .iter()
            .map(|(p, b, t)| json!({ "path": p, "bytes": b, "type": t }))
            .collect();
        json!(v)
    }

    fn caching_json(&self) -> Value {
        json!({
            "static_assets_considered": self.cache_assets,
            "with_cache_control": self.cache_with_cache_control,
            "with_max_age": self.cache_with_max_age,
            "with_etag": self.cache_with_etag,
            "with_last_modified": self.cache_with_last_modified,
            "immutable": self.cache_immutable,
            "no_store": self.cache_no_store,
            "missing_cache_headers": self.cache_missing_headers,
            "lifetime_distribution": {
                "none_or_zero": self.cache_lifetime_buckets[0],
                "under_1h": self.cache_lifetime_buckets[1],
                "1h_to_1d": self.cache_lifetime_buckets[2],
                "1d_to_30d": self.cache_lifetime_buckets[3],
                "30d_to_1y": self.cache_lifetime_buckets[4],
                "over_1y": self.cache_lifetime_buckets[5],
            },
        })
    }

    fn compression_json(&self) -> Value {
        json!({
            "text_assets_considered": self.text_assets,
            "compressed": self.text_compressed,
            "brotli": self.text_brotli,
            "gzip_or_deflate": self.text_gzip,
            "uncompressed": self.text_assets.saturating_sub(self.text_compressed),
        })
    }

    fn top_external_domains_json(&self) -> Value {
        let mut v: Vec<(&String, &usize)> = self.external_domains.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1));
        v.truncate(15);
        json!(
            v.into_iter()
                .map(|(d, n)| json!({ "domain": d, "url_count": n }))
                .collect::<Vec<_>>()
        )
    }
}

fn push_top<T, F: Fn(&T) -> i64>(top: &mut Vec<T>, item: T, key: F, limit: usize) {
    top.push(item);
    top.sort_by_key(|b| std::cmp::Reverse(key(b)));
    top.truncate(limit);
}

fn image_subtype(header: Option<&str>) -> String {
    header
        .and_then(|h| h.split(';').next())
        .and_then(|h| h.trim().strip_prefix("image/"))
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "unknown".to_string())
}

fn url_host(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
}

fn url_path(url: &str) -> String {
    let p = url::Url::parse(url)
        .ok()
        .map(|u| {
            let mut s = u.path().to_string();
            if let Some(q) = u.query() {
                s.push('?');
                s.push_str(q);
            }
            s
        })
        .unwrap_or_else(|| url.to_string());
    truncate(&p, MAX_URL_LEN)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{}…", t)
    }
}

fn strip_ansi(s: &str) -> String {
    crate::utils::remove_ansi_colors(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_apl_codes_to_correct_areas() {
        // Transfer/format concerns belong to performance, not accessibility (regression B1).
        assert_eq!(area_for_apl_code("brotli-support"), "performance");
        assert_eq!(area_for_apl_code("webp-support"), "performance");
        assert_eq!(area_for_apl_code("avif-support"), "performance");
        assert_eq!(area_for_apl_code("static-assets-uncacheable"), "performance");
        assert_eq!(area_for_apl_code("slowUrls"), "performance");
        // Accessibility / best-practices markup checks.
        assert_eq!(area_for_apl_code("pages-without-lang"), "accessibility");
        assert_eq!(area_for_apl_code("pages-without-h1"), "accessibility");
        assert_eq!(area_for_apl_code("pages-without-image-alt-attributes"), "accessibility");
        // SEO (title/description uniqueness is duplicate-metadata → seo, caught before a11y).
        assert_eq!(area_for_apl_code("seo-noindex-sitewide"), "seo");
        assert_eq!(area_for_apl_code("seo-canonical-missing"), "seo");
        assert_eq!(area_for_apl_code("title-uniqueness"), "seo");
        assert_eq!(area_for_apl_code("meta-description-uniqueness"), "seo");
        // Security.
        assert_eq!(area_for_apl_code("security"), "security");
        assert_eq!(area_for_apl_code("ssl-certificate-expiry"), "security");
        // Everything else.
        assert_eq!(area_for_apl_code("redirects"), "infrastructure");
        assert_eq!(area_for_apl_code("404"), "infrastructure");
    }

    #[test]
    fn dns_findings_route_to_infrastructure() {
        // DNS/IPv6 records are infrastructure (taxonomy), not security.
        assert_eq!(area_for_apl_code("dns-ipv6-missing"), "infrastructure");
        assert_eq!(area_for_apl_code("dns-aliases"), "infrastructure");
        // genuine security codes still security:
        assert_eq!(area_for_apl_code("hsts-missing"), "security");
        assert_eq!(area_for_apl_code("csp-weak"), "security");
    }

    #[test]
    fn compression_json_reports_coverage() {
        let mut a = CrawlAggregates::compute(&[], 0);
        a.text_assets = 10;
        a.text_brotli = 6;
        a.text_gzip = 2;
        a.text_compressed = 8;
        let j = a.compression_json();
        assert_eq!(j["text_assets_considered"], 10);
        assert_eq!(j["compressed"], 8);
        assert_eq!(j["brotli"], 6);
        assert_eq!(j["gzip_or_deflate"], 2);
        assert_eq!(j["uncompressed"], 2);
    }

    #[test]
    fn ai_seo_overall_pct_parsing_is_strict() {
        // The `overall` cells are produced by us as "NN%"; the parser must read those and
        // reject anything else (so a malformed cell never poisons the average).
        let parse_pct = |s: &str| s.trim().trim_end_matches('%').trim().parse::<i64>().ok();
        assert_eq!(parse_pct("65%"), Some(65));
        assert_eq!(parse_pct("  72%  "), Some(72));
        assert_eq!(parse_pct("n/a"), None);
        assert_eq!(parse_pct(""), None);
    }

    #[test]
    fn cap_area_json_enforces_hard_cap() {
        // Oversized findings AND oversized area_stats must both be reined in.
        let big_findings: Vec<Value> = (0..3000).map(|i| json!({ "text": "x".repeat(200), "i": i })).collect();
        let mut obj = json!({
            "area": "infrastructure",
            "findings": big_findings,
            "area_stats": { "blob": "y".repeat(80_000) },
        });
        cap_area_json(&mut obj);
        let len = serde_json::to_string(&obj).map(|s| s.len()).unwrap_or(usize::MAX);
        assert!(len <= HARD_CAP, "capped length {} exceeds HARD_CAP {}", len, HARD_CAP);
    }
}
