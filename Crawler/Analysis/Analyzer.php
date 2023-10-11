<?php

namespace Crawler\Analysis;

use Crawler\Crawler;
use Crawler\Options\Options;
use Crawler\Output\Output;
use Crawler\Result\Status;

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