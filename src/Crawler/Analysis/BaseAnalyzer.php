<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Analysis;

use Crawler\Analysis\Result\UrlAnalysisResult;
use Crawler\Crawler;
use Crawler\ExtraColumn;
use Crawler\Options\Options;
use Crawler\Output\Output;
use Crawler\Result\Status;
use Crawler\Result\VisitedUrl;
use DOMDocument;
use Exception;

abstract class BaseAnalyzer implements Analyzer
{

    protected Options $config;
    protected Output $output;
    protected Crawler $crawler;
    protected Status $status;

    /**
     * Total exec times of analyzer methods
     * @var array [string => int]
     */
    protected array $execTimes = [];

    /**
     * Total exec counts of analyzer methods
     * @var array [string => int]
     */
    protected array $execCounts = [];

    /**
     * @inheritDoc
     * @throws Exception
     */
    public function setConfig(Options $options): void
    {
        $this->config = $options;

        foreach ($options->getGroups() as $group) {
            foreach ($group->options as $option) {
                if (property_exists($this, $option->propertyToFill)) {
                    $this->{$option->propertyToFill} = $option->getValue();
                }
            }
        }
    }

    /**
     * @inheritDoc
     */
    public function setOutput(Output $output): void
    {
        $this->output = $output;
    }

    /**
     * @inheritDoc
     */
    public function setCrawler(Crawler $crawler): void
    {
        $this->crawler = $crawler;
    }

    /**
     * @inheritDoc
     */
    public function setStatus(Status $status): void
    {
        $this->status = $status;
    }

    /**
     * @inheritDoc
     */
    public function analyze(): void
    {
        throw new Exception("Not implemented method analyze() in " . get_class($this));
    }

    /**
     * @inheritDoc
     */
    public function analyzeVisitedUrl(VisitedUrl $visitedUrl, ?string $body, ?DOMDocument $dom, ?array $headers): ?UrlAnalysisResult
    {
        // you can override this method in your analyzer
        return null;
    }

    /**
     * @return ExtraColumn|null
     */
    public function showAnalyzedVisitedUrlResultAsColumn(): ?ExtraColumn
    {
        // you can override this method in your analyzer if you want to show results from analyzeVisitedUrl() in table
        return null;
    }


    public function shouldBeActivated(): bool
    {
        throw new Exception("Not implemented method shouldBeActivated() in " . get_class($this));
    }

    /**
     * Get order of this analyzer (lower = earlier)
     * @return int
     * @throws Exception
     */
    public function getOrder(): int
    {
        throw new Exception("Not implemented method getOrder() in " . get_class($this));
    }

    /**
     * Measure and increment exec time and count of analyzer method
     *
     * @param string $class
     * @param string $method
     * @param float $startTime
     * @return void
     */
    protected function measureExecTime(string $class, string $method, float $startTime): void
    {
        $endTime = microtime(true);
        $key = $class . '::' . $method;

        if (!isset($this->execTimes[$key])) {
            $this->execTimes[$key] = 0;
        }
        if (!isset($this->execCounts[$key])) {
            $this->execCounts[$key] = 0;
        }

        $this->execTimes[$key] += ($endTime - $startTime);
        $this->execCounts[$key]++;
    }

    /**
     * @return array [string => int]
     */
    public function getExecTimes(): array
    {
        return $this->execTimes;
    }

    /**
     * @return array [string => int]
     */
    public function getExecCounts(): array
    {
        return $this->execCounts;
    }

    /**
     * @inheritDoc
     */
    public static function getOptions(): Options
    {
        throw new Exception("Not implemented method getOptions() in " . get_called_class());
    }
}