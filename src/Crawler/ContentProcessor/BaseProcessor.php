<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\ContentProcessor;

use Crawler\CoreOptions;
use Crawler\Crawler;
use Crawler\Export\Utils\OfflineUrlConverter;
use Crawler\FoundUrls;
use Crawler\ParsedUrl;

abstract class BaseProcessor implements ContentProcessor
{
    protected Crawler $crawler;
    protected CoreOptions $options;
    protected bool $debugMode = false;
    protected array $relevantContentTypes = [];

    /**
     * @param Crawler $crawler
     */
    public function __construct(Crawler $crawler)
    {
        $this->crawler = $crawler;
        $this->options = $crawler->getCoreOptions();
    }

    /**
     * @inheritDoc
     */
    public function findUrls(string $content, ParsedUrl $sourceUrl): ?FoundUrls
    {
        throw new \Exception(__METHOD__ . ': Not implemented');
    }

    /**
     * @inheritDoc
     */
    public function applyContentChangesForOfflineVersion(string &$content, int $contentType, ParsedUrl $url, bool $removeUnwantedCode): void
    {
        // do nothing = optionally implemented in child classes
    }

    /**
     * @inheritDoc
     */
    public function applyContentChangesBeforeUrlParsing(string &$content, int $contentType, ParsedUrl $url): void
    {
        // do nothing = optionally implemented in child classes
    }

    public function setDebugMode(bool $debugMode): void
    {
        $this->debugMode = $debugMode;
    }

    /**
     * @param ParsedUrl $parsedBaseUrl
     * @param string $targetUrl
     * @param string|null $attribute
     * @return string
     */
    public function convertUrlToRelative(ParsedUrl $parsedBaseUrl, string $targetUrl, ?string $attribute = null): string
    {
        $urlConverter = new OfflineUrlConverter(
            $this->crawler->getInitialParsedUrl(),
            $parsedBaseUrl,
            ParsedUrl::parse($targetUrl, $parsedBaseUrl),
            [$this->crawler, 'isDomainAllowedForStaticFiles'],
            [$this->crawler, 'isExternalDomainAllowedForCrawling'],
            $attribute
        );

        return $urlConverter->convertUrlToRelative(true);
    }

    /**
     * Check if this ContentProcessor is relevant for given content type and will do something with it
     *
     * @param int $contentType See Crawler::CONTENT_TYPE_*
     * @return bool
     */
    public function isContentTypeRelevant(int $contentType): bool
    {
        return in_array($contentType, $this->relevantContentTypes);
    }

}