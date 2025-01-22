<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\HttpClient;

use Crawler\Utils;

class HttpResponse
{
    public readonly string $url;
    public readonly int $statusCode;
    public readonly ?string $body;
    public readonly array $headers;
    public readonly float $execTime;
    public ?string $skippedReason = null;
    private bool $loadedFromCache = false;

    /**
     * @param string $url
     * @param int $statusCode
     * @param string|null $body
     * @param array $headers
     * @param float $execTime
     */
    public function __construct(string $url, int $statusCode, ?string $body, array $headers, float $execTime)
    {
        $this->detectRedirectAndSetMetaRedirect($statusCode, $body, $headers);

        $this->url = $url;
        $this->statusCode = $statusCode;
        $this->body = $body;
        $this->headers = Utils::getFlatResponseHeaders($headers);
        $this->execTime = $execTime;
    }

    public function getFormattedExecTime(): string
    {
        return Utils::getFormattedDuration($this->execTime);
    }

    public function getFormattedBodyLength(): string
    {
        return Utils::getFormattedSize(strlen($this->body));
    }

    /**
     * Detect redirect and modify response to text/html with <meta> redirect (required for offline mode)
     *
     * @param int $statusCode
     * @param string|null $body
     * @param array $headers
     * @return void
     */
    private function detectRedirectAndSetMetaRedirect(int $statusCode, ?string &$body, array &$headers): void
    {
        if ($statusCode > 300 && $statusCode < 320 && isset($headers['location'])) {
            $body = sprintf(
                '<meta http-equiv="refresh" content="0; url=%s"> Redirecting to %s ...',
                $headers['location'],
                $headers['location'],
            );
            $headers['content-type'] = 'text/html';
        }
    }

    public function setLoadedFromCache(bool $loadedFromCache): void
    {
        $this->loadedFromCache = $loadedFromCache;
    }

    public function isLoadedFromCache(): bool
    {
        return $this->loadedFromCache;
    }

    public function isSkipped(): bool
    {
        return $this->skippedReason !== null;
    }

    public static function createSkipped(string $url, string $reason): self
    {
        $result = new self($url, -6, '', [], 0);
        $result->skippedReason = $reason;
        return $result;
    }

}