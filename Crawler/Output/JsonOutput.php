<?php

namespace Crawler\Output;

use Crawler\Components\SuperTable;
use Crawler\CoreOptions;
use Crawler\Result\Status;
use Crawler\Utils;
use Swoole\Coroutine\Http\Client;
use Swoole\Table;

class JsonOutput implements Output
{

    private string $version;
    private float $startTime;
    private Status $status;
    private CoreOptions $options;
    private string $command;
    private bool $printToOutput = true;

    private array $json = [];

    /**
     * @param string $version
     * @param float $startTime
     * @param Status $status
     * @param CoreOptions $options
     * @param string $command
     * @param bool $printToOutput
     */
    public function __construct(string $version, float $startTime, Status $status, CoreOptions $options, string $command, bool $printToOutput = true)
    {
        $this->version = $version;
        $this->startTime = $startTime;
        $this->status = $status;
        $this->options = $options;
        $this->command = $command;
        $this->printToOutput = $printToOutput;
    }

    public function addBanner(): void
    {
        $this->json['crawler'] = $this->status->getInfo();
    }

    public function addUsedOptions(string $finalUserAgent): void
    {
        $this->json['options'] = $this->options->toArray();
        $this->json['crawler']['finalUserAgent'] = $finalUserAgent;
    }

    public function addTotalStats(Table $visited): void
    {
        if ($this->printToOutput) {
            fwrite(STDERR, "\n\n");
        }

        $this->json['stats'] = (array)$this->status->getBasicStats();
    }

    public function addTableHeader(): void
    {
        $this->json['results'] = [];
    }

    public function addTableRow(Client $httpClient, string $url, int $status, float $elapsedTime, int $size, int $type, array $extraParsedContent, string $progressStatus): void
    {
        static $maxStdErrLength = 0;
        $row = [
            'url' => $url,
            'status' => Utils::getHttpClientCodeWithErrorDescription($status),
            'elapsedTime' => round($elapsedTime, 3),
            'size' => $size,
            'type' => $type,
            'extras' => [],
        ];

        foreach ($this->options->headersToTable as $header) {
            $value = '';
            $headerInfo = Utils::getColumnInfo($header);
            $headerName = $headerInfo['name'];
            if (array_key_exists($headerName, $extraParsedContent)) {
                $value = trim($extraParsedContent[$headerName]);
            } elseif (array_key_exists(strtolower($headerName), $httpClient->headers ?: [])) {
                $value = trim($httpClient->headers[strtolower($headerName)]);
            }
            $row['extras'][$headerName] = $value;
        }

        $this->json['results'][] = $row;

        if (!$this->options->hideProgressBar && $this->printToOutput) {
            $textWidthWithoutUrl = 65;

            // put progress to stderr
            list($done, $total) = explode('/', $progressStatus);
            $progressToStdErr = sprintf(
                "\rProgress: %s | %s %s | %s",
                str_pad($progressStatus, 7),
                Utils::getProgressBar($done, $total, 25),
                number_format($elapsedTime, 3) . " sec",
                Utils::truncateInTwoThirds($url, Utils::getConsoleWidth() - $textWidthWithoutUrl)
            );
            $maxStdErrLength = max($maxStdErrLength, strlen($progressToStdErr));
            $progressContent = str_pad($progressToStdErr, $maxStdErrLength);

            // cygwin does not support stderr, so we just print status to stdout
            if (stripos(PHP_OS, 'CYGWIN') !== false) {
                echo $progressContent . "\n";
            } else {
                fwrite(STDERR, $progressContent);
            }
        }
    }

    public function addSuperTable(SuperTable $table): void
    {
        if (!isset($this->json['tables'])) {
            $this->json['tables'] = [];
        }
        $this->json['tables'][$table->aplCode] = $table->getJsonOutput();
    }

    public function addNotice(string $text): void
    {
        if (!isset($this->json['notice'])) {
            $this->json['notice'] = [];
        }
        $this->json['notice'][] = date('Y-m-d H:i:s') . ' | ' . $text;
    }

    public function addError(string $text): void
    {
        if (!isset($this->json['error'])) {
            $this->json['error'] = [];
        }
        $this->json['error'][] = date('Y-m-d H:i:s') . ' | ' . $text;
    }

    public function getJson(): string
    {
        return json_encode($this->json, JSON_PRETTY_PRINT | JSON_INVALID_UTF8_IGNORE);
    }

    public function end(): void
    {
        if (!$this->printToOutput) {
            return;
        }

        $json = $this->getJson();
        if (!$json) {
            echo "ERROR: unable to parse JSON: " . json_last_error_msg() . "\n";
        } else {
            echo $json;
        }
    }

    public function getType(): OutputType
    {
        return OutputType::JSON;
    }

    public function getUrlsForSitemap(int $onlyType): array
    {
        return array_column(array_filter($this->json['results'], function ($row) use ($onlyType) {
            return $row['status'] === '200' && $row['type'] === $onlyType;
        }), 'url');
    }

}