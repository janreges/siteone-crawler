<?php

namespace Crawler\Analysis;

use Crawler\Analysis\Result\UrlAnalysisResult;
use Crawler\Crawler;
use Crawler\ExtraColumn;
use Crawler\Options\Options;
use Crawler\Output\Output;
use Crawler\Result\Status;
use Crawler\Result\VisitedUrl;
use Exception;

abstract class BaseAnalyzer implements Analyzer
{

    protected Options $config;
    protected Output $output;
    protected Crawler $crawler;
    protected Status $status;

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
    public function analyzeVisitedUrl(VisitedUrl $visitedUrl, ?string $body, ?array $headers): ?UrlAnalysisResult
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
        throw new Exception("Not implemented method shouldBeActivated() in " .  get_class($this));
    }

    /**
     * Get order of this analyzer (lower = earlier)
     * @return int
     * @throws Exception
     */
    public function getOrder(): int
    {
        throw new Exception("Not implemented method getOrder() in " .  get_class($this));
    }

    /**
     * @inheritDoc
     */
    public static function getOptions(): Options
    {
        throw new Exception("Not implemented method getOptions() in " .  get_called_class());
    }
}