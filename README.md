# SiteOne Website Crawler

SiteOne Website Crawler is **the best, easy-to-use, most powerful and most functional website crawler you will love ♥**.

## Table of contents

- [Features](#features)
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

- **all major platforms** supported without complicated installation (Linux, Windows, macOS, arm64)
- has incredible **C++ performance** (thanks to Swoole's coroutines)
- has **beautiful**, formatted, **interactive** and colored **console output**
- will **find the weak points** or **strange behavior** of your website for you
- provide simulation of **different device types** (desktop/mobile/tablet) thanks to predefined User-Agents
- will send you a **nice HTML report** to your email addresses
- will **export** the output to JSON, HTML or text for **your integrations**
- allows testing **public** and **local projects on specific ports**
- it will help you **warm up the application cache** or the **cache on the reverse proxy** of the entire website
- will provide you with data for **SEO analysis**
- it will perform a **stress test** and allow you to test the protection of the infrastructure
- it also crawls **all files**, styles, scripts, fonts, images, documents, etc.
- will help you create a **sitemap** for your website
- it will **clearly warn you** of any wrong use

Don't hesitate and try it. You will love it as we do! ♥

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
  --max-workers=2 \
  --timeout=5 \
  --user-agent="My User-Agent String" \
  --headers-to-table="DOM,X-Cache(10),Title,Keywords,Description" \
  --accept-encoding="gzip, deflate" \
  --url-column-size=100 \
  --max-queue-length=3000 \
  --max-visited-urls=10000 \
  --max-url-length=5000 \
  --crawl-assets="fonts,images,styles,scripts,files" \
  --include-regex="/^.*\/technologies.*/" \
  --include-regex="/^.*\/fashion.*/" \
  --ignore-regex="/^.*\/downloads\/.*\.pdf$/i" \
  --remove-query-params \
  --add-random-query-params \
  --hide-scheme-and-host \
  --do-not-truncate-url \
  --output-html-file=/dir/report.html \
  --output-json-file=/dir/report.json \
  --output-text-file=/dir/report.txt \
  --output-sitemap-xml=/dir/sitemap.xml \
  --output-sitemap-txt=/dir/sitemap.txt \
  --add-timestamp-to-output-file \
  --add-host-to-output-file \
  --mail-to=your.name@ymy-mail.tld \
  --mail-from=crawler@ymy-mail.tld \
  --mail-smtp-host=smtp.my-mail.tld \
  --mail-smtp-port=25 \
  --mail-smtp-user=smtp.user \
  --mail-smtp-pass=secretPassword123
```

### Arguments

#### Basic settings

* `--url=<url>`                    Required. HTTP or HTTPS URL address of the website to be crawled.Use quotation marks
  if the URL contains query parameters
* `--device=<device`               Device type for choosing a predefined User-Agent. Ignored when `--user-agent` is
  defined. Supported values: `desktop`, `mobile`, `tablet`. Defaults is `desktop`.
* `--user-agent=<value>`           Custom User-Agent header. Use quotation marks. If specified, it takes precedence over
  the device parameter.
* `--timeout=<num>`                Request timeout in seconds. Default is `3`.

#### Output settings

* `--output=<value>`               Output type. Supported values: `text`, `json`. Default is `text`.
* `--headers-to-table=<values>`    Comma delimited list of HTTP response headers added to output table. A special case
  is the possibility to use `Title`, `Keywords` and `Description`. Also, you can set `DOM` to display number of DOM
  elements in HTML. You can set the expected length of the column in parentheses for better look - for example `X-Cache(10)`
* `--url-column-size=<num>`        Basic URL column width. Default is `80`.
* `--do-not-truncate-url`          In the text output, long URLs are truncated by default to `--url-column-size` so the
  table does not wrap due to long URLs. With this option, you can turn off the truncation.
* `--hide-scheme-and-host`         On text output, hide scheme and host of URLs for more compact view.
* `--hide-progress-bar`            Hide progress bar visible in text and JSON output for more compact view.
* `--no-color`                     Disable colored output.

#### Advanced crawler settings

* `--max-workers=<num>`            Maximum number of concurrent workers (threads). Use carefully. A high number of
  threads can cause a DoS attack. Default is `3`.
* `--crawl-assets=<values>`        Comma delimited list of frontend assets you want to crawl too. Otherwise, URLs with
  an extension are ignored. Supported values: `fonts`, `images`, `styles`, `scripts`, `files`.
* `--include-regex=<regex>`        Regular expression compatible with PHP preg_match() for URLs that should be included.
  Argument can be specified multiple times. Example: `--include-regex='/^\/public\//'`
* `--ignore-regex=<regex>`         Regular expression compatible with PHP preg_match() for URLs that should be ignored.
  Argument can be specified multiple times. Example: `--ignore-regex='/^.*\/downloads\/.*\.pdf$/i'`
* `--accept-encoding=<value>`      Custom `Accept-Encoding` request header. Default is `gzip, deflate, br`.
* `--remove-query-params`          Remove query parameters from found URLs. Useful on websites where a lot of links are
  made to the same pages, only with different irrelevant query parameters.
* `--add-random-query-params`      Adds several random query parameters to each URL. With this, it is possible to bypass
  certain forms of server and CDN caches.
* `--max-queue-length=<num>`       The maximum length of the waiting URL queue. Increase in case of large websites, but
  expect higher memory requirements. Default is `2000`.
* `--max-visited-urls=<num>`       The maximum number of the visited URLs. Increase in case of large websites, but
  expect higher memory requirements. Default is `5000`.
* `--max-url-length=<num>`         The maximum supported URL length in chars. Increase in case of very long URLs, but
  expect higher memory requirements. Default is `2000`.

#### Export settings

* `--output-html-file=<file>`      File path for HTML output. Extension `.html` is automatically added if not specified.
* `--output-json-file=<file>`      File path for JSON output. Extension `.json` is automatically added if not
  specified.
* `--output-text-file=<file>`      File path for text output. Extension `.txt` is automatically added if not specified.
* `--output-sitemap-xml=<file>`    File path where generated XML Sitemap will be saved. Extension `.xml` is
  automatically added if not specified.
* `--output-sitemap-txt=<file>`    File path where generated TXT Sitemap will be saved. Extension `.txt` is
  automatically added if not specified.
* `--add-host-to-output-file`      Add host from initial URL as suffix to output file name. Example: you
  set `--output-json-file=/dir/report` and target filename will be `/dir/report.www.mydomain.tld.json`.
* `--add-timestamp-to-output-file` Add timestamp as suffix to output file name. Example: you
  set `--output-html-file=/dir/report` and target filename will be `/dir/report.2023-10-06.14-33-12.html`.

#### Mailer options

* `--mail-to=<email>`              Optional but required for mailer activation. Send report to given email addresses.
  You can specify multiple emails separated by comma.
* `--mail-from=<email>`            Sender email address. Default is `siteone-website-crawler@your-hostname`.
* `--mail-smtp-host=<host>`        SMTP host for sending emails. Default is `localhost`.
* `--mail-smtp-port=<port>`        SMTP port for sending emails. Default is `25`.
* `--mail-smtp-user=<user>`        SMTP user, if your SMTP server requires authentication.
* `--mail-smtp-pass=<pass>`        SMTP password, if your SMTP server requires authentication.

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

The script is provided as-is without any guarantees. You're free to modify, distribute, and use it as per your
requirements.

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
    "command": "crawler.php --url=https:\/\/www.siteone.io\/ --headers-to-table=Title --max-workers=2 --do-not-truncate-url --url-column-size=72 --output=json"
  },
  "options": {
    "url": "https:\/\/www.siteone.io\/",
    "device": "desktop",
    "outputType": "json",
    "maxWorkers": 2,
    "timeout": 10,
    "urlColumnSize": 72,
    "acceptEncoding": "gzip, deflate, br",
    "userAgent": null,
    "headersToTable": [
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