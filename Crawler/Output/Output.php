<?php

namespace Crawler\Output;

use Crawler\Components\SuperTable;
use Crawler\Result\Summary\Summary;
use Swoole\Coroutine\Http\Client;
use Swoole\Table;

interface Output
{
    public function addBanner(): void;

    public function addUsedOptions(): void;

    public function addTableHeader(): void;

    public function addTableRow(Client $httpClient, string $url, int $status, float $elapsedTime, int $size, int $type, array $extraParsedContent, string $progressStatus): void;

    public function addSuperTable(SuperTable $table): void;

    public function addTotalStats(Table $visited): void;

    public function addNotice(string $text): void;

    public function addError(string $text): void;

    public function addSummary(Summary $summary): void;

    public function getType(): OutputType;

    public function end(): void;
}