<?php

namespace Crawler\Output;

use Swoole\Coroutine\Http\Client;
use Swoole\Table;

interface Output
{
    public function printBanner(): void;

    public function printUsedOptions(): void;

    public function printTableHeader(): void;

    public function printTableRow(Client $httpClient, string $url, int $status, float $elapsedTime, int $size, array $extraParsedContent): void;

    public function printTotalStats(Table $visited): void;

    public function printError(string $text): void;

    public function printEnd(): void;
}