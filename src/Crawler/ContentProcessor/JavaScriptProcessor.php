<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\ContentProcessor;

use Crawler\Crawler;
use Crawler\FoundUrls;
use Crawler\ParsedUrl;

class JavaScriptProcessor extends BaseProcessor implements ContentProcessor
{
    protected array $relevantContentTypes = [
        Crawler::CONTENT_TYPE_ID_HTML,
        Crawler::CONTENT_TYPE_ID_SCRIPT,
    ];

    /**
     * @inheritDoc
     */
    public function findUrls(string $content, ParsedUrl $sourceUrl): ?FoundUrls
    {
        return null;
    }

    /**
     * @inheritDoc
     */
    public function applyContentChangesForOfflineVersion(string &$content, int $contentType, ParsedUrl $url): void
    {
        $content = str_ireplace('crossorigin', '_SiteOne_CO_', $content);
    }

}