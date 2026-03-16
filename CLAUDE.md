# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Setup After Clone

```bash
git config core.hooksPath .githooks               # enable pre-commit hook (fmt + clippy + tests)
```

## Build & Test Commands

```bash
cargo fmt                                         # auto-format code (always run before build)
cargo build                                       # debug build
cargo build --release                             # release build (~11s)
cargo test                                        # unit tests + offline integration tests (~300 tests)
cargo test --test integration_crawl -- --ignored --test-threads=1  # network integration tests (crawls crawler.siteone.io)
cargo test scoring::ci_gate::tests::all_checks_pass  # run a single test by name
cargo clippy -- -D warnings                       # lint (CI enforces zero warnings)
cargo fmt -- --check                              # format check
```

## Quick Run

```bash
./target/release/siteone-crawler --url=https://example.com --single-page
./target/release/siteone-crawler --url=https://example.com --output=json --http-cache-dir=  # no cache
```

## Architecture

### Crawl Lifecycle (in order)

1. **CLI Parsing** (`Initiator` → `CoreOptions::parse_argv()`): Parses 120+ CLI options, merges config file if present, validates. Exits with code 101 on error, code 2 on `--help`/`--version`.

2. **Analyzer Registration** (`Initiator::register_analyzers()`): Creates all 15 analyzer instances (Accessibility, BestPractice, Caching, ContentType, DNS, ExternalLinks, Fastest, Headers, Page404, Redirects, Security, SeoAndOpenGraph, SkippedUrls, Slowest, SourceDomains, SslTls) and registers them with `AnalysisManager`. Some analyzers receive config from CLI options (e.g. `fastest_top_limit`, `max_heading_level`).

3. **Manager Setup** (`Manager::run()`): Creates `Status` (result storage), `Output` (text/json/multi), `HttpClient` (with optional proxy, auth, cache), `ContentProcessorManager` (HTML, CSS, JS, XML, Astro, Next.js, Svelte processors), and the `Crawler` instance.

4. **Robots.txt Fetch** (`Crawler::fetch_robots_txt()`): Before crawling starts, fetches and parses `/robots.txt` from the initial domain. Respects `--ignore-robots-txt` option.

5. **Crawl Loop** (`Crawler::run()`): Breadth-first concurrent URL processing:
   - URL queue (`DashMap`) seeded with initial URL
   - Tokio tasks limited by `Semaphore` (= `--workers` count) + rate limiting (`--max-reqs-per-sec`)
   - Per-URL flow: check robots.txt → HTTP request → on error, store with negative status code → on success, run content processors → extract links from HTML → enqueue discovered URLs
   - Content processors (`HtmlProcessor`, `CssProcessor`, etc.) transform response bodies during crawl — used by offline/markdown exporters for URL rewriting
   - Each visited URL's response is stored in `Status` for post-crawl analysis
   - Per-URL data collected: status code, headers, body, response time, content type, size, redirects

6. **Post-Crawl Analysis** (`Manager::run_post_crawl()`): Sequential pipeline after crawling ends:
   - Transfer skipped URLs from crawler to `Status`
   - Run all registered analyzers (`AnalysisManager::run_analyzers()`): each analyzer gets read access to `Status` (all crawled data) and write access to `Output` (adds tables/findings)
   - Add content processor stats table

7. **Exporters** (`Manager::run_exporters()`): Generate output files based on CLI options:
   - `SitemapExporter`: XML/TXT sitemap files
   - `OfflineWebsiteExporter`: Static website copy with rewritten relative URLs
   - `MarkdownExporter`: HTML→Markdown conversion with relative .md links
   - `FileExporter`: Save text/JSON output to file
   - `HtmlReport`: Self-contained HTML report (also used by Mailer and Upload)
   - `MailerExporter`: Email HTML report via SMTP
   - `UploadExporter`: Upload report to remote server

8. **Scoring** (`scorer::calculate_scores()`): Computes quality scores (0–10) across 5 weighted categories (Performance 20%, SEO 20%, Security 25%, Accessibility 20%, Best Practices 15%). Deductions come from summary findings (criticals, warnings) and stats (404s, 5xx, slow responses).

9. **CI/CD Gate** (`ci_gate::evaluate()`): When `--ci` is active, checks scores and stats against configurable thresholds (`--ci-min-score`, `--ci-max-404`, etc.). Returns exit code 10 on failure.

10. **Summary & Output** (`Output::add_summary()`, `Output::end()`): Prints summary table with OK/Warning/Critical counts, finalizes output. Exit code: 0 = success, 3 = no pages crawled, 10 = CI gate failed.

### How Analyzers Work

Each analyzer implements the `Analyzer` trait (`analysis/analyzer.rs`). Analyzers are **post-crawl only** — they don't run during crawling. The `AnalysisManager` calls each analyzer's `analyze(&Status, &mut Output)` method after all URLs have been visited. Analyzers read crawled data from `Status` (visited URLs, response headers, bodies, skipped URLs) and produce `SuperTable` instances that get added to `Output`. Analyzers also add `Item` entries to the `Summary` (OK, Warning, Critical, Info findings) which feed into scoring.

### How Content Processors Work

Content processors implement `ContentProcessor` (`content_processor/content_processor.rs`) and run **during crawl** on each URL's response body. They serve two purposes: (1) transform content for offline/markdown export (rewrite URLs to relative paths), and (2) extract metadata (links, assets). Processors are type-specific: `HtmlProcessor` handles HTML, `CssProcessor` handles CSS `url()` references, etc. The `ContentProcessorManager` dispatches to the right processor based on content type.

### Concurrency Model

The crawler uses tokio for async I/O with a semaphore-based worker pool (`options.workers`). Shared state uses:
- `Arc<DashMap<...>>` for lock-free concurrent maps (URL queue, visited URLs, skipped URLs)
- `Arc<Mutex<...>>` for sequential-access state (Status, Output, AnalysisManager)
- `Arc<AtomicBool/AtomicUsize>` for simple flags and counters

### Key Traits

- **`Analyzer`** (`analysis/analyzer.rs`): Post-crawl analysis (SEO, security, headers, etc.). Each analyzer gets `&Status` and `&mut Output`.
- **`Exporter`** (`export/exporter.rs`): Output generators (HTML report, offline website, markdown, sitemap, mailer, upload).
- **`Output`** (`output/output.rs`): Formatting backend. Implementations: `TextOutput`, `JsonOutput`, `MultiOutput`.
- **`ContentProcessor`** (`content_processor/content_processor.rs`): Per-URL content transformation during crawl (HTML, JS, CSS, XML processors).

### Options System

CLI options are defined in `options/core_options.rs` via `get_options()` which returns an `Options` struct with typed option groups. Parsing flow: `parse_argv()` → merge config file → parse flags → `CoreOptions::from_options()` → `apply_option_value()` for each option. New CLI options require: adding the field to `CoreOptions`, a case in `apply_option_value()`, and an entry in the appropriate option group.

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (with `--ci`: all thresholds passed) |
| 1 | Runtime error |
| 2 | Help/version displayed |
| 3 | No pages successfully crawled (DNS failure, timeout, etc.) |
| 10 | CI/CD quality gate failed |
| 101 | Configuration error |

### HTTP Response Body

`HttpResponse.body` is `Option<Vec<u8>>` (not String) to preserve binary data for images, fonts, etc. Use `body_text()` for string content. Failed HTTP requests return `Ok(HttpResponse)` with negative status codes (-1 connection error, -2 timeout, -4 send error), not `Err`.

### Testing Structure

- **Unit tests**: In-file `#[cfg(test)] mod tests` blocks (standard Rust convention)
- **Integration tests**: `tests/integration_crawl.rs` with shared helpers in `tests/common/mod.rs`
- Network-dependent integration tests are `#[ignore]` — run explicitly with `--ignored`

### Key Files

- `src/engine/crawler.rs` (~1700 lines): Core crawl loop, URL queue management, HTML/content parsing
- `src/options/core_options.rs` (~2500 lines): All 120+ CLI options, parsing, validation
- `src/export/utils/offline_url_converter.rs` (~1400 lines): URL-to-file-path conversion for offline export
- `src/export/html_report/report.rs`: HTML report generation with embedded template
- `src/scoring/scorer.rs`: Quality score calculation from summary findings
- `src/scoring/ci_gate.rs`: CI/CD threshold evaluation

### Edition & Rust Version

Project uses `edition = "2024"` (Rust 1.85+) with `rust-version = "1.94"`. Edition 2024 features used throughout: `unsafe extern` blocks, `if let` chaining (`if let ... && ...`), `unsafe { std::env::set_var() }`.

### Commit Policy

**Never commit automatically.** Commits are only allowed on explicit user request. Before every commit, always run `git status`, review the changes, and stage only the relevant files — never use `git add -A` or `git add .` blindly.

### Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `refactor:`, `perf:`, `docs:`, `style:`, `ci:`, `chore:`, `test:`. Examples:
- `feat: add built-in HTTP server for markdown/offline exports`
- `fix: correct non-ASCII text corruption in heading ID generation`
- `perf: eliminate heap allocation in content_type_for_extension`
- `chore: bump version to 2.0.3`

### Releasing a New Version

1. Update version in `Cargo.toml` (`version = "X.Y.Z"`)
2. Update version in `src/version.rs` (`pub const CODE: &str = "X.Y.Z.YYYYMMDD";`)
3. Commit: `git commit -m "Bump version to X.Y.Z"`
4. Tag and push: `git tag vX.Y.Z && git push && git push --tags`

### Important Conventions

- Tables, column order, and formatting must stay consistent across versions. The HTML parser uses the `scraper` crate.
- HTTP cache lives in `tmp/http-client-cache/` by default. Delete it for fresh crawls or use `--http-cache-dir=` to disable.
- `rustls` requires explicit `ring` CryptoProvider installation in `main.rs`.
