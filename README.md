# SiteOne Crawler

SiteOne Crawler is a powerful and easy-to-use **website analyzer, cloner, and converter** designed for developers seeking security and performance insights, SEO specialists identifying optimization opportunities, and website owners needing reliable backups and offline versions.

**Now rewritten in Rust** for maximum performance, minimal resource usage, and zero runtime dependencies. The transition from PHP+Swoole to Rust resulted in **25% faster execution** and **30% lower memory consumption** while producing identical output.

**Discover the SiteOne Crawler advantage:**

*   **Run Anywhere:** Single native binary for **🪟 Windows**, **🍎 macOS**, and **🐧 Linux** (x64 & arm64). No runtime dependencies.
*   **Work Your Way:** Launch the binary without arguments for an **interactive wizard** 🧙 with 11 preset modes, use the extensive **command-line interface** 📟 ([releases](https://github.com/janreges/siteone-crawler/releases), [▶️ video](https://www.youtube.com/watch?v=25T_yx13naA&list=PL9mElgTe-s1Csfg0jXWmDS0MHFN7Cpjwp)) for automation and power, or enjoy the intuitive **desktop GUI application** 💻 ([GUI app](https://github.com/janreges/siteone-crawler-gui), [▶️ video](https://www.youtube.com/watch?v=rFW8LNEVNdw)) for visual control.
*   **Rich Output Formats:** Interactive **HTML audit report** 📊 with sortable tables and quality scoring (0.0-10.0) (see [nextjs.org sample](https://crawler.siteone.io/html/2024-08-23/forever/cl8xw4r-fdag8wg-44dd.html)), detailed **JSON** for programmatic consumption, and human-readable **text** for terminal. Send HTML reports directly to your inbox via **built-in SMTP mailer** 📧.
*   **CI/CD Integration:** Built-in **quality gate** (`--ci`) with configurable thresholds — exit code 10 on failure enables automated deployment blocking. Also useful for **cache warming** — crawling the entire site after deployment populates your reverse proxy/CDN cache.
*   **Offline & Markdown Power:** Create complete **offline clones** 💾 for browsing without a server ([nextjs.org clone](https://crawler.siteone.io/examples-exports/nextjs.org/)) or convert entire websites into clean **Markdown** 📝 — perfect for backups, documentation, or feeding content to AI models ([examples](https://github.com/janreges/siteone-crawler-markdown-examples/)).
*   **Deep Crawling & Analysis:** Thoroughly crawl every page and asset, identify errors (404s, redirects), generate **sitemaps** 🗺️, and even get **email summaries** 📧 (watch [▶️ video example](https://www.youtube.com/watch?v=PHIFSOmk0gk)).
*   **Learn More:** Dive into the 🌐 [Project Website](https://crawler.siteone.io/), explore the detailed [Documentation](https://crawler.siteone.io/configuration/command-line-options/), or check the [JSON](docs/JSON-OUTPUT.md)/[Text](docs/TEXT-OUTPUT.md) output specs.

GIF animation of the crawler in action (also available as a [▶️ video](https://www.youtube.com/watch?v=25T_yx13naA&list=PL9mElgTe-s1Csfg0jXWmDS0MHFN7Cpjwp)):

![SiteOne Crawler](docs/siteone-crawler-command-line.gif)

## Table of contents

- [✨ Features](#-features)
    * [🕷️ Crawler](#️-crawler)
    * [🛠️ Dev/DevOps assistant](#️-devdevops-assistant)
    * [📊 Analyzer](#-analyzer)
    * [📧 Reporter](#-reporter)
    * [💾 Offline website generator](#-offline-website-generator)
    * [📝 Website to markdown converter](#-website-to-markdown-converter)
    * [🗺️ Sitemap generator](#️-sitemap-generator)
    * [🤖 AI assistant (optional)](#-ai-assistant-optional)
    * [🌐 Browser rendering (optional)](#-browser-rendering-optional)
- [🚀 Installation](#-installation)
    * [📦 Pre-built binaries](#-pre-built-binaries)
    * [🍺 Homebrew (macOS / Linux)](#-homebrew-macos--linux)
    * [🐧 Debian / Ubuntu (apt)](#-debian--ubuntu-apt)
    * [🎩 Fedora / RHEL (dnf)](#-fedora--rhel-dnf)
    * [🦎 openSUSE / SLES (zypper)](#-opensuse--sles-zypper)
    * [🏔️ Alpine Linux (apk)](#️-alpine-linux-apk)
    * [🔨 Build from source](#-build-from-source)
- [▶️ Usage](#️-usage)
    * [Interactive wizard](#interactive-wizard)
    * [Basic example](#basic-example)
    * [URL list example](#url-list-example)
    * [CI/CD example](#cicd-example)
    * [Fully-featured example](#fully-featured-example)
    * [⚙️ Arguments](#️-arguments)
        + [Basic settings](#basic-settings)
        + [Output settings](#output-settings)
        + [Resource filtering](#resource-filtering)
        + [Advanced crawler settings](#advanced-crawler-settings)
        + [File export settings](#file-export-settings)
        + [Mailer options](#mailer-options)
        + [Upload options](#upload-options)
        + [Offline exporter options](#offline-exporter-options)
        + [Markdown exporter options](#markdown-exporter-options)
        + [Sitemap options](#sitemap-options)
        + [Expert options](#expert-options)
        + [Fastest URL analyzer](#fastest-url-analyzer)
        + [SEO and OpenGraph analyzer](#seo-and-opengraph-analyzer)
        + [Slowest URL analyzer](#slowest-url-analyzer)
        + [Built-in HTTP server](#built-in-http-server)
        + [HTML-to-Markdown conversion](#html-to-markdown-conversion)
        + [CI/CD settings](#cicd-settings)
        + [🤖 AI assistant (optional)](#-ai-assistant-optional-1)
        + [🌐 Browser rendering (optional)](#-browser-rendering-optional-1)
- [🏆 Quality Scoring](#-quality-scoring)
- [🔄 CI/CD Integration](#-cicd-integration)
- [📄 Output Examples](#-output-examples)
- [🧪 Testing](#-testing)
- [⚠️ Disclaimer](#️-disclaimer)
- [📜 License](#-license)

## ✨ Features

In short, the main benefits can be summarized in these points:

- **🕷️ Crawler** - very powerful crawler of the entire website reporting useful information about each URL (status code,
  response time, size, custom headers, titles, etc.)
- **🛠️ Dev/DevOps assistant** - offers stress/load testing with configurable concurrent workers (`--workers`) and request
  rate (`--max-reqs-per-sec`), cache warming, localhost testing, and rich URL/content-type filtering
- **📊 Analyzer** - analyzes all webpages and reports strange or error behaviour and useful statistics (404, redirects, bad
  practices, SEO and security issues, heading structures, etc.)
- **📧 Reporter** - interactive **HTML audit report**, structured **JSON**, and colored **text** output; built-in
  **SMTP mailer** sends HTML reports directly to your inbox
- **💾 Offline website generator** - clone entire websites to browsable local HTML files (no server needed) including all
  assets. Supports **multi-domain clones** — include subdomains or external domains with intelligent cross-linking.
- **📝 Website to markdown converter** - export the entire website to browsable text markdown (viewable on GitHub or any
  text editor), or generate a **single-file markdown** with smart header/footer deduplication — ideal for **feeding to AI
  tools**. Includes a **built-in web server** that renders markdown exports as styled HTML pages.
  Also supports **standalone HTML-to-Markdown conversion** of local files (`--html-to-markdown`).
  See [markdown examples](https://github.com/janreges/siteone-crawler-markdown-examples/).
- **🗺️ Sitemap generator** - allows you to generate `sitemap.xml` and `sitemap.txt` files with a list of all pages on your
  website
- **🏆 Quality scoring** - automatic quality scoring (0.0-10.0) across 5 categories: Performance, SEO, Security, Accessibility, Best Practices
- **🔄 CI/CD quality gate** - configurable thresholds with exit code 10 on failure for automated pipelines; also
  useful as a **post-deployment cache warmer** for reverse proxies and CDNs

The following features are summarized in greater detail:

### 🕷️ Crawler

- **all major platforms** supported without dependencies (🐧 Linux, 🪟 Windows, 🍎 macOS, arm64) — single native binary
- has incredible **🚀 native Rust performance** with async I/O and multi-threaded crawling
- provides simulation of **different device types** (desktop/mobile/tablet) thanks to predefined User-Agents
- will crawl **all files**, styles, scripts, fonts, images, documents, etc. on your website
- will respect the `robots.txt` file and will not crawl the pages that are not allowed
- has a **beautiful interactive** and **🎨 colourful output**
- it will **clearly warn you** ⚠️ of any wrong use of the tool (e.g. input parameters validation or wrong permissions)
- as `--url` parameter, you can specify also a `sitemap.xml` file (or [sitemap index](https://www.sitemaps.org/protocol.html#index)),
  which will be processed as a list of URLs. Such a URL may end in `.xml`, `.xml.gz` or any other `.gz`,
  or be served as `application/gzip` / `application/x-gzip`; gzip-compressed sitemaps are decompressed.
  A gzip file counts as a sitemap only when its XML root element is `<urlset>` or `<sitemapindex>`;
  any other `.gz` download (e.g. a `.tar.gz` archive) keeps its original bytes.
  Entries of a sitemap index may point to `.xml`, `.xml.gz` or `.gz` files (not `.tar.gz`), also with a
  query string (e.g. Shopify's `sitemap_products_1.xml?from=1&to=100`). When the URL path contains
  `sitemap` and ends in `.xml` or `.gz` (e.g. `/sitemap.xml`, `/sitemap-products.gz`), the crawler runs
  in sitemap-only mode: it follows only URLs from the sitemap and does not discover additional links
  from HTML pages.
- with `--url-list=<file>` you can crawl a **bounded list of URLs** from a plain-text file (one URL per line).
  The first URL in the file becomes the crawl base when `--url` is omitted. Combine it with `--single-page`
  to crawl exactly the listed URLs without discovering additional links.
- respects the HTML `<base href>` tag when resolving relative URLs on pages that use it.

### 🛠️ Dev/DevOps assistant

- allows testing **public** and **local projects on specific ports** (e.g. `http://localhost:3000/`)
- works as a **stress/load tester** — configure the number of **concurrent workers** (`--workers`) and the **maximum
  requests per second** (`--max-reqs-per-sec`) to simulate various traffic levels and test your infrastructure's
  resilience against high load or DoS scenarios
- combine with **rich filtering options** — include/ignore URLs by regex (`--include-regex`, `--ignore-regex`), disable
  specific asset types (`--disable-javascript`, `--disable-images`, etc.), or limit crawl depth (`--max-depth`) to focus
  the load on specific parts of your website
- will help you **warm up the application cache** or the **cache on the reverse proxy** of the entire website

### 📊 Analyzer

- will **find the weak points** or **strange behavior** of your website
- built-in analyzers cover SEO, security headers, accessibility, best practices, performance, SSL/TLS, caching, and more
- **technology detection** — the **Technologies** table lists the stack a site reveals: web server, CDN, WAF / bot
  protection, hosting platform, CMS, e-commerce platform, backend and frontend frameworks, JS libraries (with versions
  where visible), analytics / tag managers and fonts / UI kits. Detection is passive — response headers, cookie names,
  `<meta name="generator">`, script URLs and a few HTML markers of the crawled pages, no extra requests — so a
  technology missing from the table may still be in use. Available in text, JSON (`tables.technologies`) and the HTML
  report.

### 📧 Reporter

Three output formats:

- **Interactive HTML report** — a self-contained `.html` file with sortable tables, quality scores, color-coded
  findings, and sections for SEO, security, accessibility, performance, headers, redirects, 404s, and more. Open it
  in any browser — no server needed.
- **JSON output** — structured data with all crawled URLs, response details, analysis findings, scores, and CI/CD gate
  results. Ideal for programmatic consumption, dashboards, and integrations.
- **Text output** — human-readable colored terminal output with tables, progress bars, and summaries.

Additional reporting features:

- **Built-in SMTP mailer** — send the HTML audit report directly to one or more email addresses via your own SMTP
  server. Configure sender, recipients, subject template, and SMTP credentials via CLI options.
- **Large reports stay responsive** — above 1,000 items the Visited URLs table and the Image Gallery of the HTML
  report are paged in the browser (100/500/1000 items per page; sorting, fulltext search and the gallery filters work
  on all items). The report stays one self-contained file; without JavaScript the first 100 items are shown.
- will provide you with data for **SEO analysis**, just add the `Title`, `Keywords` and `Description` extra columns
- will provide useful **summaries and statistics** at the end of the processing

### 💾 Offline website generator

- will help you **export the entire website** to offline form, where it is possible to browse the site through local
  HTML files (without HTTP server) including all documents, images, styles, scripts, fonts, etc.
- supports **multi-domain clones** — include subdomains (`*.mysite.tld`) or entirely different domains in a single
  offline export. All URLs across included domains are **intelligently rewritten to relative paths**, so the resulting
  offline version cross-links pages between domains seamlessly — you get one unified browsable clone.
- you can **limit what assets** you want to download and export (see `--disable-*` directives) .. for some types of
  websites the best result is with the `--disable-javascript` option.
- you can specify by `--allowed-domain-for-external-files` (short `-adf`) from which **external domains** it is possible
  to **download** assets (JS, CSS, fonts, images, documents) including `*` option for all domains.
- you can specify by `--allowed-domain-for-crawling` (short `-adc`) which **other domains** should be included in the
  **crawling** if there are any links pointing to them. You can enable e.g. `mysite.*` to export all language mutations
  that have a different TLD or `*.mysite.tld` to export all subdomains.
- you can use `--single-page` to **export only one page** to which the URL is given (and its assets), but do not follow
  other pages.
- you can use `--single-foreign-page` to **export only one page** from another domain (if allowed by `--allowed-domain-for-crawling`),
  but do not follow other pages.
- you can use `--replace-content` to **replace content** in HTML/JS/CSS with `foo -> bar` or regexp in PCRE format, e.g.
  `/card[0-9]/i -> card`. Can be specified multiple times.
- you can use `--replace-query-string` to **replace chars in query string** in the filename.
- you can use `--max-depth` to set the **maximum crawling depth** (for pages, not assets). `1` means `/about` or `/about/`,
  `2` means `/about/contacts` etc.
- you can use it to **export your website to a static form** and host it on GitHub Pages, Netlify, Vercel, etc. as a
  static backup and part of your **disaster recovery plan** or **archival/legal needs**
- works great with **older conventional websites** but also **modern ones**, built on frameworks like Next.js, Nuxt.js,
  SvelteKit, Astro, Gatsby, etc. When a JS framework is detected, the export also performs some framework-specific code
  modifications for optimal results.
- **try it** for your website, and you will be very pleasantly surprised :-)

### 📝 Website to markdown converter

Two export modes:

- **Multi-file markdown** — exports the entire website with all subpages to a directory of **browsable `.md` files**.
  The markdown renders nicely when uploaded to GitHub, viewed in VS Code, or any text editor. Links between pages are
  converted to relative `.md` links so you can navigate between files. Optionally includes images and other files
  (PDF, etc.).
- **Single-file markdown** — combines all pages into **one large markdown file** with smart removal of duplicate website
  headers and footers across pages. Ideal for **feeding entire website content to AI tools** (ChatGPT, Claude, etc.)
  that process markdown more effectively than raw HTML.

Smart conversion features:

- **collapsible accordions** — large link lists (menus, navigation, footer links with 8+ items) are automatically
  collapsed into `<details>` accordions with contextual labels ("Menu", "Links") for better readability
- content before the main heading (typically h1) — such as the site header and navigation — is moved to the end of the
  page below a `---` separator, so the actual page content comes first
- you can set multiple selectors (CSS-like) to **remove unwanted elements** from the exported markdown
- **code block detection** and **syntax highlighting** for popular programming languages
- HTML tables are converted to proper **markdown tables**

Built-in web server:

- use `--serve-markdown=<dir>` to start a **built-in HTTP server** that renders your markdown export as styled HTML
  pages with tables, dark/light mode, breadcrumb navigation, and accordion support — perfect for browsing and sharing
  the export locally or on a network

Standalone HTML-to-Markdown conversion:

- use `--html-to-markdown=<file>` to convert a **local HTML file** directly to Markdown without crawling any website
- outputs clean Markdown to **stdout** (pipe-friendly) or to a file with `--html-to-markdown-output=<file>`
- uses the same conversion pipeline as `--markdown-export-dir` — including all cleanup, accordion collapsing, code language detection, and implicit exclusions (cookie banners, `aria-hidden` elements, `role="menu"` dropdowns)
- respects `--markdown-disable-images`, `--markdown-disable-files`, `--markdown-exclude-selector`, and `--markdown-move-content-before-h1-to-end`
- does **not** rewrite links (`.html` → `.md`) since the file is standalone with no site context

💡 Tip: you can push the exported markdown folder to your GitHub repository, where it will be automatically rendered as a browsable
documentation. You can look at the [examples](https://github.com/janreges/siteone-crawler-markdown-examples/) of converted websites to markdown.

See all available [markdown exporter options](#markdown-exporter-options) and [HTML-to-Markdown conversion options](#html-to-markdown-conversion).

### 🗺️ Sitemap generator

- will help you create a `sitemap.xml` and `sitemap.txt` for your website
- you can set the priority of individual pages based on the number of slashes in the URL
- `<lastmod>` is filled from each page's `Last-Modified` response header (written in UTC) and left
  out when the header is missing, implausible (before 1995 or in the future) or only stamps the time
  of the response, as dynamic pages do —
  Google uses `lastmod` only when it is consistently and verifiably accurate
- `--sitemap-changefreq` adds the same `<changefreq>` to every URL (Google ignores `changefreq` and
  `priority`; other search engines may use them)
- a `--sitemap-xml-file` path ending in `.xml.gz` writes a gzip-compressed sitemap

### 🤖 AI assistant (optional)

- optional, opt-in LLM integration — works with OpenAI, Anthropic, Google Gemini, and any OpenAI-compatible endpoint (vLLM, LiteLLM, MiniMax, Ollama, self-hosted)
- AI SEO analysis with concrete title/description/keyword rewrites, [`llms.txt`](https://llmstxt.org/) generation, spelling/grammar checks, and your own custom policy prompts
- smart page selection (ranks the most important pages) plus hard caps and a `--ai-dry-run` cost preview so it never blindly hits thousands of pages
- safe API-key handling (env-var by default, redacted from logs) and no extra binary dependencies — see [🤖 AI assistant options](#-ai-assistant-optional-1)

Don't hesitate and try it. You will love it as we do! ❤️

### 🌐 Browser rendering (optional)

- optional, opt-in mode (`--browser`) that renders each page in a **real Chromium** via the Chrome DevTools Protocol, so **JavaScript / SPA sites** are crawled with their post-render DOM (client-side links, hydrated content, framework markup) — link extraction, offline export and markdown export then all see the rendered page
- **screenshots** of every page — viewport (custom resolution) or **full-page** (entire scroll height), as PNG/JPG/WebP; animations are settled before each capture so pages look loaded, not mid-effect
- **screenshot extras** — assemble the per-page screenshots into a **GIF/MP4 animation** (GIF built-in, MP4 via ffmpeg), and optionally **hide cookie-consent banners** before capture
- **console / error diagnostics** per page — JavaScript console errors, uncaught exceptions, failed sub-requests (404/5xx), CSP/CORS/mixed-content violations — reported in a table and feedable to the AI assistant
- **headless by default**, or `--browser-headful` to watch the browser open each page
- **easiest possible setup, no Node.js**: it auto-detects an installed Chrome/Chromium/Edge/Brave, and if none is found it offers to download a `chrome-headless-shell` build — or point it at any browser with `--browser-path`
- **built into the default build and pre-built binaries** (adds the ~6 MB CDP client, not a browser — the actual Chromium is detected/downloaded at runtime) — see [🌐 Browser rendering (optional) usage & options](#-browser-rendering-optional-1)

> **Limitations of browser mode** (by design): the browser loads sub-resources and runs page JS, so domain-scope/robots rules apply to the top document only; `--http-cache-dir` does not cache rendered bodies (the browser always fetches live), and each rendered HTML page is fetched twice (once for status/headers, once by the browser); HTTP auth (`--http-auth`), custom `--header` values and cookies are not forwarded to the browser; the auto-download trusts Google's CDN over TLS. `--proxy` and `--resolve` are forwarded to the browser, but `--resolve` is applied **host-only** in browser mode (Chrome's host-resolver-rules ignore the port), so per-port overrides for the same host aren't honored by the browser the way they are on the HTTP path.

## 🚀 Installation

### 📦 Pre-built binaries

Download pre-built binaries from [🐙 GitHub releases](https://github.com/janreges/siteone-crawler/releases) for all major platforms (🐧 Linux, 🪟 Windows, 🍎 macOS, x64 & arm64).

The binary is self-contained — no runtime dependencies required.

```bash
# Linux / macOS — download, extract, run
./siteone-crawler --url=https://my.domain.tld
```

**🐧 Linux binary variants:**

For Linux, two binary variants are provided:

| Variant | Compatibility | Performance |
|---------|--------------|-------------|
| **glibc** (primary) | Requires glibc 2.39+ (Ubuntu 24.04+, Debian 13+, Fedora 40+) | Full native performance |
| **musl** (compatible) | Any Linux distribution (statically linked, no dependencies) | ~50–80% slower due to musl memory allocator |

The **glibc** variant is recommended for current distributions — it offers the best performance. If you are running an older distribution (e.g. Ubuntu 22.04, Debian 12) and encounter a `GLIBC_2.xx not found` error, use the **musl** variant instead. The musl binary is fully statically linked and runs on any Linux system regardless of the installed glibc version. The performance difference is mainly noticeable during CPU-intensive operations like offline and markdown exports.

**Note for macOS users**: In case that Mac refuses to start the crawler from your Download folder, move the entire folder with the Crawler **via the terminal** to another location, for example to the homefolder `~`.

### 🍺 Homebrew (macOS / Linux)

```bash
brew install janreges/tap/siteone-crawler
siteone-crawler --url=https://my.domain.tld
```

### 🐧 Debian / Ubuntu (apt)

```bash
curl -1sLf 'https://dl.cloudsmith.io/public/janreges/siteone-crawler/setup.deb.sh' | sudo -E bash
sudo apt-get install siteone-crawler
```

> **Older distributions (Ubuntu 22.04, Debian 11/12, etc.):** If you get a `GLIBC_X.XX not found` error, install the statically linked variant instead:
> ```bash
> sudo apt-get install siteone-crawler-static
> ```
> See [Linux binary variants](#-pre-built-binaries) for details on the performance difference.

### 🎩 Fedora / RHEL (dnf)

```bash
curl -1sLf 'https://dl.cloudsmith.io/public/janreges/siteone-crawler/setup.rpm.sh' | sudo -E bash
sudo dnf install siteone-crawler
```

> **Older distributions:** If you get a `GLIBC_X.XX not found` error, use `sudo dnf install siteone-crawler-static` instead.
> See [Linux binary variants](#-pre-built-binaries) for details.

### 🦎 openSUSE / SLES (zypper)

```bash
curl -1sLf 'https://dl.cloudsmith.io/public/janreges/siteone-crawler/setup.rpm.sh' | sudo -E bash
sudo zypper install siteone-crawler
```

> **Older distributions:** If you get a `GLIBC_X.XX not found` error, use `sudo zypper install siteone-crawler-static` instead.
> See [Linux binary variants](#-pre-built-binaries) for details.

### 🏔️ Alpine Linux (apk)

```bash
curl -1sLf 'https://dl.cloudsmith.io/public/janreges/siteone-crawler/setup.alpine.sh' | sudo -E bash
sudo apk add siteone-crawler
```

### 🔨 Build from source

Requires [Rust](https://www.rust-lang.org/tools/install) 1.94 or later (see `rust-version` in `Cargo.toml`).

```bash
git clone https://github.com/janreges/siteone-crawler.git
cd siteone-crawler

# Build optimized release binary
cargo build --release

# Run
./target/release/siteone-crawler --url=https://my.domain.tld
```

**Browser rendering is built into the default build and the pre-built binaries** (it adds the
~6 MB chromiumoxide CDP client; the actual browser is detected/downloaded at runtime, never
bundled). Just use `--browser`:

```bash
cargo build --release
# A browser (Chrome/Chromium/Edge/Brave) is detected automatically at runtime, downloaded on
# first use, or pointed at via --browser-path=<exe>.
./target/release/siteone-crawler --url=https://my.spa.tld --browser --screenshots
```

**Lean build without browser rendering** (drops chromiumoxide, ~6 MB smaller):

```bash
cargo build --release --no-default-features
```

**Build statically linked (musl) binary:**

```bash
# Install musl toolchain (Ubuntu/Debian)
sudo apt-get install musl-tools
rustup target add x86_64-unknown-linux-musl

# Build static binary (no system dependencies)
cargo build --release --target x86_64-unknown-linux-musl

# Run — works on any Linux distribution
./target/x86_64-unknown-linux-musl/release/siteone-crawler --url=https://my.domain.tld
```

## ▶️ Usage

### Interactive wizard

Run the binary **without any arguments** and an interactive wizard will guide you through the
configuration. Choose from 11 preset modes, enter the target URL, fine-tune settings with
arrow keys, and the crawler starts immediately — no need to remember CLI flags.

```
? Choose a crawl mode:
❯ Quick Audit               Fast site health overview — crawls all pages and assets
  SEO Analysis               Extract titles, descriptions, keywords, and OpenGraph tags
  Performance Test           Measure response times with cache disabled — find bottlenecks
  Security Check             Check SSL/TLS, security headers, and redirects site-wide
  Offline Clone              Download entire website with all assets for offline browsing
  Markdown Export            Convert pages to Markdown for AI models or documentation
  Stress Test                High-concurrency load test with cache-busting random params
  Single Page                Deep analysis of a single URL — SEO, security, performance
  Large Site Crawl           High-throughput HTML-only crawl for large sites (100k+ pages)
  Sitemap Generator          Crawl pages only and write XML and TXT sitemap files
  Custom                     Start from defaults and configure every option manually
  ──────────────────────────────────────
  Browse offline export      Serve a previously exported offline site via HTTP
  Browse markdown export     Serve a previously exported markdown site via HTTP
[↑↓ to move, enter to select, type to filter]
```

After selecting a preset and entering the URL, the wizard shows a settings form where you can
adjust workers, timeout, content types, export options, and more. A configuration summary with the
equivalent CLI command is displayed before the crawl starts — copy it for future use without the
wizard.

If existing offline or markdown exports are detected in `./tmp/`, the wizard also offers to
**serve them via the built-in HTTP server** directly from the menu.

### Basic example

To run the crawler from the command line, provide the required arguments:

```bash
./siteone-crawler --url=https://mydomain.tld/ --device=mobile
```

### URL list example

Crawl exactly a bounded set of URLs listed in a file, without following any discovered links:

```bash
# urls.txt — one URL per line, blank lines and '#' comments ignored
./siteone-crawler --url-list=urls.txt --single-page
```

### CI/CD example

```bash
# Fail deployment if quality score < 7.0 or any 5xx errors
./siteone-crawler --url=https://mydomain.tld/ --ci --ci-min-score=7.0 --ci-max-5xx=0
echo $?  # 0 = pass, 10 = fail
```

### Fully-featured example

```bash
./siteone-crawler --url=https://mydomain.tld/ \
  --output=text \
  --workers=2 \
  --max-reqs-per-sec=10 \
  --memory-limit=2048M \
  --resolve='mydomain.tld:443:127.0.0.1' \
  --timeout=5 \
  --proxy=proxy.mydomain.tld:8080 \
  --http-auth=myuser:secretPassword123 \
  --user-agent="My User-Agent String" \
  --extra-columns="DOM,X-Cache(10),Title(40),Keywords(50),Description(50>),Heading1=xpath://h1/text()(20>),ProductPrice=regexp:/Price:\s*\$?(\d+(?:\.\d{2})?)/i#1(10)" \
  --accept-encoding="gzip, deflate" \
  --url-column-size=100 \
  --max-queue-length=3000 \
  --max-visited-urls=10000 \
  --max-url-length=5000 \
  --max-non200-responses-per-basename=10 \
  --include-regex="/^.*\/technologies.*/" \
  --include-regex="/^.*\/fashion.*/" \
  --ignore-regex="/^.*\/downloads\/.*\.pdf$/i" \
  --analyzer-filter-regex="/^.*$/i" \
  --remove-query-params \
  --keep-query-param=page \
  --add-random-query-params \
  --transform-url="live-site.com -> local-site.local" \
  --transform-url="/cdn\.live-site\.com/ -> local-site.local/cdn" \
  --show-scheme-and-host \
  --do-not-truncate-url \
  --output-html-report=tmp/myreport.html \
  --html-report-options="summary,seo-opengraph,visited-urls,security,redirects" \
  --output-json-file=/dir/report.json \
  --output-text-file=/dir/report.txt \
  --add-timestamp-to-output-file \
  --add-host-to-output-file \
  --offline-export-dir=tmp/mydomain.tld \
  --replace-content='/<foo[^>]+>/ -> <bar>' \
  --ignore-store-file-error \
  --sitemap-xml-file=/dir/sitemap.xml \
  --sitemap-txt-file=/dir/sitemap.txt \
  --sitemap-base-priority=0.5 \
  --sitemap-priority-increase=0.1 \
  --markdown-export-dir=tmp/mydomain.tld.md \
  --markdown-export-single-file=tmp/mydomain.tld.combined.md \
  --markdown-move-content-before-h1-to-end \
  --markdown-disable-images \
  --markdown-disable-files \
  --markdown-remove-links-and-images-from-single-file \
  --markdown-exclude-selector='.exclude-me' \
  --markdown-replace-content='/<foo[^>]+>/ -> <bar>' \
  --markdown-replace-query-string='/([^&]+)=([^&]*)(&|$)/ -> $1-$2_' \
  --mail-to=your.name@my-mail.tld \
  --mail-to=your.friend.name@my-mail.tld \
  --mail-from=crawler@my-mail.tld \
  --mail-from-name="SiteOne Crawler" \
  --mail-subject-template="Crawler Report for %domain% (%date%)" \
  --mail-smtp-host=smtp.my-mail.tld \
  --mail-smtp-port=25 \
  --mail-smtp-user=smtp.user \
  --mail-smtp-pass=secretPassword123 \
  --ci --ci-min-score=7.0 --ci-min-security=8.0
```

## ⚙️ Arguments

For a clearer list, I recommend going to the documentation: 🌐 https://crawler.siteone.io/configuration/command-line-options/

### Basic settings

| Parameter | Description |
|-----------|-------------|
| `--url=<url>` | Required (unless `--url-list` is used). HTTP or HTTPS URL address of the website or sitemap xml<br>to be crawled. Use quotation marks `''` if the URL contains query parameters. |
| `--url-list=<file>` | Path to a plain-text file with one URL per line (blank lines and `#` comments are ignored).<br>Only absolute `http(s)` URLs are accepted; other lines are skipped with a warning.<br>When provided, `--url` is optional and the first valid URL in the file is used as the crawl base.<br>All listed URLs are seeded into the crawl queue. Combine with `--single-page` to crawl<br>exactly the listed URLs without following any discovered links.<br>Listed URLs are fetched directly: `robots.txt` and `--include-regex`/`--ignore-regex` do not apply to them,<br>and they may point to any host (the list can span domains); links discovered from them are still limited to the base domain. |
| `--single-page` | Load only one page to which the URL is given (and its assets), but do not follow other pages. |
| `--max-depth=<int>` | Maximum crawling depth (for pages, not assets). Default is `0` (no limit). `1` means `/about`<br>or `/about/`, `2` means `/about/contacts` etc. |
| `--device=<val>` | Device type for choosing a predefined User-Agent. Ignored when `--user-agent` is defined.<br>Supported values: `desktop`, `mobile`, `tablet`. Default is `desktop`. |
| `--user-agent=<val>` | Custom User-Agent header. Use quotation marks. If specified, it takes precedence over<br>the device parameter. If you add `!` at the end, the siteone-crawler/version will not be<br>added as a signature at the end of the final user-agent. |
| `--timeout=<int>` | Request timeout in seconds. Default is `5`. |
| `--proxy=<host:port>` | HTTP proxy to use in `host:port` format. Host can be hostname, IPv4 or IPv6. |
| `--http-auth=<user:pass>` | Basic HTTP authentication in `username:password` format. Sent only to the start host, its subdomains and its<br>`www.` twin (e.g. `example.com` and `www.example.com`), never to sibling subdomains or other domains, and never<br>over plain `http` when the crawl starts on `https`. For an IP address or `localhost`, only the initial port counts.<br>Tip: responses fetched with credentials are stored in the HTTP cache like any other; use `--http-cache-dir=`<br>(empty) to disable caching for authenticated crawls. |
| `--header=<header>` | Custom HTTP request header in `Name: value` format, e.g. `--header="Cookie: session=abc123"`<br>or `-H "Authorization: Bearer <token>"`. Can be specified multiple times; each occurrence is one<br>header and commas in the value are kept. When a header name repeats, the last value wins, so the command<br>line overrides the config file. Sent only where `--http-auth` is sent (the crawled site, see above), never to<br>other domains. Replaces a default header of the same name (e.g. `User-Agent`, which reports then show);<br>`Host`, `Content-Length` and hop-by-hop headers (`Connection`, `Transfer-Encoding`, …) cannot be set. Values are<br>masked (`Cookie: ***`) in the echoed command and in reports. Not forwarded to the browser in `--browser` mode:<br>there Chromium requests the rendered document with the crawler's own User-Agent (which reports then show), and<br>custom headers, a custom `User-Agent` included, apply only to the crawler's HTTP requests.<br>Tip: when crawling with a login cookie, skip logout links, e.g. `--ignore-regex='logout\|signout'`. Responses<br>fetched with credentials are stored in the HTTP cache like any other; use `--http-cache-dir=` (empty) to disable<br>caching for authenticated crawls. |
| `--config-file=<file>` | Load CLI options from a config file. One option per line, `#` comments allowed.<br>Without this flag, auto-discovers `~/.siteone-crawler.conf` or `/etc/siteone-crawler.conf`.<br>CLI arguments override config file values. |

### Output settings

| Parameter | Description |
|-----------|-------------|
| `--output=<val>` | Output type. Supported values: `text`, `json`. Default is `text`. |
| `--extra-columns=<values>` | Comma delimited list of extra columns added to output table. You can specify HTTP headers<br>(e.g. `X-Cache`), predefined values (`Title`, `Keywords`, `Description`, `DOM`), or custom<br>extraction from text files (HTML, JS, CSS, TXT, JSON, XML, etc.) using XPath or regexp.<br>For custom extraction, use the format `Custom_column_name=method:pattern#group(length)`, where<br>`method` is `xpath` or `regexp`, `pattern` is the extraction pattern, an optional `#group` specifies the<br>capturing group (or node index for XPath) to return (defaulting to the entire match or first node), and an<br>optional `(length)` sets the maximum output length (append `>` to disable truncation).<br>For example, use `Heading1=xpath://h1/text()(20>)` to extract the text of the first H1 element<br>from the HTML document, and `ProductPrice=regexp:/Price:\s*\$?(\d+(?:\.\d{2})?)/i#1(10)`<br>to extract a numeric price (e.g., "29.99") from a string like "Price: $29.99". |
| `--url-column-size=<num>` | Basic URL column width. By default, it is calculated from the size of your terminal window. |
| `--rows-limit=<num>` | Max. number of rows to display in tables with analysis results.<br>Default is `200`. |
| `--timezone=<val>` | Timezone for datetimes in HTML reports and timestamps in output folders/files, e.g. `Europe/Prague`.<br>Default is `UTC`. |
| `--do-not-truncate-url` | In the text output, long URLs are truncated by default to `--url-column-size` so the table does not<br>wrap due to long URLs. With this option, you can turn off the truncation. |
| `--show-scheme-and-host` | On text output, show scheme and host also for origin domain URLs. |
| `--hide-progress-bar` | Hide progress bar visible in text and JSON output for more compact view. |
| `--progress-interval=<int>` | Instead of one table row per URL, print at most one compact progress line every N seconds, printed as URLs<br>finish, and a final one when crawling ends, e.g. `Progress: 22232/29504 (75%) \| 31 URLs/s \| avg 70 ms \| 2xx 22000, 3xx 100, 4xx 120, 5xx 2, err 10 \| 00:12:03`.<br>Rows of failed URLs (4xx/5xx, connection error, timeout, skipped) are still printed as they finish, so the log<br>shows what failed. Keeps CI job logs small (GitLab stops a job log at 4 MB by default). The text report<br>(`--output-text-file`) still contains every row. In JSON mode the progress lines, and a `Failed: <status> <url>` line per failed URL,<br>go to stderr (hidden by `--hide-progress-bar`).<br>Default is `0` (one row per URL); `--ci` sets `10` unless this option is given. |
| `--hide-columns=<list>` | Hide specified columns from the progress table. Comma-separated list of column names:<br>`type`, `time`, `size`, `cache`. Example: `--hide-columns=cache` or `--hide-columns=cache,type`. |
| `--no-color` | Disable colored output. |
| `--force-color` | Force colored output regardless of support detection. |
| `--show-inline-criticals` | Show criticals from the analyzer directly in the URL table. |
| `--show-inline-warnings` | Show warnings from the analyzer directly in the URL table. |

### Resource filtering

| Parameter | Description |
|-----------|-------------|
| `--disable-all-assets` | Disables crawling of all assets and files and only crawls pages in href attributes.<br>Shortcut for calling all other `--disable-*` flags. |
| `--disable-javascript` | Disables JavaScript downloading and removes all JavaScript code from HTML,<br>including `onclick` and other `on*` handlers. |
| `--disable-styles` | Disables CSS file downloading and at the same time removes all style definitions<br>by `<style>` tag or inline by style attributes. |
| `--disable-fonts` | Disables font downloading and also removes all font/font-face definitions from CSS. |
| `--disable-images` | Disables downloading of all images and replaces found images in HTML with placeholder image only. |
| `--disable-files` | Disables downloading of any files (typically downloadable documents) to which various links point. |
| `--remove-all-anchor-listeners` | On all links on the page remove any event listeners. Useful on some types of sites with modern<br>JS frameworks that would like to compose content dynamically (React, Svelte, Vue, Angular, etc.). |

### Advanced crawler settings

| Parameter | Description |
|-----------|-------------|
| `--workers=<int>` | Maximum number of concurrent workers (threads).<br>Crawler will not make more simultaneous requests to the server than this number.<br>Use carefully! A high number of workers can cause a DoS attack. Default is `3`. |
| `--max-reqs-per-sec=<val>` | Max requests/s for whole crawler. Be careful not to cause a DoS attack. Default value is `10`. |
| `--memory-limit=<size>` | Memory limit in units `M` (Megabytes) or `G` (Gigabytes). Default is `2048M`. |
| `--resolve=<host:port:ip>` | Custom DNS resolution in `domain:port:ip` format. Same as [curl --resolve](https://everything.curl.dev/usingcurl/connections/name.html?highlight=resolve#provide-a-custom-ip-address-for-a-name).<br>Can be specified multiple times. |
| `--allowed-domain-for-external-files=<domain>` | Enable loading of file content from another domain (e.g. CDN).<br>Can be specified multiple times. Use `*` for all domains. |
| `--allowed-domain-for-crawling=<domain>` | Allow crawling of other listed domains — typically language mutations on other domains.<br>Can be specified multiple times. Use wildcards like `*.mysite.tld`. |
| `--single-foreign-page` | When crawling of other domains is allowed, ensures that only the linked page<br>and its assets are crawled from foreign domains. |
| `--include-regex=<regex>` | PCRE-compatible regular expression for URLs that should be included.<br>Can be specified multiple times. Example: `--include-regex='/^\/public\//'` |
| `--ignore-regex=<regex>` | PCRE-compatible regular expression for URLs that should be ignored.<br>Can be specified multiple times. |
| `--regex-filtering-only-for-pages` | Apply `*-regex` rules only to page URLs, not static assets. |
| `--analyzer-filter-regex` | PCRE-compatible regular expression for filtering analyzers by name. |
| `--accept-encoding=<val>` | Custom `Accept-Encoding` request header. Default is `gzip, deflate, br`. |
| `--remove-query-params` | Remove query parameters from found URLs. |
| `--keep-query-param=<name>` | Keep only the specified query parameter(s) in discovered URLs; all others are removed.<br>Can be specified multiple times. If `--remove-query-params` is also set, all parameters<br>are removed regardless. |
| `--add-random-query-params` | Add random query parameters to each URL to bypass caches. |
| `--transform-url=<from->to>` | Transform URLs before crawling. Use `from -> to` for simple replacement or `/regex/ -> replacement`.<br>Can be specified multiple times. |
| `--force-relative-urls` | Normalize all discovered URLs matching the initial domain (incl. www variant and protocol<br>differences) to canonical form. Prevents duplicate files in offline export when the site<br>uses inconsistent URL formats (http/https, www/non-www): links to these variants become<br>relative links to the same local files. A scheme-less `www.example.com/page` is a relative<br>path by the URL standard and is not treated as the initial host. |
| `--ignore-robots-txt` | Ignore robots.txt content. |
| `--ignore-html-comments` | Ignore URLs found inside HTML comments (`<!-- ... -->`), which search engines also<br>ignore, so commented links are not crawled or reported as broken. |
| `--http-cache-dir=<dir>` | Cache dir for HTTP responses. Disable with `--http-cache-dir='off'` or `--no-cache`.<br>Default is `~/.cache/siteone-crawler/http-cache` (XDG-compliant, respects `$XDG_CACHE_HOME`). |
| `--http-cache-compression` | Enable compression for HTTP cache storage. |
| `--http-cache-ttl=<val>` | TTL for HTTP cache entries (e.g. `1h`, `7d`, `30m`). Use `0` for infinite. Default is `24h`. |
| `--no-cache` | Disable HTTP cache completely. Shortcut for `--http-cache-dir='off'`. |
| `--max-queue-length=<num>` | Maximum length of the waiting URL queue. Default is `9000`. |
| `--max-visited-urls=<num>` | Maximum number of visited URLs. Default is `10000`. |
| `--max-skipped-urls=<num>` | Maximum number of skipped URLs. Default is `10000`. |
| `--max-url-length=<num>` | Maximum supported URL length in chars. Default is `2083`. |
| `--max-non200-responses-per-basename=<num>` | Protection against looping with dynamic non-200 URLs. Default is `5`. |

### File export settings

| Parameter | Description |
|-----------|-------------|
| `--output-html-report=<file>` | Save HTML report into that file. Set to empty `''` to disable HTML report.<br>By default saved into `tmp/%domain%.report.%datetime%.html`. |
| `--html-report-options=<sections>` | Comma-separated list of sections to include in HTML report.<br>Available sections: `summary`, `seo-opengraph`, `image-gallery`, `video-gallery`, `visited-urls`, `dns-ssl`, `crawler-stats`, `crawler-info`, `headers`, `content-types`, `skipped-urls`, `external-links`, `caching`, `best-practices`, `accessibility`, `security`, `redirects`, `404-pages`, `slowest-urls`, `fastest-urls`, `source-domains`, `technologies`.<br>Default: all sections. |
| `--output-json-file=<file>` | File path for JSON output. Set to empty `''` to disable JSON file.<br>By default saved into `tmp/%domain%.output.%datetime%.json`.<br>See [JSON Output Documentation](docs/JSON-OUTPUT.md) for format details. |
| `--output-text-file=<file>` | File path for TXT output. Set to empty `''` to disable TXT file.<br>By default saved into `tmp/%domain%.output.%datetime%.txt`.<br>See [Text Output Documentation](docs/TEXT-OUTPUT.md) for format details. |
| `--add-timestamp-to-output-file` | Append timestamp to output filenames (HTML report, JSON, TXT) except sitemaps. |
| `--add-host-to-output-file` | Append initial URL host to output filenames (HTML report, JSON, TXT) except sitemaps. |

**Default output directory:** Report files are saved into `./tmp/` in the current working directory. If `./tmp/` cannot be created (e.g. read-only filesystem), the crawler falls back to the platform's XDG data directory (`~/.local/share/siteone-crawler/` on Linux, `~/Library/Application Support/siteone-crawler/` on macOS, `%APPDATA%\siteone-crawler\` on Windows) and prints a notice to stderr.

### Mailer options

| Parameter | Description |
|-----------|-------------|
| `--mail-to=<email>` | Recipients of HTML e-mail reports. Required for mailer activation.<br>You can specify multiple emails separated by comma. |
| `--mail-from=<email>` | E-mail sender address. Default is `siteone-crawler@your-hostname.com`. |
| `--mail-from-name=<val>` | E-mail sender name. Default is `SiteOne Crawler`. |
| `--mail-subject-template=<val>` | E-mail subject template. You can use `%domain%`, `%date%` and `%datetime%`.<br>Default is `Crawler Report for %domain% (%date%)`. |
| `--mail-smtp-host=<host>` | SMTP host for sending emails. Default is `localhost`. |
| `--mail-smtp-port=<port>` | SMTP port for sending emails. Default is `25`. |
| `--mail-smtp-user=<user>` | SMTP user, if your SMTP server requires authentication. |
| `--mail-smtp-pass=<pass>` | SMTP password, if your SMTP server requires authentication. |

### Upload options

| Parameter | Description |
|-----------|-------------|
| `--upload` | Enable HTML report upload to `--upload-to`. |
| `--upload-to=<url>` | URL of the endpoint where to send the HTML report. Default is `https://crawler.siteone.io/up`. |
| `--upload-retention=<val>` | How long should the HTML report be kept in the online version?<br>Values: 1h / 4h / 12h / 24h / 3d / 7d / 30d / 365d / forever.<br>Default is `30d`. |
| `--upload-password=<val>` | Optional password (user will be 'crawler') to display the online HTML report. |
| `--upload-timeout=<int>` | Upload timeout in seconds. Default is `3600`. |

### Offline exporter options

| Parameter | Description |
|-----------|-------------|
| `--offline-export-dir=<dir>` | Path to directory where to save the offline version of the website. |
| `--offline-export-store-only-url-regex=<regex>` | Debug: store only URLs matching these PCRE regexes. Can be specified multiple times. |
| `--offline-export-remove-unwanted-code=<1/0>` | Remove unwanted code for offline mode (analytics, social networks, etc.). Default is `1`. |
| `--offline-export-no-auto-redirect-html` | Disable automatic creation of redirect HTML files for subfolders containing `index.html`. |
| `--offline-export-preserve-url-structure` | Preserve the original URL path structure. E.g. `/about` is stored as `about/index.html`<br>instead of `about.html` and links point to that file. Useful for web server deployment where<br>the clone should maintain the same URL hierarchy as the original site, see<br>[Static copy on the original URLs](#static-copy-on-the-original-urls). With this option the<br>markdown export (`--markdown-export-dir`) uses the same layout, e.g. `about/index.md`, and an<br>extension-less image gets the extension of its content type there, e.g. `logo/index.svg`. |
| `--offline-export-preserve-urls` | Preserve original URL format in exported HTML/CSS/JS — same-domain links become root-relative (`/path`), cross-domain links stay absolute. Ideal for processing with [siteone-chunker](https://github.com/janreges/siteone-chunker) and RAG pipelines where links must resolve to the production website. |
| `--offline-export-no-url-rewriting` | Disable all URL rewriting in exported HTML/CSS/JS. URLs remain exactly as in the original source. Useful for RAG indexing or other processing where original URLs must be preserved verbatim. |
| `--replace-content=<val>` | Replace content in HTML/JS/CSS with `foo -> bar` or PCRE regexp.<br>Can be specified multiple times. |
| `--replace-query-string=<val>` | Replace characters in query string filenames.<br>Can be specified multiple times. E.g. `'/([^&]+)=([^&]*)(&\|$)/ -> $1-$2_'`<br>stores `/news?start=1&sort=asc` as `news.start-1_sort-asc_.html`. |
| `--offline-export-lowercase` | Convert all filenames to lowercase for offline export. Useful for case-insensitive filesystems. |
| `--ignore-store-file-error` | Ignore any file storing errors and continue. |
| `--disable-astro-inline-modules` | Disable inlining of Astro module scripts for offline export.<br>Scripts will remain as external files with corrected relative paths. |

#### Static copy on the original URLs

To host the export as a static copy of the website on its original URLs (e.g. as a fallback for a
dynamic site), combine `--offline-export-preserve-url-structure` with `--offline-export-preserve-urls`.
Pages are stored as `index.html` files in their own directories (`/about` → `about/index.html`) and links
keep their original root-relative form (`/about`), so serve the export from the web root:

```bash
./siteone-crawler --url=https://example.com/ \
  --offline-export-dir=/var/www/example.com \
  --offline-export-preserve-url-structure \
  --offline-export-preserve-urls \
  --offline-export-no-auto-redirect-html
```

nginx:

```nginx
server {
    server_name example.com;
    root /var/www/example.com;
    index index.html;

    location / {
        try_files $uri $uri/ $uri/index.html =404;
    }
}
```

Apache (`.htaccess` in the export directory; needs `mod_rewrite` and `AllowOverride All`):

```apache
Options -Indexes -MultiViews
DirectoryIndex index.html
# Serve /about from about/index.html without redirecting to /about/
DirectorySlash Off
RewriteEngine On
RewriteCond %{REQUEST_FILENAME}/index.html -f
RewriteRule ^(.*[^/])$ $1/index.html [L]
```

URLs with a query string are stored as `…/index.<hash>.html` and are not served on their original URLs
by these rules. Without `--offline-export-preserve-urls`, links point to the exported files with relative
paths (e.g. `../about/index.html`), so the copy also works when opened directly from disk.

### Markdown exporter options

| Parameter | Description |
|-----------|-------------|
| `--markdown-export-dir=<dir>` | Path to directory where to save the markdown version of the website. |
| `--markdown-export-single-file=<file>` | Path to a file for combined markdown. Requires `--markdown-export-dir`. |
| `--markdown-move-content-before-h1-to-end` | Move content before main H1 heading to the end of the markdown. |
| `--markdown-disable-images` | Do not export and show images in markdown files. |
| `--markdown-disable-files` | Do not export files other than HTML/CSS/JS/fonts/images (e.g. PDF, ZIP). |
| `--markdown-remove-links-and-images-from-single-file` | Remove links and images from combined single file. |
| `--markdown-exclude-selector=<val>` | Exclude DOM elements by CSS selector from markdown export.<br>Can be specified multiple times. |
| `--markdown-replace-content=<val>` | Replace text content with `foo -> bar` or PCRE regexp.<br>Can be specified multiple times. |
| `--markdown-replace-query-string=<val>` | Replace characters in query string filenames.<br>Can be specified multiple times. Same syntax as `--replace-query-string`. |
| `--markdown-export-store-only-url-regex=<regex>` | Debug: store only URLs matching these PCRE regexes. Can be specified multiple times. |
| `--markdown-ignore-store-file-error` | Ignore any file storing errors and continue. |

### Sitemap options

| Parameter | Description |
|-----------|-------------|
| `--sitemap-xml-file=<file>` | File path for generated XML Sitemap. Extension `.xml` added if not specified; a path ending in `.xml.gz` writes a gzip-compressed sitemap. |
| `--sitemap-txt-file=<file>` | File path for generated TXT Sitemap. Extension `.txt` added if not specified. |
| `--sitemap-base-priority=<num>` | Base priority for XML sitemap. Default is `0.5`. |
| `--sitemap-priority-increase=<num>` | Priority increase based on slashes in URL. Default is `0.1`. |
| `--sitemap-changefreq=<val>` | `<changefreq>` for all URLs in the XML sitemap: `always`, `hourly`, `daily`, `weekly`, `monthly`, `yearly` or `never`. Not written by default. |

### Expert options

| Parameter | Description |
|-----------|-------------|
| `--debug` | Activate debug mode. |
| `--debug-log-file=<file>` | Log file for debug messages. When set without `--debug`, logging is active without visible output. |
| `--debug-url-regex=<regex>` | Regex for URL(s) to debug. Can be specified multiple times. |
| `--result-storage=<val>` | Result storage type. Values: `memory` or `file`. Use `file` for large websites. Default is `memory`. |
| `--result-storage-dir=<dir>` | Directory for `--result-storage=file`. Default is `tmp/result-storage`. |
| `--result-storage-compression` | Enable compression for results storage. |
| `--http-cache-dir=<dir>` | Cache dir for HTTP responses. Disable with `--http-cache-dir='off'` or `--no-cache`.<br>Default is `~/.cache/siteone-crawler/http-cache` (XDG-compliant, respects `$XDG_CACHE_HOME`). |
| `--http-cache-compression` | Enable compression for HTTP cache storage. |
| `--http-cache-ttl=<val>` | TTL for HTTP cache entries (e.g. `1h`, `7d`, `30m`). Use `0` for infinite. Default is `24h`. |
| `--websocket-server=<host:port>` | Start crawler with websocket server on given host:port. |
| `--console-width=<int>` | Enforce a fixed console width, disabling automatic detection. |
| `--events-file=<file>` | Write a machine-readable NDJSON account of the run to this file: crawled URLs, phases, AI requests and progress, artifacts, issues and the result. For GUIs and CI tooling; see [docs/EVENTS.md](docs/EVENTS.md). |
| `--control-stdin` | Read commands from stdin: a line `stop` (or end of input) winds the crawl down like Ctrl+C. See [docs/EVENTS.md](docs/EVENTS.md). |

### Fastest URL analyzer

| Parameter | Description |
|-----------|-------------|
| `--fastest-urls-top-limit=<int>` | Number of URLs in TOP fastest list. Default is `20`. |
| `--fastest-urls-max-time=<val>` | Maximum response time for an URL to be considered fast. Default is `1`. |

### SEO and OpenGraph analyzer

| Parameter | Description |
|-----------|-------------|
| `--max-heading-level=<int>` | Max heading level from 1 to 6 for analysis. Default is `3`. |

### Slowest URL analyzer

| Parameter | Description |
|-----------|-------------|
| `--slowest-urls-top-limit=<int>` | Number of URLs in TOP slowest list. Default is `20`. |
| `--slowest-urls-min-time=<val>` | Minimum response time threshold for slow URLs. Default is `0.01`. |
| `--slowest-urls-max-time=<val>` | Maximum response time for very slow evaluation. Default is `3`. |

### Built-in HTTP server

Browse exported markdown or offline HTML files through a local web server with a built-in viewer.

| Parameter | Description |
|-----------|-------------|
| `--serve-markdown=<dir>` | Start built-in HTTP server for browsing a markdown export directory.<br>Renders `.md` files as styled HTML with tables, accordions, dark/light mode, and breadcrumb navigation. |
| `--serve-offline=<dir>` | Start built-in HTTP server for browsing an offline HTML export directory.<br>Serves static files with Content-Security-Policy restricting assets to the same origin. |
| `--serve-port=<int>` | Port for the built-in HTTP server. Default is `8321`. |
| `--serve-bind-address=<addr>` | Bind address for the built-in HTTP server. Default is `127.0.0.1` (localhost only).<br>Use `0.0.0.0` to listen on all network interfaces and their IP addresses. |

**Example:**

```bash
# Browse markdown export
./siteone-crawler --serve-markdown=./exports/markdown

# Browse offline export on custom port, accessible from network
./siteone-crawler --serve-offline=./exports/offline --serve-port=9000 --serve-bind-address=0.0.0.0
```

### HTML-to-Markdown conversion

Convert a local HTML file to clean Markdown without crawling. Uses the same conversion pipeline as the markdown exporter.

| Parameter | Description |
|-----------|-------------|
| `--html-to-markdown=<file>` | Convert a local HTML file to Markdown and print to stdout. No crawling is performed.<br>Respects `--markdown-disable-images`, `--markdown-disable-files`, `--markdown-move-content-before-h1-to-end`, and `--markdown-exclude-selector`. |
| `--html-to-markdown-output=<file>` | Write the converted Markdown to a file instead of stdout. Requires `--html-to-markdown`. |

**Examples:**

```bash
# Convert HTML file to Markdown (printed to stdout)
./siteone-crawler --html-to-markdown=page.html

# Convert and save to a file
./siteone-crawler --html-to-markdown=page.html --html-to-markdown-output=page.md

# Convert with options: remove images, exclude navigation, move header below h1
./siteone-crawler --html-to-markdown=page.html \
  --markdown-disable-images \
  --markdown-exclude-selector=nav \
  --markdown-move-content-before-h1-to-end

# Pipe to other tools (e.g. clipboard, AI, wc)
./siteone-crawler --html-to-markdown=page.html | pbcopy
./siteone-crawler --html-to-markdown=page.html | wc -l
```

### CI/CD settings

| Parameter | Description |
|-----------|-------------|
| `--ci` | Enable CI/CD quality gate. Crawler exits with code 10 if thresholds are not met. Default file outputs (HTML, JSON, TXT reports) are suppressed unless explicitly requested via `--output-*` options.<br>Instead of a row per URL, the console shows at most one progress line every 10 seconds (`--progress-interval=10`) unless `--progress-interval` is given; rows of failed URLs (4xx/5xx, connection errors, timeouts) are still printed. |
| `--ci-min-score=<val>` | Minimum overall quality score (0.0-10.0). Default is `5.0`. |
| `--ci-min-performance=<val>` | Minimum Performance category score (0.0-10.0). Default is `5.0`. |
| `--ci-min-seo=<val>` | Minimum SEO category score (0.0-10.0). Default is `5.0`. |
| `--ci-min-security=<val>` | Minimum Security category score (0.0-10.0). Default is `5.0`. |
| `--ci-min-accessibility=<val>` | Minimum Accessibility category score (0.0-10.0). Default is `3.0`. |
| `--ci-min-best-practices=<val>` | Minimum Best Practices category score (0.0-10.0). Default is `5.0`. |
| `--ci-max-404=<int>` | Maximum number of 404 responses allowed. Default is `0`. |
| `--ci-max-5xx=<int>` | Maximum number of 5xx server error responses allowed. Default is `0`. |
| `--ci-max-criticals=<int>` | Maximum number of critical analysis findings allowed. Default is `0`. |
| `--ci-max-warnings=<int>` | Maximum number of warning analysis findings allowed. Not checked by default. |
| `--ci-max-avg-response=<val>` | Maximum average response time in seconds. Not checked by default. |
| `--ci-min-pages=<int>` | Minimum number of HTML pages that must be found. Default is `10`. |
| `--ci-min-assets=<int>` | Minimum number of assets (JS, CSS, images, fonts) that must be found. Default is `10`. |
| `--ci-min-documents=<int>` | Minimum number of documents (PDF, etc.) that must be found. Default is `0` (not checked). |
| `--ci-baseline=<file>` | Path to a previous `--output=json` file used as a baseline for regression checks. A missing/unreadable file is warned about (the check is skipped, not silently passed). |
| `--ci-max-score-drop=<val>` | Maximum allowed drop of the overall score vs the `--ci-baseline` run. Default `0` (any drop fails). |
| `--ci-fail-on-code=<code>` | Fail the build if a finding code (`aplCode`, e.g. `seo-noindex-sitewide`) is present. Can be specified multiple times. |
| `--ci-ignore-code=<code>` | Ignore a finding code (`aplCode`, e.g. `pages-without-lang`) when counting criticals/warnings; also suppresses `--ci-fail-on-code`. Can be specified multiple times. |
| `--ci-junit-file=<file>` | Write the CI gate result as a JUnit XML report (renders natively in GitLab/Jenkins/GitHub test reporters). |
| `--ci-github-annotations` | Print GitHub Actions `::error` annotations for failed checks (to stderr with `--output=json`). Auto-enabled when `GITHUB_ACTIONS=true`. |

**Default behavior with `--ci` alone:** overall score >= 5.0, each category score >= 5.0 (Performance, SEO, Security, Best Practices) and Accessibility >= 3.0, 404 errors <= 0, 5xx errors <= 0, critical findings <= 0, HTML pages >= 10, assets >= 10. File outputs (HTML, JSON, TXT reports) are not generated. To save reports in CI mode, specify the desired output explicitly, e.g. `--ci --output-html-report=report.html`. Instead of one row per URL, the console prints at most one progress line every 10 seconds plus the rows of failed URLs (4xx/5xx, connection errors, timeouts); use `--progress-interval=0` to get every row.

### 🌐 Browser rendering (optional)

Render each page in a real Chromium (CDP) instead of a plain HTTP request. **Included in the default build / pre-built binaries** (no special flag needed; for a lean build without it use `cargo build --release --no-default-features`). With `--browser` off, the crawler behaves exactly as before.

```bash
# Crawl a JavaScript / SPA site with full browser rendering
./siteone-crawler --url=https://my.spa.tld --browser

# Capture a full-page screenshot of every page (PNG by default)
./siteone-crawler --url=https://my.spa.tld --browser --screenshots --screenshot-mode=full-page

# Watch it run in a visible window (one page at a time)
./siteone-crawler --url=https://my.spa.tld --browser --browser-headful

# Use a specific browser binary and wait until the network goes idle
./siteone-crawler --url=https://my.spa.tld --browser --browser-path=/usr/bin/google-chrome --browser-wait=networkidle
```

Browser is auto-detected (Chrome/Chromium/Edge/Brave); if none is found you're offered a one-time `chrome-headless-shell` download (or pass `--browser-auto-download` for CI). Key options:

| Option | Default | Meaning |
|--------|---------|---------|
| `--browser` | off | Render pages in Chromium (built into the default binaries). |
| `--browser-path=<exe>` | — | Explicit browser binary; skips detection/download. |
| `--browser-headful` | off | Visible window (default is headless; renders one page at a time). |
| `--browser-no-sandbox` | off | Add `--no-sandbox` (often required in Docker/CI/WSL/root; weakens isolation). |
| `--browser-auto-download` | off | Pre-consent to downloading a browser in non-interactive/CI runs. |
| `--browser-workers=<n>` | 3 | Concurrent rendered pages (separate from `--workers`). |
| `--browser-wait=<mode>` | `networkidle` | Readiness: `load`, `domcontentloaded`, or `networkidle`. |
| `--browser-wait-extra=<ms>` | 0 | Extra settle delay after the wait condition. |
| `--browser-timeout=<sec>` | 30 | Hard navigation+render timeout per page. |
| `--browser-render-all` | off | Render every URL (default: only HTML documents; assets via HTTP). |
| `--browser-auto-scroll` | on | Scroll each rendered page to the bottom and back before capturing it, so lazy-loaded and scroll-triggered content is rendered (at most ~5 s, then up to 3 s for the requests the scrolling started, within `--browser-timeout`); `--browser-auto-scroll=0` turns it off. |
| `--screenshots` | off | Capture a screenshot of every rendered page (requires `--browser`). |
| `--screenshots-dir=<dir>` | `tmp/screenshots/` | Output directory for screenshots. |
| `--screenshot-mode=<m>` | `viewport` | `viewport` (set resolution) or `full-page` (full scroll height). |
| `--screenshot-viewport=<WxH,...>` | `1920x1080` | Render/viewport size: `WxH` or a preset `desktop` (1920x1080), `tablet` (768x1024), `mobile` (390x844). A comma-separated list (up to 5) captures every page in each size; the first one is used for rendering. |
| `--screenshot-format=<f>` | `png` | `png`, `jpg`, or `webp`. |
| `--screenshot-quality=<1-100>` | 80 | Quality for `jpg`/`webp`. |
| `--screenshots-animation=<fmt>` | — | Assemble screenshots into an animation; `gif`, `mp4`, or `gif,mp4`. |
| `--screenshots-animation-frame-duration=<s>` | 2 | Seconds each page is shown in the animation (0.2–10). |
| `--screenshots-animation-width=<px>` | 1024 | Output width in pixels; height is derived from the aspect ratio of the first `--screenshot-viewport` size. |
| `--ffmpeg-path=<path>` | — | Explicit ffmpeg binary (auto-detected from PATH otherwise). Required for MP4. |
| `--screenshot-hide-cookie-banners` | off | Before each screenshot, try to dismiss/hide cookie consent banners (best-effort). |
| `--screenshot-hide-selector=<css>` | — | Comma-separated CSS selectors to hide before each screenshot (site-specific banners). |
| `--console-max-messages` / `--console-msg-max-chars` / `--console-total-max-kb` | 100 / 200 / 128 | Size limits for the console diagnostics passed to the AI assistant. |

#### Animations are settled before each capture

To avoid capturing a page mid-effect, animations are **intentionally settled right before
every screenshot**: finite entrance animations (fade-/slide-in reveals) are fast-forwarded to
their final state, and infinite loops (spinners, auto-play hero animations) are paused on their
current frame. The result looks fully loaded instead of half-rendered. This is automatic and
needs no flag. Scroll-driven animations whose progress is bound to the scroll position (rather
than to time) are not covered by this.

#### Lazy-loaded and scroll-triggered content

Many pages load images or reveal sections only when they are scrolled into view. Before the
rendered HTML is captured (and before screenshots), every page taller than the viewport is
therefore scrolled to the bottom in steps of about 0.8 of the viewport height every 120 ms (at most
5 s, always within `--browser-timeout`). The crawler then waits for the requests the scrolling
started (lazy images, sections fetched when they come into view) until none has been in flight for
0.5 s, at most 3 s; the animations started by the scrolling are then settled and the page returns
to the top. Offline and markdown exports then contain the lazy-loaded images and the revealed
content, and full-page screenshots show them. The scrolling adds up to a few seconds per long page
(counted in its response time); `--browser-auto-scroll=0` turns it off.
Side effects to keep in mind: scroll-depth popups (newsletter or exit-intent modals) can appear in
screenshots and in the captured HTML — hide them in screenshots with `--screenshot-hide-selector`;
infinite feeds always use the full 5 s and keep growing while scrolled (more items and links), and
pages that keep polling or keep a request open use the full 3 s wait, so use
`--browser-auto-scroll=0` when timing or a stable page matters; content that arrives after the 3 s
wait (or appears after a timer rather than a request) is not captured; pages that scroll an inner
container instead of the document (e.g. `body { height: 100%; overflow: auto }`) are not scrolled.

#### Screenshots in several viewports

`--screenshot-viewport` accepts a comma-separated list of up to 5 sizes — `WxH` values (each side
at most 16384 px) or the presets `desktop` (1920x1080), `tablet` (768x1024) and `mobile` (390x844):

```bash
./siteone-crawler --url=https://my.domain.tld --browser --screenshots --screenshot-viewport=desktop,tablet,mobile
```

The first size is the render viewport: the page is loaded, auto-scrolled and captured in it, and
the screenshot animation uses it. For each further size the page is resized (device pixel ratio 1,
desktop mode — only the viewport changes, not the user agent), left to settle and captured again;
`--screenshot-hide-cookie-banners` and `--screenshot-hide-selector` are applied again before each capture,
and what they hide stays hidden when the page mounts it again (e.g. a responsive banner re-rendered
when a full-page capture resizes the page).
With a single size the file names are unchanged; with several, every file name ends with the size,
e.g. `example_com_about_c30b28d2_390x844.png`. The "Browser screenshots" table lists one row per file.

#### Screenshot animation

When capturing screenshots (`--browser --screenshots`), you can assemble them into an
animation in crawl order:

- `--screenshots-animation=gif,mp4` — formats to produce (`gif`, `mp4`, or both).
- `--screenshots-animation-frame-duration=2` — seconds each page is shown (0.2–10).
- `--screenshots-animation-width=1024` — output width in px; height is derived from
  the aspect ratio of the first `--screenshot-viewport` size.
- `--ffmpeg-path=/path/to/ffmpeg` — explicit ffmpeg binary (auto-detected from PATH
  otherwise). **Required for MP4**; GIF works without ffmpeg.

Output files are written next to the screenshots (default `tmp/screenshots/animation.gif`
and `animation.mp4`). If MP4 is requested but ffmpeg is unavailable, MP4 is skipped with a
warning and the GIF is still produced. MP4 is the **only** feature that needs an external
binary (ffmpeg); everything else, including GIF, is self-contained.

#### Hiding cookie consent banners

Cookie consent banners are fixed overlays that otherwise appear on every screenshot.
`--screenshot-hide-cookie-banners` injects a best-effort script before each capture that
clicks "reject" controls first (and "accept-all" as a fallback) across major consent
platforms (OneTrust, Cookiebot, Didomi, Usercentrics incl. shadow DOM, Quantcast, TrustArc,
…), removes scroll-lock, and hides remaining consent containers plus any fixed/sticky
high-z-index overlay whose text matches cookie/consent keywords (English **and** Czech). For
a stubborn site-specific banner, pass your own selectors with
`--screenshot-hide-selector="#my-banner,.overlay"`.

This is best-effort — no method removes 100 % of banners. It's a screenshot-cleanliness
helper, not a privacy tool: the accept-all fallback may **grant** consent on sites that
expose no reject control, and the heuristic may occasionally hide a legitimate sticky
element that merely mentions cookies/privacy.

Captured console/JS/network/security diagnostics appear in a "Browser issues" table and are also exposed (size-bounded) to the AI assistant via the `{{browser_diagnostics}}` placeholder in `--ai-prompt` / `--ai-prompt-file` (the `custom` AI action) — e.g. ask the model to triage the console/network errors.

### 🤖 AI assistant (optional)

The crawler can use an LLM to add **qualitative** analyses on top of the deterministic checks: AI SEO assessment with concrete title/description/keyword rewrites, `llms.txt` generation, spelling/grammar/weak-copy detection, and your own custom policy checks.

AI is **strictly opt-in**: with no `--ai-*` flag, nothing changes — no environment variable is read, no AI code runs, zero cost. Pass at least one `--ai-*` flag (typically `--ai-provider`, `--ai-model`, and an endpoint) to enable it.

Supported providers: `openai`, `anthropic`, `gemini`, and `openai-compatible` (vLLM, LiteLLM, MiniMax, LocalAI, Ollama, and any self-hosted OpenAI-compatible endpoint). The client is a thin wrapper over the crawler's own HTTP stack — **no extra dependencies**.

**Quick start:**

```bash
# OpenAI-compatible (e.g. local vLLM) — SEO analysis of the 100 most important pages
./siteone-crawler --url=https://example.com/ \
  --ai-provider=openai-compatible --ai-endpoint=http://localhost:8000/v1 \
  --ai-model=Qwen/Qwen3-32B --ai-actions=seo

# OpenAI — SEO + llms.txt (key read from OPENAI_API_KEY by default)
./siteone-crawler --url=https://example.com/ \
  --ai-provider=openai --ai-model=gpt-5-mini --ai-actions=seo,llms-txt

# Preview cost before spending anything
./siteone-crawler --url=https://example.com/ \
  --ai-provider=anthropic --ai-model=claude-sonnet-4-6 --ai-actions=seo --ai-dry-run
```

**Provider configuration:**

| Parameter | Description |
|-----------|-------------|
| `--ai-provider=<val>` | `openai`, `anthropic`, `gemini`, or `openai-compatible`. Enables the AI features. Default is `openai-compatible`. |
| `--ai-endpoint=<url>` | Base API endpoint URL. **Required** for `openai-compatible`; optional override for the others. |
| `--ai-model=<val>` | Model name, e.g. `MiniMax-M3`, `gpt-5-mini`, `claude-sonnet-4-6`, `gemini-2.5-pro`. Required when AI is enabled. |
| `--ai-max-tokens=<int>` | Max output tokens per request. Default `32000`. Auto-mapped to `max_completion_tokens` for OpenAI reasoning models. Raise it further if you enable thinking/reasoning (which consumes output tokens). |
| `--ai-use-max-completion-tokens` | Force `max_completion_tokens` instead of `max_tokens` (otherwise auto-detected). |
| `--ai-temperature=<val>` | Sampling temperature. Default `0.0` (omitted automatically for OpenAI reasoning models). |
| `--ai-extra-body=<json>` | JSON object deep-merged into the request body, overriding native fields. See [Thinking / reasoning](#thinking--reasoning) below. |
| `--ai-synthesis-extra-body=<json>` | Like `--ai-extra-body` but applied ONLY to the final `summary` synthesis call — e.g. to enable thinking/reasoning just for the synthesis. See the `summary` action below. |

**API key (security):** the key is resolved with the following precedence (first match wins). Prefer environment variables so the key never appears in process arguments, shell history, or logs — the crawler redacts `--ai-api-key=...` in the saved command and never serializes the key into JSON output or the response cache. What the crawler prints or writes about AI requests (request lines, errors, events, reports, the `--ai-list-models`/`--ai-check` answers) shows the key and the endpoint's credentials (URL userinfo, query values) as `[redacted]`; a text that repeats a credential shorter than 8 characters is withheld as a whole, since blanking a short credential out of words would spell it back out.

1. `--ai-api-key-file=<file>` — read the first line of a file (safest for CI; `chmod 600`).
2. `--ai-api-key=env:VARNAME` — read the named environment variable (indirection).
3. `--ai-api-key=<value>` — raw value (**discouraged**: leaks into `ps`/history/logs).
4. `--ai-api-key-env=<name>` — read the named environment variable.
5. **Default:** the conventional variable for the provider — `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, or `GEMINI_API_KEY`.

**Actions** (`--ai-actions=`, comma-separated; default `seo,typos,summary` — the full report set). Enabling AI without specifying actions runs per-page SEO analysis, content (typos/grammar) checks, and the executive summary, all in the HTML report. `custom` (needs a prompt) and `llms-txt`/`llms-full` (extra files) are opt-in:

| Action | What it does | Output |
|--------|--------------|--------|
| `seo` | Per-page SEO judgement with per-factor scores and recommended title/description/keywords. Complements the deterministic SEO analyzer. | "AI SEO analysis" table (text/JSON/HTML report) |
| `llms-txt` | Curated [llms.txt](https://llmstxt.org/) index of the most important pages, with AI-written names and one-line summaries grouped by section. | `<dir>/<domain>.llms.txt` |
| `llms-full` | `llms-full.txt` — the selected pages' full markdown concatenated under an AI preamble. | `<dir>/<domain>.llms-full.txt` |
| `typos` | Language-aware spelling, grammar, and weak-copy detection with suggestions. Skips brand names, code, and identifiers. | "AI content issues" table |
| `custom` | Runs your own prompt (`--ai-prompt-file` / `--ai-prompt`) against each page. | "AI custom check" table |
| `summary` | AI **executive summary** of the whole site (see below). Synthesizes the deterministic analysis (security, accessibility, SEO, performance, infrastructure) into a prioritized list of recommendations. | "AI Insights & Recommendations" box on the HTML report Summary tab |
| `extract` | First-class **AI report** engine (via `--ai-report`): extracts a typed set of fields per page (preset or custom schema) into a structured JSON + a self-contained HTML report. | `<ai-report-dir>/ai-report.<preset>.<host>.<run-id>.json` + `.html` |

> `llms.txt` / `llms-full.txt` are written next to `--markdown-export-dir` or `--offline-export-dir` if set, otherwise to `tmp/`.

> A host following the run (a GUI, CI tooling) gets every AI request with its tokens and timing, the progress of each AI task, the AI totals and every AI output file as events of the `--events-file` stream — see [docs/EVENTS.md](docs/EVENTS.md).

#### AI reports (`--ai-report`)

`--ai-report` runs the **extract** engine over selected crawled pages and produces two consistent artifacts in `--ai-report-dir` (default `tmp/`): `ai-report.<preset>.<host>.<run-id>.json` and `.html`. The shared run ID makes repeated runs collision-safe. The light/dark HTML has no external dependency by default and remains useful with JavaScript disabled: the hero summary, coverage, distributions, topic analysis, compliance evidence, and complete per-page table are server-rendered. Small inline JavaScript enhances it with search, sorting, theme switching, and spreadsheet-safe CSV export. Optional interactive ECharts visualizations use a pinned, SRI-verified CDN only with `--ai-report-cdn`.

Every artifact states its coverage: crawled HTML pages, eligible/selected/analyzed/failed pages, exclusions, cap drops, truncated evidence, include/exclude masks, and ranking method. It is visibly labelled **sampled** whenever the selected pages do not represent a complete successful crawl. Its usage/cost block covers the report extraction itself; the main crawl summary retains totals across all AI actions. Setting `--ai-report` alone runs only the report; combine it with explicit `--ai-actions=...` to run other AI actions too.

Built-in presets:

| Preset | Per-page output | Use case |
|--------|-----------------|----------|
| `ia` | URL path, cleaned title, a 200–300 char neutral description, `section` + `pageType` labels | Understand the **current information architecture** before a redesign |
| `quality` | clarity / depth / engagement / overall scores, reading grade level, tone, word count, top issue | Content **quality & readability** scoring across the site |
| `topics` | Per-page topic data plus deterministic site-wide clusters, competing topic/intent URLs, thin clusters, and missing funnel stages | Internal **topic/content coverage** and cannibalization candidates; it does not infer competitor or search-demand gaps |
| `compliance` | Deterministic `riskScore` + grounded findings with severity, rule, legal basis, SHALL/MAY status, effective date, verbatim excerpt, and recommendation | Advisory **regulatory & textual dark-pattern audit**, including EU consumer-credit (CCD2) loan-advertising readiness |

```bash
# Information-architecture inventory of a whole site
siteone-crawler --url=https://www.example.com/ --disable-all-assets \
  --ai-report=ia --ai-provider=openai-compatible \
  --ai-endpoint=http://localhost:8000/v1 --ai-model=Qwen/Qwen3-32B

# Regulatory / dark-pattern audit (advisory) — findings grounded in EU CCD2 loan-advertising rules
siteone-crawler --url=https://www.example.com/ \
  --ai-report=compliance --ai-provider=openai-compatible \
  --ai-endpoint=http://localhost:8000/v1 --ai-model=Qwen/Qwen3-32B
```

The versioned `compliance` rule pack allowlists rule/category pairs and verifies normalized excerpts against the exact retained model input. The semantic rule mapping and severity remain model judgements, not legal validation. `riskScore` is recalculated from model-classified findings with grounded excerpts (`critical=40`, `high=25`, `medium=12`, `low=5`, `info=2`, capped at 100); Member-State `MAY` observations remain visible but contribute zero. Unknown rules and ungrounded evidence trigger retries and ultimately an honest page error. The report distinguishes current UCPD duties, forward-looking **CCD2 (Directive (EU) 2023/2225) readiness from 20 November 2026**, and Member-State options whose national implementation must be checked. It also states that jurisdiction, Article 2 exclusions, and whether a page advertises an in-scope consumer-credit agreement cannot be established reliably from retained page text alone. It is prominently **advisory, not legal advice** and requires qualified legal review before reliance.

This profile supports claims and textual patterns that can be established from retained page text, including availability/approval claims, cost/risk framing, required credit-advertising information, urgency/scarcity, forced continuity, and confirm shaming. Known consent controls are retained as text so their wording can be assessed. The report deliberately does **not** claim to assess cookie network behavior, pre-ticked state, interaction flow, or visual prominence because text input cannot prove those properties. Legally relevant header/footer text is retained and bounded; when content is truncated, absence checks are marked indeterminate rather than clean.

**Custom typed extraction** (`--ai-report=extract`): define any per-page schema with a compact DSL and get one column per field in the JSON + HTML — no external script needed:

```bash
siteone-crawler --url=https://www.example.com/ --disable-all-assets \
  --ai-report=extract \
  --ai-extract-fields="title:string, summary:text, section:enum(Blog,Docs,Product,Legal,Other), quality:score, tags:string[]" \
  --ai-provider=openai-compatible --ai-endpoint=http://localhost:8000/v1 --ai-model=my-model
```

Field types: `string`, `text`, `int`, `float`, `bool`, `enum(a,b,c)`, `string[]`, `url`, `path`, `score` (0-100), `date`, and `findings`. Dates must be real calendar dates in `YYYY-MM-DD` form. Integers are limited to JSON/JavaScript's lossless range (`-9007199254740991` to `9007199254740991`); URL/path and numeric bounds are also validated. Reserved report keys (`url`, `path`, `_error`, `_evidence`) and empty or duplicate enum values are rejected. A rich schema with descriptions, `required`, `min`, `max`, and enums can be supplied via `--ai-schema-file=schema.json`; it is mutually exclusive with `--ai-extract-fields`. Required invalid/missing fields fail the page. Optional unknown values must be explicit JSON `null` and are excluded from aggregates.

**Robust JSON handling.** Mechanical repair handles syntax-only defects such as fences/prose, trailing commas, quote variants, and Python literals. Semantically incomplete objects, wrong types/ranges, malformed findings, unknown required values, and token-truncated completions are rejected even if their brackets could be repaired. A report extraction is retried up to three times; after that the page is recorded as an honest `_error` row with no fabricated cells and cannot enter aggregates.

**Schema enforcement** (`--ai-schema-enforce=auto|on|off`): `auto` is provider/model-aware. Supported hosted OpenAI models receive only the documented strict `response_format: json_schema`; Gemini receives `responseJsonSchema`; unknown hosted models, Anthropic, and OpenAI-compatible endpoints use the embedded field contract plus a generic JSON-object mode where the provider accepts it. `on` enables hosted strict output or the compatible endpoint's `response_format` + `guided_json`; Anthropic rejects explicit `on` clearly. If a provider rejects either structured-schema or generic JSON output controls, the request falls back once to the pure embedded-contract prompt. `off` skips schema enforcement but keeps the embedded contract and strict post-parse validation.

**Language and files:** `--ai-report-language=<BCP-47>` controls generated report prose, prompt output language, preset title, deterministic topic/compliance explanations, built-in schema descriptions, and HTML chrome. English and Czech chrome are built in; other tags retain the requested AI-output language and use English chrome. Stable JSON keys, enum/rule IDs, paths, dates, and verbatim excerpts are never translated; the HTML uses localized display labels for built-in IDs. `--ai-report-dir=<dir>` controls the paired output location. JSON and HTML are created as one no-clobber pair; a failed second write removes the first rather than leaving a partial report. AI artifacts are local files; existing `--mail-to` / `--upload` continue to deliver the standard crawl report and a summary notice states that decision.

**Charts** (`--ai-report-cdn`): the default offline report includes server-rendered preset summaries and CSS distributions. Add `--ai-report-cdn` for extra interactive IA, quality, topic, or compliance charts; if the CDN is unavailable, the material data remains visible.

#### Brand elaborate (`--ai-elaborate`)

`--ai-elaborate` turns a whole crawled site into **one large, richly structured brand profile** — the "who is this company/person/product, in depth" document. It produces three consistent artifacts in `--ai-report-dir` (default `tmp/`): `ai-elaborate.<template>.<host>.<run-id>.md` (readable Markdown), `.json` (structured), and a self-contained light/dark `.html`. The shared run ID makes repeated runs collision-safe, and the three files are written as one no-clobber set (a failed later write rolls back the earlier ones).

The core design is **anti-hallucination**: people, contacts, offerings, locations, facts and quotes are extracted **verbatim** from each page and deduplicated deterministically into a structured model that the crawler itself renders as lists — the model only ever writes the connective **prose** (executive summary, identity, audiences, …). It never invents a name, number, email or claim, and pages that fail to parse (after retries) are counted and listed, never filled with fabricated values.

How it selects what matters, at any site size: it ranks the full page universe, groups mass-entity pages (e.g. `/blog/*`, `/product/*`) into clusters that are **sampled rather than enumerated**, and then asks the model to pick the important pages by **integer id from a numbered list** (hallucinated URLs are impossible by construction), with a deterministic safety floor so a weak model round can never drop the obvious pages. On small sites the LLM selection is skipped entirely.

```bash
./siteone-crawler --url=https://example.com/ \
  --ai-elaborate --ai-report-language=en \
  --ai-provider=openai-compatible --ai-endpoint=http://localhost:8000/v1 \
  --ai-model=your-model
```

- `--ai-elaborate-template=corporate|personal|product` — the profile shape; default (auto) picks one from the detected site type.
- `--ai-report-language=<BCP-47>` — output language for the generated prose (verbatim names, emails, quotes stay in their original language).
- `--ai-elaborate-correct=true|false` (default `true`) — a final proofreading pass that safely fixes typos/artifacts and deletes unsupported sentences **in the prose only** (verbatim data is never altered).
- `--ai-elaborate-gap-fill=<n>` (default `20`) — fetch up to *n* important global-navigation pages the crawl never visited (e.g. under `--single-page` or a page cap) before building the profile; robots.txt and include/exclude masks are honored, `0` disables.
- `--ai-elaborate-cluster-min=<n>` (default `8`) / `--ai-elaborate-cluster-reps=<n>` (default `2`) — how many same-shape URLs form a sampled mass-entity cluster, and how many representatives per cluster to analyze.
- `--ai-elaborate-max-output-kb=<n>` (default `45`) — target prose size; above it, synthesis switches to a sectioned map-reduce to fit the model's output-token cap.
- `--ai-max-pages`, `--ai-include`, `--ai-exclude`, `--ai-max-concurrency`, `--ai-dry-run` apply as for other AI features. `--ai-elaborate` is its own pipeline (not an `--ai-actions` value): used alone it runs only the profile; combine it with explicit `--ai-actions=...` to run other AI actions too.

#### AI executive summary (`summary` action)

`--ai-actions=summary` runs **after** the deterministic analysis and produces a visually styled box at the top of the HTML report's **Summary** tab, below the Website Quality Score. It works by evaluating five areas in parallel — security, accessibility, SEO, performance, infrastructure — each grounded in compact *aggregated* crawl data (never raw per-URL lists), then synthesizing one cross-area, prioritized list of up to 15 actionable recommendations (fewer for a clean site — never padded) with severity, impact, and evidence.

Cost is **fixed at 6 LLM calls** (5 areas + 1 synthesis) regardless of site size — even for a 100,000-URL site — because only aggregates and a few capped top-N examples are sent (each area input stays well under 15 KB).

```bash
./siteone-crawler --url=https://example.com/ \
  --ai-provider=openai --ai-model=gpt-5-mini --ai-actions=summary
```

Tip: the area evaluations are cheap and usually run best without thinking, but the final synthesis benefits from reasoning. Use `--ai-synthesis-extra-body` to enable thinking **only** for the synthesis call:

```bash
--ai-extra-body='{"chat_template_kwargs":{"enable_thinking":false}}' \
--ai-synthesis-extra-body='{"chat_template_kwargs":{"enable_thinking":true}}'
```

(When you enable thinking, keep `--ai-max-tokens` generous — it defaults to `32000` — so reasoning does not truncate the JSON output.)

**Page selection & cost control** — a site can have thousands of pages, so AI runs only on the most important ones:

| Parameter | Description |
|-----------|-------------|
| `--ai-include=<regex>` | Only run AI on URLs matching this regex (repeatable). |
| `--ai-exclude=<regex>` | Skip AI on URLs matching this regex (repeatable, wins over include). E.g. `--ai-exclude='/press/'` to skip a thousand press releases. |
| `--ai-max-pages=<int>` | Hard cap on pages sent to the LLM. Default `100`. The highest-ranked (most important) pages are kept. This is the primary spend control. |
| `--ai-dry-run` | Show selected pages, initial calls, the worst-case retry request budget, and estimated input tokens, then exit **without any API call**. With supplied token prices it also shows an input-only cost floor. |

Importance ranking favors the homepage, pages linked from it, shallow click-depth, hub/navigation pages, sitemap presence, and short URL paths. Only internal HTML pages with HTTP 200 are eligible.

**Tuning:**

| Parameter | Description |
|-----------|-------------|
| `--ai-max-concurrency=<int>` | Maximum concurrent AI requests. Default `4`. |
| `--ai-max-reqs-per-sec=<val>` | Shared LLM API rate limit applied at every actual HTTP send, including transport and parse retries. |
| `--ai-timeout=<int>` | Per-request timeout for AI calls in seconds. Default `180`. Raise it for slow reasoning models. |
| `--ai-cache-dir=<dir>` | Directory for content-addressed AI responses. Parse-invalid and truncated entries are evicted immediately. Default `tmp/ai-cache`; empty disables caching. |
| `--ai-input-cost-per-million=<usd>` + `--ai-output-cost-per-million=<usd>` | Optional model-specific rates, supplied together. The artifact records rates, reported token use, and a complete/partial USD estimate without maintaining a potentially stale built-in price table. |
| `--ai-language=<code>` | Force content language (BCP-47, e.g. `cs`, `de`) for `typos`. Auto-detected otherwise. |
| `--ai-report-language=<code>` | Output language for report prose and chrome. Built-in chrome: English and Czech; other locales use English chrome fallback. |
| `--ai-report-dir=<dir>` | Paired JSON/HTML AI artifact directory. Default `tmp/`; filenames always include a unique run ID. |
| `--ai-seo-affects-score` | Let the AI SEO assessment apply a small capped deduction to the SEO quality score. **Off by default** — AI is advisory and never affects the `--ci` gate, keeping the score deterministic and reproducible. |

#### Following AI requests

Every LLM request is reported on stderr as its response arrives: the task and its progress, the page or stage, input and output tokens (with the reasoning part when the provider counts it), the time of the HTTP attempt and the output speed:

```
  AI ✓ #12 SEO 12/40 · /blog/post · 3,412 in · 812 out (540 reasoning) · 6.8 s · 119 tok/s
  AI ✓ #13 Profile: chapters 3/12 · Services · 9,870 in · 1,944 out (reasoning n/a) · 21.4 s · 91 tok/s
  AI ✓ #14 Typos 3/40 · /about · tokens not reported · 2.1 s
  AI ↻ #15 SEO 13/40 · /blog/x · HTTP 429 · 0.4 s · retrying (attempt 2/3)
  AI ✗ #16 SEO 13/40 · /blog/x · AI provider error: model not found · 0.2 s
  AI ⇢ #17 SEO 14/40 · /contact · cache hit · 1,020 in · 88 out
```

`✓` answered, `↻` will be retried (HTTP 429/5xx or a connection error), `✗` failed, `⇢` answered from `--ai-cache-dir`. Output tokens include the reasoning; `(reasoning n/a)` means the response carried reasoning text without a count (e.g. MiniMax), and a response without usage still gets its line. The time covers only the HTTP attempt (send to body), never rate-limit waits or retry pauses. `--hide-progress-bar` hides the lines. The per-category token lines at the end of the run add the reasoning total and the average output speed. A host gets the same data as `aiRequest`, `aiProgress` and `aiUsage` events of the `--events-file` stream ([docs/EVENTS.md](docs/EVENTS.md)).

#### Model list and connection check

Two utility modes help pick and test a model (a GUI uses them for its model picker) without crawling: no `--url`, exactly one JSON object on stdout — also on failure, as `{"ok":false,"error":"…"}` with exit code `1` (`101` for a configuration error) — and never the API key. They use the connection options exactly as a crawl does (`--ai-provider`, `--ai-endpoint`, the `--ai-api-key*` options, `--ai-extra-body`, `--ai-timeout`, …).

| Parameter | Description |
|-----------|-------------|
| `--ai-list-models` | Print the models the configured AI endpoint offers (with context window when known) as JSON, then exit. Needs no `--ai-model`. Anthropic and Gemini also report display names and output limits; OpenAI lists ids only. |
| `--ai-check` | Send one short test request to the configured AI model, print its statistics as JSON, then exit. The AI cache is neither read nor written; the request's line (see above) goes to stderr. |

```bash
./siteone-crawler --ai-provider=openai-compatible --ai-endpoint=http://localhost:8000/v1 --ai-list-models
{"ok":true,"provider":"openai-compatible","endpoint":"http://localhost:8000/v1","models":[{"id":"nvidia/Qwen3.8-Flash-Next-NVFP4","displayName":null,"contextWindow":262144,"maxOutputTokens":null}]}

./siteone-crawler --ai-provider=openai-compatible --ai-endpoint=http://localhost:8000/v1 \
  --ai-model=nvidia/Qwen3.8-Flash-Next-NVFP4 --ai-check
{"ok":true,"provider":"openai-compatible","model":"nvidia/Qwen3.8-Flash-Next-NVFP4","ms":275,"inputTokens":17,"outputTokens":37,"reasoningTokens":33,"cachedInputTokens":0,"outputTokensPerSecond":134.5,"finishReason":"stop","reply":"OK"}
```

What the provider does not report (e.g. a reasoning count) is left out of the `--ai-check` answer, never written as `0`.

#### Thinking / reasoning

Thinking/reasoning is controlled via the universal `--ai-extra-body` JSON, which is deep-merged into the request (overriding native fields). This avoids a separate switch for every provider's differing convention:

```bash
# vLLM / Qwen (openai-compatible) — disable thinking
--ai-extra-body='{"chat_template_kwargs":{"enable_thinking":false}}'

# OpenAI reasoning effort
--ai-extra-body='{"reasoning_effort":"minimal"}'

# Anthropic extended thinking with a token budget
--ai-extra-body='{"thinking":{"type":"enabled","budget_tokens":2048}}'

# Google Gemini — disable thinking
--ai-extra-body='{"generationConfig":{"thinkingConfig":{"thinkingBudget":0}}}'
```

For high-volume per-page analysis, thinking is usually unnecessary and increases cost/latency. Models that emit inline `<think>...</think>` (e.g. MiniMax M3) are handled automatically — the reasoning is stripped before parsing.

#### Custom prompts

`--ai-prompt-file=<file>` (or inline `--ai-prompt=<text>`) runs your prompt against each selected page. Reference page data with placeholders — the crawler injects each value **sanitized and wrapped in an XML data-boundary tag**, so a naive prompt is still injection-safe:

`{{url}}`, `{{title}}`, `{{meta_description}}`, `{{meta_keywords}}`, `{{h1}}`, `{{headings}}`, `{{content_markdown}}`, `{{lang}}`.

The model is instructed to return a JSON array of findings `{severity, label, message, location}`. Put your static instructions at the **top** of the prompt and reference `{{content_markdown}}` near the **bottom** to maximize provider prefix-cache reuse across pages.

**Example** — EU advertising-compliance check (`compliance.txt`):

```text
You are an EU advertising-law compliance reviewer. Flag unsubstantiated superlatives
("best", "cheapest", "#1"), health/financial claims without disclosure, and misleading
urgency. Report each issue as a finding. Page content:

{{content_markdown}}
```

```bash
./siteone-crawler --url=https://example.com/ \
  --ai-provider=openai --ai-model=gpt-5-mini \
  --ai-actions=custom --ai-prompt-file=compliance.txt
```

> **Prompt-injection & privacy:** crawled content is wrapped in XML data tags with angle brackets escaped, and prompts instruct the model to treat it as data. Still, only send content you are comfortable sharing with the configured provider (providers may use prompt caching).

## 🏆 Quality Scoring

The crawler automatically calculates a quality score (0.0-10.0) across 5 weighted categories:

| Category | Weight | What it measures |
|----------|--------|------------------|
| **Performance** | 20% | Response times, slow URLs |
| **SEO** | 20% | Missing H1, title uniqueness, meta descriptions, 404s, redirects |
| **Security** | 25% | SSL/TLS certificates (incl. expiry within 14 days), security headers, unsafe protocols, insecure cipher suites |
| **Accessibility** | 20% | Lang attribute, image alt text, form labels, unnamed links/buttons, main landmark, HTML structure (duplicate ids, broken ARIA references), heading levels |
| **Best Practices** | 15% | Duplicate/large SVGs, deep DOM, Brotli/WebP support |

The overall score is a weighted average of all categories. Scores are displayed in a colored box in the console output and included in JSON and HTML report outputs.

Score labels:
- **9.0-10.0** — Excellent (green)
- **7.0-8.9** — Good (blue)
- **5.0-6.9** — Fair (yellow)
- **3.0-4.9** — Poor (purple)
- **0.0-2.9** — Critical (red)

## 🔄 CI/CD Integration

The `--ci` flag enables a quality gate that evaluates configurable thresholds after crawling completes. When any threshold is not met, the crawler exits with **code 10** (distinct from exit code 1 for runtime errors). In CI mode, default file outputs (HTML, JSON, TXT reports) are automatically suppressed — only the console output and exit code matter. If you need report files in CI, specify them explicitly (e.g. `--output-html-report=report.html`). To keep job logs small (GitLab stops a job log at 4 MB by default), `--ci` also replaces the per-URL table with at most one progress line every 10 seconds, printed as URLs finish, and keeps only the rows of failed URLs (4xx/5xx, connection errors, timeouts) so the log still shows what failed; set `--progress-interval` to change the interval (`0` = every row).

**Bonus: Cache warming** — running the crawler as a post-deployment step in your CI/CD pipeline crawls every page and asset on your site, which populates the HTML/asset cache on your **reverse proxy** (Varnish, Nginx) or **CDN** (Cloudflare, CloudFront). This way, the first real visitors always hit a warm cache instead of cold origin requests.

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success (with `--ci` this also means all quality thresholds passed) |
| `1` | Runtime error |
| `2` | Help/version displayed |
| `3` | No pages crawled (e.g. DNS failure, timeout, connection refused) |
| `10` | CI/CD quality gate failed |
| `101` | Configuration error |

### Example: GitHub Actions

```yaml
- name: Check website quality
  run: |
    ./siteone-crawler \
      --url=https://staging.example.com \
      --ci \
      --ci-min-score=7.0 \
      --ci-min-security=8.0 \
      --ci-max-404=0 \
      --ci-max-5xx=0 \
      --ci-junit-file=crawler-junit.xml \
      --ci-github-annotations
```

`--ci-github-annotations` surfaces each failed check inline in the PR (and is auto-enabled under
`GITHUB_ACTIONS=true`), while `--ci-junit-file` produces a JUnit report you can upload with a
test-reporter action. To gate on **regressions** instead of absolute floors, keep a baseline JSON
from the previous run and compare against it:

```yaml
- name: Check for quality regressions
  run: |
    ./siteone-crawler --url=https://staging.example.com --ci \
      --ci-baseline=baseline.json --ci-max-score-drop=0.3 \
      --output-json-file=current.json
```

### Example: GitLab CI

```yaml
quality_check:
  script:
    - ./siteone-crawler --url=$STAGING_URL --ci --ci-min-score=6.0
  allow_failure: false
```

### Console output

When `--ci` is enabled, a quality gate box is displayed after the quality scores:

```
╔══════════════════════════════════════════════════════════════╗
║                      CI/CD QUALITY GATE                      ║
╠══════════════════════════════════════════════════════════════╣
║  [PASS] Overall score: 7.2 >= 5                              ║
║  [PASS] 404 errors: 0 <= 0                                   ║
║  [PASS] 5xx errors: 0 <= 0                                   ║
║  [FAIL] Critical findings: 2 > 0 (max: 0)                    ║
╠══════════════════════════════════════════════════════════════╣
║  RESULT: FAIL (1 of 4 checks failed) — exit code 10          ║
╚══════════════════════════════════════════════════════════════╝
```

### JSON output

When using `--output=json --ci`, the JSON includes a `ciGate` object:

```json
{
  "ciGate": {
    "passed": false,
    "exitCode": 10,
    "checks": [
      {"metric": "Overall score", "operator": ">=", "threshold": 5.0, "actual": 7.2, "passed": true},
      {"metric": "404 errors", "operator": "<=", "threshold": 0.0, "actual": 0.0, "passed": true},
      {"metric": "Critical findings", "operator": "<=", "threshold": 0.0, "actual": 2.0, "passed": false}
    ]
  }
}
```

## 📄 Output Examples

To understand the richness of the data provided by the crawler, you can examine real output examples generated from crawling `crawler.siteone.io`:

*   **Text Output Example:** [`docs/OUTPUT-crawler.siteone.io.txt`](docs/OUTPUT-crawler.siteone.io.txt)
    *   Provides a human-readable summary suitable for quick review.
    *   See the detailed [Text Output Documentation](docs/TEXT-OUTPUT.md).
*   **JSON Output Example:** [`docs/OUTPUT-crawler.siteone.io.json`](docs/OUTPUT-crawler.siteone.io.json)
    *   Provides structured data ideal for programmatic consumption and detailed analysis.
    *   See the detailed [JSON Output Documentation](docs/JSON-OUTPUT.md).

These examples showcase the various tables and metrics generated, demonstrating the tool's capabilities in analyzing website structure, performance, SEO, security, and more.

## 🧪 Testing

```bash
cargo test                                       # unit tests + offline integration tests
cargo test --test integration_crawl -- --ignored --test-threads=1  # network integration tests (crawls crawler.siteone.io)
```

Unit tests live in each source file (`#[cfg(test)] mod tests`). Integration tests are in `tests/integration_crawl.rs` — network-dependent tests are `#[ignore]` by default so that `cargo test` stays fast and offline.

## ⚠️ Disclaimer

Please use responsibly and ensure that you have the necessary permissions when crawling websites. Some sites may have
rules against automated access detailed in their robots.txt.

**The author is not responsible for any consequences caused by inappropriate use or deliberate misuse of this tool.**

## 📜 License

This work is licensed under a [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT) license.

## Powered by

[![Hosted By: Cloudsmith](https://img.shields.io/badge/OSS%20hosting%20by-cloudsmith-blue?logo=cloudsmith&style=for-the-badge)](https://cloudsmith.com)

Package repository hosting is graciously provided by  [Cloudsmith](https://cloudsmith.com).
Cloudsmith is the only fully hosted, cloud-native, universal package management solution, that
enables your organization to create, store and share packages in any format, to any place, with total
confidence.

[![PhpStorm logo.](https://resources.jetbrains.com/storage/products/company/brand/logos/PhpStorm.svg)](https://jb.gg/OpenSourceSupport)
