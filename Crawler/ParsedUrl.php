<?php

namespace Crawler;

class ParsedUrl
{
    public string $url;
    public ?string $scheme;
    public ?string $host;
    public ?int $port;
    public string $path;

    /**
     * @param string $url
     * @param string|null $scheme
     * @param string|null $host
     * @param int|null $port
     * @param string $path
     */
    public function __construct(string $url, ?string $scheme, ?string $host, ?int $port, string $path)
    {
        $this->url = $url;
        $this->scheme = $scheme;
        $this->host = $host;
        $this->port = $port;
        $this->path = $path;
    }

    public static function parse(string $url): self
    {
        $parsedUrl = parse_url($url);
        $scheme = $parsedUrl['scheme'] ?? null;
        $host = $parsedUrl['host'] ?? null;
        $port = $parsedUrl['port'] ?? null;
        if ($port === null) {
            $port = $scheme === 'https' ? 443 : 80;
        }
        $path = $parsedUrl['path'] ?? '/';

        return new self($url, $scheme, $host, $port, $path);
    }

}