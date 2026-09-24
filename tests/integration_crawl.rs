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
// ---------------------------------------------------------------------------

/// Hosts driving the crawler rely on this instead of parsing the human-readable output, so
/// the shape of the stream is part of the contract.
#[test]
fn events_file_records_the_whole_run() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = TempDir::new("events");
    let events = tmp.path.join("events.ndjson");
    let report = tmp.path.join("report.html");

    let output = run_crawler(&[
        "--url=https://crawler.siteone.io/",
        "--workers=2",
        "--max-reqs-per-sec=3",
        "--max-visited-urls=5",
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
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = TempDir::new("no-events");
    let report = tmp.path.join("report.html");

    let output = run_crawler(&[
        "--url=https://crawler.siteone.io/",
        "--single-page",
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

    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = TempDir::new("control-stdin");
    let events = tmp.path.join("events.ndjson");
    let report = tmp.path.join("report.html");

    let mut child = Command::new(env!("CARGO_BIN_EXE_siteone-crawler"))
        .args([
            "--config-file=/dev/null",
            "--url=https://crawler.siteone.io/",
            "--workers=2",
            "--max-reqs-per-sec=3",
            "--max-visited-urls=500",
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

    // Let a few pages come back, then ask it to stop.
    std::thread::sleep(std::time::Duration::from_secs(2));
    let mut stdin = child.stdin.take().expect("stdin is piped");
    writeln!(stdin, "stop").expect("stop is accepted");
    stdin.flush().ok();
    // Keep the handle open: end-of-input means stop too, and we are testing the command.
    let status = child.wait().expect("crawler exits");
    drop(stdin);

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
/// the server sent caching headers. Runs against the built-in server, so it needs no network.
#[test]
fn events_mark_only_local_http_cache_hits_as_cached() {
    use std::process::{Child, Command, Stdio};

    struct KillOnDrop(Child);
    impl Drop for KillOnDrop {
        fn drop(&mut self) {
            self.0.kill().ok();
            self.0.wait().ok();
        }
    }

    let tmp = TempDir::new("events-cache");
    let site = tmp.path.join("site");
    std::fs::create_dir_all(&site).expect("site dir");
    std::fs::write(
        site.join("index.html"),
        "<html><head><title>Cache</title></head><body><p>Hello</p></body></html>",
    )
    .expect("index.html");

    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("a free port")
        .port();
    let _server = KillOnDrop(
        Command::new(env!("CARGO_BIN_EXE_siteone-crawler"))
            .args([
                format!("--serve-offline={}", site.display()),
                format!("--serve-port={port}"),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the built-in server starts"),
    );
    assert!(
        (0..50).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(100));
            std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
        }),
        "the built-in server accepts connections"
    );

    let cache = tmp.path.join("cache");
    let crawl = |name: &str| -> Vec<serde_json::Value> {
        let events = tmp.path.join(format!("{name}.ndjson"));
        let output = Command::new(env!("CARGO_BIN_EXE_siteone-crawler"))
            .args([
                "--config-file=/dev/null".to_string(),
                format!("--url=http://127.0.0.1:{port}/"),
                "--single-page".to_string(),
                // Skips the DNS analyzer, whose lookups of an IP-literal host can sit out resolver timeouts.
                "--analyzer-filter-regex=/Headers/".to_string(),
                format!("--http-cache-dir={}", cache.display()),
                format!("--events-file={}", events.display()),
                format!(
                    "--output-html-report={}",
                    tmp.path.join(format!("{name}.html")).display()
                ),
            ])
            .output()
            .expect("the crawler runs");
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
