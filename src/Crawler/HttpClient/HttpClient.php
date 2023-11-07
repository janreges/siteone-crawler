<?php

/*
 * This file is part of the SiteOne Website Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\HttpClient;

use Crawler\Version;
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
     * @return HttpResponse
     */
    public function request(string $host, int $port, string $scheme, string $url, string $httpMethod, int $timeout, string $userAgent, string $accept, string $acceptEncoding, ?string $origin = null): HttpResponse
    {
        $path = @parse_url($url, PHP_URL_PATH);
        $extension = is_string($path) ? @pathinfo($path, PATHINFO_EXTENSION) : null;
        $cacheKey = $host . '.' . md5(serialize(func_get_args())) . ($extension ? ".{$extension}" : '');
        $cachedResult = $this->getFromCache($cacheKey);
        if ($cachedResult !== null && str_contains($url, ' ') === false) {
            $cachedResult->setLoadedFromCache(true);
            return $cachedResult;
        }

        $requestHeaders = [
            'X-Crawler-Info' => 'siteone-website-crawler/' . Version::CODE,
            'User-Agent' => $userAgent,
            'Accept' => $accept,
            'Accept-Encoding' => $acceptEncoding,
        ];
        if ($origin) {
            $requestHeaders['Origin'] = $origin;
        }

        $startTime = microtime(true);
        $client = new Client($host, $port, $scheme === 'https');

        if ($this->proxy) {
            list($proxyHost, $proxyPort) = explode(':', $this->proxy);
            $client->set([
                'http_proxy_host' => $proxyHost,
                'http_proxy_port' => $proxyPort
            ]);
        }

        if ($this->httpAuth) {
            list($username, $password) = explode(':', $this->httpAuth);
            $client->setBasicAuth($username, $password);
        }

        $client->setHeaders($requestHeaders);
        $client->set(['timeout' => $timeout]);
        $client->setMethod($httpMethod);

        $url = str_replace(["\\ ", ' '], ['%20', '%20'], $url); // fix for HTTP 400 Bad Request for URLs with spaces
        $client->execute($url);

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
        if (!is_file($cacheFile)) {
            return null;
        }

        $result = $this->compression
            ? unserialize(gzdecode(file_get_contents($cacheFile)))
            : unserialize(file_get_contents($cacheFile));

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
     */
    private function saveToCache(string $cacheKey, HttpResponse $result): void
    {
        if ($this->cacheDir === null) {
            return;
        };
        if ((!is_dir($this->cacheDir) || !is_writable($this->cacheDir)) && !mkdir($this->cacheDir, 0777, true)) {
            throw new \RuntimeException('Cannot create or write to cache dir ' . $this->cacheDir);
        }

        $cacheFile = $this->getCacheFilePath($cacheKey);
        file_put_contents($cacheFile, $this->compression ? gzencode(serialize($result)) : serialize($result));
    }

    private function getCacheFilePath(string $cacheKey): string
    {
        return $this->cacheDir . '/' . $cacheKey . '.cache' . ($this->compression ? '.gz' : '');
    }

}