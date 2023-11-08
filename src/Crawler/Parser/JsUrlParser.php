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

class JsUrlParser
{
    private readonly string $jsBody;
    private readonly string $sourceUrl;

    /**
     * @param string $jsBody
     * @param string $sourceUrl
     */
    public function __construct(string $jsBody, string $sourceUrl)
    {
        $this->jsBody = $jsBody;
        $this->sourceUrl = $sourceUrl;
    }


    /**
     * @return FoundUrls|null
     */
    public function getUrlsFromJs(): ?FoundUrls
    {
        $isNextJsManifest = str_contains($this->sourceUrl, '_next/') && stripos($this->sourceUrl, 'manifest') !== false;
        if (!$isNextJsManifest) {
            return null;
        }

        $nextJsBaseDir = preg_replace('/(\/_next\/).*$/', '$1', $this->sourceUrl);

        preg_match_all('/["\']([a-z0-9\/._\-\[\]]\.js)["\']/is', $this->jsBody, $matches);
        $foundUrlsTxt = [];
        foreach($matches[1]??[] as $match) {
            $foundUrlsTxt[] = $nextJsBaseDir . $match;
        }

        $foundUrls = new FoundUrls();
        $foundUrls->addUrlsFromTextArray($foundUrlsTxt, $this->sourceUrl, FoundUrl::SOURCE_JS_URL);
        return $foundUrls;
    }

}