<?php

namespace Crawler\Result;

use Crawler\Components\SuperTable;
use Crawler\CoreOptions;
use Crawler\Info;
use Crawler\Result\Storage\Storage;
use Crawler\Utils;

class Status
{

    private Storage $storage;
    private CoreOptions $options;
    private bool $saveContent;
    private float $startTime;

    private ?BasicStats $basicStats = null;

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
     * @param Storage $storage
     * @param bool $saveContent
     * @param Info $crawlerInfo
     * @param CoreOptions $options
     * @param float $startTime
     */
    public function __construct(Storage $storage, bool $saveContent, Info $crawlerInfo, CoreOptions $options, float $startTime)
    {
        $this->storage = $storage;
        $this->saveContent = $saveContent;
        $this->crawlerInfo = $crawlerInfo;
        $this->options = $options;
        $this->startTime = $startTime;
    }

    public function addVisitedUrl(VisitedUrl $url, ?string $body): void
    {
        $this->visitedUrls[$url->uqId] = $url;
        if ($this->saveContent && $body !== null) {
            $this->storage->save($url->uqId, trim($body));
        }
    }

    public function getUrlBody(string $uqId): ?string
    {
        return $this->saveContent ? $this->storage->load($uqId) : null;
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

    public function getUrlByUqId(string $uqId): ?string
    {
        return $this->visitedUrls[$uqId]->url ?? null;
    }

}