<?php

namespace Crawler\Output;

use Crawler\Options;
use Crawler\Utils;
use Swoole\Coroutine\Http\Client;
use Swoole\Table;

class FormattedTextOutput implements Output
{

    private string $version;
    private float $startTime;
    private Options $options;
    private string $command;

    /**
     * @param string $version
     * @param float $startTime
     * @param Options $options
     * @param string $command
     */
    public function __construct(string $version, float $startTime, Options $options, string $command)
    {
        $this->version = $version;
        $this->startTime = $startTime;
        $this->options = $options;
        $this->command = $command;
    }

    public function printBanner(): void
    {
        echo "===========================\n";
        echo "= SiteOne Website Crawler =\n";
        echo "= Version: " . $this->version . "      =\n";
        echo "= jan.reges@siteone.cz    =\n";
        echo "===========================\n\n";
    }

    public function printUsedOptions(): void
    {
        // echo "Used options: " . Utils::getColorText(print_r($this->options, true), 'gray') . "\n";
    }

    public function printTableHeader(): void
    {
        $header = str_pad("URL", $this->options->tableUrlColumnSize) . " |" . " Status " . "|" . " Time  " . "|" . " Size     ";
        foreach ($this->options->headersToTable as $headerName) {
            $header .= " | {$headerName}";
        }
        $header .= "\n";
        echo $header . str_repeat("-", strlen(trim($header))) . "\n";
    }

    public function printTableRow(Client $httpClient, string $url, int $status, float $elapsedTime, int $size, array $extraParsedContent, string $progressStatus): void
    {
        $urlForTable = $this->options->hideSchemeAndHost ? (preg_replace('/^https?:\/\/[^\/]+\//i', '/', $url)) : $url;

        if ($status == 200) {
            $coloredStatus = Utils::getColorText(str_pad($status, 6, ' '), 'green');
        } else if ($status > 300 && $status < 400) {
            $coloredStatus = Utils::getColorText(str_pad($status, 6, ' '), 'yellow', true);
        } elseif ($status == 404) {
            $coloredStatus = Utils::getColorText(str_pad($status, 6, ' '), 'magenta', true);
        } elseif ($status == 429) {
            $coloredStatus = Utils::getColorText(str_pad($status, 6, ' '), 'red', true);
        } elseif ($status > 400 && $status < 500) {
            $coloredStatus = Utils::getColorText(str_pad($status, 6, ' '), 'cyan', true);
        } else {
            $coloredStatus = Utils::getColorText(str_pad($status, 6, ' '), 'red', true);
        }

        $coloredElapsedTime = sprintf("%.3f", $elapsedTime);
        if ($coloredElapsedTime >= 2) {
            $coloredElapsedTime = Utils::getColorText($coloredElapsedTime, 'red', true);
        } else if ($coloredElapsedTime >= 1) {
            $coloredElapsedTime = Utils::getColorText($coloredElapsedTime, 'magenta', true);
        }

        $coloredSize =
            $size > 1024 * 1024
                ? Utils::getColorText(str_pad(Utils::getFormattedSize($size), 8), 'red')
                : str_pad(Utils::getFormattedSize($size), 8);

        $extraHeadersContent = '';
        foreach ($this->options->headersToTable as $headerName) {
            $value = '';
            if (array_key_exists($headerName, $extraParsedContent)) {
                $value = trim($extraParsedContent[$headerName]);
            } elseif (array_key_exists(strtolower($headerName), $httpClient->headers)) {
                $value = trim($httpClient->headers[strtolower($headerName)]);
            }
            $extraHeadersContent .= (' | ' . str_pad($value, strlen($headerName)));
        }

        if ($this->options->addRandomQueryParams) {
            $urlForTable .= Utils::getColorText('+%random-query%', 'gray');
        }

        if ($this->options->truncateUrlToColumnSize) {
            $urlForTable = Utils::truncateInTwoThirds($urlForTable, $this->options->tableUrlColumnSize, '...');
        }

        echo trim(sprintf(
                '%s | %s | %s | %s %s',
                str_pad($urlForTable, $this->options->tableUrlColumnSize),
                $coloredStatus,
                $coloredElapsedTime,
                $coloredSize,
                $extraHeadersContent
            ), ' |') . "\n";
    }

    public function printTotalStats(Table $visited): void
    {
        $info = [
            'totalUrls' => $visited->count(),
            'totalSize' => 0,
            'countByStatus' => [],
            'totalTime' => 0,
            'minTime' => null,
            'maxTime' => null,
        ];

        foreach ($visited as $row) {
            $info['totalTime'] += $row['time'];
            $info['totalSize'] += $row['size'];
            $info['countByStatus'][$row['status']] = ($info['countByStatus'][$row['status']] ?? 0) + 1;
            $info['minTime'] = $info['minTime'] === null ? $row['time'] : min($row['time'], $info['minTime']);
            $info['maxTime'] = $info['maxTime'] === null ? $row['time'] : max($row['time'], $info['maxTime']);
        }

        echo "\n";
        $resultHeader = "Total execution time: " . Utils::getColorText(number_format(microtime(true) - $this->startTime, 2, '.', ' ') . " sec", 'cyan');
        echo str_repeat('=', 80) . "\n";
        echo "{$resultHeader}\n";
        echo "Total processed URLs: " . Utils::getColorText($info['totalUrls'], 'cyan') . " with total size " . Utils::getColorText(Utils::getFormattedSize($info['totalSize']), 'cyan') . "\n";
        echo "Response times: "
            . " AVG " . Utils::getColorText(number_format($info['totalTime'] / $info['totalUrls'], 3, '.', ' ') . ' sec', 'magenta', true)
            . " MIN " . Utils::getColorText(number_format($info['minTime'], 3, '.', ' ') . ' sec', 'green', true)
            . " MAX " . Utils::getColorText(number_format($info['maxTime'], 3, '.', ' ') . ' sec', 'red', true)
            . " TOTAL " . Utils::getColorText(number_format($info['totalTime'], 3, '.', ' ') . ' sec', 'cyan', true) . "\n";
        echo "URLs by status:\n";
        ksort($info['countByStatus']);
        $statuses = '';
        foreach ($info['countByStatus'] as $status => $count) {
            $statuses .= " {$status}: $count\n";
        }
        echo Utils::getColorText(rtrim($statuses), 'yellow') . "\n";
        echo str_repeat('=', 80) . "\n";
    }

    public function printError(string $text): void
    {
        echo Utils::getColorText($text, 'red') . "\n";
    }

    public function printEnd(): void
    {
        // nothing to do
    }
}