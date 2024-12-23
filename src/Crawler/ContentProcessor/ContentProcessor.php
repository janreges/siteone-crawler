<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\ContentProcessor;

use Crawler\FoundUrls;
use Crawler\ParsedUrl;

interface ContentProcessor
{

    /**
     * Parse and find framework specific URLs in HTML/CSS/JS
     * E.g. for NextJS it will find all URLs in buildManifest.js, NextJS arrays/objects, etc.
     *
     * @param string $content
     * @param ParsedUrl $sourceUrl
     * @return FoundUrls|null
     */
    public function findUrls(string $content, ParsedUrl $sourceUrl): ?FoundUrls;

    /**
     * Apply content changes for HTML/CSS/JS before URL parsing, directly to $content passed by reference
     * Method is called by manager only if isContentTypeRelevant() returns true
     *
     * @param string $content
     * @param int $contentType
     * @param ParsedUrl $url
     * @return void
     */
    public function applyContentChangesBeforeUrlParsing(string &$content, int $contentType, ParsedUrl $url): void;

    /**
     * Apply content changes for offline version of the file, directly to $content (HTML/CSS/JS) passed by reference
     * Method is called by manager only if isContentTypeRelevant() returns true
     *
     * @param string $content
     * @param int $contentType See Crawler::CONTENT_TYPE_*
     * @param ParsedUrl $url
     * @param bool $removeUnwantedCode
     * @return void
     */
    public function applyContentChangesForOfflineVersion(string &$content, int $contentType, ParsedUrl $url, bool $removeUnwantedCode): void;

    /**
     * Check if this ContentProcessor is relevant for given content type and will do something with it
     *
     * @param int $contentType See Crawler::CONTENT_TYPE_*
     * @return bool
     */
    public function isContentTypeRelevant(int $contentType): bool;

    /**
     * Enable/disable debug debug
     *
     * @param bool $debugMode
     * @return void
     */
    public function setDebugMode(bool $debugMode): void;

}