# SiteOne Crawler

SiteOne Crawler is a powerful and easy-to-use **website analyzer, cloner, and converter** designed for developers seeking security and performance insights, SEO specialists identifying optimization opportunities, and website owners needing reliable backups and offline versions.

**Now rewritten in Rust** for maximum performance, minimal resource usage, and zero runtime dependencies. The transition from PHP+Swoole to Rust resulted in **25% faster execution** and **30% lower memory consumption** while producing identical output.

**Discover the SiteOne Crawler advantage:**

*   **Run Anywhere:** Single native binary for **🪟 Windows**, **🍎 macOS**, and **🐧 Linux** (x64 & arm64). No runtime dependencies.
*   **Work Your Way:** Launch the binary without arguments for an **interactive wizard** 🧙 with 10 preset modes, use the extensive **command-line interface** 📟 ([releases](https://github.com/janreges/siteone-crawler/releases), [▶️ video](https://www.youtube.com/watch?v=25T_yx13naA&list=PL9mElgTe-s1Csfg0jXWmDS0MHFN7Cpjwp)) for automation and power, or enjoy the intuitive **desktop GUI application** 💻 ([GUI app](https://github.com/janreges/siteone-crawler-gui), [▶️ video](https://www.youtube.com/watch?v=rFW8LNEVNdw)) for visual control.
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
        + [CI/CD settings](#cicd-settings)
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
  which will be processed as a list of URLs. In sitemap-only mode, the crawler follows only URLs from
  the sitemap — it does not discover additional links from HTML pages. Gzip-compressed sitemaps (`*.xml.gz`)
  are fully supported, both as direct URLs and when referenced from sitemap index files.
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

💡 Tip: you can push the exported markdown folder to your GitHub repository, where it will be automatically rendered as a browsable
documentation. You can look at the [examples](https://github.com/janreges/siteone-crawler-markdown-examples/) of converted websites to markdown.

See all available [markdown exporter options](#markdown-exporter-options).

### 🗺️ Sitemap generator

- will help you create a `sitemap.xml` and `sitemap.txt` for your website
- you can set the priority of individual pages based on the number of slashes in the URL

Don't hesitate and try it. You will love it as we do! ❤️

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

Requires [Rust](https://www.rust-lang.org/tools/install) 1.85 or later.

```bash
git clone https://github.com/janreges/siteone-crawler.git
cd siteone-crawler

# Build optimized release binary
cargo build --release

# Run
./target/release/siteone-crawler --url=https://my.domain.tld
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
configuration. Choose from 10 preset modes, enter the target URL, fine-tune settings with
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
  --markdown-replace-query-string='/[a-z]+=[^&]*(&|$)/i -> $1__$2' \
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
| `--url=<url>` | Required. HTTP or HTTPS URL address of the website or sitemap xml to be crawled.<br>Use quotation marks `''` if the URL contains query parameters. |
| `--single-page` | Load only one page to which the URL is given (and its assets), but do not follow other pages. |
| `--max-depth=<int>` | Maximum crawling depth (for pages, not assets). Default is `0` (no limit). `1` means `/about`<br>or `/about/`, `2` means `/about/contacts` etc. |
| `--device=<val>` | Device type for choosing a predefined User-Agent. Ignored when `--user-agent` is defined.<br>Supported values: `desktop`, `mobile`, `tablet`. Default is `desktop`. |
| `--user-agent=<val>` | Custom User-Agent header. Use quotation marks. If specified, it takes precedence over<br>the device parameter. If you add `!` at the end, the siteone-crawler/version will not be<br>added as a signature at the end of the final user-agent. |
| `--timeout=<int>` | Request timeout in seconds. Default is `5`. |
| `--proxy=<host:port>` | HTTP proxy to use in `host:port` format. Host can be hostname, IPv4 or IPv6. |
| `--http-auth=<user:pass>` | Basic HTTP authentication in `username:password` format. |
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
| `--add-random-query-params` | Add random query parameters to each URL to bypass caches. |
| `--transform-url=<from->to>` | Transform URLs before crawling. Use `from -> to` for simple replacement or `/regex/ -> replacement`.<br>Can be specified multiple times. |
| `--force-relative-urls` | Normalize all discovered URLs matching the initial domain (incl. www variant and protocol<br>differences) to canonical form. Prevents duplicate files in offline export when the site<br>uses inconsistent URL formats (http/https, www/non-www). |
| `--ignore-robots-txt` | Ignore robots.txt content. |
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
| `--html-report-options=<sections>` | Comma-separated list of sections to include in HTML report.<br>Available sections: `summary`, `seo-opengraph`, `image-gallery`, `video-gallery`, `visited-urls`, `dns-ssl`, `crawler-stats`, `crawler-info`, `headers`, `content-types`, `skipped-urls`, `external-links`, `caching`, `best-practices`, `accessibility`, `security`, `redirects`, `404-pages`, `slowest-urls`, `fastest-urls`, `source-domains`.<br>Default: all sections. |
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
| `--offline-export-preserve-url-structure` | Preserve the original URL path structure. E.g. `/about` is stored as `about/index.html`<br>instead of `about.html`. Useful for web server deployment where the clone should maintain<br>the same URL hierarchy as the original site. |
| `--replace-content=<val>` | Replace content in HTML/JS/CSS with `foo -> bar` or PCRE regexp.<br>Can be specified multiple times. |
| `--replace-query-string=<val>` | Replace characters in query string filenames.<br>Can be specified multiple times. |
| `--offline-export-lowercase` | Convert all filenames to lowercase for offline export. Useful for case-insensitive filesystems. |
| `--ignore-store-file-error` | Ignore any file storing errors and continue. |
| `--disable-astro-inline-modules` | Disable inlining of Astro module scripts for offline export.<br>Scripts will remain as external files with corrected relative paths. |

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
| `--markdown-replace-query-string=<val>` | Replace characters in query string filenames.<br>Can be specified multiple times. |
| `--markdown-export-store-only-url-regex=<regex>` | Debug: store only URLs matching these PCRE regexes. Can be specified multiple times. |
| `--markdown-ignore-store-file-error` | Ignore any file storing errors and continue. |

### Sitemap options

| Parameter | Description |
|-----------|-------------|
| `--sitemap-xml-file=<file>` | File path for generated XML Sitemap. Extension `.xml` added if not specified. |
| `--sitemap-txt-file=<file>` | File path for generated TXT Sitemap. Extension `.txt` added if not specified. |
| `--sitemap-base-priority=<num>` | Base priority for XML sitemap. Default is `0.5`. |
| `--sitemap-priority-increase=<num>` | Priority increase based on slashes in URL. Default is `0.1`. |

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

### CI/CD settings

| Parameter | Description |
|-----------|-------------|
| `--ci` | Enable CI/CD quality gate. Crawler exits with code 10 if thresholds are not met. Default file outputs (HTML, JSON, TXT reports) are suppressed unless explicitly requested via `--output-*` options. |
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

**Default behavior with `--ci` alone:** overall score >= 5.0, each category score >= 5.0 (Performance, SEO, Security, Best Practices) and Accessibility >= 3.0, 404 errors <= 0, 5xx errors <= 0, critical findings <= 0, HTML pages >= 10, assets >= 10. File outputs (HTML, JSON, TXT reports) are not generated. To save reports in CI mode, specify the desired output explicitly, e.g. `--ci --output-html-report=report.html`.

## 🏆 Quality Scoring

The crawler automatically calculates a quality score (0.0-10.0) across 5 weighted categories:

| Category | Weight | What it measures |
|----------|--------|------------------|
| **Performance** | 20% | Response times, slow URLs |
| **SEO** | 20% | Missing H1, title uniqueness, meta descriptions, 404s, redirects |
| **Security** | 25% | SSL/TLS certificates, security headers, unsafe protocols |
| **Accessibility** | 20% | Lang attribute, image alt text, form labels, ARIA, heading levels |
| **Best Practices** | 15% | Duplicate/large SVGs, deep DOM, Brotli/WebP support |

The overall score is a weighted average of all categories. Scores are displayed in a colored box in the console output and included in JSON and HTML report outputs.

Score labels:
- **9.0-10.0** — Excellent (green)
- **7.0-8.9** — Good (blue)
- **5.0-6.9** — Fair (yellow)
- **3.0-4.9** — Poor (purple)
- **0.0-2.9** — Critical (red)

## 🔄 CI/CD Integration

The `--ci` flag enables a quality gate that evaluates configurable thresholds after crawling completes. When any threshold is not met, the crawler exits with **code 10** (distinct from exit code 1 for runtime errors). In CI mode, default file outputs (HTML, JSON, TXT reports) are automatically suppressed — only the console output and exit code matter. If you need report files in CI, specify them explicitly (e.g. `--output-html-report=report.html`).

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
      --ci-max-5xx=0
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
