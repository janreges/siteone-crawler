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
    LocalServer, MockLlm, MockResponse, RecordingServer, Redirect, RedirectServer, Route, TempDir, run_crawler,
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
    // Should contain relative link to getting-started/quick-start-guide (the homepage no longer
    // links to introduction/overview)
    assert!(
        index_html.contains("getting-started/quick-start-guide/index.html"),
        "index.html should contain relative link to getting-started/quick-start-guide/index.html"
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

    let output = run_crawler(&[
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

#[test]
fn events_file_never_carries_the_credentials_of_the_url() {
    let tmp = TempDir::new("events-url-credentials");
    let site = tmp.path.join("site");
    write_site(&site, 0);
    let server = LocalServer::start(&site);
    let mut leaks = Vec::new();
    for (name, userinfo) in [
        ("plain", "user:PW_SENTINEL_0006"),
        ("undecodable", "%FFuser:PW_SENTINEL_0006"),
    ] {
        let events = tmp.path.join(format!("{name}.ndjson"));
        let url = server.url().replacen("http://", &format!("http://{userinfo}@"), 1);
        run_crawler(&[
            "--config-file=/dev/null",
            &format!("--url={url}"),
            "--single-page",
            LOCAL_ANALYZERS,
            "--http-cache-dir=",
            &format!("--events-file={}", events.display()),
        ]);
        let text = std::fs::read_to_string(&events).expect("the event file exists");
        let first: serde_json::Value =
            serde_json::from_str(text.lines().next().unwrap_or_default()).expect("a JSON event");
        assert_eq!(first["type"], "runStarted", "{name}");
        assert_eq!(first["url"], server.url(), "{name}: the URL without its userinfo");
        leaks.extend(
            text.lines()
                .filter(|line| line.contains("PW_SENTINEL_0006"))
                .map(|line| format!("{name}: {line}")),
        );
    }
    assert!(leaks.is_empty(), "{}", leaks.join("\n"));
}

/// Without `--events-file` nothing changes: the stream is entirely opt-in.
#[test]
fn no_events_file_means_no_events() {
    let tmp = TempDir::new("no-events");
    let site = tmp.path.join("site");
    write_site(&site, 0);
    let server = LocalServer::start(&site);
    let report = tmp.path.join("report.html");

    let output = run_crawler(&[
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
        let output = run_crawler(&[
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
        let output = run_crawler(&[
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

    let output = run_crawler(&[
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

    let output = run_crawler(&[
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

    let output = run_crawler(&[
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

    let output = run_crawler(&[
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

    let output = run_crawler(&[
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

/// #104: in JSON mode (`--ci --output=json`) failed URLs are named on stderr as they finish, while
/// successful ones are not and stdout stays one parseable JSON document.
#[test]
fn json_progress_names_failed_urls_on_stderr() {
    let html = |body: &str| format!("<html><head><title>T</title></head><body>{body}</body></html>");
    let server = RecordingServer::start(vec![
        Route {
            path: "/",
            headers: vec![("Content-Type", "text/html; charset=utf-8".to_string())],
            body: html(r#"<a href="/ok.html">OK</a> <a href="/boom">Boom</a>"#).into_bytes(),
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
        "--ci",
        "--output=json",
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--output-html-report=",
        "--output-json-file=",
        "--output-text-file=",
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let failed: Vec<&str> = stderr.lines().filter(|line| line.starts_with("Failed: ")).collect();
    assert_eq!(failed.len(), 1, "{stderr}");
    assert!(failed[0].contains("500") && failed[0].contains("/boom"), "{stderr}");
    assert!(!stderr.contains("/ok.html"), "successful URLs are not listed: {stderr}");
    assert!(!stderr.contains('\r'), "{stderr}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is one JSON document");
    assert_eq!(json["results"].as_array().map(Vec::len), Some(3));
}

/// #104: under GitHub Actions the CI gate's `::error` annotations go to stderr in JSON mode, so stdout
/// stays one parseable JSON document (the runner reads workflow commands from both streams).
#[test]
fn github_annotations_keep_json_stdout_parseable() {
    let html = |body: &str| format!("<html><head><title>T</title></head><body>{body}</body></html>");
    let server = RecordingServer::start(vec![Route {
        path: "/",
        headers: vec![("Content-Type", "text/html; charset=utf-8".to_string())],
        body: html("<p>One page</p>").into_bytes(),
    }]);

    let output = std::process::Command::new(common::binary_path())
        .args([
            "--config-file=/dev/null",
            &format!("--url={}", server.url()),
            "--ci",
            "--ci-min-pages=100",
            "--output=json",
            LOCAL_ANALYZERS,
            "--http-cache-dir=",
            "--output-html-report=",
            "--output-json-file=",
            "--output-text-file=",
        ])
        .env("GITHUB_ACTIONS", "true")
        .output()
        .expect("Failed to execute crawler binary");
    assert_eq!(output.status.code(), Some(10), "the gate fails on --ci-min-pages");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.lines().any(|line| line.starts_with("::error")), "{stderr}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is one JSON document");
    assert!(json["results"].is_array());
}

/// #104: `--hide-progress-bar` also hides the `--progress-interval` lines in JSON mode.
#[test]
fn hide_progress_bar_suppresses_progress_lines_in_json_mode() {
    let tmp = TempDir::new("progress-interval-hidden");
    let site = tmp.path.join("site");
    write_site(&site, 1);
    let server = LocalServer::start(&site);

    let output = run_crawler(&[
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

    let output = run_crawler(&[
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

    let output = run_crawler(&[
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

    let output = run_crawler(&[
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
    let output = run_crawler(&[
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

/// `<lastmod>` reads `Last-Modified` and `Date` in each HTTP date format (IMF-fixdate, RFC 850,
/// asctime), with the response headers kept in memory or in file storage (#108).
#[test]
fn sitemap_lastmod_reads_every_http_date_format() {
    let page = |path: &'static str, last_modified: &str, date: &str| Route {
        path,
        headers: vec![
            ("Content-Type", "text/html".to_string()),
            ("Last-Modified", last_modified.to_string()),
            ("Date", date.to_string()),
        ],
        body: b"<html><body>page</body></html>".to_vec(),
    };
    let server = RecordingServer::start(vec![
        Route {
            path: "/",
            headers: vec![("Content-Type", "text/html".to_string())],
            body: br#"<html><body><a href="/rfc850">1</a><a href="/asctime">2</a><a href="/now-rfc850">3</a><a href="/now-asctime">4</a></body></html>"#.to_vec(),
        },
        page("/rfc850", "Friday, 17-Jul-26 17:29:24 GMT", "Mon, 20 Jul 2026 12:00:00 GMT"),
        page("/asctime", "Fri Jul 17 17:29:24 2026", "Mon, 20 Jul 2026 12:00:00 GMT"),
        // Dynamic pages stamping the response time, with `Date` in an obsolete format.
        page("/now-rfc850", "Mon, 20 Jul 2026 12:00:00 GMT", "Monday, 20-Jul-26 12:00:00 GMT"),
        page("/now-asctime", "Mon, 20 Jul 2026 12:00:00 GMT", "Mon Jul 20 12:00:00 2026"),
    ]);

    for storage in ["memory", "file"] {
        let tmp = TempDir::new(&format!("sitemap-lastmod-{storage}"));
        let sitemap = tmp.path.join("sitemap.xml");
        let output = run_crawler(&[
            "--config-file=/dev/null",
            &format!("--url={}", server.url()),
            "--output=json",
            LOCAL_ANALYZERS,
            "--http-cache-dir=",
            "--output-html-report=",
            "--output-json-file=",
            "--output-text-file=",
            &format!("--sitemap-xml-file={}", sitemap.display()),
            &format!("--result-storage={storage}"),
            &format!("--result-storage-dir={}", tmp.path.join("storage").display()),
        ]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let xml = std::fs::read_to_string(&sitemap).unwrap_or_else(|e| {
            panic!(
                "{storage}: no sitemap ({e}); stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
        let lastmod = |path: &str| -> Option<String> {
            let loc = format!("<loc>{}{path}</loc>", server.url().trim_end_matches('/'));
            let entry = xml.split("<url>").find(|entry| entry.contains(&loc))?;
            let start = entry.find("<lastmod>")? + "<lastmod>".len();
            Some(entry[start..entry.find("</lastmod>")?].to_string())
        };
        for path in ["/rfc850", "/asctime"] {
            assert_eq!(
                lastmod(path).as_deref(),
                Some("2026-07-17T17:29:24+00:00"),
                "{storage} {path}: {xml}"
            );
        }
        for path in ["/now-rfc850", "/now-asctime"] {
            assert!(xml.contains(&format!("{path}</loc>")), "{storage} {path}: {xml}");
            assert_eq!(lastmod(path), None, "{storage} {path}: {xml}");
        }
    }
}

/// Crawls `url` on a local server with JSON output and returns the visited URLs.
fn crawled_urls(url: &str) -> Vec<String> {
    let output = run_crawler(&[
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

/// A `.xml.gz` sitemap is read however long its prolog is — here a comment over 64 KB before
/// `<urlset>` (#106).
#[test]
fn gzipped_sitemap_with_long_prolog_is_crawled() {
    let tmp = TempDir::new("sitemap-gz-long-prolog");
    let site = tmp.path.join("site");
    write_site(&site, 1);
    let server = LocalServer::start(&site);
    let base = server.url();
    let comment = "a".repeat(65_536);
    std::fs::write(
        site.join("sitemap.xml.gz"),
        gzip_bytes(&format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><!--{comment}--><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>{base}page-1.html</loc></url></urlset>"#
        )),
    )
    .expect("sitemap.xml.gz");

    let urls = crawled_urls(&format!("{base}sitemap.xml.gz"));

    assert!(urls.contains(&format!("{base}page-1.html")), "{urls:?}");
}

/// A `.gz` download holding XML with another root element is not a sitemap, even when it mentions
/// `<urlset>` in a comment: its `<loc>` values are not crawled and the offline export keeps its
/// gzip bytes (#106).
#[test]
fn gzipped_xml_that_is_not_a_sitemap_keeps_its_bytes() {
    let tmp = TempDir::new("gz-download-not-sitemap");
    let site = tmp.path.join("site");
    std::fs::create_dir_all(&site).expect("site dir");
    let server = LocalServer::start(&site);
    let base = server.url();
    std::fs::write(
        site.join("index.html"),
        r#"<html><head><title>Home</title></head><body><a href="/catalog.gz">Catalog</a></body></html>"#,
    )
    .expect("index.html");
    std::fs::write(
        site.join("page.html"),
        "<html><head><title>Page</title></head><body></body></html>",
    )
    .expect("page.html");
    let catalog = gzip_bytes(&format!(
        r#"<?xml version="1.0"?><catalog><!-- <urlset> --><item><loc>{base}page.html</loc></item></catalog>"#
    ));
    std::fs::write(site.join("catalog.gz"), &catalog).expect("catalog.gz");
    let export = tmp.path.join("export");

    let output = run_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={base}"),
        "--output=json",
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--output-html-report=",
        "--output-json-file=",
        "--output-text-file=",
        &format!("--offline-export-dir={}", export.display()),
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON on stdout");
    let urls: Vec<&str> = json["results"]
        .as_array()
        .expect("results")
        .iter()
        .filter_map(|result| result["url"].as_str())
        .collect();

    assert!(urls.contains(&format!("{base}catalog.gz").as_str()), "{urls:?}");
    assert!(!urls.contains(&format!("{base}page.html").as_str()), "{urls:?}");
    assert_eq!(
        std::fs::read(export.join("catalog.gz")).expect("catalog.gz is exported"),
        catalog
    );
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

    let output = run_crawler(&[
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

/// Renders the single page at `url` with `--browser` (plus `extra`) and returns the HTML the
/// offline export captured; `name` names the export directory inside `tmp`.
#[cfg(feature = "browser")]
fn render_offline(tmp: &TempDir, url: &str, name: &str, extra: &[&str]) -> String {
    let offline = tmp.path.join(name);
    let offline_arg = format!("--offline-export-dir={}", offline.display());
    let url_arg = format!("--url={url}");
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
    let output = run_crawler(&args);
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

    let html = render_offline(&tmp, &server.url(), "offline", &["--browser-timeout=8"]);

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

    let rendered = |name: &str, extra: &[&str]| render_offline(&tmp, &server.url(), name, extra);

    let scrolled = rendered("scrolled", &[]);
    assert!(scrolled.contains("revealed-on-scroll"));
    assert!(
        !scrolled.contains("data-siteone-freeze"),
        "the settle style is not captured"
    );
    assert!(!rendered("not-scrolled", &["--browser-auto-scroll=0"]).contains("revealed-on-scroll"));
}

/// A page that fetches a section from `/section` when its bottom is scrolled into view.
#[cfg(feature = "browser")]
const FETCH_ON_SCROLL_PAGE: &str = r#"<!doctype html>
<html><head><title>Fetch on scroll</title></head>
<body>
<h1>Top</h1>
<div style="height:4000px">spacer</div>
<div id="sentinel">bottom</div>
<script>
new IntersectionObserver(function (entries, observer) {
  if (entries[0].isIntersecting) {
    observer.disconnect();
    fetch('/section').then(function (r) { return r.text(); }).then(function (html) {
      document.getElementById('sentinel').insertAdjacentHTML('beforeend', html);
    });
  }
}).observe(document.getElementById('sentinel'));
</script>
</body></html>"#;

/// A server on 127.0.0.1 with `FETCH_ON_SCROLL_PAGE` at `/` and its section at `/section`, which
/// is answered only after `delay` (on the page's origin: the built-in server's CSP blocks fetches
/// from other origins); each connection on its own thread. Stopped when dropped.
#[cfg(feature = "browser")]
struct SlowSectionServer {
    port: u16,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(feature = "browser")]
impl SlowSectionServer {
    fn start(delay: std::time::Duration) -> Self {
        use std::io::{Read, Write};
        use std::sync::atomic::Ordering;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
        let port = listener.local_addr().expect("a bound address").port();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopped = stop.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stopped.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut stream) = stream else { continue };
                std::thread::spawn(move || {
                    stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
                    let mut head = [0u8; 4096];
                    let read = stream.read(&mut head).unwrap_or(0);
                    let head = &head[..read];
                    let (status, body) = if head.starts_with(b"GET / ") {
                        ("200 OK", FETCH_ON_SCROLL_PAGE)
                    } else if head.starts_with(b"GET /section ") {
                        std::thread::sleep(delay);
                        ("200 OK", "<p>section-fetched-on-scroll</p>")
                    } else {
                        ("404 Not Found", "")
                    };
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                         Connection: close\r\n\r\n{body}",
                        body.len()
                    );
                    stream.write_all(response.as_bytes()).ok();
                });
            }
        });
        SlowSectionServer { port, stop }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }
}

#[cfg(feature = "browser")]
impl Drop for SlowSectionServer {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        // Wake the blocking `accept` so the thread sees the flag.
        std::net::TcpStream::connect(("127.0.0.1", self.port)).ok();
    }
}

/// Content that the scrolling starts loading is waited for before the capture, even when it
/// arrives after the scrolling has ended (#62).
#[cfg(feature = "browser")]
#[test]
#[ignore]
fn browser_auto_scroll_waits_for_content_fetched_on_scroll() {
    let server = SlowSectionServer::start(std::time::Duration::from_millis(1200));
    let tmp = TempDir::new("auto-scroll-fetch");

    let html = render_offline(&tmp, &server.url(), "offline", &[]);

    assert!(html.contains("section-fetched-on-scroll"), "{html}");
    assert!(
        !html.contains("data-siteone-freeze"),
        "the settle style is not captured"
    );
}

/// Unknown `--screenshot-viewport` entries are a configuration error (#46).
#[cfg(feature = "browser")]
#[test]
fn screenshot_viewport_rejects_unknown_entries() {
    let output = run_crawler(&[
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

    let output = run_crawler(&[
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

/// A green page that mounts a red, full-screen cookie banner only in viewports narrower than
/// 500 px, i.e. when it is resized to a further `--screenshot-viewport` size.
#[cfg(feature = "browser")]
const NARROW_COOKIE_BANNER_PAGE: &str = r#"<!doctype html>
<html><head><title>Narrow banner</title><style>
html,body{margin:0;background:rgb(0,180,0)}
#cookie-banner{position:fixed;inset:0;background:rgb(220,0,0);z-index:9999}
</style></head><body><h1>Content</h1><script>
function render(){
  if(innerWidth<500&&!document.querySelector('#cookie-banner')){
    var e=document.createElement('div');
    e.id='cookie-banner';e.className='site-overlay';e.textContent='Cookie consent';document.body.append(e);
  }
}
render();addEventListener('resize',render);
</script></body></html>"#;

/// A tall page that removes its banner and mounts it again on every resize while narrower than
/// 500 px, as a responsive consent component re-rendering on resize does.
#[cfg(feature = "browser")]
const REMOUNTING_COOKIE_BANNER_PAGE: &str = r#"<!doctype html>
<html><head><title>Remounting banner</title><style>
html,body{margin:0;background:rgb(0,180,0)}
#cookie-banner{position:fixed;inset:0;background:rgb(220,0,0);z-index:9999}
</style></head><body><h1>Content</h1><div style="height:1800px"></div><script>
function render(){
  document.querySelectorAll('#cookie-banner').forEach(function(e){e.remove();});
  if(innerWidth<500){
    var e=document.createElement('div');
    e.id='cookie-banner';e.className='site-overlay';e.textContent='Cookie consent';document.body.append(e);
  }
}
render();addEventListener('resize',render);
</script></body></html>"#;

/// `REMOUNTING_COOKIE_BANNER_PAGE` hardened against a hiding style sheet: the page's own CSS shows
/// the banner with `!important` at ID specificity, the re-created banner shows itself with an inline
/// `display:block!important`, and a CSP lets only the page's own nonce'd styles apply.
#[cfg(feature = "browser")]
const HARDENED_REMOUNTING_COOKIE_BANNER_PAGE: &str = r#"<!doctype html>
<html><head><title>Hardened banner</title>
<meta http-equiv="Content-Security-Policy" content="default-src 'self'; style-src 'nonce-t'; script-src 'nonce-t'">
<style nonce="t">
html,body{margin:0;background:rgb(0,180,0)}
.spacer{height:1800px}
#cookie-banner{position:fixed;inset:0;background:rgb(220,0,0);z-index:9999;display:block!important}
</style></head><body><h1>Content</h1><div class="spacer"></div><script nonce="t">
function render(){
  document.querySelectorAll('#cookie-banner').forEach(function(e){e.remove();});
  if(innerWidth<500){
    var e=document.createElement('div');
    e.id='cookie-banner';e.className='site-overlay';e.textContent='Cookie consent';
    e.style.setProperty('display','block','important');
    document.body.append(e);
  }
}
render();addEventListener('resize',render);
</script></body></html>"#;

/// The color at (100, 100) of the desktop and the mobile screenshot of `page` captured with
/// `--screenshot-viewport=desktop,mobile` and the given options.
#[cfg(feature = "browser")]
fn banner_screenshot_colors(page: &str, options: &[&str]) -> ([u8; 3], [u8; 3]) {
    let tmp = TempDir::new("viewport-banner");
    let site = tmp.path.join("site");
    std::fs::create_dir_all(&site).expect("site dir");
    std::fs::write(site.join("index.html"), page).expect("index.html");
    let server = LocalServer::start(&site);
    let shots = tmp.path.join("shots");

    let url = format!("--url={}", server.url());
    let shots_dir = format!("--screenshots-dir={}", shots.display());
    let mut args = vec![
        "--config-file=/dev/null",
        url.as_str(),
        "--single-page",
        "--browser",
        "--browser-no-sandbox",
        "--screenshots",
        shots_dir.as_str(),
        "--screenshot-viewport=desktop,mobile",
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--output-html-report=",
        "--output-json-file=",
        "--output-text-file=",
    ];
    args.extend_from_slice(options);
    let output = run_crawler(&args);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let color = |suffix: &str| {
        let file = std::fs::read_dir(&shots)
            .expect("screenshots dir")
            .map(|entry| entry.expect("dir entry").path())
            .find(|f| f.to_string_lossy().ends_with(suffix))
            .unwrap_or_else(|| panic!("a {suffix} screenshot"));
        image::open(&file).expect("a PNG").to_rgb8().get_pixel(100, 100).0
    };
    (color("_1920x1080.png"), color("_390x844.png"))
}

/// `--screenshot-hide-cookie-banners` also hides a banner that appears only in a further viewport
/// size (#46).
#[cfg(feature = "browser")]
#[test]
#[ignore]
fn cookie_banners_are_hidden_in_every_viewport() {
    let (desktop, mobile) = banner_screenshot_colors(NARROW_COOKIE_BANNER_PAGE, &["--screenshot-hide-cookie-banners"]);
    assert_eq!(desktop, [0, 180, 0]);
    assert_eq!(mobile, [0, 180, 0], "the banner covers the mobile screenshot");
}

/// `--screenshot-hide-selector` also hides an element that appears only in a further viewport
/// size (#46).
#[cfg(feature = "browser")]
#[test]
#[ignore]
fn hide_selector_is_applied_in_every_viewport() {
    let (desktop, mobile) =
        banner_screenshot_colors(NARROW_COOKIE_BANNER_PAGE, &["--screenshot-hide-selector=.site-overlay"]);
    assert_eq!(desktop, [0, 180, 0]);
    assert_eq!(mobile, [0, 180, 0], "the overlay covers the mobile screenshot");
}

/// Full-page screenshots keep the viewport of each size, so a page that mounts its banner again on
/// resize cannot cover the capture after the banner was hidden (#46).
#[cfg(feature = "browser")]
#[test]
#[ignore]
fn banners_stay_hidden_in_full_page_screenshots() {
    for hide_option in [
        "--screenshot-hide-cookie-banners",
        "--screenshot-hide-selector=.site-overlay",
    ] {
        let (desktop, mobile) = banner_screenshot_colors(
            REMOUNTING_COOKIE_BANNER_PAGE,
            &["--screenshot-mode=full-page", hide_option],
        );
        assert_eq!(desktop, [0, 180, 0], "{hide_option}");
        assert_eq!(
            mobile,
            [0, 180, 0],
            "{hide_option}: the banner covers the mobile screenshot"
        );
    }
}

/// A banner the page mounts again stays hidden also when the page's CSS or the re-created element
/// shows it with `!important` and a CSP blocks injected style sheets (#46).
#[cfg(feature = "browser")]
#[test]
#[ignore]
fn banners_stay_hidden_despite_important_styles_and_csp() {
    for hide_option in [
        "--screenshot-hide-cookie-banners",
        "--screenshot-hide-selector=.site-overlay",
    ] {
        let (desktop, mobile) = banner_screenshot_colors(
            HARDENED_REMOUNTING_COOKIE_BANNER_PAGE,
            &["--screenshot-mode=full-page", hide_option],
        );
        assert_eq!(desktop, [0, 180, 0], "{hide_option}");
        assert_eq!(
            mobile,
            [0, 180, 0],
            "{hide_option}: the banner covers the mobile screenshot"
        );
    }
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

    let output = run_crawler(&[
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
    // lazy-loading image attributes included (#109)
    let html_reference = regex::Regex::new(
        r#"\s(?:href|src|srcset|data-src|data-lazy-src|data-original|data-srcset|data-lazy-srcset)="([^"]*)""#,
    )
    .unwrap();
    let meta_refresh_reference = regex::Regex::new(r#"(?i)<meta[^>]*\burl=([^"'>\s]+)"#).unwrap();
    let css_reference = regex::Regex::new(r#"url\(['"]?([^'")]+)['"]?\)"#).unwrap();
    // `](destination)` or `](destination "title")`
    let markdown_reference = regex::Regex::new(r#"\]\(([^)\s]+)(?:\s+"(?:[^"\\]|\\.)*")?\)"#).unwrap();
    let mut dangling = Vec::new();
    for file in exported_files(export) {
        let patterns = match file.extension().and_then(|ext| ext.to_str()) {
            Some("html") => vec![&html_reference, &meta_refresh_reference],
            Some("css") => vec![&css_reference],
            Some("md") => vec![&markdown_reference],
            _ => continue,
        };
        // not text: an extension-less image stored as photo/index.html (#55)
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
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

    let output = run_crawler(&[
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
    // The image, not the redirect record of /img/a.png stored at the export root (which is no image)
    let css = std::fs::read_to_string(export.join("_www.site.test/style.css")).expect("the www stylesheet");
    assert!(css.contains("url(img/a.png)"), "{css}");
    assert_eq!(
        std::fs::read(export.join("_www.site.test/img/a.png")).expect("the image"),
        b"PNG"
    );
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

    let output = run_crawler(&[
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

/// #35: the crawler fetches a redirect target as it is, so with --force-relative-urls the record of
/// /redirect (301 to /landing on the www twin) leads to the twin's copy, while a www-variant link of
/// a page to itself (even with spaces around `=`) leads to the page's own file, also in Markdown
/// (which stores no redirect records, so only the self-link is checked there).
#[test]
fn force_relative_urls_exports_redirects_to_the_www_twin_and_self_links() {
    let tmp = TempDir::new("force-relative-redirect-path");
    let site = tmp.path.join("site");
    std::fs::create_dir_all(&site).expect("site dir");
    let server = RedirectServer::start(
        &site,
        vec![Redirect {
            host: Some("site.test"),
            path: Some("/redirect"),
            location: "http://www.site.test:{port}/landing",
        }],
    );
    let port = server.port();
    std::fs::write(
        site.join("index.html"),
        format!(
            r#"<html><head><title>Home</title></head><body><h1 id="top">Home</h1>
<a href="/redirect">Redirect</a> <a href = "http://www.site.test:{port}/#top">Top</a>
</body></html>"#
        ),
    )
    .expect("index.html");
    std::fs::write(
        site.join("landing.html"),
        r#"<html><head><title>Landing</title></head><body>Destination</body></html>"#,
    )
    .expect("landing.html");
    let export = tmp.path.join("export");
    let markdown = tmp.path.join("markdown");

    let output = run_crawler(&[
        "--config-file=/dev/null",
        &format!("--url=http://site.test:{port}/"),
        &format!("--resolve=site.test:{port}:127.0.0.1"),
        &format!("--resolve=www.site.test:{port}:127.0.0.1"),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--force-relative-urls",
        &format!("--offline-export-dir={}", export.display()),
        &format!("--markdown-export-dir={}", markdown.display()),
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        export.join("_www.site.test/landing.html").is_file(),
        "the redirect target is exported"
    );
    let record = std::fs::read_to_string(export.join("redirect.html")).expect("redirect.html");
    assert!(record.contains("url=_www.site.test/landing.html"), "{record}");
    let index = std::fs::read_to_string(export.join("index.html")).expect("index.html");
    assert!(index.contains(r#"href="index.html#top""#), "{index}");
    assert_eq!(dangling_references(&export), Vec::<String>::new());
    let index_md = std::fs::read_to_string(markdown.join("index.md")).expect("index.md");
    assert!(index_md.contains("[Top](index.md#top)"), "{index_md}");
}

/// #35: when only /jump redirects to the www twin, its target page is stored under
/// `_www.site.test/`, but the crawler fetches the links and images of that page from the initial
/// origin and stores them at the export root, so with --force-relative-urls the twin's page leads
/// there, in both file layouts.
#[test]
fn force_relative_urls_leads_links_of_a_redirected_www_twin_page_to_the_stored_files() {
    let tmp = TempDir::new("force-relative-twin-links");
    let site = tmp.path.join("site");
    std::fs::create_dir_all(&site).expect("site dir");
    let server = RedirectServer::start(
        &site,
        vec![
            Redirect {
                host: Some("site.test"),
                path: Some("/jump"),
                location: "http://www.site.test:{port}/landing",
            },
            Redirect {
                host: Some("site.test"),
                path: Some("/jump-deep"),
                location: "http://www.site.test:{port}/docs/landing",
            },
        ],
    );
    let port = server.port();
    std::fs::write(
        site.join("index.html"),
        r#"<html><head><title>Home</title></head><body><a href="/jump">Jump</a><a href="/jump-deep">Deep</a></body></html>"#,
    )
    .expect("index.html");
    std::fs::write(
        site.join("landing.html"),
        r#"<html><head><title>Landing</title></head><body><a href="/next">Next</a><img src="/photo.png" alt="Photo"></body></html>"#,
    )
    .expect("landing.html");
    std::fs::write(
        site.join("next.html"),
        r#"<html><head><title>Next</title></head><body>Next</body></html>"#,
    )
    .expect("next.html");
    std::fs::write(site.join("photo.png"), "PNG").expect("png");
    // references relative to a nested twin page
    std::fs::create_dir_all(site.join("docs")).expect("docs dir");
    std::fs::write(
        site.join("docs/landing.html"),
        r#"<html><head><title>Docs</title></head><body><a href="../next">Up</a><img src="../photo.png" alt="Photo"></body></html>"#,
    )
    .expect("docs/landing.html");

    for (layout, pages) in [
        (
            None,
            [
                (
                    "_www.site.test/landing.html",
                    [r#"href="../next.html""#, r#"src="../photo.png""#],
                ),
                (
                    "_www.site.test/docs/landing.html",
                    [r#"href="../../next.html""#, r#"src="../../photo.png""#],
                ),
            ],
        ),
        (
            Some("--offline-export-preserve-url-structure"),
            [
                (
                    "_www.site.test/landing/index.html",
                    [r#"href="../../next/index.html""#, r#"src="../../photo.png""#],
                ),
                (
                    "_www.site.test/docs/landing/index.html",
                    [r#"href="../../../next/index.html""#, r#"src="../../../photo.png""#],
                ),
            ],
        ),
    ] {
        let export = tmp.path.join(format!("export-{}", layout.is_some()));
        let mut args = vec![
            "--config-file=/dev/null".to_string(),
            format!("--url=http://site.test:{port}/"),
            format!("--resolve=site.test:{port}:127.0.0.1"),
            format!("--resolve=www.site.test:{port}:127.0.0.1"),
            LOCAL_ANALYZERS.to_string(),
            "--http-cache-dir=".to_string(),
            "--force-relative-urls".to_string(),
            format!("--offline-export-dir={}", export.display()),
        ];
        args.extend(layout.map(str::to_string));
        let output = run_crawler(&args.iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(
            output.status.code(),
            Some(0),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        for (landing, expected) in pages {
            let page = std::fs::read_to_string(export.join(landing)).expect("the redirect target is exported");
            for reference in expected {
                assert!(page.contains(reference), "{layout:?}: missing {reference} in {page}");
            }
        }
        assert_eq!(dangling_references(&export), Vec::<String>::new(), "{layout:?}");
    }
}

/// #35: a www-variant reference in a stylesheet downloaded from an allowed external domain is
/// fetched from the initial origin, so the exported stylesheet leads to the initial host's file.
#[test]
fn force_relative_urls_exports_variant_references_of_external_stylesheets() {
    let tmp = TempDir::new("force-relative-cdn");
    let site = tmp.path.join("site");
    std::fs::create_dir_all(&site).expect("site dir");
    let server = RedirectServer::start(&site, Vec::new());
    let port = server.port();
    std::fs::write(
        site.join("index.html"),
        format!(
            r#"<html><head><title>Home</title><link rel="stylesheet" href="http://cdn.test:{port}/style.css"></head><body>Home</body></html>"#
        ),
    )
    .expect("index.html");
    std::fs::write(
        site.join("style.css"),
        "body{background:url(https://www.site.test/img.png)}",
    )
    .expect("css");
    std::fs::write(site.join("img.png"), "PNG").expect("png");
    let export = tmp.path.join("export");

    let output = run_crawler(&[
        "--config-file=/dev/null",
        &format!("--url=http://site.test:{port}/"),
        &format!("--resolve=site.test:{port}:127.0.0.1"),
        &format!("--resolve=cdn.test:{port}:127.0.0.1"),
        "--allowed-domain-for-external-files=cdn.test",
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
        export.join("img.png").is_file(),
        "the image is fetched from the initial origin"
    );
    let css = std::fs::read_to_string(export.join("_cdn.test/style.css")).expect("the CDN stylesheet");
    assert!(css.contains("url(../img.png)"), "{css}");
    assert_eq!(dangling_references(&export), Vec::<String>::new());
}

/// #109: an image's attribute text is one attribute value (nothing in it is crawled or rewritten),
/// and a `>` in a quoted image URL does not end the tag, so that image is downloaded and exported.
#[test]
fn image_attributes_are_read_as_the_browser_reads_them() {
    let png = || Route {
        path: "",
        headers: vec![("Content-Type", "image/png".to_string())],
        body: PNG_1X1.to_vec(),
    };
    let server = RecordingServer::start(vec![
        Route {
            path: "/",
            headers: vec![("Content-Type", "text/html; charset=utf-8".to_string())],
            body: br#"<html><head><title>Images</title></head><body>
<img src="/real.png" alt="Today's diagram: data-original=/ghost1.png">
<img src="/real.png" title="Today's diagram: src=/ghost2.png">
<img src="/placeholder.png" data-src="/image?width=200&filter=>10" alt="Filtered">
</body></html>"#
                .to_vec(),
        },
        Route {
            path: "/real.png",
            ..png()
        },
        Route {
            path: "/placeholder.png",
            ..png()
        },
        Route {
            path: "/image",
            ..png()
        },
    ]);
    let tmp = TempDir::new("image-attributes");
    let export = tmp.path.join("export");

    let output = run_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        &format!("--offline-export-dir={}", export.display()),
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let requests: Vec<String> = server
        .requests()
        .iter()
        .filter_map(|head| head.split_whitespace().nth(1).map(str::to_string))
        .collect();
    assert!(!requests.iter().any(|path| path.contains("ghost")), "{requests:?}");
    assert!(
        requests.iter().any(|path| path == "/image?width=200&filter=%3E10"),
        "{requests:?}"
    );
    let index = std::fs::read_to_string(export.join("index.html")).expect("index.html");
    for expected in [
        r#"alt="Today's diagram: data-original=/ghost1.png""#,
        r#"title="Today's diagram: src=/ghost2.png""#,
    ] {
        assert!(index.contains(expected), "missing {expected} in {index}");
    }
    assert!(!index.contains("filter=>10"), "{index}");
    assert_eq!(dangling_references(&export), Vec::<String>::new());
}

/// #109: extension-less images on a domain allowed by --allowed-domain-for-external-files that
/// are referenced by lazy srcsets (on <img> and <source>) are downloaded and the srcsets lead to the
/// saved files; a domain that is not allowed stays online.
#[test]
fn lazy_srcsets_of_allowed_extensionless_cdn_images_lead_to_the_saved_files() {
    let png = |path| Route {
        path,
        headers: vec![("Content-Type", "image/png".to_string())],
        body: PNG_1X1.to_vec(),
    };
    let cdn = RecordingServer::start(vec![png("/one"), png("/two"), png("/three"), png("/four")]);
    let cdn_port = cdn.port();
    let site = RecordingServer::start(vec![Route {
        path: "/",
        headers: vec![("Content-Type", "text/html; charset=utf-8".to_string())],
        body: format!(
            r#"<html><head><title>CDN images</title></head><body>
<img data-src="http://cdn.test:{cdn_port}/one" data-srcset="http://cdn.test:{cdn_port}/two 1x, http://cdn.test:{cdn_port}/three 2x" alt="a">
<picture><source data-lazy-srcset="http://cdn.test:{cdn_port}/four 1x" type="image/png"><img data-srcset="http://denied.test:{cdn_port}/five 1x" alt="b"></picture>
</body></html>"#
        )
        .into_bytes(),
    }]);
    let tmp = TempDir::new("lazy-srcset-cdn");
    let export = tmp.path.join("export");

    let output = run_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", site.url()),
        &format!("--resolve=cdn.test:{cdn_port}:127.0.0.1"),
        &format!("--resolve=denied.test:{cdn_port}:127.0.0.1"),
        "--allowed-domain-for-external-files=cdn.test",
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        &format!("--offline-export-dir={}", export.display()),
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let cdn_requests = cdn.requests();
    for path in ["/one", "/two", "/three", "/four"] {
        assert!(
            cdn_requests
                .iter()
                .any(|head| head.starts_with(&format!("GET {path} "))),
            "{path} is downloaded: {cdn_requests:?}"
        );
    }
    assert!(
        !cdn_requests.iter().any(|head| head.starts_with("GET /five ")),
        "{cdn_requests:?}"
    );
    let index = std::fs::read_to_string(export.join("index.html")).expect("index.html");
    for expected in [
        r#" data-src="_cdn.test/one.jpg""#,
        r#" data-srcset="_cdn.test/two.jpg 1x, _cdn.test/three.jpg 2x""#,
        r#" data-lazy-srcset="_cdn.test/four.jpg 1x""#,
        // not allowed for external files: online, without the port the export strips
        r#" data-srcset="http://denied.test/five 1x""#,
    ] {
        assert!(index.contains(expected), "missing {expected} in {index}");
    }
    assert_eq!(dangling_references(&export), Vec::<String>::new());
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
<a href="?page=2">Page 2</a>
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
        let output = run_crawler(&args);
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

        let output = run_crawler(&[
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
    let output = run_crawler(&[
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

/// #55: the static copy on the original URLs (README: --offline-export-preserve-url-structure with
/// --offline-export-preserve-urls) keeps links root-relative, so each one must be served from the
/// export by the README's `try_files $uri $uri/ $uri/index.html`, also a ../ link on a nested page.
#[test]
fn preserve_url_structure_with_original_urls_serves_every_link() {
    let tmp = TempDir::new("preserve-original-urls");
    let site = tmp.path.join("site");
    let files = [
        (
            "index.html",
            r#"<html><head><title>Home</title></head><body><a href="/a/b/page">Page</a></body></html>"#,
        ),
        (
            "a/b/page.html",
            r#"<html><head><title>Page</title><link rel="stylesheet" href="../../css/site.css"></head><body>
<a href="../sibling">Sibling</a> <a href="../../">Home</a> <img src="../img/pic.png" alt="pic">
</body></html>"#,
        ),
        (
            "a/sibling.html",
            r#"<html><head><title>Sibling</title></head><body><a href="b/page">Page</a></body></html>"#,
        ),
        ("a/img/pic.png", "PNG"),
        ("css/site.css", "body{background:url(../img/bg.png)}"),
        ("img/bg.png", "PNG"),
    ];
    for (path, content) in files {
        let file = site.join(path);
        std::fs::create_dir_all(file.parent().expect("parent dir")).expect("site dir");
        std::fs::write(file, content).expect("site file");
    }
    let server = LocalServer::start(&site);
    let export = tmp.path.join("export");

    let output = run_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--offline-export-preserve-url-structure",
        "--offline-export-preserve-urls",
        "--offline-export-no-auto-redirect-html",
        &format!("--offline-export-dir={}", export.display()),
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let page = std::fs::read_to_string(export.join("a/b/page/index.html")).expect("a/b/page/index.html");
    assert!(page.contains(r#"href="/a/sibling""#), "{page}");
    let reference = regex::Regex::new(r#"(?:\s(?:href|src)="|url\()(/[^")]*)"#).unwrap();
    let mut unserved = Vec::new();
    for file in exported_files(&export) {
        let text = std::fs::read_to_string(&file).expect("exported text file");
        for caps in reference.captures_iter(&text) {
            let path = caps[1].split(['#', '?']).next().unwrap_or("");
            let local = export.join(path.trim_start_matches('/'));
            if !local.is_file() && !local.join("index.html").is_file() {
                unserved.push(format!(
                    "{} -> {}",
                    file.strip_prefix(&export).unwrap().display(),
                    &caps[1]
                ));
            }
        }
    }
    assert_eq!(unserved, Vec::<String>::new());
}

/// A 1×1 PNG: binary, not valid UTF-8.
const PNG_1X1: [u8; 68] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xb5, 0x1c, 0x0c, 0x02, 0x00, 0x00, 0x00, 0x0b, 0x49,
    0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xfc, 0xff, 0x1f, 0x00, 0x03, 0x03, 0x02, 0x00, 0xef, 0x9a, 0xde, 0x2a, 0x00,
    0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// #55: with --offline-export-preserve-url-structure an image on an extension-less URL (/photo) is
/// stored in the page layout (photo/index.html), so the original-URL copy serves it at /photo. The
/// markdown export keeps its bytes instead of converting it to an empty photo/index.md, in a file with
/// the extension of its content type (photo/index.png, logo/index.svg), so a Markdown viewer shows it:
/// an SVG image is not shown from a file named .html.
#[test]
fn preserve_url_structure_keeps_extensionless_images_intact() {
    let tmp = TempDir::new("preserve-extensionless-image");
    let server = RecordingServer::start(vec![
        Route {
            path: "/",
            headers: vec![("Content-Type", "text/html; charset=utf-8".to_string())],
            body: br#"<html><head><title>Home</title></head><body><h1>Home</h1><img src="/photo" alt="Photo"><a href="/icons/">Icons</a></body></html>"#
                .to_vec(),
        },
        Route {
            path: "/photo",
            headers: vec![("Content-Type", "image/png".to_string())],
            body: PNG_1X1.to_vec(),
        },
        Route {
            path: "/icons/",
            headers: vec![("Content-Type", "text/html; charset=utf-8".to_string())],
            body: br#"<html><head><title>Icons</title></head><body><h1>Icons</h1><img src="/logo" alt="Logo" title="Our logo"></body></html>"#
                .to_vec(),
        },
        Route {
            path: "/logo",
            headers: vec![("Content-Type", "image/svg+xml".to_string())],
            body: SVG_LOGO.to_vec(),
        },
    ]);
    let export = tmp.path.join("offline");
    let markdown = tmp.path.join("markdown");

    let output = run_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--offline-export-preserve-url-structure",
        &format!("--offline-export-dir={}", export.display()),
        &format!("--markdown-export-dir={}", markdown.display()),
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let index = std::fs::read_to_string(export.join("index.html")).expect("index.html");
    let src = regex::Regex::new(r#"<img src="([^"]+)""#)
        .unwrap()
        .captures(&index)
        .expect("an <img> in index.html")[1]
        .to_string();
    assert_eq!(
        std::fs::read(export.join(&src)).expect("the offline image"),
        PNG_1X1,
        "offline {src}"
    );
    for (page, alt, file, bytes) in [
        ("index.md", "Photo", "photo/index.png", &PNG_1X1[..]),
        ("icons/index.md", "Logo", "../logo/index.svg", SVG_LOGO),
    ] {
        let page_md = std::fs::read_to_string(markdown.join(page)).expect("the markdown page");
        let image = regex::Regex::new(&format!(r"!\[{alt}\]\(([^)\s]+)"))
            .unwrap()
            .captures(&page_md)
            .unwrap_or_else(|| panic!("an image in {page}: {page_md}"))[1]
            .to_string();
        assert_eq!(image, file, "{page}");
        let stored = markdown.join(page).parent().expect("page directory").join(&image);
        assert_eq!(
            std::fs::read(&stored).expect("the markdown image"),
            bytes,
            "markdown {image}"
        );
    }
    assert_eq!(dangling_references(&export), Vec::<String>::new(), "offline export");
    assert_eq!(dangling_references(&markdown), Vec::<String>::new(), "markdown export");
}

const SVG_LOGO: &[u8] =
    br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="20" height="20" fill="red"/></svg>"#;

// ---------------------------------------------------------------------------
// Mock LLM server (`common::MockLlm`) used by the offline AI tests
// ---------------------------------------------------------------------------

/// Sends one POST with `body` to the mock and returns the raw response (head and body).
fn post_to_mock(mock: &MockLlm, path: &str, body: &str) -> String {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", mock.port())).expect("the mock accepts");
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).expect("the request is sent");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("the response is read");
    response
}

#[test]
fn mock_llm_serves_its_responses_in_order_per_path() {
    let fixture = include_str!("fixtures/ai-responses/vllm-qwen-think.json");
    let mock = MockLlm::start(vec![
        MockResponse {
            path_prefix: "/v1/chat/completions",
            status: 429,
            body: r#"{"error":{"message":"slow down"}}"#.to_string(),
            delay_ms: 0,
        },
        MockResponse {
            path_prefix: "/v1/chat/completions",
            status: 200,
            body: fixture.to_string(),
            delay_ms: 0,
        },
        MockResponse {
            path_prefix: "/v1/models",
            status: 200,
            body: r#"{"data":[]}"#.to_string(),
            delay_ms: 0,
        },
    ]);
    assert_eq!(mock.url(), format!("http://127.0.0.1:{}/v1", mock.port()));

    let first = post_to_mock(&mock, "/v1/chat/completions", r#"{"n":1}"#);
    assert!(first.starts_with("HTTP/1.1 429 "), "{first}");
    let second = post_to_mock(&mock, "/v1/chat/completions", r#"{"n":2}"#);
    assert!(second.starts_with("HTTP/1.1 200 "), "{second}");
    assert!(second.ends_with(fixture), "the fixture body is served verbatim");
    let models = post_to_mock(&mock, "/v1/models", "");
    assert!(models.ends_with(r#"{"data":[]}"#), "{models}");
    let third = post_to_mock(&mock, "/v1/chat/completions", r#"{"n":3}"#);
    assert!(
        third.starts_with("HTTP/1.1 200 ") && third.ends_with(fixture),
        "the last response repeats once the others are used up"
    );
    let unknown = post_to_mock(&mock, "/other", "x");
    assert!(unknown.starts_with("HTTP/1.1 404 "), "{unknown}");

    assert_eq!(
        mock.request_bodies(),
        vec![r#"{"n":1}"#, r#"{"n":2}"#, "", r#"{"n":3}"#, "x"],
        "every request body is recorded in arrival order"
    );
    let heads = mock.request_heads();
    assert!(heads[2].starts_with("POST /v1/models HTTP/1.1\r\n"), "{}", heads[2]);
}

#[test]
fn mock_llm_delays_its_answer() {
    let mock = MockLlm::start(vec![MockResponse {
        path_prefix: "/v1/",
        status: 200,
        body: "{}".to_string(),
        delay_ms: 300,
    }]);
    let started = std::time::Instant::now();
    let response = post_to_mock(&mock, "/v1/chat/completions", "{}");
    assert!(response.ends_with("{}"), "{response}");
    assert!(started.elapsed() >= std::time::Duration::from_millis(300));
}

// ---------------------------------------------------------------------------
// Per-request AI telemetry on stderr (offline, against `MockLlm`)
// ---------------------------------------------------------------------------

/// The captured vLLM Qwen response (thinking on: 17 in, 37 out, 33 of them reasoning) with its
/// answer replaced by a valid SEO result, so the SEO action succeeds on the first request.
fn qwen_seo_answer() -> String {
    let mut response: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/ai-responses/vllm-qwen-think.json")).expect("JSON");
    response["choices"][0]["message"]["content"] = serde_json::json!(r#"{"scores":{"overall":80}}"#);
    response.to_string()
}

fn chat_response(status: u16, body: String) -> MockResponse {
    MockResponse {
        path_prefix: "/v1/chat/completions",
        status,
        body,
        // Long enough for a measurable duration, and so a tok/s value.
        delay_ms: 100,
    }
}

/// A local one-page site to run the AI actions on.
fn one_page_site(tmp: &TempDir) -> LocalServer {
    let site = tmp.path.join("site");
    write_site(&site, 0);
    LocalServer::start(&site)
}

/// Crawls `server` with the SEO action against `mock`; returns stderr.
fn crawl_with_ai_seo(server: &LocalServer, mock: &MockLlm, extra: &[&str]) -> String {
    let mut args = vec!["--ai-actions=seo", "--ai-max-pages=1"];
    args.extend(extra);
    crawl_with_ai(server, mock, &args)
}

/// Crawls `server` with the AI features of `extra` against `mock`; returns stderr.
fn crawl_with_ai(server: &LocalServer, mock: &MockLlm, extra: &[&str]) -> String {
    let mut args = vec![
        "--config-file=/dev/null".to_string(),
        format!("--url={}", server.url()),
        LOCAL_ANALYZERS.to_string(),
        "--http-cache-dir=".to_string(),
        "--no-color".to_string(),
        "--ai-provider=openai-compatible".to_string(),
        format!("--ai-endpoint={}", mock.url()),
        "--ai-model=m".to_string(),
    ];
    args.extend(extra.iter().map(|arg| arg.to_string()));
    if !extra.iter().any(|arg| arg.starts_with("--ai-cache-dir=")) {
        args.push("--ai-cache-dir=".to_string());
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = run_crawler(&args);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
    stderr
}

fn assert_line(stderr: &str, pattern: &str) {
    let re = regex::Regex::new(&format!("(?m)^{pattern}$")).expect("a valid pattern");
    assert!(re.is_match(stderr), "no line matching {pattern:?} in stderr:\n{stderr}");
}

#[test]
fn ai_request_is_reported_with_tokens_reasoning_time_and_speed() {
    let tmp = TempDir::new("ai-telemetry-ok");
    let mock = MockLlm::start(vec![chat_response(200, qwen_seo_answer())]);
    let stderr = crawl_with_ai_seo(&one_page_site(&tmp), &mock, &[]);
    assert_line(
        &stderr,
        r"  AI ✓ #1 SEO 1/1 · / · 17 in · 37 out \(33 reasoning\) · \d+\.\d s · \d+ tok/s",
    );
    assert_eq!(mock.request_bodies().len(), 1);
}

#[test]
fn ai_request_with_uncounted_reasoning_says_so() {
    let tmp = TempDir::new("ai-telemetry-minimax");
    // MiniMax reasons inline in <think> and reports no reasoning count (183 in, 30 out).
    let mut response: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/ai-responses/minimax.json")).expect("JSON");
    response["choices"][0]["message"]["content"] =
        serde_json::json!("<think>The user wants an SEO review.</think>\n\n{\"scores\":{\"overall\":80}}");
    let mock = MockLlm::start(vec![chat_response(200, response.to_string())]);
    let stderr = crawl_with_ai_seo(&one_page_site(&tmp), &mock, &[]);
    assert_line(
        &stderr,
        r"  AI ✓ #1 SEO 1/1 · / · 183 in · 30 out \(reasoning n/a\) · \d+\.\d s · \d+ tok/s",
    );
}

#[test]
fn ai_request_without_usage_is_still_reported() {
    let tmp = TempDir::new("ai-telemetry-no-usage");
    let body = serde_json::json!({"id": "x", "choices": [{"message": {"content": r#"{"scores":{"overall":80}}"#}}]});
    let mock = MockLlm::start(vec![chat_response(200, body.to_string())]);
    let stderr = crawl_with_ai_seo(&one_page_site(&tmp), &mock, &[]);
    assert_line(&stderr, r"  AI ✓ #1 SEO 1/1 · / · tokens not reported · \d+\.\d s");
}

#[test]
fn ai_request_retry_and_success_are_both_reported() {
    let tmp = TempDir::new("ai-telemetry-retry");
    let mock = MockLlm::start(vec![
        chat_response(429, r#"{"error":{"message":"slow down"}}"#.to_string()),
        chat_response(200, qwen_seo_answer()),
    ]);
    // Two seconds between sends: the retry waits for the 1 s backoff and then for its rate slot.
    let stderr = crawl_with_ai_seo(&one_page_site(&tmp), &mock, &["--ai-max-reqs-per-sec=0.5"]);
    assert_line(
        &stderr,
        r"  AI ↻ #1 SEO 1/1 · / · HTTP 429 · \d+\.\d s · retrying \(attempt 2/3\)",
    );
    // The mock answers in 0.1 s; neither the backoff nor the rate-limit wait is part of that time.
    assert_line(
        &stderr,
        r"  AI ✓ #2 SEO 1/1 · / · 17 in · 37 out \(33 reasoning\) · 0\.\d s · \d+ tok/s",
    );
    assert!(stderr.find("AI ↻ #1") < stderr.find("AI ✓ #2"), "in arrival order");
}

#[test]
fn ai_provider_error_is_reported() {
    let tmp = TempDir::new("ai-telemetry-error");
    let mock = MockLlm::start(vec![chat_response(
        404,
        include_str!("fixtures/ai-responses/error-vllm-unknown-model.json").to_string(),
    )]);
    let stderr = crawl_with_ai_seo(&one_page_site(&tmp), &mock, &[]);
    assert_line(
        &stderr,
        r"  AI ✗ #1 SEO 1/1 · / · AI provider error: The model `no-such-model` does not exist\. · \d+\.\d s",
    );
}

/// Crawls `server` against `mock` with colours forced on, as a GUI runs the crawler; returns
/// stderr.
fn crawl_with_ai_in_colour(server: &LocalServer, mock: &MockLlm, extra: &[&str]) -> String {
    let url = format!("--url={}", server.url());
    let endpoint = format!("--ai-endpoint={}", mock.url());
    let mut args = vec![
        "--config-file=/dev/null",
        url.as_str(),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--force-color",
        "--ai-provider=openai-compatible",
        endpoint.as_str(),
        "--ai-model=m",
        "--ai-cache-dir=",
    ];
    args.extend(extra);
    let output = run_crawler(&args);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
    stderr
}

/// A GUI paints stderr text without a colour of its own as an error. The AI phase writes to
/// stderr, so every line of it needs a colour.
#[test]
fn every_line_of_the_ai_phase_has_a_colour() {
    let tmp = TempDir::new("ai-colours");
    let site = tmp.path.join("site");
    write_site(&site, 1);
    let server = LocalServer::start(&site);
    let mock = MockLlm::start(vec![chat_response(
        404,
        include_str!("fixtures/ai-responses/error-vllm-unknown-model.json").to_string(),
    )]);
    let cases: [(&[&str], &[&str]); 4] = [
        (
            &["--ai-actions=seo,summary"],
            &["HTML pages crawled", "AI summary: area 'seo' failed"],
        ),
        (
            &[
                "--ai-actions=seo",
                "--ai-dry-run",
                "--ai-input-cost-per-million=1",
                "--ai-output-cost-per-million=2",
            ],
            &["Estimated input-only cost floor", "1. score"],
        ),
        (&["--ai-elaborate", "--ai-dry-run"], &["1. http"]),
        (&["--ai-profile", "--ai-dry-run"], &["1. http"]),
    ];
    for (args, expected) in cases {
        let stderr = crawl_with_ai_in_colour(&server, &mock, args);
        for text in expected {
            assert!(stderr.contains(text), "{args:?}: no {text:?} in stderr:\n{stderr}");
        }
        let lines: Vec<&str> = stderr
            .lines()
            .skip_while(|line| !line.contains("AI "))
            .filter(|line| !line.trim().is_empty())
            .collect();
        for line in lines {
            assert!(
                line.trim_start().starts_with('\x1b'),
                "{args:?}: a line without a colour: {line:?}\nstderr:\n{stderr}"
            );
        }
    }
}

/// The `aiUsage` event of a crawl of a site with `pages` + 1 pages, one AI request at a time.
fn ai_usage_of(name: &str, pages: usize, responses: Vec<MockResponse>, extra: &[&str]) -> serde_json::Value {
    let tmp = TempDir::new(&format!("ai-usage-{name}"));
    let site = tmp.path.join("site");
    write_site(&site, pages);
    let server = LocalServer::start(&site);
    let mock = MockLlm::start(responses);
    let events = tmp.path.join("events.ndjson");
    let events_arg = format!("--events-file={}", events.display());
    let mut args = vec![
        "--ai-actions=seo",
        "--ai-max-pages=5",
        "--ai-max-concurrency=1",
        &events_arg,
    ];
    args.extend(extra);
    crawl_with_ai(&server, &mock, &args);
    let text = std::fs::read_to_string(&events).expect("the events file");
    text.lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a JSON event"))
        .find(|event| event["type"] == "aiUsage")
        .unwrap_or_else(|| panic!("{name}: no aiUsage event in {text}"))
}

#[test]
fn ai_usage_counts_the_time_and_answers_of_failed_calls() {
    let count = |usage: &serde_json::Value, field: &str| usage[field].as_u64().unwrap_or_default();
    let mut wrong = Vec::new();

    // A timeout: no answer, but a second spent waiting for it.
    let slow = MockResponse {
        delay_ms: 2500,
        ..chat_response(200, qwen_seo_answer())
    };
    let usage = ai_usage_of("timeout", 0, vec![slow], &["--ai-timeout=1"]);
    if (count(&usage, "calls"), count(&usage, "httpAttempts")) != (0, 1) || count(&usage, "networkMs") < 1000 {
        wrong.push(format!("timeout: {usage}"));
    }

    // A 2xx answer that is not JSON: a completed call without token usage.
    let malformed = MockResponse {
        delay_ms: 200,
        ..chat_response(200, "<html>upstream response</html>".to_string())
    };
    let usage = ai_usage_of("malformed", 0, vec![malformed], &[]);
    if (count(&usage, "calls"), count(&usage, "callsWithoutUsage")) != (1, 1) || count(&usage, "networkMs") < 200 {
        wrong.push(format!("malformed: {usage}"));
    }

    // A success, then an HTTP error: one call with its tokens, and the time of both.
    let answers = vec![
        chat_response(200, qwen_seo_answer()),
        chat_response(
            404,
            include_str!("fixtures/ai-responses/error-vllm-unknown-model.json").to_string(),
        ),
    ];
    let usage = ai_usage_of("mixed", 1, answers, &[]);
    let counts = (
        count(&usage, "calls"),
        count(&usage, "httpAttempts"),
        count(&usage, "inputTokens"),
    );
    if counts != (1, 2, 17) || count(&usage, "networkMs") < 200 {
        wrong.push(format!("mixed: {usage}"));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn ai_refusal_is_reported_with_its_tokens_and_speed() {
    let tmp = TempDir::new("ai-telemetry-refusal");
    let refusal = serde_json::json!({
        "choices": [{"message": {"content": null, "refusal": "Review refusal"}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 7, "completion_tokens": 3, "completion_tokens_details": {"reasoning_tokens": 2}}
    });
    let mock = MockLlm::start(vec![chat_response(200, refusal.to_string())]);
    let stderr = crawl_with_ai_seo(&one_page_site(&tmp), &mock, &[]);
    assert_line(
        &stderr,
        r"  AI ✗ #1 SEO 1/1 · / · AI response had no content \(refusal: Review refusal\) · 7 in · 3 out \(2 reasoning\) · \d+\.\d s · \d+ tok/s",
    );
}

#[test]
fn ai_request_that_timed_out_is_not_retried() {
    let tmp = TempDir::new("ai-telemetry-timeout");
    // Slower than the 1 s timeout: the provider may still be generating (and charging for) it.
    let mock = MockLlm::start(vec![MockResponse {
        delay_ms: 2500,
        ..chat_response(200, qwen_seo_answer())
    }]);
    let stderr = crawl_with_ai_seo(&one_page_site(&tmp), &mock, &["--ai-timeout=1"]);
    assert_eq!(mock.request_bodies().len(), 1, "a timed-out request is sent once");
    assert_line(
        &stderr,
        r"  AI ✗ #1 SEO 1/1 · / · AI request error: .*timed out.* · 1\.\d s",
    );
    assert!(!stderr.contains("AI ↻"), "stderr: {stderr}");
}

#[test]
fn ai_errors_never_carry_endpoint_credentials() {
    let tmp = TempDir::new("ai-endpoint-credentials");
    let server = one_page_site(&tmp);
    // Slower than the 1 s timeout: the requests fail with a transport error, which quotes its URL.
    let mock = MockLlm::start(vec![MockResponse {
        delay_ms: 2500,
        ..chat_response(200, qwen_seo_answer())
    }]);
    // reqwest moves `user:pass@` into an Authorization header, but userinfo it cannot decode (a
    // non-UTF-8 escape) stays in the URL of the request and so of its errors.
    for (name, userinfo) in [("plain", "user:PW_SENTINEL"), ("undecodable", "%FFuser:PW_SENTINEL")] {
        let out = tmp.path.join(name);
        std::fs::create_dir_all(&out).expect("an output dir");
        let events = out.join("events.ndjson");
        let args = [
            "--config-file=/dev/null".to_string(),
            format!("--url={}", server.url()),
            LOCAL_ANALYZERS.to_string(),
            "--http-cache-dir=".to_string(),
            "--no-color".to_string(),
            "--ai-provider=openai-compatible".to_string(),
            format!("--ai-endpoint=http://{userinfo}@127.0.0.1:{}/v1", mock.port()),
            "--ai-model=m".to_string(),
            "--ai-actions=seo".to_string(),
            "--ai-report=ia".to_string(),
            "--ai-max-pages=1".to_string(),
            "--ai-timeout=1".to_string(),
            "--ai-cache-dir=".to_string(),
            format!("--events-file={}", events.display()),
            format!("--ai-report-dir={}", out.display()),
        ];
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = run_crawler(&args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("AI SEO failed"),
            "the requests failed ({name}): {stderr}"
        );
        let mut texts = vec![
            (
                "stdout".to_string(),
                String::from_utf8_lossy(&output.stdout).into_owned(),
            ),
            ("stderr".to_string(), stderr.into_owned()),
        ];
        for entry in std::fs::read_dir(&out).expect("the output dir") {
            let path = entry.expect("an entry").path();
            let text = String::from_utf8_lossy(&std::fs::read(&path).expect("a file")).into_owned();
            texts.push((path.display().to_string(), text));
        }
        assert!(texts.len() > 3, "events and report files were written ({name})");
        for (source, text) in texts {
            let leak = text.lines().find(|line| line.contains("PW_SENTINEL"));
            assert!(leak.is_none(), "{name}: the password in {source}: {leak:?}");
        }
    }
}

/// Synthetic credentials: a key long enough to be blanked, an endpoint password and a query token.
const KEY_SENTINEL: &str = "sk-KEY_SENTINEL-0001-abcdefghijklmnopqrstuvwxyz";
const PW_SENTINEL: &str = "PW_SENTINEL_0002";
const QUERY_SENTINEL: &str = "QUERY_SENTINEL_0003";

/// Every file below `dir`, in subdirectories too.
fn files_under(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).expect("a directory") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            files.extend(files_under(&path));
        } else {
            files.push(path);
        }
    }
    files
}

/// The lines of `texts` that carry one of the sentinels or the key's first 12 characters.
fn credential_leaks(case: &str, texts: &[(String, String)]) -> Vec<String> {
    let mut leaks = Vec::new();
    for (source, text) in texts {
        for secret in [&KEY_SENTINEL[..12], PW_SENTINEL, QUERY_SENTINEL] {
            if let Some(line) = text.lines().find(|line| line.contains(secret)) {
                leaks.push(format!("{case}: {secret} in {source}: {line}"));
            }
        }
    }
    leaks
}

#[test]
fn ai_diagnostics_never_carry_credentials() {
    let tmp = TempDir::new("ai-diagnostic-credentials");
    let server = one_page_site(&tmp);
    let echo = format!(
        r#"{{"error":{{"message":"Proxy rejected http://review:{PW_SENTINEL}@proxy.test/v1?token={QUERY_SENTINEL} for key {KEY_SENTINEL}"}}}}"#
    );
    let refusal = serde_json::json!({
        "choices": [{"message": {"content": null, "refusal": format!("{}{KEY_SENTINEL}", "x".repeat(180))},
            "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 20, "completion_tokens": 4}
    });
    let report: &[&str] = &["--ai-report=ia"];
    let cases = [
        // A provider or proxy that quotes the endpoint and the key it rejected, also userinfo
        // that does not decode.
        (
            "echo",
            401,
            echo.clone(),
            100,
            format!("http://review:{PW_SENTINEL}@127.0.0.1:{{port}}/v1?token={QUERY_SENTINEL}"),
            report,
        ),
        (
            "undecodable-echo",
            401,
            echo.replace("http://review:", "http://%FFreview:"),
            100,
            format!("http://%FFreview:{PW_SENTINEL}@127.0.0.1:{{port}}/v1?token={QUERY_SENTINEL}"),
            report,
        ),
        // An answer: its events and the AI cache file written for it.
        (
            "answered",
            200,
            qwen_seo_answer(),
            100,
            format!("http://review:{PW_SENTINEL}@127.0.0.1:{{port}}/v1?token={QUERY_SENTINEL}"),
            &[],
        ),
        // A timeout: the transport error names the request URL, query token included.
        (
            "query-timeout",
            200,
            qwen_seo_answer(),
            2500,
            format!("http://127.0.0.1:{{port}}/v1?token={QUERY_SENTINEL}"),
            report,
        ),
        // A long refusal or non-JSON body quoting the key where the 200-character cut falls.
        (
            "refusal",
            200,
            refusal.to_string(),
            100,
            "http://127.0.0.1:{port}/v1".to_string(),
            report,
        ),
        (
            "non-json",
            200,
            format!("<html>{}{KEY_SENTINEL}</html>", "x".repeat(180)),
            100,
            "http://127.0.0.1:{port}/v1".to_string(),
            report,
        ),
        // A finish reason is quoted in the request's event and in the error of the stopped answer.
        (
            "finish-reason",
            200,
            serde_json::json!({"choices": [{"message": {"content": r#"{"scores":{"overall":80}}"#},
                "finish_reason": KEY_SENTINEL}]})
            .to_string(),
            100,
            "http://127.0.0.1:{port}/v1".to_string(),
            report,
        ),
    ];
    let mut leaks = Vec::new();
    for (name, status, body, delay_ms, endpoint, extra) in cases {
        let mock = MockLlm::start(vec![MockResponse {
            path_prefix: "/v1",
            status,
            body,
            delay_ms,
        }]);
        let out = tmp.path.join(name);
        std::fs::create_dir_all(&out).expect("an output dir");
        let mut args = vec![
            "--config-file=/dev/null".to_string(),
            format!("--url={}", server.url()),
            LOCAL_ANALYZERS.to_string(),
            "--http-cache-dir=".to_string(),
            "--no-color".to_string(),
            "--ai-provider=openai-compatible".to_string(),
            format!("--ai-endpoint={}", endpoint.replace("{port}", &mock.port().to_string())),
            "--ai-model=m".to_string(),
            format!("--ai-api-key={KEY_SENTINEL}"),
            "--ai-actions=seo".to_string(),
            "--ai-max-pages=1".to_string(),
            "--ai-timeout=1".to_string(),
            format!("--ai-cache-dir={}", out.join("ai-cache").display()),
            format!("--events-file={}", out.join("events.ndjson").display()),
            format!("--ai-report-dir={}", out.display()),
        ];
        args.extend(extra.iter().map(|arg| arg.to_string()));
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = run_crawler(&args);
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert!(
            !mock.request_heads().is_empty(),
            "{name}: AI requests were made: {stderr}"
        );
        let mut texts = vec![
            (
                "stdout".to_string(),
                String::from_utf8_lossy(&output.stdout).into_owned(),
            ),
            ("stderr".to_string(), stderr),
        ];
        for path in files_under(&out) {
            let text = String::from_utf8_lossy(&std::fs::read(&path).expect("a file")).into_owned();
            texts.push((path.display().to_string(), text));
        }
        let written = if extra.is_empty() {
            "events and the AI cache"
        } else {
            "events and report files"
        };
        assert!(texts.len() > 3, "{name}: {written} were written");
        leaks.extend(credential_leaks(name, &texts));
    }
    assert!(leaks.is_empty(), "{}", leaks.join("\n"));
}

#[test]
fn ai_cache_hit_is_reported() {
    let tmp = TempDir::new("ai-telemetry-cache");
    let cache = tmp.path.join("ai-cache");
    let cache_arg = format!("--ai-cache-dir={}", cache.display());
    let mock = MockLlm::start(vec![chat_response(200, qwen_seo_answer())]);
    // The same site (and so the same prompt) twice.
    let server = one_page_site(&tmp);
    crawl_with_ai_seo(&server, &mock, &[&cache_arg]);
    let stderr = crawl_with_ai_seo(&server, &mock, &[&cache_arg]);
    assert_line(
        &stderr,
        r"  AI ⇢ #1 SEO 1/1 · / · cache hit · 17 in · 37 out \(33 reasoning\)",
    );
    assert_eq!(
        mock.request_bodies().len(),
        1,
        "the second run is served from the cache"
    );
}

#[test]
fn ai_request_lines_are_hidden_with_hide_progress_bar() {
    let tmp = TempDir::new("ai-telemetry-hidden");
    let mock = MockLlm::start(vec![chat_response(200, qwen_seo_answer())]);
    let stderr = crawl_with_ai_seo(&one_page_site(&tmp), &mock, &["--hide-progress-bar"]);
    assert_eq!(mock.request_bodies().len(), 1, "the request is made");
    assert!(!stderr.contains("AI ✓"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// AI task progress on stderr: every request names its task, progress and subject
// ---------------------------------------------------------------------------

/// Every per-request line names a task with its progress and a subject: `#n Label d/t · subject · …`.
fn assert_every_request_names_its_task(stderr: &str) {
    let request = regex::Regex::new(r"^  AI [✓↻✗⇢] #\d+ ").expect("a valid pattern");
    let with_task = regex::Regex::new(r"^  AI [✓↻✗⇢] #\d+ .+? \d+/\d+ · [^·]+ · ").expect("a valid pattern");
    let lines: Vec<&str> = stderr.lines().filter(|line| request.is_match(line)).collect();
    assert!(!lines.is_empty(), "no AI request lines in stderr:\n{stderr}");
    for line in lines {
        assert!(
            with_task.is_match(line),
            "a request without its task: {line:?}\n{stderr}"
        );
    }
}

#[test]
fn ai_request_lines_carry_their_task_and_progress() {
    let tmp = TempDir::new("ai-progress-actions");
    let site = tmp.path.join("site");
    write_site(&site, 2);
    let server = LocalServer::start(&site);
    let mock = MockLlm::start(vec![chat_response(200, qwen_seo_answer())]);
    // One request at a time, so the progress numbers arrive in order.
    let stderr = crawl_with_ai(
        &server,
        &mock,
        &["--ai-actions=seo,typos", "--ai-max-pages=3", "--ai-max-concurrency=1"],
    );
    for n in 1..=3 {
        assert_line(
            &stderr,
            &format!(r"  AI ✓ #\d+ SEO {n}/3 · /\S* · 17 in · 37 out \(33 reasoning\) · \d+\.\d s · \d+ tok/s"),
        );
        assert_line(
            &stderr,
            &format!(r"  AI ✓ #\d+ Typos {n}/3 · /\S* · 17 in · 37 out \(33 reasoning\) · \d+\.\d s · \d+ tok/s"),
        );
    }
    assert_line(&stderr, r"  AI ✓ #\d+ SEO \d/3 · /page-1\.html · .*");
    assert_every_request_names_its_task(&stderr);
}

/// A valid `ia` report row (vLLM Qwen usage: 17 in, 37 out).
fn qwen_ia_answer() -> String {
    let mut response: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/ai-responses/vllm-qwen-think.json")).expect("JSON");
    let answer = serde_json::json!({
        "description": "A test page of the local sample site that holds a short heading and one paragraph of placeholder text; it plays the role of a simple content page within the site structure and links to nothing else of note.",
        "section": "Other",
        "pageType": "detail",
        "primaryEntity": null
    });
    response["choices"][0]["message"]["content"] = serde_json::json!(answer.to_string());
    response.to_string()
}

#[test]
fn ai_report_requests_carry_their_task_and_page() {
    let tmp = TempDir::new("ai-progress-report");
    let site = tmp.path.join("site");
    write_site(&site, 1);
    let server = LocalServer::start(&site);
    let mock = MockLlm::start(vec![chat_response(200, qwen_ia_answer())]);
    let report_dir = format!("--ai-report-dir={}", tmp.path.display());
    let stderr = crawl_with_ai(
        &server,
        &mock,
        &[
            "--ai-report=ia",
            "--ai-max-pages=2",
            "--ai-max-concurrency=1",
            &report_dir,
        ],
    );
    for n in 1..=2 {
        assert_line(&stderr, &format!(r"  AI ✓ #\d+ Report 'ia' {n}/2 · /\S* · 17 in · .*"));
    }
    assert_every_request_names_its_task(&stderr);
}

#[test]
fn ai_summary_requests_carry_their_task_and_area() {
    let tmp = TempDir::new("ai-progress-summary");
    let mock = MockLlm::start(vec![chat_response(200, qwen_seo_answer())]);
    let stderr = crawl_with_ai(&one_page_site(&tmp), &mock, &["--ai-actions=summary"]);
    for area in ["security", "accessibility", "seo", "performance", "infrastructure"] {
        assert_line(
            &stderr,
            &format!(r"  AI ✓ #\d+ Executive summary [1-5]/6 · {area} · .*"),
        );
    }
    assert_line(&stderr, r"  AI ✓ #6 Executive summary 6/6 · synthesis · .*");
    assert_every_request_names_its_task(&stderr);
}

/// An answer every brand-elaborate stage but the correction accepts: an IA map with no sections,
/// a page essence, and an `abstract` prose section.
fn elaborate_answer() -> String {
    let mut response: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/ai-responses/vllm-qwen-think.json")).expect("JSON");
    let answer = serde_json::json!({
        "page_role": "homepage",
        "essence": "Acme builds tools for builders.",
        "abstract": "Acme builds tools for builders."
    });
    response["choices"][0]["message"]["content"] = serde_json::json!(answer.to_string());
    response.to_string()
}

#[test]
fn ai_elaborate_requests_carry_their_stage() {
    let tmp = TempDir::new("ai-progress-elaborate");
    let site = tmp.path.join("site");
    // More than 40 candidate pages, so the model selects them.
    write_site(&site, 45);
    let server = LocalServer::start(&site);
    let mock = MockLlm::start(vec![chat_response(200, elaborate_answer())]);
    let report_dir = format!("--ai-report-dir={}", tmp.path.display());
    let stderr = crawl_with_ai(
        &server,
        &mock,
        &[
            "--ai-elaborate",
            "--ai-max-pages=3",
            "--ai-max-concurrency=1",
            "--ai-elaborate-gap-fill=0",
            &report_dir,
        ],
    );
    assert_line(&stderr, r"  AI ✓ #1 Elaborate: select 1/2 · round 1 · .*");
    for n in 1..=3 {
        assert_line(&stderr, &format!(r"  AI ✓ #\d+ Elaborate: extract {n}/3 · /\S* · .*"));
    }
    // One request per prose section, each named by its section.
    let synthesis =
        regex::Regex::new(r"(?m)^  AI ✓ #\d+ Elaborate: synthesis (\d+)/(\d+) · (\w+) · ").expect("a pattern");
    let sections: Vec<(usize, usize)> = synthesis
        .captures_iter(&stderr)
        .map(|c| (c[1].parse().expect("done"), c[2].parse().expect("total")))
        .collect();
    let total = sections.first().map(|(_, total)| *total).expect("synthesis requests");
    assert_eq!(
        sections,
        (1..=total).map(|n| (n, total)).collect::<Vec<_>>(),
        "{stderr}"
    );
    // The mock's answer is no list of edits: both correction attempts are reported.
    assert_eq!(
        stderr.matches("Elaborate: correction 1/1 · prose · ").count(),
        2,
        "stderr:\n{stderr}"
    );
    assert_every_request_names_its_task(&stderr);
}

/// An answer every AI profile stage accepts: a type pick, the page ids `[1]` (the first array in
/// it) and an empty list of edits.
fn profile_answer() -> String {
    let mut response: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/ai-responses/vllm-qwen-think.json")).expect("JSON");
    response["choices"][0]["message"]["content"] = serde_json::json!(r#"{"ids":[1],"type":1,"edits":[]}"#);
    response.to_string()
}

#[test]
fn ai_profile_requests_carry_their_stage() {
    let tmp = TempDir::new("ai-progress-profile");
    let site = tmp.path.join("site");
    write_site(&site, 2);
    let server = LocalServer::start(&site);
    let mock = MockLlm::start(vec![chat_response(200, profile_answer())]);
    let report_dir = format!("--ai-report-dir={}", tmp.path.display());
    let stderr = crawl_with_ai(
        &server,
        &mock,
        &[
            "--ai-profile",
            "--ai-max-pages=3",
            "--ai-max-concurrency=1",
            "--ai-report-language=cs",
            &report_dir,
        ],
    );
    assert_line(&stderr, r"  AI ✓ #1 Profile: summary 1/1 · 127\.0\.0\.1 · .*");
    assert_line(&stderr, r"  AI ✓ #2 Profile: classify 1/1 · 127\.0\.0\.1 · .*");
    for n in 1..=3 {
        assert_line(&stderr, &format!(r"  AI ✓ #\d+ Profile: describe {n}/3 · /\S* · .*"));
    }
    assert_line(&stderr, r"  AI ✓ #\d+ Profile: headings 1/1 · cs · .*");
    assert_line(&stderr, r"  AI ✓ #\d+ Profile: chapters \d+/\d+ · \S.* · .*");
    assert_line(
        &stderr,
        r"  AI ✓ #\d+ Profile: executive summary 1/1 · 127\.0\.0\.1 · .*",
    );
    assert_line(&stderr, r"  AI ✓ #\d+ Profile: correction 1/1 · executive summary · .*");
    assert_every_request_names_its_task(&stderr);
}

#[test]
fn ai_custom_and_llms_requests_carry_their_task_and_page() {
    let tmp = TempDir::new("ai-progress-custom-llms");
    let mock = MockLlm::start(vec![chat_response(200, qwen_seo_answer())]);
    // llms.txt is written next to the markdown export.
    let export_dir = format!("--markdown-export-dir={}", tmp.path.join("md").display());
    let (stderr, events) = crawl_with_ai_events(
        &one_page_site(&tmp),
        &mock,
        &tmp,
        &[
            "--ai-actions=custom,llms-txt,llms-full",
            "--ai-prompt=Check the page.",
            &export_dir,
        ],
    );
    assert_line(&stderr, r"  AI ✓ #\d+ Custom check 1/1 · / · 17 in · .*");
    assert_line(&stderr, r"  AI ✓ #\d+ llms\.txt 1/1 · / · 17 in · .*");
    assert_every_request_names_its_task(&stderr);
    // Both files are announced, each with its path.
    for (kind, name) in [("llms", ".llms.txt"), ("llms-full", ".llms-full.txt")] {
        let artifact = events_of(&events, "artifact")
            .into_iter()
            .find(|a| a["kind"] == kind)
            .unwrap_or_else(|| panic!("no {kind} artifact in {events:#?}"));
        let path = artifact["path"].as_str().expect("a path");
        assert!(
            path.ends_with(name) && std::path::Path::new(path).is_file(),
            "{artifact}"
        );
    }
}

// ---------------------------------------------------------------------------
// AI events in the NDJSON stream (`--events-file`)
// ---------------------------------------------------------------------------

/// Crawls like `crawl_with_ai`, with an event stream in `tmp`; returns stderr and the events.
fn crawl_with_ai_events(
    server: &LocalServer,
    mock: &MockLlm,
    tmp: &TempDir,
    extra: &[&str],
) -> (String, Vec<serde_json::Value>) {
    let events = tmp.path.join("events.ndjson");
    let events_arg = format!("--events-file={}", events.display());
    let report_dir = format!("--ai-report-dir={}", tmp.path.display());
    let mut args = extra.to_vec();
    args.extend([events_arg.as_str(), report_dir.as_str()]);
    let stderr = crawl_with_ai(server, mock, &args);
    let text = std::fs::read_to_string(&events).expect("the event file exists");
    let events = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("one JSON object per line"))
        .collect();
    (stderr, events)
}

fn events_of<'a>(events: &'a [serde_json::Value], kind: &str) -> Vec<&'a serde_json::Value> {
    events.iter().filter(|event| event["type"] == kind).collect()
}

/// The `aiProgress` events of `task`: started at 0, one `progress` per unit up to the total, then
/// finished at the total. Returns the total.
fn assert_progress_runs_to_the_end(events: &[serde_json::Value], task: &str) -> u64 {
    let states: Vec<(String, u64, u64)> = events_of(events, "aiProgress")
        .into_iter()
        .filter(|event| event["task"] == task)
        .map(|event| {
            (
                event["state"].as_str().expect("a state").to_string(),
                event["done"].as_u64().expect("done"),
                event["total"].as_u64().expect("total"),
            )
        })
        .collect();
    let total = states
        .first()
        .map(|(_, _, total)| *total)
        .unwrap_or_else(|| panic!("no progress of {task}"));
    let mut expected = vec![("started".to_string(), 0, total)];
    expected.extend((1..=total).map(|done| ("progress".to_string(), done, total)));
    expected.push(("finished".to_string(), total, total));
    assert_eq!(states, expected, "progress of {task}");
    total
}

/// Unknown values are left out, never written as null.
fn assert_no_nulls(events: &[serde_json::Value]) {
    for event in events {
        let object = event.as_object().expect("an object");
        assert!(object.values().all(|value| !value.is_null()), "a null in {event}");
    }
}

/// Concurrent requests are numbered in the order they are written, on stderr and in the stream.
fn assert_numbered_in_order(stderr: &str, events: &[serde_json::Value]) {
    let line = regex::Regex::new(r"(?m)^  AI [✓↻✗⇢] #(\d+) ").expect("a valid pattern");
    let printed: Vec<u64> = line
        .captures_iter(stderr)
        .map(|c| c[1].parse().expect("a number"))
        .collect();
    let emitted: Vec<u64> = events_of(events, "aiRequest")
        .iter()
        .map(|request| request["seq"].as_u64().expect("seq"))
        .collect();
    let expected: Vec<u64> = (1..=emitted.len() as u64).collect();
    assert_eq!(emitted, expected, "events");
    assert_eq!(printed, expected, "stderr");
}

fn sum_of(events: &[&serde_json::Value], field: &str) -> u64 {
    events.iter().filter_map(|event| event[field].as_u64()).sum()
}

#[test]
fn ai_events_report_requests_progress_usage_and_files() {
    let tmp = TempDir::new("ai-events");
    let site = tmp.path.join("site");
    write_site(&site, 1);
    let server = LocalServer::start(&site);
    // The SEO action asks first (one request per page), then the `ia` report.
    let mock = MockLlm::start(vec![
        chat_response(200, qwen_seo_answer()),
        chat_response(200, qwen_seo_answer()),
        chat_response(200, qwen_ia_answer()),
    ]);
    let (_, events) = crawl_with_ai_events(
        &server,
        &mock,
        &tmp,
        &[
            "--ai-actions=seo",
            "--ai-report=ia",
            "--ai-max-pages=2",
            "--ai-max-concurrency=1",
        ],
    );
    assert_no_nulls(&events);
    assert_eq!(assert_progress_runs_to_the_end(&events, "seo"), 2);
    assert_eq!(assert_progress_runs_to_the_end(&events, "report:ia"), 2);

    let requests = events_of(&events, "aiRequest");
    assert_eq!(requests.len(), 4, "{requests:#?}");
    let seqs: Vec<u64> = requests.iter().map(|r| r["seq"].as_u64().expect("seq")).collect();
    assert_eq!(seqs, vec![1, 2, 3, 4], "numbered in the order written");
    for (request, (task, label, done)) in requests.iter().zip([
        ("seo", "SEO", 0),
        ("seo", "SEO", 1),
        ("report:ia", "Report 'ia'", 0),
        ("report:ia", "Report 'ia'", 1),
    ]) {
        assert_eq!(request["task"], task);
        assert_eq!(request["label"], label);
        assert_eq!(
            (request["done"].as_u64(), request["total"].as_u64()),
            (Some(done), Some(2))
        );
        assert!(
            request["subject"].as_str().is_some_and(|s| s.starts_with('/')),
            "{request}"
        );
        assert_eq!(request["provider"], "openai-compatible");
        assert_eq!(request["model"], "m");
        assert_eq!(
            (request["attempt"].as_u64(), request["maxAttempts"].as_u64()),
            (Some(1), Some(3))
        );
        assert_eq!(request["outcome"], "ok");
        assert_eq!(request["status"], 200);
        // The captured vLLM Qwen response: 17 in, 37 out of which 33 reasoning, 148 reasoning chars.
        assert_eq!(request["inputTokens"], 17);
        assert_eq!(request["outputTokens"], 37);
        assert_eq!(request["reasoningTokens"], 33);
        assert_eq!(request["cachedInputTokens"], 0);
        assert_eq!(request["reasoningChars"], 148);
        assert_eq!(request["finishReason"], "stop");
        assert!(request["ms"].as_u64().is_some_and(|ms| ms >= 100), "{request}");
        assert!(
            request["outputTokensPerSecond"].as_f64().is_some_and(|v| v > 0.0),
            "{request}"
        );
        assert!(
            request["totalTokensPerSecond"].as_f64().is_some_and(|v| v > 0.0),
            "{request}"
        );
        assert!(request.get("error").is_none(), "{request}");
    }
    assert_eq!(requests[0]["category"], "SEO analysis");
    assert_eq!(requests[2]["category"], "AI report (extract)");

    let usage = events_of(&events, "aiUsage");
    assert_eq!(usage.len(), 1, "once per run");
    let usage = usage[0];
    assert_eq!(usage["provider"], "openai-compatible");
    assert_eq!(usage["model"], "m");
    assert_eq!(usage["calls"], 4);
    assert_eq!(usage["cacheHits"], 0);
    assert_eq!(usage["httpAttempts"], 4);
    assert_eq!(usage["retries"], 0);
    assert_eq!(usage["callsWithoutUsage"], 0);
    for (total, field) in [
        ("inputTokens", "inputTokens"),
        ("outputTokens", "outputTokens"),
        ("reasoningTokens", "reasoningTokens"),
        ("cachedInputTokens", "cachedInputTokens"),
    ] {
        assert_eq!(usage[total].as_u64(), Some(sum_of(&requests, field)), "{total}");
    }
    assert!(usage["networkMs"].as_u64().is_some_and(|ms| ms >= 400), "{usage}");
    let position = |event: &serde_json::Value| events.iter().position(|e| e == event).expect("present");
    assert!(position(usage) > position(requests[3]), "after the last request");

    let artifacts = events_of(&events, "artifact");
    for (kind, extension) in [("ai-report-json", ".json"), ("ai-report-html", ".html")] {
        let artifact = artifacts
            .iter()
            .find(|a| a["kind"] == kind)
            .unwrap_or_else(|| panic!("no {kind} in {artifacts:#?}"));
        let path = std::path::Path::new(artifact["path"].as_str().expect("a path"));
        assert!(path.is_absolute() && path.is_file(), "{artifact}");
        assert!(path.to_string_lossy().ends_with(extension), "{artifact}");
    }
}

#[test]
fn ai_events_report_retries_failures_and_missing_usage() {
    let tmp = TempDir::new("ai-events-failures");
    let site = tmp.path.join("site");
    write_site(&site, 1);
    let server = LocalServer::start(&site);
    let no_usage =
        serde_json::json!({"id": "x", "choices": [{"message": {"content": r#"{"scores":{"overall":80}}"#}}]});
    // First page: 429, then an answer without usage. Second page: an unknown model.
    let mock = MockLlm::start(vec![
        chat_response(429, r#"{"error":{"message":"slow down"}}"#.to_string()),
        chat_response(200, no_usage.to_string()),
        chat_response(
            404,
            include_str!("fixtures/ai-responses/error-vllm-unknown-model.json").to_string(),
        ),
    ]);
    let (_, events) = crawl_with_ai_events(
        &server,
        &mock,
        &tmp,
        &["--ai-actions=seo", "--ai-max-pages=2", "--ai-max-concurrency=1"],
    );
    assert_no_nulls(&events);
    // The failed page counts too.
    assert_eq!(assert_progress_runs_to_the_end(&events, "seo"), 2);

    let requests = events_of(&events, "aiRequest");
    let outcomes: Vec<(&str, u64, u64)> = requests
        .iter()
        .map(|r| {
            (
                r["outcome"].as_str().expect("an outcome"),
                r["status"].as_u64().expect("a status"),
                r["attempt"].as_u64().expect("an attempt"),
            )
        })
        .collect();
    assert_eq!(outcomes, vec![("retry", 429, 1), ("ok", 200, 2), ("error", 404, 1)]);
    assert_eq!(requests[0]["error"], "HTTP 429");
    for field in ["inputTokens", "outputTokens", "reasoningTokens", "cachedInputTokens"] {
        assert!(
            requests[1].get(field).is_none(),
            "no usage, no {field}: {}",
            requests[1]
        );
    }
    assert!(
        requests[2]["error"]
            .as_str()
            .is_some_and(|e| e.contains("The model `no-such-model` does not exist")),
        "{}",
        requests[2]
    );

    let usage = events_of(&events, "aiUsage")[0];
    assert_eq!(usage["calls"], 1);
    assert_eq!(usage["httpAttempts"], 3);
    assert_eq!(usage["retries"], 1);
    assert_eq!(usage["callsWithoutUsage"], 1);
    assert_eq!(usage["inputTokens"], 0);
}

#[test]
fn ai_events_report_a_skipped_ai_phase_as_an_issue() {
    let tmp = TempDir::new("ai-events-no-key");
    let events = tmp.path.join("events.ndjson");
    let server = one_page_site(&tmp);
    let output = run_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--ai-provider=openai",
        "--ai-model=m",
        "--ai-api-key-env=SITEONE_TEST_NO_SUCH_KEY",
        "--ai-actions=seo",
        &format!("--events-file={}", events.display()),
    ]);
    assert_eq!(output.status.code(), Some(0), "the AI phase is fail-soft");
    let text = std::fs::read_to_string(&events).expect("the event file exists");
    let issue = text
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSON"))
        .find(|event| event["type"] == "issue" && event["kind"] == "ai")
        .unwrap_or_else(|| panic!("no AI issue in:\n{text}"));
    assert_eq!(issue["label"], "AI phase skipped");
    assert!(
        issue["detail"].as_str().is_some_and(|d| d.contains("no API key")),
        "{issue}"
    );
}

#[test]
fn ai_events_follow_the_executive_summary() {
    let tmp = TempDir::new("ai-events-summary");
    let mock = MockLlm::start(vec![chat_response(200, qwen_seo_answer())]);
    let (_, events) = crawl_with_ai_events(&one_page_site(&tmp), &mock, &tmp, &["--ai-actions=summary"]);
    assert_eq!(assert_progress_runs_to_the_end(&events, "summary"), 6);
    let requests = events_of(&events, "aiRequest");
    assert_eq!(requests.len(), 6);
    assert_eq!(requests[5]["subject"], "synthesis");
    // The summary runs after the `ai` phase; the totals still include it.
    let usage = events_of(&events, "aiUsage");
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0]["calls"], 6);
    assert_eq!(usage[0]["inputTokens"].as_u64(), Some(sum_of(&requests, "inputTokens")));
}

#[test]
fn ai_events_follow_every_brand_elaborate_stage() {
    let tmp = TempDir::new("ai-events-elaborate");
    let site = tmp.path.join("site");
    write_site(&site, 45);
    let server = LocalServer::start(&site);
    let mock = MockLlm::start(vec![chat_response(200, elaborate_answer())]);
    let (_, events) = crawl_with_ai_events(
        &server,
        &mock,
        &tmp,
        &["--ai-elaborate", "--ai-max-pages=3", "--ai-elaborate-gap-fill=0"],
    );
    assert_no_nulls(&events);
    // Round 2 is not needed and counts as done; the correction fails and counts as done.
    assert_eq!(assert_progress_runs_to_the_end(&events, "elaborate:select"), 2);
    assert_eq!(assert_progress_runs_to_the_end(&events, "elaborate:extract"), 3);
    assert!(assert_progress_runs_to_the_end(&events, "elaborate:synthesize") >= 1);
    assert_eq!(assert_progress_runs_to_the_end(&events, "elaborate:correct"), 1);
    for request in events_of(&events, "aiRequest") {
        assert!(
            request["task"].as_str().is_some_and(|t| t.starts_with("elaborate:")) && request["subject"].is_string(),
            "{request}"
        );
    }
    for kind in ["ai-elaborate-md", "ai-elaborate-json", "ai-elaborate-html"] {
        let artifact = events_of(&events, "artifact")
            .into_iter()
            .find(|a| a["kind"] == kind)
            .unwrap_or_else(|| panic!("no {kind}"));
        assert!(
            std::path::Path::new(artifact["path"].as_str().expect("a path")).is_file(),
            "{artifact}"
        );
    }
}

#[test]
fn ai_events_follow_every_ai_profile_stage() {
    let tmp = TempDir::new("ai-events-profile");
    let site = tmp.path.join("site");
    write_site(&site, 2);
    let server = LocalServer::start(&site);
    let mock = MockLlm::start(vec![chat_response(200, profile_answer())]);
    let (stderr, events) = crawl_with_ai_events(
        &server,
        &mock,
        &tmp,
        &["--ai-profile", "--ai-max-pages=3", "--ai-report-language=cs"],
    );
    assert_no_nulls(&events);
    assert_numbered_in_order(&stderr, &events);
    for task in [
        "profile:summary",
        "profile:classify",
        "profile:localize",
        "profile:executive",
        "profile:correct",
    ] {
        assert_eq!(assert_progress_runs_to_the_end(&events, task), 1, "{task}");
    }
    assert_eq!(assert_progress_runs_to_the_end(&events, "profile:describe"), 3);
    assert!(assert_progress_runs_to_the_end(&events, "profile:chapters") > 1);
    for request in events_of(&events, "aiRequest") {
        assert!(
            request["task"].as_str().is_some_and(|t| t.starts_with("profile:")) && request["subject"].is_string(),
            "{request}"
        );
    }
    for kind in ["ai-profile-md", "ai-profile-json", "ai-profile-html"] {
        let artifact = events_of(&events, "artifact")
            .into_iter()
            .find(|a| a["kind"] == kind)
            .unwrap_or_else(|| panic!("no {kind}"));
        assert!(
            std::path::Path::new(artifact["path"].as_str().expect("a path")).is_file(),
            "{artifact}"
        );
    }
}

#[test]
#[cfg(unix)]
fn ai_events_report_failed_pipelines_as_issues() {
    let tmp = TempDir::new("ai-events-all-fail");
    let site = tmp.path.join("site");
    write_site(&site, 1);
    let server = LocalServer::start(&site);
    // Every request fails at once (a 404 is not retried).
    let mock = MockLlm::start(vec![chat_response(
        404,
        include_str!("fixtures/ai-responses/error-vllm-unknown-model.json").to_string(),
    )]);
    let events_path = tmp.path.join("events.ndjson");
    // A directory nothing can be written to: the AI report export fails.
    let read_only = tmp.path.join("read-only");
    std::fs::create_dir(&read_only).expect("a directory");
    std::fs::set_permissions(&read_only, std::os::unix::fs::PermissionsExt::from_mode(0o555)).expect("read-only");
    let output = run_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        LOCAL_ANALYZERS,
        "--http-cache-dir=",
        "--ai-provider=openai-compatible",
        &format!("--ai-endpoint={}", mock.url()),
        "--ai-model=m",
        "--ai-cache-dir=",
        "--ai-actions=summary",
        "--ai-report=ia",
        "--ai-elaborate",
        "--ai-profile",
        "--ai-max-pages=2",
        &format!("--ai-report-dir={}", read_only.display()),
        &format!("--events-file={}", events_path.display()),
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "the AI features are fail-soft: {stderr}");
    let text = std::fs::read_to_string(&events_path).expect("the event file exists");
    let events: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("one JSON object per line"))
        .collect();
    assert_no_nulls(&events);

    let issues: Vec<&str> = events_of(&events, "issue")
        .into_iter()
        .filter(|issue| issue["kind"] == "ai")
        .map(|issue| issue["label"].as_str().expect("a label"))
        .collect();
    for label in [
        "AI executive summary failed",
        "Brand elaborate failed",
        "AI profile failed",
        "AI report export failed",
    ] {
        assert!(issues.contains(&label), "no {label:?} in {issues:?}\n{stderr}");
    }
    // Failed units count: every task that started ran to its end.
    let tasks: std::collections::BTreeSet<&str> = events_of(&events, "aiProgress")
        .into_iter()
        .map(|event| event["task"].as_str().expect("a task"))
        .collect();
    for task in [
        "report:ia",
        "summary",
        "elaborate:extract",
        "profile:describe",
        "profile:chapters",
    ] {
        assert!(tasks.contains(task), "no progress of {task} in {tasks:?}");
    }
    for task in tasks {
        // The summary stops before its synthesis when every area failed.
        if task != "summary" {
            assert_progress_runs_to_the_end(&events, task);
        }
    }
    for request in events_of(&events, "aiRequest") {
        assert_eq!(request["outcome"], "error", "{request}");
        assert_eq!(request["status"], 404, "{request}");
    }
}

#[test]
fn ai_events_follow_the_elaborate_gap_fill() {
    let tmp = TempDir::new("ai-events-gap-fill");
    let site = tmp.path.join("site");
    std::fs::create_dir_all(&site).expect("site dir");
    // A navigation the single-page crawl never follows; one of its pages does not exist.
    std::fs::write(
        site.join("index.html"),
        r#"<html><head><title>Acme</title></head><body><nav><a href="/about.html">About</a> <a href="/missing.html">Missing</a></nav><p>Acme builds tools.</p></body></html>"#,
    )
    .expect("index.html");
    std::fs::write(
        site.join("about.html"),
        "<html><head><title>About</title></head><body><p>Founded in 1999.</p></body></html>",
    )
    .expect("about.html");
    let server = LocalServer::start(&site);
    let mock = MockLlm::start(vec![chat_response(200, elaborate_answer())]);
    let (_, events) = crawl_with_ai_events(
        &server,
        &mock,
        &tmp,
        &["--single-page", "--ai-elaborate", "--ai-elaborate-correct=false"],
    );
    // The page that could not be fetched counts as done too.
    assert_eq!(assert_progress_runs_to_the_end(&events, "elaborate:gapfill"), 2);
    // The fetched page joins the extraction.
    assert_eq!(assert_progress_runs_to_the_end(&events, "elaborate:extract"), 2);
}

// ---------------------------------------------------------------------------
// AI utility modes `--ai-list-models` and `--ai-check` (offline, against `MockLlm`)
// ---------------------------------------------------------------------------

/// Runs a utility mode with `args`; returns its exit code, the one JSON object it printed on
/// stdout, and stderr.
fn run_ai_tool(args: &[&str]) -> (Option<i32>, serde_json::Value, String) {
    let mut all = vec!["--config-file=/dev/null", "--no-color"];
    all.extend(args);
    let output = run_crawler(&all);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(
        stdout.lines().count(),
        1,
        "one line on stdout: {stdout:?}\nstderr: {stderr}"
    );
    let answer: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("not JSON ({e}): {stdout:?}"));
    assert!(answer.is_object(), "{answer}");
    (output.status.code(), answer, stderr)
}

fn models_response(path_prefix: &'static str, status: u16, body: &str) -> MockResponse {
    MockResponse {
        path_prefix,
        status,
        body: body.to_string(),
        delay_ms: 0,
    }
}

#[test]
fn ai_list_models_prints_the_models_of_every_provider_shape() {
    let cases = [
        (
            "openai-compatible",
            "/v1",
            "/v1/models",
            "/v1/models",
            include_str!("fixtures/ai-responses/models-vllm.json"),
            "authorization: bearer sk-test-key",
        ),
        (
            "anthropic",
            "",
            "/v1/models",
            "/v1/models?limit=1000",
            include_str!("fixtures/ai-responses/models-anthropic.json"),
            "x-api-key: sk-test-key",
        ),
        (
            "gemini",
            "/v1beta",
            "/v1beta/models",
            "/v1beta/models?pageSize=1000",
            include_str!("fixtures/ai-responses/models-gemini.json"),
            "x-goog-api-key: sk-test-key",
        ),
    ];
    for (provider, base, prefix, path, body, key_header) in cases {
        let mock = MockLlm::start(vec![models_response(prefix, 200, body)]);
        let endpoint = format!("http://127.0.0.1:{}{base}", mock.port());
        let (code, answer, stderr) = run_ai_tool(&[
            &format!("--ai-provider={provider}"),
            &format!("--ai-endpoint={endpoint}"),
            "--ai-api-key=sk-test-key",
            "--ai-list-models",
        ]);
        assert_eq!(code, Some(0), "{provider}: {stderr}");
        assert_eq!(answer["ok"], true, "{provider}: {answer}");
        assert_eq!(answer["provider"], provider);
        assert_eq!(answer["endpoint"], endpoint);
        let heads = mock.request_heads();
        assert_eq!(heads.len(), 1, "{provider}: one request");
        let head = heads[0].to_ascii_lowercase();
        assert!(
            head.starts_with(&format!("get {} http/1.1\r\n", path.to_ascii_lowercase())),
            "{provider}: {head}"
        );
        assert!(head.contains(key_header), "{provider}: {head}");
        assert!(
            !head.lines().next().unwrap_or_default().contains("sk-test-key"),
            "{provider}: the key is never in the URL"
        );
        assert!(
            !answer.to_string().contains("sk-test-key") && !stderr.contains("sk-test-key"),
            "{provider}: the key is never printed"
        );
        let models = answer["models"].as_array().expect("a model list");
        let ids: Vec<&str> = models.iter().filter_map(|m| m["id"].as_str()).collect();
        match provider {
            "openai-compatible" => {
                assert_eq!(
                    ids,
                    [
                        "deepseek-ai/DeepSeek-V4-Flash-0731",
                        "deepseek-ai/DeepSeek-V4-Flash-Vision-Exp",
                        "nvidia/Qwen3.8-Flash-Next-NVFP4"
                    ]
                );
                assert_eq!(
                    models[2],
                    serde_json::json!({"id": "nvidia/Qwen3.8-Flash-Next-NVFP4", "displayName": null,
                        "contextWindow": 262144, "maxOutputTokens": null})
                );
            }
            "anthropic" => {
                assert_eq!(ids, ["claude-fable-5-1", "claude-opus-5", "claude-opus-5-5"]);
                assert_eq!(
                    models[2],
                    serde_json::json!({"id": "claude-opus-5-5", "displayName": "Claude Opus 5.5",
                        "contextWindow": 1000000, "maxOutputTokens": 128000})
                );
            }
            _ => {
                assert_eq!(
                    ids,
                    ["gemini-2.5-flash", "gemini-2.5-flash-preview-tts", "gemini-2.5-pro"]
                );
                assert_eq!(
                    models[0],
                    serde_json::json!({"id": "gemini-2.5-flash", "displayName": "Gemini 2.5 Flash",
                        "contextWindow": 1048576, "maxOutputTokens": 65536})
                );
            }
        }
    }
}

#[test]
fn ai_list_models_failures_are_one_json_object_without_the_key() {
    // A provider that echoes the key it rejected.
    let rejected = r#"{"error":{"message":"Incorrect API key provided: secret-xyz"}}"#;
    let cases = [
        (401, rejected, "HTTP 401: Incorrect API key provided: [redacted]"),
        (502, "", "HTTP 502"),
        (
            200,
            "<html>gateway</html>",
            "The endpoint's answer is not JSON (body starts: <html>gateway</html>)",
        ),
        (
            200,
            r#"{"object":"list"}"#,
            r#"The endpoint's answer is not a model list (body starts: {"object":"list"})"#,
        ),
        (
            200,
            rejected,
            "AI provider error: Incorrect API key provided: [redacted]",
        ),
    ];
    for (status, body, error) in cases {
        let mock = MockLlm::start(vec![models_response("/v1/models", status, body)]);
        let (code, answer, stderr) = run_ai_tool(&[
            "--ai-provider=openai-compatible",
            &format!("--ai-endpoint={}", mock.url()),
            "--ai-api-key=secret-xyz",
            "--ai-list-models",
        ]);
        assert_eq!(code, Some(1), "{status} {body}: {stderr}");
        assert_eq!(answer, serde_json::json!({"ok": false, "error": error}));
        assert!(stderr.contains(error), "the error on stderr too: {stderr}");
        assert!(!stderr.contains("secret-xyz"), "{stderr}");
    }

    // Nothing listens there.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("a free port")
        .port();
    let (code, answer, _) = run_ai_tool(&[
        "--ai-provider=openai-compatible",
        &format!("--ai-endpoint=http://user:PW_SENTINEL@127.0.0.1:{port}/v1"),
        "--ai-list-models",
    ]);
    assert_eq!(code, Some(1));
    let error = answer["error"].as_str().expect("an error");
    assert!(error.starts_with("AI request error: error sending request"), "{error}");
    assert!(!answer.to_string().contains("PW_SENTINEL"), "{answer}");
}

#[test]
fn ai_check_prints_the_statistics_of_one_request() {
    let mock = MockLlm::start(vec![chat_response(
        200,
        include_str!("fixtures/ai-responses/vllm-qwen-think.json").to_string(),
    )]);
    let (code, answer, stderr) = run_ai_tool(&[
        "--ai-provider=openai-compatible",
        &format!("--ai-endpoint={}", mock.url()),
        "--ai-model=qwen3.8",
        "--ai-check",
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    let ms = answer["ms"].as_u64().expect("the request time");
    assert!(ms >= 100, "the mock answers after 0.1 s: {ms}");
    let speed = (37.0 * 1000.0 / ms as f64 * 10.0).round() / 10.0;
    assert_eq!(
        answer,
        serde_json::json!({"ok": true, "provider": "openai-compatible", "model": "qwen3.8", "ms": ms,
            "inputTokens": 17, "outputTokens": 37, "reasoningTokens": 33, "cachedInputTokens": 0,
            "outputTokensPerSecond": speed, "finishReason": "stop", "reply": "OK"})
    );
    assert_line(
        &stderr,
        r"  AI ✓ #1 Connection check · 17 in · 37 out \(33 reasoning\) · \d+\.\d s · \d+ tok/s",
    );
    let bodies = mock.request_bodies();
    assert_eq!(bodies.len(), 1, "one request");
    let body: serde_json::Value = serde_json::from_str(&bodies[0]).expect("a JSON request");
    assert_eq!(body["model"], "qwen3.8");
    assert_eq!(
        body["messages"],
        serde_json::json!([{"role": "user", "content": "Reply with the single word OK."}])
    );
}

#[test]
fn ai_check_reports_what_the_provider_did_not_say_by_leaving_it_out() {
    // MiniMax reasons inline and reports no reasoning count; this answer has no finish reason.
    let mut response: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/ai-responses/minimax.json")).expect("JSON");
    response["choices"][0]
        .as_object_mut()
        .expect("a choice")
        .remove("finish_reason");
    let mock = MockLlm::start(vec![chat_response(200, response.to_string())]);
    let (code, answer, stderr) = run_ai_tool(&[
        "--ai-provider=openai-compatible",
        &format!("--ai-endpoint={}", mock.url()),
        "--ai-model=MiniMax-M3",
        "--ai-check",
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(answer["inputTokens"], 183);
    assert_eq!(answer["cachedInputTokens"], 128);
    for field in ["reasoningTokens", "finishReason"] {
        assert!(answer.get(field).is_none(), "no {field}: {answer}");
    }
    assert_eq!(answer["reply"], "OK", "the inline reasoning is not the reply");
    assert_line(
        &stderr,
        r"  AI ✓ #1 Connection check · 183 in · 30 out \(reasoning n/a\) · \d+\.\d s · \d+ tok/s",
    );
}

#[test]
fn ai_check_never_uses_the_ai_cache() {
    let tmp = TempDir::new("ai-check-cache");
    let cache = tmp.path.join("ai-cache");
    let mock = MockLlm::start(vec![chat_response(
        200,
        include_str!("fixtures/ai-responses/vllm-qwen-think.json").to_string(),
    )]);
    for _ in 0..2 {
        let (code, answer, stderr) = run_ai_tool(&[
            "--ai-provider=openai-compatible",
            &format!("--ai-endpoint={}", mock.url()),
            "--ai-model=m",
            &format!("--ai-cache-dir={}", cache.display()),
            "--ai-check",
        ]);
        assert_eq!(code, Some(0), "{stderr}");
        assert_eq!(answer["ok"], true);
    }
    assert_eq!(mock.request_bodies().len(), 2, "every check asks the model");
    // The option layer creates the directory; nothing may be written into it.
    let entries = std::fs::read_dir(&cache).map(|dir| dir.count()).unwrap_or(0);
    assert_eq!(entries, 0, "nothing is cached");
}

#[test]
fn ai_check_failures_are_one_json_object_without_the_key() {
    let mock = MockLlm::start(vec![chat_response(
        401,
        r#"{"error":{"message":"Incorrect API key provided: secret-xyz"}}"#.to_string(),
    )]);
    let (code, answer, stderr) = run_ai_tool(&[
        "--ai-provider=openai-compatible",
        &format!("--ai-endpoint={}", mock.url()),
        "--ai-model=m",
        "--ai-api-key=secret-xyz",
        "--ai-check",
    ]);
    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(
        answer,
        serde_json::json!({"ok": false,
            "error": "AI provider error: Incorrect API key provided: [redacted]"})
    );
    assert_line(
        &stderr,
        r"  AI ✗ #1 Connection check · AI provider error: Incorrect API key provided: \[redacted\] · \d+\.\d s",
    );
    assert!(!stderr.contains("secret-xyz"), "{stderr}");

    let mock = MockLlm::start(vec![chat_response(
        404,
        include_str!("fixtures/ai-responses/error-vllm-unknown-model.json").to_string(),
    )]);
    let (code, answer, _) = run_ai_tool(&[
        "--ai-provider=openai-compatible",
        &format!("--ai-endpoint={}", mock.url()),
        "--ai-model=no-such-model",
        "--ai-check",
    ]);
    assert_eq!(code, Some(1));
    assert_eq!(
        answer["error"],
        "AI provider error: The model `no-such-model` does not exist."
    );
}

#[test]
fn ai_utility_modes_never_print_credentials() {
    // Everything the provider controls repeats the credentials: model ids, names, the reply, the
    // finish reason and the error message.
    let echoed = format!("{KEY_SENTINEL} {PW_SENTINEL} {QUERY_SENTINEL}");
    let models = serde_json::json!({"data": [{"id": format!("model-{echoed}"), "display_name": echoed}]});
    let reply = serde_json::json!({
        "choices": [{"message": {"content": format!("OK {echoed}")}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 11, "completion_tokens": 7}
    });
    let odd_finish = serde_json::json!({
        "choices": [{"message": {"content": "OK"}, "finish_reason": KEY_SENTINEL}],
    });
    let rejected = serde_json::json!({"error": {"message": format!(
        "Proxy rejected http://review:{PW_SENTINEL}@proxy.test/v1?token={QUERY_SENTINEL} for {KEY_SENTINEL}"
    )}});
    let cases = [
        ("list", "--ai-list-models", 200, models.to_string(), 0),
        ("list-rejected", "--ai-list-models", 401, rejected.to_string(), 1),
        ("check", "--ai-check", 200, reply.to_string(), 0),
        ("check-finish-reason", "--ai-check", 200, odd_finish.to_string(), 1),
        ("check-rejected", "--ai-check", 401, rejected.to_string(), 1),
    ];
    let mut leaks = Vec::new();
    // Userinfo as usual, and userinfo that does not decode (reqwest leaves it in the URL).
    for user in ["review", "%FFreview"] {
        for (name, mode, status, body, exit_code) in &cases {
            let name = format!("{name} ({user})");
            let mock = MockLlm::start(vec![MockResponse {
                path_prefix: "/v1",
                status: *status,
                body: body.clone(),
                delay_ms: 0,
            }]);
            let endpoint = format!(
                "--ai-endpoint=http://{user}:{PW_SENTINEL}@127.0.0.1:{}/v1?token={QUERY_SENTINEL}",
                mock.port()
            );
            let output = run_crawler(&[
                "--config-file=/dev/null",
                "--no-color",
                "--ai-provider=openai-compatible",
                &endpoint,
                "--ai-model=m",
                &format!("--ai-api-key={KEY_SENTINEL}"),
                mode,
            ]);
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            assert_eq!(output.status.code(), Some(*exit_code), "{name}: {stdout}\n{stderr}");
            assert_eq!(mock.request_heads().len(), 1, "{name}: one request");
            leaks.extend(credential_leaks(
                &name,
                &[("stdout".to_string(), stdout), ("stderr".to_string(), stderr)],
            ));
        }

        // An endpoint the options reject is quoted in the configuration error.
        let output = run_crawler(&[
            "--config-file=/dev/null",
            "--no-color",
            "--ai-provider=openai-compatible",
            &format!("--ai-endpoint=http://{user}:{PW_SENTINEL}@127.0.0.1:BAD/v1"),
            "--ai-model=m",
            "--ai-check",
        ]);
        assert_eq!(output.status.code(), Some(101));
        leaks.extend(credential_leaks(
            &format!("invalid endpoint ({user})"),
            &[
                (
                    "stdout".to_string(),
                    String::from_utf8_lossy(&output.stdout).into_owned(),
                ),
                (
                    "stderr".to_string(),
                    String::from_utf8_lossy(&output.stderr).into_owned(),
                ),
            ],
        ));
    }
    assert!(leaks.is_empty(), "{}", leaks.join("\n"));

    // A short key echoed by the provider is not blanked (the blanks would spell it out): the
    // message is withheld.
    let mock = MockLlm::start(vec![MockResponse {
        path_prefix: "/v1",
        status: 401,
        body: r#"{"error":{"message":"Incorrect API key provided: k3y"}}"#.to_string(),
        delay_ms: 0,
    }]);
    let (code, answer, stderr) = run_ai_tool(&[
        "--ai-provider=openai-compatible",
        &format!("--ai-endpoint={}", mock.url()),
        "--ai-model=m",
        "--ai-api-key=k3y",
        "--ai-check",
    ]);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(
        !answer.to_string().contains("k3y") && !stderr.contains("k3y"),
        "{answer}\n{stderr}"
    );
    assert!(
        answer["error"].as_str().is_some_and(|error| error.contains("withheld")),
        "{answer}"
    );
}

#[test]
fn ai_utility_modes_never_quote_the_query_of_an_invalid_endpoint() {
    // No userinfo: the query alone carries the key, also when no key is configured.
    let endpoint = format!("--ai-endpoint=http://127.0.0.1:BAD/v1?api_key={KEY_SENTINEL}");
    let key = format!("--ai-api-key={KEY_SENTINEL}");
    let mut leaks = Vec::new();
    for mode in ["--ai-check", "--ai-list-models"] {
        for with_key in [false, true] {
            let name = format!("{mode} (key configured: {with_key})");
            let mut args = vec!["--ai-provider=openai-compatible", &endpoint, "--ai-model=m", mode];
            if with_key {
                args.push(&key);
            }
            let (code, answer, stderr) = run_ai_tool(&args);
            assert_eq!(code, Some(101), "{name}: {answer}\n{stderr}");
            assert_eq!(answer["ok"], false, "{name}: {answer}");
            assert!(
                answer["error"]
                    .as_str()
                    .is_some_and(|error| error.contains("--ai-endpoint")),
                "{name}: {answer}"
            );
            leaks.extend(credential_leaks(
                &name,
                &[
                    ("stdout".to_string(), answer.to_string()),
                    ("stderr".to_string(), stderr),
                ],
            ));
        }
    }
    assert!(leaks.is_empty(), "{}", leaks.join("\n"));
}

#[test]
fn ai_utility_mode_errors_are_json_also_from_a_config_file_or_a_malformed_flag() {
    let tmp = TempDir::new("ai-utility-config");
    let config = tmp.path.join("crawler.conf");
    std::fs::write(
        &config,
        "--ai-check\n--ai-provider=openai-compatible\n--ai-endpoint=http://127.0.0.1:9/v1\n",
    )
    .expect("a config file");
    let config_arg = format!("--config-file={}", config.display());
    let connection = [
        "--config-file=/dev/null",
        "--ai-provider=openai-compatible",
        "--ai-endpoint=http://127.0.0.1:9/v1",
        "--ai-model=m",
    ];
    let cases: [(Vec<&str>, &str); 3] = [
        (vec![&config_arg], "AI is enabled but --ai-model is missing."),
        (
            [&connection[..], &["--ai-check=wat"]].concat(),
            "Option --ai-check (wat) must be boolean",
        ),
        (
            [&connection[..], &["--ai-list-models=wat"]].concat(),
            "Option --ai-list-models (wat) must be boolean",
        ),
    ];
    let mut wrong = Vec::new();
    for (mut args, message) in cases {
        args.push("--no-color");
        let output = run_crawler(&args);
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let answer = serde_json::from_str::<serde_json::Value>(&stdout).ok();
        let is_json_failure = stdout.lines().count() == 1
            && answer.as_ref().is_some_and(|answer| {
                answer["ok"] == false && answer["error"].as_str().is_some_and(|error| error.contains(message))
            });
        if output.status.code() != Some(101) || !is_json_failure {
            wrong.push(format!(
                "{args:?}: exit {:?}, {} stdout line(s) starting {:?}",
                output.status.code(),
                stdout.lines().count(),
                stdout.chars().take(80).collect::<String>()
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn ai_utility_modes_report_configuration_errors_as_json() {
    let cases: [(&[&str], &str); 6] = [
        (&["--ai-list-models"], "--ai-list-models requires --ai-provider"),
        (&["--ai-check"], "--ai-check requires --ai-provider"),
        (
            &["--ai-provider=openai-compatible", "--ai-list-models"],
            "--ai-provider=openai-compatible requires --ai-endpoint=URL.",
        ),
        (
            &[
                "--ai-provider=openai-compatible",
                "--ai-endpoint=http://127.0.0.1:9/v1",
                "--ai-check",
            ],
            "AI is enabled but --ai-model is missing.",
        ),
        (
            &["--ai-provider=nope", "--ai-list-models"],
            "Invalid --ai-provider 'nope'",
        ),
        (
            &[
                "--ai-provider=openai-compatible",
                "--ai-endpoint=http://127.0.0.1:9/v1",
                "--ai-model=m",
                "--ai-list-models",
                "--ai-check",
            ],
            "--ai-list-models and --ai-check cannot be combined",
        ),
    ];
    for (args, message) in cases {
        let (code, answer, stderr) = run_ai_tool(args);
        assert_eq!(code, Some(101), "{args:?}: {stderr}");
        assert_eq!(answer["ok"], false, "{args:?}: {answer}");
        assert!(
            answer["error"].as_str().is_some_and(|error| error.contains(message)),
            "{args:?}: {answer}"
        );
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    }

    // A key that does not resolve fails the run, like the AI phase of a crawl.
    let (code, answer, _) = run_ai_tool(&[
        "--ai-provider=anthropic",
        "--ai-api-key-env=SITEONE_TEST_NO_SUCH_KEY",
        "--ai-list-models",
    ]);
    assert_eq!(code, Some(1));
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| error.contains("no API key resolved for provider 'anthropic'")),
        "{answer}"
    );
}
