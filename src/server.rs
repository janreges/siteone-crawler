// SiteOne Crawler - Built-in HTTP server for serving exports
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Two modes:
// - Markdown: reads .md files, renders them as styled HTML with table/accordion support
// - Offline: serves static HTML files with Content-Security-Policy restricting to same origin

use std::path::{Path, PathBuf};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::utils;
use crate::version;

/// Server mode
pub enum ServeMode {
    Markdown,
    Offline,
}

/// Run the HTTP server for serving exported content.
pub async fn run(root_dir: PathBuf, mode: ServeMode, port: u16, bind_address: &str) {
    if !root_dir.is_dir() {
        eprintln!(
            "{}",
            utils::get_color_text(
                &format!("ERROR: Directory '{}' does not exist.", root_dir.display()),
                "red",
                false,
            )
        );
        std::process::exit(101);
    }

    let mode_name = match mode {
        ServeMode::Markdown => "Markdown",
        ServeMode::Offline => "Offline HTML",
    };

    let addr = format!("{}:{}", bind_address, port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!(
                "{}",
                utils::get_color_text(
                    &format!("ERROR: Cannot bind to {}:{}: {}", bind_address, port, e),
                    "red",
                    false,
                )
            );
            std::process::exit(1);
        }
    };

    let display_host = if bind_address == "0.0.0.0" || bind_address == "127.0.0.1" {
        "localhost"
    } else {
        bind_address
    };

    println!();
    println!(
        "{}",
        utils::get_color_text(
            &format!("SiteOne Crawler v{} - {} Server", version::CODE, mode_name),
            "yellow",
            false,
        )
    );
    println!(
        "  {}",
        utils::get_color_text(&format!("Serving from: {}", root_dir.display()), "gray", false,)
    );
    println!(
        "  {}",
        utils::get_color_text(&format!("URL: http://{}:{}", display_host, port), "cyan", false,)
    );
    if bind_address == "0.0.0.0" {
        println!(
            "  {}",
            utils::get_color_text("Listening on all network interfaces", "yellow", false,)
        );
    }
    println!("  {}", utils::get_color_text("Press Ctrl+C to stop", "gray", false,));
    println!();

    let is_markdown = matches!(mode, ServeMode::Markdown);

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let root = root_dir.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, &root, is_markdown).await {
                        eprintln!("Connection error: {}", e);
                    }
                });
            }
            Err(e) => eprintln!("Accept error: {}", e),
        }
    }
}

async fn handle_connection(mut stream: TcpStream, root_dir: &Path, is_markdown: bool) -> std::io::Result<()> {
    let mut buf = vec![0u8; 8192];
    let n = match tokio::time::timeout(std::time::Duration::from_secs(30), stream.read(&mut buf)).await {
        Ok(result) => result?,
        Err(_) => return Ok(()), // read timeout — close silently
    };
    if n == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buf[..n]);

    let first_line = match request.lines().next() {
        Some(line) => line,
        None => {
            stream
                .write_all(&build_response(400, "text/plain", b"Bad Request", &[]))
                .await?;
            return Ok(());
        }
    };

    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 || (parts[0] != "GET" && parts[0] != "HEAD") {
        stream
            .write_all(&build_response(
                405,
                "text/plain",
                b"Method Not Allowed",
                &[("Allow", "GET, HEAD")],
            ))
            .await?;
        return Ok(());
    }

    let is_head = parts[0] == "HEAD";

    let raw_path = parts[1];

    // Decode percent-encoding, strip query string and fragment
    let decoded = percent_encoding::percent_decode_str(raw_path)
        .decode_utf8_lossy()
        .to_string();
    let clean_path = decoded
        .split('?')
        .next()
        .unwrap_or(&decoded)
        .split('#')
        .next()
        .unwrap_or(&decoded);

    // Security: prevent path traversal (check segments, not substring)
    let normalized = clean_path.replace('\\', "/");
    if normalized.split('/').any(|seg| seg == "..") {
        stream
            .write_all(&build_response(403, "text/plain", b"Forbidden", &[]))
            .await?;
        return Ok(());
    }

    let relative_path = normalized.trim_start_matches('/');

    let mut response = if is_markdown {
        serve_markdown_request(root_dir, relative_path)
    } else {
        serve_offline_request(root_dir, relative_path)
    };

    // For HEAD requests, send only headers (Content-Length stays correct)
    if is_head && let Some(pos) = find_header_end(&response) {
        response.truncate(pos);
    }

    let status = extract_status(&response);
    let method = parts[0];
    let status_color = if status < 300 {
        "green"
    } else if status < 400 {
        "cyan"
    } else {
        "red"
    };
    println!(
        "  {} {} {}",
        utils::get_color_text(&format!("{}", status), status_color, false),
        method,
        raw_path,
    );

    stream.write_all(&response).await?;
    Ok(())
}

fn find_header_end(response: &[u8]) -> Option<usize> {
    response.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4)
}

fn extract_status(response: &[u8]) -> u16 {
    let header = String::from_utf8_lossy(&response[..std::cmp::min(30, response.len())]);
    header
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

// ---- Markdown serving ----

fn serve_markdown_request(root_dir: &Path, relative_path: &str) -> Vec<u8> {
    let csp = ("Content-Security-Policy", "default-src 'self' 'unsafe-inline' data:");

    match resolve_markdown_path(root_dir, relative_path) {
        Some(path) if !is_within_root(root_dir, &path) => build_response(403, "text/plain", b"Forbidden", &[]),
        Some(path) if path.extension().is_some_and(|ext| ext == "md") => match std::fs::read_to_string(&path) {
            Ok(content) if content.trim().is_empty() => {
                // Empty markdown file — show directory listing instead
                let dir_path = path.parent().unwrap_or(root_dir);
                let url_path = relative_path
                    .trim_end_matches('/')
                    .trim_end_matches("index.md")
                    .trim_end_matches('/');
                let listing = directory_listing(dir_path, url_path, true);
                build_response(200, "text/html; charset=utf-8", listing.as_bytes(), &[csp])
            }
            Ok(content) => {
                let html = render_markdown_to_html(&content, relative_path);
                build_response(200, "text/html; charset=utf-8", html.as_bytes(), &[csp])
            }
            Err(_) => build_404_response(true),
        },
        Some(path) => serve_static_file(&path, &[csp]),
        None => {
            let dir_path = root_dir.join(relative_path);
            if dir_path.is_dir() && is_within_root(root_dir, &dir_path) {
                let listing = directory_listing(&dir_path, relative_path, true);
                build_response(200, "text/html; charset=utf-8", listing.as_bytes(), &[csp])
            } else {
                build_404_response(true)
            }
        }
    }
}

fn resolve_markdown_path(root_dir: &Path, relative_path: &str) -> Option<PathBuf> {
    if relative_path.is_empty() {
        let index = root_dir.join("index.md");
        if index.is_file() {
            return Some(index);
        }
        return None;
    }

    let full_path = root_dir.join(relative_path);

    // Direct file match (static files, .md files with extension in URL)
    if full_path.is_file() {
        return Some(full_path);
    }

    // Try adding .md extension
    let trimmed = relative_path.trim_end_matches('/');
    let md_path = root_dir.join(format!("{}.md", trimmed));
    if md_path.is_file() {
        return Some(md_path);
    }

    // Try as directory with index.md
    let index_path = full_path.join("index.md");
    if index_path.is_file() {
        return Some(index_path);
    }

    None
}

// ---- Offline serving ----

fn serve_offline_request(root_dir: &Path, relative_path: &str) -> Vec<u8> {
    let csp = ("Content-Security-Policy", "default-src 'self' 'unsafe-inline' data:");

    match resolve_offline_path(root_dir, relative_path) {
        Some(path) if !is_within_root(root_dir, &path) => build_response(403, "text/plain", b"Forbidden", &[]),
        Some(path) => serve_static_file(&path, &[csp]),
        None => {
            let dir_path = root_dir.join(relative_path);
            if dir_path.is_dir() && is_within_root(root_dir, &dir_path) {
                let listing = directory_listing(&dir_path, relative_path, false);
                build_response(200, "text/html; charset=utf-8", listing.as_bytes(), &[csp])
            } else {
                build_404_response(false)
            }
        }
    }
}

fn resolve_offline_path(root_dir: &Path, relative_path: &str) -> Option<PathBuf> {
    if relative_path.is_empty() {
        let index = root_dir.join("index.html");
        if index.is_file() {
            return Some(index);
        }
        return None;
    }

    let full_path = root_dir.join(relative_path);

    // Direct file match
    if full_path.is_file() {
        return Some(full_path);
    }

    // Try as directory with index.html (prefer over .html redirect files)
    let dir_path = root_dir.join(relative_path.trim_end_matches('/'));
    let index_path = dir_path.join("index.html");
    if index_path.is_file() {
        return Some(index_path);
    }

    // Try with .html extension
    let trimmed = relative_path.trim_end_matches('/');
    let html_path = root_dir.join(format!("{}.html", trimmed));
    if html_path.is_file() {
        return Some(html_path);
    }

    None
}

// ---- Shared utilities ----

fn serve_static_file(path: &Path, extra_headers: &[(&str, &str)]) -> Vec<u8> {
    match std::fs::read(path) {
        Ok(content) => {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let content_type = content_type_for_extension(ext);
            build_response(200, content_type, &content, extra_headers)
        }
        Err(_) => build_response(500, "text/plain", b"Internal Server Error", &[]),
    }
}

/// Verify that the resolved path stays within the root directory (symlink-safe).
fn is_within_root(root_dir: &Path, resolved_path: &Path) -> bool {
    let Ok(canonical_root) = std::fs::canonicalize(root_dir) else {
        return false;
    };
    let Ok(canonical_path) = std::fs::canonicalize(resolved_path) else {
        return false;
    };
    canonical_path.starts_with(&canonical_root)
}

fn build_response(status: u16, content_type: &str, body: &[u8], extra_headers: &[(&str, &str)]) -> Vec<u8> {
    let status_text = match status {
        200 => "OK",
        301 => "Moved Permanently",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "Unknown",
    };

    let mut header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nX-Powered-By: siteone-crawler/{}\r\nX-Frame-Options: DENY\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n",
        status,
        status_text,
        content_type,
        body.len(),
        version::CODE
    );

    for (name, value) in extra_headers {
        header.push_str(&format!("{}: {}\r\n", name, value));
    }

    header.push_str("\r\n");

    let mut response = header.into_bytes();
    response.extend_from_slice(body);
    response
}

fn build_404_response(is_markdown: bool) -> Vec<u8> {
    let body = if is_markdown {
        format!(
            "<!DOCTYPE html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\"><title>404 Not Found</title>\n<style>{}</style>\n</head>\n<body>\n<div class=\"container\">\n<article class=\"markdown-body\">\n<h1>404 - Page Not Found</h1>\n<p>The requested page was not found.</p>\n<p><a href=\"/\">Back to home</a></p>\n</article>\n</div>\n</body>\n</html>",
            MARKDOWN_CSS
        )
    } else {
        "<!DOCTYPE html>\n<html><body><h1>404 Not Found</h1><p>The requested file was not found.</p></body></html>"
            .to_string()
    };
    build_response(404, "text/html; charset=utf-8", body.as_bytes(), &[])
}

fn content_type_for_extension(ext: &str) -> &'static str {
    // Extensions come from the filesystem and are almost always lowercase.
    // Use a small stack buffer to avoid heap allocation for the rare uppercase case.
    let mut lower = [0u8; 8];
    let ext_lower = if ext.len() <= 8 {
        for (i, b) in ext.bytes().enumerate() {
            lower[i] = b.to_ascii_lowercase();
        }
        std::str::from_utf8(&lower[..ext.len()]).unwrap_or(ext)
    } else {
        ext // fallback: will only match if already lowercase
    };
    match ext_lower {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml; charset=utf-8",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "eot" => "application/vnd.ms-fontobject",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        _ => "application/octet-stream",
    }
}

// ---- Markdown rendering ----

fn render_markdown_to_html(markdown: &str, request_path: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};

    // Replace curly/smart quotes with straight quotes
    let markdown = markdown
        .replace(['\u{201c}', '\u{201d}'], "\"")
        .replace(['\u{2018}', '\u{2019}'], "'");

    // Clean up markdown artifacts from HTML→MD conversion
    let cleaned = clean_markdown_artifacts(&markdown);

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(&cleaned, options);
    let mut html_content = String::new();
    html::push_html(&mut html_content, parser);

    // Add id attributes to h1-h4 headings for anchor linking
    html_content = add_heading_ids(&html_content);

    // Convert heading + link-only blocks (>3 links) into accordions
    html_content = collapse_link_blocks(&html_content);

    // Add link counts to existing Menu/Links accordions
    html_content = add_accordion_link_counts(&html_content);

    // Style callout blocks (Tip, Note, Caution, etc.)
    html_content = style_callout_blocks(&html_content);

    // Extract title from first heading in the cleaned markdown
    let heading = extract_title(&cleaned);
    let title = if heading == "SiteOne Crawler - Markdown Viewer" {
        heading
    } else {
        format!("{} | SiteOne Crawler - Markdown Viewer", heading)
    };

    // Build breadcrumb navigation
    let breadcrumb = build_breadcrumb(request_path);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="author" content="SiteOne Crawler - https://crawler.siteone.io/">
<title>{title}</title>
<style>
{css}
</style>
<script>
(function(){{
  var t=localStorage.getItem('md-theme'),w=localStorage.getItem('md-width');
  if(t==='dark')document.documentElement.classList.add('dark');
  if(w==='wide')document.documentElement.classList.add('wide');
}})();
</script>
</head>
<body>
<div class="container">
<nav class="breadcrumb">
<span class="breadcrumb-path">{breadcrumb}</span>
<span class="toolbar">
<button onclick="toggleWidth()" id="width-btn" title="Toggle full width"></button>
<button onclick="toggleTheme()" id="theme-btn" title="Toggle dark/light mode"></button>
</span>
</nav>
<article class="markdown-body">
{content}
</article>
<footer>
<p>Served by <a href="https://crawler.siteone.io/" target="_blank" rel="noopener">SiteOne Crawler</a> v{version}</p>
</footer>
</div>
<script>
var svgMoon='<svg viewBox="0 0 16 16" width="14" height="14"><path d="M6 .5a7.5 7.5 0 1 0 8 12A6 6 0 0 1 6 .5z" fill="currentColor"/></svg>';
var svgSun='<svg viewBox="0 0 16 16" width="14" height="14"><circle cx="8" cy="8" r="2.8" fill="currentColor"/><g stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><line x1="8" y1=".5" x2="8" y2="3"/><line x1="8" y1="13" x2="8" y2="15.5"/><line x1=".5" y1="8" x2="3" y2="8"/><line x1="13" y1="8" x2="15.5" y2="8"/><line x1="2.7" y1="2.7" x2="4.5" y2="4.5"/><line x1="11.5" y1="11.5" x2="13.3" y2="13.3"/><line x1="2.7" y1="13.3" x2="4.5" y2="11.5"/><line x1="11.5" y1="4.5" x2="13.3" y2="2.7"/></g></svg>';
var svgExpand='<svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M1 8h14M4.5 5L1 8l3.5 3M11.5 5L15 8l-3.5 3"/></svg>';
var svgContract='<svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M1 8h5M10 8h5M3.5 5L7 8l-3.5 3M12.5 5L9 8l3.5 3"/></svg>';
function toggleTheme(){{
  document.documentElement.classList.toggle('dark');
  localStorage.setItem('md-theme',document.documentElement.classList.contains('dark')?'dark':'light');
  updBtn();
}}
function toggleWidth(){{
  document.documentElement.classList.toggle('wide');
  localStorage.setItem('md-width',document.documentElement.classList.contains('wide')?'wide':'narrow');
  updBtn();
}}
function updBtn(){{
  var d=document.documentElement.classList.contains('dark');
  var w=document.documentElement.classList.contains('wide');
  document.getElementById('theme-btn').innerHTML=d?svgSun:svgMoon;
  document.getElementById('width-btn').innerHTML=w?svgContract:svgExpand;
}}
updBtn();
</script>
</body>
</html>"#,
        title = html_escape(&title),
        css = MARKDOWN_CSS,
        breadcrumb = breadcrumb,
        content = html_content,
        version = version::CODE,
    )
}

/// Clean up common artifacts left by HTML→Markdown export.
fn clean_markdown_artifacts(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut result: Vec<&str> = Vec::with_capacity(lines.len());
    let mut in_code_block = false;

    // Phase 1: Skip site navigation header before the first h1 heading.
    // If there are 3+ links before the first h1, the content is likely nav/header.
    // But preserve <details> blocks (accordion menus) from the navigation.
    let mut content_start = 0;
    {
        let mut link_count = 0;
        for idx in 0..lines.len() {
            let t = lines[idx].trim();
            if t.starts_with("```") {
                break; // don't look inside code blocks
            }

            // Check for setext h1 (line followed by ===)
            if idx + 1 < lines.len() {
                let next = lines[idx + 1].trim();
                if !next.is_empty() && next.len() >= 3 && next.chars().all(|c| c == '=') {
                    if link_count >= 3 {
                        content_start = idx;
                    }
                    break;
                }
            }
            // Check for ATX h1
            if t.starts_with("# ") && !t.starts_with("## ") {
                if link_count >= 3 {
                    content_start = idx;
                }
                break;
            }

            // Count links in non-heading lines
            link_count += t.matches("](").count();
        }
    }

    // Phase 1b: Preserve navigation from the skipped section.
    // If the section already contains <details> blocks, preserve them as-is.
    // Otherwise, wrap the entire nav content in a <details><summary>Menu</summary> block.
    if content_start > 0 {
        let has_details = lines[..content_start]
            .iter()
            .any(|l| l.trim() == "<details>" || l.trim().starts_with("<details>"));

        if has_details {
            // Preserve existing <details> blocks (e.g. astro Menu accordion)
            let mut k = 0;
            while k < content_start {
                let t = lines[k].trim();
                if t == "<details>" || t.starts_with("<details>") {
                    while k < content_start {
                        result.push(lines[k]);
                        k += 1;
                        if lines[k - 1].trim() == "</details>" {
                            result.push(""); // blank line required so pulldown-cmark ends the HTML block
                            break;
                        }
                    }
                } else {
                    k += 1;
                }
            }
        } else {
            // No <details> blocks — wrap nav content in an accordion.
            // Collect non-empty, non-artifact lines as the accordion body.
            let mut nav_lines: Vec<&str> = Vec::new();
            for line in &lines[..content_start] {
                let t = line.trim();
                // Skip header artifacts
                if t.is_empty()
                    || t == "-"
                    || t.starts_with("[Skip to content]")
                    || (t.starts_with("| [") && t.ends_with(" |"))
                {
                    continue;
                }
                // Skip setext underlines (--- or ===)
                if t.len() >= 3 && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '=')) {
                    continue;
                }
                // Skip heading text that is just "Site navigation" or similar
                if t == "Site navigation" || t == "Navigation" {
                    continue;
                }
                nav_lines.push(line);
            }
            // Trim trailing non-link plain-text lines (breadcrumb labels etc.)
            while let Some(last) = nav_lines.last() {
                let t = last.trim();
                if !t.contains("](") && !t.starts_with("- ") && !t.starts_with("### ") {
                    nav_lines.pop();
                } else {
                    break;
                }
            }
            if nav_lines.len() > 3 {
                result.push("<details>");
                result.push("<summary>Menu</summary>");
                result.push("");
                for line in &nav_lines {
                    result.push(line);
                }
                result.push("");
                result.push("</details>");
                result.push(""); // empty line so pulldown-cmark starts a new block
            }
        }
    }

    // Phase 2: Process lines from content_start, filtering artifacts
    let mut i = content_start;
    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Track code blocks to avoid filtering inside them
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            result.push(lines[i]);
            i += 1;
            continue;
        }
        if in_code_block {
            result.push(lines[i]);
            i += 1;
            continue;
        }

        // Skip empty list items (just "-" with no text content)
        if trimmed == "-" {
            i += 1;
            continue;
        }

        // Skip [Section titled "..."](...) lines
        if trimmed.starts_with("[Section titled \"") && trimmed.ends_with(')') {
            i += 1;
            continue;
        }

        // Skip [Skip to content](...) variants
        if trimmed.starts_with("[Skip to content]") {
            i += 1;
            continue;
        }

        // Detect footer: "[Go to ... homepage](...)" standalone link
        if !trimmed.starts_with("- ")
            && trimmed.starts_with("[Go to ")
            && trimmed.to_lowercase().contains("homepage")
            && trimmed.ends_with(')')
        {
            break;
        }

        // Skip "On this page" heading + its following <details> block
        if (trimmed == "On this page" || trimmed == "On this page:")
            && i + 1 < lines.len()
            && lines[i + 1].trim().starts_with('-')
        {
            // Skip underline-style heading (--- below)
            i += 1;
            if i < lines.len() && lines[i].trim().starts_with('-') && lines[i].trim().chars().all(|c| c == '-') {
                i += 1;
            }
            // Skip until next heading or end of <details> block
            while i < lines.len() {
                let t = lines[i].trim();
                if t.starts_with("# ")
                    || t.starts_with("## ")
                    || (t.starts_with('#') && t.chars().nth(1).is_some_and(|c| c == '#' || c == ' '))
                {
                    break;
                }
                if t == "</details>" {
                    i += 1;
                    break;
                }
                i += 1;
            }
            // Skip empty lines after the block
            while i < lines.len() && lines[i].trim().is_empty() {
                i += 1;
            }
            continue;
        }

        // Skip footer artifacts: "Learn" heading (astro docs pattern)
        if trimmed == "Learn"
            && i + 1 < lines.len()
            && (lines.get(i + 1).is_some_and(|l| l.trim().is_empty())
                || lines.get(i + 2).is_some_and(|l| l.trim().starts_with("| [")))
        {
            break;
        }

        result.push(lines[i]);
        i += 1;
    }

    // Phase 3: Fix broken code fences — detect unfenced code blocks between
    // a closing ``` and an opening ```lang (or at end of content).
    // The HTML→MD converter sometimes misses fences for tabbed code examples.
    let mut fixed: Vec<&str> = Vec::with_capacity(result.len());
    let mut ri = 0;
    let result_lines: Vec<&str> = result; // take ownership
    let mut in_fence = false;
    while ri < result_lines.len() {
        let t = result_lines[ri].trim();
        if t.starts_with("```") {
            in_fence = !in_fence;
            fixed.push(result_lines[ri]);
            ri += 1;
            continue;
        }
        if in_fence {
            fixed.push(result_lines[ri]);
            ri += 1;
            continue;
        }

        // Not inside a fence — check if this starts an unfenced code block.
        // Heuristic: if the previous non-empty line was a closing ```,
        // and this line looks like code (contains semicolons, braces, =>, etc.),
        // collect all lines until the next ``` or blank-line gap.
        let prev_is_fence_close = fixed
            .iter()
            .rev()
            .find(|l| !l.trim().is_empty())
            .is_some_and(|l| l.trim().starts_with("```"));

        if prev_is_fence_close && !t.is_empty() && looks_like_code(t) {
            // Collect unfenced code lines
            let start = ri;
            while ri < result_lines.len() {
                let lt = result_lines[ri].trim();
                if lt.starts_with("```") {
                    break;
                }
                // Stop if we hit a markdown heading or a completely blank line
                // followed by non-code content
                if lt.is_empty() {
                    // Look ahead: if the next non-empty line is not code-like, stop
                    let mut peek = ri + 1;
                    while peek < result_lines.len() && result_lines[peek].trim().is_empty() {
                        peek += 1;
                    }
                    if peek < result_lines.len() && !looks_like_code(result_lines[peek].trim()) {
                        break;
                    }
                }
                if lt.starts_with("# ") || lt.starts_with("## ") || lt.starts_with("### ") {
                    break;
                }
                ri += 1;
            }
            // Trim trailing empty lines from the collected block
            let mut end = ri;
            while end > start && result_lines[end - 1].trim().is_empty() {
                end -= 1;
            }
            if end > start {
                fixed.push("```");
                for line in &result_lines[start..end] {
                    fixed.push(line);
                }
                fixed.push("```");
            }
            // If the next line is an opening ```, it will be handled normally
            continue;
        }

        fixed.push(result_lines[ri]);
        ri += 1;
    }

    fixed.join("\n")
}

/// Heuristic: does this line look like source code?
/// Requires at least 2 code indicators to reduce false positives on prose.
fn looks_like_code(line: &str) -> bool {
    let mut score = 0;
    // Strong indicators (score 2) — very unlikely in prose
    if line.contains("=> {") || line.contains("=> (") {
        score += 2;
    }
    if line.contains("};") {
        score += 2;
    }
    if line.ends_with(';') {
        score += 2;
    }
    if line.starts_with("//") {
        score += 2;
    }
    if line.starts_with("if (") || line.starts_with("if (!") {
        score += 2;
    }
    if line.contains("?.") {
        score += 2;
    } // optional chaining
    if line.contains("===") || line.contains("!==") {
        score += 2;
    }
    // Moderate indicators (score 1)
    if line.contains("export ") {
        score += 1;
    }
    if line.contains("const ") {
        score += 1;
    }
    if line.contains("return ") {
        score += 1;
    }
    if line.contains("await ") {
        score += 1;
    }
    if line.contains("async ") {
        score += 1;
    }
    if line.contains("function ") {
        score += 1;
    }
    if line.ends_with('{') || line.ends_with('}') {
        score += 1;
    }
    score >= 2
}

/// Convert standalone callout paragraphs (Tip, Note, Caution, etc.) into styled divs.
fn style_callout_blocks(html: &str) -> String {
    let callout_patterns: [(&str, &str); 6] = [
        ("<p>Tip</p>", "Tip"),
        ("<p>Note</p>", "Note"),
        ("<p>Caution</p>", "Caution"),
        ("<p>Warning</p>", "Warning"),
        ("<p>Important</p>", "Important"),
        ("<p>Quick start</p>", "Quick start"),
    ];
    let mut result = String::with_capacity(html.len() + 512);
    let lines: Vec<&str> = html.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Match <p>Label</p> followed by <p>content</p>
        let mut matched_label = None;
        for (pattern, label) in &callout_patterns {
            if trimmed == *pattern {
                matched_label = Some(*label);
                break;
            }
        }

        if let Some(label) = matched_label {
            let icon = match label {
                "Tip" => "💡",
                "Note" | "Important" => "📝",
                "Caution" | "Warning" => "⚠️",
                "Quick start" => "🚀",
                _ => "ℹ️",
            };
            let css_class = match label {
                "Caution" | "Warning" => "callout callout-warning",
                "Tip" | "Quick start" => "callout callout-tip",
                _ => "callout callout-note",
            };
            // Collect following paragraphs as callout content
            result.push_str(&format!(
                "<div class=\"{}\">\n<p class=\"callout-title\">{} {}</p>\n",
                css_class, icon, label
            ));
            i += 1;
            // Include subsequent content paragraphs until next heading or another callout
            while i < lines.len() {
                let next = lines[i].trim();
                if next.is_empty() {
                    i += 1;
                    continue;
                }
                // Stop at headings, details, or another callout label
                if next.starts_with("<h") || next.starts_with("<details") || next.starts_with("<div class=\"callout") {
                    break;
                }
                // Include this line in the callout
                result.push_str(lines[i]);
                result.push('\n');
                i += 1;
                // Only include the first content element
                break;
            }
            result.push_str("</div>\n");
        } else {
            result.push_str(lines[i]);
            result.push('\n');
            i += 1;
        }
    }

    result
}

fn extract_title(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut in_code_block = false;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        // ATX h1: # Heading
        if let Some(heading) = trimmed.strip_prefix("# ") {
            return heading.trim().to_string();
        }
        // Setext h1: text followed by === on the next line
        if !trimmed.is_empty()
            && i + 1 < lines.len()
            && lines[i + 1].trim().len() >= 3
            && lines[i + 1].trim().chars().all(|c| c == '=')
        {
            return trimmed.to_string();
        }
    }
    "SiteOne Crawler - Markdown Viewer".to_string()
}

fn add_heading_ids(html: &str) -> String {
    let closing_tags: [&str; 4] = ["</h1>", "</h2>", "</h3>", "</h4>"];
    let mut result = String::with_capacity(html.len() + 256);
    let mut used_slugs: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut search_from = 0;

    while search_from < html.len() {
        // Find next '<h' pattern
        let remaining = &html[search_from..];
        let next_h = remaining.find("<h").and_then(|pos| {
            let abs = search_from + pos;
            let bytes = html.as_bytes();
            if abs + 3 < bytes.len() && bytes[abs + 2] >= b'1' && bytes[abs + 2] <= b'4' && bytes[abs + 3] == b'>' {
                Some((abs, (bytes[abs + 2] - b'1') as usize))
            } else {
                None
            }
        });

        match next_h {
            Some((tag_start, level_idx)) => {
                let tag_end = tag_start + 4; // past '>'
                let closing_tag = closing_tags[level_idx];
                if let Some(close_rel) = html[tag_end..].find(closing_tag) {
                    let inner_html = &html[tag_end..tag_end + close_rel];
                    let plain_text = strip_html_tags(inner_html);
                    let base_slug = slugify(&plain_text);

                    // Copy everything before this heading
                    result.push_str(&html[search_from..tag_start]);

                    if !base_slug.is_empty() {
                        // Deduplicate slug
                        let slug = match used_slugs.get(&base_slug) {
                            Some(&count) => {
                                let deduped = format!("{}-{}", base_slug, count);
                                *used_slugs.get_mut(&base_slug).unwrap() = count + 1;
                                deduped
                            }
                            None => {
                                used_slugs.insert(base_slug.clone(), 1);
                                base_slug
                            }
                        };
                        let escaped_slug = html_escape(&slug);
                        result.push_str(&format!(
                            "<h{0} id=\"{1}\"><a href=\"#{1}\" class=\"heading-link\">",
                            level_idx + 1,
                            escaped_slug
                        ));
                        result.push_str(inner_html);
                        result.push_str("</a>");
                        result.push_str(closing_tag);
                    } else {
                        result.push_str(&html[tag_start..tag_end]);
                        result.push_str(inner_html);
                        result.push_str(closing_tag);
                    }
                    search_from = tag_end + close_rel + closing_tag.len();
                } else {
                    // No closing tag found — emit the opening tag and continue
                    result.push_str(&html[search_from..tag_end]);
                    search_from = tag_end;
                }
            }
            None => {
                // No more headings — copy rest and done
                result.push_str(&html[search_from..]);
                break;
            }
        }
    }

    result
}

/// Detect heading level from an HTML line like `<h2 ...>` or `<h2>`.
fn detect_heading_level(line: &str) -> Option<u8> {
    let t = line.trim();
    if t.starts_with("<h") && t.len() > 3 {
        let ch = t.as_bytes()[2];
        if (b'1'..=b'6').contains(&ch) && (t.as_bytes()[3] == b'>' || t.as_bytes()[3] == b' ') {
            return Some(ch - b'0');
        }
    }
    None
}

/// When a heading is followed only by link-only content (paragraphs or lists)
/// and there are more than 3 links, collapse them into an accordion.
/// Also handles sub-headings + link blocks grouped under a parent heading.
fn collapse_link_blocks(html: &str) -> String {
    let lines: Vec<&str> = html.lines().collect();
    let mut result = String::with_capacity(html.len() + 512);
    let mut i = 0;
    let mut details_depth = 0; // track nesting inside <details> blocks

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Track <details> nesting — don't create accordions inside existing ones
        if trimmed.starts_with("<details") {
            details_depth += 1;
        }
        if trimmed == "</details>" && details_depth > 0 {
            details_depth -= 1;
        }

        let heading_level = detect_heading_level(trimmed);

        if details_depth > 0 {
            // Inside an existing <details> block — pass through without collapsing
            result.push_str(lines[i]);
            result.push('\n');
            i += 1;
            continue;
        }

        if let Some(level) = heading_level {
            let heading_line = lines[i];
            let closing = format!("</h{}>", level);
            let heading_text = if let Some(start) = heading_line.find('>') {
                let after = &heading_line[start + 1..];
                if let Some(end) = after.find(&closing) {
                    strip_html_tags(&after[..end]).trim().to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            // Scan forward: collect link-only content blocks
            // Allowed: empty lines, link-only <p>, link-only <ul>, sub-headings
            let mut j = i + 1;
            let mut link_count = 0;
            let mut content_indices: Vec<(usize, usize)> = Vec::new(); // (start, end) ranges
            let mut all_link_only = true;

            while j < lines.len() {
                let next = lines[j].trim();

                if next.is_empty() {
                    content_indices.push((j, j + 1));
                    j += 1;
                    continue;
                }

                // Stop at same-or-higher-level heading
                if let Some(next_level) = detect_heading_level(next) {
                    if next_level <= level {
                        break;
                    }
                    // Sub-heading within this section — include it
                    content_indices.push((j, j + 1));
                    j += 1;
                    continue;
                }

                // Link-only paragraph
                if is_link_only_paragraph(next) {
                    link_count += next.matches("<a ").count();
                    content_indices.push((j, j + 1));
                    j += 1;
                    continue;
                }

                // Link-only <ul> block
                if next == "<ul>" {
                    let ul_start = j;
                    let mut ul_links = 0;
                    let mut ul_ok = true;
                    let mut k = j + 1;
                    while k < lines.len() {
                        let ul_line = lines[k].trim();
                        if ul_line == "</ul>" {
                            k += 1;
                            break;
                        }
                        if ul_line.starts_with("<li>") && ul_line.contains("<a ") {
                            ul_links += 1;
                        } else if ul_line.starts_with("<li>") && ul_line != "<li></li>" {
                            // Non-link list item with content
                            let inner_text = strip_html_tags(ul_line);
                            if !inner_text.trim().is_empty() {
                                ul_ok = false;
                            }
                        }
                        k += 1;
                    }

                    if ul_ok && ul_links > 0 {
                        link_count += ul_links;
                        content_indices.push((ul_start, k));
                        j = k;
                        continue;
                    }

                    all_link_only = false;
                    break;
                }

                // Any other content → stop
                all_link_only = false;
                break;
            }

            if all_link_only && link_count > 3 && !heading_text.is_empty() {
                result.push_str(&format!(
                    "<details>\n<summary>{} ({} links)</summary>\n",
                    html_escape(&heading_text),
                    link_count
                ));
                for (start, end) in &content_indices {
                    for line in lines.iter().take(*end).skip(*start) {
                        result.push_str(line);
                        result.push('\n');
                    }
                }
                result.push_str("</details>\n");
                i = j;
            } else {
                result.push_str(heading_line);
                result.push('\n');
                i += 1;
            }
        } else {
            result.push_str(lines[i]);
            result.push('\n');
            i += 1;
        }
    }

    result
}

/// Check if an HTML line is a `<p>` containing only `<a>` links.
fn is_link_only_paragraph(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with("<p>") || !trimmed.ends_with("</p>") {
        return false;
    }
    let inner = &trimmed[3..trimmed.len() - 4];
    if !inner.contains("<a ") {
        return false;
    }
    let mut remaining = inner.to_string();
    while let Some(start) = remaining.find("<a ") {
        if let Some(end) = remaining[start..].find("</a>") {
            remaining = format!("{}{}", &remaining[..start], &remaining[start + end + 4..]);
        } else {
            break;
        }
    }
    remaining.trim().is_empty()
}

/// Find `<details>` blocks whose `<summary>` is "Menu" or "Links" and append link count.
fn add_accordion_link_counts(html: &str) -> String {
    let mut result = String::with_capacity(html.len() + 256);
    let mut search_from = 0;

    while let Some(details_start) = html[search_from..].find("<details>") {
        let abs_start = search_from + details_start;
        // Copy everything before this <details>
        result.push_str(&html[search_from..abs_start]);

        // Find matching </details> (respecting nesting)
        let after_tag = abs_start + "<details>".len();
        let details_end = {
            let mut depth = 1;
            let mut scan = after_tag;
            loop {
                let next_open = html[scan..].find("<details>");
                let next_close = html[scan..].find("</details>");
                match next_close {
                    Some(close_rel) => {
                        if let Some(open_rel) = next_open
                            && open_rel < close_rel
                        {
                            depth += 1;
                            scan += open_rel + "<details>".len();
                        } else {
                            depth -= 1;
                            scan += close_rel + "</details>".len();
                            if depth == 0 {
                                break;
                            }
                        }
                    }
                    None => break,
                }
            }
            if depth == 0 { Some(scan) } else { None }
        };
        if let Some(details_end) = details_end {
            let block = &html[abs_start..details_end];

            // Check if summary is "Menu" or "Links"
            if let Some(summary_start) = block.find("<summary>")
                && let Some(summary_end) = block.find("</summary>")
            {
                let summary_text = &block[summary_start + "<summary>".len()..summary_end];
                let trimmed_summary = summary_text.trim();

                if trimmed_summary == "Menu" || trimmed_summary == "Links" {
                    // Count <a> links inside the block
                    let link_count = block.matches("<a ").count() + block.matches("<a\n").count();
                    if link_count > 0 {
                        // Rebuild block with count in summary
                        let new_summary = format!("<summary>{} ({} links)</summary>", trimmed_summary, link_count);
                        let before_summary = &block[..summary_start];
                        let after_summary = &block[summary_end + "</summary>".len()..];
                        result.push_str(before_summary);
                        result.push_str(&new_summary);
                        result.push_str(after_summary);
                        search_from = details_end;
                        continue;
                    }
                }
            }

            // Not a Menu/Links accordion — emit as-is
            result.push_str(block);
            search_from = details_end;
        } else {
            // No closing </details> — emit rest as-is
            result.push_str(&html[abs_start..]);
            search_from = html.len();
        }
    }

    // Copy remaining text
    result.push_str(&html[search_from..]);
    result
}

fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    result
}

fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else if c == ' ' || c == '_' || c == '-' {
                '-'
            } else {
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn build_breadcrumb(request_path: &str) -> String {
    let mut parts = vec![r#"<a href="/">Home</a>"#.to_string()];

    let clean = request_path
        .trim_end_matches(".md")
        .trim_end_matches("/index")
        .trim_end_matches('/');

    if !clean.is_empty() {
        let segments: Vec<&str> = clean.split('/').filter(|s| !s.is_empty()).collect();
        let mut accumulated = String::new();
        for (i, segment) in segments.iter().enumerate() {
            accumulated.push('/');
            accumulated.push_str(segment);
            let display = title_case_segment(segment);
            if i == segments.len() - 1 {
                parts.push(format!("<span>{}</span>", html_escape(&display)));
            } else {
                parts.push(format!(
                    r#"<a href="{}">{}</a>"#,
                    html_escape(&accumulated),
                    html_escape(&display)
                ));
            }
        }
    }

    parts.join(" / ")
}

/// Convert URL path segment to Title Case: "marketing-sites" → "Marketing Sites"
fn title_case_segment(segment: &str) -> String {
    segment
        .split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => {
                    let mut s = c.to_uppercase().to_string();
                    s.extend(chars);
                    s
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ---- Directory listing ----

fn directory_listing(dir_path: &Path, url_path: &str, is_markdown: bool) -> String {
    let mut entries: Vec<(String, bool)> = Vec::new();

    if let Ok(read_dir) = std::fs::read_dir(dir_path) {
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.path().is_dir();
            entries.push((name, is_dir));
        }
    }

    // Directories first, then alphabetical
    entries.sort_by(|a, b| match (a.1, b.1) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
    });

    let url_base = if url_path.is_empty() {
        String::new()
    } else {
        format!("/{}", url_path.trim_end_matches('/'))
    };

    let mut items = String::new();

    if !url_path.is_empty() {
        items.push_str("<li class=\"dir\"><a href=\"..\">..</a></li>\n");
    }

    for (name, is_dir) in &entries {
        let css_class = if *is_dir { "dir" } else { "file" };
        let href = if *is_dir {
            format!("{}/{}/", url_base, name)
        } else {
            format!("{}/{}", url_base, name)
        };
        let display = if *is_dir { format!("{}/", name) } else { name.clone() };
        items.push_str(&format!(
            "<li class=\"{}\"><a href=\"{}\">{}</a></li>\n",
            css_class,
            html_escape(&href),
            html_escape(&display),
        ));
    }

    let title = if url_path.is_empty() {
        "Index".to_string()
    } else {
        format!("/{}", url_path)
    };

    if is_markdown {
        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} - Directory</title>
<style>
{css}
</style>
</head>
<body>
<div class="container">
<nav class="breadcrumb"><a href="/">Home</a></nav>
<article class="markdown-body">
<h1>{title}</h1>
<ul class="directory-listing">
{items}
</ul>
</article>
</div>
</body>
</html>"#,
            title = html_escape(&title),
            css = MARKDOWN_CSS,
            items = items,
        )
    } else {
        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{title} - Directory</title>
<style>
body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; margin: 20px 40px; color: #24292e; }}
a {{ color: #0366d6; text-decoration: none; }}
a:hover {{ text-decoration: underline; }}
ul {{ list-style: none; padding: 0; }}
li {{ padding: 4px 0; }}
li.dir a::before {{ content: "[ ] "; font-family: monospace; }}
li.file a::before {{ content: "  - "; font-family: monospace; }}
</style>
</head>
<body>
<h1>{title}</h1>
<ul>
{items}
</ul>
</body>
</html>"#,
            title = html_escape(&title),
            items = items,
        )
    }
}

// ---- CSS Theme ----

const MARKDOWN_CSS: &str = r##"
* { margin: 0; padding: 0; box-sizing: border-box; }

body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif;
    font-size: 15px;
    line-height: 1.6;
    color: #1f2328;
    background: #fff;
    -webkit-font-smoothing: antialiased;
}

.container {
    max-width: 860px;
    margin: 0 auto;
    padding: 16px 32px 32px;
}

/* Breadcrumb & Toolbar */
.breadcrumb {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 0;
    margin-bottom: 16px;
    border-bottom: 1px solid #d1d9e0;
    font-size: 13px;
    color: #656d76;
}
.breadcrumb a { color: #0969da; text-decoration: none; }
.breadcrumb a:hover { text-decoration: underline; }
.breadcrumb-path span { color: #1f2328; font-weight: 500; }
.toolbar { display: flex; gap: 4px; }
.toolbar button {
    background: none; border: 1px solid #d1d9e0; border-radius: 6px;
    cursor: pointer; font-size: 16px; width: 32px; height: 28px;
    display: flex; align-items: center; justify-content: center;
    color: #656d76; transition: background .15s;
}
.toolbar button:hover { background: #f3f4f6; }

/* Headings */
.markdown-body h1, .markdown-body h2, .markdown-body h3,
.markdown-body h4, .markdown-body h5, .markdown-body h6 {
    margin-top: 1.5em;
    margin-bottom: 0.5em;
    font-weight: 600;
    line-height: 1.3;
    color: #1f2328;
}
.markdown-body h1 { font-size: 1.85em; padding-bottom: .25em; border-bottom: 1px solid #d1d9e0; margin-top: 0; }
.markdown-body h2 { font-size: 1.4em; padding-bottom: .2em; border-bottom: 1px solid #d1d9e0; }
.markdown-body h3 { font-size: 1.15em; }
.markdown-body h4 { font-size: 1em; }
.markdown-body h5, .markdown-body h6 { font-size: .9em; color: #656d76; }
.markdown-body h1:first-child { margin-top: 0; }

/* Heading anchor links */
.markdown-body h1[id], .markdown-body h2[id], .markdown-body h3[id], .markdown-body h4[id] {
    position: relative;
}
.markdown-body .heading-link,
.markdown-body .heading-link:hover,
.markdown-body .heading-link:visited,
.markdown-body .heading-link:active {
    color: inherit;
    text-decoration: none;
}
.markdown-body h1[id]:hover::before, .markdown-body h2[id]:hover::before,
.markdown-body h3[id]:hover::before, .markdown-body h4[id]:hover::before {
    content: "#";
    position: absolute;
    left: -1.2em;
    color: #0969da;
    font-weight: 400;
}

/* Paragraphs and text */
.markdown-body p { margin-bottom: 12px; }
.markdown-body > p:last-child { margin-bottom: 0; }

.markdown-body a { color: #0969da; text-decoration: none; }
.markdown-body a:hover { text-decoration: underline; color: #0550ae; }
.markdown-body a:visited { color: #6639ba; }

.markdown-body strong { font-weight: 600; }
.markdown-body em { font-style: italic; }
.markdown-body del { color: #656d76; }

/* Inline code */
.markdown-body code {
    padding: .15em .35em;
    font-size: 84%;
    background-color: #eff1f3;
    border-radius: 4px;
    font-family: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, 'Liberation Mono', monospace;
    color: #1f2328;
}

/* Code blocks */
.markdown-body pre {
    padding: 14px 16px;
    overflow: auto;
    font-size: 84%;
    line-height: 1.5;
    background-color: #f6f8fa;
    border-radius: 8px;
    border: 1px solid #d1d9e0;
    margin-bottom: 14px;
}
.markdown-body pre code {
    display: block;
    padding: 0;
    overflow: visible;
    background: transparent;
    border: 0;
    font-size: 100%;
    color: inherit;
    border-radius: 0;
}

/* Tables */
.markdown-body table {
    border-collapse: collapse;
    width: 100%;
    margin-bottom: 14px;
    display: block;
    overflow-x: auto;
    font-size: 14px;
}
.markdown-body th, .markdown-body td {
    padding: 6px 12px;
    border: 1px solid #d1d9e0;
}
.markdown-body th {
    font-weight: 600;
    background-color: #f6f8fa;
    text-align: left;
}
.markdown-body tr:nth-child(2n) { background-color: #f6f8fa; }

/* Blockquotes */
.markdown-body blockquote {
    padding: 4px 16px;
    color: #656d76;
    border-left: 3px solid #d1d9e0;
    margin-bottom: 12px;
}
.markdown-body blockquote p { margin-bottom: 4px; }
.markdown-body blockquote p:last-child { margin-bottom: 0; }

/* Lists — compact */
.markdown-body ul, .markdown-body ol {
    padding-left: 1.5em;
    margin-bottom: 10px;
}
.markdown-body li {
    margin: 1px 0;
    line-height: 1.5;
}
.markdown-body li > p { margin-bottom: 4px; }
.markdown-body li > ul, .markdown-body li > ol {
    margin-top: 2px;
    margin-bottom: 2px;
}

/* Task lists */
.markdown-body input[type="checkbox"] {
    margin-right: .4em;
    vertical-align: middle;
    position: relative;
    top: -1px;
}

/* Images */
.markdown-body img {
    max-width: 100%;
    height: auto;
    border-style: none;
    border-radius: 6px;
}

/* Horizontal rules */
.markdown-body hr {
    height: 2px;
    padding: 0;
    margin: 20px 0;
    background-color: #d1d9e0;
    border: 0;
}

/* Accordions (details/summary) */
.markdown-body details {
    margin: 12px 0;
    border: 1px solid #d1d9e0;
    border-radius: 8px;
    padding: 0;
    overflow: hidden;
}
.markdown-body details summary {
    cursor: pointer;
    font-weight: 600;
    font-size: 14px;
    padding: 8px 14px;
    background-color: #f6f8fa;
    user-select: none;
    list-style: none;
    display: flex;
    align-items: center;
    gap: 6px;
}
.markdown-body details summary::before {
    content: "▶";
    font-size: 10px;
    color: #656d76;
    transition: transform .15s ease;
    display: inline-block;
    flex-shrink: 0;
}
.markdown-body details[open] summary::before {
    transform: rotate(90deg);
}
.markdown-body details summary::-webkit-details-marker { display: none; }
.markdown-body details[open] summary {
    border-bottom: 1px solid #d1d9e0;
}
.markdown-body details summary:hover {
    background-color: #eaeef2;
}
.markdown-body details > :not(summary) {
    padding: 0 14px;
}
.markdown-body details > p:first-of-type {
    margin-top: 10px;
}
.markdown-body details > ul, .markdown-body details > ol {
    padding: 8px 14px 6px 32px;
}
.markdown-body details > ul li, .markdown-body details > ol li {
    font-size: 14px;
}

/* Callout boxes (Tip, Note, Caution, etc.) */
.callout {
    margin: 14px 0;
    padding: 12px 16px;
    border-radius: 8px;
    border-left: 4px solid;
    font-size: 14px;
}
.callout-note {
    background-color: #ddf4ff;
    border-left-color: #0969da;
}
.callout-tip {
    background-color: #dafbe1;
    border-left-color: #1a7f37;
}
.callout-warning {
    background-color: #fff8c5;
    border-left-color: #9a6700;
}
.callout .callout-title {
    font-weight: 600;
    margin-bottom: 4px;
    font-size: 14px;
}
.callout p { margin-bottom: 4px; }
.callout p:last-child { margin-bottom: 0; }

/* Directory listing */
.directory-listing { list-style: none; padding: 0 !important; }
.directory-listing li {
    padding: 5px 8px;
    border-bottom: 1px solid #f0f2f4;
}
.directory-listing li:last-child { border-bottom: none; }
.directory-listing li a {
    display: block;
    text-decoration: none;
    color: #0969da;
}
.directory-listing li a:hover { text-decoration: underline; }
.directory-listing li.dir a { font-weight: 600; color: #1f2328; }
.directory-listing li.dir a::before { content: "📁  "; }
.directory-listing li.file a::before { content: "📄  "; }

/* Footer */
footer {
    margin-top: 32px;
    padding-top: 12px;
    border-top: 1px solid #d1d9e0;
    font-size: 12px;
    color: #656d76;
}
footer a { color: #0969da; text-decoration: none; }
footer a:hover { text-decoration: underline; }

/* Selection highlight */
::selection { background-color: #dbe9f9; }

/* Smooth scroll for anchor links */
html { scroll-behavior: smooth; }

/* Wide mode */
html.wide .container { max-width: 100%; }

/* Dark mode */
html.dark body { background: #0d1117; color: #e6edf3; }
html.dark .breadcrumb { border-color: #30363d; color: #8b949e; }
html.dark .breadcrumb a { color: #58a6ff; }
html.dark .breadcrumb-path span { color: #e6edf3; }
html.dark .toolbar button { border-color: #30363d; color: #8b949e; }
html.dark .toolbar button:hover { background: #21262d; }
html.dark .markdown-body { color: #e6edf3; }
html.dark .markdown-body h1,
html.dark .markdown-body h2,
html.dark .markdown-body h3,
html.dark .markdown-body h4 { color: #e6edf3; }
html.dark .markdown-body h1, html.dark .markdown-body h2 { border-color: #30363d; }
html.dark .markdown-body h5, html.dark .markdown-body h6 { color: #8b949e; }
html.dark .markdown-body a { color: #58a6ff; }
html.dark .markdown-body a:hover { color: #79c0ff; }
html.dark .markdown-body a:visited { color: #bc8cff; }
html.dark .markdown-body .heading-link,
html.dark .markdown-body .heading-link:hover,
html.dark .markdown-body .heading-link:visited { color: inherit; }
html.dark .markdown-body h1[id]:hover::before,
html.dark .markdown-body h2[id]:hover::before,
html.dark .markdown-body h3[id]:hover::before,
html.dark .markdown-body h4[id]:hover::before { color: #58a6ff; }
html.dark .markdown-body code {
    background-color: #161b22; color: #e6edf3; border-color: #30363d;
}
html.dark .markdown-body pre {
    background-color: #161b22; border-color: #30363d;
}
html.dark .markdown-body pre code { background: transparent; }
html.dark .markdown-body blockquote { border-color: #30363d; color: #8b949e; }
html.dark .markdown-body table th { background-color: #161b22; border-color: #30363d; color: #e6edf3; }
html.dark .markdown-body table td { border-color: #30363d; }
html.dark .markdown-body tr:nth-child(2n) { background-color: #161b22; }
html.dark .markdown-body hr { background-color: #30363d; }
html.dark .markdown-body img { opacity: .85; }
html.dark .markdown-body del { color: #8b949e; }
html.dark .markdown-body details { border-color: #30363d; }
html.dark .markdown-body details summary { color: #e6edf3; background: #161b22; }
html.dark .markdown-body .callout { border-color: #30363d; background: #161b22; }
html.dark .markdown-body .callout-title { color: #e6edf3; }
html.dark footer { border-color: #30363d; color: #8b949e; }
html.dark footer a { color: #58a6ff; }
html.dark ::selection { background-color: #1f3a5f; }

/* Responsive */
@media (max-width: 768px) {
    .container { padding: 12px 16px 24px; }
    .markdown-body h1 { font-size: 1.5em; }
    .markdown-body h2 { font-size: 1.25em; }
    .markdown-body pre { font-size: 80%; padding: 10px 12px; }
}

/* Print */
@media print {
    .breadcrumb, footer, .toolbar { display: none; }
    .markdown-body details { border: none; }
    .markdown-body details > summary { display: none; }
    .markdown-body details > * { display: block !important; }
    .markdown-body a { color: inherit; text-decoration: underline; }
    .markdown-body a::after { content: " (" attr(href) ")"; font-size: 80%; color: #666; }
}
"##;
