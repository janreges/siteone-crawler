<?php

namespace Crawler\Result;

use Crawler\Components\SuperTable;
use Crawler\CoreOptions;
use Crawler\Result\Storage\Storage;
use Crawler\Utils;

class Status
{

    private Storage $storage;
    private CoreOptions $options;
    private bool $saveBodies;
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

    private array $info = [
        'name' => 'SiteOne Website Crawler',
        'version' => null,
        'executedAt' => null,
        'command' => null,
        'hostname' => null,
        'finalUserAgent' => null
    ];

    /**
     * @var VisitedUrl[]
     */
    private array $visitedUrls = [];

    /**
     * @param Storage $storage
     * @param bool $saveBodies
     * @param array $info
     * @param CoreOptions $options
     * @param float $startTime
     */
    public function __construct(Storage $storage, bool $saveBodies, array $info, CoreOptions $options, float $startTime)
    {
        $this->storage = $storage;
        $this->saveBodies = $saveBodies;
        $this->info = array_merge($this->info, $info);
        $this->options = $options;
        $this->startTime = $startTime;

        $this->maskSensitiveInfoData();
    }

    public function addVisitedUrl(VisitedUrl $url, ?string $body): void
    {
        $this->visitedUrls[$url->uqId] = $url;
        if ($this->saveBodies && $body !== null) {
            $this->storage->save($url->uqId, trim($body));
        }
    }

    public function getUrlBody(string $uqId): ?string
    {
        return $this->saveBodies ? $this->storage->load($uqId) : null;
    }

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

    public function getInfo(): array
    {
        return $this->info;
    }

    public function setFinalUserAgent(string $value): void
    {
        $this->info['finalUserAgent'] = $value;
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

    public function maskSensitiveInfoData(): void
    {
        $this->info['command'] = Utils::getSafeCommand($this->info['command']);
    }


}