# SiteOne Crawler

SiteOne Crawler is a **very useful and easy-to-use tool you'll ♥ as a Dev/DevOps, website owner or consultant**. Works on all popular platforms - **Windows**, **macOS** and **Linux** (**x64** and **arm64** too).

It will crawl your entire website in depth, analyze and report problems, show useful statistics and reports, generate an offline
version of the website, generate sitemaps or send reports via email. Watch a detailed [video with a sample report](https://www.youtube.com/watch?v=PHIFSOmk0gk) for the [astro.build](https://astro.build/?utm_source=siteone-crawler-github) website.

This crawler can be used as a command-line tool (see [releases](https://github.com/janreges/siteone-crawler/releases) and [video](https://www.youtube.com/watch?v=25T_yx13naA&list=PL9mElgTe-s1Csfg0jXWmDS0MHFN7Cpjwp)), or you can use a [multi-platform desktop application](https://github.com/janreges/siteone-crawler-gui) with graphical interface (see [video](https://www.youtube.com/watch?v=rFW8LNEVNdw) about app).

I also recommend looking at the project website [crawler.siteone.io](https://crawler.siteone.io/).

GIF animation of the crawler in action (also available as a [video](https://www.youtube.com/watch?v=25T_yx13naA&list=PL9mElgTe-s1Csfg0jXWmDS0MHFN7Cpjwp)):

![SiteOne Crawler](docs/siteone-crawler-command-line.gif)

## Table of contents

- [Features](#features)
    * [Crawler](#crawler)
    * [Dev/DevOps assistant](#devdevops-assistant)
    * [Analyzer](#analyzer)
    * [Reporter](#reporter)
    * [Offline website generator](#offline-website-generator)
    * [Sitemap generator](#sitemap-generator)
    * [For active contributors](#for-active-contributors)
- [Installation](#installation)
    * [Ready-to-use releases](#ready-to-use-releases)
    * [Linux (x64)](#linux-x64)
    * [Windows (x64)](#windows-x64)
    * [macOS (arm64, x64)](#macos-arm64-x64)
    * [Linux (arm64)](#linux-arm64)
- [Usage](#usage)
    * [Basic example](#basic-example)
    * [Fully-featured example](#fully-featured-example)
    * [Arguments](#arguments)
        + [Basic settings](#basic-settings)
        + [Output settings](#output-settings)
        + [Advanced crawler settings](#advanced-crawler-settings)
        + [Export settings](#export-settings)
        + [Mailer options](#mailer-options)
        + [Upload options](#upload-options)
- [Roadmap](#roadmap)
- [Motivation to create this tool](#motivation-to-create-this-tool)
- [Disclaimer](#disclaimer)
- [License](#license)
- [Output examples](#output-examples)
    * [Text output](#text-output)
    * [JSON output](#json-output)

## Features

In short, the main benefits can be summarized in these points:

- **Crawler** - very powerful crawler of the entire website reporting useful information about each URL (status code,
  response time, size, custom headers, titles, etc.)
- **Dev/DevOps assistant** - offers a set of very useful and often necessary features for developers and devops (stress
  test, warm up cache, localhost testing, etc.)
- **Analyzer** - analyzes all webpages and reports strange or error behaviour and useful statistics (404, redirects, bad
  practices, etc.)
- **Reporter** - sends a HTML report to your email addresses with all the information about the crawled website
- **Offline website generator** - allows you to export the entire website to offline form, where it is possible to
  browse the site through local HTML files (without HTTP server) including all images, styles, scripts, fonts, etc.
- **Sitemap generator** - allows you to generate `sitemap.xml` and `sitemap.txt` files with a list of all pages on your
  website

The following features are summarized in greater detail:

### Crawler

- **all major platforms** supported without complicated installation or dependencies (Linux, Windows, macOS, arm64)
- has incredible **C++ performance** (thanks to Swoole's coroutines)
- provide simulation of **different device types** (desktop/mobile/tablet) thanks to predefined User-Agents
- will crawl **all files**, styles, scripts, fonts, images, documents, etc. on your website
- will respect the `robots.txt` file and will not crawl the pages that are not allowed
- has a **beautiful interactive** and **colourful output**
- it will **clearly warn you** of any wrong use of the tool (e.g. input parameters validation or wrong permissions)
- **captures CTRL+C** and ends with the statistics for at least the current processed URLs

### Dev/DevOps assistant

- allows testing **public** and **local projects on specific ports** (e.g. `http://localhost:3000/`)
- will perform a **stress test** and allow you to test the protection of the infrastructure against DoS attacks
- will help you **warm up the application cache** or the **cache on the reverse proxy** of the entire website

### Analyzer

- will **find the weak points** or **strange behavior** of your website
- allows you to implement **your own analyzers** by simply adding an analyzer class that implements
  the `Crawler\Analyzer` interface.

### Reporter

- will provide you with data for **SEO analysis**, just add the `Title`, `Keywords` and `Description`
- will send you a **nice HTML report** to your email addresses
- will **export** the output to JSON, HTML or text for **your integrations**
  will provide useful **summaries and statistics** at the end of the processing

### Offline website generator

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
- you can use it to **export your website to a static form** and host it on GitHub Pages, Netlify, Vercel, etc. as a
  static backup and part of your **disaster recovery plan** or **archival/legal needs**
- works great with **older conventional websites** but also **modern ones**, built on frameworks like Next.js, Nuxt.js,
  SvelteKit, Astro, Gatsby, etc. When a JS framework is detected, the export also performs some framework-specific code
  modifications for optimal results. For example, most frameworks can't handle the relative location of a project and
  linking assets from root `/`, which doesn't work with `file://` mode.
- **try it** for your website, and you will be very pleasantly surprised :-)
- roadmap: we are also planning to release a version of the export compatible with **Nginx** that will preserve all
  original URLs for your website and allow you to host it on your own infrastructure.

### Sitemap generator

- will help you create a `sitemap.xml` and `sitemap.xml` for your website
- you can set the priority of individual pages based on the number of slashes in the URL

Don't hesitate and try it. You will love it as we do! ♥

### For active contributors

- the crawler code provides some useful functionality that facilitates further **development** and **extensibility** of
  the project

## Installation

###  Ready-to-use releases

You can download ready-to-use releases from [GitHub releases](https://github.com/janreges/siteone-crawler/releases) for all major platforms (Linux, Windows, macOS, arm64).

Unpack the downloaded archive, and you will find the `crawler` or `crawler.bat` (Windows) executable binary and run crawler by `./crawler --url=https://my.domain.tld`.

**Note for Windows users**: use Cygwin-based release `*-win-x64.zip` only if you can't use WSL (Ubuntu/Debian), what is recommended. If you really have to use the Cygwin version, set `--workers=1` for higher stability.

**Note for macOS users**: In case that Mac refuses to start the crawler from your Download folder, move the entire folder with the Crawler **via the terminal** to another location, for example to the homefolder `~`.

### Linux (x64)

Most easily installation is on most Linux (x64) distributions.

```bash
git clone https://github.com/janreges/siteone-crawler.git
cd siteone-crawler

# run crawler with basic options
./crawler --url=https://my.domain.tld
````

### Windows (x64)

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

### macOS (arm64, x64)

If using macOS with latest arm64 M1/M2 CPU, download arm64
version [swoole-cli-v4.8.13-macos-arm64.tar.xz](https://github.com/swoole/swoole-src/releases/download/v4.8.13/swoole-cli-v4.8.13-macos-arm64.tar.xz),
unpack and use its precompiled `swoole-cli`.

If using macOS with Intel CPU (x64), download x64
version  [swoole-cli-v4.8.13-macos-x64.tar.xz](https://github.com/swoole/swoole-src/releases/download/v4.8.13/swoole-cli-v4.8.13-macos-x64.tar.xz),
unpack and use its precompiled `swoole-cli`.

### Linux (arm64)

If using arm64 Linux, you can
download [swoole-cli-v4.8.13-linux-arm64.tar.xz](https://github.com/swoole/swoole-src/releases/download/v4.8.13/swoole-cli-v4.8.13-linux-arm64.tar.xz)
and use its precompiled `swoole-cli`.

## Usage

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
  --memory-limit=1024M \
  --timeout=5 \
  --proxy=proxy.mydomain.tld:8080 \
  --http-auth=myuser:secretPassword123 \
  --user-agent="My User-Agent String" \
  --extra-columns="DOM,X-Cache(10),Title(40),Keywords(50),Description(50>)" \
  --accept-encoding="gzip, deflate" \
  --url-column-size=100 \
  --max-queue-length=3000 \
  --max-visited-urls=10000 \
  --max-url-length=5000 \
  --include-regex="/^.*\/technologies.*/" \
  --include-regex="/^.*\/fashion.*/" \
  --ignore-regex="/^.*\/downloads\/.*\.pdf$/i" \
  --analyzer-filter-regex="/^.*$/i" \
  --remove-query-params \
  --add-random-query-params \
  --show-scheme-and-host \
  --do-not-truncate-url \
  --output-html-report=tmp/myreport.html \
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

### Arguments

For a clearer list, I recommend going to the documentation: https://crawler.siteone.io/configuration/command-line-options/

#### Basic settings

* `--url=<url>`                    Required. HTTP or HTTPS URL address of the website to be crawled.Use quotation marks
  if the URL contains query parameters
* `--device=<val>`                 Device type for choosing a predefined User-Agent. Ignored when `--user-agent` is
  defined. Supported values: `desktop`, `mobile`, `tablet`. Defaults is `desktop`.
* `--user-agent=<val>`             Custom User-Agent header. Use quotation marks. If specified, it takes precedence over
  the device parameter.
* `--timeout=<int>`                Request timeout in seconds. Default is `3`.
* `--proxy=<host:port>`            HTTP proxy to use in `host:port` format. Host can be hostname, IPv4 or IPv6.
* `--http-auth=<user:pass>`        Basic HTTP authentication in `username:password` format.

#### Output settings

* `--output=<val>`                 Output type. Supported values: `text`, `json`. Default is `text`.
* `--extra-columns=<values>`       Comma delimited list of extra columns added to output table. It is possible to
  specify HTTP header names (e.g. `X-Cache`) or predefined `Title`, `Keywords`, `Description` or `DOM` for the number of DOM
  elements found in the HTML. You can set the expected length of the column in parentheses and `>` for do-not-truncate - e.g.
  `DOM(6),X-Cache(10),Title(40>),Description(50>)`.
* `--url-column-size=<num>`        Basic URL column width. By default, it is calculated from the size of your terminal window.
* `--rows-limit=<num>`             Max. number of rows to display in tables with analysis results (protection against very 
  long and slow report). Default is `200`.
* `--do-not-truncate-url`          In the text output, long URLs are truncated by default to `--url-column-size` so the
  table does not wrap due to long URLs. With this option, you can turn off the truncation.
* `--show-scheme-and-host`         On text output, show scheme and host also for origin domain URLs.
* `--hide-progress-bar`            Hide progress bar visible in text and JSON output for more compact view.
* `--no-color`                     Disable colored output.

Resource filtering:
-------------------

In the default setting, the crawler crawls and downloads all the content it comes across - HTML pages, images,
documents,
javascripts, stylesheets, fonts, just absolutely everything it sees. These options allow you to disable (and remove
from the HTML) individual types of assets and all related content.

For example, it is very useful to disable JavaScript on modern websites, e.g. on React with NextJS, which have SSR,
so they work fine without JavaScript from the point of view of content browsing and navigation.

It is particularly useful to disable JavaScript in the case of exporting websites built e.g. on React to offline form
(without HTTP server), where it is almost impossible to get the website to work from any location on the disk only
through the file:// protocol.

* `--disable-javascript`           Disables JavaScript downloading and removes all JavaScript code from HTML,
  including `onclick` and other `on*` handlers.
* `--disable-styles`               Disables CSS file downloading and at the same time removes all style definitions
  by `<style>` tag or inline by style attributes.
* `--disable-fonts`                Disables font downloading and also removes all font/font-face definitions from CSS.
* `--disable-images`               Disables downloading of all images and replaces found images in HTML with placeholder
  image only.
* `--disable-files `               Disables downloading of any files (typically downloadable documents) to which various
  links point.
* `--remove-all-anchor-listeners`  On all links on the page remove any event listeners. Useful on some types of sites
  with modern JS frameworks that would like to compose content dynamically (React, Svelte, Vue, Angular, etc.).

#### Advanced crawler settings

* `--workers=<int>`                Maximum number of concurrent workers (threads). Crawler will not make more 
  simultaneous requests to the server than this number. Use carefully! A high number of workers can cause a DoS attack.
  Default is `3`.
* `--memory-limit=<size>`          Memory limit in units `M` (Megabytes) or `G` (Gigabytes). Default is `512M`.
* `--include-regex=<regex>`        Regular expression compatible with PHP preg_match() for URLs that should be included.
  Argument can be specified multiple times. Example: `--include-regex='/^\/public\//'`
* `--ignore-regex=<regex>`         Regular expression compatible with PHP preg_match() for URLs that should be ignored.
  Argument can be specified multiple times. Example: `--ignore-regex='/^.*\/downloads\/.*\.pdf$/i'`
* `--regex-filtering-only-for-pages` Set if you want filtering by `*-regex` rules apply only to page URLs,
  but static assets (JS, CSS, images, fonts, documents) have to be loaded regardless of filtering. Useful where you want
  to filter only /sub-pages/ by `--include-regex='/\/sub-pages\//'`, but assets have to be loaded from any URLs.
* `--analyzer-filter-regex`        Regular expression compatible with PHP preg_match() applied to Analyzer class names 
 for analyzers filtering. Example: `/(content|accessibility)/i` or `/^(?:(?!best|access).)*$/i` for all analyzers except
 `BestPracticesAnalyzer` and `AccessibilityAnalyzer`.
* `--accept-encoding=<val>`        Custom `Accept-Encoding` request header. Default is `gzip, deflate, br`.
* `--remove-query-params`          Remove query parameters from found URLs. Useful on websites where a lot of links are
  made to the same pages, only with different irrelevant query parameters.
* `--add-random-query-params`      Adds several random query parameters to each URL. With this, it is possible to bypass
  certain forms of server and CDN caches.
* `--ignore-robots-txt`            Should robots.txt content be ignored? Useful for crawling an otherwise
  internal/private/unindexed site.
  
* `--http-cache-dir=<dir>`         Cache dir for HTTP responses. You can disable cache by `--http-cache-dir='off'`. Default 
  is `tmp/http-client-cache`.
* `--http-cache-compression`       Enable compression for HTTP cache storage. Saves disk space, but uses more CPU.
* `--max-queue-length=<num>`       The maximum length of the waiting URL queue. Increase in case of large websites, but
  expect higher memory requirements. Default is `9000`.
* `--max-visited-urls=<num>`       The maximum number of the visited URLs. Increase in case of large websites, but
  expect higher memory requirements. Default is `10000`.
* `--max-url-length=<num>`         The maximum supported URL length in chars. Increase in case of very long URLs, but
  expect higher memory requirements. Default is `2083`.

#### File export settings

* `--output-html-report=<file>`    Save HTML report into that file. Set to empty '' to disable HTML report. By default
  saved into `tmp/%domain%.report.%datetime%.html`.
* `--output-json-file=<file>`      File path for JSON output. Set to empty '' to disable JSON file. By default saved
 into `tmp/%domain%.output.%datetime%.json`.
* `--output-text-file=<file>`      File path for TXT output. Set to empty '' to disable TXT file. By default saved
  into `tmp/%domain%.output.%datetime%.txt`.

#### Mailer options

* `--mail-to=<email>`              Recipients of HTML e-mail reports. Optional but required for mailer activation. You
  can specify multiple emails separated by comma.
* `--mail-from=<email>`            E-mail sender address. Default values is `siteone-crawler@your-hostname.com`.
* `--mail-from-name=<val>`         E-mail sender name. Default values is `SiteOne Crawler`.
* `--mail-subject-template=<val>`  E-mail subject template. You can use dynamic variables %domain%, %date% and
  %datetime%. Default values is `Crawler Report for %domain% (%date%)`.
* `--mail-smtp-host=<host>`        SMTP host for sending emails. Default is `localhost`.
* `--mail-smtp-port=<port>`        SMTP port for sending emails. Default is `25`.
* `--mail-smtp-user=<user>`        SMTP user, if your SMTP server requires authentication.
* `--mail-smtp-pass=<pass>`        SMTP password, if your SMTP server requires authentication.

**NOTICE**: For now, only SMTP without encryption is supported, typically running on port 25. If you are interested in
this tool, we can also implement secure SMTP support, or simply send me a pull request with lightweight implementation.

#### Upload options

* `--upload`                         Enable HTML report upload to `--upload-to`.
* `--upload-to=<url>`                URL of the endpoint where to send the HTML report. Default value is `https://crawler.siteone.io/up`.
* `--upload-retention=<val>`         How long should the HTML report be kept in the online version? Values: 1h / 4h / 12h / 24h / 3d / 7d / 30d / 365d / forever. Default value is `30d`.
* `--upload-password=<val>`          Optional password, which must be entered (the user will be 'crawler') to display the online HTML report.
* `--upload-timeout=<int>`           Upload timeout in seconds. Default value is `3600`.

If necessary, you can also use your own endpoint `--upload-to` for saving the HTML report.

**How to implement own endpoint**: Your own endpoint need to accept a POST request, where in `htmlBody` is the gzipped HTML body of the report, `retention` is the retention value, and `password` is an optional password to encrypt access to the HTML. The response must be JSON with `url` key with the URL where the report is available.

#### Offline exporter options

* `--offline-export-dir=<dir>`     Path to directory where to save the offline version of the website. If target directory does not exist, crawler will try to create it (requires sufficient rights).
* `--offline-export-store-only-url-regex=<regex>` For debug - when filled it will activate debug mode and store only URLs which match one of these PCRE regexes. Can be specified multiple times.
* `--replace-content=<val>`        Replace content in HTML/JS/CSS with `foo -> bar` or regexp in PREG format: `/card[0-9]/i -> card`. Can be specified multiple times.
* `--ignore-store-file-error`      Enable this option to ignore any file storing errors. The export process will continue.

#### Sitemap options

* `--sitemap-xml-file=<file>`      File path where generated XML Sitemap will be saved. Extension `.xml` is
  automatically added if not specified.
* `--sitemap-txt-file=<file>`      File path where generated TXT Sitemap will be saved. Extension `.txt` is
  automatically added if not specified.
* `--sitemap-base-priority=<num>`  Base priority for XML sitemap. Default values is `0.5`.
* `--sitemap-priority-increase=<num>`  Priority increase value based on slashes count in the URL. Default values
  is `0.1`.

#### Expert options

* `--debug`                          Activate debug mode.
* `--debug-log-file=<file>`          Log file where to save debug messages. When `--debug` is not set and `--debug-log-file` is set, logging will be active without visible output.
* `--debug-url-regex=<regex>`        Regex for URL(s) to debug. When crawled URL is matched, parsing, URL replacing, and other actions are printed to output. Can be specified multiple times.
* `--result-storage=<val>`           Result storage type for content and headers. Values: `memory` or `file`. Use `file` for large websites. Default values is `memory`.
* `--result-storage-dir=<dir>`       Directory for `--result-storage=file`. Default values is `tmp/result-storage`.
* `--result-storage-compression`     Enable compression for results storage. Saves disk space, but uses more CPU.
* `--http-cache-dir=<dir>`           Cache dir for HTTP responses. You can disable cache by `--http-cache-dir='off'`. Default values is `tmp/http-client-cache`.
* `--http-cache-compression`         Enable compression for HTTP cache storage. Saves disk space, but uses more CPU.

## Roadmap

* Well tested Docker images for easy usage in CI/CD pipelines on hub.docker.com (for all platforms).
* Better static assets processing - now are assets processed immediately, same as other URLs. This can cause
  problems with large websites. We will implement a better solution with a separate queue for static assets and separate
  visualization in the output.
* Support for configurable thresholds for response times, status codes, etc. to exit with a non-zero code.
* Support for secure SMTP.

If you have any suggestions or feature requests, please open an issue on GitHub. We'd love to hear from you!

Your contributions with realized improvements, bug fixes, and new features are welcome. Please open a pull request :-)

## Motivation to create this tool

If you are interested in the author's motivation for creating this tool, read it on [the project website](https://crawler.siteone.io/introduction/motivation/).

## Disclaimer

Please use responsibly and ensure that you have the necessary permissions when crawling websites. Some sites may have
rules against automated access detailed in their robots.txt.

**The author is not responsible for any consequences caused by inappropriate use or deliberate misuse of this tool.**

## License

This work is licensed under a [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
