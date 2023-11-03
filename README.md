# SiteOne Website Crawler

SiteOne Website Crawler is **the best, easy-to-use, most powerful and functional assistant you will love ♥**.

It will crawl your entire website in detail, report problems, generate an offline version of the website, generate
sitemaps or send reports via email.

## Table of contents

- [Features](#features)
    * [Crawler](#crawler)
    * [Dev/DevOps assistant](#devdevops-assistant)
    * [Analyzer](#analyzer)
    * [Reporter](#reporter)
    * [Offline website generator](#offline-website-generator)
    * [Sitemap generator](#sitemap-generator)
    * [For active contributors](#for-active-contributors)
- [Motivation to create this tool](#motivation-to-create-this-tool)
- [Installation](#installation)
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
- [Roadmap](#roadmap)
- [Disclaimer](#disclaimer)
- [License](#license)
- [Output examples](#output-examples)
    * [Text output](#text-output)
    * [JSON output](#json-output)

## Features

In short, the main benefits can be summarized in these points:

- **Crawler** - very powerful crawler of the whole website reporting useful information about each URL (status code,
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
- you can specify by `--allowed-domain-for-external-files` from which **external domains** it is possible to **download
  ** assets (JS, CSS, fonts, images, documents) including `*` option for all domains.
- you can specify by `--allowed-domain-for-crawling` which **other domains** should be included in the **crawling** if
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
-

## Motivation to create this tool

At [SiteOne](https://www.siteone.io/) we have been creating web applications and web presentations for our clients for
more than 20 years. We have implemented hundreds of projects, and we have one need all around.

We need to check that the whole website is working great. Check that all pages respond quickly, that the title and other
SEO criteria are well-designed, that there are no non-existent pages (invalid links or missing files), that the cache or
security headers are set correctly, that we do not have unnecessary redirects. Last but not least, we need to perform
stress tests or test protections against DoS/DDoS attacks on our infrastructure.

There are GUI tools like [Xenu's Link Sleuth](https://home.snafu.de/tilman/xenulink.html)
or [Screaming Frog SEO Spider](https://www.screamingfrog.co.uk/seo-spider/), or some poor quality CLI tools. None of
these tools covered all our needs. That's why we decided to create our own tool.

Ehmmmm... Enough of the marketing bullshit! What was really the most real reason? The author, head of development and
infrastructure at [SiteOne](https://www.siteone.io/), wanted to prove that he could develop a great tool in 16 hours of
pure working time and take a break from caring for his extremely prematurely born son. And he did it! :-) The tool is
great, and his son is doing great too! ♥

## Installation

### Linux (x64)

Most easily installation on most Linux (x64) distributions thanks to precompiled `swoole-cli` binary.

```bash
git clone https://github.com/janreges/siteone-website-crawler.git
cd siteone-website-crawler
chmod +x ./swoole-cli
````

### Windows (x64)

If using Windows, the best choice is to use [Ubuntu](https://ubuntu.com/wsl)
or [Debian](https://www.linuxfordevices.com/tutorials/linux/install-debian-on-windows-wsl)
in [WSL](https://learn.microsoft.com/en-us/windows/wsl/install).

Otherwise, you can
download [swoole-cli-v4.8.13-cygwin-x64.zip](https://github.com/swoole/swoole-src/releases/download/v4.8.13/swoole-cli-v4.8.13-cygwin-x64.zip)
from [Swoole releases](https://github.com/swoole/swoole-src/releases) and use precompiled
Cygwin-based `bin/swoole-cli.exe`.

A really functional and tested Windows command looks like this (modify path to your `swoole-cli.exe` and `crawler.php`):

```bash
c:\Work\swoole-cli-v4.8.13-cygwin-x64\bin\swoole-cli.exe C:\Work\siteone-website-crawler\crawler.php --url=https://www.siteone.io/
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

To run the crawler, execute the `crawler.php` file from the command line with precompiled `swoole-cli` and provide the
required arguments:

### Basic example

```bash
./swoole-cli crawler.php --url=https://mydomain.tld/ --device=mobile
```

### Fully-featured example

```bash
./swoole-cli crawler.php --url=https://mydomain.tld/ \
  --output=text \
  --workers=2 \
  --memory-limit=512M \
  --timeout=5 \
  --user-agent="My User-Agent String" \
  --extra-columns="DOM,X-Cache(10),Title(40!),Keywords(50!),Description(50!)" \
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
  --hide-scheme-and-host \
  --do-not-truncate-url \
  --output-html-file=/dir/report.html \
  --output-json-file=/dir/report.json \
  --output-text-file=/dir/report.txt \
  --add-timestamp-to-output-file \
  --add-host-to-output-file \
  --sitemap-xml-file==/dir/sitemap.xml \
  --sitemap-txt-file==/dir/sitemap.txt \
  --sitemap-base-priority=0.5 \
  --sitemap-priority-increase=0.1 \
  --mail-to=your.name@my-mail.tld \
  --mail-to=your.friend.name@my-mail.tld \
  --mail-from=crawler@ymy-mail.tld \
  --mail-from-name="SiteOne Crawler" \
  --mail-subject-template="Crawler report for %domain% (%datetime%)" \
  --mail-smtp-host=smtp.my-mail.tld \
  --mail-smtp-port=25 \
  --mail-smtp-user=smtp.user \
  --mail-smtp-pass=secretPassword123
```

### Arguments

#### Basic settings

* `--url=<url>`                    Required. HTTP or HTTPS URL address of the website to be crawled.Use quotation marks
  if the URL contains query parameters
* `--device=<val>`                 Device type for choosing a predefined User-Agent. Ignored when `--user-agent` is
  defined. Supported values: `desktop`, `mobile`, `tablet`. Defaults is `desktop`.
* `--user-agent=<val>`             Custom User-Agent header. Use quotation marks. If specified, it takes precedence over
  the device parameter.
* `--timeout=<int>`                Request timeout in seconds. Default is `3`.

#### Output settings

* `--output=<val>`                 Output type. Supported values: `text`, `json`. Default is `text`.
* `--extra-columns=<values>`       Comma delimited list of extra columns added to output table. It is possible to
  specify HTTP header names (e.g. `X-Cache`) or predefined `Title`, `Keywords`, `Description` or `DOM` for the number of DOM
  elements found in the HTML. You can set the expected length of the column in parentheses and `!` for truncate for better look - e.g.
  `DOM(6),X-Cache(10),Title(40!),Description(50!)`.
* `--url-column-size=<num>`        Basic URL column width. Default is `80`.
* `--do-not-truncate-url`          In the text output, long URLs are truncated by default to `--url-column-size` so the
  table does not wrap due to long URLs. With this option, you can turn off the truncation.
* `--hide-scheme-and-host`         On text output, hide scheme and host of URLs for more compact view.
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
* `--analyzer-filter-regex`        Regular expression compatible with PHP preg_match() applied to Analyzer class names 
 for analyzers filtering. Example: `/(content|accessibility)/i` or `/^(?:(?!best|access).)*$/i` for all analyzers except
 `BestPracticesAnalyzer` and `AccessibilityAnalyzer`.
* `--accept-encoding=<val>`        Custom `Accept-Encoding` request header. Default is `gzip, deflate, br`.
* `--remove-query-params`          Remove query parameters from found URLs. Useful on websites where a lot of links are
  made to the same pages, only with different irrelevant query parameters.
* `--add-random-query-params`      Adds several random query parameters to each URL. With this, it is possible to bypass
  certain forms of server and CDN caches.
* `--http-cache-dir=<dir>`         Cache dir for HTTP responses. You can disable cache by `--http-cache-dir=''`. Default 
  is `tmp/http-client-cache`.
* `--http-cache-compression`       Enable compression for HTTP cache storage. Saves disk space, but uses more CPU.
* `--max-queue-length=<num>`       The maximum length of the waiting URL queue. Increase in case of large websites, but
  expect higher memory requirements. Default is `9000`.
* `--max-visited-urls=<num>`       The maximum number of the visited URLs. Increase in case of large websites, but
  expect higher memory requirements. Default is `10000`.
* `--max-url-length=<num>`         The maximum supported URL length in chars. Increase in case of very long URLs, but
  expect higher memory requirements. Default is `2083`.

#### File export settings

* `--output-html-file=<file>`      File path for HTML output. Extension `.html` is automatically added if not specified.
* `--output-json-file=<file>`      File path for JSON output. Extension `.json` is automatically added if not
  specified.
* `--output-text-file=<file>`      File path for text output. Extension `.txt` is automatically added if not specified.
* `--add-host-to-output-file`      Add host from initial URL as suffix to output file name. Example: you
  set `--output-json-file=/dir/report` and target filename will be `/dir/report.www.mydomain.tld.json`.
* `--add-timestamp-to-output-file` Add timestamp as suffix to output file name. Example: you
  set `--output-html-file=/dir/report` and target filename will be `/dir/report.2023-10-06.14-33-12.html`.

#### Mailer options

* `--mail-to=<email>`              Recipients of HTML e-mail reports. Optional but required for mailer activation. You
  can specify multiple emails separated by comma.
* `--mail-from=<email>`            E-mail sender address. Default values is `siteone-website-crawler@your-hostname.com`.
* `--mail-from-name=<val>`         E-mail sender name. Default values is `SiteOne Crawler`.
* `--mail-subject-template=<val>`  E-mail subject template. You can use dynamic variables %domain%, %date% and
  %datetime%. Default values is `Crawler report for %domain% (%datetime%)`.
* `--mail-smtp-host=<host>`        SMTP host for sending emails. Default is `localhost`.
* `--mail-smtp-port=<port>`        SMTP port for sending emails. Default is `25`.
* `--mail-smtp-user=<user>`        SMTP user, if your SMTP server requires authentication.
* `--mail-smtp-pass=<pass>`        SMTP password, if your SMTP server requires authentication.

#### Sitemap options

* `--sitemap-xml-file=<file>`      File path where generated XML Sitemap will be saved. Extension `.xml` is
  automatically added if not specified.
* `--sitemap-txt-file=<file>`      File path where generated TXT Sitemap will be saved. Extension `.txt` is
  automatically added if not specified.
* `--sitemap-base-priority=<num>`  Base priority for XML sitemap. Default values is `0.5`.
* `--sitemap-priority-increase=<num>`  Priority increase value based on slashes count in the URL. Default values
  is `0.1`.

#### Expert options

`--debug`                          Activate debug mode.
`--debug-log-file=<file>`          Log file where to save debug messages. When `--debug` is not set and `--debug-log-file` is set, logging will be active without visible output.
`--debug-url-regex=<regex>`        Regex for URL(s) to debug. When crawled URL is matched, parsing, URL replacing, and other actions are printed to output. Can be specified multiple times.
`--result-storage=<val>`           Result storage type for content and headers. Values: `memory` or `file`. Use `file` for large websites. Default values is `memory`.
`--result-storage-dir=<dir>`       Directory for `--result-storage=file`. Default values is `tmp/result-storage`.
`--result-storage-compression`     Enable compression for results storage. Saves disk space, but uses more CPU.
`--http-cache-dir=<dir>`           Cache dir for HTTP responses. You can disable cache by `--http-cache-dir=''`. Default values is `tmp/http-client-cache`.
`--http-cache-compression`         Enable compression for HTTP cache storage. Saves disk space, but uses more CPU.

**NOTICE**: For now, only SMTP without encryption is supported, typically running on port 25. If you are interested in
this tool, we can also implement secure SMTP support, or simply send me a pull request with lightweight implementation.

## Roadmap

* Well tested Docker images for easy usage in CI/CD pipelines on hub.docker.com (for all platforms).
* Better static assets processing - now are assets processed immediately, same as other URLs. This can cause
  problems with large websites. We will implement a better solution with a separate queue for static assets and separate
  visualization in the output.
* Support for configurable thresholds for response times, status codes, etc. to exit with a non-zero code.
* Support for secure SMTP.
* Support for HTTP authentication.

If you have any suggestions or feature requests, please open an issue on GitHub. We'd love to hear from you!

Your contributions with realized improvements, bug fixes, and new features are welcome. Please open a pull request :-)

## Disclaimer

Please use responsibly and ensure that you have the necessary permissions when crawling websites. Some sites may have
rules against automated access detailed in their robots.txt.

**The author is not responsible for any consequences caused by inappropriate use or deliberate misuse of this tool.**

## License

Shield: [![CC BY 4.0][cc-by-shield]][cc-by]

This work is licensed under a
[Creative Commons Attribution 4.0 International License][cc-by].

[![CC BY 4.0][cc-by-image]][cc-by]

[cc-by]: http://creativecommons.org/licenses/by/4.0/

[cc-by-image]: https://i.creativecommons.org/l/by/4.0/88x31.png

[cc-by-shield]: https://img.shields.io/badge/License-CC%20BY%204.0-lightgrey.svg

## Output examples

### Text output

![SiteOne Website Crawler](./docs/siteone-website-crawler.gif)

### JSON output

Output is truncated (only 3 URLs in results) for better readability.

```json
{
  "crawler": {
    "name": "SiteOne Website Crawler",
    "version": "2023.10.2",
    "executedAt": "2023-10-05 16:50:27",
    "command": "crawler.php --url=https:\/\/www.siteone.io\/ --extra-columns=Title --workers=2 --do-not-truncate-url --url-column-size=72 --output=json"
  },
  "options": {
    "url": "https:\/\/www.siteone.io\/",
    "device": "desktop",
    "outputType": "json",
    "workers": 2,
    "timeout": 10,
    "urlColumnSize": 72,
    "acceptEncoding": "gzip, deflate, br",
    "userAgent": null,
    "extraColumns": [
      "Title"
    ],
    "maxQueueLength": 1000,
    "maxVisitedUrls": 5000,
    "maxUrlLength": 2000,
    "crawlAssets": [],
    "addRandomQueryParams": false,
    "removeQueryParams": false,
    "hideSchemeAndHost": false,
    "doNotTruncateUrl": true
  },
  "results": [
    {
      "url": "https:\/\/www.siteone.io\/",
      "status": 200,
      "elapsedTime": 0.086,
      "size": 159815,
      "extras": {
        "Title": "SiteOne | Design. Development. Digital Transformation."
      }
    },
    {
      "url": "https:\/\/www.siteone.io\/our-projects",
      "status": 200,
      "elapsedTime": 0.099,
      "size": 132439,
      "extras": {
        "Title": "SiteOne | Our Projects &amp; Successful Solutions"
      }
    },
    {
      "url": "https:\/\/www.siteone.io\/case-study\/new-webdesign-for-e.on-energy",
      "status": 200,
      "elapsedTime": 0.099,
      "size": 156471,
      "extras": {
        "Title": "SiteOne | E.ON: Security and Stability in the Energy Market"
      }
    }
  ],
  "stats": {
    "totalExecutionTime": 0.464,
    "totalUrls": 9,
    "totalSize": 1358863,
    "totalSizeFormatted": "1.3 MB",
    "totalRequestsTimes": 0.826,
    "totalRequestsTimesAvg": 0.092,
    "totalRequestsTimesMin": 0.074,
    "totalRequestsTimesMax": 0.099,
    "countByStatus": {
      "200": 9
    }
  }
}
```