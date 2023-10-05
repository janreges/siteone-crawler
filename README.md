# SiteOne Website Crawler

SiteOne Website Crawler **is the best, most powerful and most functional website crawler you will love ♥**.

It works on all major platforms, but most easily on Linux. Windows, macOS or arm64 are also supported with little effort, see below.

## Motivation to create this tool

At [SiteOne](https://www.siteone.io/) we have been creating web applications and web presentations for our clients for more than 20 years. We have implemented hundreds of projects, and we have one need all around.

We need to check that the whole website is working great. Check that all pages respond quickly, that the title and other SEO criteria are well-designed, that there are no non-existent pages (invalid links or missing files), that the cache or security headers are set correctly, that we do not have unnecessary redirects. Last but not least, we need to perform stress tests or test protections against DoS/DDoS attacks on our infrastructure.

There are GUI tools like [Xenu's Link Sleuth](https://home.snafu.de/tilman/xenulink.html), or [Screaming Frog SEO Spider](https://www.screamingfrog.co.uk/seo-spider/), or some poor quality CLI tools. None of these tools covered all our needs. That's why we decided to create our own tool.

Ehmmmm... Enough of the marketing bullshit! What was really the most real reason? The author, head of development and infrastructure at [SiteOne](https://www.siteone.io/), wanted to prove that he could develop a great tool in 12 hours of pure working time and take a break from caring for his extremely prematurely born son. And he did it! :-) The tool is great, and his son is doing great too! ♥

## Features

* No external dependencies, no complicated installation. It works well with **any modern Linux (x64) distribution** and with little effort also on Windows, macOS or arm64 architecture.
* Efficiently crawls the website starting from the provided URL.
* Supports **public domains** and also **local domains** (e.g. `http://localhost:3000`).
* Dynamic **User-Agent** string based on selected `--device` type with the option to override by `--user-agent`.
* Option `--output=json` for better integration with CI/CD pipelines and other tools. The JSON output also has a nice visual progress reporting.
* Option `--max-workers=<num>` to specify the maximum number of workers for concurrent URL processing. You can use all your CPU cores.
* Option `--crawl-assets=<values>` to crawl also selected assets (images, fonts, styles, scripts, files).
* Option `--headers-to-table=<values>` to specify which extra headers from the HTTP response to display in the output table.
* Option `--add-random-query-params` to add random query params to test cache/anti-cache behavior.
* Option `--remove-query-params` to remove all query parameters from the URLs to skip unwanted dynamic URLs.
* Option `--hide-scheme-and-host` to hide the scheme and host from the URLs in the output table for more compact output.
* Option `--truncate-url-to-column-size` to truncate the URLs to the specified `--table-url-column-size=<size>` in the output table.
* Beautifully **formatted and colored console output** with highlighted status codes and slow response times.
* Displays **execution statistics** at the end, including AVG, MIN, and MAX response times and breakdown by HTTP status codes.
* Supports **HTTP/1.1** and **HTTP/2**. Supports **HTTP** and **HTTPS**.
* Sophisticated **error handling** and input parameters validation.
* Table output - the script provides a detailed table-like output which includes:
  * Crawled URLs
  * Status code of the HTTP response (colored based on the status)
  * Time taken for the HTTP request (colored based on the time)
  * Response size
  * Specified headers from the HTTP response (based on `--headers-to-table` argument)
  * `Title`, `Keywords` and `Description` extracted from the HTML response (based on `--headers-to-table` argument)

## Installation

Most easily installation on most Linux (x64) distributions:

```bash
git clone https://github.com/janreges/siteone-website-crawler.git
cd siteone-website-crawler
chmod +x ./swoole-cli
````

> For Windows, macOS or arm64, see below.

## Usage

To run the crawler, execute the `crawler.php` file from the command line with precompiled `swoole-cli` and provide the required arguments:

**Basic example**

```bash
./swoole-cli crawler.php --url=https://mydomain.tld/ --device=mobile
```

**Fully-featured example**

```bash
./swoole-cli crawler.php --url=https://mydomain.tld/ \
  --output=text \
  --max-workers=2 \
  --user-agent="My User-Agent String" \
  --headers-to-table="X-Cache,Title,Keywords,Description" \
  --accept-encoding="gzip, deflate" \
  --timeout=5 \
  --table-url-column-size=80 \
  --max-queue-length=3000 \
  --max-visited-urls=10000 \
  --max-url-length=5000 \
  --crawl-assets="fonts,images,styles,scripts,files" \
  --remove-query-params \
  --add-random-query-params \
  --hide-scheme-and-host \
  --truncate-url-to-column-size
```

### Arguments

#### Required:

* `--url=<value>`: The starting URL to begin crawling from.

#### Optional:

* `--device=<string>`: Specify the device type. Options: `desktop`, `mobile`, `tablet`. Defaults to `desktop` if not specified.
* `--output=<string>`: Specify the output type. Options: `text` or `json`. Defaults to `text` if not specified.
* `--max-workers=<num>`: The maximum number of workers for concurrent URL processing. Defaults to `3` if not specified.
* `--user-agent="<string>"`: The User-Agent string to use for the HTTP requests. If not provided, it defaults based on the `device` argument.
* `--headers-to-table=<string>`: Specify which extra headers from the HTTP response to display in the table output. Comma delimited. A specialty is the possibility to use `Title`, `Keywords` and `Description`. These are extracted from the HTML response and displayed in the table output.
* `--crawl-assets=<string>`: Optional. Comma delimited list of assets you want to crawl too. Supported values: `fonts`, `images`, `styles`, `scripts` and `files` (pdf, etc.). Defaults to empty if not specified so no assets are crawled.
* `--accept-encoding=<string>`: Accept-Encoding header value. Defaults to `gzip, deflate, br` if not specified.
* `--timeout=<seconds>`: Timeout duration in seconds for the HTTP requests. Defaults to `10` seconds if not specified.
* `--table-url-column-size=<num>`: Basic URL column size in chars. Defaults to `100` chars if not specified.
* `--max-queue-length=<num>`: The maximum length of the waiting URL queue. Increase in case of large websites, but expect higher memory requirements. Defaults to `2000` if not specified.
* `--max-visited-urls=<num>`: The maximum number of the visited URLs. Increase in case of large websites, but expect higher memory requirements. Defaults to `5000` if not specified.
* `--max-url-length=<num>`: The maximum supported URL length in chars. Increase in case of very long URLs with query params, but expect higher memory requirements. Defaults to `2000` if not specified.
* `--add-random-query-params`: Whether to add random query parameters to the URL. This can help in testing cache behavior.
* `--remove-query-params`: Whether to remove all query parameters from the parsed URLs.
* `--hide-scheme-and-host`: If set, URLs displayed in the output table will not include the domain.
* `--truncate-url-to-column-size`: If set, URLs displayed in the output table will be truncated to the specified column size. Otherwise, they will be wrapped to the next line.

## Windows, macOS or arm64

If using Windows, you can use [Ubuntu](https://ubuntu.com/wsl)/[Debian](https://www.linuxfordevices.com/tutorials/linux/install-debian-on-windows-wsl) in [WSL](https://learn.microsoft.com/en-us/windows/wsl/install) or you can download [swoole-cli-v4.8.13-cygwin-x64.zip](https://github.com/swoole/swoole-src/releases/download/v4.8.13/swoole-cli-v4.8.13-cygwin-x64.zip) from [Swoole releases](https://github.com/swoole/swoole-src/releases) and use precompiled `bin/swoole-cli.exe`.

If using macOS, you can download (x64) [swoole-cli-v4.8.13-macos-x64.tar.xz](https://github.com/swoole/swoole-src/releases/download/v4.8.13/swoole-cli-v4.8.13-macos-x64.tar.xz) or (arm64) [swoole-cli-v4.8.13-macos-arm64.tar.xz](https://github.com/swoole/swoole-src/releases/download/v4.8.13/swoole-cli-v4.8.13-macos-arm64.tar.xz) and use `bin/swoole-cli`.

If using arm64 Linux, you can download precompiled [swoole-cli-v4.8.13-linux-arm64.tar.xz](https://github.com/swoole/swoole-src/releases/download/v4.8.13/swoole-cli-v4.8.13-linux-arm64.tar.xz) and use `bin/swoole-cli`.

## Roadmap

* Support for configurable thresholds for response times, status codes, etc. to exit with a non-zero code.

If you have any suggestions or feature requests, please open an issue on GitHub. We'd love to hear from you!

Your contributions with realized improvements, bug fixes, and new features are welcome. Please open a pull request :-)

## Disclaimer

Please use responsibly and ensure that you have the necessary permissions when crawling websites. Some sites may have rules against automated access detailed in their robots.txt.

**The author is not responsible for any consequences caused by inappropriate use or deliberate misuse of this tool.**

## License

The script is provided as-is without any guarantees. You're free to modify, distribute, and use it as per your requirements.

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
    "command": "crawler.php --url=https:\/\/www.siteone.io\/ --headers-to-table=Title --max-workers=2 --truncate-url-to-column-size --table-url-column-size=72 --output=json"
  },
  "options": {
    "url": "https:\/\/www.siteone.io\/",
    "device": "desktop",
    "outputType": "json",
    "maxWorkers": 2,
    "timeout": 10,
    "tableUrlColumnSize": 72,
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
    "truncateUrlToColumnSize": true
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