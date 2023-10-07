<?php

namespace Crawler\Output;

use Swoole\Coroutine\Http\Client;
use Swoole\Table;

interface Output
{
    public function addBanner(): void;

    public function addUsedOptions(string $finalUserAgent): void;

    public function addTableHeader(): void;

    public function addTableRow(Client $httpClient, string $url, int $status, float $elapsedTime, int $size, int $type, array $extraParsedContent, string $progressStatus): void;

    public function addTotalStats(Table $visited): void;

    public function addNotice(string $text): void;

    public function addError(string $text): void;

    public function getType(): OutputType;

    public function end(): void;
}