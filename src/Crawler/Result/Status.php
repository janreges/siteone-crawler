<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Result;

use Crawler\Analysis\Result\UrlAnalysisResult;
use Crawler\Components\SuperTable;
use Crawler\CoreOptions;
use Crawler\Crawler;
use Crawler\Info;
use Crawler\Result\Storage\Storage;
use Crawler\Result\Summary\Item;
use Crawler\Result\Summary\ItemStatus;
use Crawler\Result\Summary\Summary;
use Crawler\Utils;

class Status
{

    /**
     * Content storage (memory or file) - used only if storeContent is true
     * Use FileStorage for large crawls with many pages when memory is not enough for all pages content
     *
     * @var Storage $storage
     */
    private Storage $storage;
    private CoreOptions $options;

    /**
     * Store content of visited URLs (HTML, CSS, JS, images, ...) to storage. For example analyzer needs it.
     * @var bool $storeContent
     */
    private bool $storeContent;
    private float $startTime;

    /**
     * Basic stats/metrics about visited URLs
     * @var BasicStats|null
     */
    private ?BasicStats $basicStats = null;

    /**
     * Overall summary of the crawl - each analyzer/exporter can add its own highlighted items
     * @var Summary
     */
    private Summary $summary;

    /**
     * SuperTables that are at the beginning of the page
     * @var SuperTable[]
     */
    private array $superTablesAtBeginning = [];

    /**
     * SuperTables that are at the end of the page
     * @var SuperTable[]
     */
    private array $superTablesAtEnd = [];

    /**
     * Crawler info
     * @var Info
     */
    private Info $crawlerInfo;

    /**
     * @var VisitedUrl[]
     */
    private array $visitedUrls = [];

    /**
     * @var array [string => UrlAnalysisResult[]]
     */
    private array $visitedUrlToAnalysisResult = [];

    /**
     * Robots.txt content - key is %scheme%://%host%:%port%
     * @var array <string, string>
     */
    private static array $robotsTxtContent = [];

    /**
     * @param Storage $storage
     * @param bool $storeContent
     * @param Info $crawlerInfo
     * @param CoreOptions $options
     * @param float $startTime
     */
    public function __construct(Storage $storage, bool $storeContent, Info $crawlerInfo, CoreOptions $options, float $startTime)
    {
        $this->storage = $storage;
        $this->storeContent = $storeContent;
        $this->crawlerInfo = $crawlerInfo;
        $this->options = $options;
        $this->startTime = $startTime;
        $this->summary = new Summary();
    }

    public function addVisitedUrl(VisitedUrl $url, ?string $body, ?array $headers): void
    {
        $this->visitedUrls[$url->uqId] = $url;
        if ($this->storeContent && $body !== null) {
            $this->storage->save($url->uqId, $url->contentType === Crawler::CONTENT_TYPE_ID_HTML ? trim($body) : $body);
            $this->storage->save($url->uqId . '.headers', serialize($headers));
        }
    }

    public function addSummaryItemByRanges(string $aplCode, float $value, array $ranges, array $textPerRange): void
    {
        $status = ItemStatus::INFO;
        $text = "{$aplCode} out of range ({$value})";
        foreach ($ranges as $rangeId => $range) {
            if ($value >= $range[0] && $value <= $range[1]) {
                $status = ItemStatus::fromRangeId($rangeId);
                $text = sprintf($textPerRange[$rangeId] ?? $text, $value);
                break;
            }
        }
        $this->summary->addItem(new Item($aplCode, $text, $status));
    }

    public function addOkToSummary(string $aplCode, string $text): void
    {
        $this->summary->addItem(new Item($aplCode, $text, ItemStatus::OK));
    }

    public function addNoticeToSummary(string $aplCode, string $text): void
    {
        $this->summary->addItem(new Item($aplCode, $text, ItemStatus::NOTICE));
    }

    public function addInfoToSummary(string $aplCode, string $text): void
    {
        $this->summary->addItem(new Item($aplCode, $text, ItemStatus::INFO));
    }

    public function addWarningToSummary(string $aplCode, string $text): void
    {
        $this->summary->addItem(new Item($aplCode, $text, ItemStatus::WARNING));
    }

    public function addCriticalToSummary(string $aplCode, string $text): void
    {
        $this->summary->addItem(new Item($aplCode, $text, ItemStatus::CRITICAL));
    }

    public function getSummary(): Summary
    {
        return $this->summary;
    }

    public function getUrlBody(string $uqId): ?string
    {
        return $this->storeContent ? $this->storage->load($uqId) : null;
    }

    public function getUrlHeaders(string $uqId): ?array
    {
        $serialized = $this->storage->load($uqId . '.headers');
        if ($serialized) {
            return unserialize($serialized);
        }
        return null;
    }

    /**
     * @return VisitedUrl[]
     */
    public function getVisitedUrls(): array
    {
        return $this->visitedUrls;
    }

    public function getOptions(): CoreOptions
    {
        return $this->options;
    }

    public function getOption(string $option): mixed
    {
        return $this->options->{$option} ?? null;
    }

    public function getCrawlerInfo(): Info
    {
        return $this->crawlerInfo;
    }

    public function getStorage(): Storage
    {
        return $this->storage;
    }

    public function setFinalUserAgent(string $value): void
    {
        $this->crawlerInfo->finalUserAgent = $value;
    }

    public function getBasicStats(): BasicStats
    {
        if (!$this->basicStats) {
            $this->basicStats = BasicStats::fromVisitedUrls($this->visitedUrls, $this->startTime);
        }

        return $this->basicStats;
    }

    public function addSuperTableAtBeginning(SuperTable $superTable): void
    {
        $this->superTablesAtBeginning[$superTable->aplCode] = $superTable;
    }

    public function addSuperTableAtEnd(SuperTable $superTable): void
    {
        $this->superTablesAtEnd[$superTable->aplCode] = $superTable;
    }

    /**
     * @return SuperTable[]
     */
    public function getSuperTablesAtBeginning(): array
    {
        return $this->superTablesAtBeginning;
    }

    /**
     * @return SuperTable[]
     */
    public function getSuperTablesAtEnd(): array
    {
        return $this->superTablesAtEnd;
    }

    public function getSuperTableByAplCode(string $aplCode): ?SuperTable
    {
        foreach ($this->superTablesAtBeginning as $superTable) {
            if ($superTable->aplCode === $aplCode) {
                return $superTable;
            }
        }
        foreach ($this->superTablesAtEnd as $superTable) {
            if ($superTable->aplCode === $aplCode) {
                return $superTable;
            }
        }
        return null;
    }

    public function getUrlByUqId(string $uqId): ?string
    {
        return $this->visitedUrls[$uqId]->url ?? null;
    }

    public function getOriginHeaderValueBySourceUqId(string $sourceUiqId): ?string
    {
        $visitedUrl = $this->visitedUrls[$sourceUiqId] ?? null;
        if ($visitedUrl === null) {
            return null;
        }

        return preg_replace('/^(https?:\/\/[^\/]+)/i', '$1', $visitedUrl->url);
    }

    public function addUrlAnalysisResultForVisitedUrl(string $visitedUrlUqId, UrlAnalysisResult $urlAnalysisResult): void
    {
        $this->visitedUrlToAnalysisResult[$visitedUrlUqId][] = $urlAnalysisResult;
    }

    public function getUrlAnalysisResultsForVisitedUrl(string $visitedUrlUqId): array
    {
        return $this->visitedUrlToAnalysisResult[$visitedUrlUqId] ?? [];
    }

    public function getDetailsByAnalysisNameAndSeverity(string $analysisName, string $severity): array
    {
        $result = [];
        foreach ($this->visitedUrlToAnalysisResult as $visitedUrlUqId => $analysisResults) {
            foreach ($analysisResults as $analysisResult) {
                /** @var UrlAnalysisResult $analysisResult */
                $result = array_merge($result, $analysisResult->getDetailsOfSeverityAndAnalysisName($analysisName, $severity));
            }
        }
        return $result;
    }

    public function getVisitedUrlToAnalysisResult(): array
    {
        return $this->visitedUrlToAnalysisResult;
    }

    /**
     * Get number of visited URLs with HTTP code >= 200
     * @return int
     */
    public function getNumberOfWorkingVisitedUrls(): int
    {
        $count = 0;
        foreach ($this->visitedUrls as $visitedUrl) {
            if ($visitedUrl->statusCode >= 200) {
                $count++;
            }
        }
        return $count;
    }

    /**
     * @param string $scheme
     * @param string $host
     * @param int $port
     * @param string $robotsTxtContent
     * @return void
     */
    public static function setRobotsTxtContent(string $scheme, string $host, int $port, string $robotsTxtContent): void
    {
        $key = "{$scheme}://{$host}:{$port}";
        self::$robotsTxtContent[$key] = $robotsTxtContent;
    }

    /**
     * @param string $scheme
     * @param string $host
     * @param int $port
     * @return string|null
     */
    public static function getRobotsTxtContent(string $scheme, string $host, int $port): ?string
    {
        $key = "{$scheme}://{$host}:{$port}";
        return self::$robotsTxtContent[$key] ?? null;
    }

}