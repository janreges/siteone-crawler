<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Output;

use Crawler\Components\SuperTable;
use Crawler\ExtraColumn;
use Crawler\HttpClient\HttpResponse;
use Crawler\Result\Summary\Summary;
use Swoole\Table;

interface Output
{
    public function addBanner(): void;

    public function addUsedOptions(): void;

    /**
     * @param ExtraColumn[] $extraColumnsFromAnalysis
     * @return void
     */
    public function setExtraColumnsFromAnalysis(array $extraColumnsFromAnalysis): void;

    public function addTableHeader(): void;

    public function addTableRow(HttpResponse $httpResponse, string $url, int $status, float $elapsedTime, int $size, int $type, array $extraParsedContent, string $progressStatus, int $cacheTypeFlags, ?int $cacheLifetime): void;

    public function addSuperTable(SuperTable $table): void;

    public function addTotalStats(Table $visited): void;

    public function addNotice(string $text): void;

    public function addError(string $text): void;

    public function addSummary(Summary $summary): void;

    public function getType(): OutputType;

    public function end(): void;
}