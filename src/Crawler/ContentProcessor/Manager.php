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
use Crawler\Result\ManagerStats;
use Exception;

class Manager
{
    const SUPER_TABLE_CONTENT_PROCESSORS_STATS = 'content-processors-stats';

    /**
     * @var ContentProcessor[]
     */
    private array $processors = [];

    private ManagerStats $stats;

    public function __construct()
    {
        $this->stats = new ManagerStats();
    }

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
     * @param int $contentType
     * @param ParsedUrl $url
     * @return FoundUrls[]
     */
    public function findUrls(string $content, int $contentType, ParsedUrl $url): array
    {
        $result = [];
        foreach ($this->processors as $processor) {
            if ($processor->isContentTypeRelevant($contentType)) {
                $s = microtime(true);
                $foundUrls = $processor->findUrls($content, $url);
                if ($foundUrls) {
                    $result[] = $foundUrls;
                }
                $this->stats->measureExecTime($processor::class, 'findUrls', $s);
            }
        }
        return $result;
    }

    /**
     * @param string $content
     * @param int $contentType
     * @param ParsedUrl $url
     * @param bool $removeUnwantedCode
     * @return void
     */
    public function applyContentChangesForOfflineVersion(string &$content, int $contentType, ParsedUrl $url, bool $removeUnwantedCode): void
    {
        foreach ($this->processors as $processor) {
            // break if content is not string (caused by previous processor)
            if (!is_string($content)) {
                break;
            }
            if ($processor->isContentTypeRelevant($contentType)) {
                $s = microtime(true);
                $processor->applyContentChangesForOfflineVersion($content, $contentType, $url, $removeUnwantedCode);
                $this->stats->measureExecTime($processor::class, 'applyContentChangesForOfflineVersion', $s);
            }
        }
    }

    /**
     * @param string $content
     * @param int $contentType
     * @param ParsedUrl $url
     * @return void
     */
    public function applyContentChangesBeforeUrlParsing(string &$content, int $contentType, ParsedUrl $url): void
    {
        foreach ($this->processors as $processor) {
            if ($processor->isContentTypeRelevant($contentType)) {
                $s = microtime(true);
                $processor->applyContentChangesBeforeUrlParsing($content, $contentType, $url);
                $this->stats->measureExecTime($processor::class, 'applyContentChangesBeforeUrlParsing', $s);
            }
        }
    }

    public function getStats(): ManagerStats
    {
        return $this->stats;
    }

}