<?php

namespace Crawler;

use Crawler\Options\Group;
use Crawler\Options\Options;
use Crawler\Options\Option;
use Crawler\Options\Type;
use Crawler\Output\OutputType;
use Crawler\Result\Storage\StorageType;
use Exception;

class CoreOptions
{

    const GROUP_BASIC_SETTINGS = 'basic-settings';
    const GROUP_OUTPUT_SETTINGS = 'output-settings';
    const GROUP_ADVANCED_CRAWLER_SETTINGS = 'advanced-crawler-settings';

    // basic settings
    public string $url;
    public DeviceType $device = DeviceType::DESKTOP;
    public ?string $userAgent = null;
    public int $timeout = 3;

    // output setting
    public OutputType $outputType = OutputType::TEXT;
    public int $urlColumnSize = 80;
    public array $headersToTable = [];
    public array $headersToTableNamesOnly = [];
    public bool $hideSchemeAndHost = false;
    public bool $doNotTruncateUrl = false;
    public bool $hideProgressBar = false;
    public bool $noColor = false;


    // advanced crawler settings
    public int $maxWorkers = 3;
    public string $memoryLimit = '512M';
    public StorageType $resultStorage = StorageType::MEMORY;
    public string $acceptEncoding = 'gzip, deflate, br';
    public int $maxQueueLength = 9000;
    public int $maxVisitedUrls = 10000;
    public int $maxUrlLength = 2083; // https://stackoverflow.com/a/417184/1118709
    public array $crawlAssets = [];
    public array $includeRegex = [];
    public array $ignoreRegex = [];
    public bool $addRandomQueryParams = false;
    public bool $removeQueryParams = false;

    /**
     * @param Options $options
     * @throws Exception
     */
    public function __construct(Options $options)
    {
        foreach ($options->getGroups() as $group) {
            foreach ($group->options as $option) {
                if (property_exists($this, $option->propertyToFill)) {
                    if ($option->propertyToFill === 'device') {
                        $this->device = DeviceType::fromText($option->getValue());
                    } else if ($option->propertyToFill === 'outputType') {
                        $this->outputType = OutputType::fromText($option->getValue());
                    } else if ($option->propertyToFill === 'resultStorage') {
                        $this->resultStorage = StorageType::fromText($option->getValue());
                    } elseif ($option->propertyToFill === 'crawlAssets') {
                        foreach ($option->getValue() as $value) {
                            $this->crawlAssets[] = AssetType::fromText($value);
                        }
                    } else {
                        $this->{$option->propertyToFill} = $option->getValue();
                    }
                }
            }
        }

        if (!$this->url) {
            throw new Exception("Invalid or undefined --url parameter.");
        } else if ($this->maxWorkers < 1) {
            throw new Exception("Invalid value '{$this->maxWorkers}' (minimum is 1) for --max-workers");
        }

        $this->headersToTableNamesOnly = [];
        foreach ($this->headersToTable as $value) {
            $this->headersToTableNamesOnly[] = preg_replace('/\s*\(.+$/', '', $value);
        }
    }


    public static function getOptions(): Options
    {
        $options = new Options();

        $options->addGroup(new Group(
            self::GROUP_BASIC_SETTINGS,
            'Basic settings', [
            new Option('--url', '-u', 'url', Type::URL, false, 'Required URL. Enclose in quotes if URL contains query parameters.', null, false),
            new Option('--device', null, 'device', Type::STRING, false, 'Device type for User-Agent selection. Values `desktop`, `tablet`, `mobile`. Ignored with `--user-agent`.', 'desktop', false),
            new Option('--user-agent', null, 'userAgent', Type::STRING, false, 'Override User-Agent selected by --device.', null, true),
            new Option('--timeout', null, 'timeout', Type::INT, false, 'Request timeout (in sec).', 3, false),
        ]));

        $options->addGroup(new Group(
            self::GROUP_OUTPUT_SETTINGS,
            'Output settings', [
            new Option('--output', '-o', 'outputType', Type::STRING, false, 'Output type `text` or `json`.', 'text', false),
            new Option('--headers-to-table', null, 'headersToTable', Type::STRING, true, 'HTTP headers for output table, e.g., `DOM,X-Cache(10),Title`.', null, true, true),
            new Option('--url-column-size', null, 'urlToColumnSize', Type::INT, false, 'URL column width.', 80, false),
            new Option('--do-not-truncate-url', null, 'doNotTruncateUrl', Type::BOOL, false, 'Avoid truncating URLs to `--url-column-size`.', false, false),
            new Option('--hide-scheme-and-host', null, 'hideSchemeAndHost', Type::BOOL, false, 'Hide URL scheme/host in output.', false, false),
            new Option('--hide-progress-bar', null, 'hideProgressBar', Type::BOOL, false, 'Suppress progress bar in output.', false, false),
            new Option('--no-color', null, 'noColor', Type::BOOL, false, 'Disable colored output.', false, false),
        ]));

        $options->addGroup(new Group(
            self::GROUP_ADVANCED_CRAWLER_SETTINGS,
            'Advanced crawler settings', [
            new Option('--max-workers', null, 'maxWorkers', Type::INT, false, 'Max concurrent workers (threads).', 3, false),
            new Option('--memory-limit', null, 'memoryLimit', Type::SIZE_M_G, false, 'Memory limit in units M (Megabytes) or G (Gigabytes).', '512M', false),
            new Option('--result-storage', null, 'resultStorage', Type::STRING, false, 'Result storage type. Values: `memory` or `file-system`. Use `file-system` for large websites.', 'memory', false),
            new Option('--crawl-assets', null, 'crawlAssets', Type::STRING, true, 'Static assets to crawl. Comma delimited. Values: `fonts`, `images`, `styles`, `scripts`, `files`', [], false, true),
            new Option('--include-regex', '--include-regexp', 'includeRegex', Type::REGEX, true, 'Include URLs matching regex. Can be specified multiple times.', [], false, true),
            new Option('--ignore-regex', '--ignore-regexp', 'ignoreRegex', Type::REGEX, true, 'Ignore URLs matching regex. Can be specified multiple times.', [], false, true),
            new Option('--accept-encoding', null, 'acceptEncoding', Type::STRING, false, 'Set `Accept-Encoding` request header.', 'gzip, deflate, br', false),
            new Option('--remove-query-params', null, 'removeQueryParams', Type::BOOL, false, 'Remove URL query parameters from crawled URLs.', false, false),
            new Option('--add-random-query-params', null, 'addRandomQueryParams', Type::BOOL, false, 'Add random query parameters to each crawled URL.', false, false),
            new Option('--max-queue-length', null, 'maxQueueLength', Type::INT, false, 'Max URL queue length. It affects memory requirements.', 9000, false),
            new Option('--max-visited-urls', null, 'maxVisitedUrls', Type::INT, false, 'Max visited URLs. It affects memory requirements.', 10000, false),
            new Option('--max-url-length', null, 'maxUrlLength', Type::INT, false, 'Max URL length in chars. It affects memory requirements.', 2083, false),
        ]));

        return $options;

    }

    public function hasHeaderToTable(string $headerName): bool
    {
        return in_array($headerName, $this->headersToTableNamesOnly);
    }

    public function hasCrawlAsset(AssetType $assetType): bool
    {
        return in_array($assetType, $this->crawlAssets);
    }

    public function toArray(bool $maskSensitive = true): array
    {
        $result = [];
        foreach ($this as $key => $value) {
            if ($maskSensitive && $key == 'mailSmtpPass') {
                $value = '***';
            }
            $result[$key] = $value;
        }
        return $result;
    }
}