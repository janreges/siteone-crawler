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

use common::{
    LocalServer, RecordingServer, Redirect, RedirectServer, Route, TempDir, run_built_crawler, run_crawler,
    run_crawler_json,
};
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
// 6b. SSL/TLS detection is pure-Rust (no openssl) and correct
// =========================================================================

#[test]
#[ignore]
fn ssl_tls_detection_is_pure_rust_and_correct() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());

    // crawler.siteone.io serves a valid, publicly-trusted cert that negotiates
    // modern TLS. The SSL/TLS analyzer must detect this entirely in Rust
    // (rustls + raw probe), with no dependency on the openssl/sh/timeout binaries.
    let output = run_crawler(&[
        "--url=https://crawler.siteone.io",
        "--single-page",
        "--output=text",
        "--workers=2",
        "--max-reqs-per-sec=5",
        "--http-cache-dir=",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Modern protocol(s) detected via rustls version-pinned handshakes.
    assert!(
        stdout.contains("TLSv1.2") || stdout.contains("TLSv1.3"),
        "expected a modern TLS protocol in the SSL/TLS table"
    );
    // In-process trust verification against the system CA store.
    assert!(
        stdout.contains("Trusted by system CA store"),
        "expected a trusted-chain verdict in the SSL/TLS table"
    );
    // The old openssl-missing false positive must NOT appear on a TLS 1.2-capable host.
    assert!(
        !stdout.contains("TLSv1.2 is not supported"),
        "must not report TLS 1.2 as unsupported on a TLS 1.2-capable host"
    );
    // BadSSL-inspired positive accents on a well-configured host (modern TLS only, strong key).
    assert!(
        stdout.contains("Only modern TLS protocols are supported"),
        "expected the modern-protocols OK finding on a host without legacy protocols"
    );
    assert!(
        stdout.contains("strong public key"),
        "expected the strong-public-key OK finding"
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
        stderr.contains("Unknown options: --nonexistent-option"),
        "Error should mention the unknown option, got: {}",
        stderr
    );
    assert!(
        !stderr.contains("--nonexistent-option=foo"),
        "Error should not echo the value (it may be a secret), got: {}",
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

// =========================================================================
// --html-to-markdown: standalone HTML-to-Markdown conversion (no network)
// =========================================================================

#[test]
fn html_to_markdown_basic_conversion() {
    let tmp = TempDir::new("htm-convert");
    let html_path = tmp.path.join("page.html");
    std::fs::write(
        &html_path,
        "<html><body><h1>Hello World</h1><p>Paragraph with <strong>bold</strong> text.</p>\
         <ul><li>Item 1</li><li>Item 2</li></ul></body></html>",
    )
    .unwrap();

    let output = run_crawler(&[&format!("--html-to-markdown={}", html_path.display())]);
    assert!(output.status.success(), "Should exit 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("# Hello World"), "Should contain h1: {}", stdout);
    assert!(stdout.contains("**bold**"), "Should contain bold: {}", stdout);
    assert!(stdout.contains("- Item 1"), "Should contain list: {}", stdout);
    assert!(stdout.contains("- Item 2"), "Should contain list item 2: {}", stdout);
}

#[test]
fn html_to_markdown_output_to_file() {
    let tmp = TempDir::new("htm-output");
    let html_path = tmp.path.join("input.html");
    let md_path = tmp.path.join("output.md");
    std::fs::write(&html_path, "<html><body><h1>Title</h1><p>Content</p></body></html>").unwrap();

    let output = run_crawler(&[
        &format!("--html-to-markdown={}", html_path.display()),
        &format!("--html-to-markdown-output={}", md_path.display()),
    ]);
    assert!(output.status.success(), "Should exit 0");
    assert!(md_path.exists(), "Output file should exist");

    let md_content = std::fs::read_to_string(&md_path).unwrap();
    assert!(
        md_content.contains("# Title"),
        "Output file should contain heading: {}",
        md_content
    );

    // stdout should be empty (output went to file)
    assert!(output.stdout.is_empty(), "stdout should be empty when writing to file");

    // status message should be on stderr
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Markdown written to"),
        "stderr should contain success message: {}",
        stderr
    );
}

#[test]
fn html_to_markdown_nonexistent_file() {
    let output = run_crawler(&["--html-to-markdown=/tmp/siteone_nonexistent_file_12345.html"]);
    assert_eq!(output.status.code(), Some(101), "Should exit 101 for nonexistent file");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not exist"),
        "Error should mention file doesn't exist: {}",
        stderr
    );
}

#[test]
fn html_to_markdown_with_disable_images() {
    let tmp = TempDir::new("htm-no-img");
    let html_path = tmp.path.join("page.html");
    std::fs::write(
        &html_path,
        "<html><body><h1>Title</h1><img src=\"photo.jpg\" alt=\"Photo\"><p>Text</p></body></html>",
    )
    .unwrap();

    let output = run_crawler(&[
        &format!("--html-to-markdown={}", html_path.display()),
        "--markdown-disable-images",
    ]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("photo.jpg"), "Images should be removed: {}", stdout);
    assert!(stdout.contains("# Title"));
    assert!(stdout.contains("Text"));
}

#[test]
fn html_to_markdown_preserves_original_links() {
    let tmp = TempDir::new("htm-links");
    let html_path = tmp.path.join("page.html");
    std::fs::write(
        &html_path,
        r#"<html><body><h1>Title</h1><a href="/about.html">About</a>
           <a href="https://example.com">External</a>
           <a href="tel:+420123456">Call</a></body></html>"#,
    )
    .unwrap();

    let output = run_crawler(&[&format!("--html-to-markdown={}", html_path.display())]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Links should NOT be rewritten to .md (standalone mode)
    assert!(
        stdout.contains("/about.html"),
        "HTML links should be preserved as-is: {}",
        stdout
    );
    assert!(
        stdout.contains("https://example.com"),
        "External links preserved: {}",
        stdout
    );
    assert!(stdout.contains("tel:+420123456"), "Tel links preserved: {}", stdout);
}

#[test]
fn html_to_markdown_with_exclude_selector() {
    let tmp = TempDir::new("htm-exclude");
    let html_path = tmp.path.join("page.html");
    std::fs::write(
        &html_path,
        "<html><body><h1>Title</h1><nav><a href=\"/\">Home</a></nav><p>Main content</p></body></html>",
    )
    .unwrap();

    let output = run_crawler(&[
        &format!("--html-to-markdown={}", html_path.display()),
        "--markdown-exclude-selector=nav",
    ]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Main content"), "Content should be present: {}", stdout);
    assert!(!stdout.contains("Home"), "Nav should be excluded: {}", stdout);
}

#[test]
fn html_to_markdown_aria_hidden_excluded() {
    let tmp = TempDir::new("htm-aria");
    let html_path = tmp.path.join("page.html");
    std::fs::write(
        &html_path,
        r#"<html><body><h1>Title</h1>
           <div aria-hidden="true"><p>Hidden mega menu</p></div>
           <p>Visible content</p></body></html>"#,
    )
    .unwrap();

    let output = run_crawler(&[&format!("--html-to-markdown={}", html_path.display())]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Visible content"));
    assert!(
        !stdout.contains("Hidden mega menu"),
        "aria-hidden should be excluded: {}",
        stdout
    );
}

#[test]
fn html_to_markdown_output_without_input_fails() {
    let output = run_crawler(&["--html-to-markdown-output=/tmp/out.md"]);
    assert_eq!(
        output.status.code(),
        Some(101),
        "Should exit 101 when output is set without input"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--html-to-markdown-output requires --html-to-markdown"),
        "Should mention missing input: {}",
        stderr
    );
}

#[test]
fn html_to_markdown_with_move_before_h1() {
    let tmp = TempDir::new("htm-move-h1");
    let html_path = tmp.path.join("page.html");
    std::fs::write(
        &html_path,
        "<html><body><nav><a href=\"/\">Home</a><a href=\"/about\">About</a></nav>\
         <h1>Main Title</h1><p>Page body</p></body></html>",
    )
    .unwrap();

    let output = run_crawler(&[
        &format!("--html-to-markdown={}", html_path.display()),
        "--markdown-move-content-before-h1-to-end",
    ]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("# Main Title"),
        "Should start with h1 heading: {}",
        stdout
    );
    assert!(stdout.contains("Page body"));
}

// =========================================================================
// --url-list: crawl a bounded set of URLs from a file
// =========================================================================

#[test]
fn url_list_nonexistent_file_exits_with_101() {
    let output = run_crawler(&["--url-list=/nonexistent/path/urls.txt"]);
    assert_eq!(
        output.status.code(),
        Some(101),
        "Expected exit code 101 for nonexistent URL list file"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("/nonexistent/path/urls.txt"),
        "Error should mention the file path, got: {}",
        stderr
    );
}

#[test]
fn url_list_empty_file_exits_with_101() {
    let tmp = TempDir::new("url-list-empty");
    let list_path = tmp.path.join("urls.txt");
    // Only blank lines and comments — no actual URLs.
    std::fs::write(&list_path, "\n# just a comment\n\n   \n").unwrap();

    let output = run_crawler(&[&format!("--url-list={}", list_path.display())]);
    assert_eq!(
        output.status.code(),
        Some(101),
        "Expected exit code 101 for empty URL list file"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("empty list") || stderr.contains("no URLs"),
        "Error should mention empty list, got: {}",
        stderr
    );
}

#[test]
fn url_list_all_invalid_urls_exits_with_101() {
    let tmp = TempDir::new("url-list-invalid");
    let list_path = tmp.path.join("urls.txt");
    // Non-empty lines, but none is an absolute http(s) URL.
    std::fs::write(&list_path, "notaurl\n/relative/path\nexample.com/no-scheme\n").unwrap();

    let output = run_crawler(&[&format!("--url-list={}", list_path.display())]);
    assert_eq!(
        output.status.code(),
        Some(101),
        "Expected exit code 101 when the URL list has no valid http(s) URLs"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no valid http(s) URLs"),
        "Error should mention there are no valid URLs, got: {}",
        stderr
    );
    assert!(
        stderr.contains("skipped"),
        "Invalid lines should be reported as skipped, got: {}",
        stderr
    );
}

#[test]
fn url_list_all_invalid_with_explicit_url_does_not_fail_config() {
    // When --url is provided, an all-invalid --url-list must NOT abort with a
    // config error (101): the skipped lines are warned about and the crawl
    // proceeds with --url. Here --url points at a closed local port, so the
    // crawl runs but finds no pages (exit 3) — crucially NOT 101.
    let tmp = TempDir::new("url-list-invalid-with-url");
    let list_path = tmp.path.join("urls.txt");
    std::fs::write(&list_path, "notaurl\n/relative\n").unwrap();

    let output = run_crawler(&[
        "--url=http://127.0.0.1:9/",
        &format!("--url-list={}", list_path.display()),
        &format!("--output-html-report={}", tmp.path.join("r.html").display()),
        "--single-page",
        "--timeout=2",
        "--http-cache-dir=",
    ]);
    assert_ne!(
        output.status.code(),
        Some(101),
        "With --url set, an all-invalid url-list must not be a config error (101)"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("skipped"),
        "Invalid lines should still be reported as skipped, got: {}",
        stderr
    );
}

#[test]
#[ignore]
fn url_list_crawls_listed_urls() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());

    let tmp = TempDir::new("url-list-crawl");
    let list_path = tmp.path.join("urls.txt");
    std::fs::write(
        &list_path,
        "# two known HTML pages on crawler.siteone.io\n\
         https://crawler.siteone.io/\n\
         https://crawler.siteone.io/introduction/overview/\n",
    )
    .unwrap();

    let list_arg = format!("--url-list={}", list_path.display());
    let mut args: Vec<&str> = vec![&list_arg, "--single-page", "--disable-all-assets", "--output=json"];
    args.extend_from_slice(&GENTLE_FLAGS);
    let json = run_crawler_json(&args);

    // With --single-page and assets disabled, exactly the 2 listed HTML pages are crawled.
    let total_urls = json["stats"]["totalUrls"].as_i64().expect("totalUrls");
    assert_eq!(total_urls, 2, "Expected exactly 2 crawled URLs, got {}", total_urls);

    let count_200 = json["stats"]["countByStatus"]
        .as_object()
        .and_then(|m| m.get("200"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    assert_eq!(count_200, 2, "Expected 2 successful pages, got {}", count_200);
}

// ---------------------------------------------------------------------------
// Machine-readable event stream (--events-file)
//
// These run against the built-in server, so they need no network and no fixed delays.
// ---------------------------------------------------------------------------

/// Runs only the Headers analyzer, so local test crawls stay fast and their output small.
const LOCAL_ANALYZERS: &str = "--analyzer-filter-regex=/Headers/";

/// Writes an index page that links to `pages` further pages.
fn write_site(dir: &Path, pages: usize) {
    std::fs::create_dir_all(dir).expect("site dir");
    let links: String = (1..=pages)
        .map(|n| format!("<a href=\"/page-{n}.html\">Page {n}</a>"))
        .collect();
    std::fs::write(
        dir.join("index.html"),
        format!("<html><head><title>Home</title></head><body>{links}</body></html>"),
    )
    .expect("index.html");
    for n in 1..=pages {
        std::fs::write(
            dir.join(format!("page-{n}.html")),
            format!("<html><head><title>Page {n}</title></head><body><p>Page {n}</p></body></html>"),
        )
        .expect("page");
    }
}

/// Hosts driving the crawler rely on this instead of parsing the human-readable output, so
/// the shape of the stream is part of the contract.
#[test]
fn events_file_records_the_whole_run() {
    let tmp = TempDir::new("events");
    let site = tmp.path.join("site");
    write_site(&site, 3);
    let server = LocalServer::start(&site);
    let events = tmp.path.join("events.ndjson");
    let report = tmp.path.join("report.html");

    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        &format!("--events-file={}", events.display()),
        &format!("--output-html-report={}", report.display()),
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let text = std::fs::read_to_string(&events).expect("the event file exists");
    let lines: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("one JSON object per line"))
        .collect();

    assert_eq!(lines.first().unwrap()["type"], "runStarted");
    assert_eq!(lines.first().unwrap()["protocol"], 1);
    assert_eq!(lines.last().unwrap()["type"], "runFinished");
    assert_eq!(lines.last().unwrap()["outcome"], "success");
    assert_eq!(lines.last().unwrap()["interrupted"], false);

    let urls: Vec<&serde_json::Value> = lines.iter().filter(|e| e["type"] == "url").collect();
    assert!(!urls.is_empty(), "every crawled URL is reported");
    assert!(urls[0]["status"].is_i64(), "with its status");
    assert!(urls[0]["timeMs"].is_u64(), "and how long it took");
    assert!(urls[0]["total"].is_u64(), "and how much is left");

    assert!(
        lines.iter().any(|e| e["type"] == "artifact" && e["kind"] == "html"),
        "the HTML report is reported as an artifact, path included"
    );
    assert!(
        lines.iter().any(|e| e["type"] == "phase" && e["name"] == "crawl"),
        "the crawl phase brackets the work"
    );
}

/// Without `--events-file` nothing changes: the stream is entirely opt-in.
#[test]
fn no_events_file_means_no_events() {
    let tmp = TempDir::new("no-events");
    let site = tmp.path.join("site");
    write_site(&site, 0);
    let server = LocalServer::start(&site);
    let report = tmp.path.join("report.html");

    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        "--single-page",
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        &format!("--output-html-report={}", report.display()),
    ]);
    assert_eq!(output.status.code(), Some(0));
    assert!(!tmp.path.join("events.ndjson").exists());
}

/// `stop` on stdin winds the crawl down like Ctrl+C: the reports for what was already
/// crawled still get written, and the stream says the run was cancelled rather than failed.
#[test]
fn control_stdin_stops_the_crawl_but_keeps_the_reports() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let tmp = TempDir::new("control-stdin");
    let site = tmp.path.join("site");
    write_site(&site, 20);
    let server = LocalServer::start(&site);
    let events = tmp.path.join("events.ndjson");
    let report = tmp.path.join("report.html");

    // One request per second keeps most of the site queued when `stop` arrives.
    let mut child = Command::new(env!("CARGO_BIN_EXE_siteone-crawler"))
        .args([
            "--config-file=/dev/null",
            &format!("--url={}", server.url()),
            "--workers=1",
            "--max-reqs-per-sec=1",
            LOCAL_ANALYZERS,
            "--http-cache-dir=",
            "--control-stdin",
            &format!("--events-file={}", events.display()),
            &format!("--output-html-report={}", report.display()),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("crawler starts");

    // Wait for the first crawled page rather than a fixed delay, then ask it to stop.
    let first_page_done = (0..300).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(100));
        std::fs::read_to_string(&events).is_ok_and(|text| text.contains(r#""type":"url""#))
    });
    let mut stdin = child.stdin.take().expect("stdin is piped");
    writeln!(stdin, "stop").expect("stop is accepted");
    stdin.flush().ok();
    // Keep the handle open: end-of-input means stop too, and we are testing the command.
    let status = child.wait().expect("crawler exits");
    drop(stdin);

    assert!(first_page_done, "the first crawled page is reported");
    assert_eq!(status.code(), Some(0), "a graceful stop is a clean exit");
    assert!(
        report.exists(),
        "the report for the pages already crawled is still written"
    );

    let text = std::fs::read_to_string(&events).expect("the event file exists");
    let last: serde_json::Value = serde_json::from_str(text.lines().last().expect("at least one event")).unwrap();
    assert_eq!(last["outcome"], "cancelled");
    assert_eq!(last["interrupted"], true);
}

/// `cached` says the body came from the local HTTP cache (`--http-cache-dir`), not merely that
/// the server sent caching headers.
#[test]
fn events_mark_only_local_http_cache_hits_as_cached() {
    let tmp = TempDir::new("events-cache");
    let site = tmp.path.join("site");
    write_site(&site, 0);
    let server = LocalServer::start(&site);

    let cache = tmp.path.join("cache");
    let crawl = |name: &str| -> Vec<serde_json::Value> {
        let events = tmp.path.join(format!("{name}.ndjson"));
        let output = run_built_crawler(&[
            "--config-file=/dev/null",
            &format!("--url={}", server.url()),
            "--single-page",
            LOCAL_ANALYZERS,
            &format!("--http-cache-dir={}", cache.display()),
            &format!("--events-file={}", events.display()),
            &format!(
                "--output-html-report={}",
                tmp.path.join(format!("{name}.html")).display()
            ),
        ]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::read_to_string(&events)
            .expect("the event file exists")
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("one JSON object per line"))
            .filter(|event| event["type"] == "url")
            .collect()
    };

    let cold = crawl("cold");
    assert!(
        !cold.is_empty() && cold.iter().all(|e| e["cached"] == false),
        "an empty cache means network fetches: {cold:?}"
    );
    let warm = crawl("warm");
    assert!(
        !warm.is_empty() && warm.iter().all(|e| e["cached"] == true),
        "a warm cache is reported as such: {warm:?}"
    );
}

fn brotli_compress(data: &[u8]) -> Vec<u8> {
    let mut writer = brotli::CompressorWriter::new(Vec::new(), 4096, 5, 22);
    std::io::Write::write_all(&mut writer, data).expect("compressing in memory");
    writer.into_inner()
}

/// #107: a response served from the local HTTP cache keeps `Content-Encoding` and is not decoded a
/// second time, so the second crawl reports exactly what the first one did.
#[test]
fn http_cache_hit_keeps_content_encoding_and_decoded_size() {
    let tmp = TempDir::new("brotli-cache");
    let page = format!(
        "<!DOCTYPE html><html lang=\"en\"><head><title>Cached</title></head><body><p>{}</p></body></html>",
        "Cached compressed content. ".repeat(200)
    );
    let server = RecordingServer::start(vec![Route {
        path: "/",
        headers: vec![
            ("Content-Type", "text/html; charset=utf-8".to_string()),
            ("Content-Encoding", "br".to_string()),
        ],
        body: brotli_compress(page.as_bytes()),
    }]);
    let cache = tmp.path.join("cache");
    let crawl = || -> serde_json::Value {
        let output = run_built_crawler(&[
            "--config-file=/dev/null",
            &format!("--url={}", server.url()),
            "--single-page",
            "--output=json",
            "--extra-columns=Content-Encoding",
            LOCAL_ANALYZERS,
            &format!("--http-cache-dir={}", cache.display()),
            "--output-html-report=",
            "--output-json-file=",
            "--output-text-file=",
        ]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("JSON on stdout")
    };
    let page_requests = || {
        server
            .requests()
            .iter()
            .filter(|head| head.starts_with("GET / "))
            .count()
    };

    let live = crawl();
    assert_eq!(page_requests(), 1);
    let cached = crawl();
    assert_eq!(page_requests(), 1, "the second crawl is served from the HTTP cache");

    for json in [&live, &cached] {
        let row = &json["results"][0];
        assert_eq!(row["extras"]["Content-Encoding"], "br", "{row}");
        assert_eq!(row["size"], page.len() as u64, "{row}");
    }
}

/// #107: the crawler must see the server's `Content-Encoding` (the HTTP client used to strip it
/// while decompressing), so Brotli pages pass the Brotli check; the size stays the decoded length.
#[test]
fn brotli_response_keeps_content_encoding_and_passes_the_brotli_check() {
    let page = format!(
        "<!DOCTYPE html><html lang=\"en\"><head><title>Brotli</title></head><body><p>{}</p></body></html>",
        "Compressed content. ".repeat(200)
    );
    let server = RecordingServer::start(vec![Route {
        path: "/",
        headers: vec![
            ("Content-Type", "text/html; charset=utf-8".to_string()),
            ("Content-Encoding", "br".to_string()),
            ("Vary", "Accept-Encoding".to_string()),
        ],
        body: brotli_compress(page.as_bytes()),
    }]);

    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        "--single-page",
        "--output=json",
        "--extra-columns=Content-Encoding",
        "--analyzer-filter-regex=/BestPractice/",
        "--http-cache-dir=",
        "--output-html-report=",
        "--output-json-file=",
        "--output-text-file=",
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON on stdout");

    let row = &json["results"][0];
    assert_eq!(row["extras"]["Content-Encoding"], "br");
    assert_eq!(row["size"], page.len() as u64, "size is the decoded body length");
    let brotli = json["summary"]["items"]
        .as_array()
        .expect("summary items")
        .iter()
        .find(|item| item["aplCode"] == "brotli-support")
        .expect("the Brotli check ran");
    assert_eq!(brotli["status"], "OK", "{brotli}");

    let requests = server.requests();
    assert!(
        requests
            .iter()
            .any(|head| head.to_ascii_lowercase().contains("accept-encoding: gzip, deflate, br")),
        "the crawler still asks for compressed responses: {requests:?}"
    );
}

/// An IP-literal host has no DNS records: the DNS analyzer says so instead of querying the
/// resolver for "127.0.0.1" (which sat out timeouts and ended in a critical finding).
#[test]
fn dns_analysis_is_skipped_for_ip_literal_hosts() {
    let tmp = TempDir::new("dns-ip-literal");
    let site = tmp.path.join("site");
    write_site(&site, 0);
    let server = LocalServer::start(&site);

    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        "--single-page",
        "--output=json",
        "--analyzer-filter-regex=/Dns/",
        "--http-cache-dir=",
        "--output-html-report=",
        "--output-json-file=",
        "--output-text-file=",
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON on stdout");

    assert_eq!(
        json["tables"]["dns"]["rows"][0]["info"],
        "DNS resolution does not apply: the crawled host 127.0.0.1 is an IP address."
    );
    let dns = json["summary"]["items"]
        .as_array()
        .expect("summary items")
        .iter()
        .find(|item| item["aplCode"] == "dns")
        .expect("a DNS summary item");
    assert_eq!(dns["status"], "INFO", "{dns}");
}

/// #21: a `--header` on the command line overrides the same header from the config file, which
/// comes first; only one value is sent.
#[test]
fn command_line_header_overrides_the_config_file() {
    let tmp = TempDir::new("header-override");
    let config = tmp.path.join("crawler.conf");
    std::fs::write(&config, "--header=Cookie: session=from-config\n").expect("config file");
    let site = RecordingServer::start(vec![Route {
        path: "/",
        headers: vec![("Content-Type", "text/html; charset=utf-8".to_string())],
        body: b"<html><head><title>Home</title></head><body>Home</body></html>".to_vec(),
    }]);

    let output = run_built_crawler(&[
        &format!("--config-file={}", config.display()),
        &format!("--url={}", site.url()),
        "--single-page",
        "--header=Cookie: session=from-cli",
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--output-html-report=",
        "--output-json-file=",
        "--output-text-file=",
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let requests = site.requests();
    assert!(!requests.is_empty(), "the crawled host was requested");
    for head in &requests {
        let head = head.to_ascii_lowercase();
        let cookies: Vec<&str> = head.lines().filter(|line| line.starts_with("cookie:")).collect();
        assert_eq!(cookies, vec!["cookie: session=from-cli"], "{head}");
    }
}

/// #21: `--header` values reach the crawled host (replacing a default header of the same name)
/// but never another domain, and they are masked in the JSON output.
#[test]
fn custom_headers_reach_the_crawled_host_only() {
    let external = RecordingServer::start(vec![Route {
        path: "/logo.png",
        headers: vec![("Content-Type", "image/png".to_string())],
        body: b"\x89PNG\r\n\x1a\n".to_vec(),
    }]);
    let site = RecordingServer::start(vec![Route {
        path: "/",
        headers: vec![("Content-Type", "text/html; charset=utf-8".to_string())],
        body: format!(
            "<html><head><title>Home</title></head><body><img src=\"http://external.test:{}/logo.png\" alt=\"Logo\"></body></html>",
            external.port()
        )
        .into_bytes(),
    }]);

    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", site.url()),
        "--single-page",
        "--output=json",
        "--header=Cookie: session=SECRET_SUFFIX",
        "-H",
        "User-Agent: CustomAgent/1.0",
        // A second domain on a local server: `external.test` is resolved to 127.0.0.1.
        &format!("--resolve=external.test:{}:127.0.0.1", external.port()),
        "--allowed-domain-for-external-files=external.test",
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--output-html-report=",
        "--output-json-file=",
        "--output-text-file=",
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let site_requests = site.requests();
    assert!(!site_requests.is_empty(), "the crawled host was requested");
    for head in &site_requests {
        let head = head.to_ascii_lowercase();
        assert!(head.contains("cookie: session=secret_suffix"), "{head}");
        let user_agents: Vec<&str> = head.lines().filter(|line| line.starts_with("user-agent:")).collect();
        assert_eq!(
            user_agents,
            vec!["user-agent: customagent/1.0"],
            "the default is replaced: {head}"
        );
    }

    let external_requests = external.requests();
    assert!(
        !external_requests.is_empty(),
        "the image on the other domain was fetched"
    );
    for head in &external_requests {
        let head = head.to_ascii_lowercase();
        assert!(!head.contains("cookie:"), "{head}");
        assert!(!head.contains("customagent"), "{head}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("SECRET_SUFFIX"),
        "header values are masked in the output"
    );
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("JSON on stdout");
    assert_eq!(
        json["crawler"]["finalUserAgent"], "CustomAgent/1.0",
        "reports show the User-Agent that the crawled host received"
    );
    assert_eq!(
        json["options"]["httpHeaders"],
        serde_json::json!(["Cookie: ***", "User-Agent: ***"])
    );
}

/// #104: with `--progress-interval` the console gets periodic progress lines instead of one table
/// row per URL, while the text report keeps every row.
#[test]
fn progress_interval_replaces_url_rows_on_stdout() {
    let tmp = TempDir::new("progress-interval");
    let site = tmp.path.join("site");
    write_site(&site, 3);
    let server = LocalServer::start(&site);
    let text_report = tmp.path.join("report.txt");

    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--progress-interval=60",
        // Colored notices end with an ANSI reset that would prefix the next console line.
        "--no-color",
        "--output-html-report=",
        "--output-json-file=",
        &format!("--output-text-file={}", text_report.display()),
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("/page-2.html"), "no per-URL rows on stdout: {stdout}");
    let progress: Vec<&str> = stdout.lines().filter(|line| line.starts_with("Progress: ")).collect();
    assert_eq!(
        progress.len(),
        1,
        "a crawl shorter than the interval prints only the final line: {stdout}"
    );
    assert!(progress[0].starts_with("Progress: 4/4 (100%) | "), "{}", progress[0]);

    let report = std::fs::read_to_string(&text_report).expect("the text report is written");
    for page in ["/page-1.html", "/page-2.html", "/page-3.html"] {
        assert!(report.contains(page), "the text report keeps the row of {page}");
    }
}

/// #104: with `--progress-interval` the console still gets the rows of failed URLs, right when
/// they finish, so a CI log shows what failed.
#[test]
fn progress_interval_keeps_rows_of_failed_urls_on_stdout() {
    let html = |body: &str| format!("<html><head><title>T</title></head><body>{body}</body></html>");
    let server = RecordingServer::start(vec![
        Route {
            path: "/",
            headers: vec![("Content-Type", "text/html; charset=utf-8".to_string())],
            body: html(r#"<a href="/ok.html">OK</a> <a href="/boom">Boom</a> <a href="/missing">Missing</a>"#)
                .into_bytes(),
        },
        Route {
            path: "/ok.html",
            headers: vec![("Content-Type", "text/html; charset=utf-8".to_string())],
            body: html("<p>OK</p>").into_bytes(),
        },
        Route {
            path: "/boom",
            headers: vec![
                ("Status", "500 Internal Server Error".to_string()),
                ("Content-Type", "text/html; charset=utf-8".to_string()),
            ],
            body: html("<p>Boom</p>").into_bytes(),
        },
    ]);

    let output = run_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--progress-interval=60",
        "--no-color",
        "--output-html-report=",
        "--output-json-file=",
        "--output-text-file=",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // The URL rows come before the final progress line; the tables after it may list URLs too.
    let rows: Vec<&str> = stdout
        .lines()
        .take_while(|line| !line.starts_with("Progress: "))
        .filter(|line| line.contains(" | "))
        .collect();
    let row_of = |path: &str| rows.iter().find(|row| row.contains(&format!(" {path} ")));
    assert!(
        row_of("/boom").is_some_and(|row| row.contains("500")),
        "the 500 row is on stdout: {stdout}"
    );
    assert!(
        row_of("/missing").is_some_and(|row| row.contains("404")),
        "the 404 row is on stdout: {stdout}"
    );
    assert!(row_of("/ok.html").is_none(), "no row for the 200 URL: {stdout}");
    assert!(
        stdout.lines().any(|line| line.starts_with("Progress: 4/4 (100%) | ")),
        "{stdout}"
    );
}

/// #104: `--hide-progress-bar` also hides the `--progress-interval` lines in JSON mode.
#[test]
fn hide_progress_bar_suppresses_progress_lines_in_json_mode() {
    let tmp = TempDir::new("progress-interval-hidden");
    let site = tmp.path.join("site");
    write_site(&site, 1);
    let server = LocalServer::start(&site);

    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--output=json",
        "--progress-interval=60",
        "--hide-progress-bar",
        "--output-html-report=",
        "--output-json-file=",
        "--output-text-file=",
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("Progress"), "{stderr}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout stays pure JSON");
    assert_eq!(json["results"].as_array().map(Vec::len), Some(2));
}

/// #104: in JSON mode the stderr progress follows the same interval, as plain lines.
#[test]
fn progress_interval_prints_plain_lines_to_stderr_in_json_mode() {
    let tmp = TempDir::new("progress-interval-json");
    let site = tmp.path.join("site");
    write_site(&site, 3);
    let server = LocalServer::start(&site);

    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--output=json",
        "--progress-interval=60",
        "--output-html-report=",
        "--output-json-file=",
        "--output-text-file=",
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains('\r'), "no carriage-return redraws: {stderr:?}");
    let progress: Vec<&str> = stderr.lines().filter(|line| line.starts_with("Progress: ")).collect();
    assert_eq!(progress.len(), 1, "{stderr}");
    assert!(progress[0].starts_with("Progress: 4/4 (100%) | "), "{}", progress[0]);

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout stays pure JSON");
    assert_eq!(json["results"].as_array().map(Vec::len), Some(4));
}

/// #21: credentials stay within the crawled site also on a multi-label public suffix: another
/// `*.co.uk` site is a different site, although both share the "2nd-level domain" `co.uk`.
#[test]
fn custom_headers_do_not_reach_other_sites_on_a_shared_public_suffix() {
    let other_site = RecordingServer::start(vec![Route {
        path: "/logo.png",
        headers: vec![("Content-Type", "image/png".to_string())],
        body: b"\x89PNG\r\n\x1a\n".to_vec(),
    }]);
    let site = RecordingServer::start(vec![Route {
        path: "/",
        headers: vec![("Content-Type", "text/html; charset=utf-8".to_string())],
        body: format!(
            "<html><head><title>Home</title></head><body><img src=\"http://www.other.co.uk:{}/logo.png\" alt=\"Logo\"></body></html>",
            other_site.port()
        )
        .into_bytes(),
    }]);

    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url=http://www.example.co.uk:{}/", site.port()),
        "--single-page",
        "--header=Cookie: session=SECRET_SUFFIX",
        // Both hosts are local servers; nothing is resolved through DNS.
        &format!("--resolve=www.example.co.uk:{}:127.0.0.1", site.port()),
        &format!("--resolve=www.other.co.uk:{}:127.0.0.1", other_site.port()),
        "--allowed-domain-for-external-files=www.other.co.uk",
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--output-html-report=",
        "--output-json-file=",
        "--output-text-file=",
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let site_requests = site.requests();
    assert!(
        site_requests
            .iter()
            .any(|head| head.to_ascii_lowercase().contains("cookie: session=secret_suffix")),
        "the crawled site gets the header: {site_requests:?}"
    );
    let other_requests = other_site.requests();
    assert!(!other_requests.is_empty(), "the image on the other site was fetched");
    for head in &other_requests {
        assert!(!head.to_ascii_lowercase().contains("cookie:"), "{head}");
    }
}

/// #21: on an IP-literal host the port tells services apart, so credentials go only to the
/// crawled port, not to another service on the same address.
#[test]
fn credentials_do_not_reach_another_port_of_an_ip_host() {
    let other_service = RecordingServer::start(vec![Route {
        path: "/logo.png",
        headers: vec![("Content-Type", "image/png".to_string())],
        body: b"\x89PNG\r\n\x1a\n".to_vec(),
    }]);
    let site = RecordingServer::start(vec![Route {
        path: "/",
        headers: vec![("Content-Type", "text/html; charset=utf-8".to_string())],
        body: format!(
            "<html><head><title>Home</title></head><body><img src=\"http://127.0.0.1:{}/logo.png\" alt=\"Logo\"></body></html>",
            other_service.port()
        )
        .into_bytes(),
    }]);

    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", site.url()),
        "--single-page",
        "--header=Cookie: session=SECRET_SUFFIX",
        "--http-auth=user:pass",
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--output-html-report=",
        "--output-json-file=",
        "--output-text-file=",
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let site_requests = site.requests();
    assert!(
        site_requests
            .iter()
            .any(|head| head.to_ascii_lowercase().contains("cookie: session=secret_suffix")),
        "the crawled port gets the header: {site_requests:?}"
    );
    let other_requests = other_service.requests();
    assert!(!other_requests.is_empty(), "the image on the other port was fetched");
    for head in &other_requests {
        let head = head.to_ascii_lowercase();
        assert!(!head.contains("cookie:"), "{head}");
        assert!(!head.contains("authorization:"), "{head}");
    }
}

// ---------------------------------------------------------------------------
// Sitemaps (#108, #106)
// ---------------------------------------------------------------------------

/// `--sitemap-changefreq` accepts only the values of the sitemaps.org protocol (#108).
#[test]
fn sitemap_changefreq_rejects_unknown_values() {
    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        "--url=https://example.com/",
        "--sitemap-changefreq=sometimes",
    ]);
    assert_eq!(output.status.code(), Some(101));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Invalid --sitemap-changefreq 'sometimes'"),
        "stderr: {stderr}"
    );
}

/// Crawls `url` on a local server with JSON output and returns the visited URLs.
fn crawled_urls(url: &str) -> Vec<String> {
    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={url}"),
        "--output=json",
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--output-html-report=",
        "--output-json-file=",
        "--output-text-file=",
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON on stdout");
    json["results"]
        .as_array()
        .expect("results")
        .iter()
        .filter_map(|result| result["url"].as_str().map(str::to_string))
        .collect()
}

/// Gzip-compresses `data` like a `.gz` sitemap file.
fn gzip_bytes(data: &str) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data.as_bytes()).expect("gzip");
    encoder.finish().expect("gzip")
}

/// A gzipped sitemap is read even when its URL does not end in `.xml.gz` (#106).
#[test]
fn gzipped_sitemap_at_plain_gz_url_is_crawled() {
    let tmp = TempDir::new("sitemap-gz-input");
    let site = tmp.path.join("site");
    write_site(&site, 2);
    let server = LocalServer::start(&site);
    let base = server.url();
    std::fs::write(
        site.join("sitemap.gz"),
        gzip_bytes(&format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>{base}page-1.html</loc></url><url><loc>{base}page-2.html</loc></url></urlset>"#
        )),
    )
    .expect("sitemap.gz");

    let urls = crawled_urls(&format!("{base}sitemap.gz"));

    assert!(urls.contains(&format!("{base}page-1.html")), "{urls:?}");
    assert!(urls.contains(&format!("{base}page-2.html")), "{urls:?}");
}

/// A sitemap index entry with a query string (Shopify's `?from=…&to=…`) is followed with the
/// whole query (#106).
#[test]
fn sitemap_index_entry_with_query_string_is_crawled() {
    let tmp = TempDir::new("sitemap-index-query");
    let site = tmp.path.join("site");
    write_site(&site, 2);
    let server = LocalServer::start(&site);
    let base = server.url();
    std::fs::write(
        site.join("sitemap_index.xml"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><sitemap><loc>{base}sitemap_products_1.xml?from=1&amp;to=2</loc></sitemap></sitemapindex>"#
        ),
    )
    .expect("sitemap_index.xml");
    std::fs::write(
        site.join("sitemap_products_1.xml"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>{base}page-1.html</loc></url><url><loc>{base}page-2.html</loc></url></urlset>"#
        ),
    )
    .expect("sitemap_products_1.xml");

    let urls = crawled_urls(&format!("{base}sitemap_index.xml"));

    assert!(
        urls.contains(&format!("{base}sitemap_products_1.xml?from=1&to=2")),
        "{urls:?}"
    );
    assert!(urls.contains(&format!("{base}page-1.html")), "{urls:?}");
    assert!(urls.contains(&format!("{base}page-2.html")), "{urls:?}");
}

/// A sitemap index entry at a plain `.gz` path is followed (#106).
#[test]
fn sitemap_index_entry_at_gz_path_is_crawled() {
    let tmp = TempDir::new("sitemap-index-gz");
    let site = tmp.path.join("site");
    write_site(&site, 2);
    let server = LocalServer::start(&site);
    let base = server.url();
    std::fs::write(
        site.join("sitemap_index.xml"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><sitemap><loc>{base}sitemap-1.gz</loc></sitemap></sitemapindex>"#
        ),
    )
    .expect("sitemap_index.xml");
    std::fs::write(
        site.join("sitemap-1.gz"),
        gzip_bytes(&format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>{base}page-1.html</loc></url><url><loc>{base}page-2.html</loc></url></urlset>"#
        )),
    )
    .expect("sitemap-1.gz");

    let urls = crawled_urls(&format!("{base}sitemap_index.xml"));

    assert!(urls.contains(&format!("{base}sitemap-1.gz")), "{urls:?}");
    assert!(urls.contains(&format!("{base}page-1.html")), "{urls:?}");
    assert!(urls.contains(&format!("{base}page-2.html")), "{urls:?}");
}

/// `--sitemap-xml-file` ending in `.xml.gz` writes gzip-compressed XML to exactly that path
/// (it used to write plain XML to `….xml.gz.xml`) (#106), with `--sitemap-changefreq` applied to
/// every URL (#108).
#[test]
fn sitemap_xml_gz_export_is_gzip_compressed() {
    use std::io::Read;

    let tmp = TempDir::new("sitemap-gz-export");
    let site = tmp.path.join("site");
    write_site(&site, 2);
    let server = LocalServer::start(&site);
    let sitemap = tmp.path.join("out").join("sitemap.xml.gz");

    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--output-html-report=",
        "--output-json-file=",
        "--output-text-file=",
        &format!("--sitemap-xml-file={}", sitemap.display()),
        "--sitemap-changefreq=weekly",
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(!tmp.path.join("out").join("sitemap.xml.gz.xml").exists());
    let bytes = std::fs::read(&sitemap).expect("the sitemap is written to the given path");
    assert!(bytes.starts_with(&[0x1f, 0x8b]), "gzip magic bytes");
    let mut xml = String::new();
    flate2::read::GzDecoder::new(&bytes[..])
        .read_to_string(&mut xml)
        .expect("valid gzip");
    assert!(
        xml.contains(r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#),
        "{xml}"
    );
    assert_eq!(xml.matches("<loc>").count(), 3, "index + 2 pages: {xml}");
    assert_eq!(
        xml.matches("<changefreq>weekly</changefreq>").count(),
        3,
        "one per URL: {xml}"
    );
}

// ---------------------------------------------------------------------------
// Browser rendering (#62, #46) — needs a Chromium-family browser; run with `-- --ignored`
// ---------------------------------------------------------------------------

/// Renders the single page at `server` with `--browser` (plus `extra`) and returns the HTML the
/// offline export captured; `name` names the export directory inside `tmp`.
#[cfg(feature = "browser")]
fn render_offline(tmp: &TempDir, server: &LocalServer, name: &str, extra: &[&str]) -> String {
    let offline = tmp.path.join(name);
    let offline_arg = format!("--offline-export-dir={}", offline.display());
    let url_arg = format!("--url={}", server.url());
    let mut args = vec![
        "--config-file=/dev/null",
        url_arg.as_str(),
        "--single-page",
        "--browser",
        "--browser-no-sandbox",
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--output-html-report=",
        "--output-json-file=",
        "--output-text-file=",
        offline_arg.as_str(),
    ];
    args.extend_from_slice(extra);
    let output = run_built_crawler(&args);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read_to_string(offline.join("index.html")).expect("rendered index.html")
}

/// A page too tall to scroll through within the cap. Every change of its `<head>` (the settle
/// style the crawler injects and removes) records the scroll position and then blocks the main
/// thread for 1.5 s, so the steps after scrolling are slow.
#[cfg(feature = "browser")]
const SLOW_SETTLE_PAGE: &str = r#"<!doctype html>
<html><head><title>Slow settle</title></head>
<body data-scroll-y="initial">
<div style="height:60000px">tall</div>
<script>
new MutationObserver(function () {
  document.body.setAttribute('data-scroll-y', String(Math.round(window.scrollY)));
  var until = Date.now() + 1500;
  while (Date.now() < until) {}
}).observe(document.head, { childList: true });
</script>
</body></html>"#;

/// Even when the scrolling uses up the render budget, auto-scroll returns to the top and removes
/// its settle style before the HTML is captured (#62).
#[cfg(feature = "browser")]
#[test]
#[ignore]
fn browser_auto_scroll_finishes_when_the_budget_runs_out() {
    let tmp = TempDir::new("auto-scroll-budget");
    let site = tmp.path.join("site");
    std::fs::create_dir_all(&site).expect("site dir");
    std::fs::write(site.join("index.html"), SLOW_SETTLE_PAGE).expect("index.html");
    let server = LocalServer::start(&site);

    let html = render_offline(&tmp, &server, "offline", &["--browser-timeout=8"]);

    assert!(
        !html.contains("data-siteone-freeze"),
        "the settle style is not captured: {html}"
    );
    assert!(
        html.contains(r#"data-scroll-y="0""#),
        "the page is back at the top: {html}"
    );
}

/// A page that adds a paragraph only when its bottom is scrolled into view.
#[cfg(feature = "browser")]
const REVEAL_ON_SCROLL_PAGE: &str = r#"<!doctype html>
<html><head><title>Reveal on scroll</title></head>
<body>
<h1>Top</h1>
<div style="height:4000px">spacer</div>
<div id="sentinel">bottom</div>
<script>
new IntersectionObserver(function (entries, observer) {
  if (entries[0].isIntersecting) {
    var p = document.createElement('p');
    p.textContent = ['revealed', 'on', 'scroll'].join('-');
    document.body.appendChild(p);
    observer.disconnect();
  }
}).observe(document.getElementById('sentinel'));
</script>
</body></html>"#;

/// `--browser` scrolls each page before capturing it (on by default), so content revealed on
/// scroll is in the rendered HTML; `--browser-auto-scroll=0` captures the unscrolled page (#62).
#[cfg(feature = "browser")]
#[test]
#[ignore]
fn browser_auto_scroll_captures_content_revealed_on_scroll() {
    let tmp = TempDir::new("auto-scroll");
    let site = tmp.path.join("site");
    std::fs::create_dir_all(&site).expect("site dir");
    std::fs::write(site.join("index.html"), REVEAL_ON_SCROLL_PAGE).expect("index.html");
    let server = LocalServer::start(&site);

    let rendered = |name: &str, extra: &[&str]| render_offline(&tmp, &server, name, extra);

    let scrolled = rendered("scrolled", &[]);
    assert!(scrolled.contains("revealed-on-scroll"));
    assert!(
        !scrolled.contains("data-siteone-freeze"),
        "the settle style is not captured"
    );
    assert!(!rendered("not-scrolled", &["--browser-auto-scroll=0"]).contains("revealed-on-scroll"));
}

/// Unknown `--screenshot-viewport` entries are a configuration error (#46).
#[cfg(feature = "browser")]
#[test]
fn screenshot_viewport_rejects_unknown_entries() {
    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        "--url=https://example.com/",
        "--browser",
        "--screenshot-viewport=desktop,phone",
    ]);
    assert_eq!(output.status.code(), Some(101));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("'phone' is neither WxH nor one of desktop, tablet, mobile"),
        "stderr: {stderr}"
    );
}

/// Width and height of a PNG file, read from its IHDR chunk.
#[cfg(feature = "browser")]
fn png_size(path: &Path) -> (u32, u32) {
    let bytes = std::fs::read(path).expect("screenshot file");
    let read_u32 = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
    (read_u32(16), read_u32(20))
}

/// Every page is captured in each `--screenshot-viewport` size, with the size in the file name
/// and one table row per file (#46).
#[cfg(feature = "browser")]
#[test]
#[ignore]
fn screenshots_are_captured_in_every_viewport() {
    let tmp = TempDir::new("viewports");
    let site = tmp.path.join("site");
    write_site(&site, 0);
    let server = LocalServer::start(&site);
    let shots = tmp.path.join("shots");

    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        "--single-page",
        "--browser",
        "--browser-no-sandbox",
        "--screenshots",
        &format!("--screenshots-dir={}", shots.display()),
        "--screenshot-viewport=desktop,mobile",
        "--output=json",
        "--analyzer-filter-regex=/BrowserConsole/",
        "--http-cache-dir=",
        "--output-html-report=",
        "--output-json-file=",
        "--output-text-file=",
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let files: Vec<std::path::PathBuf> = std::fs::read_dir(&shots)
        .expect("screenshots dir")
        .map(|entry| entry.expect("dir entry").path())
        .collect();
    assert_eq!(files.len(), 2, "{files:?}");
    let desktop = files
        .iter()
        .find(|f| f.to_string_lossy().ends_with("_1920x1080.png"))
        .expect("desktop screenshot");
    let mobile = files
        .iter()
        .find(|f| f.to_string_lossy().ends_with("_390x844.png"))
        .expect("mobile screenshot");
    assert_eq!(png_size(desktop), (1920, 1080));
    assert_eq!(png_size(mobile), (390, 844));

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON on stdout");
    let rows = json["tables"]["browser-screenshots"]["rows"]
        .as_array()
        .expect("screenshot rows");
    assert_eq!(rows.len(), 2, "{rows:?}");
}

/// #35: with --force-relative-urls a link to the https variant of the initial URL is fetched on the
/// initial scheme *and port* (it used to be requested as http://host:443/…), so the page is exported.
#[test]
fn force_relative_urls_fetches_scheme_variants_from_the_initial_port() {
    let tmp = TempDir::new("force-relative");
    let site = tmp.path.join("site");
    std::fs::create_dir_all(&site).expect("site dir");
    std::fs::write(
        site.join("index.html"),
        r#"<html><head><title>Home</title></head><body><a href="https://127.0.0.1/page4.html">Page 4</a></body></html>"#,
    )
    .expect("index.html");
    std::fs::write(
        site.join("page4.html"),
        r#"<html><head><title>Page 4</title></head><body><a href="/">Home</a></body></html>"#,
    )
    .expect("page4.html");
    let server = LocalServer::start(&site);
    let export = tmp.path.join("export");

    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--force-relative-urls",
        &format!("--offline-export-dir={}", export.display()),
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        export.join("page4.html").is_file(),
        "https://127.0.0.1/page4.html is fetched from the local server and exported"
    );
    let index = std::fs::read_to_string(export.join("index.html")).expect("index.html");
    assert!(index.contains(r#"href="page4.html""#), "{index}");
}

/// All files below `dir`, recursively.
fn exported_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).expect("export dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(exported_files(&path));
        } else {
            files.push(path);
        }
    }
    files
}

/// Local links, asset references and meta-refresh targets of the exported HTML, CSS and Markdown
/// files that do not resolve to an exported file, and meta refreshes that reload their own page
/// (fragments and query strings ignored, absolute URLs skipped).
fn dangling_references(export: &Path) -> Vec<String> {
    let html_reference = regex::Regex::new(r#"\s(?:href|src|srcset)="([^"]*)""#).unwrap();
    let meta_refresh_reference = regex::Regex::new(r#"(?i)<meta[^>]*\burl=([^"'>\s]+)"#).unwrap();
    let css_reference = regex::Regex::new(r#"url\(['"]?([^'")]+)['"]?\)"#).unwrap();
    let markdown_reference = regex::Regex::new(r"\]\(([^)\s]+)\)").unwrap();
    let mut dangling = Vec::new();
    for file in exported_files(export) {
        let patterns = match file.extension().and_then(|ext| ext.to_str()) {
            Some("html") => vec![&html_reference, &meta_refresh_reference],
            Some("css") => vec![&css_reference],
            Some("md") => vec![&markdown_reference],
            _ => continue,
        };
        let text = std::fs::read_to_string(&file).expect("exported text file");
        let directory = file.parent().expect("parent dir");
        for pattern in patterns {
            for caps in pattern.captures_iter(&text) {
                for candidate in caps[1].split(", ") {
                    let reference = candidate.split_whitespace().next().unwrap_or("");
                    let target = reference.split(['#', '?']).next().unwrap_or("");
                    // fragment-only links, absolute URLs and other schemes (mailto:, data:, …)
                    if target.is_empty() || reference.contains(':') || reference.starts_with("//") {
                        continue;
                    }
                    let resolved = directory.join(target);
                    let problem = if !resolved.is_file() {
                        "missing"
                    } else if std::ptr::eq(pattern, &meta_refresh_reference)
                        && resolved.canonicalize().ok() == file.canonicalize().ok()
                    {
                        "reloads itself"
                    } else {
                        continue;
                    };
                    dangling.push(format!(
                        "{} -> {} ({problem})",
                        file.strip_prefix(export).unwrap().display(),
                        reference
                    ));
                }
            }
        }
    }
    dangling
}

/// #35: when the initial URL redirects to its www twin, the pages are stored under
/// `_www.site.test/` and the root holds the redirect records. With --force-relative-urls every
/// record leads to the twin's copy (never to itself) and the pages keep their links to that copy.
#[test]
fn force_relative_urls_exports_a_site_redirecting_to_its_www_twin() {
    let tmp = TempDir::new("force-relative-redirect");
    let site = tmp.path.join("site");
    std::fs::create_dir_all(site.join("img")).expect("site dir");
    let server = RedirectServer::start(
        &site,
        vec![Redirect {
            host: Some("site.test"),
            path: None,
            location: "http://www.site.test:{port}{path}",
        }],
    );
    let port = server.port();
    std::fs::write(
        site.join("index.html"),
        format!(
            r#"<html><head><title>Home</title><link rel="stylesheet" href="/style.css"></head><body>
<a href="/about">About</a> <a href="http://site.test:{port}/contact">Contact</a> <img src="/img/a.png" alt="a">
</body></html>"#
        ),
    )
    .expect("index.html");
    std::fs::write(
        site.join("about.html"),
        r#"<html><head><title>About</title></head><body><a href="/">Home</a></body></html>"#,
    )
    .expect("about.html");
    std::fs::write(
        site.join("contact.html"),
        r#"<html><head><title>Contact</title></head><body><a href="/about">About</a></body></html>"#,
    )
    .expect("contact.html");
    std::fs::write(site.join("style.css"), "body{background:url(/img/a.png)}").expect("css");
    std::fs::write(site.join("img/a.png"), "PNG").expect("png");
    let export = tmp.path.join("export");

    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url=http://site.test:{port}/"),
        &format!("--resolve=site.test:{port}:127.0.0.1"),
        &format!("--resolve=www.site.test:{port}:127.0.0.1"),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--force-relative-urls",
        &format!("--offline-export-dir={}", export.display()),
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for file in ["index.html", "about.html", "contact.html", "style.css", "img/a.png"] {
        let copy = format!("_www.site.test/{file}");
        assert!(export.join(&copy).is_file(), "{copy} is exported");
    }
    let record = std::fs::read_to_string(export.join("index.html")).expect("index.html");
    assert!(record.contains("url=_www.site.test/index.html"), "{record}");
    let page = std::fs::read_to_string(export.join("_www.site.test/index.html")).expect("the www copy");
    for expected in [r#"href="style.css""#, r#"href="about.html""#, r#"src="img/a.png""#] {
        assert!(page.contains(expected), "missing {expected} in {page}");
    }
    assert_eq!(dangling_references(&export), Vec::<String>::new());
}

/// #35: with --force-relative-urls, links to the http/https and www/non-www variants of the initial
/// URL are fetched from the initial origin and lead to the same local files. `--resolve` points the
/// test host to the local server, so no DNS is needed.
#[test]
fn force_relative_urls_exports_www_and_scheme_variants_as_the_same_files() {
    let tmp = TempDir::new("force-relative-www");
    let site = tmp.path.join("site");
    std::fs::create_dir_all(site.join("img")).expect("site dir");
    std::fs::write(
        site.join("index.html"),
        r#"<html><head><title>Home</title><link rel="stylesheet" href="https://www.site.test/style.css"></head><body>
<a href="http://www.site.test/page1">1</a> <a href="https://site.test/page2">2</a>
<a href="https://www.site.test/page3">3</a> <img src="//www.site.test/img/a.png" alt="a">
<a href="https://www.site.test/">Home</a>
</body></html>"#,
    )
    .expect("index.html");
    for page in ["page1", "page2", "page3"] {
        std::fs::write(
            site.join(format!("{page}.html")),
            format!(r#"<html><head><title>{page}</title></head><body><a href="https://www.site.test/">Home</a></body></html>"#),
        )
        .expect("page");
    }
    std::fs::write(
        site.join("style.css"),
        "body{background:url(https://www.site.test/img/a.png)}",
    )
    .expect("css");
    std::fs::write(site.join("img/a.png"), "PNG").expect("png");
    let server = LocalServer::start(&site);
    let port = server
        .url()
        .trim_end_matches('/')
        .rsplit(':')
        .next()
        .unwrap()
        .to_string();
    let export = tmp.path.join("export");

    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url=http://site.test:{port}/"),
        &format!("--resolve=site.test:{port}:127.0.0.1"),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--force-relative-urls",
        &format!("--offline-export-dir={}", export.display()),
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for file in [
        "index.html",
        "page1.html",
        "page2.html",
        "page3.html",
        "style.css",
        "img/a.png",
    ] {
        assert!(export.join(file).is_file(), "{file} is exported");
    }
    assert!(!export.join("_www.site.test").exists(), "no copy of the www variant");
    assert_eq!(dangling_references(&export), Vec::<String>::new());
    let index = std::fs::read_to_string(export.join("index.html")).expect("index.html");
    for expected in [
        r#"href="style.css""#,
        r#"href="page1.html""#,
        r#"href="page2.html""#,
        r#"href="page3.html""#,
        r#"src="img/a.png""#,
    ] {
        assert!(index.contains(expected), "missing {expected} in {index}");
    }
    let page1 = std::fs::read_to_string(export.join("page1.html")).expect("page1.html");
    assert!(page1.contains(r#"href="index.html""#), "{page1}");
    let css = std::fs::read_to_string(export.join("style.css")).expect("style.css");
    assert!(css.contains("url(img/a.png)"), "{css}");
}

/// Writes a site whose pages live at extension-less URLs on several directory levels (#55).
fn write_nested_site(dir: &Path) {
    let files = [
        (
            "index.html",
            r#"<html><head><title>Home</title><link rel="stylesheet" href="/style.css"></head><body>
<a href="/about">About</a> <a href="/docs/">Docs</a> <a href="/docs/guide">Guide</a>
<a href="/contact.html">Contact</a> <a href="/news?page=2">News</a> <img src="/img/logo.png" alt="logo">
<a href="/moved">Moved</a>
</body></html>"#,
        ),
        (
            "moved.html",
            r#"<html><head><meta http-equiv="refresh" content="0; url=/about"><title>Moved</title></head><body></body></html>"#,
        ),
        (
            "about.html",
            r#"<html><head><title>About</title><link rel="stylesheet" href="/style.css"></head><body>
<a href="team">Team</a> <a href="/docs/guide#usage">Guide</a> <img src="img/photo.png" alt="photo">
</body></html>"#,
        ),
        (
            "team.html",
            r#"<html><head><title>Team</title></head><body><a href="/">Home</a></body></html>"#,
        ),
        (
            "docs/index.html",
            r#"<html><head><title>Docs</title></head><body><a href="guide">Guide</a> <a href="../about">About</a></body></html>"#,
        ),
        (
            "docs/guide.html",
            r#"<html><head><title>Guide</title><link rel="stylesheet" href="../style.css"></head><body>
<a href="intro">Intro</a> <a href="./">Docs</a> <img src="/img/logo.png" alt="logo">
</body></html>"#,
        ),
        (
            "docs/intro.html",
            r#"<html><head><title>Intro</title></head><body><a href="/">Home</a></body></html>"#,
        ),
        (
            "contact.html",
            r#"<html><head><title>Contact</title></head><body><a href="/about">About</a></body></html>"#,
        ),
        (
            "news.html",
            r#"<html><head><title>News</title></head><body><a href="/news?page=3">Next</a> <a href="/">Home</a></body></html>"#,
        ),
        ("style.css", "body{background:url(/img/bg.png)}"),
        ("img/logo.png", "PNG"),
        ("img/photo.png", "PNG"),
        ("img/bg.png", "PNG"),
    ];
    for (path, content) in files {
        let file = dir.join(path);
        std::fs::create_dir_all(file.parent().expect("parent dir")).expect("site dir");
        std::fs::write(file, content).expect("site file");
    }
}

/// #55: with --offline-export-preserve-url-structure every link and asset reference leads to a file
/// that was actually written (about/index.html), with relative paths counted from where the page is
/// stored — with and without the redirect stubs, and in the markdown export too.
#[test]
fn preserve_url_structure_links_resolve_to_exported_files() {
    let tmp = TempDir::new("preserve-structure");
    let site = tmp.path.join("site");
    write_nested_site(&site);
    let server = LocalServer::start(&site);

    for (name, no_redirect_stubs) in [("with-stubs", false), ("no-stubs", true)] {
        let export = tmp.path.join(format!("offline-{name}"));
        let markdown = tmp.path.join(format!("markdown-{name}"));
        let mut owned_args = vec![
            "--config-file=/dev/null".to_string(),
            format!("--url={}", server.url()),
            LOCAL_ANALYZERS.to_string(),
            "--http-cache-dir=".to_string(),
            "--offline-export-preserve-url-structure".to_string(),
            format!("--offline-export-dir={}", export.display()),
            format!("--markdown-export-dir={}", markdown.display()),
        ];
        if no_redirect_stubs {
            owned_args.push("--offline-export-no-auto-redirect-html".to_string());
        }
        let args: Vec<&str> = owned_args.iter().map(String::as_str).collect();
        let output = run_built_crawler(&args);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{name}: stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(
            export.join("docs/guide/index.html").is_file(),
            "{name}: /docs/guide is stored in its own directory"
        );
        let index = std::fs::read_to_string(export.join("index.html")).expect("index.html");
        assert!(
            index.contains(r#"href="about/index.html""#),
            "{name}: links point to the stored file, not to a redirect stub: {index}"
        );
        assert_eq!(
            dangling_references(&export),
            Vec::<String>::new(),
            "{name}: offline export"
        );
        assert_eq!(
            dangling_references(&markdown),
            Vec::<String>::new(),
            "{name}: markdown export"
        );
    }
}

/// #55: with --offline-export-preserve-url-structure, /docs (a 301) and /docs/ are both stored as
/// docs/index.html; the redirect record must not replace the page, whichever is crawled first and
/// wherever it redirects to.
#[test]
fn preserve_url_structure_keeps_a_directory_index_over_its_redirect() {
    let tmp = TempDir::new("preserve-redirect");
    for (name, home_link, docs_link, location) in [
        ("page-first", "/docs/", "/docs", "/docs/"),
        ("redirect-first", "/docs", "/", "/docs/"),
        ("redirect-elsewhere", "/docs/", "/docs", "/manual/"),
    ] {
        let site = tmp.path.join(format!("site-{name}"));
        std::fs::create_dir_all(site.join("docs")).expect("site dir");
        std::fs::create_dir_all(site.join("manual")).expect("site dir");
        std::fs::write(
            site.join("manual/index.html"),
            r#"<html><head><title>Manual</title></head><body><a href="/">Home</a></body></html>"#,
        )
        .expect("manual/index.html");
        std::fs::write(
            site.join("index.html"),
            format!(r#"<html><head><title>Home</title></head><body><a href="{home_link}">Docs</a></body></html>"#),
        )
        .expect("index.html");
        std::fs::write(
            site.join("docs/index.html"),
            format!(
                r#"<html><head><title>Docs</title></head><body><p>The docs index</p><a href="{docs_link}">Link</a></body></html>"#
            ),
        )
        .expect("docs/index.html");
        let server = RedirectServer::start(
            &site,
            vec![Redirect {
                host: None,
                path: Some("/docs"),
                location,
            }],
        );
        let export = tmp.path.join(format!("export-{name}"));

        let output = run_built_crawler(&[
            "--config-file=/dev/null",
            &format!("--url=http://127.0.0.1:{}/", server.port()),
            LOCAL_ANALYZERS,
            "--http-cache-dir=",
            "--offline-export-preserve-url-structure",
            &format!("--offline-export-dir={}", export.display()),
        ]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{name}: stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let docs = std::fs::read_to_string(export.join("docs/index.html")).expect("docs/index.html");
        assert!(docs.contains("The docs index"), "{name}: {docs}");
        assert_eq!(dangling_references(&export), Vec::<String>::new(), "{name}");
    }

    // /docs/ is missing (404): the record of /docs would be stored as docs/index.html and reload itself
    let site = tmp.path.join("site-missing");
    std::fs::create_dir_all(&site).expect("site dir");
    std::fs::write(
        site.join("index.html"),
        r#"<html><head><title>Home</title></head><body><a href="/docs">Docs</a></body></html>"#,
    )
    .expect("index.html");
    let server = RedirectServer::start(
        &site,
        vec![Redirect {
            host: None,
            path: Some("/docs"),
            location: "/docs/",
        }],
    );
    let export = tmp.path.join("export-missing");
    let output = run_built_crawler(&[
        "--config-file=/dev/null",
        &format!("--url=http://127.0.0.1:{}/", server.port()),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--offline-export-preserve-url-structure",
        &format!("--offline-export-dir={}", export.display()),
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(export.join("index.html").is_file(), "the home page is exported");
    assert!(
        !export.join("docs/index.html").exists(),
        "no redirect record that reloads itself"
    );
}
