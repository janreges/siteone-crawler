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
    const GROUP_RESOURCE_FILTERING = 'resource-filtering';
    const GROUP_ADVANCED_CRAWLER_SETTINGS = 'advanced-crawler-settings';
    const GROUP_EXPERT_SETTINGS = 'expert-settings';

    // basic settings
    public string $url;
    public DeviceType $device = DeviceType::DESKTOP;
    public ?string $userAgent = null;
    public int $timeout = 3;

    // output setting
    public OutputType $outputType = OutputType::TEXT;
    public int $urlColumnSize = 80;
    public bool $showInlineCriticals = false;
    public bool $showInlineWarnings = false;

    /**
     * @var ExtraColumn[]
     */
    public array $extraColumns = [];
    public array $extraColumnsNamesOnly = [];
    public bool $hideSchemeAndHost = false;
    public bool $doNotTruncateUrl = false;
    public bool $hideProgressBar = false;
    public bool $noColor = false;

    // resource filtering
    public bool $disableJavascript = false;
    public bool $disableStyles = false;
    public bool $disableFonts = false;
    public bool $disableImages = false;
    public bool $disableFiles = false;
    public bool $removeAllAnchorListeners = false;


    // advanced crawler settings
    public int $workers = 3;
    public float $maxReqsPerSec = 10;
    public string $memoryLimit = '512M';

    /**
     * Domains that are allowed for static files (e.g. CDN) but not for crawling.
     * You can use also '*', or '*.domain.tld' or '*.domain.*'
     * @var string[]
     */
    public array $allowedDomainsForExternalFiles = [];

    /**
     * Domains that are allowed for crawling. You can use also '*', or '*.domain.tld' or '*.domain.*'
     * @var string[]
     */
    public array $allowedDomainsForCrawling = [];

    public StorageType $resultStorage = StorageType::MEMORY;
    public string $resultStorageDir = 'tmp/result-storage';
    public bool $resultStorageCompression = false;
    public string $acceptEncoding = 'gzip, deflate, br';
    public int $maxQueueLength = 9000;
    public int $maxVisitedUrls = 10000;
    public int $maxUrlLength = 2083; // https://stackoverflow.com/a/417184/1118709
    public array $includeRegex = [];
    public array $ignoreRegex = [];
    public ?string $analyzerFilterRegex = null;
    public bool $addRandomQueryParams = false;
    public bool $removeQueryParams = false;

    // experts settings

    public ?string $httpCacheDir;
    public bool $httpCacheCompression = false;
    public bool $debug = false;
    public ?string $debugLogFile = null;

    /**
     * Regexes for URLs to debug. When crawled URL is matched, parsing, URL replacing and other actions are printed to output.
     * Regexes have to be PCRE compatible and are applied to full URL (including scheme and host).
     * Examples:
     *  - `/^https://www\.siteone\.io\/blog\//` - debug all URLs starting with `https://www.siteone.io/blog/`
     *  - `/contact\.html/` - debug all URLs containing `contact.html`
     *  - `/./` - debug all URLs
     */
    public array $debugUrlRegex = [];

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
                    } else if ($option->propertyToFill === 'extraColumns') {
                        foreach ($option->getValue() as $columnText) {
                            $this->extraColumns[] = ExtraColumn::fromText($columnText);
                        }
                    } else {
                        $this->{$option->propertyToFill} = $option->getValue();
                    }
                }
            }
        }

        if (!$this->url) {
            throw new Exception("Invalid or undefined --url parameter.");
        } else if ($this->workers < 1) {
            throw new Exception("Invalid value '{$this->workers}' (minimum is 1) for --workers");
        }

        $this->extraColumnsNamesOnly = [];
        foreach ($this->extraColumns as $extraColumn) {
            $this->extraColumnsNamesOnly[] = preg_replace('/\s*\(.+$/', '', $extraColumn->name);
        }

        Debugger::setConfig($this->debug, $this->debugLogFile);
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
            new Option('--extra-columns', null, 'extraColumns', Type::STRING, true, 'Extra table headers for output table with option to set width and truncate (!), e.g., `DOM,X-Cache(10),Title(40!)`.', null, true, true),
            new Option('--url-column-size', null, 'urlColumnSize', Type::INT, false, 'URL column width.', 80, false),
            new Option('--show-inline-criticals', null, 'showInlineCriticals', Type::BOOL, false, 'Show criticals from the analyzer directly in the URL table.', false, false),
            new Option('--show-inline-warnings', null, 'showInlineWarnings', Type::BOOL, false, 'Show warnings from the analyzer directly in the URL table.', false, false),
            new Option('--do-not-truncate-url', null, 'doNotTruncateUrl', Type::BOOL, false, 'Avoid truncating URLs to `--url-column-size`.', false, false),
            new Option('--hide-scheme-and-host', null, 'hideSchemeAndHost', Type::BOOL, false, 'Hide URL scheme/host in output.', false, false),
            new Option('--hide-progress-bar', null, 'hideProgressBar', Type::BOOL, false, 'Suppress progress bar in output.', false, false),
            new Option('--no-color', null, 'noColor', Type::BOOL, false, 'Disable colored output.', false, false),
        ]));

        $options->addGroup(new Group(
            self::GROUP_RESOURCE_FILTERING,
            'Resource filtering', [
            new Option('--disable-javascript', null, 'disableJavascript', Type::BOOL, false, 'Disables JavaScript downloading and removes all JavaScript code from HTML, including onclick and other on* handlers.', false, false),
            new Option('--disable-styles', null, 'disableStyles', Type::BOOL, false, 'Disables CSS file downloading and at the same time removes all style definitions by <style> tag or inline by style attributes.', false, false),
            new Option('--disable-fonts', null, 'disableFonts', Type::BOOL, false, 'Disables font downloading and also removes all font/font-face definitions from CSS.', false, false),
            new Option('--disable-images', null, 'disableImages', Type::BOOL, false, 'Disables downloading of all images and replaces found images in HTML with placeholder image only.', false, false),
            new Option('--disable-files', null, 'disableFiles', Type::BOOL, false, 'Disables downloading of any files (typically downloadable documents) to which various links point.', false, false),
            new Option('--remove-all-anchor-listeners', null, 'removeAllAnchorListeners', Type::BOOL, false, 'On all links on the page remove any event listeners. Useful on some types of sites with modern JS frameworks.', false, false),
        ]));

        $options->addGroup(new Group(
            self::GROUP_ADVANCED_CRAWLER_SETTINGS,
            'Advanced crawler settings', [
            new Option('--workers', '-w', 'workers', Type::INT, false, 'Max concurrent workers (threads). Crawler will not make more simultaneous requests to the server than this number.', 3, false),
            new Option('--max-reqs-per-sec', '-rps', 'maxReqsPerSec', Type::FLOAT, false, 'Max requests/s for whole crawler. Be careful not to cause a DoS attack.', 10, false),
            new Option('--memory-limit', null, 'memoryLimit', Type::SIZE_M_G, false, 'Memory limit in units M (Megabytes) or G (Gigabytes).', '512M', false),
            new Option('--allowed-domain-for-external-files', null, 'allowedDomainsForExternalFiles', Type::STRING, true, "Primarily, the crawler crawls only the URL within the domain for initial URL. This allows you to enable loading of file content from another domain as well (e.g. if you want to load assets from a CDN). Can be specified multiple times. Use can use domains with wildcard '*'.", [], true, true),
            new Option('--allowed-domain-for-crawling', null, 'allowedDomainsForCrawling', Type::STRING, true, "This option will allow you to crawl all content from other listed domains - typically in the case of language mutations on other domains. Can be specified multiple times. Use can use domains with wildcard '*'.", [], true, true),
            new Option('--include-regex', '--include-regexp', 'includeRegex', Type::REGEX, true, 'Include only URLs matching at least one PCRE regex. Can be specified multiple times.', [], false, true),
            new Option('--ignore-regex', '--ignore-regexp', 'ignoreRegex', Type::REGEX, true, 'Ignore URLs matching any PCRE regex. Can be specified multiple times.', [], false, true),
            new Option('--analyzer-filter-regex', '--analyzer-filter-regexp', 'analyzerFilterRegex', Type::REGEX, false, 'Use only analyzers that match the specified regexp.', null, true, false),
            new Option('--accept-encoding', null, 'acceptEncoding', Type::STRING, false, 'Set `Accept-Encoding` request header.', 'gzip, deflate, br', false),
            new Option('--remove-query-params', null, 'removeQueryParams', Type::BOOL, false, 'Remove URL query parameters from crawled URLs.', false, false),
            new Option('--add-random-query-params', null, 'addRandomQueryParams', Type::BOOL, false, 'Add random query parameters to each crawled URL.', false, false),
            new Option('--max-queue-length', null, 'maxQueueLength', Type::INT, false, 'Max URL queue length. It affects memory requirements.', 9000, false),
            new Option('--max-visited-urls', null, 'maxVisitedUrls', Type::INT, false, 'Max visited URLs. It affects memory requirements.', 10000, false),
            new Option('--max-url-length', null, 'maxUrlLength', Type::INT, false, 'Max URL length in chars. It affects memory requirements.', 2083, false),
        ]));

        $options->addGroup(new Group(
            self::GROUP_EXPERT_SETTINGS,
            'Expert settings', [
            new Option('--debug', null, 'debug', Type::BOOL, false, 'Activate debug mode.', false, true),
            new Option('--debug-log-file', null, 'debugLogFile', Type::FILE, false, 'Log file where to save debug messages. When --debug is not set and --debug-log-file is set, logging will be active without visible output.', null, true),
            new Option('--debug-url-regex', null, 'debugUrlRegex', Type::REGEX, true, 'Regex for URL(s) to debug. When crawled URL is matched, parsing, URL replacing and other actions are printed to output. Can be specified multiple times.', [], true, true),
            new Option('--result-storage', null, 'resultStorage', Type::STRING, false, 'Result storage type for content and headers. Values: `memory` or `file-system`. Use `file-system` for large websites.', 'memory', false),
            new Option('--result-storage-dir', null, 'resultStorageDir', Type::DIR, false, 'Directory for --result-storage=file-system.', 'tmp/result-storage', false),
            new Option('--result-storage-compression', null, 'resultStorageCompression', Type::BOOL, false, 'Enable compression for results storage. Saves disk space, but uses more CPU.', false, false),
            new Option('--http-cache-dir', null, 'httpCacheDir', Type::DIR, false, "Cache dir for HTTP responses. You can disable cache by --http-cache-dir=''", 'tmp/http-client-cache', true),
            new Option('--http-cache-compression', null, 'httpCacheCompression', Type::BOOL, false, "Enable compression for HTTP cache storage. Saves disk space, but uses more CPU.", false, true),
        ]));

        return $options;

    }

    public function hasHeaderToTable(string $headerName): bool
    {
        return in_array($headerName, $this->extraColumnsNamesOnly);
    }

    public function isUrlSelectedForDebug(string $url): bool
    {
        if (!$this->debugUrlRegex) {
            return false;
        }

        foreach ($this->debugUrlRegex as $regex) {
            if (preg_match($regex, $url) === 1) {
                return true;
            }
        }

        return false;
    }

    public function crawlOnlyHtmlFiles(): bool
    {
        return $this->disableJavascript && $this->disableStyles && $this->disableFonts && $this->disableImages && $this->disableFiles;
    }
}