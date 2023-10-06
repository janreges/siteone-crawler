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
        echo "Usage: ./swoole-cli crawler.php --url=https://mydomain.tld/ [options]\n";
        echo "Version: " . VERSION . "\n";
        echo "\n";
        echo "Basic settings:\n";
        echo "---------------\n";
        echo "--url=<url>                    Required URL. Enclose in quotes if URL contains query parameters.\n";
        echo "--device=<device>              Device type for User-Agent. Ignored with `--user-agent`. Defaults to `desktop`.\n";
        echo "--user-agent=<value>           Override User-Agent.\n";
        echo "--timeout=<num>                Request timeout (in sec). Default `3`.\n";
        echo "\n";
        echo "Output settings:\n";
        echo "----------------\n";
        echo "--output=<value>               Output type `text` or `json`. Default `text`.\n";
        echo "--headers-to-table=<values>    HTTP headers for output table, e.g., `X-Cache(10),Title`.\n";
        echo "--url-column-size=<num>        URL column width. Default `80`.\n";
        echo "--do-not-truncate-url          Avoid truncating URLs to `--url-column-size`.\n";
        echo "--hide-scheme-and-host         Hide URL scheme/host in output.\n";
        echo "--hide-progress-bar            Suppress progress bar in output.\n";
        echo "\n";
        echo "Advanced crawler settings:\n";
        echo "--------------------------\n";
        echo "--max-workers=<num>            Max concurrent workers (threads). Default `3`.\n";
        echo "--crawl-assets=<values>        Static assets to crawl. Values `fonts`, `images`, `styles`, `scripts`, `files`\n";
        echo "--include-regex=<regex>        Include URLs matching regex. Can be specified multiple times.\n";
        echo "--ignore-regex=<regex>         Ignore URLs matching regex. Can be specified multiple times.\n";
        echo "--accept-encoding=<value>      Set `Accept-Encoding` request header. Default `gzip, deflate, br`.\n";
        echo "--remove-query-params          Remove URL query parameters from crawled URLs.\n";
        echo "--add-random-query-params      Add random query parameters to each crawled URL.\n";
        echo "--max-queue-length=<num>       Max URL queue length. It affects memory requirements. Default `2000`.\n";
        echo "--max-visited-urls=<num>       Max visited URLs. It affects memory requirements. Default `5000`.\n";
        echo "--max-url-length=<num>         Max URL length in chars. It affects memory requirements. Default `2000`.\n";
        echo "\n";
        echo "Export settings:\n";
        echo "----------------\n";
        echo "--output-html-file=<file>      Save as HTML. `.html` added if missing.\n";
        echo "--output-json-file=<file>      Save as JSON. `.json` added if missing.\n";
        echo "--output-text-file=<file>      Save as text. `.txt` added if missing.\n";
        echo "--add-host-to-output-file      Append initial URL host to filename.\n";
        echo "--add-timestamp-to-output-file Append timestamp to filename.\n";
        echo "\n";
        echo "Mailer options:\n";
        echo "---------------\n";
        echo "--mail-to=<email>              Email report to address(es).\n";
        echo "--mail-from=<email>            Sender email. Default `siteone-website-crawler@your-hostname`.\n";
        echo "--mail-smtp-host=<host>        SMTP host. Default `localhost`.\n";
        echo "--mail-smtp-port=<port>        SMTP port. Default `25`.\n";
        echo "--mail-smtp-user=<user>        SMTP user for authentication.\n";
        echo "--mail-smtp-pass=<pass>        SMTP password for authentication.\n";
        echo "\n";
        echo "For more detailed descriptions of parameters, see README.md.\n";
        echo "\n";
        echo "Created with ♥ by Ján Regeš (jan.reges@siteone.cz) from www.SiteOne.io (Czech Republic) [10/2023]\n";
    }

}