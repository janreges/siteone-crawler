<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

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
    public bool $singlePage = false;
    public int $maxDepth = 0;
    public DeviceType $device = DeviceType::DESKTOP;
    public ?string $userAgent = null;
    public int $timeout = 5;
    public ?string $proxy = null;
    public ?string $httpAuth = null;
    public ?string $timezone = null;
    public ?bool $showVersionOnly = false;
    public ?bool $showHelpOnly = false;

    // output setting
    public OutputType $outputType = OutputType::TEXT;
    public ?int $urlColumnSize;
    public bool $showInlineCriticals = false;
    public bool $showInlineWarnings = false;
    public int $rowsLimit = 200;

    /**
     * @var ExtraColumn[]
     */
    public array $extraColumns = [];
    public array $extraColumnsNamesOnly = [];
    public bool $showSchemeAndHost = false;
    public bool $doNotTruncateUrl = false;
    public bool $hideProgressBar = false;
    public bool $noColor = false;
    public bool $forceColor = false;
    public ?int $consoleWidth = null;

    // resource filtering
    public bool $disableAllAssets = false;
    public bool $disableJavascript = false;
    public bool $disableStyles = false;
    public bool $disableFonts = false;
    public bool $disableImages = false;
    public bool $disableFiles = false;
    public bool $removeAllAnchorListeners = false;

    // advanced crawler settings
    public int $workers = 3;
    public float $maxReqsPerSec = 10;
    public string $memoryLimit = '2048M';
    public array $resolve = [];
    public ?string $websocketServer = null;
    public bool $ignoreRobotsTxt = false;

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
    public bool $singleForeignPage = false;

    public StorageType $resultStorage = StorageType::MEMORY;
    public string $resultStorageDir = 'tmp/result-storage';
    public bool $resultStorageCompression = false;
    public string $acceptEncoding = 'gzip, deflate, br';
    public int $maxQueueLength = 9000;
    public int $maxVisitedUrls = 10000;
    public int $maxUrlLength = 2083; // https://stackoverflow.com/a/417184/1118709
    public int $maxSkippedUrls = 10000;
    public int $maxNon200ResponsesPerBasename = 5;
    public array $includeRegex = [];
    public array $ignoreRegex = [];
    public bool $regexFilteringOnlyForPages = false;
    public ?string $analyzerFilterRegex = null;
    public bool $addRandomQueryParams = false;
    public bool $removeQueryParams = false;
    
    /**
     * Transform URLs before crawling with `from -> to` or regexp format
     * @var string[]
     */
    public array $transformUrl = [];

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

        // disable all assets if set
        if ($this->disableAllAssets) {
            $this->disableJavascript = true;
            $this->disableStyles = true;
            $this->disableFonts = true;
            $this->disableImages = true;
            $this->disableFiles = true;
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
            new Option('--url', '-u', 'url', Type::URL, false, 'Required URL. It can also be the URL to sitemap.xml. Enclose in quotes if URL contains query parameters.', null, false),
            new Option('--single-page', '-sp', 'singlePage', Type::BOOL, false, 'Load only one page to which the URL is given (and its assets), but do not follow other pages.', false, false),
            new Option('--max-depth', '-md', 'maxDepth', Type::INT, false, 'Maximum crawling depth (for pages, not assets). Default is `0` (no limit). `1` means `/about` or `/about/`, `2` means `/about/contacts` etc.', 0, false),
            new Option('--device', '-d', 'device', Type::STRING, false, 'Device type for User-Agent selection. Values `desktop`, `tablet`, `mobile`. Ignored with `--user-agent`.', 'desktop', false),
            new Option('--user-agent', '-ua', 'userAgent', Type::STRING, false, 'Override User-Agent selected by --device. If you add `!` at the end, the siteone-crawler/version will not be added as a signature at the end of the final user-agent.', null, true),
            new Option('--timeout', '-t', 'timeout', Type::INT, false, 'Request timeout (in sec).', 5, false),
            new Option('--proxy', '-p', 'proxy', Type::HOST_AND_PORT, false, 'HTTP proxy in `host:port` format.', null),
            new Option('--http-auth', '-ha', 'httpAuth', Type::STRING, false, 'Basic HTTP authentication in `username:password` format.', null),
            new Option('--help', '-h', 'showHelpOnly', Type::BOOL, false, 'Show help and exit.', false, false),
            new Option('--version', '-v', 'showVersionOnly', Type::BOOL, false, 'Show crawler version and exit.', false, false),
        ]));

        $options->addGroup(new Group(
            self::GROUP_OUTPUT_SETTINGS,
            'Output settings', [
            new Option('--output', '-o', 'outputType', Type::STRING, false, 'Output type `text` or `json`.', 'text', false),
            new Option('--extra-columns', '-ec', 'extraColumns', Type::STRING, true, 'Extra table headers for output table with option to set width and do-not-truncate (>), e.g., `DOM,X-Cache(10),Title(40>)`.', null, true, true),
            new Option('--url-column-size', '-ucs', 'urlColumnSize', Type::INT, false, 'URL column width. By default, it is calculated from the size of your terminal window.', null, true),
            new Option('--timezone', '-tz', 'timezone', Type::STRING, false, 'Timezone for datetimes in HTML reports and timestamps in output folders/files, e.g., `Europe/Prague`. Default is `UTC`.', null, true),
            new Option('--rows-limit', '-rl', 'rowsLimit', Type::INT, false, 'Max. number of rows to display in tables with analysis results (protection against very long and slow report)', 200, false),
            new Option('--show-inline-criticals', '-sic', 'showInlineCriticals', Type::BOOL, false, 'Show criticals from the analyzer directly in the URL table.', false, false),
            new Option('--show-inline-warnings', '-siw', 'showInlineWarnings', Type::BOOL, false, 'Show warnings from the analyzer directly in the URL table.', false, false),
            new Option('--do-not-truncate-url', '-dntu', 'doNotTruncateUrl', Type::BOOL, false, 'Avoid truncating URLs to `--url-column-size`.', false, false),
            new Option('--show-scheme-and-host', '-ssah', 'showSchemeAndHost', Type::BOOL, false, 'Show the schema://host also of the original domain URL as well. By default, only path+query is displayed for original domain.', false, false),
            new Option('--hide-progress-bar', '-hpb', 'hideProgressBar', Type::BOOL, false, 'Suppress progress bar in output.', false, false),
            new Option('--no-color', '-nc', 'noColor', Type::BOOL, false, 'Disable colored output.', false, false),
            new Option('--force-color', '-fc', 'forceColor', Type::BOOL, false, 'Force colored output regardless of support detection.', false, false),
        ]));

        $options->addGroup(new Group(
            self::GROUP_RESOURCE_FILTERING,
            'Resource filtering', [
            new Option('--disable-all-assets', '-das', 'disableAllAssets', Type::BOOL, false, 'Disables crawling of all assets and files and only crawls pages in href attributes. Shortcut for calling all other `--disable-*` flags.', false, false),
            new Option('--disable-javascript', '-dj', 'disableJavascript', Type::BOOL, false, 'Disables JavaScript downloading and removes all JavaScript code from HTML, including onclick and other on* handlers.', false, false),
            new Option('--disable-styles', '-ds', 'disableStyles', Type::BOOL, false, 'Disables CSS file downloading and at the same time removes all style definitions by <style> tag or inline by style attributes.', false, false),
            new Option('--disable-fonts', '-dfo', 'disableFonts', Type::BOOL, false, 'Disables font downloading and also removes all font/font-face definitions from CSS.', false, false),
            new Option('--disable-images', '-di', 'disableImages', Type::BOOL, false, 'Disables downloading of all images and replaces found images in HTML with placeholder image only.', false, false),
            new Option('--disable-files', '-df', 'disableFiles', Type::BOOL, false, 'Disables downloading of any files (typically downloadable documents) to which various links point.', false, false),
            new Option('--remove-all-anchor-listeners', '-raal', 'removeAllAnchorListeners', Type::BOOL, false, 'On all links on the page remove any event listeners. Useful on some types of sites with modern JS frameworks.', false, false),
        ]));

        $defaultWorkers = stripos(PHP_OS, 'CYGWIN') !== false ? 1 : 3;
        $options->addGroup(new Group(
            self::GROUP_ADVANCED_CRAWLER_SETTINGS,
            'Advanced crawler settings', [
            new Option('--workers', '-w', 'workers', Type::INT, false, 'Max concurrent workers (threads). Crawler will not make more simultaneous requests to the server than this number.', $defaultWorkers, false),
            new Option('--max-reqs-per-sec', '-rps', 'maxReqsPerSec', Type::FLOAT, false, 'Max requests/s for whole crawler. Be careful not to cause a DoS attack.', 10, false),
            new Option('--memory-limit', '-ml', 'memoryLimit', Type::SIZE_M_G, false, 'Memory limit in units M (Megabytes) or G (Gigabytes).', '2048M', false),
            new Option('--resolve', '-res', 'resolve', Type::RESOLVE, true, "The ability to force the domain+port to resolve to its own IP address, just like CURL --resolve does. Example: `--resolve='www.mydomain.tld:80:127.0.0.1'`", null, true, true),
            new Option('--allowed-domain-for-external-files', '-adf', 'allowedDomainsForExternalFiles', Type::STRING, true, "Primarily, the crawler crawls only the URL within the domain for initial URL. This allows you to enable loading of file content from another domain as well (e.g. if you want to load assets from a CDN). Can be specified multiple times. Use can use domains with wildcard '*'.", [], true, true),
            new Option('--allowed-domain-for-crawling', '-adc', 'allowedDomainsForCrawling', Type::STRING, true, "This option will allow you to crawl all content from other listed domains - typically in the case of language mutations on other domains. Can be specified multiple times. Use can use domains with wildcard '*'.", [], true, true),
            new Option('--single-foreign-page', '-sfp', 'singleForeignPage', Type::BOOL, false, "If crawling of other domains is allowed (using `--allowed-domain-for-crawling`), it ensures that when another domain is not on same second-level domain, only that linked page and its assets are crawled from that foreign domain.", false, false),
            new Option('--include-regex', '--include-regexp', 'includeRegex', Type::REGEX, true, 'Include only URLs matching at least one PCRE regex. Can be specified multiple times.', [], false, true),
            new Option('--ignore-regex', '--ignore-regexp', 'ignoreRegex', Type::REGEX, true, 'Ignore URLs matching any PCRE regex. Can be specified multiple times.', [], false, true),
            new Option('--regex-filtering-only-for-pages', null, 'regexFilteringOnlyForPages', Type::BOOL, false, 'Set if you want filtering by `*-regex` rules apply only to page URLs, but static assets are loaded regardless of filtering.', false, false),
            new Option('--analyzer-filter-regex', '--analyzer-filter-regexp', 'analyzerFilterRegex', Type::REGEX, false, 'Use only analyzers that match the specified regexp.', null, true, false),
            new Option('--accept-encoding', null, 'acceptEncoding', Type::STRING, false, 'Set `Accept-Encoding` request header.', 'gzip, deflate, br', false),
            new Option('--remove-query-params', '-rqp', 'removeQueryParams', Type::BOOL, false, 'Remove URL query parameters from crawled URLs.', false, false),
            new Option('--add-random-query-params', '-arqp', 'addRandomQueryParams', Type::BOOL, false, 'Add random query parameters to each crawled URL.', false, false),
            new Option('--transform-url', '-tu', 'transformUrl', Type::REPLACE_CONTENT, true, "Transform URLs before crawling. Format: `from -> to` or `/regex/ -> replacement`. Example: `live-site.com -> local-site.local` or `/live-site\\.com\\/wp/ -> local-site.local/`. Can be specified multiple times.", null, true, true),
            new Option('--ignore-robots-txt', '-irt', 'ignoreRobotsTxt', Type::BOOL, false, 'Should robots.txt content be ignored? Useful for crawling an otherwise private/unindexed site.', false, false),
            new Option('--max-queue-length', '-mql', 'maxQueueLength', Type::INT, false, 'Max URL queue length. It affects memory requirements.', 9000, false),
            new Option('--max-visited-urls', '-mvu', 'maxVisitedUrls', Type::INT, false, 'Max visited URLs. It affects memory requirements.', 10000, false),
            new Option('--max-skipped-urls', '-msu', 'maxSkippedUrls', Type::INT, false, 'Max skipped URLs. It affects memory requirements.', 10000, false),
            new Option('--max-url-length', '-mul', 'maxUrlLength', Type::INT, false, 'Max URL length in chars. It affects memory requirements.', 2083, false),
            new Option('--max-non200-responses-per-basename', '-mnrpb', 'maxNon200ResponsesPerBasename', Type::INT, false, 'Protection against looping with dynamic non-200 URLs. If a basename (the last part of the URL after the last slash) has more non-200 responses than this limit, other URLs with same basename will be ignored/skipped.', 5, false),
        ]));

        $options->addGroup(new Group(
            self::GROUP_EXPERT_SETTINGS,
            'Expert settings', [
            new Option('--debug', null, 'debug', Type::BOOL, false, 'Activate debug mode.', false, true),
            new Option('--debug-log-file', null, 'debugLogFile', Type::FILE, false, 'Log file where to save debug messages. When --debug is not set and --debug-log-file is set, logging will be active without visible output.', null, true),
            new Option('--debug-url-regex', null, 'debugUrlRegex', Type::REGEX, true, 'Regex for URL(s) to debug. When crawled URL is matched, parsing, URL replacing and other actions are printed to output. Can be specified multiple times.', [], true, true),
            new Option('--result-storage', '-rs', 'resultStorage', Type::STRING, false, 'Result storage type for content and headers. Values: `memory` or `file`. Use `file` for large websites.', 'memory', false),
            new Option('--result-storage-dir', '-rsd', 'resultStorageDir', Type::DIR, false, 'Directory for --result-storage=file.', 'tmp/result-storage', false),
            new Option('--result-storage-compression', '-rsc', 'resultStorageCompression', Type::BOOL, false, 'Enable compression for results storage. Saves disk space, but uses more CPU.', false, false),
            new Option('--http-cache-dir', '-hcd', 'httpCacheDir', Type::DIR, false, "Cache dir for HTTP responses. You can disable cache by --http-cache-dir='off'", 'tmp/http-client-cache', false),
            new Option('--http-cache-compression', '-hcc', 'httpCacheCompression', Type::BOOL, false, "Enable compression for HTTP cache storage. Saves disk space, but uses more CPU.", false, true),
            new Option('--websocket-server', '-ws', 'websocketServer', Type::HOST_AND_PORT, false, "Start crawler with websocket server on given host:port, typically `0.0.0.0:8000`.", null, true),
            new Option('--console-width', '-cw', 'consoleWidth', Type::INT, false, "Enforce the definition of the console width and disable automatic detection.", null, true),
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
        return $this->disableAllAssets || ($this->disableJavascript && $this->disableStyles && $this->disableFonts && $this->disableImages && $this->disableFiles);
    }

    /**
     * Get initial host from URL (with port if is explicitly set)
     *
     * @param bool $includePortIfDefined
     * @return string
     */
    public function getInitialHost(bool $includePortIfDefined = true): string
    {
        static $initialHost = null;

        if ($initialHost === null) {
            $initialHost = parse_url($this->url, PHP_URL_HOST);
            if ($includePortIfDefined) {
                $initialPort = parse_url($this->url, PHP_URL_PORT);
                if ($initialPort) {
                    $initialHost .= ":{$initialPort}";
                }
            }
        }

        return $initialHost;
    }

    /**
     * Get scheme from initial URL
     * @return string
     */
    public function getInitialScheme(): string
    {
        static $initialScheme = null;

        if ($initialScheme === null) {
            $initialScheme = parse_url($this->url, PHP_URL_SCHEME);
        }

        return $initialScheme;
    }
}