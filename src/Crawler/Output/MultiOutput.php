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

    public function addUsedOptions(): void
    {
        foreach ($this->outputs as $output) {
            $output->addUsedOptions();
        }
    }

    /**
     * @param ExtraColumn[] $extraColumnsFromAnalysis
     * @return void
     */
    public function setExtraColumnsFromAnalysis(array $extraColumnsFromAnalysis): void
    {
        foreach ($this->outputs as $output) {
            $output->setExtraColumnsFromAnalysis($extraColumnsFromAnalysis);
        }
    }

    public function addTableHeader(): void
    {
        foreach ($this->outputs as $output) {
            $output->addTableHeader();
        }
    }

    public function addTableRow(HttpResponse $httpResponse, string $url, int $status, float $elapsedTime, int $size, int $type, array $extraParsedContent, string $progressStatus, int $cacheTypeFlags, ?int $cacheLifetime): void
    {
        foreach ($this->outputs as $output) {
            $output->addTableRow($httpResponse, $url, $status, $elapsedTime, $size, $type, $extraParsedContent, $progressStatus, $cacheTypeFlags, $cacheLifetime);
        }
    }

    public function addSuperTable(SuperTable $table): void
    {
        foreach ($this->outputs as $output) {
            $output->addSuperTable($table);
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

    public function addSummary(Summary $summary): void
    {
        foreach ($this->outputs as $output) {
            $output->addSummary($summary);
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