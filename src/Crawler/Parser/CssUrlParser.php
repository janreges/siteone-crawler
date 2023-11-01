<?php

/*
 * This file is part of the SiteOne Website Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Parser;

use Crawler\FoundUrl;
use Crawler\FoundUrls;

class CssUrlParser
{
    private readonly string $cssBody;
    private readonly string $sourceUrl;
    private readonly bool $images;
    private readonly bool $fonts;

    /**
     * @param string $cssBody
     * @param string $sourceUrl
     * @param bool $images
     * @param bool $fonts
     */
    public function __construct(string $cssBody, string $sourceUrl, bool $images, bool $fonts)
    {
        $this->cssBody = $cssBody;
        $this->sourceUrl = $sourceUrl;
        $this->images = $images;
        $this->fonts = $fonts;
    }


    /**
     * @return FoundUrls
     */
    public function getUrlsFromCss(): FoundUrls
    {
        preg_match_all('/url\s*\(\s*["\']?([^"\')]+)["\']?\s*\)/im', $this->cssBody, $matches);
        $foundUrlsTxt = $matches[1];

        $foundUrlsTxt = array_filter($foundUrlsTxt, function ($url) {
            $isImage = preg_match('/\.(jpg|jpeg|png|gif|webp|avif|svg|ico|tif|bmp)(|\?.*)$/i', $url) === 1;
            $isFont = preg_match('/\.(eot|ttf|woff2|woff|otf)(|\?.*)$/i', $url) === 1;
            return ($this->images && $isImage) || ($this->fonts && $isFont);
        });

        $foundUrls = new FoundUrls();
        $foundUrls->addUrlsFromTextArray($foundUrlsTxt, $this->sourceUrl, FoundUrl::SOURCE_CSS_URL);
        return $foundUrls;
    }

}