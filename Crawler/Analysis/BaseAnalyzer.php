<?php

namespace Crawler\Analysis;

use Crawler\Crawler;
use Crawler\Options\Options;
use Crawler\Output\Output;
use Exception;

abstract class BaseAnalyzer implements Analyzer
{

    protected Options $config;
    protected Output $output;
    protected Crawler $crawler;

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
    public function analyze(): void
    {
        throw new Exception("Not implemented method analyze() in " . get_class($this));
    }

    public function shouldBeActivated(): bool
    {
        throw new Exception("Not implemented method shouldBeActivated() in " .  get_class($this));
    }

    /**
     * @inheritDoc
     */
    public static function getOptions(): Options
    {
        throw new Exception("Not implemented method getOptions() in " .  get_called_class());
    }
}