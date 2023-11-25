<?php

/*
 * This file is part of the SiteOne Crawler.
 *
 * (c) Ján Regeš <jan.reges@siteone.cz>
 */

declare(strict_types=1);

namespace Crawler\Export;

use Crawler\Crawler;
use Crawler\Options\Options;
use Crawler\Output\Output;
use Crawler\Result\Status;
use Exception;

abstract class BaseExporter implements Exporter
{

    protected Options $config;
    protected Output $output;
    protected Crawler $crawler;
    protected Status $status;

    /**
     * @param Options $options
     * @return void
     * @throws Exception
     */
    public function setConfig(Options $options): void
    {
        $this->config = $options;

        foreach ($options->getGroups() as $group) {
            foreach ($group->options as $option) {
                if (property_exists($this, $option->propertyToFill) && $option->propertyToFill !== 'output') {
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
     * @throws Exception
     */
    public function export(): void
    {
        throw new Exception("Not implemented method export() in " . get_class($this));
    }

    /**
     * @return bool
     * @throws Exception
     */
    public function shouldBeActivated(): bool
    {
        throw new Exception("Not implemented method shouldBeActivated() in " . get_class($this));
    }

    /**
     * @return Options
     * @throws Exception
     */
    public static function getOptions(): Options
    {
        throw new Exception("Not implemented method getOptions() in " . get_called_class());
    }
}