<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\HttpClient;

use Crawler\Version;
use Exception;
use Swoole\Coroutine\Http\Client;

class HttpClient
{

    /**
     * Proxy server in format "host:port"
     * @var string|null
     */
    private readonly ?string $proxy;

    /**
     * Basic HTTP authentization in format "username:password"
     * @var string|null
     */
    private readonly ?string $httpAuth;

    /**
     * Cache dir for http client. If null, cache is disabled
     * @var string|null
     */
    private readonly ?string $cacheDir;

    /**
     * If true, cache is compressed
     * @var bool $compression
     */
    private readonly bool $compression;

    /**
     * @param string|null $proxy
     * @param string|null $httpAuth
     * @param string|null $cacheDir
     * @param bool $compression
     */
    public function __construct(?string $proxy, ?string $httpAuth, ?string $cacheDir, bool $compression = false)
    {
        $this->proxy = $proxy;
        $this->httpAuth = $httpAuth;
        $this->cacheDir = $cacheDir;
        $this->compression = $compression;
    }

    /**
     * @param string $host
     * @param int $port
     * @param string $scheme
     * @param string $url
     * @param string $httpMethod
     * @param int $timeout
     * @param string $userAgent
     * @param string $accept
     * @param string $acceptEncoding
     * @param ?string $origin
     * @param bool $useHttpAuthIfConfigured
     * @return HttpResponse
     * @throws Exception
     */
    public function request(string $host, int $port, string $scheme, string $url, string $httpMethod, int $timeout, string $userAgent, string $accept, string $acceptEncoding, ?string $origin = null, bool $useHttpAuthIfConfigured = false, ?string $forcedIp = null): HttpResponse
    {
        $path = @parse_url($url, PHP_URL_PATH);
        $extension = is_string($path) ? @pathinfo($path, PATHINFO_EXTENSION) : null;

        $argsForCacheKey = [$host, $port, $scheme, $url, $httpMethod, $userAgent, $accept, $acceptEncoding, $origin];
        $cacheKey = $this->getCacheKey($host, $port, $argsForCacheKey, $extension);
        $cachedResult = $this->getFromCache($cacheKey);
        if ($cachedResult !== null && str_contains($url, ' ') === false) {
            $cachedResult->setLoadedFromCache(true);
            return $cachedResult;
        }

        $requestHeaders = [
            'X-Crawler-Info' => 'siteone-crawler/' . Version::CODE,
            'User-Agent' => $userAgent,
            'Accept' => $accept,
            'Accept-Encoding' => $acceptEncoding,
            'Connection' => 'close',
        ];

        if ($forcedIp) {
            $requestHeaders['Host'] = $host;
        }

        if ($origin) {
            $requestHeaders['Origin'] = $origin;
        }

        $startTime = microtime(true);
        $client = new Client($forcedIp ?: $host, $port, $scheme === 'https');

        if ($this->proxy) {
            list($proxyHost, $proxyPort) = explode(':', $this->proxy);
            $client->set([
                'http_proxy_host' => $proxyHost,
                'http_proxy_port' => $proxyPort
            ]);
        }

        if ($useHttpAuthIfConfigured && $this->httpAuth) {
            list($username, $password) = explode(':', $this->httpAuth);
            $client->setBasicAuth($username, $password);
        }

        $client->setHeaders($requestHeaders);
        $client->set([
            'timeout' => $timeout,
            'connect_timeout' => $timeout,
            'write_timeout' => $timeout,
            'read_timeout' => $timeout,
        ]);

        $client->setMethod($httpMethod);

        $url = str_replace(["\\ ", ' '], ['%20', '%20'], $url); // fix for HTTP 400 Bad Request for URLs with spaces
        $client->execute($url);
        $client->close();

        $headers = $client->headers ?? [];
        if ($client->set_cookie_headers) {
            $headers['set-cookie'] = $client->set_cookie_headers;
        }

        $result = new HttpResponse($url, $client->statusCode, $client->body, $headers, microtime(true) - $startTime);
        $this->saveToCache($cacheKey, $result);
        return $result;
    }

    private function getFromCache(string $cacheKey): ?HttpResponse
    {
        if ($this->cacheDir === null) {
            return null;
        }

        $cacheFile = $this->getCacheFilePath($cacheKey);
        if (!$cacheFile || !is_file($cacheFile)) {
            return null;
        }

        $result = $this->compression
            ? unserialize(gzdecode(file_get_contents($cacheFile)))
            : unserialize(file_get_contents($cacheFile));

        // If cached response is bool (true/false), it means that content was not found or not properly loaded or serialized
        if (is_bool($result)) {
            return null;
        }

        // If cached response is 429/500/503 or -1 to -4 (errors), we don't want to use it again, and we want to try to get new response
        if (in_array($result->statusCode, [429, 500, 502, 503, -1, -2, -3, -4])) {
            return null;
        }
        return $result;
    }

    /**
     * @param string $cacheKey
     * @param HttpResponse $result
     * @return void
     * @throws Exception
     */
    private function saveToCache(string $cacheKey, HttpResponse $result): void
    {
        $cacheFile = $this->getCacheFilePath($cacheKey);
        if ($cacheFile === null) {
            return;
        }
        $cacheDir = dirname($cacheFile);
        if ((!is_dir($cacheDir) || !is_writable($cacheDir)) && !@mkdir($cacheDir, 0777, true)) {
            clearstatcache(true);
            if (!is_dir($cacheDir) || !is_writable($cacheDir)) {
                throw new Exception('Cannot create or write to cache dir ' . $cacheDir);
            }
        }

        if (!@file_put_contents($cacheFile, $this->compression ? gzencode(serialize($result)) : serialize($result))) {
            throw new Exception('Cannot write to cache file ' . $cacheFile . '. Check permissions.');
        }
    }

    private function getCacheFilePath(string $cacheKey): ?string
    {
        if ($this->cacheDir === null) {
            return null;
        }
        return $this->cacheDir . '/' . $cacheKey . '.cache' . ($this->compression ? '.gz' : '');
    }

    /**
     * @param string $host
     * @param int $port
     * @param array $args
     * @param array|string|null $extension
     * @return string
     */
    private function getCacheKey(string $host, int $port, array $args, array|string|null $extension): string
    {
        $md5 = md5(serialize($args));
        return $host . '-' . $port . '/' . substr($md5, 0, 2) . '/' . $md5 . ($extension ? ".{$extension}" : '');
    }

}