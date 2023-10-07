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
    private bool $printToOutput = true;

    private string $outputText = '';

    /**
     * @param string $version
     * @param float $startTime
     * @param Options $options
     * @param string $command
     * @param bool $printToOutput
     */
    public function __construct(string $version, float $startTime, Options $options, string $command, bool $printToOutput)
    {
        $this->version = $version;
        $this->startTime = $startTime;
        $this->options = $options;
        $this->command = $command;
        $this->printToOutput = $printToOutput;
    }

    public function addBanner(): void
    {
        $this->addToOutput("===========================\n");
        $this->addToOutput("= SiteOne Website Crawler =\n");
        $this->addToOutput("= Version: " . $this->version . "      =\n");
        $this->addToOutput("= jan.reges@siteone.cz    =\n");
        $this->addToOutput("===========================\n\n");
    }

    public function addUsedOptions(string $finalUserAgent): void
    {
        // $this->addToOutput("Used options: " . Utils::getColorText(print_r($this->options, true), 'gray') . "\n");
    }

    public function addTableHeader(): void
    {
        $header = str_pad("URL", $this->options->urlColumnSize) . " |" . " Status " . "|" . " Time  " . "|" . " Size     ";
        if (!$this->options->hideProgressBar) {
            $header = str_pad("Progress report", 26) . "| " . $header;
        }

        foreach ($this->options->headersToTable as $headerName) {
            $headerInfo = Utils::getColumnInfo($headerName);
            $header .= " | " . str_pad($headerInfo['name'], max($headerInfo['size'], 4));
        }
        $header .= "\n";
        $this->addToOutput($header . str_repeat("-", strlen(trim($header))) . "\n");
    }

    public function addTableRow(Client $httpClient, string $url, int $status, float $elapsedTime, int $size, int $type, array $extraParsedContent, string $progressStatus): void
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
            $coloredStatus = Utils::getColorText(str_pad(Utils::getHttpClientCodeWithErrorDescription($status, true), 6, ' '), 'red', true);
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
        foreach ($this->options->headersToTable as $header) {
            $value = '';
            $headerInfo = Utils::getColumnInfo($header);
            $headerName = $headerInfo['name'];
            if (array_key_exists($headerName, $extraParsedContent)) {
                $value = trim($extraParsedContent[$headerName]);
            } elseif ($httpClient->headers && array_key_exists(strtolower($headerName), $httpClient->headers)) {
                $value = trim($httpClient->headers[strtolower($headerName)]);
            }

            $extraHeadersContent .= (' | ' . str_pad($value, max($headerInfo['size'], 4)));
        }

        if ($this->options->addRandomQueryParams) {
            $urlForTable .= Utils::getColorText('+%random-query%', 'gray');
        }

        if (!$this->options->doNotTruncateUrl) {
            $urlForTable = Utils::truncateInTwoThirds($urlForTable, $this->options->urlColumnSize, '...');
        }

        // put progress to stderr
        $progressContent = '';
        if (!$this->options->hideProgressBar) {
            list($done, $total) = explode('/', $progressStatus);
            $progressToStdErr = sprintf(
                "%s | %s",
                str_pad($progressStatus, 7),
                Utils::getProgressBar($done, $total, 10)
            );
            $progressContent = str_pad($progressToStdErr, 17);
        }

        $this->addToOutput(trim(sprintf(
                '%s %s | %s | %s | %s %s',
                $progressContent,
                str_pad($urlForTable, $this->options->urlColumnSize),
                $coloredStatus,
                $coloredElapsedTime,
                $coloredSize,
                $extraHeadersContent
            ), '|') . "\n");
    }

    public function addTotalStats(Table $visited): void
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

        $this->addToOutput("\n");
        $resultHeader = "Total execution time: " . Utils::getColorText(number_format(microtime(true) - $this->startTime, 2, '.', ' ') . " sec", 'cyan');
        $this->addToOutput(str_repeat('=', 80) . "\n");
        $this->addToOutput("{$resultHeader}\n");
        $this->addToOutput("Total processed URLs: " . Utils::getColorText($info['totalUrls'], 'cyan') . " with total size " . Utils::getColorText(Utils::getFormattedSize($info['totalSize']), 'cyan') . "\n");
        $this->addToOutput("Response times: "
            . " AVG " . Utils::getColorText(number_format($info['totalTime'] / $info['totalUrls'], 3, '.', ' ') . ' sec', 'magenta', true)
            . " MIN " . Utils::getColorText(number_format($info['minTime'], 3, '.', ' ') . ' sec', 'green', true)
            . " MAX " . Utils::getColorText(number_format($info['maxTime'], 3, '.', ' ') . ' sec', 'red', true)
            . " TOTAL " . Utils::getColorText(number_format($info['totalTime'], 3, '.', ' ') . ' sec', 'cyan', true) . "\n");
        $this->addToOutput("URLs by status:\n");
        ksort($info['countByStatus']);
        $statuses = '';
        foreach ($info['countByStatus'] as $status => $count) {
            $statuses .= " " . Utils::getHttpClientCodeWithErrorDescription($status, false) . ": $count\n";
        }
        $this->addToOutput(Utils::getColorText(rtrim($statuses), 'yellow') . "\n");
        $this->addToOutput(str_repeat('=', 80) . "\n");
    }

    public function addNotice(string $text): void
    {
        $this->addToOutput(Utils::getColorText($text, 'yellow') . "\n");
    }

    public function addError(string $text): void
    {
        $this->addToOutput(Utils::getColorText($text, 'red') . "\n");
    }

    public function end(): void
    {
        // nothing to do
    }

    public function addToOutput(string $output): void
    {
        if ($this->printToOutput) {
            echo $output;
        }

        $this->outputText .= $output;
    }

    public function getOutputText(): string
    {
        return $this->outputText;
    }

    public function getType(): OutputType
    {
        return OutputType::FORMATTED_TEXT;
    }
}