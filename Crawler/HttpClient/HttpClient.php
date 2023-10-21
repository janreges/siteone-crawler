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
     * @param string $accept
     * @param string $acceptEncoding
     * @param ?string $origin
     * @return HttpResponse
     */
    public function request(string $host, int $port, string $scheme, string $url, string $httpMethod, int $timeout, string $userAgent, string $accept, string $acceptEncoding, ?string $origin = null): HttpResponse
    {
        $extension = @pathinfo(@parse_url($url, PHP_URL_PATH), PATHINFO_EXTENSION);
        $cacheKey = $host . '.' . md5(serialize(func_get_args())) . ($extension ? ".{$extension}" : '');
        $cachedResult = $this->getFromCache($cacheKey);
        if ($cachedResult !== null && str_contains($url, ' ') === false) {
            return $cachedResult;
        }

        $requestHeaders = [
            'X-Crawler-Info' => 'siteone-website-crawler/' . VERSION,
            'User-Agent' => $userAgent,
            'Accept' => $accept,
            'Accept-Encoding' => $acceptEncoding,
        ];
        if ($origin) {
            $requestHeaders['Origin'] = $origin;
        }

        $startTime = microtime(true);
        $client = new Client($host, $port, $scheme === 'https');
        $client->setHeaders($requestHeaders);
        $client->set(['timeout' => $timeout]);
        $client->setMethod($httpMethod);

        $url = str_replace(["\\ ", ' '], ['%20', '%20'], $url); // fix for HTTP 400 Bad Request for URLs with spaces
        $client->execute($url);

        $result = new HttpResponse($url, $client->statusCode, $client->body, $client->headers ?? [], microtime(true) - $startTime);
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