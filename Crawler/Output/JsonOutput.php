<?php

namespace Crawler\Output;

use Crawler\Options;
use Crawler\Utils;
use Swoole\Coroutine\Http\Client;
use Swoole\Table;

class JsonOutput implements Output
{

    private string $version;
    private float $startTime;
    private Options $options;
    private string $command;

    private array $json = [];

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
        $this->json['crawler'] = ['name' => 'SiteOne Website Crawler', 'version' => $this->version, 'executedAt' => date('Y-m-d H:i:s'), 'command' => $this->command];
    }

    public function printUsedOptions(): void
    {
        $this->json['options'] = (array)$this->options;
    }

    public function printTotalStats(Table $visited): void
    {
        fwrite(STDERR, "\n\n");

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

        $this->json['stats'] = [
            'totalExecutionTime' => round(microtime(true) - $this->startTime, 3),
            'totalUrls' => $info['totalUrls'],
            'totalSize' => $info['totalSize'],
            'totalSizeFormatted' => Utils::getFormattedSize($info['totalSize']),
            'totalRequestsTimes' => round($info['totalTime'], 3),
            'totalRequestsTimesAvg' => round($info['totalTime'] / $info['totalUrls'], 3),
            'totalRequestsTimesMin' => round($info['minTime'], 3),
            'totalRequestsTimesMax' => round($info['maxTime'], 3),
            'countByStatus' => $info['countByStatus']
        ];
    }

    public function printTableHeader(): void
    {
        $this->json['results'] = [];
    }

    public function printTableRow(Client $httpClient, string $url, int $status, float $elapsedTime, int $size, array $extraParsedContent, string $progressStatus): void
    {
        static $maxStdErrLength = 0;
        $row = [
            'url' => $url,
            'status' => $status,
            'elapsedTime' => round($elapsedTime, 3),
            'size' => $size,
            'extras' => [],
        ];

        foreach ($this->options->headersToTable as $headerName) {
            $value = '';
            if (array_key_exists($headerName, $extraParsedContent)) {
                $value = trim($extraParsedContent[$headerName]);
            } elseif (array_key_exists(strtolower($headerName), $httpClient->headers ?: [])) {
                $value = trim($httpClient->headers[strtolower($headerName)]);
            }
            $row['extras'][$headerName] = $value;
        }

        $this->json['results'][] = $row;

        // put progress to stderr
        list($done, $total) = explode('/', $progressStatus);
        $progressToStdErr = sprintf(
            "\rProgress: %s | %s %s | %s",
            str_pad($progressStatus, 7),
            Utils::getProgressBar($done, $total, 25),
            number_format($elapsedTime, 3, '.') . " sec",
            $url
        );
        $maxStdErrLength = max($maxStdErrLength, strlen($progressToStdErr));
        fwrite(STDERR, str_pad($progressToStdErr, $maxStdErrLength));
    }

    public function printError(string $text): void
    {
        if (!isset($this->json['error'])) {
            $this->json['error'] = [];
        }
        $this->json['error'][] = date('Y-m-d H:i:s') . ' | ' . $text;
    }

    public function printEnd(): void
    {
        $json = json_encode($this->json, JSON_PRETTY_PRINT | JSON_INVALID_UTF8_IGNORE);

        if (!$json) {
            echo "ERROR: unable to parse JSON: " . json_last_error_msg() . "\n";
            print_r($this->json);
        } else {
            echo $json;
        }
    }

}