<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Result;

use Crawler\Crawler;
use Crawler\FoundUrl;
use Crawler\Utils;

class VisitedUrl
{
    const ERROR_CONNECTION_FAIL = -1;
    const ERROR_TIMEOUT = -2;
    const ERROR_SERVER_RESET = -3;
    const ERROR_SEND_ERROR = -4;
    const ERROR_SKIPPED = -6;

    // cache type flags for the cacheType property (uses bitwise OR)
    const CACHE_TYPE_HAS_CACHE_CONTROL = 1;
    const CACHE_TYPE_HAS_EXPIRES = 2;
    const CACHE_TYPE_HAS_ETAG = 4;
    const CACHE_TYPE_HAS_LAST_MODIFIED = 8;
    const CACHE_TYPE_HAS_MAX_AGE = 16;
    const CACHE_TYPE_HAS_S_MAX_AGE = 32;
    const CACHE_TYPE_HAS_STALE_WHILE_REVALIDATE = 64;
    const CACHE_TYPE_HAS_STALE_IF_ERROR = 128;
    const CACHE_TYPE_HAS_PUBLIC = 256;
    const CACHE_TYPE_HAS_PRIVATE = 512;
    const CACHE_TYPE_HAS_NO_CACHE = 1024;
    const CACHE_TYPE_HAS_NO_STORE = 2048;
    const CACHE_TYPE_HAS_MUST_REVALIDATE = 4096;
    const CACHE_TYPE_HAS_PROXY_REVALIDATE = 8192;
    const CACHE_TYPE_HAS_IMMUTABLE = 16384;
    const CACHE_TYPE_NO_CACHE_HEADERS = 32768;
    const CACHE_TYPE_NOT_AVAILABLE = 65536;

    /**
     * @var string Unique ID hash of this URL
     */
    public readonly string $uqId;

    /**
     * @var string Unique ID hash of the source URL where this URL was found
     */
    public readonly string $sourceUqId;

    /**
     * @var int Source attribute where this URL was found. See FoundUrl::SOURCE_* constants
     */
    public readonly int $sourceAttr;

    /**
     * Full URL with scheme, domain, path and query
     * @var string URL
     */
    public readonly string $url;

    /**
     * HTTP status code of the request
     * Negative values are errors - see self:ERROR_* constants
     * @var int
     */
    public readonly int $statusCode;

    /**
     * Request time in seconds
     * @var float
     */
    public readonly float $requestTime;

    /**
     * Request time formatted as "32 ms" or "7.4 s"
     * @var string
     */
    public readonly string $requestTimeFormatted;

    /**
     * Size of the response in bytes
     * @var int|null
     */
    public readonly ?int $size;

    /**
     * Size of the response formatted as "1.23 MB"
     * @var string|null
     */
    public readonly ?string $sizeFormatted;

    /**
     * Content-Encoding header value (br, gzip, ...)
     * @var string|null
     */
    public readonly ?string $contentEncoding;

    /**
     * Content type ID
     * @see Crawler::CONTENT_TYPE_ID_*
     * @var int
     */
    public readonly int $contentType;

    /**
     * Content type header value (text/html, application/json, ...)
     * @var string|null
     */
    public readonly ?string $contentTypeHeader;

    /**
     * Extra data from the response required by --extra-columns (headers, Title, DOM, etc.
     * @var array|null
     */
    public readonly ?array $extras;

    /**
     * Is this URL external (not from the same domain as the initial URL)
     * @var bool
     */
    public readonly bool $isExternal;

    /**
     * Is this URL allowed for crawling (based on --allowed-domain-for-crawling)
     * @var bool
     */
    public readonly bool $isAllowedForCrawling;

    /**
     * Cache type flags of the response (bitwise OR). See self::CACHE_TYPE_* constants
     * @var int
     */
    public readonly int $cacheTypeFlags;

    /**
     * How long the response is allowed to be cached in seconds (based on max-age in Cache-Control or Expires header)
     * @var int|null
     */
    public readonly int|null $cacheLifetime;

    /**
     * @var string|null
     */
    private ?string $hostCache = null;

    /**
     * @param string $uqId
     * @param string $sourceUqId
     * @param int $sourceAttr
     * @param string $url
     * @param int $statusCode
     * @param float $requestTime
     * @param int|null $size
     * @param int $contentType
     * @param string|null $contentTypeHeader
     * @param string|null $contentEncoding
     * @param array|null $extras
     * @param bool $isExternal
     * @param bool $isAllowedForCrawling
     * @param int $cacheType
     * @param int|null $cacheLifetime
     */
    public function __construct(string $uqId, string $sourceUqId, int $sourceAttr, string $url, int $statusCode, float $requestTime, ?int $size, int $contentType, ?string $contentTypeHeader, ?string $contentEncoding, ?array $extras, bool $isExternal, bool $isAllowedForCrawling, int $cacheType, ?int $cacheLifetime)
    {
        $this->uqId = $uqId;
        $this->sourceUqId = $sourceUqId;
        $this->sourceAttr = $sourceAttr;
        $this->url = $url;
        $this->statusCode = $statusCode;
        $this->requestTime = $requestTime;
        $this->requestTimeFormatted = Utils::getFormattedDuration($this->requestTime);
        $this->size = $size;
        $this->sizeFormatted = $size !== null ? Utils::getFormattedSize($size) : null;
        $this->contentType = $contentType;
        $this->contentTypeHeader = $contentTypeHeader;
        $this->contentEncoding = $contentEncoding;
        $this->extras = $extras;
        $this->isExternal = $isExternal;
        $this->isAllowedForCrawling = $isAllowedForCrawling;
        $this->cacheTypeFlags = $cacheType;
        $this->cacheLifetime = $cacheLifetime;
    }

    public function isHttps(): bool
    {
        return str_starts_with($this->url, 'https://');
    }

    public function isStaticFile(): bool
    {
        static $staticTypes = [
            Crawler::CONTENT_TYPE_ID_IMAGE,
            Crawler::CONTENT_TYPE_ID_SCRIPT,
            Crawler::CONTENT_TYPE_ID_STYLESHEET,
            Crawler::CONTENT_TYPE_ID_VIDEO,
            Crawler::CONTENT_TYPE_ID_AUDIO,
            Crawler::CONTENT_TYPE_ID_DOCUMENT,
            Crawler::CONTENT_TYPE_ID_FONT,
            Crawler::CONTENT_TYPE_ID_JSON,
            Crawler::CONTENT_TYPE_ID_XML,
        ];

        return in_array($this->contentType, $staticTypes);
    }

    public function isImage(): bool
    {
        return $this->contentType === Crawler::CONTENT_TYPE_ID_IMAGE;
    }

    public function isVideo(): bool
    {
        return $this->contentType === Crawler::CONTENT_TYPE_ID_VIDEO;
    }

    /**
     * @param string|null $sourceUrl
     * @return string
     */
    public function getSourceDescription(?string $sourceUrl): string
    {
        return match ($this->sourceAttr) {
            FoundUrl::SOURCE_INIT_URL => 'Initial URL',
            FoundUrl::SOURCE_A_HREF => "<a href> on $sourceUrl",
            FoundUrl::SOURCE_IMG_SRC => "<img src> on $sourceUrl",
            FoundUrl::SOURCE_IMG_SRCSET => "<img srcset> on $sourceUrl",
            FoundUrl::SOURCE_INPUT_SRC => "<input src> on $sourceUrl",
            FoundUrl::SOURCE_SOURCE_SRC => "<source src> on $sourceUrl",
            FoundUrl::SOURCE_VIDEO_SRC => "<video src> on $sourceUrl",
            FoundUrl::SOURCE_AUDIO_SRC => "<audio src> on $sourceUrl",
            FoundUrl::SOURCE_SCRIPT_SRC => "<script src> on $sourceUrl",
            FoundUrl::SOURCE_INLINE_SCRIPT_SRC => "<script> on $sourceUrl",
            FoundUrl::SOURCE_LINK_HREF => "<link href> on $sourceUrl",
            FoundUrl::SOURCE_CSS_URL => "CSS url() on $sourceUrl",
            FoundUrl::SOURCE_JS_URL => "JS url on $sourceUrl",
            FoundUrl::SOURCE_REDIRECT => "Redirect from $sourceUrl",
            FoundUrl::SOURCE_SITEMAP => "URL in sitemap $sourceUrl",
            default => 'Unknown source',
        };
    }

    /**
     * @return string
     */
    public function getSourceShortName(): string
    {
        return match ($this->sourceAttr) {
            FoundUrl::SOURCE_INIT_URL => 'Initial URL',
            FoundUrl::SOURCE_A_HREF => '<a href>',
            FoundUrl::SOURCE_IMG_SRC => '<img src>',
            FoundUrl::SOURCE_IMG_SRCSET => '<img srcset>',
            FoundUrl::SOURCE_INPUT_SRC => '<input src>',
            FoundUrl::SOURCE_SOURCE_SRC => '<source src>',
            FoundUrl::SOURCE_VIDEO_SRC => '<video src>',
            FoundUrl::SOURCE_AUDIO_SRC => '<audio src>',
            FoundUrl::SOURCE_SCRIPT_SRC => '<script src>',
            FoundUrl::SOURCE_INLINE_SCRIPT_SRC => 'inline <script src>',
            FoundUrl::SOURCE_LINK_HREF => '<link href>',
            FoundUrl::SOURCE_CSS_URL => 'css url()',
            FoundUrl::SOURCE_JS_URL => 'js url',
            FoundUrl::SOURCE_REDIRECT => 'redirect',
            FoundUrl::SOURCE_SITEMAP => 'sitemap',
            default => 'unknown',
        };
    }

    public function looksLikeStaticFileByUrl(): bool
    {
        return preg_match('/\.(jpg|jpeg|png|gif|webp|svg|ico|js|css|txt|woff2|woff|ttf|eot|mp4|webm|ogg|mp3|wav|flac|pdf|doc|docx|xls|xlsx|ppt|pptx|zip|rar|gz|bz2|7z|xml|json)/i', $this->url) === 1;
    }

    public function hasErrorStatusCode(): bool
    {
        return $this->statusCode < 0;
    }

    public function getScheme(): string
    {
        return parse_url($this->url, PHP_URL_SCHEME);
    }

    public function getHost(): string
    {
        if ($this->hostCache !== null) {
            return $this->hostCache;
        }
        $this->hostCache = parse_url($this->url, PHP_URL_HOST);
        return $this->hostCache;
    }

    public function getPort(): int
    {
        $port = parse_url($this->url, PHP_URL_PORT);
        if ($port === null) {
            $port = $this->isHttps() ? 443 : 80;
        }
        return (int)$port;
    }

    public function getCacheTypeLabel(): string
    {
        $labels = [];

        // Cache-Control or Expires (if Cache-Control is not defined)
        if ($this->cacheTypeFlags & self::CACHE_TYPE_HAS_CACHE_CONTROL) {
            $labels[] = 'Cache-Control';
        } else if ($this->cacheTypeFlags & self::CACHE_TYPE_HAS_EXPIRES) {
            $labels[] = 'Expires';
        }

        // ETag and Last-Modified
        if ($this->cacheTypeFlags & self::CACHE_TYPE_HAS_ETAG) {
            $labels[] = 'ETag';
        }
        if ($this->cacheTypeFlags & self::CACHE_TYPE_HAS_LAST_MODIFIED) {
            $labels[] = 'Last-Modified';
        }

        return $labels ? implode(' + ', $labels) : 'No cache headers';
    }

}