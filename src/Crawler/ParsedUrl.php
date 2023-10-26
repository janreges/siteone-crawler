<?php

namespace Crawler;

class ParsedUrl
{
    public string $url;
    public ?string $scheme;
    public ?string $host;
    public ?int $port;
    public string $path;
    public ?string $query;
    public ?string $fragment;
    public ?string $extension = null;

    private bool $debug = false;

    /**
     * @param string $url
     * @param string|null $scheme
     * @param string|null $host
     * @param int|null $port
     * @param string $path
     * @param string|null $query
     * @param string|null $fragment
     * @param string|null $extension
     */
    public function __construct(string $url, ?string $scheme, ?string $host, ?int $port, string $path, ?string $query, ?string $fragment, ?string $extension)
    {
        $this->url = $url;
        $this->scheme = $scheme;
        $this->host = $host;
        $this->port = $port;
        $this->path = $path;
        $this->query = $query;
        $this->fragment = $fragment === '' ? null : $fragment;
        $this->extension = $extension;
    }

    public function getFullUrl(bool $includeSchemeAndHost = true, bool $includeFragment = true): string
    {
        $result = $this->path . ($this->query !== null ? '?' . $this->query : '') . (($includeFragment && $this->fragment !== null) ? '#' . $this->fragment : '');
        if ($includeSchemeAndHost && $this->scheme && $this->host) {
            $port = $this->port;
            if ($port === 80 && $this->scheme === 'http') {
                $port = null;
            } else if ($port === 443 && $this->scheme === 'https') {
                $port = null;
            }
            return $this->scheme . '://' . $this->host . ($port !== null ? ':' . $port : '') . $result;
        } elseif ($includeSchemeAndHost && !$this->scheme && $this->host) {
            $port = $this->port && !in_array($this->port, [80, 443]) ? $this->port : null;
            return '//' . $this->host . ($port !== null ? ':' . $port : '') . $result;
        } else {
            return $result;
        }
    }

    /**
     * Is static file with an extension and probably not the HTML page?
     *
     * @return bool
     */
    public function isStaticFile(): bool
    {
        static $htmlExtensionsRegex = null;
        if ($htmlExtensionsRegex === null) {
            $htmlExtensionsRegex = implode('|', [
                'htm', 'html', 'shtml', 'php', 'phtml', 'ashx', 'xhtml', 'asp', 'aspx', 'jsp',
                'jspx', 'do', 'cfm', 'cgi', 'pl', 'rb', 'erb', 'gsp'
            ]);
        }

        // has an extension but is not evident HTML
        if (preg_match('/\.([a-z0-9]{1,10})$/i', $this->path) === 1 && preg_match('/\.(' . $htmlExtensionsRegex . ')$/i', $this->path) === 0) {
            return true;
        }

        return false;
    }

    public function isImage(): bool
    {
        if (preg_match('/\.(png|gif|jpg|jpeg|ico|webp|avif|tif|bmp|svg)/i', $this->path) === 1) {
            return true;
        }
        return false;
    }

    public function isFont(): bool
    {
        if (preg_match('/\.(eot|ttf|woff2|woff|otf)/i', $this->path) === 1) {
            return true;
        }
        return false;
    }

    public function isOriginRequired(): bool
    {
        return $this->isFont();
    }

    /**
     * @return string|null
     */
    public function estimateExtension(): ?string
    {
        if ($this->extension) {
            return strtolower($this->extension);
        } else if (preg_match_all('/\.([0-9a-z]{1,5})/i', $this->path . '?' . ($this->query ?? ''), $matches)) {
            if (isset($matches[1])) {
                return strtolower(end($matches[1]));
            }
        }
        return null;
    }

    /**
     * @param ParsedUrl $url
     * @param bool $scheme
     * @param bool $host
     * @param bool $port
     * @return void
     */
    public function setAttributes(ParsedUrl $url, bool $scheme = false, bool $host = false, bool $port = false): void
    {
        if ($scheme) {
            $this->scheme = $url->scheme;
        }
        if ($host) {
            $this->host = $url->host;
        }
        if ($port) {
            $this->port = $url->port;
        }
    }

    public function setPath(string $path, ?string $reason = null): void
    {
        if ($this->debug && $this->path !== $path) {
            Debugger::debug('parsed-url-set-path', "Changed from '{$this->path}' to '{$path}'" . ($reason ? " ({$reason})" : ''));
        }
        $this->path = $path;
        $this->extension = pathinfo($this->path, PATHINFO_EXTENSION) ?: null;
    }

    /**
     * @param int $change Positive or negative integer
     * @param string|null $reason
     * @return void
     */
    public function changeDepth(int $change, ?string $reason): void
    {
        $newPath = $this->path;
        if ($change > 0) {
            $newPath = str_repeat('../', $change) . ltrim($newPath, '/');
        } else if ($change < 0) {
            $newPath = preg_replace('/\.\.\//', '', $newPath, abs($change));
        }

        if ($newPath !== $this->path) {
            $this->setPath($newPath, "changed depth by {$change}, reason: {$reason})");
        }
    }

    public function setQuery(?string $query): void
    {
        if ($this->debug && $this->query !== $query) {
            Debugger::debug('parsed-url-set-query', "Changed from '{$this->query}' to '{$query}'");
        }
        $this->query = $query;
    }

    public function setFragment(?string $fragment): void
    {
        $this->fragment = $fragment;
    }

    public function setExtension(?string $extension): void
    {
        $this->extension = $extension;
    }

    public function setDebug(bool $debug): void
    {
        $this->debug = $debug;
        if ($this->debug) {
            Debugger::forceEnabledDebug('log/debug.log');
        }
    }

    public function isOnlyFragment(): bool
    {
        return $this->path === '' && $this->query === null && $this->host === null && $this->fragment !== null;
    }

    public static function parse(string $url): self
    {
        $parsedUrl = parse_url($url);
        $scheme = $parsedUrl['scheme'] ?? null;
        $host = $parsedUrl['host'] ?? null;
        $port = $parsedUrl['port'] ?? null;
        if ($port === null) {
            $port = $scheme === 'http' ? 80 : 443;
        }
        $path = $parsedUrl['path'] ?? (isset($parsedUrl['host']) ? '/' : '');
        $query = $parsedUrl['query'] ?? null;
        $fragment = $parsedUrl['fragment'] ?? null;
        $extension = ($path && str_contains($path, '.')) ? pathinfo($path, PATHINFO_EXTENSION) : null;

        return new self($url, $scheme, $host, $port, $path, $query, $fragment, $extension);
    }

}