// Offline integration tests for the analysis & report track (#19, #22). They crawl a tiny site served
// by the built-in `--serve-offline` server, so no network is needed.

#[allow(dead_code)]
mod common;

use common::{LocalServer, TempDir, run_crawler};

#[test]
fn technologies_table_reaches_text_json_and_html_outputs() {
    let tmp = TempDir::new("technologies");
    let site = tmp.path.join("site");
    std::fs::create_dir_all(site.join("js")).expect("site dir");
    std::fs::write(
        site.join("index.html"),
        r#"<!DOCTYPE html><html lang="en"><head><title>Home</title><meta name="generator" content="WordPress 6.5.2"><script src="/js/jquery-3.7.1.min.js"></script></head><body><main><a href="/about.html">About</a></main></body></html>"#,
    )
    .expect("index.html");
    std::fs::write(
        site.join("about.html"),
        r#"<!DOCTYPE html><html lang="en"><head><title>About</title><script src="/js/jquery-3.7.1.min.js"></script></head><body><main>About</main></body></html>"#,
    )
    .expect("about.html");
    std::fs::write(site.join("js/jquery-3.7.1.min.js"), "/* jquery */").expect("script");
    let server = LocalServer::start(&site);
    let text_file = tmp.path.join("report.txt");
    let html_file = tmp.path.join("report.html");

    let output = run_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        "--analyzer-filter-regex=/Technologies/",
        "--http-cache-dir=",
        "--output=json",
        &format!("--output-text-file={}", text_file.display()),
        &format!("--output-html-report={}", html_file.display()),
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON on stdout");
    let rows = json["tables"]["technologies"]["rows"]
        .as_array()
        .expect("tables.technologies in the JSON output");
    let wordpress = rows
        .iter()
        .find(|r| r["technology"] == "WordPress")
        .expect("WordPress row");
    assert_eq!(wordpress["category"], "CMS");
    assert_eq!(wordpress["version"], "6.5.2");
    assert_eq!(wordpress["pages"], "1");
    let jquery = rows.iter().find(|r| r["technology"] == "jQuery").expect("jQuery row");
    assert_eq!(jquery["version"], "3.7.1");
    assert_eq!(jquery["pages"], "2");

    let text = std::fs::read_to_string(&text_file).expect("text report");
    assert!(
        text.contains("Technologies") && text.contains("WordPress"),
        "text output lists the table"
    );
    let html = std::fs::read_to_string(&html_file).expect("HTML report");
    assert!(
        html.contains("radio_technologies") && html.contains("WordPress"),
        "the HTML report has a Technologies tab"
    );
}

#[test]
fn deeply_nested_label_does_not_abort_the_crawl() {
    // The label-text walk must not recurse once per nesting level: a <label> wrapping thousands of
    // nested elements overflowed a crawler worker's stack and aborted the whole process (#112).
    let tmp = TempDir::new("deep-label");
    let site = tmp.path.join("site");
    std::fs::create_dir_all(&site).expect("site dir");
    let depth = 20_000;
    std::fs::write(
        site.join("index.html"),
        format!(
            r#"<!DOCTYPE html><html lang="en"><head><title>Deep</title></head><body><main><label for="q">{}{}</label><input id="q"></main></body></html>"#,
            "<span>".repeat(depth),
            "</span>".repeat(depth)
        ),
    )
    .expect("index.html");
    let server = LocalServer::start(&site);

    let output = run_crawler(&[
        "--config-file=/dev/null",
        &format!("--url={}", server.url()),
        "--analyzer-filter-regex=/Accessibility/",
        "--http-cache-dir=",
        "--output=json",
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "status: {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON on stdout");
    let form_labels = json["summary"]["items"]
        .as_array()
        .expect("summary items")
        .iter()
        .find(|item| item["aplCode"] == "pages-without-form-labels")
        .expect("form-label summary item");
    assert_eq!(form_labels["status"], "WARNING", "the label has no text: {form_labels}");
}
