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
    public int $timeout = 10;
    public int $tableUrlColumnSize = 100;
    public string $acceptEncoding = 'gzip, deflate, br';
    public ?string $userAgent = null;
    public array $headersToTable = [];
    public int $maxQueueLength = 1000;
    public int $maxVisitedUrls = 5000;
    public int $maxUrlLength = 2000;
    public array $crawlAssets = [];
    public bool $addRandomQueryParams = false;
    public bool $removeQueryParams = false;
    public bool $hideSchemeAndHost = false;
    public bool $truncateUrlToColumnSize = false;

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
            }else if (str_starts_with($arg, '--max-workers=')) {
                $result->maxWorkers = (int)substr($arg, 14);
                if ($result->maxWorkers <= 0) {
                    self::errorExit("Invalid value '{$result->maxWorkers}' (minimum is 1) for --max-workers");
                }
            } else if (str_starts_with($arg, '--timeout=')) {
                $result->timeout = (int)substr($arg, 10);
                if ($result->timeout <= 0) {
                    self::errorExit("Invalid value '{$result->timeout}' (minimum is 1) for --timeout");
                }
            } else if (str_starts_with($arg, '--table-url-column-size=')) {
                $result->tableUrlColumnSize = (int)substr($arg, 24);
                if ($result->tableUrlColumnSize <= 10) {
                    self::errorExit("Invalid value '{$result->tableUrlColumnSize}' (minimum is 10) for --table-url-column-size");
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
            } else if (str_starts_with($arg, '--truncate-url-to-column-size')) {
                $result->truncateUrlToColumnSize = true;
            } elseif (str_starts_with($arg, '-')) {
                self::errorExit("Unknown parameter '{$arg}'");
            }
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
            echo json_encode(['error' => $errorContent], JSON_PRETTY_PRINT|JSON_INVALID_UTF8_IGNORE);
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

    private static function displayHelp(): void
    {
        echo "\n";
        echo "Usage: ./swoole-cli crawler.php --url=https://mydomain.tld/ [optional parameters]\n";
        echo "--url=<url>                     Required. The URL address. Use quotation marks if the URL contains query parameters.\n";
        echo "--device=<device>               Optional. Device for choosing a predefined user-agent. Ignored when --user-agent is defined. Supported values: '" . implode("', '", DeviceType::getAvailableTextTypes()) . "'. Defaults is 'desktop'.\n";
        echo "--output=<value>                Optional. Output type. Supported values: '" . implode("', '", OutputType::getAvailableTextTypes()) . "'. Default is 'text'.\n";
        echo "--max-workers=<n>               Optional. Maximum number of workers (threads). Use carefully. A high number of threads will cause a DoS attack. Default is 3.\n";
        echo "--timeout=<n>                   Optional. Timeout in seconds. Default is 10.\n";
        echo "--table-url-column-size=<value> Optional. Basic URL column width. Default is 100.\n";
        echo "--accept-encoding=<value>       Optional. Custom Accept-Encoding. Default is 'gzip, deflate, br'.\n";
        echo "--user-agent=<value>            Optional. Custom user agent. Use quotation marks. If specified, it takes precedence over the device parameter.\n";
        echo "--headers-to-table=<values>     Optional. Comma delimited list of HTTP response headers added to output table. A specialty is the possibility to use 'Title', 'Keywords' and 'Description'.\n";
        echo "--crawl-assets=<values>         Optional. Comma delimited list of frontend assets you want to crawl too. Supported values: '" . implode("', '", AssetType::getAvailableTextTypes()) . "'.\n";
        echo "--max-queue-length=<n>          Optional. The maximum length of the waiting URL queue. Increase in case of large websites, but expect higher memory requirements. Default is 1000.\n";
        echo "--max-visited-urls=<n>          Optional. The maximum number of the visited URLs. Increase in case of large websites, but expect higher memory requirements. Default is 5000.\n";
        echo "--max-url-length=<n>            Optional. The maximum supported URL length in chars. Increase in case of very long URLs, but expect higher memory requirements. Default is 2000.\n";
        echo "--remove-query-params           Optional. Remove query parameters from found URLs.\n";
        echo "--add-random-query-params       Optional. Adds several random query parameters to each URL.\n";
        echo "--hide-scheme-and-host          Optional. On output, hide scheme and host of URLs for more compact view.\n";
        echo "--truncate-url-to-column-size   Optional. On output, trim the URL to the length of the column, otherwise a long URL will break the look of the table.\n";
        echo "\n";
        echo "Version: " . VERSION . "\n";
        echo "Created with ♥ by Ján Regeš (jan.reges@siteone.cz) [10/2023]\n";
    }
}