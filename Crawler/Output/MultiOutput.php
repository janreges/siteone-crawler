<?php

namespace Crawler\Output;

use Swoole\Coroutine\Http\Client;
use Swoole\Table;

class MultiOutput implements Output
{

    /**
     * @var Output[]
     */
    private array $outputs = [];

    public function addOutput(Output $output): void
    {
        $this->outputs[] = $output;
    }

    /**
     * @return Output[]
     */
    public function getOutputs(): array
    {
        return $this->outputs;
    }

    public function getOutputByType(OutputType $type): ?Output
    {
        foreach ($this->outputs as $output) {
            if ($output->getType() === $type) {
                return $output;
            }
        }
        return null;
    }

    public function addBanner(): void
    {
        foreach ($this->outputs as $output) {
            $output->addBanner();
        }
    }

    public function addUsedOptions(string $finalUserAgent): void
    {
        foreach ($this->outputs as $output) {
            $output->addUsedOptions($finalUserAgent);
        }
    }

    public function addTableHeader(): void
    {
        foreach ($this->outputs as $output) {
            $output->addTableHeader();
        }
    }

    public function addTableRow(Client $httpClient, string $url, int $status, float $elapsedTime, int $size, int $type, array $extraParsedContent, string $progressStatus): void
    {
        foreach ($this->outputs as $output) {
            $output->addTableRow($httpClient, $url, $status, $elapsedTime, $size, $type, $extraParsedContent, $progressStatus);
        }
    }

    public function addTotalStats(Table $visited): void
    {
        foreach ($this->outputs as $output) {
            $output->addTotalStats($visited);
        }
    }

    public function addNotice(string $text): void
    {
        foreach ($this->outputs as $output) {
            $output->addNotice($text);
        }
    }

    public function addError(string $text): void
    {
        foreach ($this->outputs as $output) {
            $output->addError($text);
        }
    }

    public function getType(): OutputType
    {
        return OutputType::MULTI;
    }

    public function end(): void
    {
        foreach ($this->outputs as $output) {
            $output->end();
        }
    }
}