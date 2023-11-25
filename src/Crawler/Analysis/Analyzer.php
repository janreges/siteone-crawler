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

interface Analyzer
{

    /**
     * Set config for whole crawler with already parsed options
     * @param Options $options
     * @return void
     */
    public function setConfig(Options $options): void;

    /**
     * Set output for this exporter to show progress
     * @param Output $output
     * @return void
     */
    public function setOutput(Output $output): void;

    /**
     * Set crawler to get data from it
     * @param Crawler $crawler
     * @return void
     */
    public function setCrawler(Crawler $crawler): void;

    /**
     * Set status to get data from it
     * @param Status $status
     * @return void
     */
    public function setStatus(Status $status): void;

    /**
     * Do your analysis and set results to output
     * @return void
     */
    public function analyze(): void;

    /**
     * Do your analysis right now visited URL. Body and headers are already downloaded and decompressed.
     * Return null if you don't want to analyze this URL, otherwise return UrlAnalysisResult with your results.
     *
     * @param VisitedUrl $visitedUrl
     * @param string|null $body
     * @param DOMDocument|null $dom
     * @param array|null $headers
     * @return UrlAnalysisResult|null
     */
    public function analyzeVisitedUrl(VisitedUrl $visitedUrl, ?string $body, ?DOMDocument $dom, ?array $headers): ?UrlAnalysisResult;

    /**
     * If you want to show URL analysis results in table column (as numbers with severity icons), return name of column
     * under which your results will be shown. This column will be added to the table automatically.
     *
     * @return ExtraColumn|null
     */
    public function showAnalyzedVisitedUrlResultAsColumn(): ?ExtraColumn;

    /**
     * Should this analyzer be activated based on options?
     * @return bool
     */
    public function shouldBeActivated(): bool;

    /**
     * Get order of this analyzer (lower = earlier)
     * @return int
     */
    public function getOrder(): int;

    /**
     * Get options for this analyzer
     * @return Options
     */
    public static function getOptions(): Options;

}