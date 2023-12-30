<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\ContentProcessor;

use Crawler\Crawler;
use Crawler\FoundUrl;
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
        return $this->findUrlsImportFrom($content, $sourceUrl);
    }

    /**
     * @inheritDoc
     */
    public function applyContentChangesForOfflineVersion(string &$content, int $contentType, ParsedUrl $url): void
    {
        $content = str_ireplace('crossorigin', '_SiteOne_CO_', $content);
    }

    /**
     * Find URLs in JavaScript import from statements
     * Example JS content ...import{R as W}from"./Repl.209fef3e.js";...
     *
     * @param string $content
     * @param ParsedUrl $sourceUrl
     * @return FoundUrls|null
     */
    private function findUrlsImportFrom(string $content, ParsedUrl $sourceUrl): ?FoundUrls
    {
        if (!str_contains($content, 'from')) {
            return null;
        }

        preg_match_all('/from\s*["\']([^"\']+\.js[^"\']*)["\']/i', $content, $matches);
        $foundUrlsTxt = [];
        foreach ($matches[1] ?? [] as $match) {
            $foundUrlsTxt[] = trim($match);
        }

        if (!$foundUrlsTxt) {
            return null;
        }

        $foundUrls = new FoundUrls();
        $foundUrls->addUrlsFromTextArray($foundUrlsTxt, $sourceUrl->path, FoundUrl::SOURCE_JS_URL);
        return $foundUrls;
    }

}