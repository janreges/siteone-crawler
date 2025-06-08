<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

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
    public ?string $domain2ndLevel = null;

    /**
     * Cache used in getFullUrl() method
     * @var string[]
     */
    private array $fullUrlCache = [];

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
     * @param string|null $domain2ndLevel
     */
    public function __construct(string $url, ?string $scheme, ?string $host, ?int $port, string $path, ?string $query, ?string $fragment, ?string $extension, ?string $domain2ndLevel = null)
    {
        $this->url = $url;
        $this->scheme = $scheme;
        $this->host = $host;
        $this->port = $port;
        $this->path = $path;
        $this->query = $query;
        $this->fragment = $fragment === '' ? null : $fragment;
        $this->extension = $extension;
        $this->domain2ndLevel = $domain2ndLevel;
    }

    public function getFullUrl(bool $includeSchemeAndHost = true, bool $includeFragment = true): string
    {
        $cacheKey = ($includeSchemeAndHost ? '1' : '0') . ($includeFragment ? '1' : '0');
        if (!isset($this->fullUrlCache[$cacheKey])) {
            $fullUrl = $this->path . ($this->query !== null ? '?' . $this->query : '') . (($includeFragment && $this->fragment !== null) ? '#' . $this->fragment : '');
            if ($includeSchemeAndHost && $this->scheme && $this->host) {
                $port = $this->port;
                if ($port === 80 && $this->scheme === 'http') {
                    $port = null;
                } else if ($port === 443 && $this->scheme === 'https') {
                    $port = null;
                }
                $fullUrl = $this->scheme . '://' . $this->host . ($port !== null ? ':' . $port : '') . $fullUrl;
            } elseif ($includeSchemeAndHost && !$this->scheme && $this->host) {
                $port = $this->port && !in_array($this->port, [80, 443]) ? $this->port : null;
                $fullUrl = '//' . $this->host . ($port !== null ? ':' . $port : '') . $fullUrl;
            }
            $this->fullUrlCache[$cacheKey] = $fullUrl;
        }

        return $this->fullUrlCache[$cacheKey];
    }

    /**
     * Is probably static file/asset and probably not the HTML page?
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

        if (preg_match('/\.([a-z0-9]{1,10})$/i', $this->path) === 1 && !is_numeric($this->extension) && preg_match('/\.(' . $htmlExtensionsRegex . ')$/i', $this->path) === 0) {
            // has an extension but is not evident HTML page
            return true;
        } elseif ($this->isImage() || $this->isCss()) {
            return true;
        }

        return false;
    }

    /**
     * Is probably image? Has an image extension or is dynamic image (has query with image manipulation parameters)
     * It is not 100% accurate, but it is good enough for our purposes
     *
     * @return bool
     */
    public function isImage(): bool
    {
        $hasImageExtension = preg_match('/\.(png|gif|jpg|jpeg|ico|webp|avif|tif|bmp|svg)/i', $this->path) === 1;
        $isDynamicImage = $this->query && preg_match('/(png|gif|jpg|jpeg|ico|webp|avif|tif|bmp|svg|crop|size|landscape)/i', $this->query) === 1;
        if ($hasImageExtension || $isDynamicImage) {
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

    public function isCss(): bool
    {
        // hardcoded google domains is not ideal, ready to refactor in the future .. we need to work better
        // with source element and attribute where URL was found to estimate type/extension more accurately
        return $this->extension === 'css' || stripos($this->url, 'fonts.googleapis.com/css') !== false;
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
        // if extension is numeric, it is probably not an extension but for example /blog/about-version-3.2
        if ($this->extension && is_numeric($this->extension)) {
            return null;
        } else if ($this->extension) {
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

        $this->fullUrlCache = [];
    }

    public function setPath(string $path, ?string $reason = null): void
    {
        if ($this->debug && $this->path !== $path) {
            Debugger::debug('parsed-url-set-path', "Changed from '{$this->path}' to '{$path}'" . ($reason ? " ({$reason})" : ''));
        }
        $this->path = $path;
        $this->extension = pathinfo($this->path, PATHINFO_EXTENSION) ?: null;

        $this->fullUrlCache = [];
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
            // First trim any leading slashes to ensure clean path
            $cleanPath = ltrim($newPath, '/');
            $newPath = str_repeat('../', $change) . $cleanPath;
        } else if ($change < 0) {
            $newPath = preg_replace('/\.\.\//', '', $newPath, abs($change));
        }

        if ($newPath !== $this->path) {
            $this->setPath($newPath, "changed depth by {$change}, reason: {$reason})");
        }

        $this->fullUrlCache = [];
    }

    public function setQuery(?string $query): void
    {
        if ($this->debug && $this->query !== $query) {
            Debugger::debug('parsed-url-set-query', "Changed from '{$this->query}' to '{$query}'");
        }
        $this->query = $query;

        $this->fullUrlCache = [];
    }

    public function setFragment(?string $fragment): void
    {
        $this->fragment = $fragment;

        $this->fullUrlCache = [];
    }

    public function setExtension(?string $extension): void
    {
        $this->extension = $extension;

        $this->fullUrlCache = [];
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

    /**
     * Get full homepage URL (scheme://host[:port]) without trailing slash
     *
     * @return string
     */
    public function getFullHomepageUrl(): string
    {
        return $this->scheme . '://' . $this->host . ($this->port !== null ? ':' . $this->port : '');
    }

    /**
     * Parse URL and return ParsedUrl object
     * When $baseUrl is provided, it is used to fill missing parts of URL (scheme, host, port)
     *
     * @param string $url
     * @param ParsedUrl|null $baseUrl
     * @return self
     */
    public static function parse(string $url, ?ParsedUrl $baseUrl = null): self
    {
        if ($baseUrl) {
            if (str_starts_with($url, './')) {
                // URL is relative to base URL by ./xyz
                if (str_ends_with($baseUrl->path, '/')) {
                    $url = $baseUrl->path . substr($url, 2);
                } else {
                    $dir = dirname($baseUrl->path);
                    $file = substr($url, 2);
                    if ($dir === '/') {
                        $url = '/' . $file;
                    } else {
                        $url = $dir . '/' . $file;
                    }
                }
            } else if (!str_starts_with($url, 'http:') && !str_starts_with($url, 'https:') && preg_match('/^[a-z0-9_]/i', $url) === 1) {
                // URL is relative to base URL by xyz/abc
                if (str_ends_with($baseUrl->path, '/')) {
                    $url = $baseUrl->path . $url;
                } else {
                    $url = dirname($baseUrl->path) . $url;
                }
            } elseif (str_starts_with($url, '/') && !str_starts_with($url, '//')) {
                // absolute URL /xyz/abc
                $url = $baseUrl->getFullHomepageUrl() . $url;
            }
        }

        $parsedUrl = parse_url($url);
        $scheme = $parsedUrl['scheme'] ?? ($baseUrl?->scheme);
        $host = $parsedUrl['host'] ?? ($baseUrl?->host);
        $port = $parsedUrl['port'] ?? (!isset($parsedUrl['host']) ? ($baseUrl?->port) : null);
        if ($port === null) {
            $port = $scheme === 'http' ? 80 : 443;
        }
        $path = $parsedUrl['path'] ?? (isset($parsedUrl['host']) ? '/' : '');
        $query = $parsedUrl['query'] ?? null;
        $fragment = $parsedUrl['fragment'] ?? null;
        $extension = ($path && str_contains($path, '.')) ? pathinfo($path, PATHINFO_EXTENSION) : null;
        $domain2ndLevel = null;
        if ($host && preg_match('/([a-z0-9\-]+\.[a-z][a-z0-9]{0,10})$/i', $host, $matches) === 1) {
            $domain2ndLevel = $matches[1];
        };

        return new self($url, $scheme, $host, $port, $path, $query, $fragment, $extension, $domain2ndLevel);
    }

    public function isHttps(): bool
    {
        return $this->scheme === 'https';
    }

    /**
     * Get base name (last path part) of the URL
     * Examples:
     *  - "bar" for "https://mydomain.tld/foo/bar"
     *  - "foo" for "https://mydomain.tld/foo/?abc=def"
     *  - "my-img.jpg" for "https://mydomain.tld/foo/my-img.jpg"
     *  - null for "https://mydomain.tld/"
     *  - SPECIAL CASE: 'image?url=path/image.jpg' for "https://mydomain.tld/_next/image?url=path/image.jpg"
     *
     * @return string|null
     */
    public function getBaseName(): ?string
    {
        if (!$this->path || $this->path === '/') {
            return null;
        }

        $path = $this->path;
        if (str_ends_with($path, '/')) {
            $path = substr($path, 0, -1);
        }

        $pathParts = explode('/', $path);
        $result = end($pathParts) ?: null;

        // if query string contains path (it may be dynamic image), return path with this query
        if ($this->query && (str_contains($this->query, '/') || str_contains($this->query, '%2F'))) {
            $result .= '?' . $this->query;
        }

        return $result;
    }

    /**
     * Get depth of the URL path. Examples:
     * / -> 0
     * /about -> 1
     * /about/ -> 1
     * /about/me -> 2
     * /about/me/ -> 2
     * /about/me/contact -> 3
     * /about/me/contact/ -> 3
     * /about/me/contact/.. -> 2
     * /about/me/contact/../.. -> 1
     * ...
     *
     * @return int
     */
    public function getDepth(): int
    {
        return max(substr_count(rtrim($this->path, '/'), '/') - substr_count($this->path, '/..'), 0);
    }

}