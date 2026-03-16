// Integration tests: crawl crawler.siteone.io and verify output correctness.
//
// These tests require network access and a built binary.
// Run with: cargo test --test integration_crawl -- --ignored
//
// They are #[ignore] by default so `cargo test` stays fast and offline.
//
// Network tests use a serial mutex to prevent parallel crawls against the
// same server, which would cause rate-limiting and flaky failures.

mod common;

use common::{TempDir, run_crawler, run_crawler_json};
use std::path::Path;
use std::sync::Mutex;

/// Mutex to serialize all network tests that hit crawler.siteone.io.
/// Prevents parallel crawls from overwhelming the server.
static SERIAL: Mutex<()> = Mutex::new(());

/// Common crawler flags to be gentle on the remote server.
const GENTLE_FLAGS: [&str; 3] = ["--workers=2", "--max-reqs-per-sec=5", "--http-cache-dir="];

// =========================================================================
// 1. Full crawl of crawler.siteone.io — verify content type counts
// =========================================================================

#[test]
#[ignore]
fn crawl_siteone_content_type_counts() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());

    let mut args: Vec<&str> = vec!["--url=https://crawler.siteone.io", "--output=json"];
    args.extend_from_slice(&GENTLE_FLAGS);
    let json = run_crawler_json(&args);

    let tables = &json["tables"];
    let ct = &tables["content-types"];
    let rows = ct["rows"].as_array().expect("content-types rows");

    let find_count = |content_type: &str| -> i64 {
        rows.iter()
            .find(|r| r["contentType"].as_str() == Some(content_type))
            .and_then(|r| r["count"].as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    };

    let html_count = find_count("HTML");
    let js_count = find_count("JS");
    let css_count = find_count("CSS");
    let image_count = find_count("Image");

    // Verified baseline (March 2026): HTML=54, JS=5, CSS=3, Image=10
    // Allow ±5 tolerance for HTML (site may add/remove pages)
    assert!(
        (49..=59).contains(&html_count),
        "Expected ~54 HTML pages, got {}",
        html_count
    );
    assert!((3..=8).contains(&js_count), "Expected ~5 JS files, got {}", js_count);
    assert!((2..=6).contains(&css_count), "Expected ~3 CSS files, got {}", css_count);
    assert!(
        (7..=15).contains(&image_count),
        "Expected ~10 images, got {}",
        image_count
    );

    // Total URLs: ~73
    let total_urls = json["stats"]["totalUrls"].as_i64().expect("totalUrls");
    assert!(
        (65..=85).contains(&total_urls),
        "Expected ~73 total URLs, got {}",
        total_urls
    );

    // Only 200 and 404 status codes expected
    let status_counts = json["stats"]["countByStatus"].as_object().expect("countByStatus");
    let count_200 = status_counts.get("200").and_then(|v| v.as_i64()).unwrap_or(0);
    let count_404 = status_counts.get("404").and_then(|v| v.as_i64()).unwrap_or(0);
    assert!(count_200 > 60, "Expected >60 successful URLs, got {}", count_200);
    assert!(
        count_404 >= 0 && count_404 <= 10,
        "Expected 0-10 404s, got {}",
        count_404
    );

    // Quality score should be reasonable
    let overall_score = json["qualityScores"]["overall"]["score"]
        .as_f64()
        .expect("overall score");
    assert!(
        overall_score >= 7.0,
        "Expected overall score >= 7.0, got {}",
        overall_score
    );
}

// =========================================================================
// 2. Non-existent domain — verify exit code 3 and graceful handling
// =========================================================================

#[test]
#[ignore]
fn crawl_nonexistent_domain_exits_with_code_3() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());

    let domain = format!(
        "https://nonexistent-{}.invalid",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    let output = run_crawler(&[
        &format!("--url={}", domain),
        "--single-page",
        "--timeout=5",
        "--http-cache-dir=",
    ]);

    assert_eq!(
        output.status.code(),
        Some(3),
        "Expected exit code 3 for non-existent domain, got {:?}",
        output.status.code()
    );
}

// =========================================================================
// 3. Non-existent domain with --ci — verify exit code 10
// =========================================================================

#[test]
#[ignore]
fn crawl_nonexistent_domain_ci_exits_with_code_10() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());

    let domain = format!(
        "https://nonexistent-{}.invalid",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    let output = run_crawler(&[
        &format!("--url={}", domain),
        "--single-page",
        "--timeout=5",
        "--ci",
        "--http-cache-dir=",
    ]);

    assert_eq!(
        output.status.code(),
        Some(10),
        "Expected exit code 10 for CI gate with no pages, got {:?}",
        output.status.code()
    );
}

// =========================================================================
// 4. Offline export — verify key pages exist and links are relative
// =========================================================================

#[test]
#[ignore]
fn crawl_siteone_offline_export() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());

    let tmp = TempDir::new("offline");
    let offline_dir = tmp.path.join("site");

    let mut args: Vec<&str> = vec!["--url=https://crawler.siteone.io", &"--offline-export-dir=PLACEHOLDER"];
    // We need the offline_dir path as a string that lives long enough
    let offline_dir_str = format!("--offline-export-dir={}", offline_dir.display());
    args = vec!["--url=https://crawler.siteone.io", &offline_dir_str];
    args.extend_from_slice(&GENTLE_FLAGS);
    let output = run_crawler(&args);

    assert!(
        output.status.success(),
        "Crawler failed with {:?}",
        output.status.code()
    );

    // Key pages must exist
    assert!(offline_dir.join("index.html").exists(), "Missing index.html");
    assert!(
        offline_dir.join("introduction/overview/index.html").exists(),
        "Missing introduction/overview/index.html"
    );
    assert!(
        offline_dir
            .join("features/seo-and-opengraph-analysis/index.html")
            .exists(),
        "Missing features/seo-and-opengraph-analysis/index.html"
    );

    // Check relative links in index.html
    let index_html = std::fs::read_to_string(offline_dir.join("index.html")).expect("Failed to read index.html");
    // Should contain relative link to introduction/overview
    assert!(
        index_html.contains("introduction/overview/index.html"),
        "index.html should contain relative link to introduction/overview/index.html"
    );
    // Should contain relative CSS reference
    assert!(
        index_html.contains("_astro/index.BRwACyc2.css") || index_html.contains("_astro/"),
        "index.html should contain relative reference to CSS in _astro/"
    );
    // Should NOT contain absolute https://crawler.siteone.io links for internal pages
    // (external links like GitHub are OK)
    let internal_absolute_links: Vec<&str> = index_html
        .match_indices("href=\"https://crawler.siteone.io")
        .map(|(i, _)| &index_html[i..i.min(index_html.len()).min(i + 80)])
        .collect();
    assert!(
        internal_absolute_links.is_empty(),
        "Offline index.html should not contain absolute links to crawler.siteone.io: {:?}",
        &internal_absolute_links[..internal_absolute_links.len().min(3)]
    );

    // Check links in a subpage point correctly back up
    let overview_html = std::fs::read_to_string(offline_dir.join("introduction/overview/index.html"))
        .expect("Failed to read overview page");
    // From introduction/overview/ the root is ../../
    assert!(
        overview_html.contains("../../index.html") || overview_html.contains("../../"),
        "Overview page should have ../../ relative paths to root"
    );

    // Verify CSS and JS assets exist
    let has_css = std::fs::read_dir(offline_dir.join("_astro"))
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.path().extension().map(|ext| ext == "css").unwrap_or(false))
        })
        .unwrap_or(false);
    assert!(has_css, "Should have CSS files in _astro/");
}

// =========================================================================
// 5. Markdown export — verify pages and internal links use .md extension
// =========================================================================

#[test]
#[ignore]
fn crawl_siteone_markdown_export() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());

    let tmp = TempDir::new("markdown");
    let md_dir = tmp.path.join("md");

    let md_dir_str = format!("--markdown-export-dir={}", md_dir.display());
    let mut args: Vec<&str> = vec!["--url=https://crawler.siteone.io", &md_dir_str];
    args.extend_from_slice(&GENTLE_FLAGS);
    let output = run_crawler(&args);

    assert!(
        output.status.success(),
        "Crawler failed with {:?}",
        output.status.code()
    );

    // Key markdown files must exist
    assert!(md_dir.join("index.md").exists(), "Missing index.md");
    assert!(
        md_dir.join("introduction/overview/index.md").exists(),
        "Missing introduction/overview/index.md"
    );
    assert!(
        md_dir.join("features/seo-and-opengraph-analysis/index.md").exists(),
        "Missing features/seo-and-opengraph-analysis/index.md"
    );
    assert!(
        md_dir.join("configuration/command-line-options/index.md").exists(),
        "Missing configuration/command-line-options/index.md"
    );

    // Count total markdown files (baseline: ~51)
    let md_count = walkdir(md_dir.as_path(), "md");
    assert!(
        (45..=60).contains(&md_count),
        "Expected ~51 markdown files, got {}",
        md_count
    );

    // Check internal links in overview page use .md extension
    let overview_md = std::fs::read_to_string(md_dir.join("introduction/overview/index.md"))
        .expect("Failed to read overview markdown");

    // Internal links should be relative .md paths
    assert!(
        overview_md.contains("../../introduction/key-features/index.md"),
        "Overview should link to key-features/index.md"
    );
    assert!(
        overview_md.contains("../../getting-started/quick-start-guide/index.md"),
        "Overview should link to quick-start-guide/index.md"
    );

    // External links should remain as absolute URLs
    assert!(
        overview_md.contains("https://github.com/"),
        "External GitHub links should stay absolute"
    );

    // Check index.md links
    let index_md = std::fs::read_to_string(md_dir.join("index.md")).expect("Failed to read index.md");
    // Internal links should use .md extension, not .html
    // (index.html self-reference in nav logo is acceptable)
    let html_internal_links: Vec<&str> = index_md
        .lines()
        .filter(|line| {
            line.contains(".html)")
                && !line.contains("http://")
                && !line.contains("https://")
                && !line.contains("index.html)")
        })
        .collect();
    assert!(
        html_internal_links.is_empty(),
        "Markdown index.md should not have internal .html links: {:?}",
        &html_internal_links[..html_internal_links.len().min(3)]
    );
}

/// Count files with given extension recursively.
fn walkdir(dir: &Path, extension: &str) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += walkdir(&path, extension);
            } else if path.extension().map(|e| e == extension).unwrap_or(false) {
                count += 1;
            }
        }
    }
    count
}

// =========================================================================
// 6. Single page crawl — verify only one HTML page is fetched
// =========================================================================

#[test]
#[ignore]
fn crawl_siteone_single_page() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());

    let json = run_crawler_json(&[
        "--url=https://crawler.siteone.io",
        "--single-page",
        "--output=json",
        "--workers=2",
        "--max-reqs-per-sec=5",
        "--http-cache-dir=",
    ]);

    let tables = &json["tables"];
    let ct = &tables["content-types"];
    let rows = ct["rows"].as_array().expect("content-types rows");

    let html_count: i64 = rows
        .iter()
        .find(|r| r["contentType"].as_str() == Some("HTML"))
        .and_then(|r| r["count"].as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    assert_eq!(html_count, 1, "Single page should crawl exactly 1 HTML page");

    // Should still fetch assets (JS, CSS, images)
    let total_urls = json["stats"]["totalUrls"].as_i64().unwrap_or(0);
    assert!(
        total_urls > 1,
        "Single page should still fetch assets, got {} total URLs",
        total_urls
    );
}

// =========================================================================
// 7. --version and --help flags
// =========================================================================

#[test]
fn version_flag_exits_with_code_2() {
    let output = run_crawler(&["--version"]);
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Version:"), "Expected version output, got: {}", stdout);
}

#[test]
fn help_flag_exits_with_code_2() {
    let output = run_crawler(&["--help"]);
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--url") && stdout.contains("--output"),
        "Help should list --url and --output options"
    );
}

// =========================================================================
// 8. Invalid option — verify error and exit code 101
// =========================================================================

#[test]
fn invalid_option_exits_with_code_101() {
    let output = run_crawler(&["--url=https://example.com", "--nonexistent-option=foo"]);
    assert_eq!(
        output.status.code(),
        Some(101),
        "Expected exit code 101 for unknown option"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unknown options: --nonexistent-option=foo"),
        "Error should mention the unknown option, got: {}",
        stderr
    );
}

#[test]
fn unknown_option_after_bool_flag_detected() {
    // Regression: bool flags (--ci, --single-page, --debug) must NOT consume
    // the next argument as their "value", otherwise unknown options get skipped.
    let output = run_crawler(&["--url=https://example.com", "--ci", "--no-cach"]);
    assert_eq!(
        output.status.code(),
        Some(101),
        "Expected exit code 101 for --no-cach after --ci"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--no-cach"),
        "Error should mention --no-cach, got: {}",
        stderr
    );
}

#[test]
fn unknown_option_typo_without_value() {
    let output = run_crawler(&["--url=https://example.com", "--signle-page"]);
    assert_eq!(
        output.status.code(),
        Some(101),
        "Expected exit code 101 for misspelled --signle-page"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--signle-page"),
        "Error should mention --signle-page, got: {}",
        stderr
    );
}
