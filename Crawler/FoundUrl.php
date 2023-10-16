<?php

namespace Crawler;

class FoundUrl
{
    const SOURCE_A_HREF = 'href';
    const SOURCE_IMG_SRC = 'img-src';
    const SOURCE_SCRIPT_SRC = 'script-src';
    const SOURCE_INLINE_SCRIPT_SRC = 'inline-script-src';
    const SOURCE_LINK_HREF = 'link-href';
    const SOURCE_CSS_URL = 'css-url';

    /**
     * Founded URL, parsed from $this->sourceUrl
     * @var string
     */
    public readonly string $url;

    /**
     * URL from which this URL was found
     * @var string
     */
    public readonly string $sourceUrl;

    /**
     * Source of this URL - where in HTML/CSS was found
     * Values are constants self::SOURCE_* from this class
     * @var string
     */
    public readonly string $source;

    /**
     * @param string $url
     * @param string $sourceUrl
     * @param string $source
     */
    public function __construct(string $url, string $sourceUrl, string $source)
    {
        $this->url = $this->normalizeUrl($url);
        $this->sourceUrl = $sourceUrl;
        $this->source = $source;
    }

    /**
     * Is this URL as included asset (img src, script src, link href) and not linked by href?
     * @return bool
     */
    public function isIncludedAsset(): bool
    {
        return $this->source !== self::SOURCE_A_HREF;
    }

    public function __toString(): string
    {
        return $this->url;
    }

    /**
     * Normalize URL and remove some often used strange characters/behavior
     *
     * @param string $url
     * @return string
     */
    private function normalizeUrl(string $url): string
    {
        $url = str_replace(
            ['&#38;', '&amp;'],
            ['&', '&'], $url);
        
        $url = ltrim($url, "\"'\t ");
        return rtrim($url, "&\"'\t ");
    }

}