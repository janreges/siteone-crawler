<?php

/*
 * This file is part of the SiteOne Website Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\ContentProcessor;

use Crawler\FoundUrls;
use Crawler\ParsedUrl;
use Exception;

class Manager
{
    /**
     * @var ContentProcessor[]
     */
    private array $processors = [];

    /**
     * @param ContentProcessor $processor
     * @return void
     * @throws Exception
     */
    public function registerProcessor(ContentProcessor $processor): void
    {
        $className = get_class($processor);
        if (isset($this->processors[$className])) {
            throw new Exception("Content processor {$className} is already registered");
        }

        $this->processors[$className] = $processor;
    }

    public function getProcessors(): array
    {
        return $this->processors;
    }

    /**
     * @param string $content
     * @param ParsedUrl $url
     * @return FoundUrls[]
     */
    public function findUrls(string $content, ParsedUrl $url): array
    {
        $result = [];
        foreach ($this->processors as $processor) {
            $foundUrls = $processor->findUrls($content, $url);
            if ($foundUrls) {
                $result[] = $foundUrls;
            }
        }
        return $result;
    }

    /**
     * @param string $content
     * @param int $contentType
     * @param ParsedUrl $url
     * @return void
     */
    public function applyContentChangesForOfflineVersion(string &$content, int $contentType, ParsedUrl $url): void
    {
        foreach ($this->processors as $processor) {
            if ($processor->isContentTypeRelevant($contentType)) {
                $processor->applyContentChangesForOfflineVersion($content, $contentType, $url);
            }
        }
    }

}