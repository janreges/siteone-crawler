<?php

namespace Crawler;

class ParsedUrl
{
    public string $url;
    public ?string $scheme;
    public ?string $host;
    public ?int $port;
    public string $path;
    public ?string $extension = null;

    /**
     * @param string $url
     * @param string|null $scheme
     * @param string|null $host
     * @param int|null $port
     * @param string $path
     * @param string|null $extension
     */
    public function __construct(string $url, ?string $scheme, ?string $host, ?int $port, string $path, ?string $extension)
    {
        $this->url = $url;
        $this->scheme = $scheme;
        $this->host = $host;
        $this->port = $port;
        $this->path = $path;
        $this->extension = $extension;
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
        $extension = ($path && str_contains($path, '.')) ? pathinfo($path, PATHINFO_EXTENSION) : null;

        return new self($url, $scheme, $host, $port, $path, $extension);
    }

}