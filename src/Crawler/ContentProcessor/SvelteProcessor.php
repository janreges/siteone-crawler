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

class SvelteProcessor extends BaseProcessor implements ContentProcessor
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
    public function applyContentChangesForOfflineVersion(string &$content, int $contentType, ParsedUrl $url, bool $removeUnwantedCode): void
    {
        if (str_contains($content, '<svelte:')) {
            $content = preg_replace('/<svelte:[^>]+>\s*/i', '', $content);
        }
    }

    public function isContentTypeRelevant(int $contentType): bool
    {
        return $contentType === Crawler::CONTENT_TYPE_ID_HTML;
    }

}