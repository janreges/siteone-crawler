<?php

namespace Crawler;

use Crawler\Output\FormattedTextOutput;
use Crawler\Output\JsonOutput;
use Crawler\Output\Output;
use Crawler\Output\OutputType;
use Exception;

class Manager
{

    private string $version;
    private float $startTime;
    private Options $options;
    private Output $output;
    private string $command;

    /**
     * @param string $version
     * @param float $startTime
     * @param Options $options
     * @param string $command
     * @throws Exception
     */
    public function __construct(string $version, float $startTime, Options $options, string $command)
    {
        $this->version = $version;
        $this->startTime = $startTime;
        $this->options = $options;
        $this->command = $command;
        $this->output = $this->getOutputByType($options->outputType);
    }

    /**
     * @return void
     * @throws Exception
     */
    public function run(): void
    {
        $this->output->addBanner();
        $this->output->addUsedOptions();

        $crawler = new Crawler($this->options, $this->output);
        $crawler->run();

        $this->output->end();
    }

    /**
     * @param OutputType $outputType
     * @return Output
     * @throws Exception
     */
    private function getOutputByType(OutputType $outputType): Output
    {
        if ($outputType == OutputType::FORMATTED_TEXT) {
            return new FormattedTextOutput($this->version, $this->startTime, $this->options, $this->command);
        } elseif ($outputType == OutputType::JSON) {
            return new JsonOutput($this->version, $this->startTime, $this->options, $this->command);
        } else {
            throw new Exception("Unknown output type {$this->options->outputType}");
        }
    }

}