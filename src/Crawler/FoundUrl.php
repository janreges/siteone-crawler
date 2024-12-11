<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler;

class FoundUrl
{
    const SOURCE_INIT_URL = 5;
    const SOURCE_A_HREF = 10;
    const SOURCE_IMG_SRC = 20;
    const SOURCE_IMG_SRCSET = 21;
    const SOURCE_INPUT_SRC = 22;
    const SOURCE_SOURCE_SRC = 23;
    const SOURCE_VIDEO_SRC = 24;
    const SOURCE_AUDIO_SRC = 25;
    const SOURCE_SCRIPT_SRC = 30;
    const SOURCE_INLINE_SCRIPT_SRC = 40;
    const SOURCE_LINK_HREF = 50;
    const SOURCE_CSS_URL = 60;
    const SOURCE_JS_URL = 70;
    const SOURCE_REDIRECT = 80;
    const SOURCE_SITEMAP = 90;

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
     * @var int
     */
    public readonly int $source;

    /**
     * @param string $url
     * @param string $sourceUrl
     * @param int $source
     */
    public function __construct(string $url, string $sourceUrl, int $source)
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
            ['&#38;', '&amp;', "\\ ", ' '],
            ['&', '&', '%20', '%20'], $url);

        $url = ltrim($url, "\"'\t ");
        return rtrim($url, "&\"'\t ");
    }

    /**
     * @param int $source See self::SOURCE_*
     * @return string
     */
    public static function getShortSourceName(int $source): string
    {
        return match ($source) {
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

}