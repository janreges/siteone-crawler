<?php

namespace Crawler\HttpClient;

use Swoole\Coroutine\Http\Client;

class HttpClient
{

    /**
     * Cache dir for http client. If null, cache is disabled
     * @var string|null
     */
    private readonly ?string $cacheDir;

    /**
     * @param string|null $cacheDir
     */
    public function __construct(?string $cacheDir)
    {
        $this->cacheDir = $cacheDir;
    }

    /**
     * @param string $host
     * @param int $port
     * @param string $scheme
     * @param string $url
     * @param string $httpMethod
     * @param int $timeout
     * @param string $userAgent
     * @param string $acceptEncoding
     * @return HttpResponse
     */
    public function request(string $host, int $port, string $scheme, string $url, string $httpMethod, int $timeout, string $userAgent, string $acceptEncoding): HttpResponse
    {
        $cacheKey = $this->getRequestCacheKey($host, $port, $url, $httpMethod);
        $cachedResult = $this->getFromCache($cacheKey);
        if ($cachedResult !== null) {
            return $cachedResult;
        }

        $startTime = microtime(true);
        $client = new Client($host, $port, $scheme === 'https');
        $client->setHeaders([
                'User-Agent' => $userAgent,
                'Accept-Encoding' => $acceptEncoding
            ]
        );
        $client->set(['timeout' => $timeout]);
        $client->setMethod($httpMethod);
        $client->execute($url);

        $result = new HttpResponse($url, $client->statusCode, $client->body, $client->headers ?? [], microtime(true) - $startTime);
        $this->saveToCache($cacheKey, $result);
        return $result;
    }

    private function getRequestCacheKey(string $host, int $port, string $url, string $httpMethod): string
    {
        return $host . '.' . md5("{$host}_{$port}_{$url}_{$httpMethod}");
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

        $result = unserialize(file_get_contents($cacheFile));

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
        file_put_contents($cacheFile, serialize($result));
    }

    private function getCacheFilePath(string $cacheKey): string
    {
        return $this->cacheDir . '/' . $cacheKey . '.cache';
    }

}