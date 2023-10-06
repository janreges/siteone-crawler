<?php

namespace Crawler;

use Crawler\Output\OutputType;
use JetBrains\PhpStorm\NoReturn;

class Options
{
    public string $url;
    public DeviceType $device = DeviceType::DESKTOP;
    public OutputType $outputType = OutputType::FORMATTED_TEXT;
    public int $maxWorkers = 3;
    public int $timeout = 3;
    public int $urlColumnSize = 80;
    public string $acceptEncoding = 'gzip, deflate, br';
    public ?string $userAgent = null;
    public array $headersToTable = [];
    public int $maxQueueLength = 2000;
    public int $maxVisitedUrls = 5000;
    public int $maxUrlLength = 2000;
    public array $crawlAssets = [];
    public array $includeRegex = [];
    public array $ignoreRegex = [];
    public bool $addRandomQueryParams = false;
    public bool $removeQueryParams = false;
    public bool $hideSchemeAndHost = false;
    public bool $doNotTruncateUrl = false;
    public bool $hideProgressBar = false;

    public ?string $outputHtmlFile = null;
    public ?string $outputJsonFile = null;
    public ?string $outputTextFile = null;
    public bool $addTimestampToOutputFile = false;
    public bool $addHostToOutputFile = false;

    public ?array $mailTo = null;
    public string $mailSmtpHost = 'localhost';
    public int $mailSmtpPort = 25;
    public ?string $mailSmtpUser = null;
    public ?string $mailSmtpPass = null;
    public string $mailFrom = 'siteone-website-crawler@your-hostname';

    private static array $required = ["url"];
    private static bool $jsonOutput = false;

    public function hasHeaderToTable(string $headerName): bool
    {
        return in_array($headerName, $this->headersToTable);
    }

    public function hasCrawlAsset(AssetType $assetType): bool
    {
        return in_array($assetType, $this->crawlAssets);
    }

    public function mailerIsActivated(): bool
    {
        return $this->mailTo !== null;
    }

    public static function parse(array $argv): self
    {
        $result = new self();

        // Parsing input parameters
        foreach ($argv as $arg) {
            if (str_starts_with($arg, '--url=')) {
                $result->url = trim(substr($arg, 6), ' "\'');
                if (@parse_url($result->url) === false) {
                    self::errorExit("Invalid URL '{$result->url}'");
                }
            } else if (str_starts_with($arg, '--output=')) {
                try {
                    $result->outputType = OutputType::fromText(trim(substr($arg, 9), ' "\''));
                    if ($result->outputType == OutputType::JSON) {
                        self::$jsonOutput = true;
                    }
                } catch (\Exception $e) {
                    self::errorExit($e->getMessage());
                }
            } else if (str_starts_with($arg, '--device=')) {
                try {
                    $result->device = DeviceType::fromText(trim(substr($arg, 9), ' "\''));
                } catch (\Exception $e) {
                    self::errorExit($e->getMessage());
                }
            } else if (str_starts_with($arg, '--max-workers=')) {
                $result->maxWorkers = (int)substr($arg, 14);
                if ($result->maxWorkers <= 0) {
                    self::errorExit("Invalid value '{$result->maxWorkers}' (minimum is 1) for --max-workers");
                }
            } else if (str_starts_with($arg, '--timeout=')) {
                $result->timeout = (int)substr($arg, 10);
                if ($result->timeout <= 0) {
                    self::errorExit("Invalid value '{$result->timeout}' (minimum is 1) for --timeout");
                }
            } else if (str_starts_with($arg, '--url-column-size=')) {
                $result->urlColumnSize = (int)substr($arg, 18);
                if ($result->urlColumnSize <= 10) {
                    self::errorExit("Invalid value '{$result->urlColumnSize}' (minimum is 10) for --url-column-size");
                }
            } else if (str_starts_with($arg, '--accept-encoding=')) {
                $result->acceptEncoding = trim(substr($arg, 18), ' "\'');
            } else if (str_starts_with($arg, '--user-agent=')) {
                $result->userAgent = trim(substr($arg, 13), ' "\'');
            } else if (str_starts_with($arg, '--headers-to-table=')) {
                $result->headersToTable = explode(',', str_replace(' ', '', trim(substr($arg, 19), ' "\'')));
            } else if (str_starts_with($arg, '--crawl-assets=')) {
                $crawlAssets = explode(',', str_replace(' ', '', trim(substr($arg, 15), ' "\'')));
                foreach ($crawlAssets as $asset) {
                    try {
                        $result->crawlAssets[] = AssetType::fromText($asset);
                    } catch (\Exception $e) {
                        self::errorExit($e->getMessage());
                    }
                }
            } else if (str_starts_with($arg, '--output-html-file=')) {
                $result->outputHtmlFile = trim(substr($arg, 19), ' "\'');
            } else if (str_starts_with($arg, '--output-json-file=')) {
                $result->outputJsonFile = trim(substr($arg, 19), ' "\'');
            } else if (str_starts_with($arg, '--output-text-file=')) {
                $result->outputTextFile = trim(substr($arg, 19), ' "\'');
            } else if (str_starts_with($arg, '--add-timestamp-to-output-file')) {
                $result->addTimestampToOutputFile = true;
            } else if (str_starts_with($arg, '--add-host-to-output-file')) {
                $result->addHostToOutputFile = true;
            } else if (str_starts_with($arg, '--max-queue-length=')) {
                $result->maxQueueLength = (int)substr($arg, 19);
                if ($result->maxQueueLength < 10) {
                    self::errorExit("Invalid value '{$result->maxQueueLength}' (minimum is 10) for --max-queue-length");
                }
            } else if (str_starts_with($arg, '--max-visited-urls=')) {
                $result->maxVisitedUrls = (int)substr($arg, 19);
                if ($result->maxVisitedUrls < 20) {
                    self::errorExit("Invalid value '{$result->maxVisitedUrls}' (minimum is 20) for --max-visited-urls");
                }
            } else if (str_starts_with($arg, '--max-url-length=')) {
                $result->maxUrlLength = (int)substr($arg, 17);
                if ($result->maxUrlLength < 50) {
                    self::errorExit("Invalid value '{$result->maxUrlLength}' (minimum is 50) for --max-url-length");
                }
            } else if (str_starts_with($arg, '--add-random-query-params')) {
                $result->addRandomQueryParams = true;
            } else if (str_starts_with($arg, '--remove-query-params')) {
                $result->removeQueryParams = true;
            } else if (str_starts_with($arg, '--hide-scheme-and-host')) {
                $result->hideSchemeAndHost = true;
            } else if (str_starts_with($arg, '--do-not-truncate-url')) {
                $result->doNotTruncateUrl = true;
            } else if (str_starts_with($arg, '--hide-progress-bar')) {
                $result->hideProgressBar = true;
            } else if (str_starts_with($arg, '--mail-to=')) {
                $result->mailTo = explode(',', str_replace(' ', '', trim(substr($arg, 10), ' "\'')));
                foreach ($result->mailTo as $email) {
                    if (!filter_var($email, FILTER_VALIDATE_EMAIL)) {
                        self::errorExit("Invalid email '{$email}' in --mail-to");
                    }
                }
            } else if (str_starts_with($arg, '--mail-smtp-host=')) {
                $result->mailSmtpHost = trim(substr($arg, 17), ' "\'');
            } else if (str_starts_with($arg, '--mail-smtp-port=')) {
                $result->mailSmtpPort = intval(trim(substr($arg, 17), ' "\''));
            } else if (str_starts_with($arg, '--mail-smtp-user=')) {
                $result->mailSmtpUser = trim(substr($arg, 17), ' "\'');
            } else if (str_starts_with($arg, '--mail-smtp-pass=')) {
                $result->mailSmtpPass = trim(substr($arg, 17), ' "\'');
            } else if (str_starts_with($arg, '--mail-from=')) {
                $result->mailFrom = trim(substr($arg, 12), ' "\'');
            } else if (str_starts_with($arg, '--include-regex=')) {
                $regex = trim(substr($arg, 16), ' "\'');
                if (@preg_match($regex, '') === false) {
                    self::errorExit("Invalid regular expression '{$regex}' in --include-regex It must be valid PCRE regex.");
                }
                $result->includeRegex[] = $regex;
            } else if (str_starts_with($arg, '--ignore-regex=')) {
                $regex = trim(substr($arg, 15), ' "\'');
                if (@preg_match($regex, '') === false) {
                    self::errorExit("Invalid regular expression '{$regex}' in --ignore-regex. It must be valid PCRE regex.");
                }
                $result->ignoreRegex[] = $regex;
            } else if (str_starts_with($arg, '-')) {
                self::errorExit("Unknown parameter '{$arg}'");
            }
        }

        // update @your-hostname to real hostname if mailer is activated
        if ($result->mailerIsActivated() && str_contains($result->mailFrom, '@your-hostname')) {
            $result->mailFrom = str_replace('@your-hostname', '@' . gethostname(), $result->mailFrom);
        }

        // Checking required parameters
        foreach (self::$required as $param) {
            if (!isset($result->$param)) {
                self::errorExit("Missing required parameter --$param");
            }
        }

        return $result;
    }

    #[NoReturn] private static function errorExit(string $message): void
    {
        $errorContent = "\nERROR: " . trim($message) . "\n";

        if (self::$jsonOutput) {
            echo json_encode(['error' => $errorContent], JSON_PRETTY_PRINT | JSON_INVALID_UTF8_IGNORE);
            exit(1);
        }

        $isOutputVisible = posix_isatty(STDOUT);
        if ($isOutputVisible) {
            echo Utils::getColorText($errorContent, 'red');
        } else {
            echo $errorContent;
        }
        self::displayHelp();
        exit(1);
    }

    public function toArray(bool $maskSensitive = true): array
    {
        $result = [];
        foreach ($this as $key => $value) {
            if ($maskSensitive && in_array($key, ['mailSmtpPass'])) {
                $value = '***';
            }
            $result[$key] = $value;
        }
        return $result;
    }

    private static function displayHelp(): void
    {
        echo "\n";
        echo "Usage: ./swoole-cli crawler.php --url=https://mydomain.tld/ [optional parameters]\n";
        echo "--url=<url>                     Required. The URL address. Use quotation marks if the URL contains query parameters.\n";
        echo "--device=<device>               Optional. Device for choosing a predefined user-agent. Ignored when --user-agent is defined. Supported values: '" . implode("', '", DeviceType::getAvailableTextTypes()) . "'. Defaults is 'desktop'.\n";
        echo "--output=<value>                Optional. Output type. Supported values: '" . implode("', '", OutputType::getAvailableTextTypes()) . "'. Default is 'text'.\n";
        echo "--max-workers=<n>               Optional. Maximum number of workers (threads). Use carefully. A high number of threads will cause a DoS attack. Default is 3.\n";
        echo "--timeout=<n>                   Optional. Timeout in seconds. Default is 3.\n";
        echo "--url-column-size=<value> Optional. Basic URL column width. Default is 80.\n";
        echo "--accept-encoding=<value>       Optional. Custom Accept-Encoding. Default is 'gzip, deflate, br'.\n";
        echo "--user-agent=<value>            Optional. Custom user agent. Use quotation marks. If specified, it takes precedence over the device parameter.\n";
        echo "--headers-to-table=<values>     Optional. Comma delimited list of HTTP response headers added to output table. A specialty is the possibility to use 'Title', 'Keywords' and 'Description'. You can set the expected length of the column in parentheses for better output - for example 'X-Cache(10)'\n";
        echo "--crawl-assets=<values>         Optional. Comma delimited list of frontend assets you want to crawl too. Supported values: '" . implode("', '", AssetType::getAvailableTextTypes()) . "'.\n";
        echo "--output-html-file=<file>       Optional. File name for HTML output. Extension `.html` is automatically added if not specified.\n";
        echo "--output-json-file=<file>       Optional. File name for JSON output. Extension `.json` is automatically added if not specified.\n";
        echo "--output-text-file=<file>       Optional. File name for text output. Extension `.txt` is automatically added if not specified.\n";
        echo "--add-timestamp-to-output-file  Optional. Add timestamp suffix to output file name. Example: you set '--output-json-file=/dir/report.json' and target filename will be '/dir/report.2023-10-06.14-33-12.html'.\n";
        echo "--add-host-to-output-file       Optional. Add host from URL suffix to output file name. Example: you set '--output-json-file=/dir/report.json' and target filename will be '/dir/report.www.mydomain.tld.html'.\n";
        echo "--include-regex=<regex>       Optional. Regular expression compatible with PHP preg_match() for URLs that should be included. Argument can be specified multiple times. Example: --include-regex='/^\/public\//'\n";
        echo "--ignore-regex=<regex>        Optional. Regular expression compatible with PHP preg_match() for URLs that should be ignored. Argument can be specified multiple times. Example: --ignore-regex='/^.*\/downloads\/.*\.pdf$/i'\n";
        echo "--max-queue-length=<n>          Optional. The maximum length of the waiting URL queue. Increase in case of large websites, but expect higher memory requirements. Default is 2000.\n";
        echo "--max-visited-urls=<n>          Optional. The maximum number of the visited URLs. Increase in case of large websites, but expect higher memory requirements. Default is 5000.\n";
        echo "--max-url-length=<n>            Optional. The maximum supported URL length in chars. Increase in case of very long URLs, but expect higher memory requirements. Default is 2000.\n";
        echo "--remove-query-params           Optional. Remove query parameters from found URLs.\n";
        echo "--add-random-query-params       Optional. Adds several random query parameters to each URL.\n";
        echo "--hide-scheme-and-host          Optional. On text output, hide scheme and host of URLs for more compact view.\n";
        echo "--do-not-truncate-url           Optional. In the text output, long URLs are truncated by default so that the table does not wrap. With this option, you can turn off the truncation.\n";
        echo "--hide-progress-bar             Optional. Hide progress bar visible in text and JSON output for more compact view.\n";
        echo "\n";
        echo "HTML report mailer options:\n";
        echo "--mail-to=<email>               Optional but required for mailer activation. Send report to email addresses. You can specify multiple emails separated by comma.\n";
        echo "--mail-smtp-host=<host>         Optional. SMTP host for sending email. Default is 'localhost'.\n";
        echo "--mail-smtp-port=<port>         Optional. SMTP port for sending email. Default is 25.\n";
        echo "--mail-smtp-user=<user>         Optional. SMTP user, if your SMTP server requires authentication.\n";
        echo "--mail-smtp-pass=<pass>         Optional. SMTP password, if your SMTP server requires authentication.\n";
        echo "--mail-from=<email>             Optional. Sender email address. Default is 'siteone-website-crawler@your-hostname'.\n";
        echo "\n";
        echo "Version: " . VERSION . "\n";
        echo "Created with ♥ by Ján Regeš (jan.reges@siteone.cz) from www.SiteOne.io (Czech Republic) [10/2023]\n";
    }
}