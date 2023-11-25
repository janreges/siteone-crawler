<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Export\Utils;

use Crawler\ParsedUrl;

enum TargetDomainRelation
{
    case INITIAL_SAME__BASE_SAME;           // eg. initial www.siteone.io, base www.siteone.io, target www.siteone.io
    case INITIAL_SAME__BASE_DIFFERENT;      // eg. initial www.siteone.io, base nextjs.org,     target www.siteone.io
    case INITIAL_DIFFERENT__BASE_SAME;      // eg. initial www.siteone.io, base nextjs.org,     target nextjs.org
    case INITIAL_DIFFERENT__BASE_DIFFERENT; // eg. initial www.siteone.io, base nextjs.org,     target svelte.dev

    /**
     * @param ParsedUrl $initialUrl
     * @param ParsedUrl $baseUrl
     * @param ParsedUrl $targetUrl
     * @return TargetDomainRelation
     */
    public static function getByUrls(ParsedUrl $initialUrl, ParsedUrl $baseUrl, ParsedUrl $targetUrl): self
    {
        if (!$targetUrl->host || $targetUrl->host === $baseUrl->host) {
            // base host is the same as target host
            return $baseUrl->host === $initialUrl->host ? self::INITIAL_SAME__BASE_SAME : self::INITIAL_DIFFERENT__BASE_SAME;
        } else {
            // base host is different from target host
            return $targetUrl->host === $initialUrl->host ? self::INITIAL_SAME__BASE_DIFFERENT : self::INITIAL_DIFFERENT__BASE_DIFFERENT;
        }
    }
}
