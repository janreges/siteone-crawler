<?php

namespace Crawler\Export;

use Crawler\Crawler;
use Crawler\Options\Options;
use Crawler\Output\Output;

interface Exporter
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
     * Should this exporter be activated based on options?
     * @return bool
     */
    public function shouldBeActivated(): bool;

    /**
     * Do your export and (save it to file, send it to server, etc.) and call Output methods to show progress
     * @return void
     */
    public function export(): void;

    /**
     * Get options for this exporter
     * @return Options
     */
    public static function getOptions(): Options;
}