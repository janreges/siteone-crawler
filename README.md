# SiteOne Crawler

SiteOne Crawler is a powerful and easy-to-use **website analyzer, cloner, and converter** designed for developers seeking security and performance insights, SEO specialists identifying optimization opportunities, and website owners needing reliable backups and offline versions.

**Discover the SiteOne Crawler advantage:**

*   **Run Anywhere:** Native support for **🪟 Windows**, **🍎 macOS**, and **🐧 Linux** (x64 & arm64). No complex setup needed.
*   **Work Your Way:** Master the extensive **command-line interface** 📟 ([releases](https://github.com/janreges/siteone-crawler/releases), [▶️ video](https://www.youtube.com/watch?v=25T_yx13naA&list=PL9mElgTe-s1Csfg0jXWmDS0MHFN7Cpjwp)) for automation and power, or enjoy the intuitive **desktop GUI application** 💻 ([GUI app](https://github.com/janreges/siteone-crawler-gui), [▶️ video](https://www.youtube.com/watch?v=rFW8LNEVNdw)) for visual control.
*   **Actionable Insights:** Generate comprehensive **HTML reports** 📊 packed with data on performance, SEO, accessibility, security, and more (see [nextjs.org sample](https://crawler.siteone.io/html/2024-08-23/forever/cl8xw4r-fdag8wg-44dd.html)). Find and fix problems faster.
*   **Offline & Markdown Power:** Create complete **offline clones** 💾 for browsing without a server ([nextjs.org clone](https://crawler.siteone.io/examples-exports/nextjs.org/)) or convert entire websites into clean **Markdown** 📝 – perfect for backups, documentation, or feeding content to AI models ([examples](https://github.com/janreges/siteone-crawler-markdown-examples/)).
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
    * [🗺️ Sitemap generator](#️-sitemap-generator)
    * [🤝 For active contributors](#-for-active-contributors)
- [🚀 Installation](#-installation)
    * [📦 Ready-to-use releases](#-ready-to-use-releases)
    * [🐧 Linux (x64)](#-linux-x64)
    * [🪟 Windows (x64)](#-windows-x64)
    * [🍎 macOS (arm64, x64)](#-macos-arm64-x64)
    * [🐧 Linux (arm64)](#-linux-arm64)
- [▶️ Usage](#️-usage)
    * [Basic example](#basic-example)
    * [Fully-featured example](#fully-featured-example)
    * [Arguments](#️-arguments)
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
- [📄 Output Examples](#-output-examples)
- [🎯 Roadmap](#-roadmap)
- [🤔 Motivation to create this tool](#-motivation-to-create-this-tool)
- [⚠️ Disclaimer](#️-disclaimer)
- [📜 License](#-license)

## ✨ Features

In short, the main benefits can be summarized in these points:

- **🕷️ Crawler** - very powerful crawler of the entire website reporting useful information about each URL (status code,
  response time, size, custom headers, titles, etc.)
- **🛠️ Dev/DevOps assistant** - offers a set of very useful and often necessary features for developers and devops (stress
  test, warm up cache, localhost testing, etc.)
- **📊 Analyzer** - analyzes all webpages and reports strange or error behaviour and useful statistics (404, redirects, bad
  practices, SEO and security issues, heading structures, etc.)
- **📧 Reporter** - sends a HTML report to your email addresses with all the information about the crawled website
- **💾 Offline website generator** - allows you to export the entire website to offline form, where it is possible to
  browse the site through local HTML files (without HTTP server) including all images, styles, scripts, fonts, etc.
- **📝 Website to markdown converter** - allows you to export/convert the entire website with all subpages to browsable markdown.
  Optionally with images and other files (PDF, etc.) included. Great for documentation, archiving, and **feeding content to AI tools** that process markdown better.
  See [markdown examples](https://github.com/janreges/siteone-crawler-markdown-examples/).
- **🗺️ Sitemap generator** - allows you to generate `sitemap.xml` and `sitemap.txt` files with a list of all pages on your
  website

The following features are summarized in greater detail:

### 🕷️ Crawler

- **all major platforms** supported without complicated installation or dependencies (🐧 Linux, 🪟 Windows, 🍎 macOS, arm64)
- has incredible **🚀 C++ performance** (thanks to Swoole's coroutines)
- provide simulation of **different device types** (desktop/mobile/tablet) thanks to predefined User-Agents
- will crawl **all files**, styles, scripts, fonts, images, documents, etc. on your website
- will respect the `robots.txt` file and will not crawl the pages that are not allowed
- has a **beautiful interactive** and **🎨 colourful output**
- it will **clearly warn you** ⚠️ of any wrong use of the tool (e.g. input parameters validation or wrong permissions)
- **captures CTRL+C** and ends with the statistics for at least the current processed URLs
- as `--url` parameter, you can specify also a `sitemap.xml` file (or [sitemap index](https://www.sitemaps.org/protocol.html#index)),
  which will be processed as a list of URLs. Note: gzip pre-compressed sitemaps `*.xml.gz` are not supported.

### 🛠️ Dev/DevOps assistant

- allows testing **public** and **local projects on specific ports** (e.g. `http://localhost:3000/`)
- will perform a **stress test** and allow you to test the protection of the infrastructure against DoS attacks
- will help you **warm up the application cache** or the **cache on the reverse proxy** of the entire website

### 📊 Analyzer

- will **find the weak points** or **strange behavior** of your website
- allows you to implement **your own analyzers** by simply adding an analyzer class that implements
  the `Crawler\Analyzer` interface.

### 📧 Reporter

- will provide you with data for **SEO analysis**, just add the `Title`, `Keywords` and `Description`
- will send you a **nice HTML report** to your email addresses
- will **export** the output to JSON, HTML or text for **your integrations**
  will provide useful **summaries and statistics** at the end of the processing

### 💾 Offline website generator

- will help you **export the entire website** to offline form, where it is possible to browse the site through local
  HTML files (without HTTP server) including all document, images, styles, scripts, fonts, etc.
- you can **limit what assets** you want to download and export (see `--disable-*` directives) .. for some types of
  websites the best result is with the `--disable-javascript` option.
- you can specify by `--allowed-domain-for-external-files` (short `-adf`) from which **external domains** it is possible to **download
  ** assets (JS, CSS, fonts, images, documents) including `*` option for all domains.
- you can specify by `--allowed-domain-for-crawling` (short `-adc`) which **other domains** should be included in the **crawling** if
  there are any links pointing to them. You can enable e.g. `mysite.*` to export all language mutations that have a
  different TLD or `*.mysite.tld` to export all subdomains.
- you can try `---disable-styles` and `---disable-fonts` and see how well you handle **accessibility** and **semantics**
- you can use `--single-page` to **export only one page** to which the URL is given (and its assets), but do not follow
  other pages.
- you can use `--single-foreign-page` to **export only one page** from another domain (if allowed by `--allowed-domain-for-crawling`),
  but do not follow other pages.
- you can use `--replace-content` to **replace content** in HTML/JS/CSS with `foo -> bar` or regexp in PREG format, e.g.
  `/card[0-9]/i -> card`. Can be specified multiple times.
- you can use `--replace-query-string` to **replace chars in query string** in the filename.
- you can use `--max-depth` to set the **maximum crawling depth** (for pages, not assets). `1` means `/about` or `/about/`,
  `2` means `/about/contacts` etc.
- you can use it to **export your website to a static form** and host it on GitHub Pages, Netlify, Vercel, etc. as a
  static backup and part of your **disaster recovery plan** or **archival/legal needs**
- works great with **older conventional websites** but also **modern ones**, built on frameworks like Next.js, Nuxt.js,
  SvelteKit, Astro, Gatsby, etc. When a JS framework is detected, the export also performs some framework-specific code
  modifications for optimal results. For example, most frameworks can't handle the relative location of a project and
  linking assets from root `/`, which doesn't work with `file://` mode.
- **try it** for your website, and you will be very pleasantly surprised :-)
- roadmap: we are also planning to release a version of the export compatible with **Nginx** that will preserve all
  original URLs for your website and allow you to host it on your own infrastructure.

### 📝 Website to markdown converter

- will help you **export/convert the entire website** with all subpages to **browsable markdown**. This is particularly useful for feeding website content (like documentation) into **AI tools** that often handle markdown more effectively than raw HTML.
- you can optionally disable export and hide images and other files (PDF, etc.) which are included by default.
- you can set multiple selectors (CSS like) to **remove unwanted elements** from the exported markdown.
- to prevent that at the beginning of the markdown of all pages the header, long menu etc. will be repeated, so the
  first occurrence of the most important heading (typically h1) is searched and all content before this heading is moved
  to the end of the page below the line `---`.
- converter has implemented **code block detection** and **syntax highlighting** for the most popular languages.
- html tables are converted to **markdown tables**.
- can combine all exported markdown files into a **single large markdown file** - ideal for AI tools that need to
  process the entire website content in one go.
- smart removal of duplicate website headers and footers is also implemented in exported single large markdown file.
- see all available [markdown exporter options](#markdown-exporter-options).

💡 Tip: you can push the exported markdown folder to your GitHub repository, where it will be automatically rendered as a browsable
documentation. You can look at the [examples](https://github.com/janreges/siteone-crawler-markdown-examples/) of converted websites to markdown.

### 🗺️ Sitemap generator

- will help you create a `sitemap.xml` and `sitemap.xml` for your website
- you can set the priority of individual pages based on the number of slashes in the URL

Don't hesitate and try it. You will love it as we do! ❤️

### 🤝 For active contributors

- the crawler code provides some useful functionality that facilitates further **development** and **extensibility** of
  the project

## 🚀 Installation

### 📦 Ready-to-use releases

You can download ready-to-use releases from [🐙 GitHub releases](https://github.com/janreges/siteone-crawler/releases) for all major platforms (🐧 Linux, 🪟 Windows, 🍎 macOS, arm64).

Unpack the downloaded archive, and you will find the `crawler` or `crawler.bat` (Windows) executable binary and run crawler by `./crawler --url=https://my.domain.tld`.

**Note for Windows users**: use Cygwin-based release `*-win-x64.zip` only if you can't use WSL (Ubuntu/Debian), what is recommended. If you really have to use the Cygwin version, set `--workers=1` for higher stability.

**Note for macOS users**: In case that Mac refuses to start the crawler from your Download folder, move the entire folder with the Crawler **via the terminal** to another location, for example to the homefolder `~`.

### 🐧 Linux (x64)

Most easily installation is on most Linux (x64) distributions.

```bash
git clone https://github.com/janreges/siteone-crawler.git
cd siteone-crawler

# run crawler with basic options
./crawler --url=https://my.domain.tld
```

### 🪟 Windows (x64)

If using Windows, the best choice is to use [Ubuntu](https://ubuntu.com/wsl)
or [Debian](https://www.linuxfordevices.com/tutorials/linux/install-debian-on-windows-wsl)
in [WSL](https://learn.microsoft.com/en-us/windows/wsl/install).

Otherwise, you can
download [swoole-cli-v4.8.13-cygwin-x64.zip](https://github.com/swoole/swoole-src/releases/download/v4.8.13/swoole-cli-v4.8.13-cygwin-x64.zip)
from [Swoole releases](https://github.com/swoole/swoole-src/releases) and use precompiled
Cygwin-based `bin/swoole-cli.exe`.

A really functional and tested Windows command looks like this (modify path to your `swoole-cli.exe` and `src\crawler.php`):

```bash
c:\Tools\swoole-cli-v4.8.13-cygwin-x64\bin\swoole-cli.exe C:\Tools\siteone-crawler\src\crawler.php --url=https://www.siteone.io/
```

**NOTICE**: Cygwin does not support STDERR with rewritable lines in the console. Therefore, the output is not as
beautiful as on Linux or macOS.

### 🍎 macOS (arm64, x64)

If using macOS with latest arm64 M1/M2 CPU, download arm64
version [swoole-cli-v4.8.13-macos-arm64.tar.xz](https://github.com/swoole/swoole-src/releases/download/v4.8.13/swoole-cli-v4.8.13-macos-arm64.tar.xz),
unpack and use its precompiled `swoole-cli`.

If using macOS with Intel CPU (x64), download x64
version  [swoole-cli-v4.8.13-macos-x64.tar.xz](https://github.com/swoole/swoole-src/releases/download/v4.8.13/swoole-cli-v4.8.13-macos-x64.tar.xz),
unpack and use its precompiled `swoole-cli`.

### 🐧 Linux (arm64)

If using arm64 Linux, you can
download [swoole-cli-v4.8.13-linux-arm64.tar.xz](https://github.com/swoole/swoole-src/releases/download/v4.8.13/swoole-cli-v4.8.13-linux-arm64.tar.xz)
and use its precompiled `swoole-cli`.

## ▶️ Usage

To run the crawler, execute the `crawler` executable file from the command line and provide the
required arguments:

### Basic example

```bash
./crawler --url=https://mydomain.tld/ --device=mobile
```

### Fully-featured example

```bash
./crawler --url=https://mydomain.tld/ \
  --output=text \
  --workers=2 \
  --max-reqs-per-sec=10 \
  --memory-limit=1024M \
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
  --sitemap-xml-file==/dir/sitemap.xml \
  --sitemap-txt-file==/dir/sitemap.txt \
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
  --mail-from=crawler@ymy-mail.tld \
  --mail-from-name="SiteOne Crawler" \
  --mail-subject-template="Crawler Report for %domain% (%date%)" \
  --mail-smtp-host=smtp.my-mail.tld \
  --mail-smtp-port=25 \
  --mail-smtp-user=smtp.user \
  --mail-smtp-pass=secretPassword123
```

## ⚙️ Arguments

For a clearer list, I recommend going to the documentation: 🌐 https://crawler.siteone.io/configuration/command-line-options/

### Basic settings

| Parameter | Description |
|-----------|-------------|
| `--url=<url>` | Required. HTTP or HTTPS URL address of the website or sitemap xml to be crawled.<br>Use quotation marks `''` if the URL contains query parameters. |
| `--single-page` | Load only one page to which the URL is given (and its assets), but do not follow other pages. |
| `--max-depth=<int>` | Maximum crawling depth (for pages, not assets). Default is `0` (no limit). `1` means `/about`<br>or `/about/`, `2` means `/about/contacts` etc. |
| `--device=<val>` | Device type for choosing a predefined User-Agent. Ignored when `--user-agent` is defined.<br>Supported values: `desktop`, `mobile`, `tablet`. Defaults is `desktop`. |
| `--user-agent=<val>` | Custom User-Agent header. Use quotation marks. If specified, it takes precedence over<br>the device parameter. If you add `!` at the end, the siteone-crawler/version will not be<br>added as a signature at the end of the final user-agent. |
| `--timeout=<int>` | Request timeout in seconds. Default is `3`. |
| `--proxy=<host:port>` | HTTP proxy to use in `host:port` format. Host can be hostname, IPv4 or IPv6. |
| `--http-auth=<user:pass>` | Basic HTTP authentication in `username:password` format. |

### Output settings

| Parameter | Description |
|-----------|-------------|
| `--output=<val>` | Output type. Supported values: `text`, `json`. Default is `text`. |
| `--extra-columns=<values>` | Comma delimited list of extra columns added to output table. You can specify HTTP headers<br>(e.g. `X-Cache`), predefined values (`Title`, `Keywords`, `Description`, `DOM`), or custom<br>extraction from text files (HTML, JS, CSS, TXT, JSON, XML, etc.) using XPath or regexp.<br>For custom extraction, use the format `Custom_column_name=method:pattern#group(length)`, where<br>`method` is `xpath` or `regexp`, `pattern` is the extraction pattern, an optional `#group` specifies the<br>capturing group (or node index for XPath) to return (defaulting to the entire match or first node), and an<br>optional `(length)` sets the maximum output length (append `>` to disable truncation).<br>For example, use `Heading1=xpath://h1/text()(20>)` to extract the text of the first H1 element<br>from the HTML document, and `ProductPrice=regexp:/Price:\s*\$?(\d+(?:\.\d{2})?)/i#1(10)`<br>to extract a numeric price (e.g., "29.99") from a string like "Price: $29.99". |
| `--url-column-size=<num>` | Basic URL column width. By default, it is calculated from the size of your terminal window. |
| `--rows-limit=<num>` | Max. number of rows to display in tables with analysis results (protection against very long and slow report).<br>Default is `200`. |
| `--timezone=<val>` | Timezone for datetimes in HTML reports and timestamps in output folders/files, e.g. `Europe/Prague`.<br>Default is `UTC`. Available values can be found at [Timezones Documentation](https://www.php.net/manual/en/timezones.php). |
| `--do-not-truncate-url` | In the text output, long URLs are truncated by default to `--url-column-size` so the table does not<br>wrap due to long URLs. With this option, you can turn off the truncation. |
| `--show-scheme-and-host` | On text output, show scheme and host also for origin domain URLs. |
| `--hide-progress-bar` | Hide progress bar visible in text and JSON output for more compact view. |
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
| `--resolve=<host:port:ip>` | Custom DNS resolution in `domain:port:ip` format. Same as [curl --resolve](https://everything.curl.dev/usingcurl/connections/name.html?highlight=resolve#provide-a-custom-ip-address-for-a-name).<br>Can be specified multiple times for multiple domain:port pairs.<br>Example: `--resolve='mydomain.tld:443:127.0.0.1` |
| `--allowed-domain-for-external-files=<domain>` | Primarily, the crawler crawls only the URL within the domain for initial URL. This allows<br>you to enable loading of file content from another domain as well (e.g. if you want to<br>load assets from a CDN). Can be specified multiple times. Use can use domains with wildcard `*`. |
| `--allowed-domain-for-crawling=<domain>` | This option will allow you to crawl all content from other listed domains - typically in the case<br>of language mutations on other domains. Can be specified multiple times.<br>Use can use domains with wildcard `*` including e.g. `*.siteone.*`. |
| `--single-foreign-page` | If crawling of other domains is allowed (using `--allowed-domain-for-crawling`),<br>it ensures that when another domain is not on same second-level domain, only that linked page<br>and its assets are crawled from that foreign domain. |
| `--include-regex=<regex>` | Regular expression compatible with PHP preg_match() for URLs that should be included.<br>Argument can be specified multiple times. Example: `--include-regex='/^\/public\//'` |
| `--ignore-regex=<regex>` | Regular expression compatible with PHP preg_match() for URLs that should be ignored.<br>Argument can be specified multiple times.<br>Example: `--ignore-regex='/^.*\/downloads\/.*\.pdf$/i'` |
| `--regex-filtering-only-for-pages` | Set if you want filtering by `*-regex` rules apply only to page URLs, but static assets (JS, CSS, images,<br>fonts, documents) have to be loaded regardless of filtering.<br>Useful where you want to filter only /sub-pages/ by `--include-regex='/\/sub-pages\//'`, but<br>assets have to be loaded from any URLs. |
| `--analyzer-filter-regex` | Regular expression compatible with PHP preg_match() applied to Analyzer class names<br> for analyzers filtering.<br>Example: `/(content\|accessibility)/i` or `/^(?:(?!best\|access).)*$/i` for all<br>analyzers except `BestPracticesAnalyzer` and `AccessibilityAnalyzer`. |
| `--accept-encoding=<val>` | Custom `Accept-Encoding` request header. Default is `gzip, deflate, br`. |
| `--remove-query-params` | Remove query parameters from found URLs. Useful on websites where a lot of links<br>are made to the same pages, only with different irrelevant query parameters. |
| `--add-random-query-params` | Adds several random query parameters to each URL.<br>With this, it is possible to bypass certain forms of server and CDN caches. |
| `--transform-url=<from->to>` | Transform URLs before crawling. Use `from -> to` format for simple replacement or `/regex/ -> replacement` for pattern matching.<br>Useful when archiving sites that reference different domains.<br>Example: `--transform-url="live-site.com -> local-site.local"`.<br>Can be specified multiple times. |
| `--ignore-robots-txt` | Should robots.txt content be ignored? Useful for crawling an otherwise internal/private/unindexed site. |
| `--http-cache-dir=<dir>` | Cache dir for HTTP responses. You can disable cache by `--http-cache-dir='off'`.<br>Default values is `tmp/http-client-cache`. |
| `--http-cache-compression` | Enable compression for HTTP cache storage. Saves disk space, but uses more CPU. |
| `--max-queue-length=<num>` | The maximum length of the waiting URL queue. Increase in case of large websites,<br>but expect higher memory requirements. Default is `9000`. |
| `--max-visited-urls=<num>` | The maximum number of the visited URLs. Increase in case of large websites, but expect<br>higher memory requirements. Default is `10000`. |
| `--max-skipped-urls=<num>` | The maximum number of the skipped URLs. Increase in case of large websites, but expect<br>higher memory requirements. Default is `10000`. |
| `--max-url-length=<num>` | The maximum supported URL length in chars. Increase in case of very long URLs, but expect<br>higher memory requirements. Default is `2083`. |
| `--max-non200-responses-per-basename=<num>` | Protection against looping with dynamic non-200 URLs. If a basename (the last part of the URL<br>after the last slash) has more non-200 responses than this limit, other URLs with same basename<br>will be ignored/skipped. Default is `5`. |

### File export settings

| Parameter | Description |
|-----------|-------------|
| `--output-html-report=<file>` | Save HTML report into that file. Set to empty '' to disable HTML report.<br>By default saved into `tmp/%domain%.report.%datetime%.html`. |
| `--html-report-options=<sections>` | Comma-separated list of sections to include in HTML report.<br>Available sections: `summary`, `seo-opengraph`, `image-gallery`, `video-gallery`, `visited-urls`, `dns-ssl`, `crawler-stats`, `crawler-info`, `headers`, `content-types`, `skipped-urls`, `caching`, `best-practices`, `accessibility`, `security`, `redirects`, `404-pages`, `slowest-urls`, `fastest-urls`, `source-domains`.<br>Default: all sections. |
| `--output-json-file=<file>` | File path for JSON output. Set to empty '' to disable JSON file.<br>By default saved into `tmp/%domain%.output.%datetime%.json`.<br>See [JSON Output Documentation](docs/JSON-OUTPUT.md) for format details. |
| `--output-text-file=<file>` | File path for TXT output. Set to empty '' to disable TXT file.<br>By default saved into `tmp/%domain%.output.%datetime%.txt`.<br>See [Text Output Documentation](docs/TEXT-OUTPUT.md) for format details. |

### Mailer options

| Parameter | Description |
|-----------|-------------|
| `--mail-to=<email>` | Recipients of HTML e-mail reports. Optional but required for mailer activation.<br>You can specify multiple emails separated by comma. |
| `--mail-from=<email>` | E-mail sender address. Default values is `siteone-crawler@your-hostname.com`. |
| `--mail-from-name=<val>` | E-mail sender name. Default values is `SiteOne Crawler`. |
| `--mail-subject-template=<val>` | E-mail subject template. You can use dynamic variables `%domain%`, `%date%` and `%datetime%`.<br>Default values is `Crawler Report for %domain% (%date%)`. |
| `--mail-smtp-host=<host>` | SMTP host for sending emails. Default is `localhost`. |
| `--mail-smtp-port=<port>` | SMTP port for sending emails. Default is `25`. |
| `--mail-smtp-user=<user>` | SMTP user, if your SMTP server requires authentication. |
| `--mail-smtp-pass=<pass>` | SMTP password, if your SMTP server requires authentication. |

### Upload options

| Parameter | Description |
|-----------|-------------|
| `--upload` | Enable HTML report upload to `--upload-to`. |
| `--upload-to=<url>` | URL of the endpoint where to send the HTML report. Default value is `https://crawler.siteone.io/up`. |
| `--upload-retention=<val>` | How long should the HTML report be kept in the online version?<br>Values: 1h / 4h / 12h / 24h / 3d / 7d / 30d / 365d / forever.<br>Default value is `30d`. |
| `--upload-password=<val>` | Optional password, which must be entered (the user will be 'crawler')<br>to display the online HTML report. |
| `--upload-timeout=<int>` | Upload timeout in seconds. Default value is `3600`. |

### Offline exporter options

| Parameter | Description |
|-----------|-------------|
| `--offline-export-dir=<dir>` | Path to directory where to save the offline version of the website. If target directory<br>does not exist, crawler will try to create it (requires sufficient rights). |
| `--offline-export-store-only-url-regex=<regex>` | For debug - when filled it will activate debug mode and store only URLs<br>which match one of these PCRE regexes. Can be specified multiple times. |
| `--offline-export-remove-unwanted-code=<1/0>` | Remove unwanted code for offline mode? Typically, JS of the analytics, social networks,<br>cookie consent, cross origins, etc. Default values is `1`. |
| `--offline-export-no-auto-redirect-html` | Disables the automatic creation of redirect HTML files for each subfolder that contains<br>an `index.html`. This solves situations for URLs where sometimes the URL ends with a slash,<br>sometimes it doesn't. |
| `--replace-content=<val>` | Replace content in HTML/JS/CSS with `foo -> bar` or regexp in PREG format,<br>e.g. `/card[0-9]/i -> card`. Can be specified multiple times. |
| `--replace-query-string=<val>` | Instead of using a short hash instead of a query string in the filename, just replace some characters.<br>You can use simple format `foo -> bar` or regexp in PREG format,<br> e.g. `'/([a-z]+)=([^&]*)(&|$)/i -> $1__$2'`. Can be specified multiple times. |
| `--ignore-store-file-error` | Enable this option to ignore any file storing errors.<br>The export process will continue. |

### Markdown exporter options

| Parameter | Description |
|-----------|-------------|
| `--markdown-export-dir=<dir>` | Path to directory where to save the markdown version of the website.<br>Directory will be created if it doesn't exist. |
| `--markdown-export-single-file=<file>` | Path to a file where to save the combined markdown files into one document. Requires `--markdown-export-dir` to be set. Ideal for AI tools that need to process the entire website content in one go. |
| `--markdown-move-content-before-h1-to-end` | Move all content before the main H1 heading (typically the header with the menu) to the end of the markdown. |
| `--markdown-disable-images` | Do not export and show images in markdown files.<br>Images are enabled by default. |
| `--markdown-disable-files` | Do not export and link files other than HTML/CSS/JS/fonts/images - eg. PDF, ZIP, etc.<br>These files are enabled by default. |
| `--markdown-remove-links-and-images-from-single-file` | Remove links and images from the combined single markdown file. Useful for AI tools that don't need these elements.<br>Requires `--markdown-export-single-file` to be set. |
| `--markdown-exclude-selector=<val>` | Exclude some page content (DOM elements) from markdown export defined by CSS selectors like 'header', '.header', '#header', etc.<br>Can be specified multiple times. |
| `--markdown-replace-content=<val>` | Replace text content with `foo -> bar` or regexp in PREG format: `/card[0-9]/i -> card`.<br>Can be specified multiple times. |
| `--markdown-replace-query-string=<val>` | Instead of using a short hash instead of a query string in the filename, just replace some characters.<br>You can use simple format 'foo -> bar' or regexp in PREG format, e.g.<br>`'/([a-z]+)=([^&]*)(&|$)/i -> $1__$2'`. Can be specified multiple times. |
| `--markdown-export-store-only-url-regex=<regex>` | For debug - when filled it will activate debug mode and store only URLs which match one of these<br>PCRE regexes.<br>Can be specified multiple times. |
| `--markdown-ignore-store-file-error` | Ignores any file storing errors. The export process will continue. |

### Sitemap options

| Parameter | Description |
|-----------|-------------|
| `--sitemap-xml-file=<file>` | File path where generated XML Sitemap will be saved.<br>Extension `.xml` is automatically added if not specified. |
| `--sitemap-txt-file=<file>` | File path where generated TXT Sitemap will be saved.<br>Extension `.txt` is automatically added if not specified. |
| `--sitemap-base-priority=<num>` | Base priority for XML sitemap. Default values is `0.5`. |
| `--sitemap-priority-increase=<num>` | Priority increase value based on slashes count in the URL. Default values is `0.1`. |

### Expert options

| Parameter | Description |
|-----------|-------------|
| `--debug` | Activate debug mode. |
| `--debug-log-file=<file>` | Log file where to save debug messages. When `--debug` is not set and `--debug-log-file`<br> is set, logging will be active without visible output. |
| `--debug-url-regex=<regex>` | Regex for URL(s) to debug. When crawled URL is matched, parsing, URL replacing,<br>and other actions are printed to output. Can be specified multiple times. |
| `--result-storage=<val>` | Result storage type for content and headers. Values: `memory` or `file`.<br>Use `file` for large websites. Default values is `memory`. |
| `--result-storage-dir=<dir>` | Directory for `--result-storage=file`. Default values is `tmp/result-storage`. |
| `--result-storage-compression` | Enable compression for results storage. Saves disk space, but uses more CPU. |
| `--http-cache-dir=<dir>` | Cache dir for HTTP responses. You can disable cache by `--http-cache-dir='off'`.<br>Default values is `tmp/http-client-cache`. |
| `--http-cache-compression` | Enable compression for HTTP cache storage.<br>Saves disk space, but uses more CPU. |
| `--websocket-server=<host:port>` | Start crawler with websocket server on given host:port, e.g. `0.0.0.0:8000`.<br>To connected clients will be sent this message after each URL is crawled:<br>`{"type":"urlResult","url":"...","statusCode":200,"size":4528,"execTime":0.823}`. |
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
| `--slowest-urls-max-time=<val>` | Maximum response time for an URL to be considered very slow.<br>Default is `3`. |

## 📄 Output Examples

To understand the richness of the data provided by the crawler, you can examine real output examples generated from crawling `crawler.siteone.io`:

*   **Text Output Example:** [`docs/OUTPUT-crawler.siteone.io.txt`](docs/OUTPUT-crawler.siteone.io.txt)
    *   Provides a human-readable summary suitable for quick review.
    *   See the detailed [Text Output Documentation](docs/TEXT-OUTPUT.md).
*   **JSON Output Example:** [`docs/OUTPUT-crawler.siteone.io.json`](docs/OUTPUT-crawler.siteone.io.json)
    *   Provides structured data ideal for programmatic consumption and detailed analysis.
    *   See the detailed [JSON Output Documentation](docs/JSON-OUTPUT.md).

These examples showcase the various tables and metrics generated, demonstrating the tool's capabilities in analyzing website structure, performance, SEO, security, and more.

## 🎯 Roadmap

* Well tested Docker images for easy usage in CI/CD pipelines on hub.docker.com (for all platforms).
* Better static assets processing - now are assets processed immediately, same as other URLs. This can cause
  problems with large websites. We will implement a better solution with a separate queue for static assets and separate
  visualization in the output.
* Support for configurable thresholds for response times, status codes, etc. to exit with a non-zero code.
* Support for secure SMTP.

If you have any suggestions or feature requests, please open an issue on GitHub. We'd love to hear from you!

Your contributions with realized improvements, bug fixes, and new features are welcome. Please open a pull request :-)

## 🤔 Motivation to create this tool

If you are interested in the author's motivation for creating this tool, read it on [the project website 🌐](https://crawler.siteone.io/introduction/motivation/).

## ⚠️ Disclaimer

Please use responsibly and ensure that you have the necessary permissions when crawling websites. Some sites may have
rules against automated access detailed in their robots.txt.

**The author is not responsible for any consequences caused by inappropriate use or deliberate misuse of this tool.**

## 📜 License

This work is licensed under a [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT) license.

## Powered by

[![PhpStorm logo.](https://resources.jetbrains.com/storage/products/company/brand/logos/PhpStorm.svg)](https://jb.gg/OpenSourceSupport)
