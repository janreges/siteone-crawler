<?php

/*
 * This file is part of the SiteOne Website Crawler.
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
use Crawler\Export\MailerExporter;
use Crawler\HttpClient\HttpClient;
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

    private ContentProcessorManager $contentProcessorManager;

    private ParsedUrl $initialParsedUrl;
    private string $finalUserAgent;
    private ?array $doneCallback = null;
    private ?array $visitedUrlCallback = null;
    private bool $terminated = false;

    // rate limiting
    private ?float $optimalDelayBetweenRequests;
    private float $lastRequestTime = 0;

    private string $acceptHeader = 'text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7';
    private static int $loadedRobotsTxtCount = 0;

    // websocket server & client to send messages through it
    private ?Process $websocketServerProcess = null;
    private ?Coroutine\Client $websocketClient = null;

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
        $this->statusTable->column('workers', Table::TYPE_INT, 2);
        $this->statusTable->column('doneUrls', Table::TYPE_INT, 8);
        $this->statusTable->create();
        $this->statusTable->set('1', ['workers' => 0, 'doneUrls' => 0]);

        $this->queue = new Table($this->options->maxQueueLength);
        $this->queue->column('url', Table::TYPE_STRING, $this->options->maxUrlLength);
        $this->queue->column('uqId', Table::TYPE_STRING, 8);
        $this->queue->column('sourceUqId', Table::TYPE_STRING, 8);
        $this->queue->create();

        $this->visited = new Table(intval($this->options->maxVisitedUrls * 1.33));
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
        $this->addUrlToQueue($this->initialParsedUrl);

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
        $result['Title'] = trim($matches[1][0] ?? '');

        preg_match_all('/<meta\s+[^>]*name=["\']description["\']\s+[^>]*content=["\']([^"\']+)["\'][^>]*>/i', $body, $matches);
        $result['Description'] = trim($matches[1][0] ?? '');

        if ($this->options->hasHeaderToTable('Keywords')) {
            preg_match_all('/<meta\s+[^>]*name=["\']keywords["\']\s+[^>]*content=["\']([^"\']+)["\'][^>]*>/i', $body, $matches);
            $result['Keywords'] = trim($matches[1][0] ?? '');
        }

        if ($this->options->hasHeaderToTable('DOM')) {
            @preg_match_all('/<\w+/', $body, $matches);
            $dom = count($matches[0] ?? []);
            $result['DOM'] = $dom;
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
                $this->addUrlToVisited(ParsedUrl::parse($url), $queuedUrl['uqId'], $sourceUqId);
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

            // setup HTTP client, send request and get response
            $httpResponse = $this->httpClient->request(
                $parsedUrl->host,
                $parsedUrl->port ?: ($scheme === 'https' ? 443 : 80),
                $scheme,
                $finalUrlForHttpClient,
                'GET',
                $this->options->timeout,
                $this->finalUserAgent,
                $this->acceptHeader,
                $this->options->acceptEncoding,
                $origin
            );

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

            // decrement workers count after request is done
            $this->statusTable->decr('1', 'workers');

            // parse HTML body and fill queue with new URLs
            $isHtmlBody = isset($httpResponse->headers['content-type']) && stripos($httpResponse->headers['content-type'], 'text/html') !== false;
            $isCssBody = isset($httpResponse->headers['content-type']) && stripos($httpResponse->headers['content-type'], 'text/css') !== false;
            $isJsBody = isset($httpResponse->headers['content-type']) && (stripos($httpResponse->headers['content-type'], 'application/javascript') !== false || stripos($httpResponse->headers['content-type'], 'text/javascript') !== false);
            $isAllowedForCrawling = $this->isUrlAllowedByRegexes($parsedUrl) && $this->isExternalDomainAllowedForCrawling($parsedUrl->host);
            $extraParsedContent = [];

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
            } elseif ($body && ($isJsBody || $isCssBody)) {
                $this->parseContentAndFillUrlQueue($body, $contentType, $parsedUrl, $parsedUrlUqId);
            }

            // handle redirect
            if ($status >= 301 && $status <= 308 && isset($httpResponse->headers['location'])) {
                $redirectLocation = $httpResponse->headers['location'];
                if ($redirectLocation) {
                    $extraParsedContent['Location'] = $redirectLocation;
                    $this->addRedirectLocationToQueueIfSuitable($redirectLocation, $parsedUrlUqId, $scheme, $hostAndPort, $parsedUrl);
                }
            }

            // update info about visited URL
            $isExternal = $parsedUrl->host && $parsedUrl->host !== $this->initialParsedUrl->host;
            $visitedUrl = $this->updateVisitedUrl($parsedUrl, $elapsedTime, $status, $bodySize, $contentType, $body, $httpResponse->headers, $extraParsedContent, $isExternal, $isAllowedForCrawling);

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
            $progressStatus = $this->statusTable->get('1', 'doneUrls') . '/' . ($this->queue->count() + $this->visited->count());
            $this->output->addTableRow($httpResponse, $absoluteUrl, $status, $elapsedTime, $bodySize, $contentType, $extraParsedContent, $progressStatus);

            // check if crawler is done and exit or start new coroutine to process the next URL
            if ($this->queue->count() === 0 && $this->getActiveWorkersNumber() === 0) {
                $this->stopWebSocketServer();
                call_user_func($this->doneCallback);
                Coroutine::cancel(Coroutine::getCid());
            } else {
                while ($this->getActiveWorkersNumber() < $this->options->workers && $this->queue->count() > 0) {
                    // rate limiting
                    $currentTimestamp = microtime(true);
                    if (!$httpResponse->isLoadedFromCache() && ($currentTimestamp - $this->lastRequestTime) < $this->optimalDelayBetweenRequests) {
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
        } catch(\Exception $e) {
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
        $isUrlTooLong = strlen($fullUrl) > $this->options->maxUrlLength;
        $allowedAreOnlyHtmlFiles = $this->options->crawlOnlyHtmlFiles();

        return !$isInQueue && !$isAlreadyVisited && !$isUrlTooLong && ($isUrlWithHtml || !$allowedAreOnlyHtmlFiles);
    }


    /**
     * Add URL returned as redirect location to queue if is suitable
     *
     * @param string $redirectLocation
     * @param string|null $sourceUqId
     * @param string $scheme
     * @param string $hostAndPort
     * @param ParsedUrl $sourceUrl
     * @return void
     * @throws Exception
     */
    private function addRedirectLocationToQueueIfSuitable(string $redirectLocation, ?string $sourceUqId, string $scheme, string $hostAndPort, ParsedUrl $sourceUrl): void
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
            $this->addUrlToQueue($parsedRedirectUrl, $sourceUqId);
        }
    }

    private function isUrlAllowedByRegexes(ParsedUrl $url): bool
    {
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
     * @return void
     * @throws Exception
     */
    private function addUrlToQueue(ParsedUrl $url, ?string $sourceUqId = null): void
    {
        $urlStr = $url->getFullUrl(true, false);
        if (!$this->queue->set($this->getUrlKeyForSwooleTable($url), [
            'url' => $urlStr,
            'uqId' => $this->getUrlUqId($url),
            'sourceUqId' => $sourceUqId,
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
     * @return void
     * @throws Exception
     */
    private function addUrlToVisited(ParsedUrl $url, string $uqId, string $sourceUqId): void
    {
        $urlStr = $url->getFullUrl(true, false);
        if (!$this->visited->set($this->getUrlKeyForSwooleTable($url), ['url' => $urlStr, 'uqId' => $uqId, 'sourceUqId' => $sourceUqId])) {
            $error = "ERROR: Unable to add visited URL '{$urlStr}'. Set higher --max-visited-urls or --max-url-length.";
            $this->output->addError($error);
            throw new Exception($error);
        }
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
     * @return VisitedUrl
     * @throws Exception
     */
    private function updateVisitedUrl(ParsedUrl $url, float $elapsedTime, int $status, int $size, int $type, ?string $body, ?array $headers, ?array $extras, bool $isExternal, bool $isAllowedForCrawling): VisitedUrl
    {
        $urlKey = $this->getUrlKeyForSwooleTable($url);
        $visitedUrl = $this->visited->get($urlKey);
        if (!$visitedUrl) {
            throw new Exception("ERROR: Unable to handle visited URL '{$url->getFullUrl(true, false)}'. Set higher --max-visited-urls or --max-url-length.");
        }
        $visitedUrl['time'] = $elapsedTime;
        $visitedUrl['status'] = $status;
        $visitedUrl['size'] = $size;
        $visitedUrl['type'] = $type;
        $this->visited->set($urlKey, $visitedUrl);

        $this->statusTable->incr('1', 'doneUrls');

        $visitedUrl = new VisitedUrl(
            $visitedUrl['uqId'],
            $visitedUrl['sourceUqId'],
            $visitedUrl['url'],
            $visitedUrl['status'],
            $visitedUrl['time'],
            $visitedUrl['size'],
            $visitedUrl['type'],
            $headers['content-type'] ?? null,
            $headers['content-encoding'] ?? null,
            $extras,
            $isExternal,
            $isAllowedForCrawling,
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

        // WARNING: Please do not remove this signature, it's used to detect crawler
        // in logs and also for possibility to block our crawler by website owner

        return $result . ' ' . self::getCrawlerUserAgentSignature();
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

            if (!$isRequestableResource) {
                $isUrlForDebug && Debugger::debug('ignored-url_not-resource', "URL '{$urlForQueue}' ignored because it's not requestable resource.");
                continue;
            } elseif (!$isUrlOnSameHost && !$isUrlOnAllowedHost) {
                $isUrlForDebug && Debugger::debug('ignored-url_not-allowed-host', "URL '{$urlForQueue}' ignored because it's not requestable resource.");
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
                $this->addUrlToQueue($parsedUrlForQueue, $sourceUrlUqId);
            }
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
                $httpClient = new HttpClient($proxy, $httpAuth, null);
                $robotsTxtResponse = $httpClient->request(
                    $domain,
                    $port,
                    $port === 443 ? 'https' : 'http', // warning: this will not work for HTTPS with non-standard port
                    '/robots.txt',
                    'GET',
                    1,
                    self::getCrawlerUserAgentSignature(),
                    'text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7',
                    'gzip, deflate, br'
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
                    foreach ($lines as $line) {
                        $line = trim(preg_replace('/#.*/', '', $line)); // remove comments
                        if (preg_match('/^Disallow:\s*(.*)/i', $line, $matches)) {
                            if (trim($matches[1]) !== '') {
                                $disallowedPaths[] = trim($matches[1]);
                            }
                        }
                    }

                    Status::setRobotsTxtContent($robotsTxt);

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

        return 'siteone-website-crawler/' . Version::CODE;
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

}