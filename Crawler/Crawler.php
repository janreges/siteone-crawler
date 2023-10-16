<?php

namespace Crawler;

use Crawler\HttpClient\HttpClient;
use Crawler\Output\Output;
use Crawler\Parser\CssUrlParser;
use Crawler\Parser\HtmlUrlParser;
use Crawler\Result\Status;
use Crawler\Result\VisitedUrl;
use Exception;
use Swoole\Process;
use Swoole\Table;
use Swoole\Coroutine;
use Swoole\ExitException;

class Crawler
{
    private CoreOptions $options;
    private Output $output;
    private Status $status;
    private HttpClient $httpClient;

    private Table $statusTable;
    private Table $queue;
    private Table $visited;

    private ParsedUrl $initialParsedUrl;
    private string $finalUserAgent;
    private ?array $doneCallback = null;

    private static array $htmlPagesExtensions = ['htm', 'html', 'shtml', 'php', 'phtml', 'ashx', 'xhtml', 'asp', 'aspx', 'jsp', 'jspx', 'do', 'cfm', 'cgi', 'pl'];

    const CONTENT_TYPE_ID_HTML = 1;
    const CONTENT_TYPE_ID_SCRIPT = 2;
    const CONTENT_TYPE_ID_STYLESHEET = 3;
    const CONTENT_TYPE_ID_IMAGE = 4;
    const CONTENT_TYPE_ID_VIDEO = 5;
    const CONTENT_TYPE_ID_FONT = 6;
    const CONTENT_TYPE_ID_DOCUMENT = 7;
    const CONTENT_TYPE_ID_JSON = 8;
    const CONTENT_TYPE_ID_REDIRECT = 9;
    const CONTENT_TYPE_ID_OTHER = 10;

    /**
     * @param CoreOptions $options
     * @param HttpClient $httpClient
     * @param Output $output
     * @param Status $status
     * @throws Exception
     */
    public function __construct(CoreOptions $options, HttpClient $httpClient, Output $output, Status $status)
    {
        $this->options = $options;
        $this->httpClient = $httpClient;
        $this->output = $output;
        $this->status = $status;

        $this->finalUserAgent = $this->getFinalUserAgent();
        $this->status->setFinalUserAgent($this->finalUserAgent);
        $this->initialParsedUrl = ParsedUrl::parse($this->options->url);
        $this->status->setFinalUserAgent($this->finalUserAgent);
    }

    /**
     * @return void
     * @throws Exception
     */
    public function init(): void
    {
        $this->statusTable = new Table(1);
        $this->statusTable->column('workers', Table::TYPE_INT, 2);
        $this->statusTable->column('doneUrls', Table::TYPE_INT, 8);
        $this->statusTable->create();
        $this->statusTable->set('1', ['workers' => 0, 'doneUrls' => 0]);

        $this->queue = new Table($this->options->maxQueueLength);
        $this->queue->column('url', Table::TYPE_STRING, $this->options->maxUrlLength);
        $this->queue->column('uqId', Table::TYPE_STRING, 8);
        $this->queue->column('sourceUqId', Table::TYPE_STRING, 8);
        $this->queue->create();

        $this->visited = new Table($this->options->maxVisitedUrls * 1.33);
        $this->visited->column('url', Table::TYPE_STRING, $this->options->maxUrlLength);
        $this->visited->column('uqId', Table::TYPE_STRING, 8);
        $this->visited->column('sourceUqId', Table::TYPE_STRING, 8);
        $this->visited->column('time', Table::TYPE_FLOAT, 8);
        $this->visited->column('status', Table::TYPE_INT, 8);
        $this->visited->column('size', Table::TYPE_INT, 8);
        $this->visited->column('type', Table::TYPE_INT, 1); // @see self::CONTENT_TYPE_ID_*
        $this->visited->create();
    }

    /**
     * @param callable $doneCallback
     * @return void
     * @throws Exception
     */
    public function run(callable $doneCallback): void
    {
        $this->doneCallback = $doneCallback;

        // add initial URL to queue
        $this->addUrlToQueue($this->options->url);

        // print table header
        $this->output->addTableHeader();

        // start recursive coroutine to process URLs
        Coroutine\run(function () {

            // catch SIGINT (Ctrl+C), print statistics and stop crawler
            Process::signal(SIGINT, function () {
                Coroutine::cancel(Coroutine::getCid());
                call_user_func($this->doneCallback);
                throw new ExitException(Utils::getColorText('I caught the manual stop of the script. Therefore, the statistics only contain processed URLs until the script stops.', 'red', true));
            });

            while ($this->getActiveWorkersNumber() < $this->options->maxWorkers && $this->queue->count() > 0) {
                Coroutine::create([$this, 'processNextUrl']);
            }
        });
    }

    /**
     * Parses HTML body and fills queue with new founded URLs
     * Returns array with extra parsed content (Title, Keywords, Description, DOM, ...)
     *
     * @param string $body
     * @param string $url
     * @return array
     * @throws Exception
     */
    private function parseHtmlBodyAndFillQueue(string $body, string $url): array
    {
        static $regexForHtmlExtensions = null;
        if (!$regexForHtmlExtensions) {
            $regexForHtmlExtensions = '/\.(' . implode('|', HtmlUrlParser::$htmlPagesExtensions) . ')/i';
        }

        $result = [];
        $urlUqId = $this->getUrlUqId($url);

        $urlParser = new HtmlUrlParser(
            $body,
            $url,
            $this->options->hasCrawlAsset(AssetType::FILES),
            $this->options->hasCrawlAsset(AssetType::IMAGES),
            $this->options->hasCrawlAsset(AssetType::SCRIPTS),
            $this->options->hasCrawlAsset(AssetType::STYLES),
            $this->options->hasCrawlAsset(AssetType::FONTS),
        );

        $foundUrls = $urlParser->getUrlsFromHtml();

        // add suitable URLs to queue
        $this->addSuitableUrlsToQueue($foundUrls, $url, $urlUqId);

        // add extra parsed content to result (Title, Keywords, Description) if needed
        if ($this->options->hasHeaderToTable('Title')) {
            preg_match_all('/<title>([^<]*)<\/title>/i', $body, $matches);
            $result['Title'] = trim($matches[1][0] ?? '');
        }

        if ($this->options->hasHeaderToTable('Description')) {
            preg_match_all('/<meta\s+.*?name=["\']description["\']\s+content=["\']([^"\']+)["\'][^>]*>/i', $body, $matches);
            $result['Description'] = trim($matches[1][0] ?? '');
        }

        if ($this->options->hasHeaderToTable('Keywords')) {
            preg_match_all('/<meta\s+.*?name=["\']keywords["\']\s+content=["\']([^"\']+)["\'][^>]*>/i', $body, $matches);
            $result['Keywords'] = trim($matches[1][0] ?? '');
        }

        if ($this->options->hasHeaderToTable('DOM')) {
            @preg_match_all('/<\w+/', $body, $matches);
            $dom = count($matches[0] ?? []);
            $result['DOM'] = $dom;
        }

        return $result;
    }

    private function canCrawlAssetType(AssetType $assetType): bool
    {
        return in_array($assetType, $this->options->crawlAssets);
    }

    /**
     * Parse CSS body for url('xyz') and fill queue with new founded URLs (images, fonts) if are allowed
     *
     * @param string $body
     * @param string $url
     * @param string $urlUqId
     * @return void
     * @throws Exception
     */
    private function parseCssBodyAndFillQueue(string $body, string $url, string $urlUqId): void
    {
        $urlParser = new CssUrlParser(
            $body,
            $url,
            $this->canCrawlAssetType(AssetType::IMAGES),
            $this->canCrawlAssetType(AssetType::FONTS)
        );

        $this->addSuitableUrlsToQueue($urlParser->getUrlsFromCss(), $url, $urlUqId);
    }

    /**
     * Check if domain for static file download is allowed
     * Supported records:
     *  - exact.domain.tld
     *  - *.domain.tld
     *  - *.com
     *  - *.domain.*
     *  - *
     * @param string $domain
     * @return bool
     */
    public function isDomainAllowedForStaticFiles(string $domain): bool
    {
        static $cache = [];
        if (array_key_exists($domain, $cache)) {
            return $cache[$domain];
        }

        $result = false;
        foreach ($this->options->allowedDomainsForExternalFiles as $allowedDomain) {
            $wildcardRegex = '/^' . str_replace('\*', '.*', preg_quote($allowedDomain, '/')) . '$/i';
            if ($allowedDomain === '*' || $allowedDomain === $domain || preg_match($wildcardRegex, $domain) === 1) {
                $result = true;
                break;
            }
        }

        $cache[$domain] = $result;
        return $result;
    }

    /**
     * Check if external domain is allowed for whole domain crawling
     * Supported records:
     *  - exact.domain.tld
     *  - *.domain.tld
     *  - *.com
     *  - *.domain.*
     *  - *
     * @param string $domain
     * @return bool
     */
    public function isExternalDomainAllowedForCrawling(string $domain): bool
    {
        static $cache = [];
        if (array_key_exists($domain, $cache)) {
            return $cache[$domain];
        }

        $result = false;
        if ($domain === $this->initialParsedUrl->host) {
            $result = true;
        } else {
            foreach ($this->options->allowedDomainsForCrawling as $allowedDomain) {
                $wildcardRegex = '/^' . str_replace('\*', '.*', preg_quote($allowedDomain, '/')) . '$/i';
                if ($allowedDomain === '*' || $allowedDomain === $domain || preg_match($wildcardRegex, $domain) === 1) {
                    $result = true;
                    break;
                }
            }
        }

        $cache[$domain] = $result;
        return $result;
    }

    /**
     * @return void
     * @throws Exception
     */
    private function processNextUrl(): void
    {
        if ($this->queue->count() === 0) {
            return;
        }

        // take first URL from queue, remove it from queue and add it to visited
        $url = null;
        foreach ($this->queue as $urlKey => $queuedUrl) {
            $url = $queuedUrl['url'];
            $this->addUrlToVisited($url, $queuedUrl['uqId'], $queuedUrl['sourceUqId']);
            $this->queue->del($urlKey);
            break;
        }

        // end if queue is empty
        if (!$url) {
            return;
        }

        // increment workers count
        $this->statusTable->incr('1', 'workers');

        $parsedUrl = ParsedUrl::parse($url);
        $isAssetUrl = $parsedUrl->extension && !in_array($parsedUrl->extension, HtmlUrlParser::$htmlPagesExtensions);

        $scheme = $parsedUrl->scheme ?: $this->initialParsedUrl->scheme;
        if (!$parsedUrl->host || $parsedUrl->host === $this->initialParsedUrl->host) {
            $hostAndPort = $this->initialParsedUrl->host . ($this->initialParsedUrl->port !== 80 && $this->initialParsedUrl->port !== 443 ? ':' . $this->initialParsedUrl->port : '');
        } else {
            $hostAndPort = $parsedUrl->host . ($parsedUrl->port && $parsedUrl->port !== 80 && $parsedUrl->port !== 443 ? ':' . $parsedUrl->port : '');
        }

        if (!$parsedUrl->host) {
            $this->output->addError("Invalid/unsupported URL found: " . print_r($parsedUrl, true));
            return;
        }

        $absoluteUrl = $scheme . '://' . $hostAndPort . $parsedUrl->path . ($parsedUrl->query ? '?' . $parsedUrl->query : '');
        $finalUrlForHttpClient = $this->options->addRandomQueryParams ? Utils::addRandomQueryParams($parsedUrl->path) : ($parsedUrl->path . ($parsedUrl->query ? '?' . $parsedUrl->query : ''));

        // setup HTTP client, send request and get response
        $httpResponse = $this->httpClient->request(
            $parsedUrl->host,
            $this->initialParsedUrl->port,
            $scheme,
            $finalUrlForHttpClient,
            'GET',
            $this->options->timeout,
            $this->finalUserAgent,
            $this->options->acceptEncoding
        );

        $body = $httpResponse->body;
        $status = $httpResponse->statusCode;
        $elapsedTime = $httpResponse->execTime;

        if ($isAssetUrl && isset($httpResponse->headers['content-length'])) {
            $bodySize = (int)$httpResponse->headers['content-length'];
        } else {
            $bodySize = $body ? strlen($body) : 0;
        }

        // decrement workers count after request is done
        $this->statusTable->decr('1', 'workers');

        // parse HTML body and fill queue with new URLs
        $isHtmlBody = isset($httpResponse->headers['content-type']) && stripos($httpResponse->headers['content-type'], 'text/html') !== false;
        $isCssBody = isset($httpResponse->headers['content-type']) && stripos($httpResponse->headers['content-type'], 'text/css') !== false;
        $isAllowedForCrawling = $this->isUrlAllowedByRegexes($url) && $this->isExternalDomainAllowedForCrawling($parsedUrl->host);
        $extraParsedContent = [];
        if ($body && $isHtmlBody && $isAllowedForCrawling) {
            $extraParsedContent = $this->parseHtmlBodyAndFillQueue($body, $url);
        } elseif ($body && $isCssBody) {
            $this->parseCssBodyAndFillQueue($body, $url, $this->getUrlUqId($url));
        }

        if ($status >= 301 && $status <= 308) {
            $extraParsedContent['Location'] = $httpResponse->headers['location'] ?? '';
        }

        // get type self::URL_TYPE_* based on content-type header
        $contentType = $httpResponse->headers['content-type'] ?? '';
        if (isset($extraParsedContent['Location'])) {
            $type = self::CONTENT_TYPE_ID_REDIRECT;
        } else {
            $type = $this->getContentTypeIdByContentTypeHeader($contentType);
        }

        // update info about visited URL
        $isExternal = $parsedUrl->host && $parsedUrl->host !== $this->initialParsedUrl->host;
        $this->updateVisitedUrl($url, $elapsedTime, $status, $bodySize, $type, $body, $extraParsedContent, $isExternal, $isAllowedForCrawling);

        // print table row to output
        $progressStatus = $this->statusTable->get('1', 'doneUrls') . '/' . ($this->queue->count() + $this->visited->count());
        $this->output->addTableRow($httpResponse, $absoluteUrl, $status, $elapsedTime, $bodySize, $type, $extraParsedContent, $progressStatus);

        // check if crawler is done and exit or start new coroutine to process next URL
        if ($this->queue->count() === 0 && $this->getActiveWorkersNumber() === 0) {
            call_user_func($this->doneCallback);
            Coroutine::cancel(Coroutine::getCid());
        } else {
            while ($this->getActiveWorkersNumber() < $this->options->maxWorkers && $this->queue->count() > 0) {
                Coroutine::create([$this, 'processNextUrl']);
                Coroutine::sleep(0.001);
            }
        }
    }

    private function isUrlSuitableForQueue(string $url): bool
    {
        static $regexForHtmlExtensions = null;
        if (!$regexForHtmlExtensions) {
            $regexForHtmlExtensions = '/\.(' . implode('|', HtmlUrlParser::$htmlPagesExtensions) . ')/i';
        }

        if (!$this->isUrlAllowedByRegexes($url)) {
            return false;
        }

        if (($this->visited->count() + $this->queue->count()) >= $this->options->maxVisitedUrls) {
            return false;
        }

        $urlKey = $this->getUrlKeyForSwooleTable($url);

        $isInQueue = $this->queue->exist($urlKey);
        $isAlreadyVisited = $this->visited->exist($urlKey);
        $isParsable = @parse_url($url) !== false;
        $isUrlWithHtml = preg_match('/\.[a-z0-9]{1,10}(|\?.*)$/i', $url) === 0 || preg_match($regexForHtmlExtensions, $url) === 1;
        $parseableAreOnlyHtmlFiles = empty($this->options->crawlAssets);

        return !$isInQueue && !$isAlreadyVisited && $isParsable && ($isUrlWithHtml || !$parseableAreOnlyHtmlFiles);
    }

    private function isUrlAllowedByRegexes(string $url): bool
    {
        $isAllowed = $this->options->includeRegex === [];
        foreach ($this->options->includeRegex as $includeRegex) {
            if (preg_match($includeRegex, $url) === 1) {
                $isAllowed = true;
                break;
            }
        }
        foreach ($this->options->ignoreRegex as $ignoreRegex) {
            if (preg_match($ignoreRegex, $url) === 1) {
                $isAllowed = false;
                break;
            }
        }
        return $isAllowed;
    }

    /**
     * @param string $url
     * @param string|null $sourceUqId
     * @return void
     * @throws Exception
     */
    private function addUrlToQueue(string $url, ?string $sourceUqId = null): void
    {
        if (!$this->queue->set($this->getUrlKeyForSwooleTable($url), [
            'url' => $url,
            'uqId' => $this->getUrlUqId($url),
            'sourceUqId' => $sourceUqId,
        ])) {
            $error = "ERROR: Unable to queue URL '{$url}'. Set higher --max-queue-length.";
            $this->output->addError($error);
            throw new Exception($error);
        }
    }

    /**
     * @param string $url
     * @param string $uqId
     * @param string $sourceUqId
     * @return void
     * @throws Exception
     */
    private function addUrlToVisited(string $url, string $uqId, string $sourceUqId): void
    {
        if (!$this->visited->set($this->getUrlKeyForSwooleTable($url), ['url' => $url, 'uqId' => $uqId, 'sourceUqId' => $sourceUqId])) {
            $error = "ERROR: Unable to add visited URL '{$url}'. Set higher --max-visited-urls or --max-url-length.";
            $this->output->addError($error);
            throw new Exception($error);
        }
    }

    /**
     * @param string $url
     * @param float $elapsedTime
     * @param int $status
     * @param int $size
     * @param int $type @see self::URL_TYPE_*
     * @param string|null $body
     * @param array|null $extras
     * @param bool $isExternal
     * @param bool $isAllowedForCrawling
     * @return void
     * @throws Exception
     */
    private function updateVisitedUrl(string $url, float $elapsedTime, int $status, int $size, int $type, ?string $body, ?array $extras, bool $isExternal, bool $isAllowedForCrawling): void
    {
        $urlKey = $this->getUrlKeyForSwooleTable($url);
        $visitedUrl = $this->visited->get($urlKey);
        if (!$visitedUrl) {
            throw new Exception("ERROR: Unable to handle visited URL '{$url}'. Set higher --max-visited-urls or --max-url-length.");
        }
        $visitedUrl['time'] = $elapsedTime;
        $visitedUrl['status'] = $status;
        $visitedUrl['size'] = $size;
        $visitedUrl['type'] = $type;
        $this->visited->set($urlKey, $visitedUrl);

        $this->statusTable->incr('1', 'doneUrls');

        $this->status->addVisitedUrl(
            new VisitedUrl(
                $visitedUrl['uqId'],
                $visitedUrl['sourceUqId'],
                $visitedUrl['url'],
                $visitedUrl['status'],
                $visitedUrl['time'],
                $visitedUrl['size'],
                $visitedUrl['type'],
                $extras,
                $isExternal,
                $isAllowedForCrawling,
            ), $body
        );
    }

    private function getUrlKeyForSwooleTable(string $url): string
    {
        $parsedUrl = parse_url($url);
        $relevantParts = ($parsedUrl['host'] ?? '') . ($parsedUrl['path'] ?? '/') . ($parsedUrl['query'] ?? '');
        return md5($relevantParts);
    }

    private function getActiveWorkersNumber(): int
    {
        return $this->statusTable->get('1', 'workers');
    }

    private function getUrlUqId(string $url): string
    {
        return substr(md5($url), 0, 8);
    }

    private function getContentTypeIdByContentTypeHeader(string $contentTypeHeader): int
    {
        $typeId = self::CONTENT_TYPE_ID_OTHER;
        if (str_contains($contentTypeHeader, 'text/html')) {
            $typeId = self::CONTENT_TYPE_ID_HTML;
        } elseif (str_contains($contentTypeHeader, 'text/javascript') || str_contains($contentTypeHeader, 'application/javascript') || str_contains($contentTypeHeader, 'application/x-javascript')) {
            $typeId = self::CONTENT_TYPE_ID_SCRIPT;
        } elseif (str_contains($contentTypeHeader, 'text/css')) {
            $typeId = self::CONTENT_TYPE_ID_STYLESHEET;
        } elseif (str_contains($contentTypeHeader, 'image/')) {
            $typeId = self::CONTENT_TYPE_ID_IMAGE;
        } elseif (str_contains($contentTypeHeader, 'video/')) {
            $typeId = self::CONTENT_TYPE_ID_VIDEO;
        } elseif (str_contains($contentTypeHeader, 'font/')) {
            $typeId = self::CONTENT_TYPE_ID_FONT;
        } elseif (str_contains($contentTypeHeader, 'application/json')) {
            $typeId = self::CONTENT_TYPE_ID_JSON;
        } elseif (str_contains($contentTypeHeader, 'application/pdf') || str_contains($typeId, 'application/msword') || str_contains($typeId, 'application/vnd.ms-excel') || str_contains($typeId, 'application/vnd.ms-powerpoint')) {
            $typeId = self::CONTENT_TYPE_ID_DOCUMENT;
        }
        return $typeId;
    }

    /**
     * @return string
     * @throws Exception
     */
    public function getFinalUserAgent(): string
    {
        if ($this->options->userAgent) {
            return $this->options->userAgent;
        }

        return match ($this->options->device) {
            DeviceType::DESKTOP => 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/' . date('y') . '.0.0.0 Safari/537.36',
            DeviceType::MOBILE => 'Mozilla/5.0 (iPhone; CPU iPhone OS 15_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.0 Mobile/15A5370a Safari/604.1',
            DeviceType::TABLET => 'Mozilla/5.0 (Linux; Android 11; SAMSUNG SM-T875) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/14.0 Chrome/87.0.4280.141 Safari/537.36',
            default => throw new Exception("Unsupported device '{$this->options->device}'"),
        };
    }

    public function getVisited(): Table
    {
        return $this->visited;
    }

    public function getQueue(): Table
    {
        return $this->queue;
    }

    public function getOutput(): Output
    {
        return $this->output;
    }

    public function getCoreOptions(): CoreOptions
    {
        return $this->options;
    }

    /**
     * @return int[]
     */
    public static function getContentTypeIds(): array
    {
        return [
            self::CONTENT_TYPE_ID_HTML,
            self::CONTENT_TYPE_ID_SCRIPT,
            self::CONTENT_TYPE_ID_STYLESHEET,
            self::CONTENT_TYPE_ID_IMAGE,
            self::CONTENT_TYPE_ID_VIDEO,
            self::CONTENT_TYPE_ID_FONT,
            self::CONTENT_TYPE_ID_DOCUMENT,
            self::CONTENT_TYPE_ID_JSON,
            self::CONTENT_TYPE_ID_REDIRECT,
            self::CONTENT_TYPE_ID_OTHER,
        ];
    }

    /**
     * @param FoundUrls $foundUrls
     * @param string $url
     * @param string $urlUqId
     * @return void
     * @throws Exception
     */
    private function addSuitableUrlsToQueue(FoundUrls $foundUrls, string $url, string $urlUqId): void
    {
        foreach ($foundUrls->getUrls() as $foundUrl) {
            $urlForQueue = trim($foundUrl->url);
            $origUrlForQueue = $urlForQueue;
            $isUrlForDebug = $this->options->isUrlSelectedForDebug($origUrlForQueue);
            $parsedUrlForQueue = ParsedUrl::parse(trim($urlForQueue));

            // skip URLs that are not on the same host, allowed domain or are not real HTML URLs
            $isRequestableResource = Utils::isHrefForRequestableResource($urlForQueue);
            $isUrlOnSameHost = !$parsedUrlForQueue->host || $parsedUrlForQueue->host === $this->initialParsedUrl->host;
            $isUrlOnAllowedHost = false;
            if ($parsedUrlForQueue->host && $parsedUrlForQueue->host !== $this->initialParsedUrl->host) {
                $isUrlOnAllowedStaticFileHost = $this->options->allowedDomainsForExternalFiles && $this->isDomainAllowedForStaticFiles($parsedUrlForQueue->host);
                $isUrlOnAllowedCrawlableDomain = $this->options->allowedDomainsForCrawling && $this->isExternalDomainAllowedForCrawling($parsedUrlForQueue->host);
                if (($isUrlOnAllowedStaticFileHost && $foundUrl->isIncludedAsset()) || $isUrlOnAllowedCrawlableDomain) {
                    $isUrlOnAllowedHost = true;
                }
            }

            if (!$isRequestableResource) {
                $isUrlForDebug && Debugger::debug('ignored-url_not-resource', "URL '{$urlForQueue}' ignored because it's not requestable resource.");
                continue;
            } elseif (!$isUrlOnSameHost && !$isUrlOnAllowedHost) {
                $isUrlForDebug && Debugger::debug('ignored-url_not-allowed-host', "URL '{$urlForQueue}' ignored because it's not requestable resource.");
                continue;
            } elseif (!self::isUrlAllowedByRobotsTxt($parsedUrlForQueue->host ?: $this->initialParsedUrl->host, $urlForQueue, $parsedUrlForQueue->port ?: $this->initialParsedUrl->port)) {
                $isUrlForDebug && Debugger::debug('ignored-url_blocked-by-robots-txt', "URL '{$urlForQueue}' ignored because is blocked by website's robots.txt.");
                continue;
            }

            // build URL for queue
            $urlForQueue = Utils::getAbsoluteUrlByBaseUrl($url, $urlForQueue);

            if (!$urlForQueue) {
                $isUrlForDebug && Debugger::debug('ignored-url_unable-to-build-absolute', "URL '{$origUrlForQueue}' ignored because it's not possible to build absolute URL.");
                continue;
            }

            // remove hash from URL
            $urlForQueue = preg_replace('/#.*$/', '', $urlForQueue);

            // remove query params from URL if needed
            if ($this->options->removeQueryParams) {
                $urlForQueue = preg_replace('/\?.*$/', '', $urlForQueue);
            }

            // add URL to queue if it's not already there
            if ($this->isUrlSuitableForQueue($urlForQueue)) {
                $this->addUrlToQueue($urlForQueue, $urlUqId);
            }
        }
    }

    /**
     * Checks if URL is allowed by robots.txt of given domain. It respects all Disallow rules and User-Agent or Allow rules are ignored
     * Has internal static cache for disallowed paths to minimize requests to robots.txt
     *
     * @param string $domain
     * @param string $url
     * @param int|null $extraPort
     * @return bool
     */
    public static function isUrlAllowedByRobotsTxt(string $domain, string $url, ?int $extraPort = null): bool
    {
        // when URL is for frontend asset (internal or external), we can assume that it's allowed
        if (preg_match('/\.(js|css|json|eot|ttf|woff2|woff|otf|png|gif|jpg|jpeg|ico|webp|avif|tif|bmp|svg)/i', $url) === 1) {
            return true;
        }

        static $disallowedPathsPerDomain = [];
        $disallowedPaths = null;

        $cacheKey = $domain . ($extraPort ? ':' . $extraPort : '');
        if (array_key_exists($cacheKey, $disallowedPathsPerDomain)) {
            $disallowedPaths = $disallowedPathsPerDomain[$cacheKey];
        } else {
            $ports = $extraPort ? [$extraPort] : [443, 80];
            foreach ($ports as $port) {
                $httpClient = new HttpClient(null);
                $robotsTxtResponse = $httpClient->request(
                    $domain,
                    $port,
                    $port === 443 ? 'https' : 'http', // warning: this will not work for HTTPS with non-standard port
                    '/robots.txt',
                    'GET',
                    1,
                    self::getCrawlerUserAgent(),
                    'gzip, deflate'
                );

                if ($robotsTxtResponse->statusCode === 200 && $robotsTxtResponse->body) {
                    $robotsTxt = $robotsTxtResponse->body;
                    $lines = explode("\n", $robotsTxt);
                    $disallowedPaths = [];
                    foreach ($lines as $line) {
                        $line = trim(preg_replace('/#.*/', '', $line)); // remove comments
                        if (preg_match('/^Disallow:\s*(.*)/i', $line, $matches)) {
                            if (trim($matches[1]) !== '') {
                                $disallowedPaths[] = trim($matches[1]);
                            }
                        }
                    }

                    Debugger::debug(
                        'robots-txt',
                        "Loaded robots.txt for domain '%s' and port '%s'. Disallowed paths: %s",
                        [$domain . '/' . $url, $port, implode(', ', $disallowedPaths)],
                        Debugger::DEBUG,
                        $robotsTxtResponse->execTime,
                        strlen($robotsTxtResponse->body)
                    );
                    break;
                } else {
                    Debugger::debug(
                        'robots-txt',
                        "Unable to load robots.txt for domain '%s' and port '%s'. Response code: %d",
                        [$domain . '/' . $url, $port, $robotsTxtResponse->statusCode],
                        Debugger::NOTICE,
                        $robotsTxtResponse->execTime,
                        strlen($robotsTxtResponse->body ?: '')
                    );
                }
            }

            $disallowedPathsPerDomain[$cacheKey] = $disallowedPaths;
        }

        // if we don't have disallowed paths, we can assume that everything is allowed
        if (!$disallowedPaths) {
            return true;
        }

        $urlPath = parse_url($url, PHP_URL_PATH);
        foreach ($disallowedPaths as $disallowedPath) {
            if ($urlPath && stripos($urlPath, $disallowedPath) === 0) {
                return false;
            }
        }

        return true;
    }

    /**
     * Get User-Agent used for specific cases (e.g. downloading of robots.txt)
     * @return string
     */
    public static function getCrawlerUserAgent(): string
    {
        return 'siteone-website-crawler/v' . VERSION;
    }

}