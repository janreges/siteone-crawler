<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler;

use Crawler\ContentProcessor\AstroProcessor;
use Crawler\ContentProcessor\CssProcessor;
use Crawler\ContentProcessor\HtmlProcessor;
use Crawler\ContentProcessor\JavaScriptProcessor;
use Crawler\ContentProcessor\Manager as ContentProcessorManager;
use Crawler\ContentProcessor\NextJsProcessor;
use Crawler\ContentProcessor\SvelteProcessor;
use Crawler\ContentProcessor\XmlProcessor;
use Crawler\Export\MailerExporter;
use Crawler\HttpClient\HttpClient;
use Crawler\HttpClient\HttpResponse;
use Crawler\Output\Output;
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
    private Table $skipped;

    private ContentProcessorManager $contentProcessorManager;

    private ParsedUrl $initialParsedUrl;
    private string $finalUserAgent;
    private ?array $doneCallback = null;
    private ?array $visitedUrlCallback = null;
    private bool $initialExistingUrlFound = false;
    private bool $terminated = false;

    // rate limiting
    private ?float $optimalDelayBetweenRequests;
    private float $lastRequestTime = 0;

    private string $acceptHeader = 'text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7';
    private static int $loadedRobotsTxtCount = 0;

    // websocket server & client to send messages through it
    private ?Process $websocketServerProcess = null;
    private ?Coroutine\Client $websocketClient = null;

    /**
     * This array contains the basenames (the slug after the last slash in the URL) of URLs != 200 OK and the number of their 404s.
     * This is used to prevent the queue from being filled with meaningless relative URLs - for example, when website contains
     * a non-existent file <img src="relative/my-image.jpg" /> .. real use-case from the 404 page on the v2.svelte.dev.
     *
     * When the crawler finds a more than --max-non200-responses-per-basename non-existent URLs with the same basename,
     * it will not add any more URLs with this basename to the queue.
     *
     * @var array <string, int>
     */
    private array $non200BasenamesToOccurrences = [];

    const CONTENT_TYPE_ID_HTML = 1;
    const CONTENT_TYPE_ID_SCRIPT = 2;
    const CONTENT_TYPE_ID_STYLESHEET = 3;
    const CONTENT_TYPE_ID_IMAGE = 4;
    const CONTENT_TYPE_ID_AUDIO = 11;
    const CONTENT_TYPE_ID_VIDEO = 5;
    const CONTENT_TYPE_ID_FONT = 6;
    const CONTENT_TYPE_ID_DOCUMENT = 7;
    const CONTENT_TYPE_ID_JSON = 8;
    const CONTENT_TYPE_ID_REDIRECT = 9;
    const CONTENT_TYPE_ID_OTHER = 10;
    const CONTENT_TYPE_ID_XML = 12;

    const SKIPPED_REASON_NOT_ALLOWED_HOST = 1;
    const SKIPPED_REASON_ROBOTS_TXT = 2;
    const SKIPPED_REASON_EXCEEDS_MAX_DEPTH = 3;

    const SKIPPED_REASONS = [
        self::SKIPPED_REASON_NOT_ALLOWED_HOST => 'Not allowed host',
        self::SKIPPED_REASON_ROBOTS_TXT => 'Robots.txt',
        self::SKIPPED_REASON_EXCEEDS_MAX_DEPTH => 'Exceeds max depth',
    ];

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

        $this->registerContentProcessors();
    }

    /**
     * @return void
     * @throws Exception
     */
    public function init(): void
    {
        $this->statusTable = new Table(1);
        $this->statusTable->column('workers', Table::TYPE_INT, 4);
        $this->statusTable->column('doneUrls', Table::TYPE_INT, 8);
        $this->statusTable->column('skippedUrls', Table::TYPE_INT, 8);
        $this->statusTable->create();
        $this->statusTable->set('1', ['workers' => 0, 'doneUrls' => 0, 'skippedUrls' => 0]);

        $this->queue = new Table($this->options->maxQueueLength);
        $this->queue->column('url', Table::TYPE_STRING, $this->options->maxUrlLength);
        $this->queue->column('uqId', Table::TYPE_STRING, 8);
        $this->queue->column('sourceUqId', Table::TYPE_STRING, 8);
        $this->queue->column('sourceAttr', Table::TYPE_INT, 2);
        $this->queue->create();

        $this->visited = new Table(intval($this->options->maxVisitedUrls * 1.33));
        $this->visited->column('url', Table::TYPE_STRING, $this->options->maxUrlLength);
        $this->visited->column('uqId', Table::TYPE_STRING, 8);
        $this->visited->column('sourceUqId', Table::TYPE_STRING, 8);
        $this->visited->column('sourceAttr', Table::TYPE_INT, 2);
        $this->visited->column('time', Table::TYPE_FLOAT, 8);
        $this->visited->column('status', Table::TYPE_INT, 8);
        $this->visited->column('size', Table::TYPE_INT, 8);
        $this->visited->column('type', Table::TYPE_INT, 1); // @see self::CONTENT_TYPE_ID_*
        $this->visited->create();

        $this->skipped = new Table(intval($this->options->maxSkippedUrls * 1.33));
        $this->skipped->column('url', Table::TYPE_STRING, $this->options->maxUrlLength);
        $this->skipped->column('reason', Table::TYPE_INT, 1); // @see self::SKIPPED_REASON_*
        $this->skipped->column('sourceUqId', Table::TYPE_STRING, 8);
        $this->skipped->column('sourceAttr', Table::TYPE_INT, 2);
        $this->skipped->create();
    }

    /**
     * @param callable $doneCallback
     * @param callable $visitedUrlCallback
     * @return void
     * @throws Exception
     */
    public function run(callable $doneCallback, callable $visitedUrlCallback): void
    {
        $this->doneCallback = $doneCallback;
        $this->visitedUrlCallback = $visitedUrlCallback;
        $this->optimalDelayBetweenRequests = max(1 / ($this->options->maxReqsPerSec), 0.001);

        // add initial URL to queue
        $this->addUrlToQueue($this->initialParsedUrl, null, FoundUrl::SOURCE_INIT_URL);

        // print table header
        $this->output->addTableHeader();

        // set corouting settings
        $this->setupCoroutines();

        // start websocket if needed
        $this->startWebSocketServerIfNeeded();

        // start recursive coroutine to process URLs
        Coroutine\run(function () {
            // catch SIGINT (Ctrl+C), print statistics and stop crawler
            Process::signal(SIGINT, function () {
                $this->stopWebSocketServer();
                Coroutine::cancel(Coroutine::getCid());
                MailerExporter::$crawlerInterrupted = true; // stop sending emails
                call_user_func($this->doneCallback);
                throw new ExitException(Utils::getColorText('I caught the manual stop of the script. Therefore, the statistics only contain processed URLs until the script stops.', 'red', true));
            });

            // run first recursive coroutine
            Coroutine::create([$this, 'processNextUrl']);
        });
    }

    /**
     * @return void
     * @throws Exception
     */
    private function startWebSocketServerIfNeeded(): void
    {
        if ($this->options->websocketServer) {
            list($wsHost, $wsPort) = explode(':', $this->options->websocketServer);
            $wsPort = intval($wsPort);
            $tcpHost = $wsHost;
            $tcpPort = $wsPort + 1;

            $swooleBinFile = BASE_DIR . '/bin/swoole-cli';
            if (!is_file($swooleBinFile) && is_file($swooleBinFile . '.exe')) {
                $swooleBinFile = $swooleBinFile . '.exe';
            } elseif (!is_file($swooleBinFile) || !is_executable($swooleBinFile)) {
                throw new Exception("Swoole binary file '{$swooleBinFile}' not found or is not executable.");
            }

            $this->websocketServerProcess = new Process(function ($process) use ($swooleBinFile, $wsHost, $wsPort, $tcpHost, $tcpPort) {
                $process->exec($swooleBinFile, [
                    BASE_DIR . '/src/ws-server.php',
                    '--tcp-host=' . $tcpHost,
                    '--tcp-port=' . $tcpPort,
                    '--ws-host=' . $wsHost,
                    '--ws-port=' . $wsPort,
                ]);
            });

            $this->websocketServerProcess->start();
            sleep(1);
            $this->websocketClient = new Coroutine\Client(SWOOLE_SOCK_TCP);
        }
    }

    public function stopWebSocketServer(): void
    {
        $this->websocketClient?->close();
        if ($this->websocketServerProcess) {
            $this->output->addNotice("Stopping WebSocket server...");
            $this->websocketServerProcess->kill($this->websocketServerProcess->pid, SIGKILL);
            $this->websocketServerProcess->wait();
        }
    }

    public function sendWebSocketMessage(string $message): void
    {
        static $connected = false;
        if (!$connected && $this->websocketClient && !$this->websocketClient->isConnected()) {
            $connected = true;
            list($wsHost, $wsPort) = explode(':', $this->options->websocketServer);
            $wsPort = intval($wsPort);
            $tcpPort = $wsPort + 1;
            $wsHost = $wsHost === '0.0.0.0' ? '127.0.0.1' : $wsHost;
            $this->websocketClient->connect($wsHost, $tcpPort, 1);
            $this->output->addNotice("WebSocket client connected to {$wsHost}:{$tcpPort}");
        }
        $this->websocketClient?->send($message);
    }

    /**
     * Parses HTML body and fills queue with new founded URLs
     * Returns array with extra parsed content (Title, Keywords, Description, DOM, ...)
     *
     * @param string $body
     * @param int $contentType
     * @param ParsedUrl $url
     * @return array
     * @throws Exception
     */
    private function parseHtmlBodyAndFillQueue(string $body, int $contentType, ParsedUrl $url): array
    {
        static $regexForHtmlExtensions = null;
        if (!$regexForHtmlExtensions) {
            $regexForHtmlExtensions = '/\.(' . implode('|', HtmlProcessor::$htmlPagesExtensions) . ')/i';
        }

        $result = [];
        $urlUqId = $this->getUrlUqId($url);

        // add suitable URLs to queue
        $this->parseContentAndFillUrlQueue($body, $contentType, $url, $urlUqId);

        // add extra parsed content to result (Title, Keywords, Description) if needed
        // title & descriptions are needed by BestPracticeAnalyzer so they are parsed even if not needed for output
        preg_match_all('/<title[^>]*>([^<]*)<\/title>/i', $body, $matches);
        $result['Title'] = $this->decodeEncodedHtmlEntities(trim($matches[1][0] ?? ''));

        preg_match_all('/<meta\s+[^>]*name=["\']description["\']\s+[^>]*content=["\']([^"\']+)["\'][^>]*>/i', $body, $matches);
        $result['Description'] = $this->decodeEncodedHtmlEntities(trim($matches[1][0] ?? ''));

        if ($this->options->hasHeaderToTable('Keywords')) {
            preg_match_all('/<meta\s+[^>]*name=["\']keywords["\']\s+[^>]*content=["\']([^"\']+)["\'][^>]*>/i', $body, $matches);
            $result['Keywords'] = $this->decodeEncodedHtmlEntities(trim($matches[1][0] ?? ''));
        }

        if ($this->options->hasHeaderToTable('DOM')) {
            @preg_match_all('/<\w+/', $body, $matches);
            $dom = count($matches[0] ?? []);
            $result['DOM'] = $dom;
        }

        // Add custom extraction for extra columns defined using xpath or regexp
        foreach ($this->options->extraColumns as $extraColumn) {
            if (isset($extraColumn->customMethod)) {
                $result[$extraColumn->name] = $extraColumn->extractValue($body);
            }
        }

        return $result;
    }

    /**
     * Parse HTML/CSS/JS body and fill queue with new founded URLs if are allowed
     *
     * @param string $content
     * @param int $contentType
     * @param ParsedUrl $url
     * @param string $urlUqId
     * @return void
     * @throws Exception
     */
    private function parseContentAndFillUrlQueue(string $content, int $contentType, ParsedUrl $url, string $urlUqId): void
    {
        $foundUrlsList = $this->contentProcessorManager->findUrls($content, $contentType, $url);
        foreach ($foundUrlsList as $foundUrls) {
            $this->addSuitableUrlsToQueue($foundUrls, $url, $urlUqId);
        }
    }

    /**
     * @param string|null $text
     * @return string|null
     */
    private function decodeEncodedHtmlEntities(?string $text): ?string
    {
        if ($text === null) {
            return null;
        }

        if (str_contains($text, '&#x') || str_contains($text, '&amp;') || str_contains($text, '&ndash;')) {
            $text = html_entity_decode($text, ENT_QUOTES | ENT_HTML5, 'UTF-8');
        }

        return $text;
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
        if ($this->queue->count() === 0 || $this->terminated) {
            return;
        }

        try {
            // take first URL from queue, remove it from queue and add it to visited
            $url = null;
            $sourceUqId = null;
            foreach ($this->queue as $urlKey => $queuedUrl) {
                $url = $queuedUrl['url'];
                $sourceUqId = $queuedUrl['sourceUqId'];
                $this->addUrlToVisited(ParsedUrl::parse($url), $queuedUrl['uqId'], $sourceUqId, $queuedUrl['sourceAttr']);
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
            $parsedUrlUqId = $this->getUrlUqId($parsedUrl);
            $isAssetUrl = $parsedUrl->extension && !in_array($parsedUrl->extension, HtmlProcessor::$htmlPagesExtensions);

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
            $origin = $sourceUqId ? $this->status->getOriginHeaderValueBySourceUqId($sourceUqId) : null;

            $isImage = $parsedUrl->extension && in_array($parsedUrl->extension, ['jpg', 'jpeg', 'png', 'gif', 'webp', 'avif', 'svg', 'ico']);

            // set Origin header only for non-image URLs (otherwise, e.g. cdn.sanity.io response to *.svg with 403 and JSON error with 'CORS Origin not allowed')
            $setOrigin = $origin && !$isImage;

            // for security reasons, we only send auth data to the same 2nd tier domain (and possibly subdomains). With HTTP basic auth, the name
            // and password are only base64 encoded and we would send them to foreign domains (which are referred to from the crawled website).
            $useHttpAuthIfConfigured = $this->initialParsedUrl->domain2ndLevel
                ? ($parsedUrl->domain2ndLevel === $this->initialParsedUrl->domain2ndLevel)
                : ($parsedUrl->host === $this->initialParsedUrl->host);

            // setup HTTP client, send request and get response
            $urlBaseName = $parsedUrl->getBaseName();
            if ($urlBaseName && isset($this->non200BasenamesToOccurrences[$urlBaseName]) && $this->non200BasenamesToOccurrences[$urlBaseName] > $this->options->maxNon200ResponsesPerBasename) {
                $httpResponse = HttpResponse::createSkipped($finalUrlForHttpClient, "URL with basename '{$urlBaseName}' has more than " . $this->options->maxNon200ResponsesPerBasename . " non-200 responses (" . $this->non200BasenamesToOccurrences[$urlBaseName] . ").");
            } else {
                $port = $parsedUrl->port ?: ($scheme === 'https' ? 443 : 80);
                
                // Apply URL transformations for HTTP request
                $transformedRequest = $this->applyHttpRequestTransformations($parsedUrl->host, $finalUrlForHttpClient);
                $httpRequestHost = $transformedRequest['host'];
                $httpRequestPath = $transformedRequest['path'];
                
                $httpResponse = $this->httpClient->request(
                    $httpRequestHost,
                    $port,
                    $scheme,
                    $httpRequestPath,
                    'GET',
                    $this->options->timeout,
                    $this->finalUserAgent,
                    $this->acceptHeader,
                    $this->options->acceptEncoding,
                    $setOrigin ? $origin : null,
                    $useHttpAuthIfConfigured,
                    $this->getForcedIpForDomainAndPort($httpRequestHost, $port)
                );
            }

            // when the crawler has been terminated in the meantime, do not process response, otherwise output
            // will be corrupted because request table-row will be somewhere in the middle of output of the analyzers
            if ($this->terminated) {
                return;
            }

            $body = $httpResponse->body;
            $status = $httpResponse->statusCode;
            $elapsedTime = $httpResponse->execTime;

            if ($isAssetUrl && isset($httpResponse->headers['content-length'])) {
                $bodySize = (int)$httpResponse->headers['content-length'];
            } else {
                $bodySize = $body ? strlen($body) : 0;
            }

            if ($status !== 200) {
                $this->processNon200Url($parsedUrl);
            }

            // parse HTML body and fill queue with new URLs
            $isHtmlBody = isset($httpResponse->headers['content-type']) && stripos($httpResponse->headers['content-type'], 'text/html') !== false;
            $isCssBody = isset($httpResponse->headers['content-type']) && stripos($httpResponse->headers['content-type'], 'text/css') !== false;
            $isJsBody = isset($httpResponse->headers['content-type']) && (stripos($httpResponse->headers['content-type'], 'application/javascript') !== false || stripos($httpResponse->headers['content-type'], 'text/javascript') !== false);
            $isXmlBody = isset($httpResponse->headers['content-type']) && (stripos($httpResponse->headers['content-type'], 'application/xml') !== false || stripos($httpResponse->headers['content-type'], 'text/xml') !== false);
            $isAllowedForCrawling = $this->isUrlAllowedByRegexes($parsedUrl) && $this->isExternalDomainAllowedForCrawling($parsedUrl->host);
            $extraParsedContent = [];

            // mark initial URL as found if it is HTML and not redirected
            if (!$this->initialExistingUrlFound && $isHtmlBody && $status === 200 && $bodySize > 0) {
                $this->initialExistingUrlFound = true;
            }

            // get type self::URL_TYPE_* based on content-type header
            $contentTypeHeader = $httpResponse->headers['content-type'] ?? '';
            if (isset($httpResponse->headers['location']) && $httpResponse->headers['location']) {
                $contentType = self::CONTENT_TYPE_ID_REDIRECT;
            } else {
                $contentType = $this->getContentTypeIdByContentTypeHeader($contentTypeHeader);
            }

            $this->contentProcessorManager->applyContentChangesBeforeUrlParsing($body, $contentType, $parsedUrl);

            if ($body && $isHtmlBody && $isAllowedForCrawling) {
                $extraParsedContent = $this->parseHtmlBodyAndFillQueue($body, $contentType, $parsedUrl);
            } elseif ($body && ($isJsBody || $isCssBody || $isXmlBody)) {
                $this->parseContentAndFillUrlQueue($body, $contentType, $parsedUrl, $parsedUrlUqId);
            }

            // handle redirect
            if ($status >= 301 && $status <= 308 && isset($httpResponse->headers['location'])) {
                $redirectLocation = $httpResponse->headers['location'];
                if ($redirectLocation) {
                    $extraParsedContent['Location'] = $redirectLocation;
                    $parsedRedirectUrl = $this->addRedirectLocationToQueueIfSuitable($redirectLocation, $parsedUrlUqId, $scheme, $hostAndPort, $parsedUrl);

                    // if the initial URL is redirected to another domain but within the same 2nd level
                    // domain (typically mydomain.tld -> www.mydomain.tld), update the initial url
                    if ($parsedRedirectUrl && !$this->initialExistingUrlFound && $this->initialParsedUrl->domain2ndLevel === $parsedRedirectUrl->domain2ndLevel) {
                        $this->initialParsedUrl = $parsedRedirectUrl;
                    }
                }
            }

            // set extras from headers
            $extraColumns = $this->options->extraColumns;
            foreach ($extraColumns as $extraColumn) {
                $extraColumnNameLowerCase = strtolower($extraColumn->name);
                if (isset($httpResponse->headers[$extraColumnNameLowerCase])) {
                    $extraParsedContent[$extraColumn->name] = $httpResponse->headers[$extraColumnNameLowerCase];
                }
            }

            // caching
            if ($httpResponse->statusCode > 0) {
                $cacheTypeFlags = Utils::getVisitedUrlCacheTypeFlags($httpResponse->headers);
                $cacheLifetime = Utils::getVisitedUrlCacheLifetime($httpResponse->headers);
            } else {
                $cacheTypeFlags = VisitedUrl::CACHE_TYPE_NOT_AVAILABLE;
                $cacheLifetime = null;
            }

            // update info about visited URL
            $isExternal = $parsedUrl->host && $parsedUrl->host !== $this->initialParsedUrl->host;
            $visitedUrl = $this->updateVisitedUrl($parsedUrl, $elapsedTime, $status, $bodySize, $contentType, $body, $httpResponse->headers, $extraParsedContent, $isExternal, $isAllowedForCrawling, $cacheTypeFlags, $cacheLifetime);

            // send message to websocket clients
            if ($this->websocketServerProcess && !$visitedUrl->isExternal && !$visitedUrl->isStaticFile() && !$visitedUrl->looksLikeStaticFileByUrl()) {
                $this->sendWebSocketMessage(json_encode([
                    'type' => 'urlResult',
                    'url' => $absoluteUrl,
                    'statusCode' => $status,
                    'size' => $bodySize,
                    'execTime' => $elapsedTime,
                ], JSON_UNESCAPED_UNICODE));
            }

            // call visited URL callback (it runs analyzers)
            $extraColumnsFromAnalysis = call_user_func($this->visitedUrlCallback, $visitedUrl, $body, $httpResponse->headers);
            if ($extraColumnsFromAnalysis) {
                $extraParsedContent = array_merge($extraParsedContent, $extraColumnsFromAnalysis);
            }

            // print table row to output
            $doneUrlsCount = $this->statusTable->get('1', 'doneUrls');
            $totalUrlsCount = ($this->queue->count() + $this->visited->count());
            $progressStatus = $doneUrlsCount . '/' . $totalUrlsCount;
            $this->output->addTableRow($httpResponse, $absoluteUrl, $status, $elapsedTime, $bodySize, $contentType, $extraParsedContent, $progressStatus, $cacheTypeFlags, $cacheLifetime);

            // decrement workers count after request is done
            $this->statusTable->decr('1', 'workers');

            // check if crawler is done and exit or start new coroutine to process the next URL
            $isDoneByCounts = $totalUrlsCount >= 2 && $doneUrlsCount >= $totalUrlsCount;
            if (($this->queue->count() === 0 && $this->getActiveWorkersNumber() === 0) || $isDoneByCounts) {
                $this->stopWebSocketServer();
                call_user_func($this->doneCallback);
                Coroutine::cancel(Coroutine::getCid());
            } else {
                while ($this->getActiveWorkersNumber() < $this->options->workers && $this->queue->count() > 0) {
                    // rate limiting
                    $currentTimestamp = microtime(true);
                    if (!$httpResponse->isLoadedFromCache() && !$httpResponse->isSkipped() && ($currentTimestamp - $this->lastRequestTime) < $this->optimalDelayBetweenRequests) {
                        $sleep = $this->optimalDelayBetweenRequests - ($currentTimestamp - $this->lastRequestTime);
                        Coroutine::sleep(max($sleep, 0.001));
                        continue;
                    } else {
                        $this->lastRequestTime = $currentTimestamp;
                    }

                    if (Coroutine::create([$this, 'processNextUrl']) === false) {
                        $message = "ERROR: Unable to create coroutine for next URL.";
                        $this->output->addError($message);
                        throw new \Exception($message);
                    }
                }
            }
        } catch (\Exception $e) {
            $this->stopWebSocketServer();
            throw $e;
        }
    }

    private function isUrlSuitableForQueue(ParsedUrl $url): bool
    {
        static $regexForHtmlExtensions = null;
        if (!$regexForHtmlExtensions) {
            $regexForHtmlExtensions = '/\.(' . implode('|', HtmlProcessor::$htmlPagesExtensions) . ')/i';
        }

        if (!$this->isUrlAllowedByRegexes($url)) {
            return false;
        }

        if (($this->visited->count() + $this->queue->count()) >= $this->options->maxVisitedUrls) {
            return false;
        }

        $fullUrl = $url->getFullUrl(true, false);
        $urlKey = $this->getUrlKeyForSwooleTable($url);

        $isInQueue = $this->queue->exist($urlKey);
        $isAlreadyVisited = $this->visited->exist($urlKey);
        $isUrlWithHtml = !$url->extension || preg_match($regexForHtmlExtensions, $url->path) === 1;
        $isUrlWithSitemap = stripos($url->path, 'sitemap') !== false && str_ends_with($url->path, '.xml');
        $isUrlTooLong = strlen($fullUrl) > $this->options->maxUrlLength;
        $allowedAreOnlyHtmlFiles = $this->options->crawlOnlyHtmlFiles();

        if (!$isInQueue && !$isAlreadyVisited && !$isUrlTooLong) {
            if ($isUrlWithHtml || !$allowedAreOnlyHtmlFiles) {
                return true;
            } elseif ($isUrlWithSitemap) {
                return true;
            }
        }

        return false;
    }


    /**
     * Add URL returned as redirect location to queue if is suitable
     *
     * @param string $redirectLocation
     * @param string|null $sourceUqId
     * @param string $scheme
     * @param string $hostAndPort
     * @param ParsedUrl $sourceUrl
     * @return ParsedUrl|null
     * @throws Exception
     */
    private function addRedirectLocationToQueueIfSuitable(string $redirectLocation, ?string $sourceUqId, string $scheme, string $hostAndPort, ParsedUrl $sourceUrl): ?ParsedUrl
    {
        if (str_starts_with($redirectLocation, '//')) {
            $redirectUrlToQueue = $scheme . ':' . $redirectLocation;
        } elseif (str_starts_with($redirectLocation, '/')) {
            $redirectUrlToQueue = $scheme . '://' . $hostAndPort . $redirectLocation;
        } elseif (str_starts_with($redirectLocation, 'http://') || str_starts_with($redirectLocation, 'https://')) {
            $redirectUrlToQueue = $redirectLocation;
        } else {
            $redirectUrlToQueue = $scheme . '://' . $hostAndPort . $sourceUrl->path . '/' . $redirectLocation;
        }

        $parsedRedirectUrl = ParsedUrl::parse($redirectUrlToQueue, $sourceUrl);

        if ($this->isUrlSuitableForQueue($parsedRedirectUrl)) {
            $this->addUrlToQueue($parsedRedirectUrl, $sourceUqId, FoundUrl::SOURCE_REDIRECT);
            return $parsedRedirectUrl;
        }

        return null;
    }

    private function isUrlAllowedByRegexes(ParsedUrl $url): bool
    {
        // bypass regex filtering for static files if --regex-filtering-only-for-pages is set
        if ($this->options->regexFilteringOnlyForPages && $url->isStaticFile()) {
            return true;
        }

        $isAllowed = $this->options->includeRegex === [];
        foreach ($this->options->includeRegex as $includeRegex) {
            if (preg_match($includeRegex, $url->getFullUrl()) === 1) {
                $isAllowed = true;
                break;
            }
        }
        foreach ($this->options->ignoreRegex as $ignoreRegex) {
            if (preg_match($ignoreRegex, $url->getFullUrl()) === 1) {
                $isAllowed = false;
                break;
            }
        }
        return $isAllowed;
    }

    /**
     * @param ParsedUrl $url
     * @param string|null $sourceUqId
     * @param int|null $sourceAttr
     * @return void
     * @throws Exception
     */
    private function addUrlToQueue(ParsedUrl $url, ?string $sourceUqId = null, ?int $sourceAttr = null): void
    {
        // if all URLs are done, do not add new URLs to queue (this is just infinite-loop protection
        // due to edge-case when last coroutine parsed some new URLs, but queue processing is already stopped)
        if ($this->isProcessingDoneByCounts()) {
            return;
        }

        $urlStr = $url->getFullUrl(true, false);
        if (!$this->queue->set($this->getUrlKeyForSwooleTable($url), [
            'url' => $urlStr,
            'uqId' => $this->getUrlUqId($url),
            'sourceUqId' => $sourceUqId,
            'sourceAttr' => $sourceAttr
        ])) {
            $error = "ERROR: Unable to queue URL '{$urlStr}'. Set higher --max-queue-length.";
            $this->output->addError($error);
            throw new Exception($error);
        }
    }

    /**
     * @param ParsedUrl $url
     * @param string $uqId
     * @param string $sourceUqId
     * @param int|null $sourceAttr
     * @return void
     * @throws Exception
     */
    private function addUrlToVisited(ParsedUrl $url, string $uqId, string $sourceUqId, ?int $sourceAttr): void
    {
        $urlStr = $url->getFullUrl(true, false);
        if (!$this->visited->set($this->getUrlKeyForSwooleTable($url), [
            'url' => $urlStr,
            'uqId' => $uqId,
            'sourceUqId' => $sourceUqId,
            'sourceAttr' => $sourceAttr,
        ])) {
            $error = "ERROR: Unable to add visited URL '{$urlStr}'. Set higher --max-visited-urls or --max-url-length.";
            $this->output->addError($error);
            throw new Exception($error);
        }
    }

    /**
     * @param ParsedUrl $url
     * @param int $reason See self::SKIPPED_REASON_*
     * @param string $sourceUqId
     * @param int|null $sourceAttr
     * @return void
     * @throws Exception
     */
    public function addUrlToSkipped(ParsedUrl $url, int $reason, string $sourceUqId, ?int $sourceAttr): void
    {
        $urlStr = $url->getFullUrl(true, false);
        $uqId = $this->getUrlUqId($url);

        $urlKey = $this->getUrlKeyForSwooleTable($url);

        // if URL is already in skipped table, do not add it again (first reason & source is always used)
        $exists = $this->skipped->get($urlKey);
        if ($exists) {
            return;
        }

        if (!$this->skipped->set($urlKey, [
            'url' => $urlStr,
            'reason' => $reason,
            'uqId' => $uqId,
            'sourceUqId' => $sourceUqId,
            'sourceAttr' => $sourceAttr,
        ])) {
            $error = "ERROR: Unable to add skipped URL '{$urlStr}'. Set higher --max-skipped-urls or --max-url-length.";
            $this->output->addError($error);
            throw new Exception($error);
        }

        $this->statusTable->incr('1', 'skippedUrls');
    }

    /**
     * @param ParsedUrl $url
     * @param float $elapsedTime
     * @param int $status
     * @param int $size
     * @param int $type @see self::URL_TYPE_*
     * @param string|null $body
     * @param array|null $headers
     * @param array|null $extras
     * @param bool $isExternal
     * @param bool $isAllowedForCrawling
     * @param int $cacheType
     * @param int|null $cacheLifetime
     * @return VisitedUrl
     * @throws Exception
     */
    private function updateVisitedUrl(ParsedUrl $url, float $elapsedTime, int $status, int $size, int $type, ?string $body, ?array $headers, ?array $extras, bool $isExternal, bool $isAllowedForCrawling, int $cacheType, ?int $cacheLifetime): VisitedUrl
    {
        $urlKey = $this->getUrlKeyForSwooleTable($url);
        $visitedUrlInTable = $this->visited->get($urlKey);
        if (!$visitedUrlInTable) {
            throw new Exception("ERROR: Unable to handle visited URL '{$url->getFullUrl(true, false)}'. Set higher --max-visited-urls or --max-url-length.");
        }
        $visitedUrlInTable['time'] = $elapsedTime;
        $visitedUrlInTable['status'] = $status;
        $visitedUrlInTable['size'] = $size;
        $visitedUrlInTable['type'] = $type;
        $visitedUrlInTable['cacheType'] = $cacheType;
        $visitedUrlInTable['cacheLifetime'] = $cacheLifetime;
        $this->visited->set($urlKey, $visitedUrlInTable);

        $this->statusTable->incr('1', 'doneUrls');

        $visitedUrl = new VisitedUrl(
            $visitedUrlInTable['uqId'],
            $visitedUrlInTable['sourceUqId'],
            $visitedUrlInTable['sourceAttr'],
            $visitedUrlInTable['url'],
            $visitedUrlInTable['status'],
            $visitedUrlInTable['time'],
            $visitedUrlInTable['size'],
            $visitedUrlInTable['type'],
            $headers['content-type'] ?? null,
            $headers['content-encoding'] ?? null,
            $extras,
            $isExternal,
            $isAllowedForCrawling,
            $visitedUrlInTable['cacheType'],
            $visitedUrlInTable['cacheLifetime']
        );

        $this->status->addVisitedUrl($visitedUrl, $body, $headers);
        return $visitedUrl;
    }

    private function getUrlKeyForSwooleTable(ParsedUrl $url): string
    {
        $relevantParts = $url->getFullUrl(true, false);
        return md5($relevantParts);
    }

    private function getActiveWorkersNumber(): int
    {
        return $this->statusTable->get('1', 'workers');
    }

    public function getUrlUqId(ParsedUrl $url): string
    {
        return substr(md5($url->getFullUrl(true, false)), 0, 8);
    }

    private function getContentTypeIdByContentTypeHeader(string $contentTypeHeader): int
    {
        static $cache = [];
        if (array_key_exists($contentTypeHeader, $cache)) {
            return $cache[$contentTypeHeader];
        }

        $typeId = self::CONTENT_TYPE_ID_OTHER;
        if (str_contains($contentTypeHeader, 'text/html')) {
            $typeId = self::CONTENT_TYPE_ID_HTML;
        } elseif (str_contains($contentTypeHeader, 'text/javascript') || str_contains($contentTypeHeader, 'application/javascript')
            || str_contains($contentTypeHeader, 'application/x-javascript')) {
            $typeId = self::CONTENT_TYPE_ID_SCRIPT;
        } elseif (str_contains($contentTypeHeader, 'text/css')) {
            $typeId = self::CONTENT_TYPE_ID_STYLESHEET;
        } elseif (str_contains($contentTypeHeader, 'image/')) {
            $typeId = self::CONTENT_TYPE_ID_IMAGE;
        } elseif (str_contains($contentTypeHeader, 'audio/')) {
            $typeId = self::CONTENT_TYPE_ID_AUDIO;
        } elseif (str_contains($contentTypeHeader, 'video/')) {
            $typeId = self::CONTENT_TYPE_ID_VIDEO;
        } elseif (str_contains($contentTypeHeader, 'font/')) {
            $typeId = self::CONTENT_TYPE_ID_FONT;
        } elseif (str_contains($contentTypeHeader, 'application/json')) {
            $typeId = self::CONTENT_TYPE_ID_JSON;
        } elseif (str_contains($contentTypeHeader, 'application/xml') || str_contains($contentTypeHeader, 'text/xml') || str_contains($contentTypeHeader, '+xml')) {
            $typeId = self::CONTENT_TYPE_ID_XML;
        } elseif (str_contains($contentTypeHeader, 'application/pdf') || str_contains($contentTypeHeader, 'application/msword')
            || str_contains($contentTypeHeader, 'application/vnd.ms-excel') || str_contains($contentTypeHeader, 'application/vnd.ms-powerpoint')
            || str_contains($contentTypeHeader, 'text/plain') || str_contains($contentTypeHeader, 'document')) {
            $typeId = self::CONTENT_TYPE_ID_DOCUMENT;
        }

        $cache[$contentTypeHeader] = $typeId;
        return $typeId;
    }

    /**
     * Get final user agent string with respect to options
     *
     * @return string
     * @throws Exception
     */
    public function getFinalUserAgent(): string
    {
        if ($this->options->userAgent) {
            $result = $this->options->userAgent;
        } else {
            $result = match ($this->options->device) {
                DeviceType::DESKTOP => 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/' . date('y') . '.0.0.0 Safari/537.36',
                DeviceType::MOBILE => 'Mozilla/5.0 (iPhone; CPU iPhone OS 15_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.0 Mobile/15A5370a Safari/604.1',
                DeviceType::TABLET => 'Mozilla/5.0 (Linux; Android 11; SAMSUNG SM-T875) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/14.0 Chrome/87.0.4280.141 Safari/537.36',
            };
        }

        // add siteone-crawler signature only if user-agent not ends with '!'
        $addSignature = !str_ends_with($result, '!');
        if ($addSignature) {
            // WARNING: Please do not remove this signature, it's used to detect crawler
            // in logs and also for possibility to block our crawler by website owner
            return $result . ' ' . self::getCrawlerUserAgentSignature();
        } else {
            // remove trailing spaces and '!'
            return rtrim($result, '! ');
        }
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
     * Apply URL transformations for HTTP request based on --transform-url options
     * This transforms the host and potentially the path for the actual HTTP request
     * @param string $host
     * @param string $path
     * @return array{host: string, path: string}
     */
    public function applyHttpRequestTransformations(string $host, string $path): array
    {
        if (empty($this->options->transformUrl)) {
            return ['host' => $host, 'path' => $path];
        }
        
        // Reconstruct full URL for transformation
        $fullUrl = $host . $path;
        $originalUrl = $fullUrl;
        
        foreach ($this->options->transformUrl as $transform) {
            $parts = explode('->', $transform);
            if (count($parts) !== 2) {
                continue;
            }
            
            $from = trim($parts[0]);
            $to = trim($parts[1]);
            
            // Check if it's a regex pattern
            if (preg_match('/^([\/#~%]).*\1[a-z]*$/i', $from)) {
                // Regex replacement
                $newUrl = @preg_replace($from, $to, $fullUrl);
                if ($newUrl !== null) {
                    $fullUrl = $newUrl;
                }
            } else {
                // Simple string replacement
                $fullUrl = str_replace($from, $to, $fullUrl);
            }
        }
        
        // Debug output if URL was transformed
        if ($fullUrl !== $originalUrl) {
            Debugger::debug('http-request-transformation', "Transformed HTTP request: '{$originalUrl}' -> '{$fullUrl}'");
            $this->output->addNotice("HTTP request transformed: '{$originalUrl}' -> '{$fullUrl}'");
        }
        
        // Parse transformed URL back to host and path
        $parsedUrl = parse_url('http://' . $fullUrl);
        $newHost = $parsedUrl['host'] ?? $host;
        $newPath = ($parsedUrl['path'] ?? '/') . (isset($parsedUrl['query']) ? '?' . $parsedUrl['query'] : '');
        
        return ['host' => $newHost, 'path' => $newPath];
    }

    /**
     * @return int[]
     */
    public static function getContentTypeIds(): array
    {
        return [
            self::CONTENT_TYPE_ID_HTML,
            self::CONTENT_TYPE_ID_IMAGE,
            self::CONTENT_TYPE_ID_SCRIPT,
            self::CONTENT_TYPE_ID_STYLESHEET,
            self::CONTENT_TYPE_ID_FONT,
            self::CONTENT_TYPE_ID_DOCUMENT,
            self::CONTENT_TYPE_ID_AUDIO,
            self::CONTENT_TYPE_ID_VIDEO,
            self::CONTENT_TYPE_ID_JSON,
            self::CONTENT_TYPE_ID_XML,
            self::CONTENT_TYPE_ID_REDIRECT,
            self::CONTENT_TYPE_ID_OTHER,
        ];
    }

    /**
     * @param FoundUrls $foundUrls
     * @param ParsedUrl $sourceUrl
     * @param string $sourceUrlUqId
     * @return void
     * @throws Exception
     */
    private function addSuitableUrlsToQueue(FoundUrls $foundUrls, ParsedUrl $sourceUrl, string $sourceUrlUqId): void
    {
        foreach ($foundUrls->getUrls() as $foundUrl) {
            $urlForQueue = trim($foundUrl->url);
            $origUrlForQueue = $urlForQueue;
            $isUrlForDebug = $this->options->isUrlSelectedForDebug($origUrlForQueue);
            $parsedUrlForQueue = ParsedUrl::parse(trim($urlForQueue), $sourceUrl);

            // skip URLs that are not on the same host, allowed domain or are not real resource URL (data:, mailto:, etc.
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

            // skip URLs with basename that exceeded max occurrences in non-200 URLs
            $baseName = $parsedUrlForQueue->getBaseName();
            if ($baseName && isset($this->non200BasenamesToOccurrences[$baseName]) && $this->non200BasenamesToOccurrences[$baseName] >= $this->options->maxNon200ResponsesPerBasename) {
                if ($this->non200BasenamesToOccurrences[$baseName] === $this->options->maxNon200ResponsesPerBasename) {
                    $msg = "URL '{$urlForQueue}' ignored because there are too many (>= " . $this->options->maxNon200ResponsesPerBasename . ") non-200 URLs with same basename.";
                    $this->output->addNotice($msg);
                    $this->status->addNoticeToSummary('non-200-occurrences-for-basenames', $msg);
                    $isUrlForDebug && Debugger::debug('ignored-url_too-many-non-200-urls-with-same-basename', $msg);
                }
                $this->non200BasenamesToOccurrences[$baseName]++;
                continue;
            }

            if (!$isRequestableResource) {
                $isUrlForDebug && Debugger::debug('ignored-url_not-resource', "URL '{$urlForQueue}' ignored because it's not requestable resource.");
                continue;
            } elseif (!$isUrlOnSameHost && !$isUrlOnAllowedHost) {
                $isUrlForDebug && Debugger::debug('ignored-url_not-allowed-host', "URL '{$urlForQueue}' ignored because it's not on allowed host.");
                $this->addUrlToSkipped($parsedUrlForQueue, self::SKIPPED_REASON_NOT_ALLOWED_HOST, $sourceUrlUqId, $foundUrl->source);
                continue;
            } elseif (!$parsedUrlForQueue->isStaticFile() && !self::isUrlAllowedByRobotsTxt(
                    $parsedUrlForQueue->host ?: $this->initialParsedUrl->host,
                    $urlForQueue,
                    $this->options->proxy,
                    $this->options->httpAuth,
                    $this,
                    $parsedUrlForQueue->port ?: $this->initialParsedUrl->port)
            ) {
                $isUrlForDebug && Debugger::debug('ignored-url_blocked-by-robots-txt', "URL '{$urlForQueue}' ignored because is blocked by website's robots.txt.");
                $this->addUrlToSkipped($parsedUrlForQueue, self::SKIPPED_REASON_ROBOTS_TXT, $sourceUrlUqId, $foundUrl->source);
                continue;
            }

            // build URL for queue
            $urlForQueue = Utils::getAbsoluteUrlByBaseUrl($sourceUrl->getFullUrl(), $urlForQueue);

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
            $parsedUrlForQueue = ParsedUrl::parse($urlForQueue, $sourceUrl);
            if ($this->isUrlSuitableForQueue($parsedUrlForQueue)) {
                $this->addUrlToQueue($parsedUrlForQueue, $sourceUrlUqId, $foundUrl->source);
            }
        }
    }

    /**
     * Process URL that do not return status code 200 OK (redirects, 404, 500, etc.)
     * @param ParsedUrl $url
     * @return void
     */
    private function processNon200Url(ParsedUrl $url): void
    {
        $baseName = $url->getBaseName();
        if ($baseName && $baseName !== 'index.html' && $baseName !== 'index.htm' && $baseName !== 'index') {
            $this->non200BasenamesToOccurrences[$baseName] = ($this->non200BasenamesToOccurrences[$baseName] ?? 0) + 1;
        }
    }

    /**
     * Remove AVIF and WebP support from Accept header
     *
     * @return void
     */
    public function removeAvifAndWebpSupportFromAcceptHeader(): void
    {
        $this->acceptHeader = str_replace(['image/avif', 'image/webp'], ['', ''], $this->acceptHeader);
    }

    /**
     * Terminate crawler and ignore all URLs in queue or request processing
     * @return void
     */
    public function terminate(): void
    {
        $this->terminated = true;
    }

    /**
     * Checks if URL is allowed by robots.txt of given domain. It respects all Disallow rules and User-Agent or Allow rules are ignored
     * Has internal static cache for disallowed paths to minimize requests to robots.txt
     *
     * @param string $domain
     * @param string $url
     * @param string|null $proxy
     * @param string|null $httpAuth
     * @param Crawler $crawler
     * @param int|null $extraPort
     * @return bool
     */
    public static function isUrlAllowedByRobotsTxt(string $domain, string $url, ?string $proxy, ?string $httpAuth, Crawler $crawler, ?int $extraPort = null): bool
    {
        if ($crawler->getCoreOptions()->ignoreRobotsTxt) {
            return true;
        }

        // when URL is for frontend asset (internal or external), we can assume that it's allowed
        if (preg_match('/\.(js|css|json|eot|ttf|woff2|woff|otf|png|gif|jpg|jpeg|ico|webp|avif|tif|bmp|svg)/i', $url) === 1) {
            return true;
        }

        static $disallowedPathsPerDomain = [];
        $disallowedPaths = null;

        $cacheKey = $domain . ($extraPort ? ':' . $extraPort : '');

        // if we are crawling the same domain of 2nd level (regardless of subdomains) or exactly the same domain/IP,
        // we can use HTTP auth if configured
        $useHttpAuthIfConfigured = $crawler->getInitialParsedUrl()->domain2ndLevel
            ? str_ends_with($domain, $crawler->getInitialParsedUrl()->domain2ndLevel)
            : ($domain === $crawler->getInitialParsedUrl()->host);

        if (array_key_exists($cacheKey, $disallowedPathsPerDomain)) {
            $disallowedPaths = $disallowedPathsPerDomain[$cacheKey];
        } else {
            $disallowedPathsPerDomain[$cacheKey] = []; // prevent multiple parallel requests to robots.txt for same domain
            $ports = $extraPort ? [$extraPort] : [443, 80];
            foreach ($ports as $port) {
                $httpClient = new HttpClient($proxy, $httpAuth, null);
                
                // Apply URL transformations for robots.txt request
                $transformedRequest = $crawler->applyHttpRequestTransformations($domain, '/robots.txt');
                $robotsTxtHost = $transformedRequest['host'];
                $robotsTxtPath = $transformedRequest['path'];
                
                $robotsTxtResponse = $httpClient->request(
                    $robotsTxtHost,
                    $port,
                    $port === 443 ? 'https' : 'http', // warning: this will not work for HTTPS with non-standard port
                    $robotsTxtPath,
                    'GET',
                    3,
                    self::getCrawlerUserAgentSignature(),
                    'text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7',
                    'gzip, deflate, br',
                    null,
                    $useHttpAuthIfConfigured,
                    $crawler->getForcedIpForDomainAndPort($robotsTxtHost, $port)
                );
                self::$loadedRobotsTxtCount++;

                $maxReportedRobotsTxt = 10;
                if (self::$loadedRobotsTxtCount <= $maxReportedRobotsTxt) {
                    $crawler->getStatus()->addNoticeToSummary('robots-txt-' . $domain, sprintf(
                        "Loaded robots.txt for domain '%s': status code %d, size %s and took %s.",
                        $domain,
                        $robotsTxtResponse->statusCode,
                        Utils::getFormattedSize(strlen($robotsTxtResponse->body ?: '')),
                        Utils::getFormattedDuration($robotsTxtResponse->execTime)
                    ));
                } elseif (self::$loadedRobotsTxtCount === ($maxReportedRobotsTxt + 1)) {
                    $crawler->getStatus()->addNoticeToSummary(
                        'robots-txt-limited',
                        'The limit of the number of loaded robots.txt (' . $maxReportedRobotsTxt . ') has been exceeded. Other robots.txt will not be in this summary for the sake of clarity.'
                    );
                }

                if ($robotsTxtResponse->statusCode === 200 && $robotsTxtResponse->body) {
                    $robotsTxt = $robotsTxtResponse->body;
                    $lines = explode("\n", $robotsTxt);
                    $disallowedPaths = [];
                    $currentUserAgent = null;

                    foreach ($lines as $line) {
                        $line = trim(preg_replace('/#.*/', '', $line)); // remove comments

                        if (preg_match('/^User-agent:\s*(.*)/i', $line, $matches)) {
                            $currentUserAgent = trim($matches[1]);
                        } elseif (($currentUserAgent === '*' || $currentUserAgent === 'SiteOne-Crawler') &&
                            preg_match('/^Disallow:\s*(.*)/i', $line, $matches)) {
                            if (trim($matches[1]) !== '') {
                                $disallowedPaths[] = trim($matches[1]);
                            }
                        }
                    }

                    Status::setRobotsTxtContent($port === 443 ? 'https' : 'http', $domain, $port, $robotsTxt);

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
    public static function getCrawlerUserAgentSignature(): string
    {
        // WARNING: Please do not change or remove this signature, it's used to detect crawler
        // in logs and also for possibility to block our crawler by website owner

        return 'siteone-crawler/' . Version::CODE;
    }

    /**
     * Basic coroutines setup
     * @return void
     */
    private function setupCoroutines(): void
    {
        $options = [
            'max_concurrency' => $this->options->workers,
            'max_coroutine' => 8192,
            'stack_size' => 2 * 1024 * 1024,
            'socket_connect_timeout' => $this->options->timeout + 1,
            'socket_timeout' => $this->options->timeout + 2,
            'socket_read_timeout' => -1,
            'socket_write_timeout' => -1,
            'log_level' => SWOOLE_LOG_INFO,
            'hook_flags' => SWOOLE_HOOK_ALL,
            'trace_flags' => SWOOLE_TRACE_ALL,
            'dns_cache_expire' => 60,
            'dns_cache_capacity' => 1000,
            'dns_server' => '8.8.8.8',
            'display_errors' => false,
            'aio_core_worker_num' => $this->options->workers + 1,
            'aio_worker_num' => $this->options->workers + 1,
            'aio_max_wait_time' => 1,
            'aio_max_idle_time' => 1,
            'exit_condition' => function () {
                return Coroutine::stats()['coroutine_num'] === 0;
            },
        ];

        Coroutine::set($options);
    }

    /**
     * Is processing already done by counts of URLs in queues/visited tables?
     * @return bool
     */
    private function isProcessingDoneByCounts(): bool
    {
        $doneUrlsCount = $this->statusTable->get('1', 'doneUrls');
        $totalUrlsCount = ($this->queue->count() + $this->visited->count());
        return $totalUrlsCount >= 2 && $doneUrlsCount >= $totalUrlsCount;
    }

    /**
     * @return void
     * @throws Exception
     */
    private function registerContentProcessors(): void
    {
        $this->contentProcessorManager = new ContentProcessorManager();
        $this->contentProcessorManager->registerProcessor(new AstroProcessor($this));
        $this->contentProcessorManager->registerProcessor(new HtmlProcessor($this));
        $this->contentProcessorManager->registerProcessor(new JavaScriptProcessor($this));
        $this->contentProcessorManager->registerProcessor(new CssProcessor($this));
        $this->contentProcessorManager->registerProcessor(new XmlProcessor($this));
        $this->contentProcessorManager->registerProcessor(new NextJsProcessor($this));
        $this->contentProcessorManager->registerProcessor(new SvelteProcessor($this));
    }

    public function getContentProcessorManager(): ContentProcessorManager
    {
        return $this->contentProcessorManager;
    }

    public function getInitialParsedUrl(): ParsedUrl
    {
        return $this->initialParsedUrl;
    }

    public function getStatus(): Status
    {
        return $this->status;
    }

    /**
     * Get IP address for domain and port if it's forced by --resolve option
     *
     * @param string $domain
     * @param int $port
     * @return string|null
     * @throws Exception
     */
    public function getForcedIpForDomainAndPort(string $domain, int $port): ?string
    {
        if (!$this->options->resolve) {
            return null;
        }

        static $domainPortToIpCache = null;
        if ($domainPortToIpCache === null) {
            $domainPortToIpCache = [];
            foreach ($this->options->resolve as $resolve) {
                if (preg_match('/^([^:]+):([0-9]+):(.+)$/', $resolve, $matches) === 1) {
                    $domainPortToIpCache[$matches[1] . ':' . $matches[2]] = $matches[3];
                } else {
                    throw new Exception("Invalid --resolve option value: '{$resolve}'. Expected format: 'domain:port:ip'.");
                }
            }
        }

        return $domainPortToIpCache[$domain . ':' . $port] ?? null;
    }

    /**
     * @return Table
     */
    public function getSkippedUrls(): Table
    {
        return $this->skipped;
    }

}