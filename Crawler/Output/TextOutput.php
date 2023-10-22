<?php

namespace Crawler\Output;

use Crawler\Components\SuperTable;
use Crawler\CoreOptions;
use Crawler\HttpClient\HttpResponse;
use Crawler\Result\Status;
use Crawler\Result\Summary\Summary;
use Crawler\Utils;
use Swoole\Table;

class TextOutput implements Output
{

    private string $version;
    private float $startTime;
    private Status $status;
    private CoreOptions $options;
    private string $command;
    private bool $printToOutput = true;

    private string $outputText = '';

    /**
     * @param string $version
     * @param float $startTime
     * @param Status $status
     * @param CoreOptions $options
     * @param string $command
     * @param bool $printToOutput
     */
    public function __construct(string $version, float $startTime, Status $status, CoreOptions $options, string $command, bool $printToOutput)
    {
        $this->version = $version;
        $this->startTime = $startTime;
        $this->status = $status;;
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

    public function addUsedOptions(): void
    {
        // $this->addToOutput("Used options: " . Utils::getColorText(print_r($this->options, true), 'gray') . "\n");
    }

    public function addTableHeader(): void
    {
        $header = str_pad("URL", $this->options->urlColumnSize) . " |" . " Status " . "|" . " Type     " . "|" . " Time   " . "|" . " Size     ";
        if (!$this->options->hideProgressBar) {
            $header = str_pad("Progress report", 26) . "| " . $header;
        }

        foreach ($this->options->extraColumns as $extraColumn) {
            $header .= " | " . str_pad($extraColumn->name, max($extraColumn->getLength(), 4));
        }
        $header .= "\n";
        $this->addToOutput(Utils::getColorText($header, 'gray') . str_repeat("-", strlen($header)) . "\n");
    }

    public function addTableRow(HttpResponse $httpResponse, string $url, int $status, float $elapsedTime, int $size, int $type, array $extraParsedContent, string $progressStatus): void
    {
        $urlForTable = $this->options->hideSchemeAndHost ? (preg_replace('/^https?:\/\/[^\/]+\//i', '/', $url)) : $url;

        $coloredStatus = Utils::getColoredStatusCode($status);
        $contentType = str_pad(Utils::getContentTypeNameById($type), 8);
        $coloredElapsedTime = Utils::getColoredRequestTime($elapsedTime);
        $coloredSize =
            $size > 1024 * 1024
                ? Utils::getColorText(str_pad(Utils::getFormattedSize($size), 8), 'red')
                : str_pad(Utils::getFormattedSize($size), 8);

        $extraHeadersContent = '';
        foreach ($this->options->extraColumns as $extraColumn) {
            $value = '';
            $headerName = $extraColumn->name;
            if (array_key_exists($headerName, $extraParsedContent)) {
                $value = trim($extraParsedContent[$headerName]);
            } elseif ($httpResponse->headers && array_key_exists(strtolower($headerName), $httpResponse->headers)) {
                $value = trim($httpResponse->headers[strtolower($headerName)]);
            }

            $extraHeadersContent .= (' | ' . str_pad($extraColumn->getTruncatedValue($value), max($extraColumn->getLength(), 4)));
        }

        if ($this->options->addRandomQueryParams) {
            $urlForTable .= Utils::getColorText('+%random-query%', 'gray');
        }

        if (!$this->options->doNotTruncateUrl) {
            $urlForTable = Utils::truncateInTwoThirds($urlForTable, $this->options->urlColumnSize);
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
                '%s %s | %s | %s | %s | %s %s',
                $progressContent,
                str_pad($urlForTable, $this->options->urlColumnSize),
                $coloredStatus,
                $contentType,
                $coloredElapsedTime,
                $coloredSize,
                $extraHeadersContent
            ), '|') . "\n");
    }

    public function addSuperTable(SuperTable $table): void
    {
        $this->addToOutput("\n");
        $this->addToOutput($table->getConsoleOutput());
    }

    public function addTotalStats(Table $visited): void
    {
        $stats = $this->status->getBasicStats();

        $this->addToOutput("\n");
        $resultHeader = sprintf(
            "Total execution time %s using %s workers and %s memory limit (max used %s)\n",
            Utils::getColorText(Utils::getFormattedDuration($stats->totalExecutionTime), 'cyan'),
            Utils::getColorText($this->options->workers, 'cyan'),
            Utils::getColorText($this->options->memoryLimit, 'cyan'),
            Utils::getColorText(Utils::getFormattedSize(memory_get_peak_usage(true)), 'cyan')
        );
        $this->addToOutput(str_repeat('=', Utils::getConsoleWidth()) . "\n");
        $this->addToOutput($resultHeader);
        $this->addToOutput(
            sprintf("Total of %s visited URLs with a total size of %s and power of %s with download speed %s\n",
                Utils::getColorText($stats->totalUrls, 'cyan'),
                Utils::getColorText($stats->totalSizeFormatted, 'cyan'),
                Utils::getColorText(intval($stats->totalUrls / $stats->totalExecutionTime) . " reqs/s", 'magenta'),
                Utils::getColorText(Utils::getFormattedSize(intval($stats->totalSize / $stats->totalExecutionTime), 0) . "/s", 'magenta'),
            )
        );
        $this->addToOutput(
            sprintf(
                "Response times: AVG %s MIN %s MAX %s TOTAL %s\n",
                Utils::getColorText(Utils::getFormattedDuration($stats->totalRequestsTimesAvg), 'magenta'),
                Utils::getColorText(Utils::getFormattedDuration($stats->totalRequestsTimesMin), 'green'),
                Utils::getColorText(Utils::getFormattedDuration($stats->totalRequestsTimesMax), 'red'),
                Utils::getColorText(Utils::getFormattedDuration($stats->totalRequestsTimes), 'cyan')
            )
        );

        /*
        $this->addToOutput("URLs by status:\n");
        $statuses = '';
        foreach ($stats->countByStatus as $status => $count) {
            $statuses .= " " . Utils::getHttpClientCodeWithErrorDescription($status) . ": $count\n";
        }
        $this->addToOutput(Utils::getColorText(rtrim($statuses), 'yellow') . "\n");
        */

        $this->addToOutput(str_repeat('=', Utils::getConsoleWidth()) . "\n");
    }

    public function addNotice(string $text): void
    {
        $this->addToOutput(Utils::getColorText($text, 'yellow') . "\n");
    }

    public function addError(string $text): void
    {
        $this->addToOutput(Utils::getColorText($text, 'red') . "\n");
    }

    public function addSummary(Summary $summary): void
    {
        $this->addToOutput("\n");
        $this->addToOutput($summary->getAsConsoleText());
    }

    public function end(): void
    {
        $this->addToOutput("\n");
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
        return OutputType::TEXT;
    }
}