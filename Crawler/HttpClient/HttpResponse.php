<?php

namespace Crawler\HttpClient;

class HttpResponse
{
    public readonly string $url;
    public readonly int $statusCode;
    public readonly ?string $body;
    public readonly array $headers;
    public readonly float $execTime;

    /**
     * @param string $url
     * @param int $statusCode
     * @param string|null $body
     * @param array $headers
     * @param float $execTime
     */
    public function __construct(string $url, int $statusCode, ?string $body, array $headers, float $execTime)
    {
        $this->url = $url;
        $this->statusCode = $statusCode;
        $this->body = $body;
        $this->headers = $headers;
        $this->execTime = $execTime;
    }

    public function getFormattedExecTime(): string
    {
        return number_format($this->execTime, 3) . 's';
    }

    public function getFormattedBodyLength(): string
    {
        return number_format(strlen($this->body), 0, '.', ' ') . ' B';
    }

}